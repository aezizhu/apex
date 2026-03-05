import type {
  Agent,
  AgentCreateRequest,
  AgentUpdateRequest,
  Task,
  TaskCreateRequest,
  TaskCancelRequest,
  DelegationResponse,
  ApprovalRequest,
  ApprovalDecision,
  SystemMetrics,
  SystemSettings,
  ApiKeySummary,
  CreateApiKeyRequest,
  ApiResponse,
  ApiListResponse,
  DAG,
  DAGCreateRequest,
} from '../types'

const BASE_URL = '/api'

async function fetchApi<T>(url: string, options?: RequestInit): Promise<T> {
  const response = await fetch(`${BASE_URL}${url}`, {
    headers: {
      'Content-Type': 'application/json',
      ...options?.headers,
    },
    ...options,
  })

  if (!response.ok) {
    const error = await response.json().catch(() => ({ message: 'Request failed' }))
    throw new Error(error.message || `HTTP ${response.status}`)
  }

  return response.json()
}

// ─────────────────────────────────────────────────────────────────────────────
// Agent API
// ─────────────────────────────────────────────────────────────────────────────

export const agentApi = {
  list: (params?: { page?: number; pageSize?: number; status?: string }) => {
    const searchParams = new URLSearchParams()
    if (params?.page) searchParams.set('page', String(params.page))
    if (params?.pageSize) searchParams.set('pageSize', String(params.pageSize))
    if (params?.status) searchParams.set('status', params.status)
    const query = searchParams.toString()
    return fetchApi<ApiListResponse<Agent>>(`/agents${query ? `?${query}` : ''}`)
  },

  listRaw: () => fetchApi<ApiListResponse<Agent>>('/agents'),

  get: (id: string) => fetchApi<ApiResponse<Agent>>(`/agents/${id}`),

  create: (data: AgentCreateRequest) =>
    fetchApi<ApiResponse<Agent>>('/agents', {
      method: 'POST',
      body: JSON.stringify(data),
    }),

  update: (id: string, data: AgentUpdateRequest) =>
    fetchApi<ApiResponse<Agent>>(`/agents/${id}`, {
      method: 'PATCH',
      body: JSON.stringify(data),
    }),

  delete: (id: string) =>
    fetchApi<ApiResponse<{ deleted: boolean }>>(`/agents/${id}`, {
      method: 'DELETE',
    }),

  pause: (id: string) =>
    fetchApi<ApiResponse<Agent>>(`/agents/${id}/pause`, {
      method: 'POST',
    }),

  resume: (id: string) =>
    fetchApi<ApiResponse<Agent>>(`/agents/${id}/resume`, {
      method: 'POST',
    }),

  delegate: (agentId: string, req: { task: string; strategy?: string; priority?: string }) =>
    fetchApi<ApiResponse<DelegationResponse>>(`/agents/${agentId}/delegate`, {
      method: 'POST',
      body: JSON.stringify(req),
    }),
}

// ─────────────────────────────────────────────────────────────────────────────
// Task API
// ─────────────────────────────────────────────────────────────────────────────

export const taskApi = {
  list: (params?: { page?: number; pageSize?: number; status?: string; agentId?: string }) => {
    const searchParams = new URLSearchParams()
    if (params?.page) searchParams.set('page', String(params.page))
    if (params?.pageSize) searchParams.set('pageSize', String(params.pageSize))
    if (params?.status) searchParams.set('status', params.status)
    if (params?.agentId) searchParams.set('agent_id', params.agentId)
    const query = searchParams.toString()
    return fetchApi<ApiListResponse<Task>>(`/tasks${query ? `?${query}` : ''}`)
  },

  get: (id: string) => fetchApi<ApiResponse<Task>>(`/tasks/${id}`),

  create: (data: TaskCreateRequest) =>
    fetchApi<ApiResponse<Task>>('/tasks', {
      method: 'POST',
      body: JSON.stringify(data),
    }),

  cancel: (id: string, reason?: string) =>
    fetchApi<ApiResponse<Task>>(`/tasks/${id}/cancel`, {
      method: 'POST',
      body: JSON.stringify({ reason } as TaskCancelRequest),
    }),

  retry: (id: string) =>
    fetchApi<ApiResponse<Task>>(`/tasks/${id}/retry`, {
      method: 'POST',
    }),
}

// ─────────────────────────────────────────────────────────────────────────────
// Metrics API
// ─────────────────────────────────────────────────────────────────────────────

export const metricsApi = {
  getSystem: () => fetchApi<ApiResponse<SystemMetrics>>('/metrics/system'),

  getAgent: (agentId: string) =>
    fetchApi<ApiResponse<{ agentId: string; tasksCompleted: number; tasksFailed: number; avgLatencyMs: number; totalTokens: number; totalCost: number; successRate: number }>>(`/metrics/agents/${agentId}`),
}

// ─────────────────────────────────────────────────────────────────────────────
// Approval API
// ─────────────────────────────────────────────────────────────────────────────

export const approvalApi = {
  list: (params?: { status?: string; page?: number; pageSize?: number }) => {
    const searchParams = new URLSearchParams()
    if (params?.status) searchParams.set('status', params.status)
    if (params?.page) searchParams.set('page', String(params.page))
    if (params?.pageSize) searchParams.set('pageSize', String(params.pageSize))
    const query = searchParams.toString()
    return fetchApi<ApiListResponse<ApprovalRequest>>(`/approvals${query ? `?${query}` : ''}`)
  },

  get: (id: string) => fetchApi<ApiResponse<ApprovalRequest>>(`/approvals/${id}`),

  approve: (id: string, reason?: string) =>
    fetchApi<ApiResponse<ApprovalRequest>>(`/approvals/${id}/decide`, {
      method: 'POST',
      body: JSON.stringify({ status: 'approved', reason } as ApprovalDecision),
    }),

  deny: (id: string, reason?: string) =>
    fetchApi<ApiResponse<ApprovalRequest>>(`/approvals/${id}/decide`, {
      method: 'POST',
      body: JSON.stringify({ status: 'denied', reason } as ApprovalDecision),
    }),

  decide: (id: string, decision: ApprovalDecision) =>
    fetchApi<ApiResponse<ApprovalRequest>>(`/approvals/${id}/decide`, {
      method: 'POST',
      body: JSON.stringify(decision),
    }),
}

// ─────────────────────────────────────────────────────────────────────────────
// Settings API
// ─────────────────────────────────────────────────────────────────────────────

export const settingsApi = {
  get: () => fetchApi<ApiResponse<SystemSettings>>('/settings'),

  update: (settings: Partial<SystemSettings>) =>
    fetchApi<ApiResponse<SystemSettings>>('/settings', {
      method: 'PATCH',
      body: JSON.stringify(settings),
    }),

  listApiKeys: () => fetchApi<ApiListResponse<ApiKeySummary>>('/settings/api-keys'),

  getApiKeys: () => fetchApi<ApiListResponse<ApiKeySummary>>('/settings/api-keys'),

  createApiKey: (data: CreateApiKeyRequest) =>
    fetchApi<ApiResponse<ApiKeySummary>>('/settings/api-keys', {
      method: 'POST',
      body: JSON.stringify(data),
    }),

  deleteApiKey: (id: string) =>
    fetchApi<ApiResponse<{ deleted: boolean }>>(`/settings/api-keys/${id}`, {
      method: 'DELETE',
    }),

  revokeApiKey: (id: string) =>
    fetchApi<ApiResponse<{ deleted: boolean }>>(`/settings/api-keys/${id}/revoke`, {
      method: 'POST',
    }),
}

// ─────────────────────────────────────────────────────────────────────────────
// DAG/Workflow API
// ─────────────────────────────────────────────────────────────────────────────

export const dagApi = {
  list: (params?: { page?: number; pageSize?: number }) => {
    const searchParams = new URLSearchParams()
    if (params?.page) searchParams.set('page', String(params.page))
    if (params?.pageSize) searchParams.set('pageSize', String(params.pageSize))
    const query = searchParams.toString()
    return fetchApi<ApiListResponse<DAG>>(`/dags${query ? `?${query}` : ''}`)
  },

  get: (id: string) => fetchApi<ApiResponse<DAG>>(`/dags/${id}`),

  create: (data: DAGCreateRequest) =>
    fetchApi<ApiResponse<DAG>>('/dags', {
      method: 'POST',
      body: JSON.stringify(data),
    }),

  delete: (id: string) =>
    fetchApi<ApiResponse<{ deleted: boolean }>>(`/dags/${id}`, {
      method: 'DELETE',
    }),
}
