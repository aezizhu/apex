"""
Worker process that runs agents in a loop.

The worker:
- Runs continuously, pulling tasks from the queue
- Handles graceful shutdown on SIGINT/SIGTERM
- Implements heartbeat to the orchestrator
- Manages worker lifecycle
"""

from __future__ import annotations

import asyncio
import json
import signal
import sys
import uuid
from datetime import datetime, timezone
from enum import Enum
from typing import Any

import redis.asyncio as redis
import structlog

from apex_agents.config import Settings, get_settings
from apex_agents.executor import AgentExecutor
from apex_agents.tracing import get_tracer, init_tracing, shutdown_tracing

logger = structlog.get_logger()


class WorkerState(str, Enum):
    """Worker lifecycle state."""

    STARTING = "starting"
    RUNNING = "running"
    DRAINING = "draining"
    STOPPING = "stopping"
    STOPPED = "stopped"


class Worker:
    """
    Worker process that runs agents in a loop.

    The worker manages the lifecycle of agent execution, including:
    - Initializing the executor and connections
    - Running the main task processing loop
    - Sending heartbeats to the orchestrator
    - Handling graceful shutdown
    """

    def __init__(
        self,
        settings: Settings | None = None,
        worker_id: str | None = None,
    ):
        """
        Initialize the worker.

        Args:
            settings: Application settings. Loads from environment if None.
            worker_id: Unique worker identifier. Auto-generated if None.
        """
        self.settings = settings or get_settings()
        self.worker_id = worker_id or self.settings.worker.worker_id or str(uuid.uuid4())

        self._executor: AgentExecutor | None = None
        self._redis: redis.Redis | None = None
        self._state = WorkerState.STOPPED
        self._shutdown_event = asyncio.Event()
        self._heartbeat_task: asyncio.Task[Any] | None = None
        self._main_task: asyncio.Task[Any] | None = None
        self._tasks_processed = 0
        self._tasks_failed = 0
        self._started_at: datetime | None = None

        self._logger = logger.bind(
            component="worker",
            worker_id=self.worker_id,
        )

    @property
    def state(self) -> WorkerState:
        """Get current worker state."""
        return self._state

    @property
    def is_running(self) -> bool:
        """Check if worker is running."""
        return self._state == WorkerState.RUNNING

    @property
    def stats(self) -> dict[str, Any]:
        """Get worker statistics."""
        uptime_seconds = 0
        if self._started_at:
            uptime_seconds = int(
                (datetime.now(timezone.utc) - self._started_at).total_seconds()
            )

        return {
            "worker_id": self.worker_id,
            "state": self._state.value,
            "tasks_processed": self._tasks_processed,
            "tasks_failed": self._tasks_failed,
            "uptime_seconds": uptime_seconds,
            "active_tasks": self._executor.active_task_count if self._executor else 0,
        }

    async def start(self) -> None:
        """
        Start the worker.

        Initializes all components and begins processing tasks.
        """
        if self._state != WorkerState.STOPPED:
            raise RuntimeError(f"Cannot start worker in state: {self._state}")

        self._state = WorkerState.STARTING
        self._logger.info("Starting worker")

        try:
            # Initialize tracing
            init_tracing(self.settings)

            # Connect to Redis for heartbeat
            await self._connect_redis()

            # Initialize executor
            self._executor = AgentExecutor(settings=self.settings)
            await self._executor.initialize()

            # Register signal handlers
            self._setup_signal_handlers()

            # Start heartbeat task
            self._heartbeat_task = asyncio.create_task(self._heartbeat_loop())

            # Mark as running
            self._state = WorkerState.RUNNING
            self._started_at = datetime.now(timezone.utc)

            self._logger.info(
                "Worker started",
                num_agents=self.settings.worker.num_agents,
            )

            # Run main loop
            await self._run_loop()

        except Exception as e:
            self._logger.exception("Failed to start worker", error=str(e))
            self._state = WorkerState.STOPPED
            raise

    async def stop(self, timeout: float | None = None) -> None:
        """
        Stop the worker gracefully.

        Args:
            timeout: Shutdown timeout in seconds. Uses config default if None.
        """
        if self._state == WorkerState.STOPPED:
            return

        timeout = timeout or self.settings.worker.graceful_shutdown_timeout_seconds

        self._logger.info("Stopping worker", timeout=timeout)
        self._state = WorkerState.DRAINING

        # Signal shutdown
        self._shutdown_event.set()

        # Wait for main loop to finish
        if self._main_task and not self._main_task.done():
            try:
                await asyncio.wait_for(self._main_task, timeout=timeout)
            except asyncio.TimeoutError:
                self._logger.warning("Main loop did not stop in time, cancelling")
                self._main_task.cancel()

        self._state = WorkerState.STOPPING

        # Stop heartbeat
        if self._heartbeat_task and not self._heartbeat_task.done():
            self._heartbeat_task.cancel()
            try:
                await self._heartbeat_task
            except asyncio.CancelledError:
                pass

        # Shutdown executor
        if self._executor:
            await self._executor.shutdown()

        # Close Redis
        await self._close_redis()

        # Shutdown tracing
        shutdown_tracing()

        self._state = WorkerState.STOPPED
        self._logger.info(
            "Worker stopped",
            tasks_processed=self._tasks_processed,
            tasks_failed=self._tasks_failed,
        )

    async def _run_loop(self) -> None:
        """Run the main task processing loop."""
        self._logger.info("Starting main processing loop")

        while not self._shutdown_event.is_set():
            try:
                # Pull and execute task
                assert self._executor is not None, "Executor not initialized"
                result = await self._executor.pull_and_execute()

                if result:
                    # Update stats
                    self._tasks_processed += 1
                    if result.status.value == "failed":
                        self._tasks_failed += 1

                    # Report result
                    await self._executor.report_result(result)

            except asyncio.CancelledError:
                self._logger.info("Processing loop cancelled")
                break

            except Exception as e:
                self._logger.exception("Error in processing loop", error=str(e))
                # Add a small delay to avoid tight loop on persistent errors
                await asyncio.sleep(1.0)

        self._logger.info("Processing loop ended")

    async def _heartbeat_loop(self) -> None:
        """Send periodic heartbeats to the orchestrator."""
        self._logger.debug("Starting heartbeat loop")

        while not self._shutdown_event.is_set():
            try:
                await self._send_heartbeat()
                await asyncio.sleep(self.settings.worker.heartbeat_interval_seconds)

            except asyncio.CancelledError:
                break

            except Exception as e:
                self._logger.warning("Failed to send heartbeat", error=str(e))
                await asyncio.sleep(5.0)  # Retry after short delay

        self._logger.debug("Heartbeat loop ended")

    async def _send_heartbeat(self) -> None:
        """Send a heartbeat to Redis."""
        if not self._redis:
            return

        key = f"{self.settings.redis.heartbeat_key_prefix}{self.worker_id}"
        heartbeat_data = json.dumps(
            {
                "worker_id": self.worker_id,
                "state": self._state.value,
                "tasks_processed": self._tasks_processed,
                "tasks_failed": self._tasks_failed,
                "active_tasks": self._executor.active_task_count if self._executor else 0,
                "timestamp": datetime.now(timezone.utc).isoformat(),
            }
        )

        await self._redis.setex(
            key,
            self.settings.redis.heartbeat_ttl_seconds,
            heartbeat_data,
        )

    async def _connect_redis(self) -> None:
        """Connect to Redis."""
        self._redis = redis.from_url(  # type: ignore[no-untyped-call]
            self.settings.redis.url,
            encoding="utf-8",
            decode_responses=True,
        )
        self._logger.debug("Connected to Redis for heartbeat")

    async def _close_redis(self) -> None:
        """Close Redis connection."""
        if self._redis:
            await self._redis.aclose()
            self._redis = None

    def _setup_signal_handlers(self) -> None:
        """Setup signal handlers for graceful shutdown."""
        loop = asyncio.get_event_loop()

        for sig in (signal.SIGINT, signal.SIGTERM):
            loop.add_signal_handler(
                sig,
                lambda s=sig: asyncio.create_task(self._handle_signal(s)),  # type: ignore[misc]
            )

        self._logger.debug("Signal handlers configured")

    async def _handle_signal(self, sig: signal.Signals) -> None:
        """
        Handle shutdown signal.

        Args:
            sig: The signal received.
        """
        self._logger.info("Received shutdown signal", signal=sig.name)
        await self.stop()


class WorkerPool:
    """
    Manages multiple worker processes.

    For running multiple workers in a single process for testing
    or lightweight deployments.
    """

    def __init__(
        self,
        num_workers: int = 1,
        settings: Settings | None = None,
    ):
        """
        Initialize the worker pool.

        Args:
            num_workers: Number of workers to run.
            settings: Shared settings for all workers.
        """
        self.num_workers = num_workers
        self.settings = settings or get_settings()
        self._workers: list[Worker] = []
        self._tasks: list[asyncio.Task[Any]] = []
        self._logger = logger.bind(component="worker_pool")

    async def start(self) -> None:
        """Start all workers in the pool."""
        self._logger.info("Starting worker pool", num_workers=self.num_workers)

        for i in range(self.num_workers):
            worker_id = f"worker-{i}-{uuid.uuid4().hex[:8]}"
            worker = Worker(settings=self.settings, worker_id=worker_id)
            self._workers.append(worker)
            task = asyncio.create_task(worker.start())
            self._tasks.append(task)

        self._logger.info("Worker pool started")

    async def stop(self) -> None:
        """Stop all workers in the pool."""
        self._logger.info("Stopping worker pool")

        # Stop all workers
        stop_tasks = [worker.stop() for worker in self._workers]
        await asyncio.gather(*stop_tasks, return_exceptions=True)

        # Cancel any remaining tasks
        for task in self._tasks:
            if not task.done():
                task.cancel()

        self._logger.info("Worker pool stopped")

    async def wait(self) -> None:
        """Wait for all workers to complete."""
        if self._tasks:
            await asyncio.gather(*self._tasks, return_exceptions=True)

    @property
    def stats(self) -> list[dict[str, Any]]:
        """Get statistics for all workers."""
        return [worker.stats for worker in self._workers]


async def run_worker(settings: Settings | None = None) -> None:
    """
    Run a single worker.

    This is the main entry point for running a worker process.

    Args:
        settings: Application settings. Loads from environment if None.
    """
    worker = Worker(settings=settings)

    try:
        await worker.start()
    except KeyboardInterrupt:
        logger.info("Keyboard interrupt received")
    except Exception as e:
        logger.exception("Worker failed", error=str(e))
        sys.exit(1)
    finally:
        if worker.state != WorkerState.STOPPED:
            await worker.stop()


async def run_worker_pool(
    num_workers: int = 1,
    settings: Settings | None = None,
) -> None:
    """
    Run a pool of workers.

    Args:
        num_workers: Number of workers to run.
        settings: Application settings. Loads from environment if None.
    """
    pool = WorkerPool(num_workers=num_workers, settings=settings)

    try:
        await pool.start()
        await pool.wait()
    except KeyboardInterrupt:
        logger.info("Keyboard interrupt received")
    except Exception as e:
        logger.exception("Worker pool failed", error=str(e))
        sys.exit(1)
    finally:
        await pool.stop()
