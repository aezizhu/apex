// ═══════════════════════════════════════════════════════════════════════════════
// Agent Types
// ═══════════════════════════════════════════════════════════════════════════════

export type AgentStatus = 'idle' | 'busy' | 'error' | 'paused'

export interface Agent {
  id: string
  name: string
  model: string
  status: AgentStatus
  currentLoad: number
  maxLoad: number
  successRate: number
  reputationScore: number
  totalTokens: number
  totalCost: number
  confidence?: number
  createdAt?: string
  updatedAt?: string
}

export interface AgentConfig {
  name: string
  model: string
  maxLoad: number
  systemPrompt?: string
  temperature?: number
  maxTokens?: number
}

export interface AgentCreateRequest {
  config: AgentConfig
}

export interface AgentUpdateRequest {
  status?: AgentStatus
  maxLoad?: number
  systemPrompt?: string
}

// ═══════════════════════════════════════════════════════════════════════════════
// Task Types
// ═══════════════════════════════════════════════════════════════════════════════

export type TaskStatus =
  | 'pending'
  | 'ready'
  | 'running'
  | 'completed'
  | 'failed'
  | 'cancelled'

export interface Task {
  id: string
  dagId: string
  name: string
  status: TaskStatus
  agentId?: string
  tokensUsed: number
  costDollars: number
  createdAt: string
  startedAt?: string
  completedAt?: string
  errorMessage?: string
  result?: unknown
}

export interface TaskCreateRequest {
  name: string
  dagId?: string
  prompt: string
  priority?: number
  dependencies?: string[]
}

export interface TaskCancelRequest {
  reason?: string
}

// ═══════════════════════════════════════════════════════════════════════════════
// DAG Types
// ═══════════════════════════════════════════════════════════════════════════════

export interface DAGNode {
  id: string
  taskId: string
  dependencies: string[]
}

export interface DAG {
  id: string
  name: string
  status: 'pending' | 'running' | 'completed' | 'failed'
  nodes: DAGNode[]
  createdAt: string
  completedAt?: string
}

export interface DAGCreateRequest {
  name: string
  tasks: Array<{
    name: string
    prompt: string
    dependencies?: string[]
  }>
}

// ═══════════════════════════════════════════════════════════════════════════════
// Approval Types
// ═══════════════════════════════════════════════════════════════════════════════

export type ApprovalStatus = 'pending' | 'approved' | 'denied' | 'expired'

export interface ApprovalRequest {
  id: string
  taskId: string
  agentId: string
  actionType: string
  actionData: Record<string, unknown>
  riskScore: number
  status: ApprovalStatus
  createdAt: string
  expiresAt?: string
  decidedAt?: string
  decidedBy?: string
}

export interface ApprovalDecision {
  status: 'approved' | 'denied'
  reason?: string
}

// ═══════════════════════════════════════════════════════════════════════════════
// Metrics Types
// ═══════════════════════════════════════════════════════════════════════════════

export interface SystemMetrics {
  totalTasks: number
  completedTasks: number
  failedTasks: number
  runningTasks: number
  totalAgents: number
  activeAgents: number
  totalTokens: number
  totalCost: number
  avgLatencyMs: number
  successRate: number
}

export interface AgentMetrics {
  agentId: string
  tasksCompleted: number
  tasksFailed: number
  avgLatencyMs: number
  totalTokens: number
  totalCost: number
  successRate: number
}

export interface TimeSeriesDataPoint {
  timestamp: string
  value: number
}

export interface MetricsTimeSeries {
  metric: string
  dataPoints: TimeSeriesDataPoint[]
}

// ═══════════════════════════════════════════════════════════════════════════════
// API Response Types
// ═══════════════════════════════════════════════════════════════════════════════

export interface ApiResponse<T> {
  data: T
  message?: string
}

export interface ApiListResponse<T> {
  data: T[]
  total: number
  page: number
  pageSize: number
}

export interface ApiError {
  code: string
  message: string
  details?: Record<string, unknown>
}

// ═══════════════════════════════════════════════════════════════════════════════
// WebSocket Types
// ═══════════════════════════════════════════════════════════════════════════════

export type WsMessageType =
  | 'Subscribe'
  | 'Unsubscribe'
  | 'Ping'
  | 'AgentUpdate'
  | 'TaskUpdate'
  | 'MetricsUpdate'
  | 'ApprovalRequest'
  | 'Error'

export interface WsMessage {
  type: WsMessageType
  [key: string]: unknown
}

export interface WsSubscribeMessage {
  type: 'Subscribe' | 'Unsubscribe'
  resource: 'agents' | 'tasks' | 'metrics' | 'approvals'
}

// ═══════════════════════════════════════════════════════════════════════════════
// Settings Types
// ═══════════════════════════════════════════════════════════════════════════════

export interface SystemSettings {
  maxConcurrentTasks: number
  defaultAgentModel: string
  approvalThreshold: number
  autoRetryEnabled: boolean
  maxRetries: number
  logLevel: 'debug' | 'info' | 'warn' | 'error'
}

// ═══════════════════════════════════════════════════════════════════════════════
// Pagination Types
// ═══════════════════════════════════════════════════════════════════════════════

export interface PaginationParams {
  page?: number
  pageSize?: number
  sortBy?: string
  sortOrder?: 'asc' | 'desc'
}

export interface FilterParams {
  status?: string
  agentId?: string
  from?: string
  to?: string
  search?: string
}
