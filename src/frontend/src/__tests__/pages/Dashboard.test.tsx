import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen } from '@testing-library/react'
import Dashboard from '@/pages/Dashboard'
import { useStore } from '@/lib/store'

// Mock framer-motion
vi.mock('framer-motion', () => ({
  motion: {
    div: ({ children, className, ...props }: any) => (
      <div className={className} {...props}>
        {children}
      </div>
    ),
  },
}))

// Mock child components
vi.mock('@/components/agents/AgentGrid', () => ({
  default: () => <div data-testid="agent-grid">AgentGrid</div>,
}))

vi.mock('@/components/metrics/MetricsChart', () => ({
  default: () => <div data-testid="metrics-chart">MetricsChart</div>,
}))

// Mock API calls
vi.mock('@/lib/api', () => ({
  agentApi: { listRaw: vi.fn().mockResolvedValue({ data: [] }) },
  taskApi: { list: vi.fn().mockResolvedValue({ data: [] }) },
  metricsApi: { getSystem: vi.fn().mockResolvedValue({ data: {} }) },
}))

describe('Dashboard', () => {
  beforeEach(() => {
    const store = useStore.getState()
    store.setMetrics({
      totalTasks: 200,
      completedTasks: 180,
      failedTasks: 5,
      runningTasks: 15,
      totalAgents: 20,
      activeAgents: 12,
      totalTokens: 500000,
      totalCost: 42.5,
      avgLatencyMs: 350,
      successRate: 0.96,
    })
    store.setAgents([])
    store.setTasks([])
  })

  describe('rendering', () => {
    it('renders the dashboard heading', () => {
      render(<Dashboard />)
      expect(screen.getByText('Dashboard')).toBeInTheDocument()
    })

    it('renders the subtitle', () => {
      render(<Dashboard />)
      expect(
        screen.getByText('Real-time overview of your agent swarm')
      ).toBeInTheDocument()
    })

    it('renders the live indicator', () => {
      render(<Dashboard />)
      expect(screen.getByText('Live')).toBeInTheDocument()
    })
  })

  describe('stat cards', () => {
    it('displays Active Agents stat', () => {
      render(<Dashboard />)
      expect(screen.getByText('Active Agents')).toBeInTheDocument()
      expect(screen.getByText('12')).toBeInTheDocument()
    })

    it('displays Running Tasks stat', () => {
      render(<Dashboard />)
      expect(screen.getByText('Running Tasks')).toBeInTheDocument()
      expect(screen.getByText('15')).toBeInTheDocument()
    })

    it('displays Total Cost stat', () => {
      render(<Dashboard />)
      expect(screen.getByText('Total Cost')).toBeInTheDocument()
    })

    it('displays Total Tokens stat', () => {
      render(<Dashboard />)
      expect(screen.getByText('Total Tokens')).toBeInTheDocument()
    })
  })

  describe('sections', () => {
    it('renders Agent Swarm section', () => {
      render(<Dashboard />)
      expect(screen.getByText('Agent Swarm')).toBeInTheDocument()
    })

    it('renders AgentGrid component', () => {
      render(<Dashboard />)
      expect(screen.getByTestId('agent-grid')).toBeInTheDocument()
    })

    it('renders Today\'s Summary section', () => {
      render(<Dashboard />)
      expect(screen.getByText("Today's Summary")).toBeInTheDocument()
    })

    it('renders Recent Tasks section', () => {
      render(<Dashboard />)
      expect(screen.getByText('Recent Tasks')).toBeInTheDocument()
    })

    it('renders Performance Trends section', () => {
      render(<Dashboard />)
      expect(screen.getByText('Performance Trends')).toBeInTheDocument()
    })

    it('renders MetricsChart component', () => {
      render(<Dashboard />)
      expect(screen.getByTestId('metrics-chart')).toBeInTheDocument()
    })
  })

  describe("today's summary", () => {
    it('shows Completed label', () => {
      render(<Dashboard />)
      expect(screen.getByText('Completed')).toBeInTheDocument()
    })

    it('shows Avg. Latency label', () => {
      render(<Dashboard />)
      expect(screen.getByText('Avg. Latency')).toBeInTheDocument()
    })

    it('shows Success Rate label', () => {
      render(<Dashboard />)
      expect(screen.getByText('Success Rate')).toBeInTheDocument()
    })

    it('shows success rate percentage with green color for high rates', () => {
      render(<Dashboard />)
      const rate = screen.getByText('96.0%')
      expect(rate).toBeInTheDocument()
      expect(rate).toHaveClass('text-green-500')
    })
  })

  describe('recent tasks', () => {
    it('shows empty state when no tasks', () => {
      render(<Dashboard />)
      expect(screen.getByText('No recent tasks')).toBeInTheDocument()
    })

    it('shows tasks when available', () => {
      useStore.getState().setTasks([
        {
          id: 't1',
          name: 'Test Task',
          status: 'completed',
          createdAt: new Date().toISOString(),
          completedAt: new Date().toISOString(),
          costDollars: 0.5,
          tokensUsed: 1000,
          prompt: '',
          priority: 5,
          agentId: 'a1',
        },
      ])
      render(<Dashboard />)
      expect(screen.getByText('Test Task')).toBeInTheDocument()
    })
  })

  describe('busy agent count', () => {
    it('displays count of busy agents', () => {
      useStore.getState().setAgents([
        { id: 'a1', name: 'Agent 1', status: 'busy', model: 'gpt-4o', currentLoad: 1, maxLoad: 5, totalTasks: 10, successRate: 0.9, costDollars: 1, lastActiveAt: '' },
        { id: 'a2', name: 'Agent 2', status: 'idle', model: 'gpt-4o', currentLoad: 0, maxLoad: 5, totalTasks: 5, successRate: 1, costDollars: 0.5, lastActiveAt: '' },
        { id: 'a3', name: 'Agent 3', status: 'busy', model: 'gpt-4o', currentLoad: 2, maxLoad: 5, totalTasks: 15, successRate: 0.95, costDollars: 2, lastActiveAt: '' },
      ])
      render(<Dashboard />)
      expect(screen.getByText('2 busy')).toBeInTheDocument()
    })
  })
})
