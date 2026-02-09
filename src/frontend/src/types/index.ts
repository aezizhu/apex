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
// Delegation Types
// ═══════════════════════════════════════════════════════════════════════════════

export type DelegationStrategy =
  | 'direct'
  | 'broadcast'
  | 'round_robin'
  | 'least_busy'
  | 'capability'

export type DelegationPriority = 'low' | 'normal' | 'high' | 'critical'

export type DelegationStatusType =
  | 'accepted'
  | 'no_agent_available'
  | 'target_not_found'
  | 'target_unavailable'
  | 'rejected'

export interface DelegateRequest {
  task: string
  toAgent?: string
  strategy?: DelegationStrategy
  priority?: DelegationPriority
  requiredCapabilities?: string[]
}

export interface DelegationResponse {
  delegationId: string
  status: DelegationStatusType
  assignedAgent?: string
  candidatesEvaluated: number
  reason: string
  resolvedAt: string
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
  instruction: string
  priority?: number
  context?: Record<string, unknown>
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

export type WsServerMessageType =
  | 'connected'
  | 'session_restored'
  | 'subscribed'
  | 'heartbeat'
  | 'agent_update'
  | 'task_update'
  | 'metrics'
  | 'approval_required'
  | 'pong'
  | 'error'

export type WsClientMessageType =
  | 'subscribe'
  | 'unsubscribe'
  | 'ping'
  | 'session_restore'

export interface WsMessage {
  type: string
  [key: string]: unknown
}

export interface WsSubscribeMessage {
  type: 'subscribe' | 'unsubscribe'
  target: {
    resource: 'all_agents' | 'all_tasks' | 'metrics' | 'approvals'
    interval_secs?: number
  }
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

export interface ApiKeySummary {
  id: string
  provider: string
  maskedKey: string
  createdAt: string
}

export interface CreateApiKeyRequest {
  provider: string
  key: string
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
