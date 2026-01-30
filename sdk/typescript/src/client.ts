/**
 * Project Apex TypeScript SDK - API Client
 *
 * Provides the {@link ApexClient} class for interacting with the Apex Agent
 * Swarm Orchestration API.  All resource operations (tasks, agents, DAGs,
 * approvals) are exposed as typed async methods with automatic retry and
 * exponential back-off for transient errors.
 *
 * @example
 * ```typescript
 * import { ApexClient } from '@apex-swarm/sdk';
 *
 * const client = new ApexClient({
 *   baseUrl: 'http://localhost:8080',
 *   apiKey: process.env.APEX_API_KEY!,
 * });
 *
 * const health = await client.healthCheck();
 * console.log(health.status);
 * ```
 *
 * @module client
 */

import axios, {
  AxiosInstance,
  AxiosRequestConfig,
  AxiosResponse,
  AxiosError,
  InternalAxiosRequestConfig,
} from 'axios';
import {
  ApexClientConfig,
  RequestConfig,
  PaginatedResponse,
  ApiResponse,
  HealthCheckResponse,
  // Task types
  Task,
  TaskFilter,
  CreateTaskRequest,
  UpdateTaskRequest,
  TaskLog,
  // Agent types
  Agent,
  AgentFilter,
  CreateAgentRequest,
  UpdateAgentRequest,
  // DAG types
  DAG,
  DAGFilter,
  CreateDAGRequest,
  UpdateDAGRequest,
  DAGExecution,
  // Approval types
  Approval,
  ApprovalFilter,
  CreateApprovalRequest,
  ApprovalResponse,
} from './types';
import {
  ApexError,
  parseApiError,
  MaxRetriesExceededError,
  TimeoutError,
} from './errors';
import { ApexWebSocket, ApexWebSocketConfig } from './websocket';

// ============================================================================
// Utility Functions
// ============================================================================

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function buildQueryString(params: Record<string, unknown>): string {
  const searchParams = new URLSearchParams();

  for (const [key, value] of Object.entries(params)) {
    if (value === undefined || value === null) {
      continue;
    }

    if (Array.isArray(value)) {
      for (const item of value) {
        searchParams.append(key, String(item));
      }
    } else {
      searchParams.append(key, String(value));
    }
  }

  const queryString = searchParams.toString();
  return queryString ? `?${queryString}` : '';
}

// ============================================================================
// ApexClient Class
// ============================================================================

/**
 * Full-featured client for the Apex Agent Swarm Orchestration API.
 *
 * Encapsulates HTTP communication (via axios), automatic retry with
 * exponential back-off, and optional WebSocket access for real-time events.
 *
 * @example
 * ```typescript
 * const client = new ApexClient({
 *   baseUrl: 'http://localhost:8080',
 *   apiKey: 'my-api-key',
 *   timeout: 15000,
 *   retries: 5,
 * });
 *
 * // Create and wait for a task
 * const task = await client.runTask({ name: 'Summarise document', input: { url } });
 * console.log(task.output);
 * ```
 */
export class ApexClient {
  private readonly config: Required<ApexClientConfig>;
  private readonly httpClient: AxiosInstance;
  private websocket: ApexWebSocket | null = null;

  /**
   * Create a new ApexClient.
   *
   * @param config - Client configuration including base URL, API key,
   *   timeout, and retry settings.  Only `baseUrl` is required; all other
   *   fields have sensible defaults.
   */
  constructor(config: ApexClientConfig) {
    this.config = {
      baseUrl: config.baseUrl.replace(/\/$/, ''),
      apiKey: config.apiKey ?? '',
      timeout: config.timeout ?? 30000,
      retries: config.retries ?? 3,
      retryDelay: config.retryDelay ?? 1000,
      maxRetryDelay: config.maxRetryDelay ?? 30000,
      headers: config.headers ?? {},
      websocketUrl: config.websocketUrl ?? config.baseUrl.replace(/^http/, 'ws') + '/ws',
    };

    this.httpClient = this.createHttpClient();
  }

  // ============================================================================
  // HTTP Client Setup
  // ============================================================================

  private createHttpClient(): AxiosInstance {
    const client = axios.create({
      baseURL: this.config.baseUrl,
      timeout: this.config.timeout,
      headers: {
        'Content-Type': 'application/json',
        ...this.config.headers,
      },
    });

    // Request interceptor for auth
    client.interceptors.request.use(
      (config: InternalAxiosRequestConfig) => {
        if (this.config.apiKey) {
          config.headers['Authorization'] = `Bearer ${this.config.apiKey}`;
        }
        return config;
      },
      (error: AxiosError) => Promise.reject(error)
    );

    // Response interceptor for error handling
    client.interceptors.response.use(
      (response: AxiosResponse) => response,
      async (error: AxiosError) => {
        if (error.response) {
          const apiError = parseApiError(
            error.response.status,
            error.config?.url ?? '',
            error.config?.method?.toUpperCase() ?? 'UNKNOWN',
            error.response.data
          );
          throw apiError;
        }

        if (error.code === 'ECONNABORTED' || error.message.includes('timeout')) {
          throw new TimeoutError(
            `Request timed out after ${this.config.timeout}ms`,
            this.config.timeout
          );
        }

        throw new ApexError(
          error.message ?? 'Network error',
          'NETWORK_ERROR',
          { originalError: error.message }
        );
      }
    );

    return client;
  }

  // ============================================================================
  // Request Methods with Retry Logic
  // ============================================================================

  private async request<T>(
    method: string,
    path: string,
    data?: unknown,
    requestConfig?: RequestConfig
  ): Promise<T> {
    const config: AxiosRequestConfig = {
      method,
      url: path,
      data,
      timeout: requestConfig?.timeout ?? this.config.timeout,
      headers: requestConfig?.headers,
      signal: requestConfig?.signal,
    };

    let lastError: Error | undefined;
    let attempt = 0;

    while (attempt < this.config.retries) {
      try {
        const response = await this.httpClient.request<ApiResponse<T>>(config);
        return response.data.data;
      } catch (error) {
        lastError = error as Error;
        attempt++;

        // Don't retry on client errors (4xx) except rate limits (429)
        if (error instanceof ApexError) {
          const statusCode = (error as { statusCode?: number }).statusCode;
          if (statusCode !== undefined && statusCode >= 400 && statusCode < 500 && statusCode !== 429) {
            throw error;
          }
        }

        // Check if request was aborted
        if (requestConfig?.signal?.aborted) {
          throw error;
        }

        if (attempt < this.config.retries) {
          const delay = this.calculateRetryDelay(attempt);
          await sleep(delay);
        }
      }
    }

    throw new MaxRetriesExceededError(attempt, lastError);
  }

  private calculateRetryDelay(attempt: number): number {
    // Exponential backoff with jitter
    const exponentialDelay = this.config.retryDelay * Math.pow(2, attempt - 1);
    const jitter = Math.random() * 1000;
    return Math.min(exponentialDelay + jitter, this.config.maxRetryDelay);
  }

  private async get<T>(path: string, requestConfig?: RequestConfig): Promise<T> {
    return this.request<T>('GET', path, undefined, requestConfig);
  }

  private async post<T>(path: string, data?: unknown, requestConfig?: RequestConfig): Promise<T> {
    return this.request<T>('POST', path, data, requestConfig);
  }

  private async put<T>(path: string, data?: unknown, requestConfig?: RequestConfig): Promise<T> {
    return this.request<T>('PUT', path, data, requestConfig);
  }

  private async patch<T>(path: string, data?: unknown, requestConfig?: RequestConfig): Promise<T> {
    return this.request<T>('PATCH', path, data, requestConfig);
  }

  private async delete<T>(path: string, requestConfig?: RequestConfig): Promise<T> {
    return this.request<T>('DELETE', path, undefined, requestConfig);
  }

  // ============================================================================
  // Health Check
  // ============================================================================

  /**
   * Check API health status.
   *
   * @param config - Optional per-request overrides (timeout, headers, signal).
   * @returns Health status of the API and its dependencies.
   * @throws {@link ApexError} if the server is unreachable.
   */
  async healthCheck(config?: RequestConfig): Promise<HealthCheckResponse> {
    return this.get<HealthCheckResponse>('/health', config);
  }

  // ============================================================================
  // Task API
  // ============================================================================

  /**
   * List tasks with optional filters and pagination.
   *
   * @param filter - Optional filter criteria (status, priority, tags, pagination).
   * @param config - Optional per-request overrides.
   * @returns Paginated list of matching {@link Task} objects.
   */
  async listTasks(
    filter?: TaskFilter,
    config?: RequestConfig
  ): Promise<PaginatedResponse<Task>> {
    const query = filter ? buildQueryString(filter as Record<string, unknown>) : '';
    return this.get<PaginatedResponse<Task>>(`/api/v1/tasks${query}`, config);
  }

  /**
   * Retrieve a single task by its unique identifier.
   *
   * @param taskId - UUID of the task.
   * @param config - Optional per-request overrides.
   * @returns The requested {@link Task}.
   * @throws {@link ApexError} with status 404 if the task does not exist.
   */
  async getTask(taskId: string, config?: RequestConfig): Promise<Task> {
    return this.get<Task>(`/api/v1/tasks/${taskId}`, config);
  }

  /**
   * Submit a new task for execution.
   *
   * @param task - Task specification (name, description, input, priority, tags).
   * @param config - Optional per-request overrides.
   * @returns The newly created {@link Task} with a server-assigned ID.
   * @throws {@link ApexError} with status 422 if validation fails.
   *
   * @example
   * ```typescript
   * const task = await client.createTask({
   *   name: 'Analyse dataset',
   *   description: 'Run statistical analysis on Q4 data',
   *   input: { datasetUrl: 's3://bucket/data.parquet' },
   *   priority: 'high',
   * });
   * ```
   */
  async createTask(task: CreateTaskRequest, config?: RequestConfig): Promise<Task> {
    return this.post<Task>('/api/v1/tasks', task, config);
  }

  /**
   * Update mutable fields of an existing task.
   *
   * @param taskId - UUID of the task to update.
   * @param updates - Fields to change.
   * @param config - Optional per-request overrides.
   * @returns The updated {@link Task}.
   * @throws {@link ApexError} with status 404 if the task does not exist.
   */
  async updateTask(
    taskId: string,
    updates: UpdateTaskRequest,
    config?: RequestConfig
  ): Promise<Task> {
    return this.patch<Task>(`/api/v1/tasks/${taskId}`, updates, config);
  }

  /**
   * Permanently delete a task.
   *
   * @param taskId - UUID of the task to delete.
   * @param config - Optional per-request overrides.
   * @throws {@link ApexError} with status 404 if the task does not exist.
   */
  async deleteTask(taskId: string, config?: RequestConfig): Promise<void> {
    return this.delete<void>(`/api/v1/tasks/${taskId}`, config);
  }

  /**
   * Cancel a running or pending task.
   *
   * @param taskId - UUID of the task to cancel.
   * @param config - Optional per-request overrides.
   * @returns The {@link Task} in `cancelled` status.
   */
  async cancelTask(taskId: string, config?: RequestConfig): Promise<Task> {
    return this.post<Task>(`/api/v1/tasks/${taskId}/cancel`, undefined, config);
  }

  /**
   * Retry a failed task, resetting it to `pending` status.
   *
   * @param taskId - UUID of the task to retry.
   * @param config - Optional per-request overrides.
   * @returns The {@link Task} reset to `pending` status.
   */
  async retryTask(taskId: string, config?: RequestConfig): Promise<Task> {
    return this.post<Task>(`/api/v1/tasks/${taskId}/retry`, undefined, config);
  }

  /**
   * Pause a running task
   */
  async pauseTask(taskId: string, config?: RequestConfig): Promise<Task> {
    return this.post<Task>(`/api/v1/tasks/${taskId}/pause`, undefined, config);
  }

  /**
   * Resume a paused task
   */
  async resumeTask(taskId: string, config?: RequestConfig): Promise<Task> {
    return this.post<Task>(`/api/v1/tasks/${taskId}/resume`, undefined, config);
  }

  /**
   * Get task logs
   */
  async getTaskLogs(
    taskId: string,
    params?: { limit?: number; offset?: number; level?: string },
    config?: RequestConfig
  ): Promise<TaskLog[]> {
    const query = params ? buildQueryString(params as Record<string, unknown>) : '';
    return this.get<TaskLog[]>(`/api/v1/tasks/${taskId}/logs${query}`, config);
  }

  /**
   * Get child tasks of a parent task
   */
  async getChildTasks(
    taskId: string,
    filter?: TaskFilter,
    config?: RequestConfig
  ): Promise<PaginatedResponse<Task>> {
    const query = filter ? buildQueryString(filter as Record<string, unknown>) : '';
    return this.get<PaginatedResponse<Task>>(
      `/api/v1/tasks/${taskId}/children${query}`,
      config
    );
  }

  // ============================================================================
  // Agent API
  // ============================================================================

  /**
   * List registered agents with optional filters and pagination.
   *
   * @param filter - Optional filter criteria (status, tags, pagination).
   * @param config - Optional per-request overrides.
   * @returns Paginated list of matching {@link Agent} objects.
   */
  async listAgents(
    filter?: AgentFilter,
    config?: RequestConfig
  ): Promise<PaginatedResponse<Agent>> {
    const query = filter ? buildQueryString(filter as Record<string, unknown>) : '';
    return this.get<PaginatedResponse<Agent>>(`/api/v1/agents${query}`, config);
  }

  /**
   * Retrieve a single agent by its unique identifier.
   *
   * @param agentId - UUID of the agent.
   * @param config - Optional per-request overrides.
   * @returns The requested {@link Agent}.
   * @throws {@link ApexError} with status 404 if the agent does not exist.
   */
  async getAgent(agentId: string, config?: RequestConfig): Promise<Agent> {
    return this.get<Agent>(`/api/v1/agents/${agentId}`, config);
  }

  /**
   * Register a new agent with the orchestrator.
   *
   * @param agent - Agent specification (name, capabilities, model config).
   * @param config - Optional per-request overrides.
   * @returns The newly registered {@link Agent}.
   * @throws {@link ApexError} with status 422 if validation fails.
   */
  async createAgent(agent: CreateAgentRequest, config?: RequestConfig): Promise<Agent> {
    return this.post<Agent>('/api/v1/agents', agent, config);
  }

  /**
   * Update an agent
   */
  async updateAgent(
    agentId: string,
    updates: UpdateAgentRequest,
    config?: RequestConfig
  ): Promise<Agent> {
    return this.patch<Agent>(`/api/v1/agents/${agentId}`, updates, config);
  }

  /**
   * Delete an agent
   */
  async deleteAgent(agentId: string, config?: RequestConfig): Promise<void> {
    return this.delete<void>(`/api/v1/agents/${agentId}`, config);
  }

  /**
   * Get tasks assigned to an agent
   */
  async getAgentTasks(
    agentId: string,
    filter?: TaskFilter,
    config?: RequestConfig
  ): Promise<PaginatedResponse<Task>> {
    const query = filter ? buildQueryString(filter as Record<string, unknown>) : '';
    return this.get<PaginatedResponse<Task>>(
      `/api/v1/agents/${agentId}/tasks${query}`,
      config
    );
  }

  /**
   * Assign a task to an agent
   */
  async assignTask(
    agentId: string,
    taskId: string,
    config?: RequestConfig
  ): Promise<Task> {
    return this.post<Task>(`/api/v1/agents/${agentId}/assign`, { taskId }, config);
  }

  /**
   * Unassign a task from an agent
   */
  async unassignTask(
    agentId: string,
    taskId: string,
    config?: RequestConfig
  ): Promise<Task> {
    return this.post<Task>(`/api/v1/agents/${agentId}/unassign`, { taskId }, config);
  }

  // ============================================================================
  // DAG API
  // ============================================================================

  /**
   * List DAG definitions with optional filters and pagination.
   *
   * @param filter - Optional filter criteria (status, tags, pagination).
   * @param config - Optional per-request overrides.
   * @returns Paginated list of matching {@link DAG} objects.
   */
  async listDAGs(
    filter?: DAGFilter,
    config?: RequestConfig
  ): Promise<PaginatedResponse<DAG>> {
    const query = filter ? buildQueryString(filter as Record<string, unknown>) : '';
    return this.get<PaginatedResponse<DAG>>(`/api/v1/dags${query}`, config);
  }

  /**
   * Retrieve a DAG by ID, including its nodes and edges.
   *
   * @param dagId - UUID of the DAG.
   * @param config - Optional per-request overrides.
   * @returns The requested {@link DAG}.
   * @throws {@link ApexError} with status 404 if the DAG does not exist.
   */
  async getDAG(dagId: string, config?: RequestConfig): Promise<DAG> {
    return this.get<DAG>(`/api/v1/dags/${dagId}`, config);
  }

  /**
   * Create a new DAG workflow definition.
   *
   * The DAG is stored but not executed until {@link startDAG} is called.
   *
   * @param dag - Full DAG specification (nodes, edges, metadata).
   * @param config - Optional per-request overrides.
   * @returns The persisted {@link DAG} with a server-assigned ID.
   * @throws {@link ApexError} with status 422 if the definition is invalid
   *   (e.g. contains cycles or references missing nodes).
   *
   * @example
   * ```typescript
   * const dag = await client.createDAG({
   *   name: 'ETL Pipeline',
   *   nodes: [
   *     { id: 'extract', taskTemplate: { name: 'Extract' } },
   *     { id: 'transform', taskTemplate: { name: 'Transform' }, dependsOn: ['extract'] },
   *     { id: 'load', taskTemplate: { name: 'Load' }, dependsOn: ['transform'] },
   *   ],
   * });
   * ```
   */
  async createDAG(dag: CreateDAGRequest, config?: RequestConfig): Promise<DAG> {
    return this.post<DAG>('/api/v1/dags', dag, config);
  }

  /**
   * Update a DAG
   */
  async updateDAG(
    dagId: string,
    updates: UpdateDAGRequest,
    config?: RequestConfig
  ): Promise<DAG> {
    return this.patch<DAG>(`/api/v1/dags/${dagId}`, updates, config);
  }

  /**
   * Delete a DAG
   */
  async deleteDAG(dagId: string, config?: RequestConfig): Promise<void> {
    return this.delete<void>(`/api/v1/dags/${dagId}`, config);
  }

  /**
   * Start executing a DAG, scheduling its root nodes immediately.
   *
   * @param dagId - UUID of the DAG to execute.
   * @param input - Optional key-value input passed to root nodes.
   * @param config - Optional per-request overrides.
   * @returns A {@link DAGExecution} representing the running instance.
   */
  async startDAG(
    dagId: string,
    input?: Record<string, unknown>,
    config?: RequestConfig
  ): Promise<DAGExecution> {
    return this.post<DAGExecution>(`/api/v1/dags/${dagId}/start`, { input }, config);
  }

  /**
   * Stop a running DAG
   */
  async stopDAG(dagId: string, config?: RequestConfig): Promise<DAG> {
    return this.post<DAG>(`/api/v1/dags/${dagId}/stop`, undefined, config);
  }

  /**
   * Pause a running DAG
   */
  async pauseDAG(dagId: string, config?: RequestConfig): Promise<DAG> {
    return this.post<DAG>(`/api/v1/dags/${dagId}/pause`, undefined, config);
  }

  /**
   * Resume a paused DAG
   */
  async resumeDAG(dagId: string, config?: RequestConfig): Promise<DAG> {
    return this.post<DAG>(`/api/v1/dags/${dagId}/resume`, undefined, config);
  }

  /**
   * Get DAG execution history
   */
  async getDAGExecutions(
    dagId: string,
    params?: { limit?: number; offset?: number },
    config?: RequestConfig
  ): Promise<PaginatedResponse<DAGExecution>> {
    const query = params ? buildQueryString(params as Record<string, unknown>) : '';
    return this.get<PaginatedResponse<DAGExecution>>(
      `/api/v1/dags/${dagId}/executions${query}`,
      config
    );
  }

  /**
   * Get a specific DAG execution
   */
  async getDAGExecution(
    dagId: string,
    executionId: string,
    config?: RequestConfig
  ): Promise<DAGExecution> {
    return this.get<DAGExecution>(
      `/api/v1/dags/${dagId}/executions/${executionId}`,
      config
    );
  }

  // ============================================================================
  // Approval API
  // ============================================================================

  /**
   * List approvals with optional filters
   */
  async listApprovals(
    filter?: ApprovalFilter,
    config?: RequestConfig
  ): Promise<PaginatedResponse<Approval>> {
    const query = filter ? buildQueryString(filter as Record<string, unknown>) : '';
    return this.get<PaginatedResponse<Approval>>(`/api/v1/approvals${query}`, config);
  }

  /**
   * Get an approval by ID
   */
  async getApproval(approvalId: string, config?: RequestConfig): Promise<Approval> {
    return this.get<Approval>(`/api/v1/approvals/${approvalId}`, config);
  }

  /**
   * Create an approval request
   */
  async createApproval(
    approval: CreateApprovalRequest,
    config?: RequestConfig
  ): Promise<Approval> {
    return this.post<Approval>('/api/v1/approvals', approval, config);
  }

  /**
   * Respond to an approval request
   */
  async respondToApproval(
    approvalId: string,
    response: ApprovalResponse,
    config?: RequestConfig
  ): Promise<Approval> {
    return this.post<Approval>(
      `/api/v1/approvals/${approvalId}/respond`,
      response,
      config
    );
  }

  /**
   * Cancel an approval request
   */
  async cancelApproval(approvalId: string, config?: RequestConfig): Promise<Approval> {
    return this.post<Approval>(
      `/api/v1/approvals/${approvalId}/cancel`,
      undefined,
      config
    );
  }

  /**
   * Get pending approvals for a specific approver
   */
  async getPendingApprovals(
    approverId: string,
    config?: RequestConfig
  ): Promise<Approval[]> {
    return this.get<Approval[]>(
      `/api/v1/approvals/pending?approverId=${encodeURIComponent(approverId)}`,
      config
    );
  }

  // ============================================================================
  // WebSocket Methods
  // ============================================================================

  /**
   * Get or create a WebSocket client for real-time event streaming.
   *
   * The WebSocket instance is lazily created and reused across calls.
   * Call {@link connectWebSocket} to open the connection, or use the
   * returned object's `connect()` method directly.
   *
   * @param config - Optional WebSocket configuration overrides.
   * @returns A configured {@link ApexWebSocket} instance.
   */
  getWebSocket(config?: Partial<ApexWebSocketConfig>): ApexWebSocket {
    if (!this.websocket) {
      this.websocket = new ApexWebSocket({
        url: config?.url ?? this.config.websocketUrl,
        apiKey: config?.apiKey ?? this.config.apiKey,
        ...config,
      });
    }
    return this.websocket;
  }

  /**
   * Connect to WebSocket
   */
  async connectWebSocket(config?: Partial<ApexWebSocketConfig>): Promise<ApexWebSocket> {
    const ws = this.getWebSocket(config);
    await ws.connect();
    return ws;
  }

  /**
   * Disconnect WebSocket
   */
  disconnectWebSocket(): void {
    if (this.websocket) {
      this.websocket.disconnect();
      this.websocket = null;
    }
  }

  // ============================================================================
  // Convenience Methods
  // ============================================================================

  /**
   * Wait for a task to reach a terminal state (`completed`, `failed`, or `cancelled`).
   *
   * Supports two strategies: polling (default) or WebSocket push.
   *
   * @param taskId - UUID of the task to monitor.
   * @param options - Polling interval, timeout, and whether to use WebSocket.
   * @returns The {@link Task} in `completed` status.
   * @throws Error if the task fails or is cancelled.
   * @throws {@link TimeoutError} if the timeout elapses.
   *
   * @example
   * ```typescript
   * const task = await client.createTask({ name: 'Long job' });
   * const completed = await client.waitForTask(task.id, {
   *   pollInterval: 3000,
   *   timeout: 120000,
   * });
   * ```
   */
  async waitForTask(
    taskId: string,
    options?: {
      pollInterval?: number;
      timeout?: number;
      useWebSocket?: boolean;
    }
  ): Promise<Task> {
    const pollInterval = options?.pollInterval ?? 2000;
    const timeout = options?.timeout ?? 300000; // 5 minutes default
    const useWebSocket = options?.useWebSocket ?? false;

    if (useWebSocket) {
      const ws = await this.connectWebSocket();
      ws.subscribeToTask(taskId);

      try {
        return await ws.waitFor<Task>(
          'task.completed' as never,
          (task: Task) => task.id === taskId,
          timeout
        );
      } catch (error) {
        // Check if task failed
        const task = await this.getTask(taskId);
        if (task.status === 'failed') {
          throw new Error(`Task ${taskId} failed: ${task.error?.message ?? 'Unknown error'}`);
        }
        throw error;
      }
    }

    // Polling fallback
    const startTime = Date.now();

    while (Date.now() - startTime < timeout) {
      const task = await this.getTask(taskId);

      if (task.status === 'completed') {
        return task;
      }

      if (task.status === 'failed' || task.status === 'cancelled') {
        throw new Error(
          `Task ${taskId} ${task.status}: ${task.error?.message ?? 'Unknown error'}`
        );
      }

      await sleep(pollInterval);
    }

    throw new TimeoutError(`Timeout waiting for task ${taskId}`, timeout);
  }

  /**
   * Wait for a DAG to complete
   */
  async waitForDAG(
    dagId: string,
    executionId: string,
    options?: {
      pollInterval?: number;
      timeout?: number;
    }
  ): Promise<DAGExecution> {
    const pollInterval = options?.pollInterval ?? 5000;
    const timeout = options?.timeout ?? 600000; // 10 minutes default
    const startTime = Date.now();

    while (Date.now() - startTime < timeout) {
      const execution = await this.getDAGExecution(dagId, executionId);

      if (execution.status === 'completed') {
        return execution;
      }

      if (execution.status === 'failed') {
        throw new Error(
          `DAG ${dagId} execution ${executionId} failed: ${execution.error?.message ?? 'Unknown error'}`
        );
      }

      await sleep(pollInterval);
    }

    throw new TimeoutError(
      `Timeout waiting for DAG ${dagId} execution ${executionId}`,
      timeout
    );
  }

  /**
   * Create a task and block until it completes.
   *
   * Convenience wrapper combining {@link createTask} and {@link waitForTask}.
   *
   * @param task - Task specification.
   * @param options - Polling interval, timeout, and WebSocket flag.
   * @returns The completed {@link Task} with output data.
   * @throws Error if the task fails or is cancelled.
   * @throws {@link TimeoutError} if the timeout elapses.
   */
  async runTask(
    task: CreateTaskRequest,
    options?: {
      pollInterval?: number;
      timeout?: number;
      useWebSocket?: boolean;
    }
  ): Promise<Task> {
    const createdTask = await this.createTask(task);
    return this.waitForTask(createdTask.id, options);
  }

  /**
   * Start a DAG and block until execution completes.
   *
   * Convenience wrapper combining {@link startDAG} and {@link waitForDAG}.
   *
   * @param dagId - UUID of the DAG to execute.
   * @param input - Optional initial input for root nodes.
   * @param options - Polling interval and timeout.
   * @returns The completed {@link DAGExecution}.
   * @throws Error if the execution fails.
   * @throws {@link TimeoutError} if the timeout elapses.
   */
  async runDAG(
    dagId: string,
    input?: Record<string, unknown>,
    options?: {
      pollInterval?: number;
      timeout?: number;
    }
  ): Promise<DAGExecution> {
    const execution = await this.startDAG(dagId, input);
    return this.waitForDAG(dagId, execution.id, options);
  }
}

// ============================================================================
// Factory Function
// ============================================================================

export function createApexClient(config: ApexClientConfig): ApexClient {
  return new ApexClient(config);
}
