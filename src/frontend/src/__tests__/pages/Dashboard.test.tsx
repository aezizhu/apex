import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
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
  AnimatePresence: ({ children }: any) => <>{children}</>,
}))

// Mock child components
vi.mock('@/components/agents/AgentGrid', () => ({
  default: () => <div data-testid="agent-grid">AgentGrid</div>,
}))

vi.mock('@/components/metrics/MetricsChart', () => ({
  default: () => <div data-testid="metrics-chart">MetricsChart</div>,
}))

// Mock react-hot-toast
vi.mock('react-hot-toast', () => ({
  default: {
    success: vi.fn(),
    error: vi.fn(),
  },
}))

// Mock API calls
vi.mock('@/lib/api', () => ({
  agentApi: { listRaw: vi.fn().mockResolvedValue({ data: [] }) },
  taskApi: {
    list: vi.fn().mockResolvedValue({ data: [] }),
    create: vi.fn().mockResolvedValue({ data: { id: 't1', name: 'Test', status: 'pending' } }),
  },
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
      expect(screen.getByText('Command Center')).toBeInTheDocument()
    })

    it('renders the subtitle', () => {
      render(<Dashboard />)
      expect(
        screen.getByText(/Multi-agent orchestration/)
      ).toBeInTheDocument()
    })

    it('renders the status indicator', () => {
      render(<Dashboard />)
      expect(screen.getByText('Standby')).toBeInTheDocument()
    })

    it('shows Operational when agents exist', () => {
      useStore.getState().setAgents([
        { id: 'a1', name: 'Agent 1', status: 'busy', model: 'claude-sonnet-4-5', currentLoad: 1, maxLoad: 5, successRate: 0.9, reputationScore: 0.9, totalTokens: 10000, totalCost: 1.0 },
      ])
      render(<Dashboard />)
      expect(screen.getByText('Operational')).toBeInTheDocument()
    })
  })

  describe('mission command input', () => {
    it('renders the Mission Command section', () => {
      render(<Dashboard />)
      expect(screen.getByText('Mission Command')).toBeInTheDocument()
    })

    it('renders the textarea with placeholder', () => {
      render(<Dashboard />)
      expect(screen.getByPlaceholderText(/Describe your mission objective/)).toBeInTheDocument()
    })

    it('renders the Launch button', () => {
      render(<Dashboard />)
      expect(screen.getByText('Launch')).toBeInTheDocument()
    })

    it('renders priority selector', () => {
      render(<Dashboard />)
      expect(screen.getByText('Normal')).toBeInTheDocument()
    })

    it('shows agent execution hint', () => {
      render(<Dashboard />)
      expect(screen.getByText(/Agents will bid, plan, and execute in parallel/)).toBeInTheDocument()
    })

    it('Launch button is disabled when input is empty', () => {
      render(<Dashboard />)
      const launchBtn = screen.getByText('Launch').closest('button')
      expect(launchBtn).toBeDisabled()
    })

    it('Launch button enables when input has text', async () => {
      const user = userEvent.setup()
      render(<Dashboard />)
      const textarea = screen.getByPlaceholderText(/Describe your mission objective/)
      await user.type(textarea, 'Analyze the dataset')
      const launchBtn = screen.getByText('Launch').closest('button')
      expect(launchBtn).not.toBeDisabled()
    })

    it('shows priority options when priority button is clicked', async () => {
      const user = userEvent.setup()
      render(<Dashboard />)
      await user.click(screen.getByText('Normal'))
      expect(screen.getByText('High')).toBeInTheDocument()
      expect(screen.getByText('Critical')).toBeInTheDocument()
    })
  })

  describe('stat readouts', () => {
    it('displays Agents stat', () => {
      render(<Dashboard />)
      expect(screen.getByText('Agents')).toBeInTheDocument()
      expect(screen.getByText('20')).toBeInTheDocument()
    })

    it('displays Tasks stat', () => {
      render(<Dashboard />)
      expect(screen.getByText('Tasks')).toBeInTheDocument()
      expect(screen.getByText('15')).toBeInTheDocument()
    })

    it('displays Cost stat', () => {
      render(<Dashboard />)
      expect(screen.getByText('Cost')).toBeInTheDocument()
    })

    it('displays Tokens stat', () => {
      render(<Dashboard />)
      expect(screen.getByText('Tokens')).toBeInTheDocument()
    })

    it('displays Success stat', () => {
      render(<Dashboard />)
      expect(screen.getByText('Success')).toBeInTheDocument()
      expect(screen.getByText('96.0%')).toBeInTheDocument()
    })

    it('displays Completed stat', () => {
      render(<Dashboard />)
      expect(screen.getByText('Completed')).toBeInTheDocument()
    })
  })

  describe('sections', () => {
    it('renders Swarm Topology section', () => {
      render(<Dashboard />)
      expect(screen.getByText('SWARM TOPOLOGY')).toBeInTheDocument()
    })

    it('renders AgentGrid component when agents exist', () => {
      useStore.getState().setAgents([
        { id: 'a1', name: 'Agent 1', status: 'idle', model: 'claude-sonnet-4-5', currentLoad: 0, maxLoad: 5, successRate: 0.9, reputationScore: 0.9, totalTokens: 10000, totalCost: 1.0 },
      ])
      render(<Dashboard />)
      expect(screen.getByTestId('agent-grid')).toBeInTheDocument()
    })

    it('renders System Health section', () => {
      render(<Dashboard />)
      expect(screen.getByText('SYSTEM HEALTH')).toBeInTheDocument()
    })

    it('renders Mission Feed section', () => {
      render(<Dashboard />)
      expect(screen.getByText('MISSION FEED')).toBeInTheDocument()
    })

    it('renders Performance Trends section', () => {
      render(<Dashboard />)
      expect(screen.getByText('PERFORMANCE TRENDS')).toBeInTheDocument()
    })

    it('renders MetricsChart component', () => {
      render(<Dashboard />)
      expect(screen.getByTestId('metrics-chart')).toBeInTheDocument()
    })
  })

  describe('system health', () => {
    it('shows Avg Latency label', () => {
      render(<Dashboard />)
      expect(screen.getByText('Avg Latency')).toBeInTheDocument()
    })

    it('shows Throughput label', () => {
      render(<Dashboard />)
      expect(screen.getByText('Throughput')).toBeInTheDocument()
    })

    it('shows Error Rate label', () => {
      render(<Dashboard />)
      expect(screen.getByText('Error Rate')).toBeInTheDocument()
    })

    it('shows error rate with color coding', () => {
      render(<Dashboard />)
      const errorRate = screen.getByText('2.5%')
      expect(errorRate).toBeInTheDocument()
    })
  })

  describe('mission feed', () => {
    it('shows empty state when no tasks', () => {
      render(<Dashboard />)
      expect(screen.getByText('No missions dispatched yet')).toBeInTheDocument()
    })

    it('shows launch hint in empty state', () => {
      render(<Dashboard />)
      expect(screen.getByText(/Launch a mission above/)).toBeInTheDocument()
    })

    it('shows tasks when available', () => {
      useStore.getState().setTasks([
        {
          id: 't1',
          name: 'Test Task',
          dagId: 'd1',
          status: 'completed',
          createdAt: new Date().toISOString(),
          completedAt: new Date().toISOString(),
          costDollars: 0.5,
          tokensUsed: 1000,
        },
      ])
      render(<Dashboard />)
      expect(screen.getByText('Test Task')).toBeInTheDocument()
    })
  })

  describe('busy agent count', () => {
    it('displays count of active agents in sublabel', () => {
      useStore.getState().setAgents([
        { id: 'a1', name: 'Agent 1', status: 'busy', model: 'claude-sonnet-4-5', currentLoad: 1, maxLoad: 5, successRate: 0.9, reputationScore: 0.9, totalTokens: 10000, totalCost: 1.0 },
        { id: 'a2', name: 'Agent 2', status: 'idle', model: 'claude-sonnet-4-5', currentLoad: 0, maxLoad: 5, successRate: 1, reputationScore: 0.95, totalTokens: 5000, totalCost: 0.5 },
        { id: 'a3', name: 'Agent 3', status: 'busy', model: 'claude-sonnet-4-5', currentLoad: 2, maxLoad: 5, successRate: 0.95, reputationScore: 0.9, totalTokens: 15000, totalCost: 2.0 },
      ])
      render(<Dashboard />)
      expect(screen.getByText('2 active')).toBeInTheDocument()
    })
  })
})
