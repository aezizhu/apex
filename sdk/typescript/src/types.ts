/**
 * Project Apex TypeScript SDK Types
 */

// ============================================================================
// Enums
// ============================================================================

export enum TaskStatus {
  PENDING = 'pending',
  QUEUED = 'queued',
  RUNNING = 'running',
  PAUSED = 'paused',
  COMPLETED = 'completed',
  FAILED = 'failed',
  CANCELLED = 'cancelled',
  WAITING_APPROVAL = 'waiting_approval',
}

export enum TaskPriority {
  LOW = 'low',
  NORMAL = 'normal',
  HIGH = 'high',
  CRITICAL = 'critical',
}

export enum AgentStatus {
  IDLE = 'idle',
  BUSY = 'busy',
  OFFLINE = 'offline',
  ERROR = 'error',
}

export enum ApprovalStatus {
  PENDING = 'pending',
  APPROVED = 'approved',
  REJECTED = 'rejected',
  EXPIRED = 'expired',
}

export enum DAGStatus {
  DRAFT = 'draft',
  ACTIVE = 'active',
  RUNNING = 'running',
  COMPLETED = 'completed',
  FAILED = 'failed',
  PAUSED = 'paused',
}

export enum WebSocketEventType {
  TASK_CREATED = 'task.created',
  TASK_UPDATED = 'task.updated',
  TASK_COMPLETED = 'task.completed',
  TASK_FAILED = 'task.failed',
  AGENT_STATUS_CHANGED = 'agent.status_changed',
  DAG_STARTED = 'dag.started',
  DAG_COMPLETED = 'dag.completed',
  DAG_FAILED = 'dag.failed',
  APPROVAL_REQUIRED = 'approval.required',
  APPROVAL_RESOLVED = 'approval.resolved',
  LOG_MESSAGE = 'log.message',
  HEARTBEAT = 'heartbeat',
}

// ============================================================================
// Base Types
// ============================================================================

export interface Timestamps {
  createdAt: string;
  updatedAt: string;
}

export interface PaginationParams {
  page?: number;
  limit?: number;
  sortBy?: string;
  sortOrder?: 'asc' | 'desc';
}

export interface PaginatedResponse<T> {
  data: T[];
  pagination: {
    page: number;
    limit: number;
    total: number;
    totalPages: number;
    hasMore: boolean;
  };
}

// ============================================================================
// Task Types
// ============================================================================

export interface Task extends Timestamps {
  id: string;
  name: string;
  description?: string;
  status: TaskStatus;
  priority: TaskPriority;
  agentId?: string;
  dagId?: string;
  dagNodeId?: string;
  input?: Record<string, unknown>;
  output?: Record<string, unknown>;
  error?: TaskError;
  metadata?: Record<string, unknown>;
  startedAt?: string;
  completedAt?: string;
  timeoutSeconds?: number;
  retryCount: number;
  maxRetries: number;
  parentTaskId?: string;
  childTaskIds?: string[];
}

export interface TaskError {
  code: string;
  message: string;
  details?: Record<string, unknown>;
  stack?: string;
}

export interface CreateTaskRequest {
  name: string;
  description?: string;
  priority?: TaskPriority;
  agentId?: string;
  input?: Record<string, unknown>;
  metadata?: Record<string, unknown>;
  timeoutSeconds?: number;
  maxRetries?: number;
  parentTaskId?: string;
}

export interface UpdateTaskRequest {
  name?: string;
  description?: string;
  priority?: TaskPriority;
  status?: TaskStatus;
  input?: Record<string, unknown>;
  metadata?: Record<string, unknown>;
}

export interface TaskFilter extends PaginationParams {
  status?: TaskStatus | TaskStatus[];
  priority?: TaskPriority | TaskPriority[];
  agentId?: string;
  dagId?: string;
  parentTaskId?: string;
  createdAfter?: string;
  createdBefore?: string;
}

export interface TaskLog {
  id: string;
  taskId: string;
  level: 'debug' | 'info' | 'warn' | 'error';
  message: string;
  timestamp: string;
  metadata?: Record<string, unknown>;
}

// ============================================================================
// Agent Types
// ============================================================================

export interface Agent extends Timestamps {
  id: string;
  name: string;
  description?: string;
  status: AgentStatus;
  capabilities: string[];
  currentTaskId?: string;
  lastHeartbeat?: string;
  metadata?: Record<string, unknown>;
  stats: AgentStats;
}

export interface AgentStats {
  totalTasksCompleted: number;
  totalTasksFailed: number;
  averageTaskDuration: number;
  uptime: number;
}

export interface CreateAgentRequest {
  name: string;
  description?: string;
  capabilities?: string[];
  metadata?: Record<string, unknown>;
}

export interface UpdateAgentRequest {
  name?: string;
  description?: string;
  capabilities?: string[];
  status?: AgentStatus;
  metadata?: Record<string, unknown>;
}

export interface AgentFilter extends PaginationParams {
  status?: AgentStatus | AgentStatus[];
  capabilities?: string[];
  hasCurrentTask?: boolean;
}

// ============================================================================
// DAG Types
// ============================================================================

export interface DAG extends Timestamps {
  id: string;
  name: string;
  description?: string;
  status: DAGStatus;
  nodes: DAGNode[];
  edges: DAGEdge[];
  metadata?: Record<string, unknown>;
  startedAt?: string;
  completedAt?: string;
  currentNodeIds?: string[];
}

export interface DAGNode {
  id: string;
  name: string;
  type: 'task' | 'condition' | 'parallel' | 'approval';
  config: DAGNodeConfig;
  status?: TaskStatus;
  taskId?: string;
}

export interface DAGNodeConfig {
  taskTemplate?: CreateTaskRequest;
  condition?: DAGCondition;
  parallelNodes?: string[];
  approvalConfig?: ApprovalConfig;
  timeout?: number;
  retries?: number;
}

export interface DAGCondition {
  expression: string;
  trueBranch: string;
  falseBranch: string;
}

export interface ApprovalConfig {
  approvers?: string[];
  requiredApprovals?: number;
  timeoutSeconds?: number;
  autoApprove?: boolean;
}

export interface DAGEdge {
  id: string;
  sourceNodeId: string;
  targetNodeId: string;
  condition?: string;
}

export interface CreateDAGRequest {
  name: string;
  description?: string;
  nodes: Omit<DAGNode, 'status' | 'taskId'>[];
  edges: Omit<DAGEdge, 'id'>[];
  metadata?: Record<string, unknown>;
}

export interface UpdateDAGRequest {
  name?: string;
  description?: string;
  nodes?: Omit<DAGNode, 'status' | 'taskId'>[];
  edges?: Omit<DAGEdge, 'id'>[];
  metadata?: Record<string, unknown>;
}

export interface DAGFilter extends PaginationParams {
  status?: DAGStatus | DAGStatus[];
  createdAfter?: string;
  createdBefore?: string;
}

export interface DAGExecution extends Timestamps {
  id: string;
  dagId: string;
  status: DAGStatus;
  input?: Record<string, unknown>;
  output?: Record<string, unknown>;
  nodeExecutions: DAGNodeExecution[];
  startedAt?: string;
  completedAt?: string;
  error?: TaskError;
}

export interface DAGNodeExecution {
  nodeId: string;
  taskId?: string;
  status: TaskStatus;
  startedAt?: string;
  completedAt?: string;
  input?: Record<string, unknown>;
  output?: Record<string, unknown>;
  error?: TaskError;
}

// ============================================================================
// Approval Types
// ============================================================================

export interface Approval extends Timestamps {
  id: string;
  taskId?: string;
  dagId?: string;
  dagNodeId?: string;
  status: ApprovalStatus;
  requestedBy: string;
  approvers: string[];
  requiredApprovals: number;
  currentApprovals: ApprovalDecision[];
  reason?: string;
  expiresAt?: string;
  metadata?: Record<string, unknown>;
}

export interface ApprovalDecision {
  approverId: string;
  decision: 'approved' | 'rejected';
  comment?: string;
  timestamp: string;
}

export interface CreateApprovalRequest {
  taskId?: string;
  dagId?: string;
  dagNodeId?: string;
  approvers: string[];
  requiredApprovals?: number;
  reason?: string;
  expiresAt?: string;
  metadata?: Record<string, unknown>;
}

export interface ApprovalResponse {
  decision: 'approved' | 'rejected';
  comment?: string;
}

export interface ApprovalFilter extends PaginationParams {
  status?: ApprovalStatus | ApprovalStatus[];
  taskId?: string;
  dagId?: string;
  approverId?: string;
  createdAfter?: string;
  createdBefore?: string;
}

// ============================================================================
// WebSocket Types
// ============================================================================

export interface WebSocketMessage<T = unknown> {
  type: WebSocketEventType;
  payload: T;
  timestamp: string;
  correlationId?: string;
}

export interface WebSocketSubscription {
  event: WebSocketEventType | WebSocketEventType[];
  filter?: {
    taskId?: string;
    agentId?: string;
    dagId?: string;
  };
}

export interface TaskCreatedPayload {
  task: Task;
}

export interface TaskUpdatedPayload {
  task: Task;
  changes: Partial<Task>;
}

export interface TaskCompletedPayload {
  task: Task;
  duration: number;
}

export interface TaskFailedPayload {
  task: Task;
  error: TaskError;
}

export interface AgentStatusChangedPayload {
  agent: Agent;
  previousStatus: AgentStatus;
}

export interface DAGStartedPayload {
  dag: DAG;
  execution: DAGExecution;
}

export interface DAGCompletedPayload {
  dag: DAG;
  execution: DAGExecution;
  duration: number;
}

export interface DAGFailedPayload {
  dag: DAG;
  execution: DAGExecution;
  error: TaskError;
}

export interface ApprovalRequiredPayload {
  approval: Approval;
}

export interface ApprovalResolvedPayload {
  approval: Approval;
  decision: 'approved' | 'rejected';
}

export interface LogMessagePayload {
  log: TaskLog;
}

export interface HeartbeatPayload {
  serverTime: string;
  connectionId: string;
}

// ============================================================================
// Client Configuration Types
// ============================================================================

export interface ApexClientConfig {
  baseUrl: string;
  apiKey?: string;
  timeout?: number;
  retries?: number;
  retryDelay?: number;
  maxRetryDelay?: number;
  headers?: Record<string, string>;
  websocketUrl?: string;
}

export interface RequestConfig {
  timeout?: number;
  headers?: Record<string, string>;
  signal?: AbortSignal;
}

// ============================================================================
// API Response Types
// ============================================================================

export interface ApiResponse<T> {
  success: boolean;
  data: T;
  message?: string;
}

export interface ApiError {
  success: false;
  error: {
    code: string;
    message: string;
    details?: Record<string, unknown>;
  };
}

export interface HealthCheckResponse {
  status: 'healthy' | 'degraded' | 'unhealthy';
  version: string;
  uptime: number;
  services: {
    database: 'up' | 'down';
    queue: 'up' | 'down';
    websocket: 'up' | 'down';
  };
}
