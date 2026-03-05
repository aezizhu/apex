import { describe, it, expect, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
import Workflows from '@/pages/Workflows'

// Mock react-hot-toast
vi.mock('react-hot-toast', () => ({
  default: {
    success: vi.fn(),
    error: vi.fn(),
  },
}))

// Mock API
vi.mock('@/lib/api', () => ({
  dagApi: {
    create: vi.fn().mockResolvedValue({}),
  },
}))

// Mock WorkflowBuilder
vi.mock('@/components/workflow/WorkflowBuilder', () => ({
  WorkflowBuilder: ({ onSave, onExecute }: any) => (
    <div data-testid="workflow-builder">
      <button data-testid="save-btn" onClick={() => onSave({ name: 'test', nodes: [], edges: [] })}>
        Save
      </button>
      <button data-testid="execute-btn" onClick={() => onExecute({ name: 'test', nodes: [], edges: [] })}>
        Execute
      </button>
    </div>
  ),
}))

describe('Workflows', () => {
  it('renders WorkflowBuilder component', () => {
    render(<Workflows />)
    expect(screen.getByTestId('workflow-builder')).toBeInTheDocument()
  })

  it('passes onSave callback to WorkflowBuilder', () => {
    render(<Workflows />)
    expect(screen.getByTestId('save-btn')).toBeInTheDocument()
  })

  it('passes onExecute callback to WorkflowBuilder', () => {
    render(<Workflows />)
    expect(screen.getByTestId('execute-btn')).toBeInTheDocument()
  })

  it('renders with correct viewport height', () => {
    const { container } = render(<Workflows />)
    const wrapper = container.firstChild as HTMLElement
    expect(wrapper).toHaveClass('h-[calc(100vh-8rem)]')
  })
})
