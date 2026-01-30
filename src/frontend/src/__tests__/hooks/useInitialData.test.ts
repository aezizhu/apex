import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { renderHook, waitFor } from '@testing-library/react'
import { useInitialData } from '@/hooks/useInitialData'
import { useStore } from '@/lib/store'

vi.mock('@/lib/api', () => ({
  agentApi: { listRaw: vi.fn() },
  taskApi: { list: vi.fn() },
  metricsApi: { getSystem: vi.fn() },
  approvalApi: { list: vi.fn() },
}))

import { agentApi, taskApi, metricsApi, approvalApi } from '@/lib/api'
const mockedAgentApi = vi.mocked(agentApi)
const mockedTaskApi = vi.mocked(taskApi)
const mockedMetricsApi = vi.mocked(metricsApi)
const mockedApprovalApi = vi.mocked(approvalApi)

describe('useInitialData', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    useStore.setState({
      wsConnected: false, agents: new Map(), tasks: new Map(), approvals: [],
      metrics: { totalTasks: 0, completedTasks: 0, failedTasks: 0, runningTasks: 0, totalAgents: 0, activeAgents: 0, totalTokens: 0, totalCost: 0, avgLatencyMs: 0, successRate: 0 },
      selectedAgentId: null, sidebarCollapsed: false,
    })
    mockedAgentApi.listRaw.mockResolvedValue({ data: [{ id: 'a1', name: 'Agent 1', model: 'gpt-4', status: 'idle', currentLoad: 0, maxLoad: 10, successRate: 0.9, reputationScore: 80, totalTokens: 1000, totalCost: 0.5 }] } as any)
    mockedTaskApi.list.mockResolvedValue({ data: [{ id: 't1', dagId: 'd1', name: 'Task 1', status: 'running', tokensUsed: 100, costDollars: 0.01, createdAt: '2024-01-15T10:00:00Z' }], total: 1, page: 1, pageSize: 200 } as any)
    mockedMetricsApi.getSystem.mockResolvedValue({ data: { totalTasks: 50, completedTasks: 40, failedTasks: 2, runningTasks: 8, totalAgents: 5, activeAgents: 3, totalTokens: 100000, totalCost: 25.0, avgLatencyMs: 150, successRate: 0.95 } } as any)
    mockedApprovalApi.list.mockResolvedValue({ data: [{ id: 'ap1', taskId: 't1', agentId: 'a1', actionType: 'file_write', actionData: {}, riskScore: 0.5, status: 'pending', createdAt: '2024-01-15T10:00:00Z' }], total: 1, page: 1, pageSize: 100 } as any)
  })
  afterEach(() => { vi.restoreAllMocks() })

  it('fetches agents on mount', async () => {
    renderHook(() => useInitialData())
    await waitFor(() => { expect(mockedAgentApi.listRaw).toHaveBeenCalledTimes(1) })
    await waitFor(() => { expect(useStore.getState().agents.size).toBe(1) })
  })
  it('fetches tasks on mount', async () => {
    renderHook(() => useInitialData())
    await waitFor(() => { expect(mockedTaskApi.list).toHaveBeenCalledWith({ pageSize: 200 }) })
    await waitFor(() => { expect(useStore.getState().tasks.size).toBe(1) })
  })
  it('fetches metrics on mount', async () => {
    renderHook(() => useInitialData())
    await waitFor(() => { expect(mockedMetricsApi.getSystem).toHaveBeenCalled() })
    await waitFor(() => { expect(useStore.getState().metrics.totalTasks).toBe(50) })
  })
  it('fetches approvals on mount', async () => {
    renderHook(() => useInitialData())
    await waitFor(() => { expect(mockedApprovalApi.list).toHaveBeenCalledWith({ pageSize: 100 }) })
    await waitFor(() => { expect(useStore.getState().approvals).toHaveLength(1) })
  })
  it('handles fetch error gracefully', async () => {
    const spy = vi.spyOn(console, 'warn').mockImplementation(() => {})
    mockedAgentApi.listRaw.mockRejectedValue(new Error('Network error'))
    renderHook(() => useInitialData())
    await waitFor(() => { expect(spy).toHaveBeenCalledWith('[Init] Failed to fetch agents:', expect.any(Error)) })
    expect(useStore.getState().agents.size).toBe(0)
    spy.mockRestore()
  })
  it('sets up polling', async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true })
    renderHook(() => useInitialData())
    await waitFor(() => { expect(mockedMetricsApi.getSystem).toHaveBeenCalledTimes(1) })
    await vi.advanceTimersByTimeAsync(30000)
    expect(mockedMetricsApi.getSystem).toHaveBeenCalledTimes(2)
    vi.useRealTimers()
  })
})
