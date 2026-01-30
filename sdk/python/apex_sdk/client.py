"""Sync and async HTTP clients for the Apex API.

This module provides two client implementations for interacting with the
Apex Agent Swarm Orchestration API:

- :class:`ApexClient` -- synchronous client backed by ``httpx.Client``
- :class:`AsyncApexClient` -- asynchronous client backed by ``httpx.AsyncClient``

Both clients share a common base (:class:`BaseApexClient`) that handles
authentication, header management, and error mapping.  Transient errors
(server errors, connection failures, timeouts) are retried automatically
using exponential back-off via the ``tenacity`` library.

Quick start (synchronous)::

    from apex_sdk import ApexClient

    client = ApexClient(base_url="http://localhost:8080", api_key="my-key")
    health = client.health()
    print(health.status)
    client.close()

Quick start (asynchronous)::

    import asyncio
    from apex_sdk import AsyncApexClient

    async def main():
        async with AsyncApexClient(
            base_url="http://localhost:8080", api_key="my-key"
        ) as client:
            health = await client.health()
            print(health.status)

    asyncio.run(main())
"""

from __future__ import annotations

import logging
from typing import Any, TypeVar

import httpx
from pydantic import BaseModel
from tenacity import (
    retry,
    retry_if_exception_type,
    stop_after_attempt,
    wait_exponential,
)

from .exceptions import (
    ApexAPIError,
    ApexAuthenticationError,
    ApexAuthorizationError,
    ApexConnectionError,
    ApexNotFoundError,
    ApexRateLimitError,
    ApexServerError,
    ApexTimeoutError,
    ApexValidationError,
)
from .models import (
    Agent,
    AgentCreate,
    AgentList,
    AgentUpdate,
    Approval,
    ApprovalCreate,
    ApprovalDecision,
    ApprovalList,
    DAG,
    DAGCreate,
    DAGList,
    DAGUpdate,
    HealthStatus,
    Task,
    TaskCreate,
    TaskList,
    TaskUpdate,
)
from .websocket import ApexWebSocketClient

logger = logging.getLogger(__name__)

T = TypeVar("T", bound=BaseModel)


class BaseApexClient:
    """Base class with shared configuration for Apex API clients.

    This class is not intended to be instantiated directly. Use
    :class:`ApexClient` for synchronous access or :class:`AsyncApexClient`
    for ``async``/``await`` workflows.

    The base class centralises:

    * Connection parameters (URL, timeout, retry policy).
    * Authentication via API key (``X-API-Key`` header) **or** bearer token.
    * HTTP error-response mapping to typed SDK exceptions.
    """

    def __init__(
        self,
        base_url: str,
        api_key: str | None = None,
        token: str | None = None,
        timeout: float = 30.0,
        max_retries: int = 3,
        retry_delay: float = 1.0,
    ) -> None:
        """Initialise connection settings shared by sync and async clients.

        Exactly one of *api_key* or *token* should be provided for
        authenticated requests.  If both are supplied, *api_key* takes
        precedence.

        Args:
            base_url: Root URL of the Apex API (e.g. ``http://localhost:8080``).
                A trailing slash is stripped automatically.
            api_key: API key sent in the ``X-API-Key`` header. Takes precedence
                over *token* when both are provided.
            token: Bearer token sent in the ``Authorization`` header. Used only
                when *api_key* is ``None``.
            timeout: Default request timeout in seconds.
            max_retries: Maximum number of automatic retry attempts for
                transient errors (5xx, connection errors, timeouts).
            retry_delay: Initial delay (in seconds) between retries. The delay
                grows exponentially on subsequent attempts.
        """
        self.base_url = base_url.rstrip("/")
        self.api_key = api_key
        self.token = token
        self.timeout = timeout
        self.max_retries = max_retries
        self.retry_delay = retry_delay

    def _get_headers(self) -> dict[str, str]:
        """Build default HTTP headers including authentication.

        Returns:
            A dictionary of headers. Always includes ``Content-Type`` and
            ``Accept`` as ``application/json``. Adds ``X-API-Key`` or
            ``Authorization: Bearer`` depending on the configured credential.
        """
        headers = {"Content-Type": "application/json", "Accept": "application/json"}
        if self.api_key:
            headers["X-API-Key"] = self.api_key
        elif self.token:
            headers["Authorization"] = f"Bearer {self.token}"
        return headers

    def _handle_error_response(self, response: httpx.Response) -> None:
        """Map an HTTP error response to a typed SDK exception.

        The mapping is:

        ====  =============================
        Code  Exception
        ====  =============================
        401   :class:`ApexAuthenticationError`
        403   :class:`ApexAuthorizationError`
        404   :class:`ApexNotFoundError`
        422   :class:`ApexValidationError`
        429   :class:`ApexRateLimitError`
        5xx   :class:`ApexServerError`
        other :class:`ApexAPIError`
        ====  =============================

        Args:
            response: The ``httpx.Response`` with a 4xx/5xx status code.

        Raises:
            ApexAuthenticationError: HTTP 401.
            ApexAuthorizationError: HTTP 403.
            ApexNotFoundError: HTTP 404.
            ApexValidationError: HTTP 422.
            ApexRateLimitError: HTTP 429 (includes ``Retry-After`` if present).
            ApexServerError: HTTP 5xx.
            ApexAPIError: Any other non-success status code.
        """
        status_code = response.status_code
        try:
            body = response.json()
        except Exception:
            body = {"message": response.text}

        message = body.get("message", body.get("error", f"HTTP {status_code}"))

        if status_code == 401:
            raise ApexAuthenticationError(message, body)
        elif status_code == 403:
            raise ApexAuthorizationError(message, body)
        elif status_code == 404:
            raise ApexNotFoundError(message, body)
        elif status_code == 422:
            raise ApexValidationError(message, body)
        elif status_code == 429:
            retry_after = response.headers.get("Retry-After")
            raise ApexRateLimitError(
                message,
                body,
                retry_after=int(retry_after) if retry_after else None,
            )
        elif status_code >= 500:
            raise ApexServerError(message, status_code, body)
        else:
            raise ApexAPIError(message, status_code, body)


class ApexClient(BaseApexClient):
    """Synchronous HTTP client for the Apex Agent Swarm API.

    Wraps ``httpx.Client`` and exposes typed methods for every API resource
    (tasks, agents, DAGs, approvals).  Supports the context-manager protocol
    for deterministic resource cleanup::

        with ApexClient(base_url="http://localhost:8080", api_key="key") as client:
            tasks = client.list_tasks()
            print(tasks.items)

    All public methods return Pydantic model instances parsed from the JSON
    response body.  Transient server/network errors are retried automatically
    (up to *max_retries* times with exponential back-off).

    See :class:`AsyncApexClient` for the ``async``/``await`` variant.
    """

    def __init__(
        self,
        base_url: str,
        api_key: str | None = None,
        token: str | None = None,
        timeout: float = 30.0,
        max_retries: int = 3,
        retry_delay: float = 1.0,
    ) -> None:
        super().__init__(base_url, api_key, token, timeout, max_retries, retry_delay)
        self._client = httpx.Client(
            base_url=self.base_url,
            headers=self._get_headers(),
            timeout=timeout,
        )

    def __enter__(self) -> "ApexClient":
        return self

    def __exit__(self, exc_type: Any, exc_val: Any, exc_tb: Any) -> None:
        self.close()

    def close(self) -> None:
        """Close the underlying HTTP connection pool.

        This is called automatically when the client is used as a context
        manager.  After calling ``close()``, no further requests can be made.
        """
        self._client.close()

    @retry(
        retry=retry_if_exception_type((ApexServerError, ApexConnectionError, ApexTimeoutError)),
        stop=stop_after_attempt(3),
        wait=wait_exponential(multiplier=1, min=1, max=60),
        reraise=True,
    )
    def _request(
        self,
        method: str,
        path: str,
        params: dict[str, Any] | None = None,
        json_data: dict[str, Any] | None = None,
    ) -> dict[str, Any]:
        """Execute an HTTP request with automatic retry on transient errors.

        Args:
            method: HTTP method (``GET``, ``POST``, ``PATCH``, ``DELETE``).
            path: URL path relative to *base_url* (e.g. ``/tasks``).
            params: Optional query-string parameters.
            json_data: Optional JSON request body.

        Returns:
            Parsed JSON response as a dictionary, or ``{}`` for 204 responses.

        Raises:
            ApexConnectionError: Unable to reach the server.
            ApexTimeoutError: Request exceeded the configured timeout.
            ApexAPIError: The server returned a non-success status code.
        """
        try:
            response = self._client.request(
                method,
                path,
                params=params,
                json=json_data,
            )
        except httpx.ConnectError as e:
            raise ApexConnectionError(f"Connection error: {e}") from e
        except httpx.TimeoutException as e:
            raise ApexTimeoutError(f"Request timeout: {e}") from e

        if response.status_code >= 400:
            self._handle_error_response(response)

        if response.status_code == 204:
            return {}

        return response.json()

    def _paginated_request(
        self,
        path: str,
        params: dict[str, Any] | None = None,
        page: int = 1,
        per_page: int = 20,
    ) -> dict[str, Any]:
        """Execute a paginated GET request.

        Args:
            path: URL path relative to *base_url*.
            params: Additional query-string parameters.
            page: Page number (1-indexed).
            per_page: Number of items per page.

        Returns:
            Parsed JSON response containing paginated results.
        """
        request_params = params or {}
        request_params["page"] = page
        request_params["perPage"] = per_page
        return self._request("GET", path, params=request_params)

    # -------------------------------------------------------------------------
    # Health Check
    # -------------------------------------------------------------------------

    def health(self) -> HealthStatus:
        """Check the API health status.

        Returns:
            A :class:`HealthStatus` indicating whether the service and its
            dependencies (database, cache, etc.) are operational.

        Raises:
            ApexConnectionError: If the API server is unreachable.
        """
        data = self._request("GET", "/health")
        return HealthStatus(**data)

    # -------------------------------------------------------------------------
    # Tasks
    # -------------------------------------------------------------------------

    def list_tasks(
        self,
        page: int = 1,
        per_page: int = 20,
        status: str | None = None,
        agent_id: str | None = None,
        dag_id: str | None = None,
        tags: list[str] | None = None,
    ) -> TaskList:
        """List tasks with optional filtering and pagination.

        Args:
            page: Page number (1-indexed).
            per_page: Number of tasks per page (max 100).
            status: Filter by task status (e.g. ``"pending"``, ``"running"``).
            agent_id: Filter by the agent currently assigned to the task.
            dag_id: Filter by the DAG that owns the task.
            tags: Filter by one or more tags (comma-joined in the request).

        Returns:
            A :class:`TaskList` containing the matching tasks and pagination
            metadata.
        """
        params: dict[str, Any] = {}
        if status:
            params["status"] = status
        if agent_id:
            params["agentId"] = agent_id
        if dag_id:
            params["dagId"] = dag_id
        if tags:
            params["tags"] = ",".join(tags)
        data = self._paginated_request("/tasks", params, page, per_page)
        return TaskList(**data)

    def get_task(self, task_id: str) -> Task:
        """Retrieve a single task by its unique identifier.

        Args:
            task_id: UUID of the task.

        Returns:
            The :class:`Task` object.

        Raises:
            ApexNotFoundError: No task exists with the given ID.
        """
        data = self._request("GET", f"/tasks/{task_id}")
        return Task(**data)

    def create_task(self, task: TaskCreate) -> Task:
        """Submit a new task for execution.

        Args:
            task: Task specification including name, description, priority,
                input data, and optional tags.

        Returns:
            The newly created :class:`Task` with a server-assigned ID and
            ``pending`` status.

        Raises:
            ApexValidationError: The request payload failed validation.
        """
        data = self._request(
            "POST",
            "/tasks",
            json_data=task.model_dump(by_alias=True, exclude_none=True),
        )
        return Task(**data)

    def update_task(self, task_id: str, task: TaskUpdate) -> Task:
        """Update mutable fields of an existing task.

        Args:
            task_id: UUID of the task to update.
            task: A :class:`TaskUpdate` with the fields to change.

        Returns:
            The updated :class:`Task`.

        Raises:
            ApexNotFoundError: No task exists with the given ID.
            ApexValidationError: The update payload is invalid.
        """
        data = self._request(
            "PATCH",
            f"/tasks/{task_id}",
            json_data=task.model_dump(by_alias=True, exclude_none=True),
        )
        return Task(**data)

    def delete_task(self, task_id: str) -> None:
        """Permanently delete a task.

        Args:
            task_id: UUID of the task to delete.

        Raises:
            ApexNotFoundError: No task exists with the given ID.
        """
        self._request("DELETE", f"/tasks/{task_id}")

    def cancel_task(self, task_id: str) -> Task:
        """Cancel a running or pending task.

        The task transitions to ``cancelled`` status. Agents that were
        processing the task are notified to stop.

        Args:
            task_id: UUID of the task to cancel.

        Returns:
            The updated :class:`Task` with ``cancelled`` status.

        Raises:
            ApexNotFoundError: No task exists with the given ID.
        """
        data = self._request("POST", f"/tasks/{task_id}/cancel")
        return Task(**data)

    def retry_task(self, task_id: str) -> Task:
        """Retry a failed task.

        Resets the task to ``pending`` status so it can be picked up by an
        agent again.

        Args:
            task_id: UUID of the failed task.

        Returns:
            The :class:`Task` reset to ``pending`` status.

        Raises:
            ApexNotFoundError: No task exists with the given ID.
        """
        data = self._request("POST", f"/tasks/{task_id}/retry")
        return Task(**data)

    # -------------------------------------------------------------------------
    # Agents
    # -------------------------------------------------------------------------

    def list_agents(
        self,
        page: int = 1,
        per_page: int = 20,
        status: str | None = None,
        tags: list[str] | None = None,
    ) -> AgentList:
        """List registered agents with optional filtering.

        Args:
            page: Page number (1-indexed).
            per_page: Number of agents per page (max 100).
            status: Filter by agent status (e.g. ``"idle"``, ``"busy"``).
            tags: Filter by one or more tags.

        Returns:
            An :class:`AgentList` containing matching agents.
        """
        params: dict[str, Any] = {}
        if status:
            params["status"] = status
        if tags:
            params["tags"] = ",".join(tags)
        data = self._paginated_request("/agents", params, page, per_page)
        return AgentList(**data)

    def get_agent(self, agent_id: str) -> Agent:
        """Retrieve a single agent by ID.

        Args:
            agent_id: UUID of the agent.

        Returns:
            The :class:`Agent` object.

        Raises:
            ApexNotFoundError: No agent exists with the given ID.
        """
        data = self._request("GET", f"/agents/{agent_id}")
        return Agent(**data)

    def create_agent(self, agent: AgentCreate) -> Agent:
        """Register a new agent with the orchestrator.

        Args:
            agent: Agent specification including name, capabilities, and
                optional model configuration.

        Returns:
            The newly registered :class:`Agent`.

        Raises:
            ApexValidationError: The request payload failed validation.
        """
        data = self._request(
            "POST",
            "/agents",
            json_data=agent.model_dump(by_alias=True, exclude_none=True),
        )
        return Agent(**data)

    def update_agent(self, agent_id: str, agent: AgentUpdate) -> Agent:
        """Update an existing agent's configuration.

        Args:
            agent_id: UUID of the agent to update.
            agent: An :class:`AgentUpdate` with the fields to change.

        Returns:
            The updated :class:`Agent`.

        Raises:
            ApexNotFoundError: No agent exists with the given ID.
        """
        data = self._request(
            "PATCH",
            f"/agents/{agent_id}",
            json_data=agent.model_dump(by_alias=True, exclude_none=True),
        )
        return Agent(**data)

    def delete_agent(self, agent_id: str) -> None:
        """Remove an agent from the orchestrator.

        Args:
            agent_id: UUID of the agent to delete.

        Raises:
            ApexNotFoundError: No agent exists with the given ID.
        """
        self._request("DELETE", f"/agents/{agent_id}")

    # -------------------------------------------------------------------------
    # DAGs
    # -------------------------------------------------------------------------

    def list_dags(
        self,
        page: int = 1,
        per_page: int = 20,
        status: str | None = None,
        tags: list[str] | None = None,
    ) -> DAGList:
        """List DAG definitions with optional filtering.

        Args:
            page: Page number (1-indexed).
            per_page: Number of DAGs per page (max 100).
            status: Filter by DAG status.
            tags: Filter by one or more tags.

        Returns:
            A :class:`DAGList` containing matching DAGs.
        """
        params: dict[str, Any] = {}
        if status:
            params["status"] = status
        if tags:
            params["tags"] = ",".join(tags)
        data = self._paginated_request("/dags", params, page, per_page)
        return DAGList(**data)

    def get_dag(self, dag_id: str) -> DAG:
        """Retrieve a DAG by ID, including its nodes and edges.

        Args:
            dag_id: UUID of the DAG.

        Returns:
            The :class:`DAG` object.

        Raises:
            ApexNotFoundError: No DAG exists with the given ID.
        """
        data = self._request("GET", f"/dags/{dag_id}")
        return DAG(**data)

    def create_dag(self, dag: DAGCreate) -> DAG:
        """Create a new DAG workflow definition.

        The DAG is stored but not started until :meth:`start_dag` is called.

        Args:
            dag: Full DAG specification with nodes, edges, and optional
                schedule or metadata.

        Returns:
            The persisted :class:`DAG` with a server-assigned ID.

        Raises:
            ApexValidationError: The DAG definition is invalid (e.g. cycles,
                missing node references).
        """
        data = self._request(
            "POST",
            "/dags",
            json_data=dag.model_dump(by_alias=True, exclude_none=True),
        )
        return DAG(**data)

    def update_dag(self, dag_id: str, dag: DAGUpdate) -> DAG:
        """Update a DAG definition.

        Args:
            dag_id: UUID of the DAG to update.
            dag: A :class:`DAGUpdate` with the fields to change.

        Returns:
            The updated :class:`DAG`.

        Raises:
            ApexNotFoundError: No DAG exists with the given ID.
        """
        data = self._request(
            "PATCH",
            f"/dags/{dag_id}",
            json_data=dag.model_dump(by_alias=True, exclude_none=True),
        )
        return DAG(**data)

    def delete_dag(self, dag_id: str) -> None:
        """Delete a DAG and all associated data.

        Args:
            dag_id: UUID of the DAG to delete.

        Raises:
            ApexNotFoundError: No DAG exists with the given ID.
        """
        self._request("DELETE", f"/dags/{dag_id}")

    def start_dag(self, dag_id: str, input_data: dict[str, Any] | None = None) -> DAG:
        """Start executing a DAG.

        Triggers the orchestrator to begin scheduling the DAG's root nodes.
        Downstream nodes execute as their dependencies complete.

        Args:
            dag_id: UUID of the DAG to execute.
            input_data: Optional key-value input passed to the DAG's root
                nodes as initial context.

        Returns:
            The :class:`DAG` in ``running`` status.

        Raises:
            ApexNotFoundError: No DAG exists with the given ID.
        """
        data = self._request(
            "POST",
            f"/dags/{dag_id}/start",
            json_data={"input": input_data} if input_data else None,
        )
        return DAG(**data)

    def cancel_dag(self, dag_id: str) -> DAG:
        """Cancel a running DAG and all of its in-flight tasks.

        Args:
            dag_id: UUID of the DAG.

        Returns:
            The :class:`DAG` in ``cancelled`` status.
        """
        data = self._request("POST", f"/dags/{dag_id}/cancel")
        return DAG(**data)

    def pause_dag(self, dag_id: str) -> DAG:
        """Pause a running DAG.

        Currently executing tasks run to completion but no new nodes are
        scheduled until the DAG is resumed.

        Args:
            dag_id: UUID of the DAG.

        Returns:
            The :class:`DAG` in ``paused`` status.
        """
        data = self._request("POST", f"/dags/{dag_id}/pause")
        return DAG(**data)

    def resume_dag(self, dag_id: str) -> DAG:
        """Resume a previously paused DAG.

        Args:
            dag_id: UUID of the DAG.

        Returns:
            The :class:`DAG` in ``running`` status.
        """
        data = self._request("POST", f"/dags/{dag_id}/resume")
        return DAG(**data)

    # -------------------------------------------------------------------------
    # Approvals
    # -------------------------------------------------------------------------

    def list_approvals(
        self,
        page: int = 1,
        per_page: int = 20,
        status: str | None = None,
        task_id: str | None = None,
    ) -> ApprovalList:
        """List approval requests with optional filtering.

        Args:
            page: Page number (1-indexed).
            per_page: Number of approvals per page.
            status: Filter by approval status (``pending``, ``approved``,
                ``rejected``).
            task_id: Filter approvals linked to a specific task.

        Returns:
            An :class:`ApprovalList` containing matching approvals.
        """
        params: dict[str, Any] = {}
        if status:
            params["status"] = status
        if task_id:
            params["taskId"] = task_id
        data = self._paginated_request("/approvals", params, page, per_page)
        return ApprovalList(**data)

    def get_approval(self, approval_id: str) -> Approval:
        """Retrieve a single approval request.

        Args:
            approval_id: UUID of the approval.

        Returns:
            The :class:`Approval` object.

        Raises:
            ApexNotFoundError: No approval exists with the given ID.
        """
        data = self._request("GET", f"/approvals/{approval_id}")
        return Approval(**data)

    def create_approval(self, approval: ApprovalCreate) -> Approval:
        """Create a new approval request.

        Approval requests gate task execution on human review. The
        associated task remains paused until the approval is decided.

        Args:
            approval: Approval specification including the linked task,
                required approvers, and optional message.

        Returns:
            The created :class:`Approval` in ``pending`` status.

        Raises:
            ApexValidationError: The request payload failed validation.
        """
        data = self._request(
            "POST",
            "/approvals",
            json_data=approval.model_dump(by_alias=True, exclude_none=True),
        )
        return Approval(**data)

    def decide_approval(self, approval_id: str, decision: ApprovalDecision) -> Approval:
        """Approve or reject an approval request.

        On approval the linked task resumes execution; on rejection it
        transitions to ``failed``.

        Args:
            approval_id: UUID of the approval to decide.
            decision: The :class:`ApprovalDecision` containing the verdict
                and optional comment.

        Returns:
            The updated :class:`Approval`.

        Raises:
            ApexNotFoundError: No approval exists with the given ID.
        """
        data = self._request(
            "POST",
            f"/approvals/{approval_id}/decide",
            json_data=decision.model_dump(by_alias=True, exclude_none=True),
        )
        return Approval(**data)


class AsyncApexClient(BaseApexClient):
    """Asynchronous HTTP client for the Apex Agent Swarm API.

    Wraps ``httpx.AsyncClient`` and mirrors the synchronous
    :class:`ApexClient` interface using ``async``/``await``.  Supports the
    async context-manager protocol::

        async with AsyncApexClient(
            base_url="http://localhost:8080",
            api_key="my-key",
        ) as client:
            health = await client.health()
            tasks = await client.list_tasks(status="running")

    Also provides a :meth:`websocket` accessor for obtaining a
    :class:`ApexWebSocketClient` pre-configured with the same credentials.
    """

    def __init__(
        self,
        base_url: str,
        api_key: str | None = None,
        token: str | None = None,
        timeout: float = 30.0,
        max_retries: int = 3,
        retry_delay: float = 1.0,
    ) -> None:
        super().__init__(base_url, api_key, token, timeout, max_retries, retry_delay)
        self._client = httpx.AsyncClient(
            base_url=self.base_url,
            headers=self._get_headers(),
            timeout=timeout,
        )
        self._ws_client: ApexWebSocketClient | None = None

    async def __aenter__(self) -> "AsyncApexClient":
        return self

    async def __aexit__(self, exc_type: Any, exc_val: Any, exc_tb: Any) -> None:
        await self.close()

    async def close(self) -> None:
        """Close the HTTP connection pool and any open WebSocket.

        Called automatically when the client is used as an ``async with``
        context manager.
        """
        await self._client.aclose()
        if self._ws_client:
            await self._ws_client.disconnect()

    @retry(
        retry=retry_if_exception_type((ApexServerError, ApexConnectionError, ApexTimeoutError)),
        stop=stop_after_attempt(3),
        wait=wait_exponential(multiplier=1, min=1, max=60),
        reraise=True,
    )
    async def _request(
        self,
        method: str,
        path: str,
        params: dict[str, Any] | None = None,
        json_data: dict[str, Any] | None = None,
    ) -> dict[str, Any]:
        """Execute an async HTTP request with automatic retry.

        Args:
            method: HTTP method.
            path: URL path relative to *base_url*.
            params: Optional query-string parameters.
            json_data: Optional JSON request body.

        Returns:
            Parsed JSON response as a dictionary.

        Raises:
            ApexConnectionError: Unable to reach the server.
            ApexTimeoutError: Request exceeded the configured timeout.
            ApexAPIError: The server returned a non-success status code.
        """
        try:
            response = await self._client.request(
                method,
                path,
                params=params,
                json=json_data,
            )
        except httpx.ConnectError as e:
            raise ApexConnectionError(f"Connection error: {e}") from e
        except httpx.TimeoutException as e:
            raise ApexTimeoutError(f"Request timeout: {e}") from e

        if response.status_code >= 400:
            self._handle_error_response(response)

        if response.status_code == 204:
            return {}

        return response.json()

    async def _paginated_request(
        self,
        path: str,
        params: dict[str, Any] | None = None,
        page: int = 1,
        per_page: int = 20,
    ) -> dict[str, Any]:
        """Execute a paginated async GET request.

        Args:
            path: URL path relative to *base_url*.
            params: Additional query-string parameters.
            page: Page number (1-indexed).
            per_page: Number of items per page.

        Returns:
            Parsed JSON response containing paginated results.
        """
        request_params = params or {}
        request_params["page"] = page
        request_params["perPage"] = per_page
        return await self._request("GET", path, params=request_params)

    # -------------------------------------------------------------------------
    # WebSocket
    # -------------------------------------------------------------------------

    def websocket(self) -> ApexWebSocketClient:
        """Get or create a WebSocket client for real-time event streaming.

        The WebSocket client is lazily initialised and shares the same
        authentication credentials as this HTTP client.

        Returns:
            A :class:`ApexWebSocketClient` instance.  Call ``connect()``
            on the returned object to open the connection.
        """
        if self._ws_client is None:
            self._ws_client = ApexWebSocketClient(
                base_url=self.base_url,
                api_key=self.api_key,
                token=self.token,
            )
        return self._ws_client

    # -------------------------------------------------------------------------
    # Health Check
    # -------------------------------------------------------------------------

    async def health(self) -> HealthStatus:
        """Check the API health status.

        Returns:
            A :class:`HealthStatus` indicating service health.
        """
        data = await self._request("GET", "/health")
        return HealthStatus(**data)

    # -------------------------------------------------------------------------
    # Tasks
    # -------------------------------------------------------------------------

    async def list_tasks(
        self,
        page: int = 1,
        per_page: int = 20,
        status: str | None = None,
        agent_id: str | None = None,
        dag_id: str | None = None,
        tags: list[str] | None = None,
    ) -> TaskList:
        """List tasks with optional filtering and pagination.

        Args:
            page: Page number (1-indexed).
            per_page: Number of tasks per page.
            status: Filter by task status.
            agent_id: Filter by assigned agent.
            dag_id: Filter by owning DAG.
            tags: Filter by tags.

        Returns:
            A :class:`TaskList` with matching tasks.
        """
        params: dict[str, Any] = {}
        if status:
            params["status"] = status
        if agent_id:
            params["agentId"] = agent_id
        if dag_id:
            params["dagId"] = dag_id
        if tags:
            params["tags"] = ",".join(tags)
        data = await self._paginated_request("/tasks", params, page, per_page)
        return TaskList(**data)

    async def get_task(self, task_id: str) -> Task:
        """Retrieve a single task by ID.

        Args:
            task_id: UUID of the task.

        Returns:
            The :class:`Task` object.

        Raises:
            ApexNotFoundError: No task exists with the given ID.
        """
        data = await self._request("GET", f"/tasks/{task_id}")
        return Task(**data)

    async def create_task(self, task: TaskCreate) -> Task:
        """Submit a new task for execution.

        Args:
            task: Task specification.

        Returns:
            The newly created :class:`Task`.

        Raises:
            ApexValidationError: The request payload failed validation.
        """
        data = await self._request(
            "POST",
            "/tasks",
            json_data=task.model_dump(by_alias=True, exclude_none=True),
        )
        return Task(**data)

    async def update_task(self, task_id: str, task: TaskUpdate) -> Task:
        """Update mutable fields of an existing task.

        Args:
            task_id: UUID of the task.
            task: Fields to update.

        Returns:
            The updated :class:`Task`.

        Raises:
            ApexNotFoundError: No task exists with the given ID.
        """
        data = await self._request(
            "PATCH",
            f"/tasks/{task_id}",
            json_data=task.model_dump(by_alias=True, exclude_none=True),
        )
        return Task(**data)

    async def delete_task(self, task_id: str) -> None:
        """Delete a task.

        Args:
            task_id: UUID of the task.

        Raises:
            ApexNotFoundError: No task exists with the given ID.
        """
        await self._request("DELETE", f"/tasks/{task_id}")

    async def cancel_task(self, task_id: str) -> Task:
        """Cancel a running or pending task.

        Args:
            task_id: UUID of the task.

        Returns:
            The :class:`Task` in ``cancelled`` status.
        """
        data = await self._request("POST", f"/tasks/{task_id}/cancel")
        return Task(**data)

    async def retry_task(self, task_id: str) -> Task:
        """Retry a failed task.

        Args:
            task_id: UUID of the task.

        Returns:
            The :class:`Task` reset to ``pending`` status.
        """
        data = await self._request("POST", f"/tasks/{task_id}/retry")
        return Task(**data)

    # -------------------------------------------------------------------------
    # Agents
    # -------------------------------------------------------------------------

    async def list_agents(
        self,
        page: int = 1,
        per_page: int = 20,
        status: str | None = None,
        tags: list[str] | None = None,
    ) -> AgentList:
        """List agents with optional filtering.

        Args:
            page: Page number (1-indexed).
            per_page: Number of agents per page.
            status: Filter by agent status.
            tags: Filter by tags.

        Returns:
            An :class:`AgentList` with matching agents.
        """
        params: dict[str, Any] = {}
        if status:
            params["status"] = status
        if tags:
            params["tags"] = ",".join(tags)
        data = await self._paginated_request("/agents", params, page, per_page)
        return AgentList(**data)

    async def get_agent(self, agent_id: str) -> Agent:
        """Retrieve a single agent by ID.

        Args:
            agent_id: UUID of the agent.

        Returns:
            The :class:`Agent` object.

        Raises:
            ApexNotFoundError: No agent exists with the given ID.
        """
        data = await self._request("GET", f"/agents/{agent_id}")
        return Agent(**data)

    async def create_agent(self, agent: AgentCreate) -> Agent:
        """Register a new agent.

        Args:
            agent: Agent specification.

        Returns:
            The newly registered :class:`Agent`.
        """
        data = await self._request(
            "POST",
            "/agents",
            json_data=agent.model_dump(by_alias=True, exclude_none=True),
        )
        return Agent(**data)

    async def update_agent(self, agent_id: str, agent: AgentUpdate) -> Agent:
        """Update an agent's configuration.

        Args:
            agent_id: UUID of the agent.
            agent: Fields to update.

        Returns:
            The updated :class:`Agent`.
        """
        data = await self._request(
            "PATCH",
            f"/agents/{agent_id}",
            json_data=agent.model_dump(by_alias=True, exclude_none=True),
        )
        return Agent(**data)

    async def delete_agent(self, agent_id: str) -> None:
        """Remove an agent from the orchestrator.

        Args:
            agent_id: UUID of the agent.

        Raises:
            ApexNotFoundError: No agent exists with the given ID.
        """
        await self._request("DELETE", f"/agents/{agent_id}")

    # -------------------------------------------------------------------------
    # DAGs
    # -------------------------------------------------------------------------

    async def list_dags(
        self,
        page: int = 1,
        per_page: int = 20,
        status: str | None = None,
        tags: list[str] | None = None,
    ) -> DAGList:
        """List DAG definitions with optional filtering.

        Args:
            page: Page number (1-indexed).
            per_page: Number of DAGs per page.
            status: Filter by DAG status.
            tags: Filter by tags.

        Returns:
            A :class:`DAGList` with matching DAGs.
        """
        params: dict[str, Any] = {}
        if status:
            params["status"] = status
        if tags:
            params["tags"] = ",".join(tags)
        data = await self._paginated_request("/dags", params, page, per_page)
        return DAGList(**data)

    async def get_dag(self, dag_id: str) -> DAG:
        """Retrieve a DAG by ID.

        Args:
            dag_id: UUID of the DAG.

        Returns:
            The :class:`DAG` object.

        Raises:
            ApexNotFoundError: No DAG exists with the given ID.
        """
        data = await self._request("GET", f"/dags/{dag_id}")
        return DAG(**data)

    async def create_dag(self, dag: DAGCreate) -> DAG:
        """Create a new DAG workflow definition.

        Args:
            dag: Full DAG specification.

        Returns:
            The persisted :class:`DAG`.

        Raises:
            ApexValidationError: The DAG definition is invalid.
        """
        data = await self._request(
            "POST",
            "/dags",
            json_data=dag.model_dump(by_alias=True, exclude_none=True),
        )
        return DAG(**data)

    async def update_dag(self, dag_id: str, dag: DAGUpdate) -> DAG:
        """Update a DAG definition.

        Args:
            dag_id: UUID of the DAG.
            dag: Fields to update.

        Returns:
            The updated :class:`DAG`.
        """
        data = await self._request(
            "PATCH",
            f"/dags/{dag_id}",
            json_data=dag.model_dump(by_alias=True, exclude_none=True),
        )
        return DAG(**data)

    async def delete_dag(self, dag_id: str) -> None:
        """Delete a DAG and all associated data.

        Args:
            dag_id: UUID of the DAG.

        Raises:
            ApexNotFoundError: No DAG exists with the given ID.
        """
        await self._request("DELETE", f"/dags/{dag_id}")

    async def start_dag(self, dag_id: str, input_data: dict[str, Any] | None = None) -> DAG:
        """Start executing a DAG.

        Args:
            dag_id: UUID of the DAG.
            input_data: Optional initial context for root nodes.

        Returns:
            The :class:`DAG` in ``running`` status.
        """
        data = await self._request(
            "POST",
            f"/dags/{dag_id}/start",
            json_data={"input": input_data} if input_data else None,
        )
        return DAG(**data)

    async def cancel_dag(self, dag_id: str) -> DAG:
        """Cancel a running DAG and all in-flight tasks.

        Args:
            dag_id: UUID of the DAG.

        Returns:
            The :class:`DAG` in ``cancelled`` status.
        """
        data = await self._request("POST", f"/dags/{dag_id}/cancel")
        return DAG(**data)

    async def pause_dag(self, dag_id: str) -> DAG:
        """Pause a running DAG.

        Args:
            dag_id: UUID of the DAG.

        Returns:
            The :class:`DAG` in ``paused`` status.
        """
        data = await self._request("POST", f"/dags/{dag_id}/pause")
        return DAG(**data)

    async def resume_dag(self, dag_id: str) -> DAG:
        """Resume a previously paused DAG.

        Args:
            dag_id: UUID of the DAG.

        Returns:
            The :class:`DAG` in ``running`` status.
        """
        data = await self._request("POST", f"/dags/{dag_id}/resume")
        return DAG(**data)

    # -------------------------------------------------------------------------
    # Approvals
    # -------------------------------------------------------------------------

    async def list_approvals(
        self,
        page: int = 1,
        per_page: int = 20,
        status: str | None = None,
        task_id: str | None = None,
    ) -> ApprovalList:
        """List approval requests with optional filtering.

        Args:
            page: Page number (1-indexed).
            per_page: Number of approvals per page.
            status: Filter by approval status.
            task_id: Filter by linked task.

        Returns:
            An :class:`ApprovalList` with matching approvals.
        """
        params: dict[str, Any] = {}
        if status:
            params["status"] = status
        if task_id:
            params["taskId"] = task_id
        data = await self._paginated_request("/approvals", params, page, per_page)
        return ApprovalList(**data)

    async def get_approval(self, approval_id: str) -> Approval:
        """Retrieve a single approval request.

        Args:
            approval_id: UUID of the approval.

        Returns:
            The :class:`Approval` object.

        Raises:
            ApexNotFoundError: No approval exists with the given ID.
        """
        data = await self._request("GET", f"/approvals/{approval_id}")
        return Approval(**data)

    async def create_approval(self, approval: ApprovalCreate) -> Approval:
        """Create a new approval request.

        Args:
            approval: Approval specification.

        Returns:
            The created :class:`Approval` in ``pending`` status.
        """
        data = await self._request(
            "POST",
            "/approvals",
            json_data=approval.model_dump(by_alias=True, exclude_none=True),
        )
        return Approval(**data)

    async def decide_approval(self, approval_id: str, decision: ApprovalDecision) -> Approval:
        """Approve or reject an approval request.

        Args:
            approval_id: UUID of the approval.
            decision: The verdict and optional comment.

        Returns:
            The updated :class:`Approval`.
        """
        data = await self._request(
            "POST",
            f"/approvals/{approval_id}/decide",
            json_data=decision.model_dump(by_alias=True, exclude_none=True),
        )
        return Approval(**data)
