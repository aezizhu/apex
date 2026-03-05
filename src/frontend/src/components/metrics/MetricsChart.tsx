import { useState, useEffect, useRef, useCallback } from 'react'
import Plot from 'react-plotly.js'
import { metricsApi } from '@/lib/api'

interface MetricsSnapshot {
  timestamp: string
  taskCompletions: number
  avgLatency: number
}

const MAX_DATA_POINTS = 60

export default function MetricsChart() {
  const [history, setHistory] = useState<MetricsSnapshot[]>([])
  const prevMetrics = useRef<{ completedTasks: number }>({ completedTasks: 0 })

  const fetchAndAppend = useCallback(async () => {
    try {
      const response = await metricsApi.getSystem()
      const m = response.data
      if (!m) return

      const completedDelta = prevMetrics.current.completedTasks > 0
        ? Math.max(0, m.completedTasks - prevMetrics.current.completedTasks)
        : 0
      prevMetrics.current = { completedTasks: m.completedTasks }

      const snapshot: MetricsSnapshot = {
        timestamp: new Date().toISOString(),
        taskCompletions: completedDelta,
        avgLatency: m.avgLatencyMs,
      }

      setHistory((prev) => {
        const next = [...prev, snapshot]
        return next.length > MAX_DATA_POINTS ? next.slice(-MAX_DATA_POINTS) : next
      })
    } catch {
      // Silently skip failed metric fetches
    }
  }, [])

  // Fetch on mount and every 30s
  useEffect(() => {
    fetchAndAppend()
    const interval = setInterval(fetchAndAppend, 30000)
    return () => clearInterval(interval)
  }, [fetchAndAppend])

  const data = {
    timestamps: history.map((s) => s.timestamp),
    taskCompletions: history.map((s) => s.taskCompletions),
    avgLatency: history.map((s) => s.avgLatency),
  }

  return (
    <Plot
      data={[
        {
          x: data.timestamps,
          y: data.taskCompletions,
          type: 'scatter',
          mode: 'lines',
          name: 'Tasks Completed',
          line: { color: '#10b981', width: 2 },
          fill: 'tozeroy',
          fillcolor: 'rgba(16, 185, 129, 0.1)',
        },
        {
          x: data.timestamps,
          y: data.avgLatency,
          type: 'scatter',
          mode: 'lines',
          name: 'Avg Latency (ms)',
          line: { color: '#3b82f6', width: 2 },
          yaxis: 'y2',
        },
      ]}
      layout={{
        autosize: true,
        margin: { l: 60, r: 60, t: 20, b: 40 },
        paper_bgcolor: 'transparent',
        plot_bgcolor: 'transparent',
        font: { color: '#94a3b8', size: 11 },
        showlegend: true,
        legend: {
          x: 0,
          y: 1.1,
          orientation: 'h',
          bgcolor: 'transparent',
        },
        xaxis: {
          showgrid: true,
          gridcolor: '#1a1a2e',
          tickformat: '%H:%M',
          tickfont: { color: '#64748b' },
        },
        yaxis: {
          title: { text: 'Tasks', font: { color: '#10b981' } },
          showgrid: true,
          gridcolor: '#1a1a2e',
          tickfont: { color: '#64748b' },
        },
        yaxis2: {
          title: { text: 'Latency (ms)', font: { color: '#3b82f6' } },
          overlaying: 'y',
          side: 'right',
          showgrid: false,
          tickfont: { color: '#64748b' },
        },
        hovermode: 'x unified',
      }}
      config={{
        displayModeBar: false,
        responsive: true,
      }}
      style={{ width: '100%', height: '100%' }}
    />
  )
}
