"""Unit tests for the Apex SDK client."""

from datetime import datetime, timezone
from unittest.mock import AsyncMock, MagicMock, patch

import httpx
import pytest

from apex_sdk import (
    Agent,
    AgentCreate,
    AgentStatus,
    ApexClient,
    ApexNotFoundError,
    ApexRateLimitError,
    ApexServerError,
    ApexValidationError,
    Approval,
    ApprovalCreate,
    ApprovalDecision,
    ApprovalStatus,
    ApprovalType,
    AsyncApexClient,
    DAG,
    DAGCreate,
    DAGNode,
    DAGStatus,
    Task,
    TaskCreate,
    TaskPriority,
    TaskStatus,
)


# Fixtures


@pytest.fixture
def base_url():
    return "https://api.apex.example.com"


@pytest.fixture
def api_key():
    return "test-api-key"


@pytest.fixture
def mock_task_response():
    return {
        "id": "task-123",
        "name": "Test Task",
        "description": "A test task",
        "status": "pending",
        "priority": "normal",
        "agentId": None,
        "input": {"data": {"key": "value"}},
        "output": None,
        "error": None,
        "timeoutSeconds": 3600,
        "retries": 3,
        "retryCount": 0,
        "tags": ["test"],
        "metadata": {},
        "dagId": None,
        "dependsOn": [],
        "createdAt": "2024-01-15T10:00:00Z",
        "updatedAt": "2024-01-15T10:00:00Z",
        "startedAt": None,
        "completedAt": None,
    }


@pytest.fixture
def mock_agent_response():
    return {
        "id": "agent-456",
        "name": "Test Agent",
        "description": "A test agent",
        "status": "idle",
        "capabilities": [{"name": "code-analysis", "version": "1.0"}],
        "maxConcurrentTasks": 5,
        "currentTasks": 0,
        "totalTasksCompleted": 100,
        "tags": ["python"],
        "metadata": {},
        "lastHeartbeat": "2024-01-15T10:00:00Z",
        "createdAt": "2024-01-15T09:00:00Z",
        "updatedAt": "2024-01-15T10:00:00Z",
    }


@pytest.fixture
def mock_dag_response():
    return {
        "id": "dag-789",
        "name": "Test DAG",
        "description": "A test DAG",
        "status": "pending",
        "nodes": [
            {
                "id": "node-1",
                "taskTemplate": {"name": "Task 1"},
                "dependsOn": [],
            }
        ],
        "edges": [],
        "input": {},
        "output": {},
        "tags": [],
        "metadata": {},
        "schedule": None,
        "taskStatuses": [],
        "createdAt": "2024-01-15T09:00:00Z",
        "updatedAt": "2024-01-15T10:00:00Z",
        "startedAt": None,
        "completedAt": None,
    }


@pytest.fixture
def mock_approval_response():
    return {
        "id": "approval-101",
        "taskId": "task-123",
        "status": "pending",
        "type": "manual",
        "description": "Please approve this task",
        "requiredApprovers": ["user-1"],
        "approvers": [],
        "expiresAt": "2024-01-16T10:00:00Z",
        "decidedAt": None,
        "comment": None,
        "metadata": {},
        "createdAt": "2024-01-15T10:00:00Z",
        "updatedAt": "2024-01-15T10:00:00Z",
    }


@pytest.fixture
def mock_task_list_response(mock_task_response):
    return {
        "items": [mock_task_response],
        "total": 1,
        "page": 1,
        "perPage": 20,
        "totalPages": 1,
    }


# Synchronous Client Tests


class TestApexClient:
    """Tests for the synchronous ApexClient."""

    def test_client_initialization(self, base_url, api_key):
        """Test client initialization with API key."""
        client = ApexClient(base_url, api_key=api_key)
        assert client.base_url == base_url
        assert client.api_key == api_key
        assert client.timeout == 30.0
        client.close()

    def test_client_initialization_with_token(self, base_url):
        """Test client initialization with bearer token."""
        token = "test-bearer-token"
        client = ApexClient(base_url, token=token)
        assert client.token == token
        headers = client._get_headers()
        assert headers["Authorization"] == f"Bearer {token}"
        client.close()

    def test_client_context_manager(self, base_url, api_key):
        """Test client as context manager."""
        with ApexClient(base_url, api_key=api_key) as client:
            assert client is not None

    @patch.object(httpx.Client, "request")
    def test_list_tasks(self, mock_request, base_url, api_key, mock_task_list_response):
        """Test listing tasks."""
        mock_response = MagicMock()
        mock_response.status_code = 200
        mock_response.json.return_value = mock_task_list_response
        mock_request.return_value = mock_response

        with ApexClient(base_url, api_key=api_key) as client:
            result = client.list_tasks()

        assert len(result.items) == 1
        assert result.items[0].id == "task-123"
        assert result.total == 1

    @patch.object(httpx.Client, "request")
    def test_get_task(self, mock_request, base_url, api_key, mock_task_response):
        """Test getting a task by ID."""
        mock_response = MagicMock()
        mock_response.status_code = 200
        mock_response.json.return_value = mock_task_response
        mock_request.return_value = mock_response

        with ApexClient(base_url, api_key=api_key) as client:
            task = client.get_task("task-123")

        assert task.id == "task-123"
        assert task.name == "Test Task"
        assert task.status == TaskStatus.PENDING

    @patch.object(httpx.Client, "request")
    def test_create_task(self, mock_request, base_url, api_key, mock_task_response):
        """Test creating a task."""
        mock_response = MagicMock()
        mock_response.status_code = 201
        mock_response.json.return_value = mock_task_response
        mock_request.return_value = mock_response

        with ApexClient(base_url, api_key=api_key) as client:
            task = client.create_task(
                TaskCreate(
                    name="Test Task",
                    description="A test task",
                    priority=TaskPriority.HIGH,
                )
            )

        assert task.id == "task-123"
        assert isinstance(task, Task)

    @patch.object(httpx.Client, "request")
    def test_create_agent(self, mock_request, base_url, api_key, mock_agent_response):
        """Test creating an agent."""
        mock_response = MagicMock()
        mock_response.status_code = 201
        mock_response.json.return_value = mock_agent_response
        mock_request.return_value = mock_response

        with ApexClient(base_url, api_key=api_key) as client:
            agent = client.create_agent(
                AgentCreate(
                    name="Test Agent",
                    description="A test agent",
                )
            )

        assert agent.id == "agent-456"
        assert agent.status == AgentStatus.IDLE

    @patch.object(httpx.Client, "request")
    def test_create_dag(self, mock_request, base_url, api_key, mock_dag_response):
        """Test creating a DAG."""
        mock_response = MagicMock()
        mock_response.status_code = 201
        mock_response.json.return_value = mock_dag_response
        mock_request.return_value = mock_response

        with ApexClient(base_url, api_key=api_key) as client:
            dag = client.create_dag(
                DAGCreate(
                    name="Test DAG",
                    nodes=[
                        DAGNode(
                            id="node-1",
                            taskTemplate=TaskCreate(name="Task 1"),
                        )
                    ],
                )
            )

        assert dag.id == "dag-789"
        assert dag.status == DAGStatus.PENDING

    @patch.object(httpx.Client, "request")
    def test_error_handling_404(self, mock_request, base_url, api_key):
        """Test 404 error handling."""
        mock_response = MagicMock()
        mock_response.status_code = 404
        mock_response.json.return_value = {"message": "Task not found"}
        mock_request.return_value = mock_response

        with ApexClient(base_url, api_key=api_key) as client:
            with pytest.raises(ApexNotFoundError) as exc_info:
                client.get_task("nonexistent")

        assert exc_info.value.status_code == 404

    @patch.object(httpx.Client, "request")
    def test_error_handling_422(self, mock_request, base_url, api_key):
        """Test validation error handling."""
        mock_response = MagicMock()
        mock_response.status_code = 422
        mock_response.json.return_value = {
            "message": "Validation failed",
            "details": {"name": "required"},
        }
        mock_request.return_value = mock_response

        with ApexClient(base_url, api_key=api_key) as client:
            with pytest.raises(ApexValidationError) as exc_info:
                client.create_task(TaskCreate(name=""))

        assert exc_info.value.status_code == 422

    @patch.object(httpx.Client, "request")
    def test_error_handling_429(self, mock_request, base_url, api_key):
        """Test rate limit error handling."""
        mock_response = MagicMock()
        mock_response.status_code = 429
        mock_response.headers = {"Retry-After": "60"}
        mock_response.json.return_value = {"message": "Rate limit exceeded"}
        mock_request.return_value = mock_response

        with ApexClient(base_url, api_key=api_key) as client:
            with pytest.raises(ApexRateLimitError) as exc_info:
                client.list_tasks()

        assert exc_info.value.status_code == 429
        assert exc_info.value.retry_after == 60


# Asynchronous Client Tests


class TestAsyncApexClient:
    """Tests for the asynchronous AsyncApexClient."""

    @pytest.mark.asyncio
    async def test_async_client_initialization(self, base_url, api_key):
        """Test async client initialization."""
        async with AsyncApexClient(base_url, api_key=api_key) as client:
            assert client.base_url == base_url
            assert client.api_key == api_key

    @pytest.mark.asyncio
    @patch.object(httpx.AsyncClient, "request")
    async def test_async_list_tasks(
        self, mock_request, base_url, api_key, mock_task_list_response
    ):
        """Test async listing tasks."""
        mock_response = MagicMock()
        mock_response.status_code = 200
        mock_response.json.return_value = mock_task_list_response
        mock_request.return_value = mock_response

        async with AsyncApexClient(base_url, api_key=api_key) as client:
            result = await client.list_tasks()

        assert len(result.items) == 1
        assert result.items[0].id == "task-123"

    @pytest.mark.asyncio
    @patch.object(httpx.AsyncClient, "request")
    async def test_async_get_task(
        self, mock_request, base_url, api_key, mock_task_response
    ):
        """Test async getting a task."""
        mock_response = MagicMock()
        mock_response.status_code = 200
        mock_response.json.return_value = mock_task_response
        mock_request.return_value = mock_response

        async with AsyncApexClient(base_url, api_key=api_key) as client:
            task = await client.get_task("task-123")

        assert task.id == "task-123"

    @pytest.mark.asyncio
    @patch.object(httpx.AsyncClient, "request")
    async def test_async_create_task(
        self, mock_request, base_url, api_key, mock_task_response
    ):
        """Test async creating a task."""
        mock_response = MagicMock()
        mock_response.status_code = 201
        mock_response.json.return_value = mock_task_response
        mock_request.return_value = mock_response

        async with AsyncApexClient(base_url, api_key=api_key) as client:
            task = await client.create_task(
                TaskCreate(name="Test Task", priority=TaskPriority.CRITICAL)
            )

        assert task.id == "task-123"

    @pytest.mark.asyncio
    @patch.object(httpx.AsyncClient, "request")
    async def test_async_cancel_task(
        self, mock_request, base_url, api_key, mock_task_response
    ):
        """Test async cancelling a task."""
        cancelled_response = mock_task_response.copy()
        cancelled_response["status"] = "cancelled"

        mock_response = MagicMock()
        mock_response.status_code = 200
        mock_response.json.return_value = cancelled_response
        mock_request.return_value = mock_response

        async with AsyncApexClient(base_url, api_key=api_key) as client:
            task = await client.cancel_task("task-123")

        assert task.status == TaskStatus.CANCELLED

    @pytest.mark.asyncio
    @patch.object(httpx.AsyncClient, "request")
    async def test_async_start_dag(
        self, mock_request, base_url, api_key, mock_dag_response
    ):
        """Test async starting a DAG."""
        running_response = mock_dag_response.copy()
        running_response["status"] = "running"

        mock_response = MagicMock()
        mock_response.status_code = 200
        mock_response.json.return_value = running_response
        mock_request.return_value = mock_response

        async with AsyncApexClient(base_url, api_key=api_key) as client:
            dag = await client.start_dag("dag-789", input_data={"param": "value"})

        assert dag.status == DAGStatus.RUNNING

    @pytest.mark.asyncio
    @patch.object(httpx.AsyncClient, "request")
    async def test_async_create_approval(
        self, mock_request, base_url, api_key, mock_approval_response
    ):
        """Test async creating an approval."""
        mock_response = MagicMock()
        mock_response.status_code = 201
        mock_response.json.return_value = mock_approval_response
        mock_request.return_value = mock_response

        async with AsyncApexClient(base_url, api_key=api_key) as client:
            approval = await client.create_approval(
                ApprovalCreate(
                    taskId="task-123",
                    type=ApprovalType.MANUAL,
                    description="Please approve",
                )
            )

        assert approval.id == "approval-101"
        assert approval.status == ApprovalStatus.PENDING

    @pytest.mark.asyncio
    @patch.object(httpx.AsyncClient, "request")
    async def test_async_decide_approval(
        self, mock_request, base_url, api_key, mock_approval_response
    ):
        """Test async deciding an approval."""
        approved_response = mock_approval_response.copy()
        approved_response["status"] = "approved"
        approved_response["decidedAt"] = "2024-01-15T11:00:00Z"
        approved_response["comment"] = "Looks good"

        mock_response = MagicMock()
        mock_response.status_code = 200
        mock_response.json.return_value = approved_response
        mock_request.return_value = mock_response

        async with AsyncApexClient(base_url, api_key=api_key) as client:
            approval = await client.decide_approval(
                "approval-101",
                ApprovalDecision(
                    status=ApprovalStatus.APPROVED,
                    approverId="user-1",
                    comment="Looks good",
                ),
            )

        assert approval.status == ApprovalStatus.APPROVED

    @pytest.mark.asyncio
    async def test_async_websocket_client(self, base_url, api_key):
        """Test getting WebSocket client from async client."""
        async with AsyncApexClient(base_url, api_key=api_key) as client:
            ws = client.websocket()
            assert ws is not None
            assert ws.api_key == api_key
            assert "ws" in ws.ws_url

    @pytest.mark.asyncio
    @patch.object(httpx.AsyncClient, "request")
    async def test_async_error_handling_500(self, mock_request, base_url, api_key):
        """Test server error handling with retries disabled."""
        mock_response = MagicMock()
        mock_response.status_code = 500
        mock_response.json.return_value = {"message": "Internal server error"}
        mock_request.return_value = mock_response

        # Create client with retry disabled by mocking tenacity
        with patch("apex_sdk.client.retry", lambda **kwargs: lambda f: f):
            async with AsyncApexClient(base_url, api_key=api_key) as client:
                with pytest.raises(ApexServerError) as exc_info:
                    await client.get_task("task-123")

            assert exc_info.value.status_code == 500


# Model Tests


class TestModels:
    """Tests for Pydantic models."""

    def test_task_create_validation(self):
        """Test TaskCreate validation."""
        task = TaskCreate(
            name="Valid Task",
            priority=TaskPriority.HIGH,
            retries=5,
        )
        assert task.name == "Valid Task"
        assert task.priority == TaskPriority.HIGH
        assert task.retries == 5

    def test_task_create_invalid_name(self):
        """Test TaskCreate with invalid name."""
        with pytest.raises(ValueError):
            TaskCreate(name="")

    def test_task_create_invalid_retries(self):
        """Test TaskCreate with invalid retries."""
        with pytest.raises(ValueError):
            TaskCreate(name="Task", retries=20)

    def test_agent_create_validation(self):
        """Test AgentCreate validation."""
        agent = AgentCreate(
            name="Valid Agent",
            maxConcurrentTasks=10,
        )
        assert agent.name == "Valid Agent"
        assert agent.max_concurrent_tasks == 10

    def test_dag_node_validation(self):
        """Test DAGNode validation."""
        node = DAGNode(
            id="node-1",
            taskTemplate=TaskCreate(name="Node Task"),
            dependsOn=["node-0"],
        )
        assert node.id == "node-1"
        assert node.depends_on == ["node-0"]

    def test_approval_decision_validation(self):
        """Test ApprovalDecision validation."""
        decision = ApprovalDecision(
            status=ApprovalStatus.APPROVED,
            approverId="user-123",
            comment="Approved with comments",
        )
        assert decision.status == ApprovalStatus.APPROVED
        assert decision.approver_id == "user-123"

    def test_task_serialization(self, mock_task_response):
        """Test Task model serialization."""
        task = Task(**mock_task_response)
        serialized = task.model_dump(by_alias=True)
        assert serialized["id"] == "task-123"
        assert serialized["agentId"] is None
        assert "createdAt" in serialized

    def test_agent_serialization(self, mock_agent_response):
        """Test Agent model serialization."""
        agent = Agent(**mock_agent_response)
        serialized = agent.model_dump(by_alias=True)
        assert serialized["id"] == "agent-456"
        assert serialized["maxConcurrentTasks"] == 5


# Pagination Tests


class TestPagination:
    """Tests for pagination functionality."""

    @patch.object(httpx.Client, "request")
    def test_pagination_parameters(self, mock_request, base_url, api_key):
        """Test pagination parameters are passed correctly."""
        mock_response = MagicMock()
        mock_response.status_code = 200
        mock_response.json.return_value = {
            "items": [],
            "total": 0,
            "page": 2,
            "perPage": 50,
            "totalPages": 0,
        }
        mock_request.return_value = mock_response

        with ApexClient(base_url, api_key=api_key) as client:
            client.list_tasks(page=2, per_page=50)

        # Verify pagination params were passed
        call_args = mock_request.call_args
        assert call_args[1]["params"]["page"] == 2
        assert call_args[1]["params"]["perPage"] == 50

    @patch.object(httpx.Client, "request")
    def test_filter_parameters(self, mock_request, base_url, api_key):
        """Test filter parameters are passed correctly."""
        mock_response = MagicMock()
        mock_response.status_code = 200
        mock_response.json.return_value = {
            "items": [],
            "total": 0,
            "page": 1,
            "perPage": 20,
            "totalPages": 0,
        }
        mock_request.return_value = mock_response

        with ApexClient(base_url, api_key=api_key) as client:
            client.list_tasks(status="running", tags=["urgent", "critical"])

        call_args = mock_request.call_args
        assert call_args[1]["params"]["status"] == "running"
        assert call_args[1]["params"]["tags"] == "urgent,critical"
