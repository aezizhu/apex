# Apex SDK for Python

A Python SDK for the Project Apex API, providing both synchronous and asynchronous clients with full support for REST endpoints, WebSocket streaming, and Pydantic validation.

## Installation

```bash
pip install apex-sdk
```

Or install from source:

```bash
pip install -e .
```

## Features

- **Sync and Async Clients**: Choose between `ApexClient` (synchronous) or `AsyncApexClient` (asynchronous)
- **Full API Coverage**: Tasks, Agents, DAGs, and Approvals
- **WebSocket Streaming**: Real-time updates with async generators
- **Pydantic Models**: Type-safe request/response validation
- **Automatic Retries**: Built-in retry logic with exponential backoff
- **Comprehensive Error Handling**: Typed exceptions for all error cases

## Quick Start

### Synchronous Client

```python
from apex_sdk import ApexClient, TaskCreate, TaskPriority

# Initialize the client
client = ApexClient(
    base_url="https://api.apex.example.com",
    api_key="your-api-key"
)

# Create a task
task = client.create_task(
    TaskCreate(
        name="Process Data",
        description="Process the input dataset",
        priority=TaskPriority.HIGH,
    )
)
print(f"Created task: {task.id}")

# List tasks
tasks = client.list_tasks(status="pending", page=1, per_page=10)
for t in tasks.items:
    print(f"Task {t.id}: {t.name} ({t.status})")

# Get task details
task = client.get_task("task-123")
print(f"Task status: {task.status}")

# Close the client when done
client.close()
```

### Asynchronous Client

```python
import asyncio
from apex_sdk import AsyncApexClient, TaskCreate

async def main():
    async with AsyncApexClient(
        base_url="https://api.apex.example.com",
        api_key="your-api-key"
    ) as client:
        # Create a task
        task = await client.create_task(
            TaskCreate(name="Async Task")
        )
        print(f"Created task: {task.id}")

        # List all running tasks
        tasks = await client.list_tasks(status="running")
        print(f"Found {tasks.total} running tasks")

asyncio.run(main())
```

## Working with Tasks

```python
from apex_sdk import (
    ApexClient,
    TaskCreate,
    TaskUpdate,
    TaskInput,
    TaskPriority,
)

client = ApexClient("https://api.apex.example.com", api_key="your-key")

# Create a task with input data
task = client.create_task(
    TaskCreate(
        name="Data Processing",
        description="Process customer data",
        priority=TaskPriority.HIGH,
        input=TaskInput(
            data={"customer_id": "12345"},
            parameters={"format": "json"},
        ),
        timeout_seconds=3600,
        retries=3,
        tags=["data", "processing"],
        metadata={"source": "api"},
    )
)

# Update a task
updated_task = client.update_task(
    task.id,
    TaskUpdate(
        priority=TaskPriority.CRITICAL,
        tags=["data", "processing", "urgent"],
    )
)

# Cancel a task
cancelled_task = client.cancel_task(task.id)

# Retry a failed task
retried_task = client.retry_task(task.id)

# Delete a task
client.delete_task(task.id)
```

## Working with Agents

```python
from apex_sdk import (
    ApexClient,
    AgentCreate,
    AgentUpdate,
    AgentCapability,
)

client = ApexClient("https://api.apex.example.com", api_key="your-key")

# Create an agent
agent = client.create_agent(
    AgentCreate(
        name="Python Agent",
        description="Agent for Python code analysis",
        capabilities=[
            AgentCapability(name="code-analysis", version="1.0"),
            AgentCapability(name="testing", version="2.0"),
        ],
        max_concurrent_tasks=5,
        tags=["python", "analysis"],
    )
)

# List agents
agents = client.list_agents(status="idle")
for a in agents.items:
    print(f"Agent {a.id}: {a.name} ({a.status})")

# Update an agent
updated_agent = client.update_agent(
    agent.id,
    AgentUpdate(max_concurrent_tasks=10)
)

# Delete an agent
client.delete_agent(agent.id)
```

## Working with DAGs

```python
from apex_sdk import (
    ApexClient,
    DAGCreate,
    DAGNode,
    DAGEdge,
    TaskCreate,
)

client = ApexClient("https://api.apex.example.com", api_key="your-key")

# Create a DAG with multiple nodes
dag = client.create_dag(
    DAGCreate(
        name="Data Pipeline",
        description="ETL pipeline for data processing",
        nodes=[
            DAGNode(
                id="extract",
                taskTemplate=TaskCreate(name="Extract Data"),
                dependsOn=[],
            ),
            DAGNode(
                id="transform",
                taskTemplate=TaskCreate(name="Transform Data"),
                dependsOn=["extract"],
            ),
            DAGNode(
                id="load",
                taskTemplate=TaskCreate(name="Load Data"),
                dependsOn=["transform"],
            ),
        ],
        edges=[
            DAGEdge(source="extract", target="transform"),
            DAGEdge(source="transform", target="load"),
        ],
    )
)

# Start a DAG
running_dag = client.start_dag(
    dag.id,
    input_data={"source": "s3://bucket/data"}
)

# Pause a DAG
paused_dag = client.pause_dag(dag.id)

# Resume a DAG
resumed_dag = client.resume_dag(dag.id)

# Cancel a DAG
cancelled_dag = client.cancel_dag(dag.id)
```

## Working with Approvals

```python
from apex_sdk import (
    ApexClient,
    ApprovalCreate,
    ApprovalDecision,
    ApprovalType,
    ApprovalStatus,
)

client = ApexClient("https://api.apex.example.com", api_key="your-key")

# Create an approval request
approval = client.create_approval(
    ApprovalCreate(
        taskId="task-123",
        type=ApprovalType.MANUAL,
        description="Please review and approve this deployment",
        requiredApprovers=["user-1", "user-2"],
    )
)

# List pending approvals
approvals = client.list_approvals(status="pending")
for a in approvals.items:
    print(f"Approval {a.id}: {a.description}")

# Approve or reject
decision = client.decide_approval(
    approval.id,
    ApprovalDecision(
        status=ApprovalStatus.APPROVED,
        approverId="user-1",
        comment="Looks good, approved!",
    )
)
```

## WebSocket Streaming

```python
import asyncio
from apex_sdk import AsyncApexClient, WebSocketEventType

async def stream_events():
    async with AsyncApexClient(
        base_url="https://api.apex.example.com",
        api_key="your-api-key"
    ) as client:
        ws = client.websocket()

        # Subscribe to specific events
        await ws.subscribe(
            events=[
                WebSocketEventType.TASK_COMPLETED,
                WebSocketEventType.TASK_FAILED,
            ],
            task_ids=["task-123", "task-456"],
        )

        # Listen for events
        async for message in ws.listen():
            print(f"Event: {message.type}")
            print(f"Data: {message.data}")

            if message.type == WebSocketEventType.TASK_COMPLETED:
                print("Task completed!")
                break

asyncio.run(stream_events())
```

### Using Event Handlers

```python
import asyncio
from apex_sdk import AsyncApexClient, WebSocketEventType, WebSocketMessage

async def main():
    async with AsyncApexClient(
        base_url="https://api.apex.example.com",
        api_key="your-api-key"
    ) as client:
        ws = client.websocket()

        # Register event handlers using decorators
        @ws.on_event(WebSocketEventType.TASK_COMPLETED)
        async def handle_completed(message: WebSocketMessage):
            print(f"Task completed: {message.data}")

        @ws.on_event(WebSocketEventType.TASK_FAILED)
        async def handle_failed(message: WebSocketMessage):
            print(f"Task failed: {message.data}")

        # Or register handlers programmatically
        def handle_approval(message: WebSocketMessage):
            print(f"Approval required: {message.data}")

        ws.add_event_handler(
            WebSocketEventType.APPROVAL_REQUIRED,
            handle_approval
        )

        # Run the WebSocket client
        await ws.run()

asyncio.run(main())
```

## Error Handling

The SDK provides typed exceptions for different error scenarios:

```python
from apex_sdk import (
    ApexClient,
    ApexError,
    ApexAPIError,
    ApexAuthenticationError,
    ApexAuthorizationError,
    ApexNotFoundError,
    ApexValidationError,
    ApexRateLimitError,
    ApexServerError,
    ApexConnectionError,
    ApexTimeoutError,
)

client = ApexClient("https://api.apex.example.com", api_key="your-key")

try:
    task = client.get_task("nonexistent-task")
except ApexNotFoundError as e:
    print(f"Task not found: {e.message}")
except ApexAuthenticationError as e:
    print(f"Authentication failed: {e.message}")
except ApexRateLimitError as e:
    print(f"Rate limited. Retry after: {e.retry_after} seconds")
except ApexValidationError as e:
    print(f"Validation error: {e.response_body}")
except ApexServerError as e:
    print(f"Server error ({e.status_code}): {e.message}")
except ApexConnectionError as e:
    print(f"Connection failed: {e.message}")
except ApexTimeoutError as e:
    print(f"Request timed out: {e.message}")
except ApexAPIError as e:
    print(f"API error ({e.status_code}): {e.message}")
except ApexError as e:
    print(f"General error: {e.message}")
```

## Configuration

### Client Options

```python
from apex_sdk import ApexClient, AsyncApexClient

# Synchronous client with custom configuration
client = ApexClient(
    base_url="https://api.apex.example.com",
    api_key="your-api-key",        # API key authentication
    # OR
    token="bearer-token",           # Bearer token authentication
    timeout=60.0,                   # Request timeout in seconds
    max_retries=5,                  # Maximum retry attempts
    retry_delay=2.0,                # Initial retry delay
)

# Async client with same options
async_client = AsyncApexClient(
    base_url="https://api.apex.example.com",
    api_key="your-api-key",
    timeout=60.0,
)
```

### WebSocket Options

```python
from apex_sdk import ApexWebSocketClient

ws = ApexWebSocketClient(
    base_url="https://api.apex.example.com",
    api_key="your-api-key",
    reconnect=True,              # Auto-reconnect on disconnect
    reconnect_delay=1.0,         # Initial reconnect delay
    max_reconnect_delay=60.0,    # Maximum reconnect delay
    ping_interval=30.0,          # Ping interval
    ping_timeout=10.0,           # Ping timeout
)
```

## Development

### Setup

```bash
# Clone the repository
git clone https://github.com/aezi/apex-sdk.git
cd apex-sdk

# Install development dependencies
pip install -e ".[dev]"
```

### Running Tests

```bash
# Run all tests
pytest

# Run with coverage
pytest --cov=apex_sdk

# Run specific tests
pytest tests/test_client.py -k "test_create_task"
```

### Code Quality

```bash
# Format code
ruff format .

# Lint code
ruff check .

# Type checking
mypy apex_sdk
```

## API Reference

### Clients

- `ApexClient` - Synchronous HTTP client
- `AsyncApexClient` - Asynchronous HTTP client
- `ApexWebSocketClient` - WebSocket client for streaming

### Models

#### Tasks
- `Task` - Task resource
- `TaskCreate` - Create task request
- `TaskUpdate` - Update task request
- `TaskInput` - Task input data
- `TaskOutput` - Task output data
- `TaskError` - Task error information
- `TaskStatus` - Task status enum
- `TaskPriority` - Task priority enum

#### Agents
- `Agent` - Agent resource
- `AgentCreate` - Create agent request
- `AgentUpdate` - Update agent request
- `AgentCapability` - Agent capability
- `AgentStatus` - Agent status enum

#### DAGs
- `DAG` - DAG resource
- `DAGCreate` - Create DAG request
- `DAGUpdate` - Update DAG request
- `DAGNode` - DAG node definition
- `DAGEdge` - DAG edge definition
- `DAGStatus` - DAG status enum

#### Approvals
- `Approval` - Approval resource
- `ApprovalCreate` - Create approval request
- `ApprovalDecision` - Approval decision
- `ApprovalStatus` - Approval status enum
- `ApprovalType` - Approval type enum

### Exceptions

- `ApexError` - Base exception
- `ApexAPIError` - API error response
- `ApexAuthenticationError` - 401 Unauthorized
- `ApexAuthorizationError` - 403 Forbidden
- `ApexNotFoundError` - 404 Not Found
- `ApexValidationError` - 422 Validation Error
- `ApexRateLimitError` - 429 Rate Limited
- `ApexServerError` - 5xx Server Error
- `ApexConnectionError` - Connection failed
- `ApexTimeoutError` - Request timeout
- `ApexWebSocketError` - WebSocket error
- `ApexWebSocketClosed` - WebSocket closed

## License

MIT License - see LICENSE file for details.
