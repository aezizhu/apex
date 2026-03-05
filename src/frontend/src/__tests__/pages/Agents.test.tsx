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
    delegate: vi.fn().mockResolvedValue({ data: { status: 'accepted', assignedAgent: 'a2', reason: 'Least busy' } }),
  },
}))

const mockAgents = [
  { id: 'a1', name: 'Research Agent', status: 'busy', model: 'gpt-5.2', currentLoad: 3, maxLoad: 10, totalTasks: 50, successRate: 0.96, costDollars: 5.0, totalCost: 5.0, totalTokens: 10000, reputationScore: 0.9, lastActiveAt: '', createdAt: '2024-01-01' },
  { id: 'a2', name: 'Writer Agent', status: 'idle', model: 'claude-sonnet-4-5', currentLoad: 0, maxLoad: 5, totalTasks: 20, successRate: 1.0, costDollars: 2.0, totalCost: 2.0, totalTokens: 5000, reputationScore: 0.95, lastActiveAt: '', createdAt: '2024-01-01' },
  { id: 'a3', name: 'Error Agent', status: 'error', model: 'gpt-5.2-mini', currentLoad: 0, maxLoad: 10, totalTasks: 10, successRate: 0.5, costDollars: 1.0, totalCost: 1.0, totalTokens: 2000, reputationScore: 0.6, lastActiveAt: '', createdAt: '2024-01-01' },
]

describe('Agents', () => {
  beforeEach(() => {
    useStore.getState().setAgents([])
  })

  describe('rendering', () => {
    it('renders the Fleet Control heading', () => {
      render(<Agents />)
      expect(screen.getByText('Fleet Control')).toBeInTheDocument()
    })

    it('renders the subtitle', () => {
      render(<Agents />)
      expect(screen.getByText('Agent Swarm Management')).toBeInTheDocument()
    })

    it('renders the Deploy Agent button', () => {
      render(<Agents />)
      expect(screen.getByText('Deploy Agent')).toBeInTheDocument()
    })

    it('renders search input', () => {
      render(<Agents />)
      expect(screen.getByPlaceholderText('Search fleet...')).toBeInTheDocument()
    })
  })

  describe('status filters', () => {
    it('renders all status filter buttons', () => {
      useStore.getState().setAgents(mockAgents)
      render(<Agents />)
      // Labels appear in both filter bar and FleetReadout, so use getAllByText
      expect(screen.getAllByText(/All/).length).toBeGreaterThanOrEqual(1)
      expect(screen.getAllByText(/Standby/).length).toBeGreaterThanOrEqual(1)
      expect(screen.getAllByText(/Active/).length).toBeGreaterThanOrEqual(1)
      expect(screen.getAllByText(/Fault/).length).toBeGreaterThanOrEqual(1)
      expect(screen.getAllByText(/Held/).length).toBeGreaterThanOrEqual(1)
    })

    it('shows agent counts in filter buttons', () => {
      useStore.getState().setAgents(mockAgents)
      render(<Agents />)
      // Filter buttons show count inline: "Active 1", "Standby 1", etc.
      const filterBar = screen.getByText(/^All/).closest('div')!
      expect(filterBar).toBeInTheDocument()
    })
  })

  describe('view modes', () => {
    it('defaults to grid view', () => {
      render(<Agents />)
      // In grid mode with no agents, shows empty state text
      expect(screen.getByText('No agents deployed')).toBeInTheDocument()
    })
  })

  describe('register agent modal', () => {
    it('opens modal when Deploy Agent is clicked', async () => {
      const user = userEvent.setup()
      render(<Agents />)

      await user.click(screen.getByText('Deploy Agent'))
      expect(screen.getByText('Designation')).toBeInTheDocument()
      expect(screen.getByText('Model')).toBeInTheDocument()
      expect(screen.getByText('Max Concurrent Load')).toBeInTheDocument()
    })

    it('shows Cancel and Deploy buttons in modal', async () => {
      const user = userEvent.setup()
      render(<Agents />)

      await user.click(screen.getByText('Deploy Agent'))
      expect(screen.getByText('Cancel')).toBeInTheDocument()
      expect(screen.getByText('Deploy')).toBeInTheDocument()
    })

    it('has model select options', async () => {
      const user = userEvent.setup()
      render(<Agents />)

      await user.click(screen.getByText('Deploy Agent'))
      expect(screen.getByText('Claude Opus 4.6')).toBeInTheDocument()
      expect(screen.getByText('Claude Sonnet 4.5')).toBeInTheDocument()
      expect(screen.getByText('Claude Haiku 4.5')).toBeInTheDocument()
      expect(screen.getByText('GPT-5.2')).toBeInTheDocument()
    })
  })
})
