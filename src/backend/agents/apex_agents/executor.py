"""
Agent executor that manages a pool of agents and coordinates task execution.

The AgentExecutor connects to the Rust backend, pulls tasks from the queue,
executes them using the appropriate agent, and reports results back.
"""

from __future__ import annotations

import asyncio
import json
import uuid
from dataclasses import dataclass, field
from datetime import datetime, timezone
from enum import Enum
from typing import Any

import httpx
import redis.asyncio as redis
import structlog
from opentelemetry import trace
from tenacity import (
    retry,
    retry_if_exception_type,
    stop_after_attempt,
    wait_exponential,
)

from apex_agents.agent import Agent, AgentConfig, TaskInput, TaskOutput
from apex_agents.config import Settings, get_settings
from apex_agents.llm import LLMClient
from apex_agents.tools import ToolRegistry, create_default_registry
from apex_agents.tracing import (
    TaskSpanContext,
    add_span_attributes,
    get_tracer,
    traced_async,
)

logger = structlog.get_logger()


class TaskStatus(str, Enum):
    """Task status matching Rust backend."""

    PENDING = "pending"
    READY = "ready"
    RUNNING = "running"
    COMPLETED = "completed"
    FAILED = "failed"
    CANCELLED = "cancelled"


@dataclass
class QueuedTask:
    """A task pulled from the queue."""

    id: str
    name: str
    instruction: str
    context: dict[str, Any] = field(default_factory=dict)
    parameters: dict[str, Any] = field(default_factory=dict)
    priority: int = 0
    max_retries: int = 3
    retry_count: int = 0
    trace_id: str | None = None
    span_id: str | None = None
    agent_config: AgentConfig | None = None

    @classmethod
    def from_json(cls, data: dict[str, Any]) -> "QueuedTask":
        """Create a QueuedTask from JSON data."""
        agent_config = None
        if "agent_config" in data and data["agent_config"]:
            agent_config = AgentConfig(**data["agent_config"])

        return cls(
            id=data["id"],
            name=data["name"],
            instruction=data.get("instruction", ""),
            context=data.get("context", {}),
            parameters=data.get("parameters", {}),
            priority=data.get("priority", 0),
            max_retries=data.get("max_retries", 3),
            retry_count=data.get("retry_count", 0),
            trace_id=data.get("trace_id"),
            span_id=data.get("span_id"),
            agent_config=agent_config,
        )


@dataclass
class TaskResult:
    """Result of task execution."""

    task_id: str
    status: TaskStatus
    result: str | None = None
    data: dict[str, Any] = field(default_factory=dict)
    error: str | None = None
    tokens_used: int = 0
    cost_dollars: float = 0.0
    duration_ms: int = 0
    trace_id: str | None = None
    span_id: str | None = None

    def to_json(self) -> dict[str, Any]:
        """Convert to JSON-serializable dict."""
        return {
            "task_id": self.task_id,
            "status": self.status.value,
            "result": self.result,
            "data": self.data,
            "error": self.error,
            "tokens_used": self.tokens_used,
            "cost_dollars": self.cost_dollars,
            "duration_ms": self.duration_ms,
            "trace_id": self.trace_id,
            "span_id": self.span_id,
        }


class BackendClient:
    """
    Client for communicating with the Rust backend.

    Supports both REST and gRPC communication modes.
    """

    def __init__(self, settings: Settings):
        """
        Initialize the backend client.

        Args:
            settings: Application settings.
        """
        self.settings = settings
        self._http_client: httpx.AsyncClient | None = None
        self._logger = logger.bind(component="backend_client")

    async def _get_http_client(self) -> httpx.AsyncClient:
        """Get or create the HTTP client."""
        if self._http_client is None:
            self._http_client = httpx.AsyncClient(
                base_url=self.settings.backend.http_base_url,
                timeout=self.settings.backend.timeout_seconds,
            )
        return self._http_client

    async def close(self) -> None:
        """Close the client connections."""
        if self._http_client:
            await self._http_client.aclose()
            self._http_client = None

    @retry(
        stop=stop_after_attempt(3),
        wait=wait_exponential(multiplier=1, min=1, max=10),
        retry=retry_if_exception_type((httpx.HTTPError, httpx.TimeoutException)),
    )
    @traced_async("backend_request")
    async def _request(
        self,
        method: str,
        path: str,
        json_data: dict[str, Any] | None = None,
    ) -> dict[str, Any]:
        """
        Make an HTTP request to the backend.

        Args:
            method: HTTP method.
            path: API path.
            json_data: Optional JSON body.

        Returns:
            Response JSON data.

        Raises:
            httpx.HTTPError: On HTTP errors.
        """
        client = await self._get_http_client()
        response = await client.request(method, path, json=json_data)
        response.raise_for_status()
        return response.json()  # type: ignore[return-value]

    async def report_task_started(self, task_id: str, agent_id: str) -> None:
        """
        Report that a task has started execution.

        Args:
            task_id: The task ID.
            agent_id: The ID of the executing agent.
        """
        try:
            await self._request(
                "POST",
                f"/api/v1/tasks/{task_id}/start",
                json_data={"agent_id": agent_id},
            )
            self._logger.debug("Reported task started", task_id=task_id, agent_id=agent_id)
        except Exception as e:
            self._logger.warning(
                "Failed to report task started",
                task_id=task_id,
                error=str(e),
            )

    async def report_task_result(self, result: TaskResult) -> None:
        """
        Report task execution result to the backend.

        Args:
            result: The task result.
        """
        try:
            await self._request(
                "POST",
                f"/api/v1/tasks/{result.task_id}/complete",
                json_data=result.to_json(),
            )
            self._logger.info(
                "Reported task result",
                task_id=result.task_id,
                status=result.status.value,
                tokens=result.tokens_used,
                cost=result.cost_dollars,
            )
        except Exception as e:
            self._logger.error(
                "Failed to report task result",
                task_id=result.task_id,
                error=str(e),
            )

    async def get_task(self, task_id: str) -> dict[str, Any] | None:
        """
        Get task details from the backend.

        Args:
            task_id: The task ID.

        Returns:
            Task data or None if not found.
        """
        try:
            response = await self._request("GET", f"/api/v1/tasks/{task_id}")
            if response.get("success"):
                return response.get("data")
            return None
        except httpx.HTTPStatusError as e:
            if e.response.status_code == 404:
                return None
            raise

    async def health_check(self) -> bool:
        """
        Check backend health.

        Returns:
            True if backend is healthy.
        """
        try:
            response = await self._request("GET", "/health")
            return response.get("status") == "healthy"
        except Exception:
            return False


class TaskQueue:
    """
    Task queue backed by Redis.

    Uses Redis lists for task queuing with priority support.
    """

    def __init__(self, settings: Settings):
        """
        Initialize the task queue.

        Args:
            settings: Application settings.
        """
        self.settings = settings
        self._redis: redis.Redis | None = None
        self._logger = logger.bind(component="task_queue")

    async def connect(self) -> None:
        """Connect to Redis."""
        self._redis = redis.from_url(  # type: ignore[no-untyped-call]
            self.settings.redis.url,
            encoding="utf-8",
            decode_responses=True,
        )
        self._logger.info("Connected to Redis", url=self.settings.redis.url)

    async def close(self) -> None:
        """Close Redis connection."""
        if self._redis:
            await self._redis.aclose()
            self._redis = None

    async def pull_task(self, timeout: float = 1.0) -> QueuedTask | None:
        """
        Pull a task from the queue.

        Uses BRPOP for blocking pop with timeout.

        Args:
            timeout: Timeout in seconds.

        Returns:
            QueuedTask or None if no task available.
        """
        if not self._redis:
            raise RuntimeError("Not connected to Redis")

        try:
            result = await self._redis.brpop(  # type: ignore[misc,arg-type]
                self.settings.redis.task_queue_key,
                timeout=int(timeout),
            )
            if result:
                _, data = result
                task_data = json.loads(data)
                self._logger.debug("Pulled task from queue", task_id=task_data.get("id"))
                return QueuedTask.from_json(task_data)
            return None
        except Exception as e:
            self._logger.error("Failed to pull task", error=str(e))
            return None

    async def push_result(self, result: TaskResult) -> None:
        """
        Push a task result to the result queue.

        Args:
            result: The task result.
        """
        if not self._redis:
            raise RuntimeError("Not connected to Redis")

        try:
            await self._redis.lpush(  # type: ignore[misc]
                self.settings.redis.result_queue_key,
                json.dumps(result.to_json()),
            )
            self._logger.debug("Pushed result to queue", task_id=result.task_id)
        except Exception as e:
            self._logger.error("Failed to push result", task_id=result.task_id, error=str(e))

    async def requeue_task(self, task: QueuedTask) -> None:
        """
        Requeue a task for retry.

        Args:
            task: The task to requeue.
        """
        if not self._redis:
            raise RuntimeError("Not connected to Redis")

        task.retry_count += 1
        task_data = {
            "id": task.id,
            "name": task.name,
            "instruction": task.instruction,
            "context": task.context,
            "parameters": task.parameters,
            "priority": task.priority,
            "max_retries": task.max_retries,
            "retry_count": task.retry_count,
            "trace_id": task.trace_id,
            "span_id": task.span_id,
        }

        try:
            await self._redis.lpush(  # type: ignore[misc]
                self.settings.redis.task_queue_key,
                json.dumps(task_data),
            )
            self._logger.info(
                "Requeued task for retry",
                task_id=task.id,
                retry_count=task.retry_count,
            )
        except Exception as e:
            self._logger.error("Failed to requeue task", task_id=task.id, error=str(e))


class AgentExecutor:
    """
    Manages a pool of agents and coordinates task execution.

    The executor:
    - Maintains a pool of agent instances
    - Pulls tasks from the queue
    - Routes tasks to appropriate agents
    - Handles execution with tracing
    - Reports results back to the backend
    """

    def __init__(
        self,
        settings: Settings | None = None,
        tool_registry: ToolRegistry | None = None,
    ):
        """
        Initialize the agent executor.

        Args:
            settings: Application settings. Loads from environment if None.
            tool_registry: Tool registry. Creates default if None.
        """
        self.settings = settings or get_settings()
        self.tool_registry = tool_registry or create_default_registry()

        self._llm_client: LLMClient | None = None
        self._backend_client: BackendClient | None = None
        self._task_queue: TaskQueue | None = None
        self._agents: dict[str, Agent] = {}
        self._running_tasks: dict[str, asyncio.Task[Any]] = {}
        self._semaphore: asyncio.Semaphore | None = None

        self._logger = logger.bind(component="agent_executor")
        self._tracer = get_tracer("apex_agents.executor")

    async def initialize(self) -> None:
        """
        Initialize the executor.

        Sets up connections and creates the agent pool.
        """
        self._logger.info("Initializing agent executor")

        # Initialize LLM client
        self._llm_client = LLMClient(
            openai_api_key=self.settings.llm.openai_api_key,
            anthropic_api_key=self.settings.llm.anthropic_api_key,
            timeout=self.settings.llm.timeout_seconds,
        )

        # Initialize backend client
        self._backend_client = BackendClient(self.settings)

        # Initialize task queue
        self._task_queue = TaskQueue(self.settings)
        await self._task_queue.connect()

        # Initialize concurrency semaphore
        self._semaphore = asyncio.Semaphore(self.settings.worker.num_agents)

        # Create default agent pool
        await self._create_agent_pool()

        self._logger.info(
            "Agent executor initialized",
            num_agents=len(self._agents),
            max_concurrent=self.settings.worker.num_agents,
        )

    async def shutdown(self) -> None:
        """
        Shutdown the executor.

        Cancels running tasks and closes connections.
        """
        self._logger.info("Shutting down agent executor")

        # Cancel running tasks
        for task_id, task in self._running_tasks.items():
            if not task.done():
                task.cancel()
                self._logger.warning("Cancelled running task", task_id=task_id)

        # Wait for tasks to complete with timeout
        if self._running_tasks:
            try:
                await asyncio.wait_for(
                    asyncio.gather(*self._running_tasks.values(), return_exceptions=True),
                    timeout=self.settings.worker.graceful_shutdown_timeout_seconds,
                )
            except asyncio.TimeoutError:
                self._logger.warning("Timeout waiting for tasks to complete")

        # Close connections
        if self._task_queue:
            await self._task_queue.close()

        if self._backend_client:
            await self._backend_client.close()

        self._logger.info("Agent executor shutdown complete")

    async def _create_agent_pool(self) -> None:
        """Create the default agent pool."""
        # Create a general-purpose agent
        default_config = AgentConfig(
            name="default",
            model=self.settings.llm.default_model,
            system_prompt=(
                "You are a helpful AI assistant. Complete tasks accurately and efficiently."
            ),
            tools=list(self.tool_registry.list_names()),
            max_iterations=10,
            temperature=0.7,
        )

        assert self._llm_client is not None, "LLM client must be initialized before creating agent pool"
        agent = Agent(
            config=default_config,
            llm_client=self._llm_client,
            tool_registry=self.tool_registry,
        )
        self._agents["default"] = agent

    def get_agent(self, name: str | None = None) -> Agent:
        """
        Get an agent by name.

        Args:
            name: Agent name. Returns default agent if None.

        Returns:
            Agent instance.

        Raises:
            KeyError: If agent not found.
        """
        name = name or "default"
        if name not in self._agents:
            raise KeyError(f"Agent not found: {name}")
        return self._agents[name]

    def register_agent(self, agent: Agent) -> None:
        """
        Register an agent with the executor.

        Args:
            agent: Agent to register.
        """
        self._agents[agent.config.name] = agent
        self._logger.info(
            "Registered agent",
            name=agent.config.name,
            model=agent.config.model,
        )

    async def pull_and_execute(self) -> TaskResult | None:
        """
        Pull a task from the queue and execute it.

        Returns:
            TaskResult or None if no task available.
        """
        if not self._task_queue:
            raise RuntimeError("Executor not initialized")

        # Pull task from queue
        task = await self._task_queue.pull_task(
            timeout=self.settings.worker.poll_interval_seconds
        )
        if not task:
            return None

        # Execute with concurrency limit
        assert self._semaphore is not None, "Executor not initialized"
        async with self._semaphore:
            return await self.execute_task(task)

    async def execute_task(self, task: QueuedTask) -> TaskResult:
        """
        Execute a single task.

        Args:
            task: The task to execute.

        Returns:
            TaskResult with execution outcome.
        """
        start_time = datetime.now(timezone.utc)
        agent = self._get_agent_for_task(task)

        self._logger.info(
            "Starting task execution",
            task_id=task.id,
            task_name=task.name,
            agent=agent.config.name,
        )

        # Report task started
        if self._backend_client:
            await self._backend_client.report_task_started(task.id, str(agent.id))

        # Execute with tracing
        with TaskSpanContext(
            task_id=task.id,
            task_name=task.name,
            agent_name=agent.config.name,
            trace_id=task.trace_id,
            span_id=task.span_id,
        ) as span_ctx:
            try:
                # Build task input
                task_input = TaskInput(
                    instruction=task.instruction,
                    context=task.context,
                    parameters=task.parameters,
                )

                # Run with timeout
                output = await asyncio.wait_for(
                    agent.run(task_input, trace_id=task.trace_id),
                    timeout=self.settings.worker.max_task_duration_seconds,
                )

                # Calculate duration
                end_time = datetime.now(timezone.utc)
                duration_ms = int((end_time - start_time).total_seconds() * 1000)

                # Record metrics
                span_ctx.record_metrics(
                    tokens=agent.metrics.tokens_used,
                    cost=agent.metrics.cost_dollars,
                    duration_ms=duration_ms,
                )

                # Get trace context for result
                trace_ctx = span_ctx.get_trace_context()

                result = TaskResult(
                    task_id=task.id,
                    status=TaskStatus.COMPLETED,
                    result=output.result,
                    data=output.data,
                    tokens_used=agent.metrics.tokens_used,
                    cost_dollars=agent.metrics.cost_dollars,
                    duration_ms=duration_ms,
                    trace_id=trace_ctx.get("traceparent", "").split("-")[1]
                    if trace_ctx.get("traceparent")
                    else None,
                )

                self._logger.info(
                    "Task completed successfully",
                    task_id=task.id,
                    tokens=result.tokens_used,
                    cost=result.cost_dollars,
                    duration_ms=result.duration_ms,
                )

                return result

            except asyncio.TimeoutError:
                self._logger.error(
                    "Task execution timed out",
                    task_id=task.id,
                    timeout=self.settings.worker.max_task_duration_seconds,
                )
                return await self._handle_task_failure(
                    task,
                    f"Task timed out after {self.settings.worker.max_task_duration_seconds} seconds",
                    start_time,
                )

            except Exception as e:
                self._logger.exception(
                    "Task execution failed",
                    task_id=task.id,
                    error=str(e),
                )
                return await self._handle_task_failure(task, str(e), start_time)

    async def _handle_task_failure(
        self,
        task: QueuedTask,
        error: str,
        start_time: datetime,
    ) -> TaskResult:
        """
        Handle task failure with retry logic.

        Args:
            task: The failed task.
            error: Error message.
            start_time: When execution started.

        Returns:
            TaskResult with failure status.
        """
        end_time = datetime.now(timezone.utc)
        duration_ms = int((end_time - start_time).total_seconds() * 1000)

        # Check if task should be retried
        if task.retry_count < task.max_retries:
            self._logger.info(
                "Requeuing task for retry",
                task_id=task.id,
                retry_count=task.retry_count,
                max_retries=task.max_retries,
            )
            if self._task_queue:
                await self._task_queue.requeue_task(task)

        return TaskResult(
            task_id=task.id,
            status=TaskStatus.FAILED,
            error=error,
            duration_ms=duration_ms,
        )

    def _get_agent_for_task(self, task: QueuedTask) -> Agent:
        """
        Get the appropriate agent for a task.

        Args:
            task: The task to execute.

        Returns:
            Agent instance.
        """
        # If task has specific agent config, create a new agent
        if task.agent_config:
            assert self._llm_client is not None, "LLM client must be initialized"
            return Agent(
                config=task.agent_config,
                llm_client=self._llm_client,
                tool_registry=self.tool_registry,
            )

        # Otherwise use default agent
        return self.get_agent("default")

    @property
    def active_task_count(self) -> int:
        """Get the number of active tasks."""
        return len([t for t in self._running_tasks.values() if not t.done()])

    @property
    def registered_agents(self) -> list[str]:
        """Get list of registered agent names."""
        return list(self._agents.keys())

    async def report_result(self, result: TaskResult) -> None:
        """
        Report a task result.

        Args:
            result: The task result to report.
        """
        # Push to result queue
        if self._task_queue:
            await self._task_queue.push_result(result)

        # Report to backend
        if self._backend_client:
            await self._backend_client.report_task_result(result)
