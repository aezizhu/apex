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

// ═══════════════════════════════════════════════════════════════════════════════
// WebSocket Message Validation Schemas (Zod)
// ═══════════════════════════════════════════════════════════════════════════════

import { z } from 'zod'

// Base schema for all WebSocket messages
const wsMessageBaseSchema = z.object({
  type: z.string(),
})

// Connected message - server confirms connection
export const wsConnectedSchema = wsMessageBaseSchema.extend({
  type: z.literal('connected'),
  session_id: z.string().optional(),
})

// Session restored - server confirms session recovery
export const wsSessionRestoredSchema = wsMessageBaseSchema.extend({
  type: z.literal('session_restored'),
})

// Subscribed - server confirms subscription
export const wsSubscribedSchema = wsMessageBaseSchema.extend({
  type: z.literal('subscribed'),
  target: z.object({
    resource: z.string(),
  }),
})

// Heartbeat - keep-alive from server
export const wsHeartbeatSchema = wsMessageBaseSchema.extend({
  type: z.literal('heartbeat'),
})

// Agent update - real-time agent status change
export const wsAgentUpdateSchema = wsMessageBaseSchema.extend({
  type: z.literal('agent_update'),
  agent_id: z.string(),
  name: z.string().optional(),
  model: z.string().optional(),
  status: z.string().optional(),
  current_load: z.number().optional(),
  max_load: z.number().optional(),
  success_rate: z.number().optional(),
  reputation_score: z.number().optional(),
  total_tokens: z.number().optional(),
  total_cost: z.number().optional(),
  confidence: z.number().optional(),
})

// Task update - real-time task status change
export const wsTaskUpdateSchema = wsMessageBaseSchema.extend({
  type: z.literal('task_update'),
  task_id: z.string(),
  dag_id: z.string().optional(),
  name: z.string().optional(),
  status: z.string().optional(),
  agent_id: z.string().optional(),
  tokens_used: z.number().optional(),
  cost_dollars: z.number().optional(),
  created_at: z.string().optional(),
  timestamp: z.string().optional(),
  started_at: z.string().optional(),
  completed_at: z.string().optional(),
})

// Metrics - periodic system metrics snapshot
export const wsMetricsSchema = wsMessageBaseSchema.extend({
  type: z.literal('metrics'),
  agents: z
    .object({
      total: z.number().optional(),
      active: z.number().optional(),
      avg_success_rate: z.number().optional(),
    })
    .optional(),
  tasks: z
    .object({
      running: z.number().optional(),
      completed_last_hour: z.number().optional(),
      failed_last_hour: z.number().optional(),
      avg_duration_ms: z.number().optional(),
    })
    .optional(),
  resources: z
    .object({
      total_tokens_used: z.number().optional(),
      total_cost_dollars: z.number().optional(),
    })
    .optional(),
})

// Approval required - server requests human approval
export const wsApprovalRequiredSchema = wsMessageBaseSchema.extend({
  type: z.literal('approval_required'),
  request_id: z.string(),
  task_id: z.string(),
  agent_id: z.string(),
  approval_type: z.string(),
  details: z.record(z.unknown()).optional(),
  created_at: z.string().optional(),
})

// Pong - response to ping
export const wsPongSchema = wsMessageBaseSchema.extend({
  type: z.literal('pong'),
})

// Error - server-side error
export const wsErrorSchema = wsMessageBaseSchema.extend({
  type: z.literal('error'),
  message: z.string().optional(),
  code: z.string().optional(),
})

// Discriminated union for all server messages
export const wsServerMessageSchema = z.discriminatedUnion('type', [
  wsConnectedSchema,
  wsSessionRestoredSchema,
  wsSubscribedSchema,
  wsHeartbeatSchema,
  wsAgentUpdateSchema,
  wsTaskUpdateSchema,
  wsMetricsSchema,
  wsApprovalRequiredSchema,
  wsPongSchema,
  wsErrorSchema,
])

// Type inference from schemas
export type WsConnected = z.infer<typeof wsConnectedSchema>
export type WsSessionRestored = z.infer<typeof wsSessionRestoredSchema>
export type WsSubscribed = z.infer<typeof wsSubscribedSchema>
export type WsHeartbeat = z.infer<typeof wsHeartbeatSchema>
export type WsAgentUpdate = z.infer<typeof wsAgentUpdateSchema>
export type WsTaskUpdate = z.infer<typeof wsTaskUpdateSchema>
export type WsMetrics = z.infer<typeof wsMetricsSchema>
export type WsApprovalRequired = z.infer<typeof wsApprovalRequiredSchema>
export type WsPong = z.infer<typeof wsPongSchema>
export type WsError = z.infer<typeof wsErrorSchema>

// Union type for all server messages
export type WsServerMessage =
  | WsConnected
  | WsSessionRestored
  | WsSubscribed
  | WsHeartbeat
  | WsAgentUpdate
  | WsTaskUpdate
  | WsMetrics
  | WsApprovalRequired
  | WsPong
  | WsError

// Helper function to validate and parse incoming WebSocket messages
export function parseWsMessage(data: unknown): WsServerMessage | null {
  const result = wsServerMessageSchema.safeParse(data)
  if (result.success) {
    return result.data
  }
  console.warn('[WS] Invalid message format:', result.error.flatten())
  return null
}
