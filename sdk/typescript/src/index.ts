/**
 * Project Apex TypeScript SDK
 *
 * A comprehensive TypeScript SDK for interacting with the Project Apex API.
 * Supports REST endpoints, WebSocket streaming, automatic retries, and full type safety.
 *
 * @packageDocumentation
 */

// ============================================================================
// Client Exports
// ============================================================================

export { ApexClient, createApexClient } from './client';
export { ApexWebSocket, createApexWebSocket } from './websocket';

// ============================================================================
// Type Exports
// ============================================================================

export type { ApexWebSocketConfig, WebSocketEventMap } from './websocket';

export {
  // Enums
  TaskStatus,
  TaskPriority,
  AgentStatus,
  ApprovalStatus,
  DAGStatus,
  WebSocketEventType,
} from './types';

export type {
  // Base types
  Timestamps,
  PaginationParams,
  PaginatedResponse,

  // Task types
  Task,
  TaskError,
  CreateTaskRequest,
  UpdateTaskRequest,
  TaskFilter,
  TaskLog,

  // Agent types
  Agent,
  AgentStats,
  CreateAgentRequest,
  UpdateAgentRequest,
  AgentFilter,

  // DAG types
  DAG,
  DAGNode,
  DAGNodeConfig,
  DAGCondition,
  ApprovalConfig,
  DAGEdge,
  CreateDAGRequest,
  UpdateDAGRequest,
  DAGFilter,
  DAGExecution,
  DAGNodeExecution,

  // Approval types
  Approval,
  ApprovalDecision,
  CreateApprovalRequest,
  ApprovalResponse,
  ApprovalFilter,

  // WebSocket types
  WebSocketMessage,
  WebSocketSubscription,
  TaskCreatedPayload,
  TaskUpdatedPayload,
  TaskCompletedPayload,
  TaskFailedPayload,
  AgentStatusChangedPayload,
  DAGStartedPayload,
  DAGCompletedPayload,
  DAGFailedPayload,
  ApprovalRequiredPayload,
  ApprovalResolvedPayload,
  LogMessagePayload,
  HeartbeatPayload,

  // Configuration types
  ApexClientConfig,
  RequestConfig,

  // API response types
  ApiResponse,
  ApiError,
  HealthCheckResponse,
} from './types';

// ============================================================================
// Error Exports
// ============================================================================

export {
  ApexError,
  ApiRequestError,
  AuthenticationError,
  AuthorizationError,
  NotFoundError,
  ValidationError,
  RateLimitError,
  TimeoutError,
  WebSocketError,
  TaskExecutionError,
  MaxRetriesExceededError,
  DAGExecutionError,
  parseApiError,
} from './errors';

export type { ValidationFieldError } from './errors';

// ============================================================================
// Default Export
// ============================================================================

import { ApexClient, createApexClient } from './client';
export default createApexClient;
