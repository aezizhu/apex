"""
Apex SDK - Python client for the Project Apex API.

This SDK provides both synchronous and asynchronous clients for interacting
with the Apex API, including support for WebSocket streaming.

Example usage:

    # Synchronous client
    from apex_sdk import ApexClient, TaskCreate

    client = ApexClient("https://api.apex.example.com", api_key="your-api-key")
    task = client.create_task(TaskCreate(name="My Task"))
    print(task.id)

    # Asynchronous client
    from apex_sdk import AsyncApexClient

    async with AsyncApexClient("https://api.apex.example.com", api_key="your-api-key") as client:
        task = await client.create_task(TaskCreate(name="My Task"))
        print(task.id)

    # WebSocket streaming
    async with AsyncApexClient("https://api.apex.example.com", api_key="your-api-key") as client:
        ws = client.websocket()
        await ws.subscribe(events=[WebSocketEventType.TASK_COMPLETED])
        async for message in ws.listen():
            print(f"Event: {message.type}")
"""

from .client import ApexClient, AsyncApexClient
from .exceptions import (
    ApexAPIError,
    ApexAuthenticationError,
    ApexAuthorizationError,
    ApexConnectionError,
    ApexError,
    ApexNotFoundError,
    ApexRateLimitError,
    ApexServerError,
    ApexTimeoutError,
    ApexValidationError,
    ApexWebSocketClosed,
    ApexWebSocketError,
)
from .models import (
    Agent,
    AgentCapability,
    AgentCreate,
    AgentList,
    AgentStatus,
    AgentUpdate,
    Approval,
    ApprovalCreate,
    ApprovalDecision,
    ApprovalList,
    ApprovalStatus,
    ApprovalType,
    DAG,
    DAGCreate,
    DAGEdge,
    DAGList,
    DAGNode,
    DAGStatus,
    DAGTaskStatus,
    DAGUpdate,
    HealthStatus,
    PaginatedResponse,
    PaginationParams,
    Task,
    TaskCreate,
    TaskError,
    TaskInput,
    TaskList,
    TaskOutput,
    TaskPriority,
    TaskStatus,
    TaskUpdate,
    WebSocketEventType,
    WebSocketMessage,
    WebSocketSubscription,
)
from .websocket import ApexWebSocketClient

__version__ = "0.1.0"

__all__ = [
    # Version
    "__version__",
    # Clients
    "ApexClient",
    "AsyncApexClient",
    "ApexWebSocketClient",
    # Exceptions
    "ApexError",
    "ApexAPIError",
    "ApexAuthenticationError",
    "ApexAuthorizationError",
    "ApexNotFoundError",
    "ApexValidationError",
    "ApexRateLimitError",
    "ApexServerError",
    "ApexConnectionError",
    "ApexTimeoutError",
    "ApexWebSocketError",
    "ApexWebSocketClosed",
    # Task Models
    "Task",
    "TaskCreate",
    "TaskUpdate",
    "TaskInput",
    "TaskOutput",
    "TaskError",
    "TaskList",
    "TaskStatus",
    "TaskPriority",
    # Agent Models
    "Agent",
    "AgentCreate",
    "AgentUpdate",
    "AgentCapability",
    "AgentList",
    "AgentStatus",
    # DAG Models
    "DAG",
    "DAGCreate",
    "DAGUpdate",
    "DAGNode",
    "DAGEdge",
    "DAGTaskStatus",
    "DAGList",
    "DAGStatus",
    # Approval Models
    "Approval",
    "ApprovalCreate",
    "ApprovalDecision",
    "ApprovalList",
    "ApprovalStatus",
    "ApprovalType",
    # WebSocket Models
    "WebSocketEventType",
    "WebSocketMessage",
    "WebSocketSubscription",
    # Utility Models
    "HealthStatus",
    "PaginatedResponse",
    "PaginationParams",
]
