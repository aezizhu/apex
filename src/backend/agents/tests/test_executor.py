"""Tests for the AgentExecutor class."""

import asyncio
import json
from datetime import datetime, timezone
from unittest.mock import AsyncMock, MagicMock, patch

import pytest

from apex_agents.agent import AgentConfig
from apex_agents.config import Settings
from apex_agents.executor import (
    AgentExecutor,
    BackendClient,
    QueuedTask,
    TaskQueue,
    TaskResult,
    TaskStatus,
)


@pytest.fixture
def mock_settings():
    """Create mock settings."""
    with patch.dict(
        "os.environ",
        {
            "APEX_LLM_OPENAI_API_KEY": "sk-test",
            "APEX_REDIS_URL": "redis://localhost:6379",
            "APEX_BACKEND_HOST": "localhost",
            "APEX_BACKEND_HTTP_PORT": "8080",
        },
    ):
        return Settings()


@pytest.fixture
def queued_task():
    """Create a sample queued task."""
    return QueuedTask(
        id="task-123",
        name="test-task",
        instruction="Test the system",
        context={"key": "value"},
        parameters={"param1": "value1"},
        priority=5,
        max_retries=3,
        retry_count=0,
        trace_id="abc123",
        span_id="def456",
    )


@pytest.fixture
def task_result():
    """Create a sample task result."""
    return TaskResult(
        task_id="task-123",
        status=TaskStatus.COMPLETED,
        result="Task completed successfully",
        data={"output": "value"},
        tokens_used=100,
        cost_dollars=0.01,
        duration_ms=1500,
    )


class TestQueuedTask:
    """Tests for QueuedTask."""

    def test_from_json_minimal(self):
        """Test creating task from minimal JSON."""
        data = {
            "id": "task-1",
            "name": "minimal-task",
        }

        task = QueuedTask.from_json(data)

        assert task.id == "task-1"
        assert task.name == "minimal-task"
        assert task.instruction == ""
        assert task.context == {}
        assert task.priority == 0
        assert task.agent_config is None

    def test_from_json_full(self):
        """Test creating task from full JSON."""
        data = {
            "id": "task-2",
            "name": "full-task",
            "instruction": "Do something",
            "context": {"key": "value"},
            "parameters": {"p1": "v1"},
            "priority": 10,
            "max_retries": 5,
            "retry_count": 2,
            "trace_id": "trace-123",
            "span_id": "span-456",
            "agent_config": {
                "name": "custom-agent",
                "model": "gpt-4o",
                "system_prompt": "You are helpful",
                "tools": ["tool1"],
                "max_iterations": 5,
                "temperature": 0.5,
            },
        }

        task = QueuedTask.from_json(data)

        assert task.id == "task-2"
        assert task.instruction == "Do something"
        assert task.context["key"] == "value"
        assert task.priority == 10
        assert task.retry_count == 2
        assert task.agent_config is not None
        assert task.agent_config.name == "custom-agent"


class TestTaskResult:
    """Tests for TaskResult."""

    def test_to_json(self, task_result):
        """Test converting result to JSON."""
        json_data = task_result.to_json()

        assert json_data["task_id"] == "task-123"
        assert json_data["status"] == "completed"
        assert json_data["result"] == "Task completed successfully"
        assert json_data["tokens_used"] == 100
        assert json_data["cost_dollars"] == 0.01
        assert json_data["duration_ms"] == 1500

    def test_to_json_with_error(self):
        """Test converting failed result to JSON."""
        result = TaskResult(
            task_id="task-456",
            status=TaskStatus.FAILED,
            error="Something went wrong",
        )

        json_data = result.to_json()

        assert json_data["status"] == "failed"
        assert json_data["error"] == "Something went wrong"
        assert json_data["result"] is None


class TestTaskStatus:
    """Tests for TaskStatus enum."""

    def test_status_values(self):
        """Test status enum values."""
        assert TaskStatus.PENDING.value == "pending"
        assert TaskStatus.READY.value == "ready"
        assert TaskStatus.RUNNING.value == "running"
        assert TaskStatus.COMPLETED.value == "completed"
        assert TaskStatus.FAILED.value == "failed"
        assert TaskStatus.CANCELLED.value == "cancelled"


class TestBackendClient:
    """Tests for BackendClient."""

    @pytest.fixture
    def backend_client(self, mock_settings):
        """Create a backend client."""
        return BackendClient(mock_settings)

    @pytest.mark.asyncio
    async def test_health_check_success(self, backend_client):
        """Test successful health check."""
        with patch.object(
            backend_client,
            "_request",
            new_callable=AsyncMock,
            return_value={"status": "healthy"},
        ):
            result = await backend_client.health_check()
            assert result is True

    @pytest.mark.asyncio
    async def test_health_check_failure(self, backend_client):
        """Test failed health check."""
        with patch.object(
            backend_client,
            "_request",
            new_callable=AsyncMock,
            side_effect=Exception("Connection refused"),
        ):
            result = await backend_client.health_check()
            assert result is False

    @pytest.mark.asyncio
    async def test_report_task_result(self, backend_client, task_result):
        """Test reporting task result."""
        with patch.object(
            backend_client,
            "_request",
            new_callable=AsyncMock,
            return_value={"success": True},
        ) as mock_request:
            await backend_client.report_task_result(task_result)

            mock_request.assert_called_once()
            call_args = mock_request.call_args
            assert call_args[0][0] == "POST"
            assert f"/api/v1/tasks/{task_result.task_id}/complete" in call_args[0][1]

    @pytest.mark.asyncio
    async def test_get_task(self, backend_client):
        """Test getting task details."""
        task_data = {
            "id": "task-123",
            "name": "test",
            "status": "running",
        }

        with patch.object(
            backend_client,
            "_request",
            new_callable=AsyncMock,
            return_value={"success": True, "data": task_data},
        ):
            result = await backend_client.get_task("task-123")
            assert result == task_data

    @pytest.mark.asyncio
    async def test_close(self, backend_client):
        """Test closing the client."""
        # Create a mock HTTP client
        mock_http = AsyncMock()
        backend_client._http_client = mock_http

        await backend_client.close()

        mock_http.aclose.assert_called_once()
        assert backend_client._http_client is None


class TestTaskQueue:
    """Tests for TaskQueue."""

    @pytest.fixture
    def task_queue(self, mock_settings):
        """Create a task queue."""
        return TaskQueue(mock_settings)

    @pytest.mark.asyncio
    async def test_pull_task_success(self, task_queue):
        """Test pulling a task from queue."""
        task_data = {
            "id": "task-123",
            "name": "test-task",
            "instruction": "Do something",
        }

        mock_redis = AsyncMock()
        mock_redis.brpop.return_value = ("queue-key", json.dumps(task_data))
        task_queue._redis = mock_redis

        task = await task_queue.pull_task(timeout=1.0)

        assert task is not None
        assert task.id == "task-123"
        assert task.name == "test-task"

    @pytest.mark.asyncio
    async def test_pull_task_empty(self, task_queue):
        """Test pulling from empty queue."""
        mock_redis = AsyncMock()
        mock_redis.brpop.return_value = None
        task_queue._redis = mock_redis

        task = await task_queue.pull_task(timeout=1.0)

        assert task is None

    @pytest.mark.asyncio
    async def test_push_result(self, task_queue, task_result):
        """Test pushing a result to queue."""
        mock_redis = AsyncMock()
        task_queue._redis = mock_redis

        await task_queue.push_result(task_result)

        mock_redis.lpush.assert_called_once()

    @pytest.mark.asyncio
    async def test_requeue_task(self, task_queue, queued_task):
        """Test requeuing a task."""
        mock_redis = AsyncMock()
        task_queue._redis = mock_redis

        original_retry_count = queued_task.retry_count
        await task_queue.requeue_task(queued_task)

        assert queued_task.retry_count == original_retry_count + 1
        mock_redis.lpush.assert_called_once()

    @pytest.mark.asyncio
    async def test_not_connected_error(self, task_queue):
        """Test error when not connected."""
        with pytest.raises(RuntimeError) as exc_info:
            await task_queue.pull_task()

        assert "Not connected to Redis" in str(exc_info.value)


class TestAgentExecutor:
    """Tests for AgentExecutor."""

    @pytest.fixture
    def executor(self, mock_settings):
        """Create an agent executor."""
        with patch("apex_agents.executor.create_default_registry"):
            return AgentExecutor(settings=mock_settings)

    @pytest.mark.asyncio
    async def test_initialize(self, executor):
        """Test executor initialization."""
        with patch.object(
            executor, "_create_agent_pool", new_callable=AsyncMock
        ) as mock_create_pool:
            with patch.object(TaskQueue, "connect", new_callable=AsyncMock):
                await executor.initialize()

                mock_create_pool.assert_called_once()
                assert executor._semaphore is not None
                assert executor._llm_client is not None

    @pytest.mark.asyncio
    async def test_shutdown(self, executor):
        """Test executor shutdown."""
        executor._task_queue = AsyncMock()
        executor._backend_client = AsyncMock()
        executor._running_tasks = {}

        await executor.shutdown()

        executor._task_queue.close.assert_called_once()
        executor._backend_client.close.assert_called_once()

    def test_get_agent_default(self, executor):
        """Test getting default agent."""
        # Create a mock agent
        from apex_agents.agent import Agent

        mock_agent = MagicMock(spec=Agent)
        mock_agent.config = MagicMock()
        mock_agent.config.name = "default"
        executor._agents["default"] = mock_agent

        agent = executor.get_agent()
        assert agent == mock_agent

    def test_get_agent_not_found(self, executor):
        """Test getting non-existent agent."""
        with pytest.raises(KeyError) as exc_info:
            executor.get_agent("non-existent")

        assert "Agent not found" in str(exc_info.value)

    def test_register_agent(self, executor):
        """Test registering an agent."""
        from apex_agents.agent import Agent

        mock_agent = MagicMock(spec=Agent)
        mock_agent.config = MagicMock()
        mock_agent.config.name = "custom-agent"
        mock_agent.config.model = "gpt-4o"

        executor.register_agent(mock_agent)

        assert "custom-agent" in executor._agents
        assert executor._agents["custom-agent"] == mock_agent

    @pytest.mark.asyncio
    async def test_execute_task_success(self, executor, queued_task):
        """Test successful task execution."""
        from apex_agents.agent import Agent, TaskOutput

        # Setup mock agent
        mock_agent = AsyncMock(spec=Agent)
        mock_agent.id = "agent-123"
        mock_agent.config = MagicMock()
        mock_agent.config.name = "test-agent"
        mock_agent.metrics = MagicMock()
        mock_agent.metrics.tokens_used = 100
        mock_agent.metrics.cost_dollars = 0.01

        mock_output = TaskOutput(
            result="Task completed",
            data={"key": "value"},
        )
        mock_agent.run.return_value = mock_output

        executor._agents["default"] = mock_agent
        executor._backend_client = AsyncMock()
        executor._llm_client = MagicMock()

        with patch("apex_agents.executor.TaskSpanContext"):
            result = await executor.execute_task(queued_task)

        assert result.status == TaskStatus.COMPLETED
        assert result.result == "Task completed"
        assert result.tokens_used == 100

    @pytest.mark.asyncio
    async def test_execute_task_timeout(self, executor, queued_task):
        """Test task execution timeout."""
        from apex_agents.agent import Agent

        mock_agent = AsyncMock(spec=Agent)
        mock_agent.id = "agent-123"
        mock_agent.config = MagicMock()
        mock_agent.config.name = "test-agent"

        # Make agent.run hang
        async def slow_run(*args, **kwargs):
            await asyncio.sleep(10)

        mock_agent.run = slow_run

        executor._agents["default"] = mock_agent
        executor._backend_client = AsyncMock()
        executor._llm_client = MagicMock()
        executor._task_queue = AsyncMock()
        executor.settings.worker.max_task_duration_seconds = 1

        with patch("apex_agents.executor.TaskSpanContext"):
            result = await executor.execute_task(queued_task)

        assert result.status == TaskStatus.FAILED
        assert "timed out" in result.error.lower()

    @pytest.mark.asyncio
    async def test_execute_task_failure_with_retry(self, executor, queued_task):
        """Test task failure with retry."""
        from apex_agents.agent import Agent

        mock_agent = AsyncMock(spec=Agent)
        mock_agent.id = "agent-123"
        mock_agent.config = MagicMock()
        mock_agent.config.name = "test-agent"
        mock_agent.run.side_effect = Exception("Agent error")

        executor._agents["default"] = mock_agent
        executor._backend_client = AsyncMock()
        executor._llm_client = MagicMock()
        executor._task_queue = AsyncMock()

        with patch("apex_agents.executor.TaskSpanContext"):
            result = await executor.execute_task(queued_task)

        assert result.status == TaskStatus.FAILED
        # Task should be requeued
        executor._task_queue.requeue_task.assert_called_once()

    def test_active_task_count(self, executor):
        """Test active task count."""
        mock_task1 = MagicMock()
        mock_task1.done.return_value = False

        mock_task2 = MagicMock()
        mock_task2.done.return_value = True

        mock_task3 = MagicMock()
        mock_task3.done.return_value = False

        executor._running_tasks = {
            "task-1": mock_task1,
            "task-2": mock_task2,
            "task-3": mock_task3,
        }

        assert executor.active_task_count == 2

    def test_registered_agents(self, executor):
        """Test getting registered agent names."""
        executor._agents = {
            "agent-1": MagicMock(),
            "agent-2": MagicMock(),
        }

        agents = executor.registered_agents

        assert "agent-1" in agents
        assert "agent-2" in agents
        assert len(agents) == 2
