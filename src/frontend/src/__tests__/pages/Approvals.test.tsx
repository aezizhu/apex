import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import Approvals from '@/pages/Approvals'
import { useStore } from '@/lib/store'
import { approvalApi } from '@/lib/api'

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

// Mock API
vi.mock('@/lib/api', () => ({
  approvalApi: {
    list: vi.fn().mockResolvedValue({ data: [] }),
    approve: vi.fn().mockResolvedValue({}),
    deny: vi.fn().mockResolvedValue({}),
  },
}))

const mockApprovals = [
  {
    id: 'ap1',
    agentId: 'agent-001',
    taskId: 'task-001',
    actionType: 'tool_call',
    riskScore: 0.85,
    status: 'pending',
    actionData: { tool: 'web_search', args: { query: 'test' } },
    createdAt: '2024-01-15T12:00:00Z',
  },
  {
    id: 'ap2',
    agentId: 'agent-002',
    taskId: 'task-002',
    actionType: 'cost_override',
    riskScore: 0.3,
    status: 'approved',
    actionData: { amount: 5.0 },
    createdAt: '2024-01-15T11:00:00Z',
  },
  {
    id: 'ap3',
    agentId: 'agent-003',
    taskId: 'task-003',
    actionType: 'external_call',
    riskScore: 0.6,
    status: 'pending',
    actionData: { endpoint: 'https://api.example.com' },
    createdAt: '2024-01-15T10:00:00Z',
  },
]

describe('Approvals', () => {
  beforeEach(() => {
    useStore.getState().setApprovals([])
  })

  describe('rendering', () => {
    it('renders the Approval Queue heading', () => {
      render(<Approvals />)
      expect(screen.getByText('Approval Queue')).toBeInTheDocument()
    })

    it('renders the subtitle', () => {
      render(<Approvals />)
      expect(screen.getByText('Review and approve high-impact agent actions')).toBeInTheDocument()
    })

    it('renders filter buttons', () => {
      render(<Approvals />)
      // Filter buttons: Pending, Resolved, All
      const buttons = screen.getAllByRole('button')
      const filterTexts = buttons.map((b) => b.textContent)
      expect(filterTexts).toContain('Pending')
      expect(filterTexts).toContain('Resolved')
      expect(filterTexts).toContain('All')
    })

    it('renders keyboard shortcuts hint', () => {
      render(<Approvals />)
      expect(screen.getByText(/navigate/i)).toBeInTheDocument()
    })
  })

  describe('stats', () => {
    it('shows pending count in stats card', async () => {
      vi.mocked(approvalApi.list).mockResolvedValueOnce({ data: mockApprovals })
      render(<Approvals />)
      await waitFor(() => {
        const statCards = screen.getAllByText('2')
        expect(statCards.length).toBeGreaterThan(0)
      })
    })

    it('shows approved and denied stats', () => {
      render(<Approvals />)
      expect(screen.getByText('Approved')).toBeInTheDocument()
      expect(screen.getByText('Denied')).toBeInTheDocument()
    })
  })

  describe('filtering', () => {
    it('defaults to pending filter showing pending items', async () => {
      vi.mocked(approvalApi.list).mockResolvedValueOnce({ data: mockApprovals })
      render(<Approvals />)
      // Wait for fetch to settle and items to appear
      await waitFor(() => {
        expect(screen.getByText('tool_call')).toBeInTheDocument()
      })
      expect(screen.getByText('external_call')).toBeInTheDocument()
      // Should not show resolved item (ap2)
      expect(screen.queryByText('cost_override')).not.toBeInTheDocument()
    })

    it('switches to All filter', async () => {
      vi.mocked(approvalApi.list).mockResolvedValueOnce({ data: mockApprovals })
      const user = userEvent.setup()
      render(<Approvals />)

      await waitFor(() => {
        expect(screen.getByText('tool_call')).toBeInTheDocument()
      })

      await user.click(screen.getByRole('button', { name: 'All' }))
      expect(screen.getByText('tool_call')).toBeInTheDocument()
      expect(screen.getByText('cost_override')).toBeInTheDocument()
      expect(screen.getByText('external_call')).toBeInTheDocument()
    })
  })

  describe('approval actions', () => {
    it('shows Approve All button when pending items exist', async () => {
      vi.mocked(approvalApi.list).mockResolvedValueOnce({ data: mockApprovals })
      render(<Approvals />)
      await waitFor(() => {
        expect(screen.getByText(/Approve All/)).toBeInTheDocument()
      })
    })

    it('does not show Approve All when no pending items', async () => {
      vi.mocked(approvalApi.list).mockResolvedValueOnce({ data: [mockApprovals[1]] })
      render(<Approvals />)
      // Wait for fetch to settle
      await waitFor(() => {
        expect(screen.getByText('Approved')).toBeInTheDocument()
      })
      expect(screen.queryByText(/Approve All/)).not.toBeInTheDocument()
    })
  })

  describe('risk scores', () => {
    it('displays risk scores for approval items', async () => {
      vi.mocked(approvalApi.list).mockResolvedValueOnce({ data: mockApprovals })
      render(<Approvals />)
      await waitFor(() => {
        expect(screen.getByText('Risk: 85%')).toBeInTheDocument()
      })
      expect(screen.getByText('Risk: 60%')).toBeInTheDocument()
    })
  })

  describe('empty state', () => {
    it('shows message when no pending approvals', () => {
      render(<Approvals />)
      expect(screen.getByText('No pending approvals')).toBeInTheDocument()
    })

    it('shows generic message when "all" filter has no items', async () => {
      const user = userEvent.setup()
      render(<Approvals />)
      await user.click(screen.getByRole('button', { name: 'All' }))
      expect(screen.getByText('No approvals found')).toBeInTheDocument()
    })
  })
})
