import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import Agents from '@/pages/Agents'
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

// Mock react-hot-toast
vi.mock('react-hot-toast', () => ({
  default: {
    success: vi.fn(),
    error: vi.fn(),
  },
}))

// Mock AgentGrid
vi.mock('@/components/agents/AgentGrid', () => ({
  default: () => <div data-testid="agent-grid">AgentGrid</div>,
}))

// Mock InterventionPanel
vi.mock('@/components/InterventionPanel', () => ({
  InterventionPanel: ({ agent, onClose }: any) => (
    <div data-testid="intervention-panel">
      <span>{agent.name}</span>
      <button onClick={onClose}>Close</button>
    </div>
  ),
}))

// Mock API
vi.mock('@/lib/api', () => ({
  agentApi: {
    listRaw: vi.fn().mockResolvedValue({ data: [] }),
    create: vi.fn().mockResolvedValue({}),
    pause: vi.fn().mockResolvedValue({}),
    resume: vi.fn().mockResolvedValue({}),
    delete: vi.fn().mockResolvedValue({}),
    update: vi.fn().mockResolvedValue({}),
  },
}))

const mockAgents = [
  { id: 'a1', name: 'Research Agent', status: 'busy', model: 'gpt-4o', currentLoad: 3, maxLoad: 10, totalTasks: 50, successRate: 0.96, costDollars: 5.0, totalCost: 5.0, totalTokens: 10000, reputationScore: 0.9, lastActiveAt: '', createdAt: '2024-01-01' },
  { id: 'a2', name: 'Writer Agent', status: 'idle', model: 'claude-3.5-sonnet', currentLoad: 0, maxLoad: 5, totalTasks: 20, successRate: 1.0, costDollars: 2.0, totalCost: 2.0, totalTokens: 5000, reputationScore: 0.95, lastActiveAt: '', createdAt: '2024-01-01' },
  { id: 'a3', name: 'Error Agent', status: 'error', model: 'gpt-4o-mini', currentLoad: 0, maxLoad: 10, totalTasks: 10, successRate: 0.5, costDollars: 1.0, totalCost: 1.0, totalTokens: 2000, reputationScore: 0.6, lastActiveAt: '', createdAt: '2024-01-01' },
]

describe('Agents', () => {
  beforeEach(() => {
    useStore.getState().setAgents([])
  })

  describe('rendering', () => {
    it('renders the Agents heading', () => {
      render(<Agents />)
      expect(screen.getByText('Agents')).toBeInTheDocument()
    })

    it('renders the subtitle', () => {
      render(<Agents />)
      expect(screen.getByText('Manage and monitor your agent swarm')).toBeInTheDocument()
    })

    it('renders the Register Agent button', () => {
      render(<Agents />)
      expect(screen.getByText('Register Agent')).toBeInTheDocument()
    })

    it('renders search input', () => {
      render(<Agents />)
      expect(screen.getByPlaceholderText('Search agents...')).toBeInTheDocument()
    })
  })

  describe('status filters', () => {
    it('renders all status filter buttons', () => {
      useStore.getState().setAgents(mockAgents)
      render(<Agents />)
      expect(screen.getByText(/^All/)).toBeInTheDocument()
      expect(screen.getByText(/^Idle/)).toBeInTheDocument()
      expect(screen.getByText(/^Busy/)).toBeInTheDocument()
      expect(screen.getByText(/^Error/)).toBeInTheDocument()
      expect(screen.getByText(/^Paused/)).toBeInTheDocument()
    })

    it('shows agent counts in filter buttons', () => {
      useStore.getState().setAgents(mockAgents)
      render(<Agents />)
      // All(3), Idle(1), Busy(1), Error(1), Paused(0)
      expect(screen.getByText(/All/)).toHaveTextContent('(3)')
      expect(screen.getByText(/Busy/)).toHaveTextContent('(1)')
      expect(screen.getByText(/Idle/)).toHaveTextContent('(1)')
    })
  })

  describe('view modes', () => {
    it('defaults to grid view with AgentGrid', () => {
      render(<Agents />)
      expect(screen.getByTestId('agent-grid')).toBeInTheDocument()
    })
  })

  describe('register agent modal', () => {
    it('opens modal when Register Agent is clicked', async () => {
      const user = userEvent.setup()
      render(<Agents />)

      await user.click(screen.getByText('Register Agent'))
      expect(screen.getByText('Name')).toBeInTheDocument()
      expect(screen.getByText('Model')).toBeInTheDocument()
      expect(screen.getByText('Max Concurrent Load')).toBeInTheDocument()
    })

    it('shows Cancel and Register buttons in modal', async () => {
      const user = userEvent.setup()
      render(<Agents />)

      await user.click(screen.getByText('Register Agent'))
      expect(screen.getByText('Cancel')).toBeInTheDocument()
      expect(screen.getByText('Register')).toBeInTheDocument()
    })

    it('has model select options', async () => {
      const user = userEvent.setup()
      render(<Agents />)

      await user.click(screen.getByText('Register Agent'))
      expect(screen.getByText('GPT-4o Mini')).toBeInTheDocument()
      expect(screen.getByText('GPT-4o')).toBeInTheDocument()
      expect(screen.getByText('Claude 3.5 Haiku')).toBeInTheDocument()
      expect(screen.getByText('Claude 3.5 Sonnet')).toBeInTheDocument()
    })
  })
})
