import { describe, it, expect, vi } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import { InterventionPanel } from '@/components/InterventionPanel/InterventionPanel'
import type { Agent } from '@/lib/store'

vi.mock('framer-motion', () => ({
  motion: { div: ({ children, ...props }: React.PropsWithChildren<Record<string, unknown>>) => <div>{children}</div> },
  AnimatePresence: ({ children }: React.PropsWithChildren) => <>{children}</>,
}))
vi.mock('@radix-ui/react-alert-dialog', () => {
  const C = ({ children }: React.PropsWithChildren) => <div>{children}</div>
  const P = ({ children }: React.PropsWithChildren) => <>{children}</>
  return { Root: C, Trigger: P, Portal: C, Overlay: C, Content: C, Title: C, Description: C, Cancel: P, Action: P }
})

const mockAgent: Agent = { id: 'agent-abc', name: 'Writer', model: 'gpt-4', status: 'busy', currentLoad: 3, maxLoad: 10, successRate: 0.95, reputationScore: 85, totalTokens: 5000, totalCost: 1.25 }

describe('InterventionPanel', () => {
  it('renders header', () => { render(<InterventionPanel agent={mockAgent} onClose={vi.fn()} />); expect(screen.getByText('Intervention')).toBeInTheDocument() })
  it('renders all sections', () => {
    render(<InterventionPanel agent={mockAgent} onClose={vi.fn()} />)
    expect(screen.getByText('Nudge')).toBeInTheDocument()
    expect(screen.getByText('Pause & Patch')).toBeInTheDocument()
    expect(screen.getByText('Takeover')).toBeInTheDocument()
    expect(screen.getByText('Kill Switch')).toBeInTheDocument()
  })
  it('calls onClose', () => {
    const onClose = vi.fn()
    render(<InterventionPanel agent={mockAgent} onClose={onClose} />)
    fireEvent.click(screen.getAllByRole('button')[0])
    expect(onClose).toHaveBeenCalled()
  })
  it('shows nudge section by default', () => {
    render(<InterventionPanel agent={mockAgent} onClose={vi.fn()} />)
    expect(screen.getByPlaceholderText(/Focus on completing/)).toBeInTheDocument()
  })
  it('sends nudge message', () => {
    const onSend = vi.fn()
    render(<InterventionPanel agent={mockAgent} onClose={vi.fn()} onSendMessage={onSend} />)
    fireEvent.change(screen.getByPlaceholderText(/Focus on completing/), { target: { value: 'Hurry up' } })
    fireEvent.click(screen.getByText('Send Message'))
    expect(onSend).toHaveBeenCalledWith('agent-abc', 'Hurry up')
  })
  it('toggles pause section', () => {
    render(<InterventionPanel agent={mockAgent} onClose={vi.fn()} />)
    fireEvent.click(screen.getByText('Pause & Patch'))
    expect(screen.getByText('Pause Agent')).toBeInTheDocument()
  })
  it('shows kill confirmation', () => {
    render(<InterventionPanel agent={mockAgent} onClose={vi.fn()} />)
    fireEvent.click(screen.getByText('Kill Switch'))
    expect(screen.getByText('Emergency Stop')).toBeInTheDocument()
    expect(screen.getByText('Confirm Emergency Stop')).toBeInTheDocument()
  })
  it('calls onKill', () => {
    const onKill = vi.fn()
    render(<InterventionPanel agent={mockAgent} onClose={vi.fn()} onKill={onKill} />)
    fireEvent.click(screen.getByText('Kill Switch'))
    fireEvent.click(screen.getByText('Kill Agent'))
    expect(onKill).toHaveBeenCalledWith('agent-abc')
  })
  it('handles Escape key', () => {
    const onClose = vi.fn()
    render(<InterventionPanel agent={mockAgent} onClose={onClose} />)
    fireEvent.keyDown(window, { key: 'Escape' })
    expect(onClose).toHaveBeenCalled()
  })
})
