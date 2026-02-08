import { describe, it, expect, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { WorkflowToolbar } from '@/components/workflow/WorkflowToolbar'

const defaultProps = {
  onAddNode: vi.fn(),
  onDelete: vi.fn(),
  onZoomIn: vi.fn(),
  onZoomOut: vi.fn(),
  onFitView: vi.fn(),
  onUndo: vi.fn(),
  onRedo: vi.fn(),
  onExport: vi.fn(),
  onImport: vi.fn(),
  onSave: vi.fn(),
  onExecute: vi.fn(),
  canUndo: false,
  canRedo: false,
  hasSelection: false,
  workflowName: 'My Workflow',
  onNameChange: vi.fn(),
}

describe('WorkflowToolbar', () => {
  describe('rendering', () => {
    it('renders workflow name input', () => {
      render(<WorkflowToolbar {...defaultProps} />)
      const input = screen.getByDisplayValue('My Workflow')
      expect(input).toBeInTheDocument()
    })

    it('renders action buttons', () => {
      render(<WorkflowToolbar {...defaultProps} />)
      expect(screen.getByText('Import')).toBeInTheDocument()
      expect(screen.getByText('Export')).toBeInTheDocument()
      expect(screen.getByText('Save')).toBeInTheDocument()
      expect(screen.getByText('Execute')).toBeInTheDocument()
    })

    it('renders node palette with all 5 node types', () => {
      render(<WorkflowToolbar {...defaultProps} />)
      expect(screen.getByText('Task')).toBeInTheDocument()
      expect(screen.getByText('Condition')).toBeInTheDocument()
      expect(screen.getByText('Fork')).toBeInTheDocument()
      expect(screen.getByText('Join')).toBeInTheDocument()
      expect(screen.getByText('Approval Gate')).toBeInTheDocument()
    })

    it('renders Nodes label', () => {
      render(<WorkflowToolbar {...defaultProps} />)
      expect(screen.getByText('Nodes')).toBeInTheDocument()
    })
  })

  describe('undo/redo buttons', () => {
    it('disables Undo when canUndo is false', () => {
      render(<WorkflowToolbar {...defaultProps} canUndo={false} />)
      const undoBtn = screen.getByTitle('Undo')
      expect(undoBtn).toBeDisabled()
    })

    it('enables Undo when canUndo is true', () => {
      render(<WorkflowToolbar {...defaultProps} canUndo={true} />)
      const undoBtn = screen.getByTitle('Undo')
      expect(undoBtn).not.toBeDisabled()
    })

    it('disables Redo when canRedo is false', () => {
      render(<WorkflowToolbar {...defaultProps} canRedo={false} />)
      const redoBtn = screen.getByTitle('Redo')
      expect(redoBtn).toBeDisabled()
    })

    it('enables Redo when canRedo is true', () => {
      render(<WorkflowToolbar {...defaultProps} canRedo={true} />)
      const redoBtn = screen.getByTitle('Redo')
      expect(redoBtn).not.toBeDisabled()
    })
  })

  describe('delete button', () => {
    it('disables Delete when hasSelection is false', () => {
      render(<WorkflowToolbar {...defaultProps} hasSelection={false} />)
      const deleteBtn = screen.getByTitle('Delete Selected')
      expect(deleteBtn).toBeDisabled()
    })

    it('enables Delete when hasSelection is true', () => {
      render(<WorkflowToolbar {...defaultProps} hasSelection={true} />)
      const deleteBtn = screen.getByTitle('Delete Selected')
      expect(deleteBtn).not.toBeDisabled()
    })
  })

  describe('interactions', () => {
    it('calls onSave when Save is clicked', async () => {
      const onSave = vi.fn()
      const user = userEvent.setup()
      render(<WorkflowToolbar {...defaultProps} onSave={onSave} />)

      await user.click(screen.getByText('Save'))
      expect(onSave).toHaveBeenCalledOnce()
    })

    it('calls onExecute when Execute is clicked', async () => {
      const onExecute = vi.fn()
      const user = userEvent.setup()
      render(<WorkflowToolbar {...defaultProps} onExecute={onExecute} />)

      await user.click(screen.getByText('Execute'))
      expect(onExecute).toHaveBeenCalledOnce()
    })

    it('calls onImport when Import is clicked', async () => {
      const onImport = vi.fn()
      const user = userEvent.setup()
      render(<WorkflowToolbar {...defaultProps} onImport={onImport} />)

      await user.click(screen.getByText('Import'))
      expect(onImport).toHaveBeenCalledOnce()
    })

    it('calls onExport when Export is clicked', async () => {
      const onExport = vi.fn()
      const user = userEvent.setup()
      render(<WorkflowToolbar {...defaultProps} onExport={onExport} />)

      await user.click(screen.getByText('Export'))
      expect(onExport).toHaveBeenCalledOnce()
    })

    it('calls onAddNode with correct type when palette button clicked', async () => {
      const onAddNode = vi.fn()
      const user = userEvent.setup()
      render(<WorkflowToolbar {...defaultProps} onAddNode={onAddNode} />)

      await user.click(screen.getByText('Task'))
      expect(onAddNode).toHaveBeenCalledWith('task')

      await user.click(screen.getByText('Condition'))
      expect(onAddNode).toHaveBeenCalledWith('condition')

      await user.click(screen.getByText('Fork'))
      expect(onAddNode).toHaveBeenCalledWith('fork')
    })

    it('calls onNameChange when name input changes', async () => {
      const onNameChange = vi.fn()
      const user = userEvent.setup()
      render(<WorkflowToolbar {...defaultProps} onNameChange={onNameChange} />)

      const input = screen.getByDisplayValue('My Workflow')
      await user.clear(input)
      await user.type(input, 'New Name')
      expect(onNameChange).toHaveBeenCalled()
    })

    it('calls onZoomIn when Zoom In clicked', async () => {
      const onZoomIn = vi.fn()
      const user = userEvent.setup()
      render(<WorkflowToolbar {...defaultProps} onZoomIn={onZoomIn} />)

      await user.click(screen.getByTitle('Zoom In'))
      expect(onZoomIn).toHaveBeenCalledOnce()
    })

    it('calls onZoomOut when Zoom Out clicked', async () => {
      const onZoomOut = vi.fn()
      const user = userEvent.setup()
      render(<WorkflowToolbar {...defaultProps} onZoomOut={onZoomOut} />)

      await user.click(screen.getByTitle('Zoom Out'))
      expect(onZoomOut).toHaveBeenCalledOnce()
    })

    it('calls onFitView when Fit View clicked', async () => {
      const onFitView = vi.fn()
      const user = userEvent.setup()
      render(<WorkflowToolbar {...defaultProps} onFitView={onFitView} />)

      await user.click(screen.getByTitle('Fit View'))
      expect(onFitView).toHaveBeenCalledOnce()
    })

    it('calls onDelete when Delete Selected clicked', async () => {
      const onDelete = vi.fn()
      const user = userEvent.setup()
      render(<WorkflowToolbar {...defaultProps} hasSelection={true} onDelete={onDelete} />)

      await user.click(screen.getByTitle('Delete Selected'))
      expect(onDelete).toHaveBeenCalledOnce()
    })
  })
})
