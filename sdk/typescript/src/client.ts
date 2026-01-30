/**
 * Project Apex TypeScript SDK - API Client
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

export class ApexClient {
  private readonly config: Required<ApexClientConfig>;
  private readonly httpClient: AxiosInstance;
  private websocket: ApexWebSocket | null = null;

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
   * Check API health status
   */
  async healthCheck(config?: RequestConfig): Promise<HealthCheckResponse> {
    return this.get<HealthCheckResponse>('/health', config);
  }

  // ============================================================================
  // Task API
  // ============================================================================

  /**
   * List tasks with optional filters
   */
  async listTasks(
    filter?: TaskFilter,
    config?: RequestConfig
  ): Promise<PaginatedResponse<Task>> {
    const query = filter ? buildQueryString(filter as Record<string, unknown>) : '';
    return this.get<PaginatedResponse<Task>>(`/api/v1/tasks${query}`, config);
  }

  /**
   * Get a task by ID
   */
  async getTask(taskId: string, config?: RequestConfig): Promise<Task> {
    return this.get<Task>(`/api/v1/tasks/${taskId}`, config);
  }

  /**
   * Create a new task
   */
  async createTask(task: CreateTaskRequest, config?: RequestConfig): Promise<Task> {
    return this.post<Task>('/api/v1/tasks', task, config);
  }

  /**
   * Update a task
   */
  async updateTask(
    taskId: string,
    updates: UpdateTaskRequest,
    config?: RequestConfig
  ): Promise<Task> {
    return this.patch<Task>(`/api/v1/tasks/${taskId}`, updates, config);
  }

  /**
   * Delete a task
   */
  async deleteTask(taskId: string, config?: RequestConfig): Promise<void> {
    return this.delete<void>(`/api/v1/tasks/${taskId}`, config);
  }

  /**
   * Cancel a running task
   */
  async cancelTask(taskId: string, config?: RequestConfig): Promise<Task> {
    return this.post<Task>(`/api/v1/tasks/${taskId}/cancel`, undefined, config);
  }

  /**
   * Retry a failed task
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
   * List agents with optional filters
   */
  async listAgents(
    filter?: AgentFilter,
    config?: RequestConfig
  ): Promise<PaginatedResponse<Agent>> {
    const query = filter ? buildQueryString(filter as Record<string, unknown>) : '';
    return this.get<PaginatedResponse<Agent>>(`/api/v1/agents${query}`, config);
  }

  /**
   * Get an agent by ID
   */
  async getAgent(agentId: string, config?: RequestConfig): Promise<Agent> {
    return this.get<Agent>(`/api/v1/agents/${agentId}`, config);
  }

  /**
   * Create a new agent
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
   * List DAGs with optional filters
   */
  async listDAGs(
    filter?: DAGFilter,
    config?: RequestConfig
  ): Promise<PaginatedResponse<DAG>> {
    const query = filter ? buildQueryString(filter as Record<string, unknown>) : '';
    return this.get<PaginatedResponse<DAG>>(`/api/v1/dags${query}`, config);
  }

  /**
   * Get a DAG by ID
   */
  async getDAG(dagId: string, config?: RequestConfig): Promise<DAG> {
    return this.get<DAG>(`/api/v1/dags/${dagId}`, config);
  }

  /**
   * Create a new DAG
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
   * Start a DAG execution
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
   * Get WebSocket client for real-time updates
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
   * Wait for a task to complete
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
   * Create and run a task, waiting for completion
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
   * Start a DAG and wait for completion
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
