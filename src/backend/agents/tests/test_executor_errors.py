"""Tests for executor error handling paths."""

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
def executor(mock_settings):
    """Create an agent executor."""
    with patch("apex_agents.executor.create_default_registry"):
        return AgentExecutor(settings=mock_settings)


class TestAgentExecutorErrorPaths:
    """Tests for executor error handling paths."""

    @pytest.mark.asyncio
    async def test_pull_and_execute_not_initialized(self, executor):
        """Test pull_and_execute raises when executor not initialized."""
        executor._task_queue = None

        with pytest.raises(RuntimeError):
            await executor.pull_and_execute()

    @pytest.mark.asyncio
    async def test_pull_and_execute_no_task(self, executor):
        """Test pull_and_execute returns None when no task is available."""
        mock_queue = AsyncMock()
        mock_queue.pull_task.return_value = None
        executor._task_queue = mock_queue
        executor._semaphore = asyncio.Semaphore(5)

        result = await executor.pull_and_execute()

        assert result is None

    @pytest.mark.asyncio
    async def test_execute_task_with_custom_agent_config(self, executor):
        """Test that task with agent_config creates a new agent."""
        from apex_agents.agent import Agent, TaskOutput

        custom_config = AgentConfig(
            name="custom-agent",
            model="gpt-4o",
            system_prompt="Custom prompt",
            tools=[],
            max_iterations=3,
        )

        task = QueuedTask(
            id="task-custom",
            name="custom-task",
            instruction="Do custom work",
            agent_config=custom_config,
        )

        mock_llm = MagicMock()
        executor._llm_client = mock_llm
        executor._backend_client = AsyncMock()

        mock_output = TaskOutput(result="Custom result", data={})

        with patch("apex_agents.executor.Agent") as mock_agent_cls:
            mock_agent = AsyncMock()
            mock_agent.id = "custom-id"
            mock_agent.config = custom_config
            mock_agent.metrics = MagicMock()
            mock_agent.metrics.tokens_used = 50
            mock_agent.metrics.cost_dollars = 0.005
            mock_agent.run.return_value = mock_output
            mock_agent_cls.return_value = mock_agent

            with patch("apex_agents.executor.TaskSpanContext"):
                result = await executor.execute_task(task)

        assert result.status == TaskStatus.COMPLETED
        assert result.result == "Custom result"

    @pytest.mark.asyncio
    async def test_handle_task_failure_no_retry_when_exhausted(self, executor):
        """Test that task is not requeued when retries are exhausted."""
        task = QueuedTask(
            id="task-exhausted",
            name="exhausted-task",
            instruction="Failing task",
            max_retries=3,
            retry_count=3,
        )

        executor._task_queue = AsyncMock()

        result = await executor._handle_task_failure(
            task, "fatal error", datetime.now(timezone.utc)
        )

        assert result.status == TaskStatus.FAILED
        assert result.error == "fatal error"
        executor._task_queue.requeue_task.assert_not_called()

    @pytest.mark.asyncio
    async def test_handle_task_failure_requeues_when_retries_remain(self, executor):
        """Test that task is requeued when retries remain."""
        task = QueuedTask(
            id="task-retry",
            name="retry-task",
            instruction="Retryable task",
            max_retries=3,
            retry_count=1,
        )

        executor._task_queue = AsyncMock()

        result = await executor._handle_task_failure(
            task, "transient error", datetime.now(timezone.utc)
        )

        assert result.status == TaskStatus.FAILED
        executor._task_queue.requeue_task.assert_called_once_with(task)

    @pytest.mark.asyncio
    async def test_report_result(self, executor):
        """Test reporting a result pushes to queue and backend."""
        executor._task_queue = AsyncMock()
        executor._backend_client = AsyncMock()

        result = TaskResult(
            task_id="task-report",
            status=TaskStatus.COMPLETED,
            result="Done",
        )

        await executor.report_result(result)

        executor._task_queue.push_result.assert_called_once_with(result)
        executor._backend_client.report_task_result.assert_called_once_with(result)

    @pytest.mark.asyncio
    async def test_report_result_no_queue(self, executor):
        """Test reporting result when queue is None."""
        executor._task_queue = None
        executor._backend_client = AsyncMock()

        result = TaskResult(
            task_id="task-no-queue",
            status=TaskStatus.COMPLETED,
            result="Done",
        )

        await executor.report_result(result)

        executor._backend_client.report_task_result.assert_called_once()

    @pytest.mark.asyncio
    async def test_shutdown_cancels_running_tasks(self, executor):
        """Test that shutdown cancels running tasks."""
        mock_task = MagicMock()
        mock_task.done.return_value = False
        mock_task.cancel = MagicMock()

        executor._running_tasks = {"task-1": mock_task}
        executor._task_queue = AsyncMock()
        executor._backend_client = AsyncMock()
        executor.settings.worker.graceful_shutdown_timeout_seconds = 1

        with patch("asyncio.gather", new_callable=AsyncMock):
            await executor.shutdown()

        mock_task.cancel.assert_called_once()


class TestTaskQueueErrorHandling:
    """Tests for TaskQueue error handling."""

    @pytest.fixture
    def task_queue(self, mock_settings):
        """Create a task queue."""
        return TaskQueue(mock_settings)

    @pytest.mark.asyncio
    async def test_pull_task_error_returns_none(self, task_queue):
        """Test that pull_task returns None on Redis error."""
        mock_redis = AsyncMock()
        mock_redis.brpop.side_effect = Exception("Redis connection lost")
        task_queue._redis = mock_redis

        result = await task_queue.pull_task()

        assert result is None

    @pytest.mark.asyncio
    async def test_push_result_not_connected(self, task_queue):
        """Test push_result raises when not connected."""
        task_queue._redis = None

        result = TaskResult(
            task_id="task-1",
            status=TaskStatus.COMPLETED,
            result="Done",
        )

        with pytest.raises(RuntimeError):
            await task_queue.push_result(result)

    @pytest.mark.asyncio
    async def test_requeue_task_not_connected(self, task_queue):
        """Test requeue_task raises when not connected."""
        task_queue._redis = None

        task = QueuedTask(
            id="task-1",
            name="test",
            instruction="test",
        )

        with pytest.raises(RuntimeError):
            await task_queue.requeue_task(task)

    @pytest.mark.asyncio
    async def test_close_when_not_connected(self, task_queue):
        """Test close when not connected is a no-op."""
        task_queue._redis = None
        await task_queue.close()


class TestBackendClientErrorPaths:
    """Tests for BackendClient error handling paths."""

    @pytest.fixture
    def backend_client(self, mock_settings):
        """Create a backend client."""
        return BackendClient(mock_settings)

    @pytest.mark.asyncio
    async def test_report_task_started_handles_error(self, backend_client):
        """Test that report_task_started does not raise on error."""
        with patch.object(
            backend_client,
            "_request",
            new_callable=AsyncMock,
            side_effect=Exception("Network error"),
        ):
            await backend_client.report_task_started("task-1", "agent-1")

    @pytest.mark.asyncio
    async def test_report_task_result_handles_error(self, backend_client):
        """Test that report_task_result does not raise on error."""
        with patch.object(
            backend_client,
            "_request",
            new_callable=AsyncMock,
            side_effect=Exception("Network error"),
        ):
            result = TaskResult(
                task_id="task-1",
                status=TaskStatus.COMPLETED,
                result="Done",
            )
            await backend_client.report_task_result(result)

    @pytest.mark.asyncio
    async def test_get_task_not_found(self, backend_client):
        """Test get_task returns None on 404."""
        import httpx

        mock_response = MagicMock()
        mock_response.status_code = 404
        error = httpx.HTTPStatusError(
            "Not Found",
            request=MagicMock(),
            response=mock_response,
        )

        with patch.object(
            backend_client,
            "_request",
            new_callable=AsyncMock,
            side_effect=error,
        ):
            result = await backend_client.get_task("nonexistent-task")
            assert result is None

    @pytest.mark.asyncio
    async def test_get_task_returns_none_on_failure(self, backend_client):
        """Test get_task returns None when success is False."""
        with patch.object(
            backend_client,
            "_request",
            new_callable=AsyncMock,
            return_value={"success": False},
        ):
            result = await backend_client.get_task("task-1")
            assert result is None

    @pytest.mark.asyncio
    async def test_get_http_client_lazy_creation(self, backend_client):
        """Test that _get_http_client creates client lazily."""
        assert backend_client._http_client is None

        client = await backend_client._get_http_client()

        assert client is not None
        assert backend_client._http_client is not None

        client2 = await backend_client._get_http_client()
        assert client is client2

        await backend_client.close()
