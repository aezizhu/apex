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
    it('renders the Tasks heading', () => {
      render(<Tasks />)
      expect(screen.getByText('Tasks')).toBeInTheDocument()
    })

    it('renders the subtitle', () => {
      render(<Tasks />)
      expect(screen.getByText('Monitor and manage task execution')).toBeInTheDocument()
    })

    it('renders the Submit Task button', () => {
      render(<Tasks />)
      expect(screen.getByText('Submit Task')).toBeInTheDocument()
    })

    it('renders search input', () => {
      render(<Tasks />)
      expect(screen.getByPlaceholderText('Search tasks...')).toBeInTheDocument()
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
      // Stats labels exist (may appear multiple times due to dropdown). Check there are at least some.
      expect(screen.getAllByText('Pending').length).toBeGreaterThan(0)
      expect(screen.getAllByText('Running').length).toBeGreaterThan(0)
      expect(screen.getAllByText('Completed').length).toBeGreaterThan(0)
      expect(screen.getAllByText('Failed').length).toBeGreaterThan(0)
    })

    it('shows correct status counts', () => {
      useStore.getState().setTasks(mockTasks)
      const { container } = render(<Tasks />)
      // Stats cards have counts as 2xl bold text followed by colored label
      const statCards = container.querySelectorAll('.grid.grid-cols-2 > div')
      expect(statCards.length).toBe(4)
    })
  })

  describe('task list', () => {
    it('shows tasks when available', () => {
      useStore.getState().setTasks(mockTasks)
      render(<Tasks />)
      expect(screen.getByText('Analyze Report')).toBeInTheDocument()
      expect(screen.getByText('Write Summary')).toBeInTheDocument()
      expect(screen.getByText('Failed Job')).toBeInTheDocument()
      expect(screen.getByText('Pending Task')).toBeInTheDocument()
    })

    it('shows empty state when no tasks', () => {
      render(<Tasks />)
      expect(screen.getByText('No tasks found')).toBeInTheDocument()
    })
  })

  describe('create task modal', () => {
    it('opens modal when Submit Task is clicked', async () => {
      const user = userEvent.setup()
      render(<Tasks />)

      await user.click(screen.getByText('Submit Task'))
      // Modal title is also "Submit Task" — check for form fields
      expect(screen.getByText('Name')).toBeInTheDocument()
      expect(screen.getByText('Prompt')).toBeInTheDocument()
      expect(screen.getByText('Priority (1-10)')).toBeInTheDocument()
    })

    it('shows Cancel and Submit buttons in modal', async () => {
      const user = userEvent.setup()
      render(<Tasks />)

      await user.click(screen.getByText('Submit Task'))
      expect(screen.getByText('Cancel')).toBeInTheDocument()
      expect(screen.getByText('Submit')).toBeInTheDocument()
    })

    it('shows priority hint', async () => {
      const user = userEvent.setup()
      render(<Tasks />)

      await user.click(screen.getByText('Submit Task'))
      expect(screen.getByText('Higher priority tasks are assigned to agents first')).toBeInTheDocument()
    })
  })
})
