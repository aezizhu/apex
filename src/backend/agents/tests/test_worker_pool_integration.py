"""Integration tests for WorkerPool and worker error handling paths."""

import asyncio
from datetime import datetime, timezone
from unittest.mock import AsyncMock, MagicMock, patch

import pytest

from apex_agents.config import Settings
from apex_agents.executor import TaskResult, TaskStatus
from apex_agents.worker import Worker, WorkerPool, WorkerState, run_worker_pool


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


class TestWorkerPoolIntegration:
    """Integration tests for WorkerPool lifecycle."""

    @pytest.fixture
    def pool(self, mock_settings):
        """Create a worker pool."""
        return WorkerPool(num_workers=3, settings=mock_settings)

    @pytest.mark.asyncio
    async def test_pool_creates_correct_number_of_workers(self, pool):
        """Test that start creates the exact number of requested workers."""
        with patch.object(Worker, "start", new_callable=AsyncMock):
            await pool.start()

            assert len(pool._workers) == 3
            assert len(pool._tasks) == 3

            ids = [w.worker_id for w in pool._workers]
            assert len(set(ids)) == 3

    @pytest.mark.asyncio
    async def test_pool_worker_ids_contain_index(self, pool):
        """Test that worker IDs contain their index number."""
        with patch.object(Worker, "start", new_callable=AsyncMock):
            await pool.start()

            for i, worker in enumerate(pool._workers):
                assert f"worker-{i}" in worker.worker_id

    @pytest.mark.asyncio
    async def test_pool_stop_handles_worker_errors(self, pool):
        """Test that pool.stop handles errors from individual workers gracefully."""
        mock_worker1 = MagicMock()
        mock_worker1.stop = AsyncMock()

        mock_worker2 = MagicMock()
        mock_worker2.stop = AsyncMock(side_effect=RuntimeError("Worker 2 failed"))

        mock_worker3 = MagicMock()
        mock_worker3.stop = AsyncMock()

        pool._workers = [mock_worker1, mock_worker2, mock_worker3]
        pool._tasks = [asyncio.create_task(asyncio.sleep(10)) for _ in range(3)]

        await pool.stop()

        mock_worker1.stop.assert_called_once()
        mock_worker2.stop.assert_called_once()
        mock_worker3.stop.assert_called_once()

    @pytest.mark.asyncio
    async def test_pool_wait_completes(self, pool):
        """Test that wait completes when all tasks are done."""
        pool._tasks = [asyncio.create_task(asyncio.sleep(0.01)) for _ in range(3)]
        await pool.wait()
        for task in pool._tasks:
            assert task.done()

    @pytest.mark.asyncio
    async def test_pool_stats_aggregation(self, pool):
        """Test that pool stats aggregates stats from all workers."""
        workers = []
        for i in range(3):
            w = MagicMock()
            w.stats = {
                "worker_id": f"w{i}",
                "state": "running",
                "tasks_processed": (i + 1) * 5,
                "tasks_failed": i,
            }
            workers.append(w)

        pool._workers = workers
        stats = pool.stats

        assert len(stats) == 3
        total_processed = sum(s["tasks_processed"] for s in stats)
        assert total_processed == 5 + 10 + 15

    @pytest.mark.asyncio
    async def test_pool_empty_wait(self, pool):
        """Test that wait on empty pool completes immediately."""
        pool._tasks = []
        await pool.wait()


class TestWorkerErrorPaths:
    """Tests for worker error handling paths."""

    @pytest.fixture
    def worker(self, mock_settings):
        """Create a worker instance."""
        return Worker(settings=mock_settings, worker_id="error-test-worker")

    @pytest.mark.asyncio
    async def test_send_heartbeat_no_redis(self, worker):
        """Test that send_heartbeat is a no-op when redis is None."""
        worker._redis = None
        await worker._send_heartbeat()

    @pytest.mark.asyncio
    async def test_run_loop_cancelled_error(self, worker):
        """Test that run_loop handles CancelledError cleanly."""
        mock_executor = AsyncMock()
        mock_executor.pull_and_execute.side_effect = asyncio.CancelledError()

        worker._executor = mock_executor
        worker._state = WorkerState.RUNNING

        await worker._run_loop()

    @pytest.mark.asyncio
    async def test_stop_with_main_task_timeout(self, worker):
        """Test stop handles main task timeout gracefully."""
        worker._state = WorkerState.RUNNING

        async def never_finish():
            await asyncio.sleep(100)

        worker._main_task = asyncio.create_task(never_finish())
        worker._executor = AsyncMock()
        worker._redis = AsyncMock()
        worker._heartbeat_task = None

        worker.settings.worker.graceful_shutdown_timeout_seconds = 1

        with patch("apex_agents.worker.shutdown_tracing"):
            await worker.stop(timeout=0.1)

        assert worker.state == WorkerState.STOPPED

    def test_stats_with_started_at(self, worker):
        """Test stats calculation when worker has been running."""
        worker._started_at = datetime(2024, 1, 1, tzinfo=timezone.utc)
        worker._state = WorkerState.RUNNING
        worker._tasks_processed = 42
        worker._tasks_failed = 3

        stats = worker.stats

        assert stats["tasks_processed"] == 42
        assert stats["tasks_failed"] == 3
        assert stats["uptime_seconds"] > 0


class TestRunWorkerPool:
    """Tests for run_worker_pool function."""

    @pytest.mark.asyncio
    async def test_run_worker_pool_starts_and_waits(self, mock_settings):
        """Test run_worker_pool function."""
        with patch("apex_agents.worker.WorkerPool") as mock_pool_cls:
            mock_pool = AsyncMock()
            mock_pool_cls.return_value = mock_pool

            await run_worker_pool(num_workers=2, settings=mock_settings)

            mock_pool.start.assert_called_once()
            mock_pool.wait.assert_called_once()
            mock_pool.stop.assert_called_once()

    @pytest.mark.asyncio
    async def test_run_worker_pool_handles_keyboard_interrupt(self, mock_settings):
        """Test run_worker_pool handles keyboard interrupt."""
        with patch("apex_agents.worker.WorkerPool") as mock_pool_cls:
            mock_pool = AsyncMock()
            mock_pool.start.side_effect = KeyboardInterrupt()
            mock_pool_cls.return_value = mock_pool

            await run_worker_pool(num_workers=2, settings=mock_settings)

            mock_pool.stop.assert_called_once()
