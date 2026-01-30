import { describe, it, expect, vi } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import AgentSight from '@/components/AgentSight/AgentSight'
import type { AgentScreen } from '@/components/AgentSight/AgentSight'

vi.mock('framer-motion', () => ({
  motion: {
    div: ({ children, ...props }: React.PropsWithChildren<Record<string, unknown>>) => <div>{children}</div>,
    span: ({ children, ...props }: React.PropsWithChildren<Record<string, unknown>>) => <span>{children}</span>,
  },
  AnimatePresence: ({ children }: React.PropsWithChildren) => <>{children}</>,
}))

const createAgent = (overrides: Partial<AgentScreen> = {}): AgentScreen => ({
  agentId: 'agent-1', agentName: 'Code Writer', status: 'busy',
  currentAction: 'Writing tests', screenContent: 'Analyzing coverage.',
  tokensUsed: 5000, costSoFar: 0.25, startedAt: new Date(Date.now() - 60000).toISOString(),
  ...overrides,
})

describe('AgentSight', () => {
  it('renders agent cards', () => {
    render(<AgentSight agents={[createAgent({ agentId: 'a1', agentName: 'Writer' }), createAgent({ agentId: 'a2', agentName: 'Reviewer' })]} />)
    expect(screen.getByText('Writer')).toBeInTheDocument()
    expect(screen.getByText('Reviewer')).toBeInTheDocument()
  })
  it('displays current action', () => {
    render(<AgentSight agents={[createAgent({ currentAction: 'Parsing AST' })]} />)
    expect(screen.getByText('Parsing AST')).toBeInTheDocument()
  })
  it('displays screen content', () => {
    render(<AgentSight agents={[createAgent({ screenContent: 'Reviewing PR #42' })]} />)
    expect(screen.getByText('Reviewing PR #42')).toBeInTheDocument()
  })
  it('displays focus area', () => {
    render(<AgentSight agents={[createAgent({ focusArea: 'Auth Module' })]} />)
    expect(screen.getByText('Auth Module')).toBeInTheDocument()
  })
  it('displays tool call', () => {
    render(<AgentSight agents={[createAgent({ lastToolCall: { name: 'file_read', params: { path: '/src/auth.ts' } } })]} />)
    expect(screen.getByText('file_read')).toBeInTheDocument()
  })
  it('opens expanded modal', () => {
    render(<AgentSight agents={[createAgent({ agentName: 'ExpandMe' })]} />)
    fireEvent.click(screen.getByText('ExpandMe'))
    expect(screen.getByText('agent-1')).toBeInTheDocument()
  })
  it('shows formatted stats', () => {
    render(<AgentSight agents={[createAgent({ tokensUsed: 5000, costSoFar: 0.25 })]} />)
    expect(screen.getAllByText('5.0K').length).toBeGreaterThanOrEqual(1)
    expect(screen.getAllByText('$0.2500').length).toBeGreaterThanOrEqual(1)
  })
})
