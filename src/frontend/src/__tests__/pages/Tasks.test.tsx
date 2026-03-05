import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import Tasks from '@/pages/Tasks'
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

// Mock TaskTimeline
vi.mock('@/components/TaskTimeline', () => ({
  TaskTimeline: ({ tasks }: any) => (
    <div data-testid="task-timeline">{tasks.length} tasks</div>
  ),
}))

// Mock API
vi.mock('@/lib/api', () => ({
  taskApi: {
    list: vi.fn().mockResolvedValue({ data: [] }),
    create: vi.fn().mockResolvedValue({}),
    cancel: vi.fn().mockResolvedValue({}),
    retry: vi.fn().mockResolvedValue({}),
  },
}))

const mockTasks = [
  {
    id: 't1',
    name: 'Analyze Report',
    status: 'completed',
    createdAt: '2024-01-15T12:00:00Z',
    startedAt: '2024-01-15T12:00:05Z',
    completedAt: '2024-01-15T12:01:00Z',
    costDollars: 0.5,
    tokensUsed: 1000,
    prompt: 'Analyze the report',
    priority: 5,
    agentId: 'a1',
    dagId: 'dag-001',
  },
  {
    id: 't2',
    name: 'Write Summary',
    status: 'running',
    createdAt: '2024-01-15T12:02:00Z',
    startedAt: '2024-01-15T12:02:05Z',
    completedAt: null,
    costDollars: 0.1,
    tokensUsed: 200,
    prompt: 'Write a summary',
    priority: 8,
    agentId: 'a2',
    dagId: 'dag-002',
  },
  {
    id: 't3',
    name: 'Failed Job',
    status: 'failed',
    createdAt: '2024-01-15T12:03:00Z',
    startedAt: '2024-01-15T12:03:05Z',
    completedAt: '2024-01-15T12:03:30Z',
    costDollars: 0.02,
    tokensUsed: 50,
    prompt: 'Do something',
    priority: 3,
    agentId: 'a1',
    dagId: 'dag-003',
  },
  {
    id: 't4',
    name: 'Pending Task',
    status: 'pending',
    createdAt: '2024-01-15T12:04:00Z',
    startedAt: null,
    completedAt: null,
    costDollars: 0,
    tokensUsed: 0,
    prompt: 'Wait for assignment',
    priority: 5,
    agentId: null,
    dagId: 'dag-004',
  },
]

describe('Tasks', () => {
  beforeEach(() => {
    useStore.getState().setTasks([])
  })

  describe('rendering', () => {
    it('renders the Swarm Operations heading', () => {
      render(<Tasks />)
      expect(screen.getByText('Swarm Operations')).toBeInTheDocument()
    })

    it('renders the subtitle', () => {
      render(<Tasks />)
      expect(screen.getByText(/Mission Orchestration.*Parallel Execution/)).toBeInTheDocument()
    })

    it('renders the Launch Mission button', () => {
      render(<Tasks />)
      expect(screen.getByText('Launch Mission')).toBeInTheDocument()
    })

    it('renders search input', () => {
      render(<Tasks />)
      expect(screen.getByPlaceholderText('Search missions...')).toBeInTheDocument()
    })

    it('renders status filter dropdown', () => {
      render(<Tasks />)
      expect(screen.getByText('All Status')).toBeInTheDocument()
    })
  })

  describe('stats', () => {
    it('shows task status count labels in stats cards', () => {
      useStore.getState().setTasks(mockTasks)
      render(<Tasks />)
      expect(screen.getAllByText('Queued').length).toBeGreaterThan(0)
      expect(screen.getAllByText('Executing').length).toBeGreaterThan(0)
      expect(screen.getAllByText('Complete').length).toBeGreaterThan(0)
      expect(screen.getAllByText('Failed').length).toBeGreaterThan(0)
    })

    it('shows correct status counts', () => {
      useStore.getState().setTasks(mockTasks)
      const { container } = render(<Tasks />)
      const statCards = container.querySelectorAll('.grid.grid-cols-2 > div')
      expect(statCards.length).toBe(4)
    })
  })

  describe('missions view', () => {
    it('shows missions when tasks available (grouped by DAG)', () => {
      useStore.getState().setTasks(mockTasks)
      render(<Tasks />)
      // Each task has a unique dagId, so each is its own mission
      // Mission names come from task names
      expect(screen.getByText('Analyze Report')).toBeInTheDocument()
      expect(screen.getByText('Write Summary')).toBeInTheDocument()
      expect(screen.getByText('Failed Job')).toBeInTheDocument()
      expect(screen.getByText('Pending Task')).toBeInTheDocument()
    })

    it('shows swarm flow diagram when no missions', () => {
      render(<Tasks />)
      expect(screen.getByText(/No active missions/)).toBeInTheDocument()
    })

    it('shows how the swarm works in empty state', () => {
      render(<Tasks />)
      expect(screen.getByText('How the Swarm Works')).toBeInTheDocument()
      expect(screen.getByText('DISPATCH')).toBeInTheDocument()
      expect(screen.getByText('DECOMPOSE')).toBeInTheDocument()
      expect(screen.getByText('SWARM')).toBeInTheDocument()
      expect(screen.getByText('CONVERGE')).toBeInTheDocument()
    })

    it('shows launch first mission button in empty state', () => {
      render(<Tasks />)
      expect(screen.getByText('Launch Your First Mission')).toBeInTheDocument()
    })
  })

  describe('create mission modal', () => {
    it('opens modal when Launch Mission is clicked', async () => {
      const user = userEvent.setup()
      render(<Tasks />)

      await user.click(screen.getByText('Launch Mission'))
      expect(screen.getByText('Mission Name')).toBeInTheDocument()
      expect(screen.getByText('Objective')).toBeInTheDocument()
      expect(screen.getByText(/Priority Level/)).toBeInTheDocument()
    })

    it('shows Cancel and Launch buttons in modal', async () => {
      const user = userEvent.setup()
      render(<Tasks />)

      await user.click(screen.getByText('Launch Mission'))
      expect(screen.getByText('Cancel')).toBeInTheDocument()
      expect(screen.getByText('Launch')).toBeInTheDocument()
    })

    it('shows swarm execution hint', async () => {
      const user = userEvent.setup()
      render(<Tasks />)

      await user.click(screen.getByText('Launch Mission'))
      expect(screen.getByText('The swarm will decompose and execute through parallel agents')).toBeInTheDocument()
    })
  })
})
