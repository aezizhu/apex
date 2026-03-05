import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { WorkflowProperties } from '@/components/workflow/WorkflowProperties'
import { useStore } from '@/lib/store'
import type { WorkflowNodeData, WorkflowEdgeData } from '@/components/workflow/types'

const mockTaskNode: WorkflowNodeData = {
  id: 'node-1',
  type: 'task',
  label: 'Research Task',
  x: 100,
  y: 200,
  agentId: 'agent-001',
  instructions: 'Research the topic',
  maxRetries: 3,
  timeoutMs: 60000,
}

const mockConditionNode: WorkflowNodeData = {
  id: 'node-2',
  type: 'condition',
  label: 'Check Result',
  x: 300,
  y: 200,
  conditionExpression: 'result.status === "success"',
}

const mockApprovalNode: WorkflowNodeData = {
  id: 'node-3',
  type: 'approval',
  label: 'Human Review',
  x: 500,
  y: 200,
  approvalThreshold: 0.8,
}

const mockEdge: WorkflowEdgeData = {
  id: 'edge-1',
  source: 'node-1-source',
  target: 'node-2-target',
  label: 'success path',
  conditionBranch: 'true',
}

const defaultProps = {
  selectedNode: null as WorkflowNodeData | null,
  selectedEdge: null as WorkflowEdgeData | null,
  onUpdateNode: vi.fn(),
  onUpdateEdge: vi.fn(),
  onClose: vi.fn(),
}

describe('WorkflowProperties', () => {
  beforeEach(() => {
    useStore.getState().setAgents([])
  })

  describe('empty state', () => {
    it('shows hint when nothing selected', () => {
      render(<WorkflowProperties {...defaultProps} />)
      expect(screen.getByText('Select a node or edge to view its properties')).toBeInTheDocument()
    })
  })

  describe('node properties', () => {
    it('shows Task Properties header for task node', () => {
      render(<WorkflowProperties {...defaultProps} selectedNode={mockTaskNode} />)
      expect(screen.getByText('Task Properties')).toBeInTheDocument()
    })

    it('shows name input with node label', () => {
      render(<WorkflowProperties {...defaultProps} selectedNode={mockTaskNode} />)
      expect(screen.getByDisplayValue('Research Task')).toBeInTheDocument()
    })

    it('shows Instructions textarea for task nodes', () => {
      render(<WorkflowProperties {...defaultProps} selectedNode={mockTaskNode} />)
      expect(screen.getByText('Instructions')).toBeInTheDocument()
      expect(screen.getByDisplayValue('Research the topic')).toBeInTheDocument()
    })

    it('shows Assigned Agent select for task nodes', () => {
      render(<WorkflowProperties {...defaultProps} selectedNode={mockTaskNode} />)
      expect(screen.getByText('Assigned Agent')).toBeInTheDocument()
      expect(screen.getByText('Auto-assign')).toBeInTheDocument()
    })

    it('shows Max Retries and Timeout inputs', () => {
      render(<WorkflowProperties {...defaultProps} selectedNode={mockTaskNode} />)
      expect(screen.getByText('Max Retries')).toBeInTheDocument()
      expect(screen.getByText('Timeout (ms)')).toBeInTheDocument()
      expect(screen.getByText('Limits')).toBeInTheDocument()
    })

    it('shows node ID and position', () => {
      render(<WorkflowProperties {...defaultProps} selectedNode={mockTaskNode} />)
      expect(screen.getByText('node-1')).toBeInTheDocument()
      expect(screen.getByText('(100, 200)')).toBeInTheDocument()
    })

    it('calls onUpdateNode when name changes', async () => {
      const onUpdateNode = vi.fn()
      const user = userEvent.setup()
      render(<WorkflowProperties {...defaultProps} selectedNode={mockTaskNode} onUpdateNode={onUpdateNode} />)

      const nameInput = screen.getByDisplayValue('Research Task')
      await user.clear(nameInput)
      await user.type(nameInput, 'New Name')
      expect(onUpdateNode).toHaveBeenCalledWith('node-1', expect.objectContaining({ label: expect.any(String) }))
    })

    it('calls onClose when close button clicked', async () => {
      const onClose = vi.fn()
      const user = userEvent.setup()
      render(<WorkflowProperties {...defaultProps} selectedNode={mockTaskNode} onClose={onClose} />)

      // The close button has an X icon
      const buttons = screen.getAllByRole('button')
      const closeBtn = buttons.find(b => b.querySelector('svg'))
      if (closeBtn) await user.click(closeBtn)
      expect(onClose).toHaveBeenCalled()
    })
  })

  describe('condition node properties', () => {
    it('shows Condition Properties header', () => {
      render(<WorkflowProperties {...defaultProps} selectedNode={mockConditionNode} />)
      expect(screen.getByText('Condition Properties')).toBeInTheDocument()
    })

    it('shows Condition Expression textarea', () => {
      render(<WorkflowProperties {...defaultProps} selectedNode={mockConditionNode} />)
      expect(screen.getByText('Condition Expression')).toBeInTheDocument()
      expect(screen.getByDisplayValue('result.status === "success"')).toBeInTheDocument()
    })

    it('does not show Instructions for condition nodes', () => {
      render(<WorkflowProperties {...defaultProps} selectedNode={mockConditionNode} />)
      expect(screen.queryByText('Instructions')).not.toBeInTheDocument()
    })

    it('does not show Assigned Agent for condition nodes', () => {
      render(<WorkflowProperties {...defaultProps} selectedNode={mockConditionNode} />)
      expect(screen.queryByText('Assigned Agent')).not.toBeInTheDocument()
    })
  })

  describe('approval node properties', () => {
    it('shows Approval Gate Properties header', () => {
      render(<WorkflowProperties {...defaultProps} selectedNode={mockApprovalNode} />)
      expect(screen.getByText('Approval Gate Properties')).toBeInTheDocument()
    })

    it('shows Risk Threshold input', () => {
      render(<WorkflowProperties {...defaultProps} selectedNode={mockApprovalNode} />)
      expect(screen.getByText('Risk Threshold')).toBeInTheDocument()
    })

    it('shows Assigned Agent for approval nodes', () => {
      render(<WorkflowProperties {...defaultProps} selectedNode={mockApprovalNode} />)
      expect(screen.getByText('Assigned Agent')).toBeInTheDocument()
    })
  })

  describe('edge properties', () => {
    it('shows Edge Properties header', () => {
      render(<WorkflowProperties {...defaultProps} selectedEdge={mockEdge} />)
      expect(screen.getByText('Edge Properties')).toBeInTheDocument()
    })

    it('shows label input with edge label', () => {
      render(<WorkflowProperties {...defaultProps} selectedEdge={mockEdge} />)
      expect(screen.getByDisplayValue('success path')).toBeInTheDocument()
    })

    it('shows Condition Branch selector', () => {
      render(<WorkflowProperties {...defaultProps} selectedEdge={mockEdge} />)
      expect(screen.getByText('Condition Branch')).toBeInTheDocument()
      expect(screen.getByText('None')).toBeInTheDocument()
      expect(screen.getByText('true')).toBeInTheDocument()
      expect(screen.getByText('false')).toBeInTheDocument()
    })

    it('shows source and target IDs', () => {
      render(<WorkflowProperties {...defaultProps} selectedEdge={mockEdge} />)
      expect(screen.getByText(/Source:/)).toBeInTheDocument()
      expect(screen.getByText(/Target:/)).toBeInTheDocument()
    })

    it('calls onUpdateEdge when label changes', async () => {
      const onUpdateEdge = vi.fn()
      const user = userEvent.setup()
      render(<WorkflowProperties {...defaultProps} selectedEdge={mockEdge} onUpdateEdge={onUpdateEdge} />)

      const labelInput = screen.getByDisplayValue('success path')
      await user.clear(labelInput)
      await user.type(labelInput, 'new label')
      expect(onUpdateEdge).toHaveBeenCalledWith('edge-1', expect.objectContaining({ label: expect.any(String) }))
    })

    it('calls onUpdateEdge when condition branch clicked', async () => {
      const onUpdateEdge = vi.fn()
      const user = userEvent.setup()
      render(<WorkflowProperties {...defaultProps} selectedEdge={mockEdge} onUpdateEdge={onUpdateEdge} />)

      await user.click(screen.getByText('false'))
      expect(onUpdateEdge).toHaveBeenCalledWith('edge-1', expect.objectContaining({ conditionBranch: 'false' }))
    })
  })
})
