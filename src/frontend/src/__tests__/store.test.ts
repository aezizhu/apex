import { describe, it, expect, beforeEach } from 'vitest'
import { useStore, selectAgentList, selectTaskList, selectPendingApprovals, selectAgentsByStatus } from '../lib/store'
import type { Agent, Task, ApprovalRequest } from '../lib/store'

const mockAgent = (overrides: Partial<Agent> = {}): Agent => ({
  id: 'agent-1', name: 'Agent Alpha', model: 'gpt-4', status: 'idle',
  currentLoad: 0, maxLoad: 10, successRate: 0.95, reputationScore: 85,
  totalTokens: 5000, totalCost: 1.25, ...overrides,
})
const mockTask = (overrides: Partial<Task> = {}): Task => ({
  id: 'task-1', dagId: 'dag-1', name: 'Test Task', status: 'pending',
  tokensUsed: 100, costDollars: 0.01, createdAt: '2024-01-15T10:00:00Z', ...overrides,
})
const mockApproval = (overrides: Partial<ApprovalRequest> = {}): ApprovalRequest => ({
  id: 'approval-1', taskId: 'task-1', agentId: 'agent-1', actionType: 'file_write',
  actionData: { path: '/tmp/test.txt' }, riskScore: 0.7, status: 'pending',
  createdAt: '2024-01-15T10:00:00Z', ...overrides,
})

describe('Apex Store', () => {
  beforeEach(() => {
    useStore.setState({
      wsConnected: false, agents: new Map(), tasks: new Map(), approvals: [],
      metrics: { totalTasks: 0, completedTasks: 0, failedTasks: 0, runningTasks: 0, totalAgents: 0, activeAgents: 0, totalTokens: 0, totalCost: 0, avgLatencyMs: 0, successRate: 0 },
      selectedAgentId: null, sidebarCollapsed: false,
    })
  })

  describe('connection state', () => {
    it('starts disconnected', () => { expect(useStore.getState().wsConnected).toBe(false) })
    it('sets connected', () => { useStore.getState().setWsConnected(true); expect(useStore.getState().wsConnected).toBe(true) })
    it('sets disconnected', () => { useStore.getState().setWsConnected(true); useStore.getState().setWsConnected(false); expect(useStore.getState().wsConnected).toBe(false) })
  })

  describe('agents', () => {
    it('starts empty', () => { expect(useStore.getState().agents.size).toBe(0) })
    it('sets single agent', () => { const a = mockAgent(); useStore.getState().setAgent(a); expect(useStore.getState().agents.get('agent-1')).toEqual(a) })
    it('sets multiple agents', () => { useStore.getState().setAgents([mockAgent({ id: 'a1' }), mockAgent({ id: 'a2' })]); expect(useStore.getState().agents.size).toBe(2) })
    it('updates agent', () => { useStore.getState().setAgent(mockAgent()); useStore.getState().setAgent(mockAgent({ status: 'busy' })); expect(useStore.getState().agents.get('agent-1')?.status).toBe('busy') })
    it('removes agent', () => { useStore.getState().setAgents([mockAgent({ id: 'a1' }), mockAgent({ id: 'a2' })]); useStore.getState().removeAgent('a1'); expect(useStore.getState().agents.size).toBe(1) })
  })

  describe('tasks', () => {
    it('starts empty', () => { expect(useStore.getState().tasks.size).toBe(0) })
    it('sets single task', () => { useStore.getState().setTask(mockTask()); expect(useStore.getState().tasks.get('task-1')).toBeDefined() })
    it('sets multiple tasks', () => { useStore.getState().setTasks([mockTask({ id: 't1' }), mockTask({ id: 't2' })]); expect(useStore.getState().tasks.size).toBe(2) })
    it('updates task', () => { useStore.getState().setTask(mockTask()); useStore.getState().setTask(mockTask({ status: 'completed' })); expect(useStore.getState().tasks.get('task-1')?.status).toBe('completed') })
  })

  describe('approvals', () => {
    it('starts empty', () => { expect(useStore.getState().approvals).toEqual([]) })
    it('sets approvals', () => { useStore.getState().setApprovals([mockApproval(), mockApproval({ id: 'a2' })]); expect(useStore.getState().approvals).toHaveLength(2) })
    it('adds approval (prepends)', () => { useStore.getState().setApprovals([mockApproval({ id: 'old' })]); useStore.getState().addApproval(mockApproval({ id: 'new' })); expect(useStore.getState().approvals[0].id).toBe('new') })
    it('updates approval status', () => { useStore.getState().setApprovals([mockApproval()]); useStore.getState().updateApproval('approval-1', 'approved'); expect(useStore.getState().approvals[0].status).toBe('approved') })
  })

  describe('metrics', () => {
    it('starts zeroed', () => { expect(useStore.getState().metrics.totalTasks).toBe(0) })
    it('partially updates', () => { useStore.getState().setMetrics({ totalTasks: 100 }); expect(useStore.getState().metrics.totalTasks).toBe(100); expect(useStore.getState().metrics.failedTasks).toBe(0) })
  })

  describe('UI state', () => {
    it('selects agent', () => { useStore.getState().setSelectedAgentId('a1'); expect(useStore.getState().selectedAgentId).toBe('a1') })
    it('deselects agent', () => { useStore.getState().setSelectedAgentId('a1'); useStore.getState().setSelectedAgentId(null); expect(useStore.getState().selectedAgentId).toBeNull() })
    it('toggles sidebar', () => { useStore.getState().setSidebarCollapsed(true); expect(useStore.getState().sidebarCollapsed).toBe(true) })
  })

  describe('selectors', () => {
    it('selectAgentList', () => { useStore.getState().setAgents([mockAgent({ id: 'a1' }), mockAgent({ id: 'a2' })]); expect(selectAgentList(useStore.getState())).toHaveLength(2) })
    it('selectTaskList', () => { useStore.getState().setTasks([mockTask({ id: 't1' }), mockTask({ id: 't2' })]); expect(selectTaskList(useStore.getState())).toHaveLength(2) })
    it('selectPendingApprovals', () => { useStore.getState().setApprovals([mockApproval({ id: 'a1', status: 'pending' }), mockApproval({ id: 'a2', status: 'approved' })]); expect(selectPendingApprovals(useStore.getState())).toHaveLength(1) })
    it('selectAgentsByStatus', () => { useStore.getState().setAgents([mockAgent({ id: 'a1', status: 'idle' }), mockAgent({ id: 'a2', status: 'busy' }), mockAgent({ id: 'a3', status: 'busy' })]); expect(selectAgentsByStatus('busy')(useStore.getState())).toHaveLength(2) })
  })
})
