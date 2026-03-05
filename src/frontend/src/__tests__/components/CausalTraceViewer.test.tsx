import { describe, it, expect, vi } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import { CausalTraceViewer, type TraceStep } from '@/components/CausalTrace/CausalTraceViewer'

vi.mock('framer-motion', () => ({
  motion: { div: ({ children, ...props }: React.PropsWithChildren<Record<string, unknown>>) => <div>{children}</div> },
  AnimatePresence: ({ children }: React.PropsWithChildren) => <>{children}</>,
}))

const createStep = (overrides: Partial<TraceStep> = {}): TraceStep => ({
  id: 'step-1', type: 'llm_call', label: 'Generate response',
  timestamp: '2024-01-15T10:00:00Z', durationMs: 1500,
  tokensUsed: 500, costDollars: 0.02, ...overrides,
})

describe('CausalTraceViewer', () => {
  it('renders empty state', () => { render(<CausalTraceViewer steps={[]} />); expect(screen.getByText('No trace data available')).toBeInTheDocument() })
  it('renders summary bar', () => {
    render(<CausalTraceViewer steps={[createStep(), createStep({ id: 's2', tokensUsed: 300 })]} />)
    expect(screen.getByText('2 steps')).toBeInTheDocument()
    expect(screen.getByText('800 tokens')).toBeInTheDocument()
  })
  it('renders step labels', () => {
    render(<CausalTraceViewer steps={[createStep({ type: 'llm_call', label: 'Generate response' })]} />)
    expect(screen.getByText('Generate response')).toBeInTheDocument()
    expect(screen.getByText('LLM Call')).toBeInTheDocument()
  })
  it('expands step details', () => {
    render(<CausalTraceViewer steps={[createStep({ prompt: 'What is life?', response: '42' })]} />)
    fireEvent.click(screen.getAllByRole('button')[0])
    expect(screen.getByText('What is life?')).toBeInTheDocument()
    expect(screen.getByText('42')).toBeInTheDocument()
  })
  it('shows error details', () => {
    render(<CausalTraceViewer steps={[createStep({ type: 'error', label: 'API Error', errorMessage: 'Timeout' })]} />)
    fireEvent.click(screen.getAllByRole('button')[0])
    expect(screen.getByText('Timeout')).toBeInTheDocument()
  })
  it('shows tool details', () => {
    render(<CausalTraceViewer steps={[createStep({ type: 'tool_call', label: 'Read', toolName: 'file_read', toolInput: '/path', toolOutput: 'content' })]} />)
    fireEvent.click(screen.getAllByRole('button')[0])
    expect(screen.getByText('/path')).toBeInTheDocument()
    expect(screen.getByText('content')).toBeInTheDocument()
  })
  it('renders Jaeger link', () => {
    render(<CausalTraceViewer steps={[createStep({ jaegerTraceId: 'abc123' })]} jaegerBaseUrl="http://jaeger:16686" />)
    fireEvent.click(screen.getAllByRole('button')[0])
    const link = screen.getByText('View in Jaeger')
    expect(link.closest('a')).toHaveAttribute('href', 'http://jaeger:16686/trace/abc123')
  })
  it('renders nested children', () => {
    render(<CausalTraceViewer steps={[createStep({ label: 'Parent', children: [createStep({ id: 'c1', label: 'Child' })] })]} />)
    expect(screen.getByText('Parent')).toBeInTheDocument()
    expect(screen.getByText('Child')).toBeInTheDocument()
  })
})
