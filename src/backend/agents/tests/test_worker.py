"""Tests for the Worker class."""

import asyncio
import json
import signal
from unittest.mock import AsyncMock, MagicMock, patch

import pytest

from apex_agents.config import Settings
from apex_agents.executor import TaskResult, TaskStatus
from apex_agents.worker import Worker, WorkerPool, WorkerState, run_worker


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
            "APEX_WORKER_NUM_AGENTS": "2",
            "APEX_WORKER_POLL_INTERVAL_SECONDS": "0.1",
            "APEX_WORKER_HEARTBEAT_INTERVAL_SECONDS": "1",
        },
    ):
        return Settings()


class TestWorkerState:
    """Tests for WorkerState enum."""

    def test_state_values(self):
        """Test worker state values."""
        assert WorkerState.STARTING.value == "starting"
        assert WorkerState.RUNNING.value == "running"
        assert WorkerState.DRAINING.value == "draining"
        assert WorkerState.STOPPING.value == "stopping"
        assert WorkerState.STOPPED.value == "stopped"


class TestWorker:
    """Tests for Worker class."""

    @pytest.fixture
    def worker(self, mock_settings):
        """Create a worker instance."""
        return Worker(settings=mock_settings, worker_id="test-worker-123")

    def test_worker_creation(self, worker):
        """Test worker is created correctly."""
        assert worker.worker_id == "test-worker-123"
        assert worker.state == WorkerState.STOPPED
        assert worker.is_running is False

    def test_worker_auto_id(self, mock_settings):
        """Test worker generates ID if not provided."""
        worker = Worker(settings=mock_settings)
        assert worker.worker_id is not None
        assert len(worker.worker_id) > 0

    def test_stats(self, worker):
        """Test worker statistics."""
        stats = worker.stats

        assert stats["worker_id"] == "test-worker-123"
        assert stats["state"] == "stopped"
        assert stats["tasks_processed"] == 0
        assert stats["tasks_failed"] == 0
        assert stats["uptime_seconds"] == 0

    @pytest.mark.asyncio
    async def test_start_and_stop(self, worker):
        """Test worker start and stop lifecycle."""
        # Mock dependencies
        with patch.object(worker, "_connect_redis", new_callable=AsyncMock):
            with patch("apex_agents.worker.AgentExecutor") as mock_executor_cls:
                mock_executor = AsyncMock()
                mock_executor.active_task_count = 0
                mock_executor_cls.return_value = mock_executor

                with patch.object(worker, "_run_loop", new_callable=AsyncMock):
                    with patch.object(worker, "_setup_signal_handlers"):
                        with patch("apex_agents.worker.init_tracing"):
                            # Start worker in background
                            start_task = asyncio.create_task(worker.start())

                            # Wait a bit for startup
                            await asyncio.sleep(0.1)

                            # Should be running
                            assert worker.state == WorkerState.RUNNING

                            # Stop worker
                            await worker.stop()

                            # Should be stopped
                            assert worker.state == WorkerState.STOPPED

                            # Clean up
                            try:
                                await asyncio.wait_for(start_task, timeout=1.0)
                            except asyncio.CancelledError:
                                pass

    @pytest.mark.asyncio
    async def test_cannot_start_when_running(self, worker):
        """Test that worker cannot be started when already running."""
        worker._state = WorkerState.RUNNING

        with pytest.raises(RuntimeError) as exc_info:
            await worker.start()

        assert "Cannot start worker" in str(exc_info.value)

    @pytest.mark.asyncio
    async def test_stop_when_already_stopped(self, worker):
        """Test that stopping an already stopped worker is a no-op."""
        assert worker.state == WorkerState.STOPPED

        # Should not raise
        await worker.stop()

        assert worker.state == WorkerState.STOPPED

    @pytest.mark.asyncio
    async def test_heartbeat_loop(self, worker):
        """Test heartbeat loop sends heartbeats."""
        mock_redis = AsyncMock()
        worker._redis = mock_redis
        worker._state = WorkerState.RUNNING
        worker._executor = MagicMock()
        worker._executor.active_task_count = 0

        # Run heartbeat for a short time
        heartbeat_task = asyncio.create_task(worker._heartbeat_loop())

        # Let it run for a bit
        await asyncio.sleep(0.15)

        # Trigger shutdown
        worker._shutdown_event.set()

        # Wait for loop to finish
        await asyncio.wait_for(heartbeat_task, timeout=1.0)

        # Should have sent at least one heartbeat
        assert mock_redis.setex.called

    @pytest.mark.asyncio
    async def test_send_heartbeat(self, worker):
        """Test sending a single heartbeat."""
        mock_redis = AsyncMock()
        worker._redis = mock_redis
        worker._state = WorkerState.RUNNING
        worker._executor = MagicMock()
        worker._executor.active_task_count = 2
        worker._tasks_processed = 10
        worker._tasks_failed = 1

        await worker._send_heartbeat()

        mock_redis.setex.assert_called_once()

        # Verify heartbeat content
        call_args = mock_redis.setex.call_args
        heartbeat_key = call_args[0][0]
        heartbeat_data = json.loads(call_args[0][2])

        assert "apex:workers:heartbeat:" in heartbeat_key
        assert heartbeat_data["worker_id"] == "test-worker-123"
        assert heartbeat_data["state"] == "running"
        assert heartbeat_data["active_tasks"] == 2
        assert heartbeat_data["tasks_processed"] == 10

    @pytest.mark.asyncio
    async def test_run_loop_processes_tasks(self, worker):
        """Test that run loop processes tasks."""
        mock_executor = AsyncMock()
        mock_result = TaskResult(
            task_id="task-1",
            status=TaskStatus.COMPLETED,
            result="Done",
        )

        call_count = 0

        async def pull_and_execute_side_effect():
            nonlocal call_count
            call_count += 1
            if call_count <= 2:
                return mock_result
            # After processing a few tasks, signal shutdown and return None
            worker._shutdown_event.set()
            return None

        mock_executor.pull_and_execute = AsyncMock(side_effect=pull_and_execute_side_effect)
        mock_executor.report_result = AsyncMock()

        worker._executor = mock_executor
        worker._state = WorkerState.RUNNING

        # Run loop
        loop_task = asyncio.create_task(worker._run_loop())
        await asyncio.wait_for(loop_task, timeout=5.0)

        # Should have processed at least one task
        assert worker._tasks_processed >= 1
        assert mock_executor.report_result.called

    @pytest.mark.asyncio
    async def test_run_loop_handles_failures(self, worker):
        """Test that run loop handles task failures."""
        mock_executor = AsyncMock()
        mock_result = TaskResult(
            task_id="task-1",
            status=TaskStatus.FAILED,
            error="Something went wrong",
        )

        call_count = 0

        async def pull_and_execute_side_effect():
            nonlocal call_count
            call_count += 1
            if call_count <= 2:
                return mock_result
            worker._shutdown_event.set()
            return None

        mock_executor.pull_and_execute = AsyncMock(side_effect=pull_and_execute_side_effect)
        mock_executor.report_result = AsyncMock()

        worker._executor = mock_executor
        worker._state = WorkerState.RUNNING

        loop_task = asyncio.create_task(worker._run_loop())
        await asyncio.wait_for(loop_task, timeout=5.0)

        # Should track failed tasks
        assert worker._tasks_failed >= 1

    @pytest.mark.asyncio
    async def test_run_loop_handles_errors(self, worker):
        """Test that run loop handles errors gracefully."""
        mock_executor = AsyncMock()
        mock_executor.pull_and_execute.side_effect = Exception("Error")

        worker._executor = mock_executor
        worker._state = WorkerState.RUNNING

        loop_task = asyncio.create_task(worker._run_loop())

        # Let it handle a few errors
        await asyncio.sleep(0.2)

        # Trigger shutdown
        worker._shutdown_event.set()

        # Should complete without raising
        await asyncio.wait_for(loop_task, timeout=1.0)


class TestWorkerPool:
    """Tests for WorkerPool class."""

    @pytest.fixture
    def pool(self, mock_settings):
        """Create a worker pool."""
        return WorkerPool(num_workers=2, settings=mock_settings)

    def test_pool_creation(self, pool):
        """Test pool is created correctly."""
        assert pool.num_workers == 2
        assert len(pool._workers) == 0
        assert len(pool._tasks) == 0

    @pytest.mark.asyncio
    async def test_start_creates_workers(self, pool):
        """Test that start creates the correct number of workers."""
        with patch.object(Worker, "start", new_callable=AsyncMock) as mock_start:
            await pool.start()

            assert len(pool._workers) == 2
            assert len(pool._tasks) == 2
            assert mock_start.call_count == 2

    @pytest.mark.asyncio
    async def test_stop_stops_all_workers(self, pool):
        """Test that stop stops all workers."""
        # Create mock workers
        mock_worker1 = MagicMock()
        mock_worker1.stop = AsyncMock()

        mock_worker2 = MagicMock()
        mock_worker2.stop = AsyncMock()

        pool._workers = [mock_worker1, mock_worker2]
        pool._tasks = [asyncio.create_task(asyncio.sleep(10)) for _ in range(2)]

        await pool.stop()

        mock_worker1.stop.assert_called_once()
        mock_worker2.stop.assert_called_once()

    def test_stats_returns_all_worker_stats(self, pool):
        """Test that stats returns stats for all workers."""
        mock_worker1 = MagicMock()
        mock_worker1.stats = {"worker_id": "w1", "tasks_processed": 5}

        mock_worker2 = MagicMock()
        mock_worker2.stats = {"worker_id": "w2", "tasks_processed": 10}

        pool._workers = [mock_worker1, mock_worker2]

        stats = pool.stats

        assert len(stats) == 2
        assert stats[0]["worker_id"] == "w1"
        assert stats[1]["worker_id"] == "w2"


class TestRunWorker:
    """Tests for run_worker function."""

    @pytest.mark.asyncio
    async def test_run_worker_starts_and_stops(self, mock_settings):
        """Test run_worker function."""
        with patch("apex_agents.worker.Worker") as mock_worker_cls:
            mock_worker = AsyncMock()
            mock_worker.state = WorkerState.STOPPED
            mock_worker_cls.return_value = mock_worker

            await run_worker(mock_settings)

            mock_worker.start.assert_called_once()

    @pytest.mark.asyncio
    async def test_run_worker_handles_keyboard_interrupt(self, mock_settings):
        """Test run_worker handles keyboard interrupt."""
        with patch("apex_agents.worker.Worker") as mock_worker_cls:
            mock_worker = AsyncMock()
            mock_worker.state = WorkerState.RUNNING
            mock_worker.start.side_effect = KeyboardInterrupt()
            mock_worker_cls.return_value = mock_worker

            # Should not raise
            await run_worker(mock_settings)

            mock_worker.stop.assert_called_once()
