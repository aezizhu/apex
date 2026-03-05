import { useMemo, useCallback } from 'react'
import Plot from 'react-plotly.js'
import { type Task } from '@/lib/store'

interface TaskTimelineProps {
  tasks: Task[]
  onTaskClick?: (taskId: string) => void
}

const STATUS_COLORS: Record<string, string> = {
  running: '#3b82f6',
  completed: '#10b981',
  failed: '#ef4444',
  pending: '#6b7280',
  ready: '#f59e0b',
  cancelled: '#64748b',
}

export function TaskTimeline({ tasks, onTaskClick }: TaskTimelineProps) {
  const chartData = useMemo(() => {
    if (tasks.length === 0) return null

    // Sort tasks by creation time
    const sorted = [...tasks].sort(
      (a, b) => new Date(a.createdAt).getTime() - new Date(b.createdAt).getTime()
    )

    // Build Gantt-style horizontal bars using Plotly bar chart
    // Each task gets a row (y axis) and a horizontal bar from start to end
    const now = new Date()

    const taskNames: string[] = []
    const starts: number[] = []
    const durations: number[] = []
    const colors: string[] = []
    const hoverTexts: string[] = []
    const taskIds: string[] = []

    for (const task of sorted) {
      const startTime = task.startedAt
        ? new Date(task.startedAt)
        : new Date(task.createdAt)
      const endTime = task.completedAt
        ? new Date(task.completedAt)
        : task.status === 'running'
          ? now
          : new Date(startTime.getTime() + 5000) // Default 5s bar for pending

      const durationMs = endTime.getTime() - startTime.getTime()
      const durationSec = Math.max(durationMs / 1000, 0.5)

      const label =
        task.name.length > 30 ? task.name.slice(0, 27) + '...' : task.name

      taskNames.push(label)
      starts.push(startTime.getTime())
      durations.push(durationSec)
      colors.push(STATUS_COLORS[task.status] ?? '#6b7280')
      taskIds.push(task.id)

      const durationStr =
        durationMs < 1000
          ? `${durationMs}ms`
          : durationMs < 60000
            ? `${(durationMs / 1000).toFixed(1)}s`
            : `${(durationMs / 60000).toFixed(1)}m`

      const costStr =
        task.costDollars < 0.01
          ? `$${task.costDollars.toFixed(6)}`
          : task.costDollars < 1
            ? `$${task.costDollars.toFixed(4)}`
            : `$${task.costDollars.toFixed(2)}`

      const tokenStr =
        task.tokensUsed < 1000
          ? `${task.tokensUsed}`
          : `${(task.tokensUsed / 1000).toFixed(1)}K`

      hoverTexts.push(
        `<b>${task.name}</b><br>` +
          `Status: ${task.status}<br>` +
          `Agent: ${task.agentId?.slice(0, 8) ?? 'Unassigned'}<br>` +
          `Duration: ${durationStr}<br>` +
          `Tokens: ${tokenStr}<br>` +
          `Cost: ${costStr}`
      )
    }

    // Convert starts to Date strings for Plotly
    const startDates = starts.map((s) => new Date(s).toISOString())
    // Convert durations to milliseconds for the bar width
    const durationMs = durations.map((d) => d * 1000)

    return { taskNames, startDates, durationMs, colors, hoverTexts, taskIds }
  }, [tasks])

  const handleClick = useCallback(
    (event: Readonly<Plotly.PlotMouseEvent>) => {
      if (!onTaskClick || !chartData) return
      const point = event.points[0]
      if (point) {
        const idx = point.pointIndex
        const taskId = chartData.taskIds[idx]
        if (taskId) {
          onTaskClick(taskId)
        }
      }
    },
    [onTaskClick, chartData]
  )

  if (!chartData || tasks.length === 0) {
    return (
      <div className="flex items-center justify-center h-[500px] text-apex-text-secondary">
        No tasks to display in timeline
      </div>
    )
  }

  // Build traces grouped by status for the legend
  const statusGroups = new Map<string, number[]>()
  for (let i = 0; i < chartData.colors.length; i++) {
    const status = tasks[i]?.status ?? 'pending'
    const existing = statusGroups.get(status)
    if (existing) {
      existing.push(i)
    } else {
      statusGroups.set(status, [i])
    }
  }

  const traces: Plotly.Data[] = []

  for (const [status, indices] of statusGroups) {
    const baseValues = indices.map((i) => chartData.startDates[i] ?? '')
    const widths = indices.map((i) => chartData.durationMs[i] ?? 0)
    const yLabels = indices.map((i) => chartData.taskNames[i] ?? '')
    const hoverTexts = indices.map((i) => chartData.hoverTexts[i] ?? '')

    const trace: Record<string, unknown> = {
      type: 'bar',
      orientation: 'h',
      name: status.charAt(0).toUpperCase() + status.slice(1),
      y: yLabels,
      x: widths,
      base: baseValues,
      marker: {
        color: STATUS_COLORS[status] ?? '#6b7280',
        line: {
          color: 'rgba(255,255,255,0.1)',
          width: 1,
        },
      },
      hovertext: hoverTexts,
      hoverinfo: 'text',
      hoverlabel: {
        bgcolor: '#1a1a2e',
        bordercolor: '#3a3a4e',
        font: { color: '#f8fafc', size: 12, family: 'Inter, system-ui' },
      },
    }
    traces.push(trace as Plotly.Data)
  }

  // Limit visible tasks for performance
  const visibleCount = Math.min(tasks.length, 40)
  const chartHeight = Math.max(300, visibleCount * 28 + 100)

  return (
    <div className="w-full overflow-auto">
      <Plot
        data={traces}
        layout={{
          autosize: true,
          height: chartHeight,
          margin: { l: 200, r: 40, t: 20, b: 60 },
          paper_bgcolor: 'transparent',
          plot_bgcolor: 'transparent',
          font: { color: '#94a3b8', size: 11, family: 'Inter, system-ui' },
          barmode: 'overlay',
          showlegend: true,
          legend: {
            x: 0,
            y: 1.05,
            orientation: 'h',
            bgcolor: 'transparent',
            font: { color: '#f8fafc', size: 12 },
          },
          xaxis: {
            type: 'date',
            showgrid: true,
            gridcolor: '#1a1a2e',
            tickformat: '%H:%M:%S',
            tickfont: { color: '#64748b', size: 10 },
            zeroline: false,
          },
          yaxis: {
            autorange: 'reversed',
            showgrid: false,
            tickfont: { color: '#94a3b8', size: 11, family: 'JetBrains Mono, monospace' },
            fixedrange: true,
          },
          hovermode: 'closest',
          dragmode: 'pan',
        }}
        config={{
          displayModeBar: false,
          responsive: true,
          scrollZoom: true,
        }}
        style={{ width: '100%', height: '100%' }}
        onClick={handleClick}
      />
    </div>
  )
}
