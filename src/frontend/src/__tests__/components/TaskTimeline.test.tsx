import { describe, it, expect, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
import { TaskTimeline } from '@/components/TaskTimeline/TaskTimeline'
import type { Task } from '@/lib/store'

vi.mock('react-plotly.js', () => ({
  default: (props: { data: unknown[]; onClick?: (event: unknown) => void }) => (
    <div data-testid="plotly-chart" onClick={() => props.onClick?.({ points: [{ pointIndex: 0 }] })}>
      Plotly Chart ({props.data.length} traces)
    </div>
  ),
}))

const createTask = (overrides: Partial<Task> = {}): Task => ({
  id: 'task-1', dagId: 'dag-1', name: 'Test Task', status: 'running',
  tokensUsed: 500, costDollars: 0.05, createdAt: '2024-01-15T10:00:00Z',
  startedAt: '2024-01-15T10:00:05Z', ...overrides,
})

describe('TaskTimeline', () => {
  it('renders empty state', () => { render(<TaskTimeline tasks={[]} />); expect(screen.getByText('No tasks to display in timeline')).toBeInTheDocument() })
  it('renders Plotly chart with tasks', () => {
    render(<TaskTimeline tasks={[createTask({ id: 't1', status: 'completed', completedAt: '2024-01-15T10:01:00Z' }), createTask({ id: 't2' })]} />)
    expect(screen.getByTestId('plotly-chart')).toBeInTheDocument()
  })
  it('groups traces by status', () => {
    render(<TaskTimeline tasks={[createTask({ id: 't1', status: 'completed', completedAt: '2024-01-15T10:01:00Z' }), createTask({ id: 't2', status: 'running' }), createTask({ id: 't3', status: 'completed', completedAt: '2024-01-15T10:02:00Z' })]} />)
    expect(screen.getByText('Plotly Chart (2 traces)')).toBeInTheDocument()
  })
  it('calls onTaskClick', () => {
    const onClick = vi.fn()
    render(<TaskTimeline tasks={[createTask()]} onTaskClick={onClick} />)
    screen.getByTestId('plotly-chart').click()
    expect(onClick).toHaveBeenCalledWith('task-1')
  })
  it('handles pending tasks without startedAt', () => {
    render(<TaskTimeline tasks={[createTask({ id: 't1', status: 'pending', startedAt: undefined })]} />)
    expect(screen.getByTestId('plotly-chart')).toBeInTheDocument()
  })
})
