import { create } from 'zustand'
import type { Agent, AgentStatus, Task, TaskStatus, ApprovalRequest, ApprovalStatus, SystemMetrics } from '../types'

// Settings type for the Settings page
export interface SettingsState {
  maxConcurrentAgents: number
  defaultModel: string
  autoRetryEnabled: boolean
  maxRetries: number
  logLevel: 'debug' | 'info' | 'warn' | 'error'
  costThreshold: number
  enableModelRouting: boolean
  defaultTokenLimit: number
  defaultCostLimit: number
  defaultTimeLimit: number
  circuitBreakerThreshold: number
  notifyOnFailure: boolean
  notifyOnCostThreshold: number
  theme: string
  compactMode: boolean
}

interface StoreState {
  wsConnected: boolean
  setWsConnected: (connected: boolean) => void

  agents: Map<string, Agent>
  setAgent: (agent: Agent) => void
  setAgents: (agents: Agent[]) => void
  removeAgent: (agentId: string) => void

  tasks: Map<string, Task>
  setTask: (task: Task) => void
  setTasks: (tasks: Task[]) => void

  approvals: ApprovalRequest[]
  setApprovals: (approvals: ApprovalRequest[]) => void
  addApproval: (approval: ApprovalRequest) => void
  updateApproval: (id: string, status: ApprovalStatus, decidedBy?: string) => void

  metrics: SystemMetrics
  setMetrics: (metrics: Partial<SystemMetrics>) => void

  selectedAgentId: string | null
  setSelectedAgentId: (id: string | null) => void

  sidebarCollapsed: boolean
  setSidebarCollapsed: (collapsed: boolean) => void

  // Auth state
  isAuthenticated: boolean
  setIsAuthenticated: (authenticated: boolean) => void

  // Settings state
  settings: SettingsState
  setSettings: (settings: Partial<SettingsState>) => void
  apiKeys: unknown[]
  setApiKeys: (keys: unknown[]) => void
  addApiKey: (key: unknown) => void
  removeApiKey: (keyId: string) => void
}

// Default settings values
const defaultSettings: SettingsState = {
  maxConcurrentAgents: 10,
  defaultModel: 'gpt-4',
  autoRetryEnabled: true,
  maxRetries: 3,
  logLevel: 'info',
  costThreshold: 10,
  enableModelRouting: false,
  defaultTokenLimit: 100000,
  defaultCostLimit: 5,
  defaultTimeLimit: 300,
  circuitBreakerThreshold: 5,
  notifyOnFailure: true,
  notifyOnCostThreshold: 100,
  theme: 'dark',
  compactMode: false,
}

export const useStore = create<StoreState>((set) => ({
  wsConnected: false,
  setWsConnected: (connected) => set({ wsConnected: connected }),

  agents: new Map(),
  setAgent: (agent) =>
    set((state) => {
      const agents = new Map(state.agents)
      agents.set(agent.id, agent)
      return { agents }
    }),
  setAgents: (agents) =>
    set(() => {
      const map = new Map<string, Agent>()
      agents.forEach((a) => map.set(a.id, a))
      return { agents: map }
    }),
  removeAgent: (agentId) =>
    set((state) => {
      const agents = new Map(state.agents)
      agents.delete(agentId)
      return { agents }
    }),

  tasks: new Map(),
  setTask: (task) =>
    set((state) => {
      const tasks = new Map(state.tasks)
      tasks.set(task.id, task)
      return { tasks }
    }),
  setTasks: (tasks) =>
    set(() => {
      const map = new Map<string, Task>()
      tasks.forEach((t) => map.set(t.id, t))
      return { tasks: map }
    }),

  approvals: [],
  setApprovals: (approvals) => set({ approvals }),
  addApproval: (approval) =>
    set((state) => ({
      approvals: [approval, ...state.approvals],
    })),
  updateApproval: (id, status, decidedBy) =>
    set((state) => ({
      approvals: state.approvals.map((a) =>
        a.id === id ? { ...a, status, decidedBy, decidedAt: new Date().toISOString() } : a
      ),
    })),

  metrics: {
    totalTasks: 0,
    completedTasks: 0,
    failedTasks: 0,
    runningTasks: 0,
    totalAgents: 0,
    activeAgents: 0,
    totalTokens: 0,
    totalCost: 0,
    avgLatencyMs: 0,
    successRate: 0,
  },
  setMetrics: (metrics) =>
    set((state) => ({
      metrics: { ...state.metrics, ...metrics },
    })),

  selectedAgentId: null,
  setSelectedAgentId: (id) => set({ selectedAgentId: id }),

  sidebarCollapsed: false,
  setSidebarCollapsed: (collapsed) => set({ sidebarCollapsed: collapsed }),

  isAuthenticated: false,
  setIsAuthenticated: (authenticated) => set({ isAuthenticated: authenticated }),

  settings: defaultSettings,
  setSettings: (settings) =>
    set((state) => ({
      settings: { ...state.settings, ...settings },
    })),

  apiKeys: [] as unknown[],
  setApiKeys: (keys) => set({ apiKeys: keys }),
  addApiKey: (key) => set((state) => ({ apiKeys: [...state.apiKeys, key] })),
  removeApiKey: (keyId) => set((state) => ({
    apiKeys: state.apiKeys.filter((k) => (k as { id?: string }).id !== keyId)
  })),
}))

// Selectors
export const selectAgentList = (state: StoreState): Agent[] => Array.from(state.agents.values())
export const selectTaskList = (state: StoreState): Task[] => Array.from(state.tasks.values())
export const selectPendingApprovals = (state: StoreState): ApprovalRequest[] =>
  state.approvals.filter((a) => a.status === 'pending')
export const selectAgentsByStatus = (status: AgentStatus) => (state: StoreState): Agent[] =>
  selectAgentList(state).filter((a) => a.status === status)
export const selectActiveAgents = (state: StoreState): Agent[] =>
  selectAgentList(state).filter((a) => a.status === 'busy')
export const selectRunningTasks = (state: StoreState): Task[] =>
  selectTaskList(state).filter((t) => t.status === 'running')
export const selectTasksByStatus = (status: TaskStatus) => (state: StoreState): Task[] =>
  selectTaskList(state).filter((t) => t.status === status)
export const selectTasksByAgent = (agentId: string) => (state: StoreState): Task[] =>
  selectTaskList(state).filter((t) => t.agentId === agentId)

// Re-export types from types/index for convenience
export type { Agent, Task, ApprovalRequest, SystemMetrics }
