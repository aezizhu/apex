"""Pydantic models for the Apex SDK."""

from datetime import datetime
from enum import Enum
from typing import Any

from pydantic import BaseModel, ConfigDict, Field


# Enums


class TaskStatus(str, Enum):
    """Task execution status."""

    PENDING = "pending"
    QUEUED = "queued"
    RUNNING = "running"
    PAUSED = "paused"
    COMPLETED = "completed"
    FAILED = "failed"
    CANCELLED = "cancelled"


class TaskPriority(str, Enum):
    """Task priority levels."""

    LOW = "low"
    NORMAL = "normal"
    HIGH = "high"
    CRITICAL = "critical"


class AgentStatus(str, Enum):
    """Agent status."""

    IDLE = "idle"
    BUSY = "busy"
    OFFLINE = "offline"
    ERROR = "error"


class DAGStatus(str, Enum):
    """DAG execution status."""

    PENDING = "pending"
    RUNNING = "running"
    COMPLETED = "completed"
    FAILED = "failed"
    CANCELLED = "cancelled"
    PAUSED = "paused"


class ApprovalStatus(str, Enum):
    """Approval status."""

    PENDING = "pending"
    APPROVED = "approved"
    REJECTED = "rejected"
    EXPIRED = "expired"


class ApprovalType(str, Enum):
    """Approval type."""

    MANUAL = "manual"
    AUTO = "auto"
    CONDITIONAL = "conditional"


class WebSocketEventType(str, Enum):
    """WebSocket event types."""

    TASK_CREATED = "task.created"
    TASK_UPDATED = "task.updated"
    TASK_COMPLETED = "task.completed"
    TASK_FAILED = "task.failed"
    AGENT_STATUS_CHANGED = "agent.status_changed"
    DAG_STARTED = "dag.started"
    DAG_COMPLETED = "dag.completed"
    DAG_FAILED = "dag.failed"
    APPROVAL_REQUIRED = "approval.required"
    APPROVAL_COMPLETED = "approval.completed"
    LOG_MESSAGE = "log.message"
    HEARTBEAT = "heartbeat"
    ERROR = "error"


# Base Models


class ApexBaseModel(BaseModel):
    """Base model with common configuration."""

    model_config = ConfigDict(
        populate_by_name=True,
        use_enum_values=True,
        extra="ignore",
    )


class TimestampedModel(ApexBaseModel):
    """Model with timestamp fields."""

    created_at: datetime = Field(alias="createdAt")
    updated_at: datetime = Field(alias="updatedAt")


# Task Models


class TaskInput(ApexBaseModel):
    """Input data for a task."""

    data: dict[str, Any] = Field(default_factory=dict)
    files: list[str] | None = None
    parameters: dict[str, Any] | None = None


class TaskOutput(ApexBaseModel):
    """Output data from a task."""

    result: Any | None = None
    artifacts: list[str] | None = None
    metrics: dict[str, Any] | None = None


class TaskError(ApexBaseModel):
    """Task error information."""

    code: str
    message: str
    details: dict[str, Any] | None = None
    stack_trace: str | None = Field(default=None, alias="stackTrace")


class TaskCreate(ApexBaseModel):
    """Request model for creating a task."""

    name: str = Field(min_length=1, max_length=255)
    description: str | None = None
    agent_id: str | None = Field(default=None, alias="agentId")
    priority: TaskPriority = TaskPriority.NORMAL
    input: TaskInput | None = None
    timeout_seconds: int | None = Field(default=None, alias="timeoutSeconds", ge=0)
    retries: int = Field(default=0, ge=0, le=10)
    tags: list[str] | None = None
    metadata: dict[str, Any] | None = None
    dag_id: str | None = Field(default=None, alias="dagId")
    depends_on: list[str] | None = Field(default=None, alias="dependsOn")


class TaskUpdate(ApexBaseModel):
    """Request model for updating a task."""

    name: str | None = Field(default=None, min_length=1, max_length=255)
    description: str | None = None
    priority: TaskPriority | None = None
    status: TaskStatus | None = None
    input: TaskInput | None = None
    output: TaskOutput | None = None
    tags: list[str] | None = None
    metadata: dict[str, Any] | None = None


class Task(TimestampedModel):
    """Task model."""

    id: str
    name: str
    description: str | None = None
    status: TaskStatus
    priority: TaskPriority
    agent_id: str | None = Field(default=None, alias="agentId")
    input: TaskInput | None = None
    output: TaskOutput | None = None
    error: TaskError | None = None
    timeout_seconds: int | None = Field(default=None, alias="timeoutSeconds")
    retries: int = 0
    retry_count: int = Field(default=0, alias="retryCount")
    tags: list[str] = Field(default_factory=list)
    metadata: dict[str, Any] = Field(default_factory=dict)
    dag_id: str | None = Field(default=None, alias="dagId")
    depends_on: list[str] = Field(default_factory=list, alias="dependsOn")
    started_at: datetime | None = Field(default=None, alias="startedAt")
    completed_at: datetime | None = Field(default=None, alias="completedAt")


# Agent Models


class AgentCapability(ApexBaseModel):
    """Agent capability definition."""

    name: str
    version: str | None = None
    parameters: dict[str, Any] | None = None


class AgentCreate(ApexBaseModel):
    """Request model for creating an agent."""

    name: str = Field(min_length=1, max_length=255)
    description: str | None = None
    capabilities: list[AgentCapability] = Field(default_factory=list)
    max_concurrent_tasks: int = Field(default=1, alias="maxConcurrentTasks", ge=1, le=100)
    tags: list[str] | None = None
    metadata: dict[str, Any] | None = None


class AgentUpdate(ApexBaseModel):
    """Request model for updating an agent."""

    name: str | None = Field(default=None, min_length=1, max_length=255)
    description: str | None = None
    status: AgentStatus | None = None
    capabilities: list[AgentCapability] | None = None
    max_concurrent_tasks: int | None = Field(default=None, alias="maxConcurrentTasks", ge=1, le=100)
    tags: list[str] | None = None
    metadata: dict[str, Any] | None = None


class Agent(TimestampedModel):
    """Agent model."""

    id: str
    name: str
    description: str | None = None
    status: AgentStatus
    capabilities: list[AgentCapability] = Field(default_factory=list)
    max_concurrent_tasks: int = Field(default=1, alias="maxConcurrentTasks")
    current_tasks: int = Field(default=0, alias="currentTasks")
    total_tasks_completed: int = Field(default=0, alias="totalTasksCompleted")
    tags: list[str] = Field(default_factory=list)
    metadata: dict[str, Any] = Field(default_factory=dict)
    last_heartbeat: datetime | None = Field(default=None, alias="lastHeartbeat")


# DAG Models


class DAGNode(ApexBaseModel):
    """DAG node definition."""

    id: str
    task_template: TaskCreate = Field(alias="taskTemplate")
    depends_on: list[str] = Field(default_factory=list, alias="dependsOn")
    condition: str | None = None
    retry_policy: dict[str, Any] | None = Field(default=None, alias="retryPolicy")


class DAGEdge(ApexBaseModel):
    """DAG edge definition."""

    source: str
    target: str
    condition: str | None = None


class DAGCreate(ApexBaseModel):
    """Request model for creating a DAG."""

    name: str = Field(min_length=1, max_length=255)
    description: str | None = None
    nodes: list[DAGNode]
    edges: list[DAGEdge] | None = None
    input: dict[str, Any] | None = None
    tags: list[str] | None = None
    metadata: dict[str, Any] | None = None
    schedule: str | None = None  # Cron expression


class DAGUpdate(ApexBaseModel):
    """Request model for updating a DAG."""

    name: str | None = Field(default=None, min_length=1, max_length=255)
    description: str | None = None
    nodes: list[DAGNode] | None = None
    edges: list[DAGEdge] | None = None
    tags: list[str] | None = None
    metadata: dict[str, Any] | None = None
    schedule: str | None = None


class DAGTaskStatus(ApexBaseModel):
    """Status of a task within a DAG execution."""

    node_id: str = Field(alias="nodeId")
    task_id: str | None = Field(default=None, alias="taskId")
    status: TaskStatus
    started_at: datetime | None = Field(default=None, alias="startedAt")
    completed_at: datetime | None = Field(default=None, alias="completedAt")


class DAG(TimestampedModel):
    """DAG model."""

    id: str
    name: str
    description: str | None = None
    status: DAGStatus
    nodes: list[DAGNode]
    edges: list[DAGEdge] = Field(default_factory=list)
    input: dict[str, Any] = Field(default_factory=dict)
    output: dict[str, Any] = Field(default_factory=dict)
    tags: list[str] = Field(default_factory=list)
    metadata: dict[str, Any] = Field(default_factory=dict)
    schedule: str | None = None
    task_statuses: list[DAGTaskStatus] = Field(default_factory=list, alias="taskStatuses")
    started_at: datetime | None = Field(default=None, alias="startedAt")
    completed_at: datetime | None = Field(default=None, alias="completedAt")


# Approval Models


class ApprovalCreate(ApexBaseModel):
    """Request model for creating an approval."""

    task_id: str = Field(alias="taskId")
    type: ApprovalType = ApprovalType.MANUAL
    description: str | None = None
    required_approvers: list[str] | None = Field(default=None, alias="requiredApprovers")
    expires_at: datetime | None = Field(default=None, alias="expiresAt")
    metadata: dict[str, Any] | None = None


class ApprovalDecision(ApexBaseModel):
    """Request model for making an approval decision."""

    status: ApprovalStatus
    comment: str | None = None
    approver_id: str = Field(alias="approverId")


class Approval(TimestampedModel):
    """Approval model."""

    id: str
    task_id: str = Field(alias="taskId")
    status: ApprovalStatus
    type: ApprovalType
    description: str | None = None
    required_approvers: list[str] = Field(default_factory=list, alias="requiredApprovers")
    approvers: list[str] = Field(default_factory=list)
    expires_at: datetime | None = Field(default=None, alias="expiresAt")
    decided_at: datetime | None = Field(default=None, alias="decidedAt")
    comment: str | None = None
    metadata: dict[str, Any] = Field(default_factory=dict)


# Pagination Models


class PaginationParams(ApexBaseModel):
    """Pagination parameters."""

    page: int = Field(default=1, ge=1)
    per_page: int = Field(default=20, ge=1, le=100, alias="perPage")
    sort_by: str | None = Field(default=None, alias="sortBy")
    sort_order: str | None = Field(default="asc", alias="sortOrder")


class PaginatedResponse(ApexBaseModel):
    """Paginated response wrapper."""

    items: list[Any]
    total: int
    page: int
    per_page: int = Field(alias="perPage")
    total_pages: int = Field(alias="totalPages")


class TaskList(ApexBaseModel):
    """Paginated list of tasks."""

    items: list[Task]
    total: int
    page: int
    per_page: int = Field(alias="perPage")
    total_pages: int = Field(alias="totalPages")


class AgentList(ApexBaseModel):
    """Paginated list of agents."""

    items: list[Agent]
    total: int
    page: int
    per_page: int = Field(alias="perPage")
    total_pages: int = Field(alias="totalPages")


class DAGList(ApexBaseModel):
    """Paginated list of DAGs."""

    items: list[DAG]
    total: int
    page: int
    per_page: int = Field(alias="perPage")
    total_pages: int = Field(alias="totalPages")


class ApprovalList(ApexBaseModel):
    """Paginated list of approvals."""

    items: list[Approval]
    total: int
    page: int
    per_page: int = Field(alias="perPage")
    total_pages: int = Field(alias="totalPages")


# WebSocket Models


class WebSocketMessage(ApexBaseModel):
    """WebSocket message model."""

    type: WebSocketEventType
    timestamp: datetime
    data: dict[str, Any]


class WebSocketSubscription(ApexBaseModel):
    """WebSocket subscription request."""

    events: list[WebSocketEventType]
    task_ids: list[str] | None = Field(default=None, alias="taskIds")
    agent_ids: list[str] | None = Field(default=None, alias="agentIds")
    dag_ids: list[str] | None = Field(default=None, alias="dagIds")


# Health Check Models


class HealthStatus(ApexBaseModel):
    """Health check status."""

    status: str
    version: str
    uptime: float
    services: dict[str, str] = Field(default_factory=dict)
