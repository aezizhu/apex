# Apex API Documentation

> Complete reference for the Apex Orchestration API

## Base URL

```
http://localhost:8080/api/v1
```

## Authentication

All API requests require an API key in the `Authorization` header:

```
Authorization: Bearer <api-key>
```

---

## Tasks

### Submit Task

Create and execute a new task.

**POST** `/tasks`

```json
{
  "name": "Research AI Trends",
  "instruction": "Research the latest trends in AI agents and summarize key findings",
  "context": {
    "domain": "technology",
    "depth": "comprehensive"
  },
  "parameters": {
    "output_format": "markdown",
    "max_sources": 10
  },
  "limits": {
    "token_limit": 10000,
    "cost_limit": 0.10,
    "api_call_limit": 50,
    "time_limit_seconds": 300
  },
  "priority": 5
}
```

**Response** `201 Created`

```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "name": "Research AI Trends",
  "status": "pending",
  "created_at": "2024-01-15T10:30:00Z",
  "estimated_completion": "2024-01-15T10:35:00Z"
}
```

### Get Task

Retrieve task details and status.

**GET** `/tasks/{task_id}`

**Response** `200 OK`

```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "name": "Research AI Trends",
  "status": "completed",
  "input": {
    "instruction": "Research the latest trends in AI agents...",
    "context": { "domain": "technology" }
  },
  "output": {
    "result": "## AI Agent Trends 2024\n\n...",
    "data": { "sources_used": 8 },
    "artifacts": []
  },
  "agent_id": "agent-001",
  "contract": {
    "tokens_used": 8542,
    "cost_used": 0.085,
    "api_calls": 12
  },
  "created_at": "2024-01-15T10:30:00Z",
  "started_at": "2024-01-15T10:30:05Z",
  "completed_at": "2024-01-15T10:32:15Z"
}
```

### List Tasks

List all tasks with optional filtering.

**GET** `/tasks?status=pending&limit=20&offset=0`

**Query Parameters:**
- `status` - Filter by status: `pending`, `running`, `completed`, `failed`, `cancelled`
- `agent_id` - Filter by assigned agent
- `dag_id` - Filter by parent DAG
- `created_after` - Filter by creation time (ISO 8601)
- `created_before` - Filter by creation time (ISO 8601)
- `limit` - Max results (default 20, max 100)
- `offset` - Pagination offset

**Response** `200 OK`

```json
{
  "tasks": [...],
  "total": 150,
  "limit": 20,
  "offset": 0
}
```

### Cancel Task

Cancel a pending or running task.

**POST** `/tasks/{task_id}/cancel`

**Response** `200 OK`

```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "status": "cancelled",
  "cancelled_at": "2024-01-15T10:31:00Z"
}
```

### Retry Task

Retry a failed task.

**POST** `/tasks/{task_id}/retry`

**Response** `200 OK`

```json
{
  "id": "660e8400-e29b-41d4-a716-446655440000",
  "original_task_id": "550e8400-e29b-41d4-a716-446655440000",
  "status": "pending"
}
```

---

## DAGs (Directed Acyclic Graphs)

### Create DAG

Create a new task DAG for complex workflows.

**POST** `/dags`

```json
{
  "name": "Research Pipeline",
  "description": "Multi-stage research and analysis workflow",
  "tasks": [
    {
      "id": "research",
      "name": "Research",
      "instruction": "Research the topic",
      "limits": { "token_limit": 5000 }
    },
    {
      "id": "analyze",
      "name": "Analyze",
      "instruction": "Analyze the research findings"
    },
    {
      "id": "report",
      "name": "Report",
      "instruction": "Generate final report"
    }
  ],
  "dependencies": [
    { "from": "research", "to": "analyze" },
    { "from": "analyze", "to": "report" }
  ],
  "limits": {
    "token_limit": 30000,
    "cost_limit": 0.50,
    "time_limit_seconds": 900
  }
}
```

**Response** `201 Created`

```json
{
  "id": "dag-550e8400-e29b-41d4-a716-446655440000",
  "name": "Research Pipeline",
  "status": "pending",
  "task_count": 3,
  "created_at": "2024-01-15T10:30:00Z"
}
```

### Execute DAG

Start execution of a DAG.

**POST** `/dags/{dag_id}/execute`

**Response** `200 OK`

```json
{
  "id": "dag-550e8400-e29b-41d4-a716-446655440000",
  "status": "running",
  "started_at": "2024-01-15T10:30:05Z"
}
```

### Get DAG Status

Retrieve DAG status and progress.

**GET** `/dags/{dag_id}`

**Response** `200 OK`

```json
{
  "id": "dag-550e8400-e29b-41d4-a716-446655440000",
  "name": "Research Pipeline",
  "status": "running",
  "progress": {
    "total": 3,
    "pending": 1,
    "running": 1,
    "completed": 1,
    "failed": 0
  },
  "tasks": [
    { "id": "research", "status": "completed" },
    { "id": "analyze", "status": "running" },
    { "id": "report", "status": "pending" }
  ],
  "contract": {
    "tokens_used": 12000,
    "tokens_limit": 30000,
    "cost_used": 0.18,
    "cost_limit": 0.50
  }
}
```

### Cancel DAG

Cancel a running DAG and all pending tasks.

**POST** `/dags/{dag_id}/cancel`

**Response** `200 OK`

```json
{
  "id": "dag-550e8400-e29b-41d4-a716-446655440000",
  "status": "cancelled",
  "tasks_cancelled": 2
}
```

---

## Agents

### List Agents

List all registered agents.

**GET** `/agents?status=active&limit=50`

**Query Parameters:**
- `status` - Filter by status: `idle`, `busy`, `paused`, `offline`
- `model` - Filter by model type
- `limit` - Max results
- `offset` - Pagination offset

**Response** `200 OK`

```json
{
  "agents": [
    {
      "id": "agent-001",
      "name": "Research Agent",
      "status": "busy",
      "model": "gpt-4o",
      "current_task": "550e8400-e29b-41d4-a716-446655440000",
      "stats": {
        "tasks_completed": 150,
        "success_rate": 0.98,
        "avg_latency_ms": 2500
      }
    }
  ],
  "total": 25,
  "active": 20,
  "idle": 3,
  "paused": 2
}
```

### Get Agent

Retrieve agent details.

**GET** `/agents/{agent_id}`

**Response** `200 OK`

```json
{
  "id": "agent-001",
  "name": "Research Agent",
  "status": "busy",
  "model": "gpt-4o",
  "config": {
    "system_prompt": "You are a research assistant...",
    "tools": ["web_search", "read_file", "write_file"],
    "max_iterations": 10,
    "temperature": 0.7
  },
  "current_task": {
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "name": "Research AI Trends",
    "started_at": "2024-01-15T10:30:05Z"
  },
  "contract": {
    "tokens_used": 5000,
    "tokens_limit": 20000,
    "cost_used": 0.05,
    "cost_limit": 0.25
  },
  "stats": {
    "tasks_completed": 150,
    "tasks_failed": 3,
    "success_rate": 0.98,
    "total_tokens": 2500000,
    "total_cost": 25.50,
    "avg_latency_ms": 2500
  }
}
```

### Pause Agent

Pause an agent (finishes current task, won't accept new tasks).

**POST** `/agents/{agent_id}/pause`

**Response** `200 OK`

### Resume Agent

Resume a paused agent.

**POST** `/agents/{agent_id}/resume`

**Response** `200 OK`

---

## Approvals

### List Pending Approvals

List approval requests requiring human decision.

**GET** `/approvals?status=pending`

**Response** `200 OK`

```json
{
  "approvals": [
    {
      "id": "approval-001",
      "type": "high_cost_action",
      "status": "pending",
      "task_id": "550e8400-e29b-41d4-a716-446655440000",
      "agent_id": "agent-001",
      "action": {
        "type": "external_api_call",
        "description": "Make payment of $50.00",
        "estimated_cost": 50.00
      },
      "context": {
        "task_name": "Process Invoice",
        "iteration": 3
      },
      "created_at": "2024-01-15T10:30:00Z",
      "expires_at": "2024-01-15T11:30:00Z"
    }
  ],
  "total": 5
}
```

### Approve Request

Approve a pending approval request.

**POST** `/approvals/{approval_id}/approve`

```json
{
  "comment": "Approved after verification"
}
```

**Response** `200 OK`

### Deny Request

Deny a pending approval request.

**POST** `/approvals/{approval_id}/deny`

```json
{
  "reason": "Amount exceeds budget threshold"
}
```

**Response** `200 OK`

---

## Metrics

### System Metrics

Get current system-wide metrics.

**GET** `/metrics/system`

**Response** `200 OK`

```json
{
  "agents": {
    "total": 50,
    "active": 45,
    "idle": 3,
    "paused": 2,
    "offline": 0
  },
  "tasks": {
    "pending": 120,
    "running": 45,
    "completed_today": 850,
    "failed_today": 12,
    "success_rate": 0.986
  },
  "resources": {
    "tokens_used_today": 15000000,
    "cost_today": 150.25,
    "api_calls_today": 25000
  },
  "performance": {
    "avg_task_latency_ms": 3500,
    "p95_task_latency_ms": 8000,
    "p99_task_latency_ms": 15000,
    "throughput_per_minute": 45
  },
  "timestamp": "2024-01-15T10:30:00Z"
}
```

### Time Series Metrics

Get historical metrics for charting.

**GET** `/metrics/timeseries?metric=task_completions&interval=1h&from=2024-01-14T00:00:00Z&to=2024-01-15T00:00:00Z`

**Query Parameters:**
- `metric` - Metric name: `task_completions`, `tokens_used`, `cost`, `latency`, `agent_utilization`
- `interval` - Aggregation interval: `1m`, `5m`, `15m`, `1h`, `1d`
- `from` - Start time (ISO 8601)
- `to` - End time (ISO 8601)

**Response** `200 OK`

```json
{
  "metric": "task_completions",
  "interval": "1h",
  "data": [
    { "timestamp": "2024-01-14T00:00:00Z", "value": 42 },
    { "timestamp": "2024-01-14T01:00:00Z", "value": 38 },
    { "timestamp": "2024-01-14T02:00:00Z", "value": 35 }
  ]
}
```

---

## WebSocket Streams

### Real-time Updates

Connect to receive real-time updates.

**WS** `/ws`

**Subscribe to channels:**

```json
{
  "type": "subscribe",
  "channels": ["tasks", "agents", "metrics", "approvals"]
}
```

**Event types:**

```json
// Task update
{
  "type": "task_update",
  "data": {
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "status": "completed",
    "updated_at": "2024-01-15T10:32:15Z"
  }
}

// Agent update
{
  "type": "agent_update",
  "data": {
    "id": "agent-001",
    "status": "idle",
    "updated_at": "2024-01-15T10:32:15Z"
  }
}

// Metrics update (every 5 seconds)
{
  "type": "metrics_update",
  "data": {
    "active_agents": 45,
    "pending_tasks": 118,
    "tasks_per_minute": 45,
    "timestamp": "2024-01-15T10:32:15Z"
  }
}

// New approval required
{
  "type": "approval_required",
  "data": {
    "id": "approval-001",
    "type": "high_cost_action",
    "task_id": "550e8400-e29b-41d4-a716-446655440000"
  }
}
```

---

## Health Checks

### Health

Basic health check.

**GET** `/health`

**Response** `200 OK`

```json
{
  "status": "healthy",
  "version": "0.1.0"
}
```

### Readiness

Readiness probe (checks dependencies).

**GET** `/ready`

**Response** `200 OK`

```json
{
  "ready": true,
  "checks": {
    "database": "ok",
    "redis": "ok",
    "agents": "ok"
  }
}
```

### Liveness

Liveness probe.

**GET** `/live`

**Response** `200 OK`

```json
{
  "alive": true
}
```

---

## Error Responses

All errors follow this format:

```json
{
  "error": {
    "code": "TASK_NOT_FOUND",
    "message": "Task with ID '550e8400-e29b-41d4-a716-446655440000' not found",
    "details": {
      "task_id": "550e8400-e29b-41d4-a716-446655440000"
    }
  }
}
```

**Common Error Codes:**
- `400` Bad Request - Invalid request body or parameters
- `401` Unauthorized - Missing or invalid API key
- `403` Forbidden - Insufficient permissions
- `404` Not Found - Resource not found
- `409` Conflict - Resource conflict (e.g., duplicate task)
- `429` Too Many Requests - Rate limit exceeded
- `500` Internal Server Error - Server error
- `503` Service Unavailable - Service temporarily unavailable

---

## Rate Limits

- **Standard tier**: 100 requests/minute
- **Pro tier**: 1000 requests/minute
- **Enterprise tier**: Unlimited

Rate limit headers:
```
X-RateLimit-Limit: 100
X-RateLimit-Remaining: 95
X-RateLimit-Reset: 1705315860
```

---

## SDKs

Official SDKs:
- **Python**: `pip install apex-sdk`
- **TypeScript**: `npm install @apex-swarm/sdk`
- **Rust**: `cargo add apex-sdk`

Example (Python):

```python
from apex import ApexClient

client = ApexClient(api_key="your-api-key")

# Submit a task
task = client.tasks.create(
    name="Research AI Trends",
    instruction="Research the latest trends in AI agents",
    limits={"token_limit": 10000, "cost_limit": 0.10}
)

# Wait for completion
result = task.wait()
print(result.output)
```
