import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen } from '@testing-library/react'
import MetricsChart from '@/components/metrics/MetricsChart'

// Mock react-plotly.js
vi.mock('react-plotly.js', () => ({
  default: ({ data, layout, config, style }: any) => (
    <div data-testid="plotly-chart" data-layout={JSON.stringify(layout)} data-config={JSON.stringify(config)} style={style}>
      <div data-testid="chart-data">{JSON.stringify(data)}</div>
      <div data-testid="chart-traces">{data.length} traces</div>
      {data.map((trace: any, index: number) => (
        <div key={index} data-testid={`trace-${index}`}>
          <span data-testid={`trace-${index}-name`}>{trace.name}</span>
          <span data-testid={`trace-${index}-type`}>{trace.type}</span>
          <span data-testid={`trace-${index}-mode`}>{trace.mode}</span>
        </div>
      ))}
    </div>
  ),
}))

describe('MetricsChart', () => {
  beforeEach(() => {
    // Mock Date.now for consistent test data
    vi.spyOn(Date, 'now').mockReturnValue(new Date('2024-01-15T12:00:00Z').getTime())
  })

  describe('rendering', () => {
    it('renders the chart component', () => {
      render(<MetricsChart />)
      expect(screen.getByTestId('plotly-chart')).toBeInTheDocument()
    })

    it('renders with correct number of traces', () => {
      render(<MetricsChart />)
      expect(screen.getByTestId('chart-traces')).toHaveTextContent('2 traces')
    })

    it('renders Tasks Completed trace', () => {
      render(<MetricsChart />)
      expect(screen.getByTestId('trace-0-name')).toHaveTextContent('Tasks Completed')
    })

    it('renders Avg Latency trace', () => {
      render(<MetricsChart />)
      expect(screen.getByTestId('trace-1-name')).toHaveTextContent('Avg Latency (ms)')
    })
  })

  describe('chart configuration', () => {
    it('uses scatter chart type', () => {
      render(<MetricsChart />)
      expect(screen.getByTestId('trace-0-type')).toHaveTextContent('scatter')
      expect(screen.getByTestId('trace-1-type')).toHaveTextContent('scatter')
    })

    it('uses lines mode for traces', () => {
      render(<MetricsChart />)
      expect(screen.getByTestId('trace-0-mode')).toHaveTextContent('lines')
      expect(screen.getByTestId('trace-1-mode')).toHaveTextContent('lines')
    })

    it('configures full width and height style', () => {
      render(<MetricsChart />)
      const chart = screen.getByTestId('plotly-chart')
      expect(chart).toHaveStyle({ width: '100%', height: '100%' })
    })
  })

  describe('layout configuration', () => {
    it('uses transparent backgrounds', () => {
      render(<MetricsChart />)
      const chart = screen.getByTestId('plotly-chart')
      const layout = JSON.parse(chart.dataset.layout || '{}')

      expect(layout.paper_bgcolor).toBe('transparent')
      expect(layout.plot_bgcolor).toBe('transparent')
    })

    it('configures autosize', () => {
      render(<MetricsChart />)
      const chart = screen.getByTestId('plotly-chart')
      const layout = JSON.parse(chart.dataset.layout || '{}')

      expect(layout.autosize).toBe(true)
    })

    it('shows legend', () => {
      render(<MetricsChart />)
      const chart = screen.getByTestId('plotly-chart')
      const layout = JSON.parse(chart.dataset.layout || '{}')

      expect(layout.showlegend).toBe(true)
    })

    it('configures horizontal legend at top', () => {
      render(<MetricsChart />)
      const chart = screen.getByTestId('plotly-chart')
      const layout = JSON.parse(chart.dataset.layout || '{}')

      expect(layout.legend.orientation).toBe('h')
      expect(layout.legend.x).toBe(0)
      expect(layout.legend.y).toBe(1.1)
    })

    it('uses x unified hover mode', () => {
      render(<MetricsChart />)
      const chart = screen.getByTestId('plotly-chart')
      const layout = JSON.parse(chart.dataset.layout || '{}')

      expect(layout.hovermode).toBe('x unified')
    })

    it('configures dual y-axes', () => {
      render(<MetricsChart />)
      const chart = screen.getByTestId('plotly-chart')
      const layout = JSON.parse(chart.dataset.layout || '{}')

      expect(layout.yaxis.title.text).toBe('Tasks')
      expect(layout.yaxis2.title.text).toBe('Latency (ms)')
      expect(layout.yaxis2.overlaying).toBe('y')
      expect(layout.yaxis2.side).toBe('right')
    })

    it('configures x-axis time format', () => {
      render(<MetricsChart />)
      const chart = screen.getByTestId('plotly-chart')
      const layout = JSON.parse(chart.dataset.layout || '{}')

      expect(layout.xaxis.tickformat).toBe('%H:%M')
    })

    it('configures margins', () => {
      render(<MetricsChart />)
      const chart = screen.getByTestId('plotly-chart')
      const layout = JSON.parse(chart.dataset.layout || '{}')

      expect(layout.margin).toEqual({ l: 60, r: 60, t: 20, b: 40 })
    })
  })

  describe('plotly config', () => {
    it('hides mode bar', () => {
      render(<MetricsChart />)
      const chart = screen.getByTestId('plotly-chart')
      const config = JSON.parse(chart.dataset.config || '{}')

      expect(config.displayModeBar).toBe(false)
    })

    it('enables responsive mode', () => {
      render(<MetricsChart />)
      const chart = screen.getByTestId('plotly-chart')
      const config = JSON.parse(chart.dataset.config || '{}')

      expect(config.responsive).toBe(true)
    })
  })

  describe('data generation', () => {
    it('starts with empty data arrays before API response', () => {
      render(<MetricsChart />)
      const chartData = screen.getByTestId('chart-data')
      const data = JSON.parse(chartData.textContent || '[]')

      // Chart renders with two traces, each initially empty until API responds
      expect(data).toHaveLength(2)
      expect(data[0].x).toEqual([])
      expect(data[0].y).toEqual([])
      expect(data[1].x).toEqual([])
      expect(data[1].y).toEqual([])
    })

    it('has two traces for task completions and latency', () => {
      render(<MetricsChart />)
      const chartData = screen.getByTestId('chart-data')
      const data = JSON.parse(chartData.textContent || '[]')

      expect(data[0].name).toBe('Tasks Completed')
      expect(data[1].name).toBe('Avg Latency (ms)')
    })
  })

  describe('trace styling', () => {
    it('uses green color for tasks completed', () => {
      render(<MetricsChart />)
      const chartData = screen.getByTestId('chart-data')
      const data = JSON.parse(chartData.textContent || '[]')

      expect(data[0].line.color).toBe('#10b981')
    })

    it('uses blue color for latency', () => {
      render(<MetricsChart />)
      const chartData = screen.getByTestId('chart-data')
      const data = JSON.parse(chartData.textContent || '[]')

      expect(data[1].line.color).toBe('#3b82f6')
    })

    it('fills tasks completed area', () => {
      render(<MetricsChart />)
      const chartData = screen.getByTestId('chart-data')
      const data = JSON.parse(chartData.textContent || '[]')

      expect(data[0].fill).toBe('tozeroy')
      expect(data[0].fillcolor).toBe('rgba(16, 185, 129, 0.1)')
    })

    it('uses secondary y-axis for latency', () => {
      render(<MetricsChart />)
      const chartData = screen.getByTestId('chart-data')
      const data = JSON.parse(chartData.textContent || '[]')

      expect(data[1].yaxis).toBe('y2')
    })

    it('sets line width to 2', () => {
      render(<MetricsChart />)
      const chartData = screen.getByTestId('chart-data')
      const data = JSON.parse(chartData.textContent || '[]')

      expect(data[0].line.width).toBe(2)
      expect(data[1].line.width).toBe(2)
    })
  })

  describe('memoization', () => {
    it('generates data only once (via useMemo)', () => {
      const { rerender } = render(<MetricsChart />)
      const chartData1 = screen.getByTestId('chart-data').textContent

      rerender(<MetricsChart />)
      const chartData2 = screen.getByTestId('chart-data').textContent

      // Data should be the same between renders due to useMemo with empty deps
      expect(chartData1).toBe(chartData2)
    })
  })

  describe('grid styling', () => {
    it('shows grid on x-axis', () => {
      render(<MetricsChart />)
      const chart = screen.getByTestId('plotly-chart')
      const layout = JSON.parse(chart.dataset.layout || '{}')

      expect(layout.xaxis.showgrid).toBe(true)
    })

    it('shows grid on primary y-axis', () => {
      render(<MetricsChart />)
      const chart = screen.getByTestId('plotly-chart')
      const layout = JSON.parse(chart.dataset.layout || '{}')

      expect(layout.yaxis.showgrid).toBe(true)
    })

    it('hides grid on secondary y-axis', () => {
      render(<MetricsChart />)
      const chart = screen.getByTestId('plotly-chart')
      const layout = JSON.parse(chart.dataset.layout || '{}')

      expect(layout.yaxis2.showgrid).toBe(false)
    })

    it('uses dark grid color', () => {
      render(<MetricsChart />)
      const chart = screen.getByTestId('plotly-chart')
      const layout = JSON.parse(chart.dataset.layout || '{}')

      expect(layout.xaxis.gridcolor).toBe('#1a1a2e')
      expect(layout.yaxis.gridcolor).toBe('#1a1a2e')
    })
  })

  describe('font styling', () => {
    it('uses appropriate font color for labels', () => {
      render(<MetricsChart />)
      const chart = screen.getByTestId('plotly-chart')
      const layout = JSON.parse(chart.dataset.layout || '{}')

      expect(layout.font.color).toBe('#94a3b8')
    })

    it('uses font size 11', () => {
      render(<MetricsChart />)
      const chart = screen.getByTestId('plotly-chart')
      const layout = JSON.parse(chart.dataset.layout || '{}')

      expect(layout.font.size).toBe(11)
    })

    it('uses color-coded title fonts for y-axes', () => {
      render(<MetricsChart />)
      const chart = screen.getByTestId('plotly-chart')
      const layout = JSON.parse(chart.dataset.layout || '{}')

      expect(layout.yaxis.title.font.color).toBe('#10b981') // Green for tasks
      expect(layout.yaxis2.title.font.color).toBe('#3b82f6') // Blue for latency
    })
  })
})
