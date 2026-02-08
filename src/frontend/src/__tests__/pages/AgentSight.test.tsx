import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen } from '@testing-library/react'
import AgentSightPage from '@/pages/AgentSight'
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

// Mock AgentSight component
vi.mock('@/components/AgentSight', () => ({
  AgentSight: ({ agents, zoomLevel }: any) => (
    <div data-testid="agent-sight-grid">
      {agents.length} agents at {zoomLevel}x
    </div>
  ),
}))

// Mock API
vi.mock('@/lib/api', () => ({
  agentApi: {
    listRaw: vi.fn().mockResolvedValue({ data: [] }),
  },
}))

const mockAgents = [
  { id: 'a1', name: 'Research Agent', status: 'busy', model: 'gpt-4o', currentLoad: 3, maxLoad: 10, totalTasks: 50, successRate: 0.96, costDollars: 5.0, totalCost: 5.0, totalTokens: 10000, reputationScore: 0.9, lastActiveAt: '', createdAt: '2024-01-01' },
  { id: 'a2', name: 'Writer Agent', status: 'idle', model: 'claude-3.5-sonnet', currentLoad: 0, maxLoad: 5, totalTasks: 20, successRate: 1.0, costDollars: 2.0, totalCost: 2.0, totalTokens: 5000, reputationScore: 0.95, lastActiveAt: '', createdAt: '2024-01-01' },
]

describe('AgentSight', () => {
  beforeEach(() => {
    useStore.getState().setAgents([])
    useStore.getState().setWsConnected(false)
  })

  describe('rendering', () => {
    it('renders the Agent Sight heading', () => {
      render(<AgentSightPage />)
      expect(screen.getByText('Agent Sight')).toBeInTheDocument()
    })

    it('renders the subtitle', () => {
      render(<AgentSightPage />)
      expect(screen.getByText('Live screen feeds from your agent swarm')).toBeInTheDocument()
    })

    it('renders search input', () => {
      render(<AgentSightPage />)
      expect(screen.getByPlaceholderText('Search agents...')).toBeInTheDocument()
    })
  })

  describe('connection status', () => {
    it('shows Offline when WebSocket not connected', () => {
      render(<AgentSightPage />)
      expect(screen.getByText('Offline')).toBeInTheDocument()
    })

    it('shows Live when WebSocket is connected', () => {
      useStore.getState().setWsConnected(true)
      render(<AgentSightPage />)
      expect(screen.getByText('Live')).toBeInTheDocument()
    })
  })

  describe('status filters', () => {
    it('renders all status filter buttons', () => {
      render(<AgentSightPage />)
      expect(screen.getByText(/^All/)).toBeInTheDocument()
      expect(screen.getByText(/^Idle/)).toBeInTheDocument()
      expect(screen.getByText(/^Busy/)).toBeInTheDocument()
      expect(screen.getByText(/^Error/)).toBeInTheDocument()
      expect(screen.getByText(/^Paused/)).toBeInTheDocument()
    })
  })

  describe('zoom controls', () => {
    it('shows zoom level indicator', () => {
      render(<AgentSightPage />)
      expect(screen.getByText('1x')).toBeInTheDocument()
    })
  })

  describe('agent display', () => {
    it('shows AgentSight grid when agents exist', () => {
      useStore.getState().setAgents(mockAgents)
      render(<AgentSightPage />)
      expect(screen.getByTestId('agent-sight-grid')).toBeInTheDocument()
    })

    it('shows empty state when no agents', () => {
      render(<AgentSightPage />)
      expect(screen.getByText('No agents found')).toBeInTheDocument()
    })

    it('shows agent count in header', () => {
      useStore.getState().setAgents(mockAgents)
      render(<AgentSightPage />)
      expect(screen.getByText('2 agents')).toBeInTheDocument()
    })
  })
})
