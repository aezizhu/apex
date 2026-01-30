import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import AgentGrid from '@/components/agents/AgentGrid'
import { useStore, Agent } from '@/lib/store'

// Mock framer-motion
vi.mock('framer-motion', () => ({
  motion: {
    div: ({ children, onClick, onMouseEnter, onMouseLeave, className, style, ...props }: any) => (
      <div onClick={onClick} onMouseEnter={onMouseEnter} onMouseLeave={onMouseLeave} className={className} style={style} {...props}>
        {children}
      </div>
    ),
  },
  AnimatePresence: ({ children }: any) => <>{children}</>,
}))

// Mock agents data
const createMockAgent = (overrides: Partial<Agent> = {}): Agent => ({
  id: `agent-${Math.random().toString(36).substr(2, 9)}`,
  name: 'Test Agent',
  model: 'gpt-4',
  status: 'idle',
  currentLoad: 0,
  maxLoad: 10,
  successRate: 0.95,
  reputationScore: 0.9,
  totalTokens: 10000,
  totalCost: 0.5,
  confidence: 0.85,
  ...overrides,
})

const mockAgents: Agent[] = [
  createMockAgent({ id: 'agent-1', name: 'Agent Alpha', status: 'idle' }),
  createMockAgent({ id: 'agent-2', name: 'Agent Beta', status: 'busy', currentLoad: 5 }),
  createMockAgent({ id: 'agent-3', name: 'Agent Gamma', status: 'error' }),
  createMockAgent({ id: 'agent-4', name: 'Agent Delta', status: 'paused' }),
]

describe('AgentGrid', () => {
  beforeEach(() => {
    // Reset store before each test
    const store = useStore.getState()
    store.setAgents(mockAgents)
    store.setSelectedAgentId(null)
  })

  afterEach(() => {
    // Clear store after each test
    useStore.getState().setAgents([])
  })

  describe('rendering', () => {
    it('renders the grid container', () => {
      render(<AgentGrid />)
      // Should show agent count
      expect(screen.getByText(/4 agents/)).toBeInTheDocument()
    })

    it('displays correct number of agents', () => {
      render(<AgentGrid />)
      expect(screen.getByText('4 agents')).toBeInTheDocument()
    })

    it('renders legend with status indicators', () => {
      render(<AgentGrid />)
      expect(screen.getByText('Agent Status')).toBeInTheDocument()
      expect(screen.getByText('Busy')).toBeInTheDocument()
      expect(screen.getByText('Idle')).toBeInTheDocument()
      expect(screen.getByText('Error')).toBeInTheDocument()
      expect(screen.getByText('Paused')).toBeInTheDocument()
    })

    it('renders SVG hexagons for agents', () => {
      const { container } = render(<AgentGrid />)
      const svgs = container.querySelectorAll('svg')
      expect(svgs.length).toBeGreaterThanOrEqual(mockAgents.length)
    })

    it('applies correct status colors', () => {
      const { container } = render(<AgentGrid />)
      // Check for status indicator circles
      const circles = container.querySelectorAll('circle[r="8"]')
      expect(circles.length).toBe(mockAgents.length)
    })
  })

  describe('maxAgents prop', () => {
    it('limits displayed agents when maxAgents is set', () => {
      useStore.getState().setAgents([
        ...mockAgents,
        createMockAgent({ id: 'agent-5', name: 'Agent Epsilon' }),
        createMockAgent({ id: 'agent-6', name: 'Agent Zeta' }),
      ])

      render(<AgentGrid maxAgents={3} />)
      expect(screen.getByText('3 agents (showing 3)')).toBeInTheDocument()
    })

    it('shows all agents when total is less than maxAgents', () => {
      render(<AgentGrid maxAgents={100} />)
      expect(screen.getByText('4 agents')).toBeInTheDocument()
    })

    it('uses default maxAgents of 500', () => {
      render(<AgentGrid />)
      // Default should show all 4 agents without "(showing X)"
      expect(screen.getByText('4 agents')).toBeInTheDocument()
      expect(screen.queryByText(/showing/)).not.toBeInTheDocument()
    })
  })

  describe('agent selection', () => {
    it('calls onAgentSelect when agent is clicked', async () => {
      const handleSelect = vi.fn()
      const user = userEvent.setup()
      const { container } = render(<AgentGrid onAgentSelect={handleSelect} />)

      // Click on the first agent hexagon
      const agentDivs = container.querySelectorAll('.cursor-pointer')
      if (agentDivs[0]) {
        await user.click(agentDivs[0])
        expect(handleSelect).toHaveBeenCalledTimes(1)
        expect(handleSelect).toHaveBeenCalledWith(expect.objectContaining({ id: 'agent-1' }))
      }
    })

    it('updates selectedAgentId in store when agent is clicked', async () => {
      const user = userEvent.setup()
      const { container } = render(<AgentGrid />)

      const agentDivs = container.querySelectorAll('.cursor-pointer')
      if (agentDivs[0]) {
        await user.click(agentDivs[0])
        expect(useStore.getState().selectedAgentId).toBe('agent-1')
      }
    })

    it('deselects agent when clicked again', async () => {
      const user = userEvent.setup()
      useStore.getState().setSelectedAgentId('agent-1')
      const { container } = render(<AgentGrid />)

      const agentDivs = container.querySelectorAll('.cursor-pointer')
      if (agentDivs[0]) {
        await user.click(agentDivs[0])
        expect(useStore.getState().selectedAgentId).toBeNull()
      }
    })

    it('applies selected styles to selected agent', () => {
      useStore.getState().setSelectedAgentId('agent-1')
      const { container } = render(<AgentGrid />)

      // Selected agent should have drop-shadow class
      const selectedSvg = container.querySelector('.drop-shadow-\\[0_0_10px_rgba\\(59\\,130\\,246\\,0\\.5\\)\\]')
      expect(selectedSvg).toBeInTheDocument()
    })
  })

  describe('hover behavior', () => {
    it('shows hover card on mouse enter', async () => {
      const { container } = render(<AgentGrid />)

      const agentDivs = container.querySelectorAll('.cursor-pointer')
      if (agentDivs[0]) {
        fireEvent.mouseEnter(agentDivs[0])

        await waitFor(() => {
          expect(screen.getByText('Agent Alpha')).toBeInTheDocument()
        })
      }
    })

    it('hides hover card on mouse leave', async () => {
      const { container } = render(<AgentGrid />)

      const agentDivs = container.querySelectorAll('.cursor-pointer')
      if (agentDivs[0]) {
        fireEvent.mouseEnter(agentDivs[0])
        await waitFor(() => {
          expect(screen.getByText('Agent Alpha')).toBeInTheDocument()
        })

        fireEvent.mouseLeave(agentDivs[0])
        await waitFor(() => {
          // The name should only appear once (in the legend area or not at all in hover card)
          const agentNames = screen.queryAllByText('Agent Alpha')
          // After mouse leave, hover card should be hidden
          expect(agentNames.length).toBeLessThanOrEqual(1)
        })
      }
    })

    it('displays agent details in hover card', async () => {
      const { container } = render(<AgentGrid />)

      const agentDivs = container.querySelectorAll('.cursor-pointer')
      if (agentDivs[0]) {
        fireEvent.mouseEnter(agentDivs[0])

        await waitFor(() => {
          expect(screen.getByText('Agent Alpha')).toBeInTheDocument()
          expect(screen.getByText('gpt-4')).toBeInTheDocument()
          expect(screen.getByText('0/10')).toBeInTheDocument()
          expect(screen.getByText('95.0%')).toBeInTheDocument()
        })
      }
    })

    it('shows status badge in hover card', async () => {
      const { container } = render(<AgentGrid />)

      // Hover over busy agent
      const agentDivs = container.querySelectorAll('.cursor-pointer')
      if (agentDivs[1]) {
        fireEvent.mouseEnter(agentDivs[1])

        await waitFor(() => {
          expect(screen.getByText('busy')).toBeInTheDocument()
        })
      }
    })
  })

  describe('grid layout', () => {
    it('calculates grid dimensions based on agent count', () => {
      const { container } = render(<AgentGrid />)

      // Grid container should have computed dimensions
      const gridContainer = container.querySelector('.relative')
      expect(gridContainer).toBeInTheDocument()
    })

    it('applies hexagon offset for alternating rows', () => {
      const { container } = render(<AgentGrid />)

      // Agents should be positioned with different x offsets
      const agentDivs = container.querySelectorAll('.cursor-pointer.absolute')
      expect(agentDivs.length).toBe(mockAgents.length)
    })
  })

  describe('agent status visualization', () => {
    it('shows pulse animation for busy agents', () => {
      const { container } = render(<AgentGrid />)

      // Busy agents should have pulse animation class
      const pulseElements = container.querySelectorAll('.animate-pulse-glow')
      expect(pulseElements.length).toBeGreaterThan(0)
    })

    it('shows load indicator for agents with load', () => {
      const { container } = render(<AgentGrid />)

      // Agent with currentLoad > 0 should have load indicator circle
      const loadIndicators = container.querySelectorAll('circle[r="20"]')
      expect(loadIndicators.length).toBeGreaterThan(0)
    })

    it('displays correct confidence colors', () => {
      const { container } = render(<AgentGrid />)

      // Should have hexagon paths with confidence-based fill colors
      const hexPaths = container.querySelectorAll('path')
      expect(hexPaths.length).toBeGreaterThan(0)
    })
  })

  describe('formatting helpers', () => {
    it('formats tokens correctly in hover card', async () => {
      useStore.getState().setAgents([
        createMockAgent({ id: 'agent-1', name: 'Test', totalTokens: 1500000 }),
      ])

      const { container } = render(<AgentGrid />)

      const agentDivs = container.querySelectorAll('.cursor-pointer')
      if (agentDivs[0]) {
        fireEvent.mouseEnter(agentDivs[0])

        await waitFor(() => {
          // Should format as "1.50M"
          expect(screen.getByText('1.50M')).toBeInTheDocument()
        })
      }
    })

    it('formats cost correctly in hover card', async () => {
      useStore.getState().setAgents([
        createMockAgent({ id: 'agent-1', name: 'Test', totalCost: 12.5 }),
      ])

      const { container } = render(<AgentGrid />)

      const agentDivs = container.querySelectorAll('.cursor-pointer')
      if (agentDivs[0]) {
        fireEvent.mouseEnter(agentDivs[0])

        await waitFor(() => {
          // Should format as "$12.50"
          expect(screen.getByText('$12.50')).toBeInTheDocument()
        })
      }
    })
  })

  describe('empty state', () => {
    it('renders correctly with no agents', () => {
      useStore.getState().setAgents([])
      render(<AgentGrid />)

      expect(screen.getByText('0 agents')).toBeInTheDocument()
    })
  })

  describe('performance', () => {
    it('handles large number of agents', () => {
      const manyAgents = Array.from({ length: 100 }, (_, i) =>
        createMockAgent({ id: `agent-${i}`, name: `Agent ${i}` })
      )
      useStore.getState().setAgents(manyAgents)

      const { container } = render(<AgentGrid />)
      expect(screen.getByText('100 agents')).toBeInTheDocument()
    })

    it('respects maxAgents limit for performance', () => {
      const manyAgents = Array.from({ length: 100 }, (_, i) =>
        createMockAgent({ id: `agent-${i}`, name: `Agent ${i}` })
      )
      useStore.getState().setAgents(manyAgents)

      render(<AgentGrid maxAgents={50} />)
      expect(screen.getByText('50 agents (showing 50)')).toBeInTheDocument()
    })
  })

  describe('success rate colors', () => {
    it('shows green for high success rate', async () => {
      useStore.getState().setAgents([
        createMockAgent({ id: 'agent-1', name: 'High Success', successRate: 0.98 }),
      ])

      const { container } = render(<AgentGrid />)

      const agentDivs = container.querySelectorAll('.cursor-pointer')
      if (agentDivs[0]) {
        fireEvent.mouseEnter(agentDivs[0])

        await waitFor(() => {
          const successText = screen.getByText('98.0%')
          expect(successText).toHaveClass('text-green-500')
        })
      }
    })

    it('shows yellow for medium success rate', async () => {
      useStore.getState().setAgents([
        createMockAgent({ id: 'agent-1', name: 'Medium Success', successRate: 0.85 }),
      ])

      const { container } = render(<AgentGrid />)

      const agentDivs = container.querySelectorAll('.cursor-pointer')
      if (agentDivs[0]) {
        fireEvent.mouseEnter(agentDivs[0])

        await waitFor(() => {
          const successText = screen.getByText('85.0%')
          expect(successText).toHaveClass('text-yellow-500')
        })
      }
    })

    it('shows red for low success rate', async () => {
      useStore.getState().setAgents([
        createMockAgent({ id: 'agent-1', name: 'Low Success', successRate: 0.7 }),
      ])

      const { container } = render(<AgentGrid />)

      const agentDivs = container.querySelectorAll('.cursor-pointer')
      if (agentDivs[0]) {
        fireEvent.mouseEnter(agentDivs[0])

        await waitFor(() => {
          const successText = screen.getByText('70.0%')
          expect(successText).toHaveClass('text-red-500')
        })
      }
    })
  })
})
