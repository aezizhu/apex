"""Sync and async HTTP clients for the Apex API."""

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
    """Base class with shared configuration for Apex clients."""

    def __init__(
        self,
        base_url: str,
        api_key: str | None = None,
        token: str | None = None,
        timeout: float = 30.0,
        max_retries: int = 3,
        retry_delay: float = 1.0,
    ) -> None:
        """
        Initialize the Apex client.

        Args:
            base_url: The base URL of the Apex API.
            api_key: API key for authentication.
            token: Bearer token for authentication (alternative to api_key).
            timeout: Request timeout in seconds.
            max_retries: Maximum number of retry attempts.
            retry_delay: Initial delay between retries.
        """
        self.base_url = base_url.rstrip("/")
        self.api_key = api_key
        self.token = token
        self.timeout = timeout
        self.max_retries = max_retries
        self.retry_delay = retry_delay

    def _get_headers(self) -> dict[str, str]:
        """Get authentication headers."""
        headers = {"Content-Type": "application/json", "Accept": "application/json"}
        if self.api_key:
            headers["X-API-Key"] = self.api_key
        elif self.token:
            headers["Authorization"] = f"Bearer {self.token}"
        return headers

    def _handle_error_response(self, response: httpx.Response) -> None:
        """Handle error responses from the API."""
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
    """Synchronous HTTP client for the Apex API."""

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
        """Close the HTTP client."""
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
        """Make an HTTP request with automatic retries."""
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
        """Make a paginated request."""
        request_params = params or {}
        request_params["page"] = page
        request_params["perPage"] = per_page
        return self._request("GET", path, params=request_params)

    # Health Check

    def health(self) -> HealthStatus:
        """Check the API health status."""
        data = self._request("GET", "/health")
        return HealthStatus(**data)

    # Tasks

    def list_tasks(
        self,
        page: int = 1,
        per_page: int = 20,
        status: str | None = None,
        agent_id: str | None = None,
        dag_id: str | None = None,
        tags: list[str] | None = None,
    ) -> TaskList:
        """List tasks with optional filtering."""
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
        """Get a task by ID."""
        data = self._request("GET", f"/tasks/{task_id}")
        return Task(**data)

    def create_task(self, task: TaskCreate) -> Task:
        """Create a new task."""
        data = self._request(
            "POST",
            "/tasks",
            json_data=task.model_dump(by_alias=True, exclude_none=True),
        )
        return Task(**data)

    def update_task(self, task_id: str, task: TaskUpdate) -> Task:
        """Update an existing task."""
        data = self._request(
            "PATCH",
            f"/tasks/{task_id}",
            json_data=task.model_dump(by_alias=True, exclude_none=True),
        )
        return Task(**data)

    def delete_task(self, task_id: str) -> None:
        """Delete a task."""
        self._request("DELETE", f"/tasks/{task_id}")

    def cancel_task(self, task_id: str) -> Task:
        """Cancel a running task."""
        data = self._request("POST", f"/tasks/{task_id}/cancel")
        return Task(**data)

    def retry_task(self, task_id: str) -> Task:
        """Retry a failed task."""
        data = self._request("POST", f"/tasks/{task_id}/retry")
        return Task(**data)

    # Agents

    def list_agents(
        self,
        page: int = 1,
        per_page: int = 20,
        status: str | None = None,
        tags: list[str] | None = None,
    ) -> AgentList:
        """List agents with optional filtering."""
        params: dict[str, Any] = {}
        if status:
            params["status"] = status
        if tags:
            params["tags"] = ",".join(tags)
        data = self._paginated_request("/agents", params, page, per_page)
        return AgentList(**data)

    def get_agent(self, agent_id: str) -> Agent:
        """Get an agent by ID."""
        data = self._request("GET", f"/agents/{agent_id}")
        return Agent(**data)

    def create_agent(self, agent: AgentCreate) -> Agent:
        """Create a new agent."""
        data = self._request(
            "POST",
            "/agents",
            json_data=agent.model_dump(by_alias=True, exclude_none=True),
        )
        return Agent(**data)

    def update_agent(self, agent_id: str, agent: AgentUpdate) -> Agent:
        """Update an existing agent."""
        data = self._request(
            "PATCH",
            f"/agents/{agent_id}",
            json_data=agent.model_dump(by_alias=True, exclude_none=True),
        )
        return Agent(**data)

    def delete_agent(self, agent_id: str) -> None:
        """Delete an agent."""
        self._request("DELETE", f"/agents/{agent_id}")

    # DAGs

    def list_dags(
        self,
        page: int = 1,
        per_page: int = 20,
        status: str | None = None,
        tags: list[str] | None = None,
    ) -> DAGList:
        """List DAGs with optional filtering."""
        params: dict[str, Any] = {}
        if status:
            params["status"] = status
        if tags:
            params["tags"] = ",".join(tags)
        data = self._paginated_request("/dags", params, page, per_page)
        return DAGList(**data)

    def get_dag(self, dag_id: str) -> DAG:
        """Get a DAG by ID."""
        data = self._request("GET", f"/dags/{dag_id}")
        return DAG(**data)

    def create_dag(self, dag: DAGCreate) -> DAG:
        """Create a new DAG."""
        data = self._request(
            "POST",
            "/dags",
            json_data=dag.model_dump(by_alias=True, exclude_none=True),
        )
        return DAG(**data)

    def update_dag(self, dag_id: str, dag: DAGUpdate) -> DAG:
        """Update an existing DAG."""
        data = self._request(
            "PATCH",
            f"/dags/{dag_id}",
            json_data=dag.model_dump(by_alias=True, exclude_none=True),
        )
        return DAG(**data)

    def delete_dag(self, dag_id: str) -> None:
        """Delete a DAG."""
        self._request("DELETE", f"/dags/{dag_id}")

    def start_dag(self, dag_id: str, input_data: dict[str, Any] | None = None) -> DAG:
        """Start a DAG execution."""
        data = self._request(
            "POST",
            f"/dags/{dag_id}/start",
            json_data={"input": input_data} if input_data else None,
        )
        return DAG(**data)

    def cancel_dag(self, dag_id: str) -> DAG:
        """Cancel a running DAG."""
        data = self._request("POST", f"/dags/{dag_id}/cancel")
        return DAG(**data)

    def pause_dag(self, dag_id: str) -> DAG:
        """Pause a running DAG."""
        data = self._request("POST", f"/dags/{dag_id}/pause")
        return DAG(**data)

    def resume_dag(self, dag_id: str) -> DAG:
        """Resume a paused DAG."""
        data = self._request("POST", f"/dags/{dag_id}/resume")
        return DAG(**data)

    # Approvals

    def list_approvals(
        self,
        page: int = 1,
        per_page: int = 20,
        status: str | None = None,
        task_id: str | None = None,
    ) -> ApprovalList:
        """List approvals with optional filtering."""
        params: dict[str, Any] = {}
        if status:
            params["status"] = status
        if task_id:
            params["taskId"] = task_id
        data = self._paginated_request("/approvals", params, page, per_page)
        return ApprovalList(**data)

    def get_approval(self, approval_id: str) -> Approval:
        """Get an approval by ID."""
        data = self._request("GET", f"/approvals/{approval_id}")
        return Approval(**data)

    def create_approval(self, approval: ApprovalCreate) -> Approval:
        """Create a new approval request."""
        data = self._request(
            "POST",
            "/approvals",
            json_data=approval.model_dump(by_alias=True, exclude_none=True),
        )
        return Approval(**data)

    def decide_approval(self, approval_id: str, decision: ApprovalDecision) -> Approval:
        """Make a decision on an approval."""
        data = self._request(
            "POST",
            f"/approvals/{approval_id}/decide",
            json_data=decision.model_dump(by_alias=True, exclude_none=True),
        )
        return Approval(**data)


class AsyncApexClient(BaseApexClient):
    """Asynchronous HTTP client for the Apex API."""

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
        """Close the HTTP and WebSocket clients."""
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
        """Make an HTTP request with automatic retries."""
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
        """Make a paginated request."""
        request_params = params or {}
        request_params["page"] = page
        request_params["perPage"] = per_page
        return await self._request("GET", path, params=request_params)

    # WebSocket

    def websocket(self) -> ApexWebSocketClient:
        """Get the WebSocket client for real-time updates."""
        if self._ws_client is None:
            self._ws_client = ApexWebSocketClient(
                base_url=self.base_url,
                api_key=self.api_key,
                token=self.token,
            )
        return self._ws_client

    # Health Check

    async def health(self) -> HealthStatus:
        """Check the API health status."""
        data = await self._request("GET", "/health")
        return HealthStatus(**data)

    # Tasks

    async def list_tasks(
        self,
        page: int = 1,
        per_page: int = 20,
        status: str | None = None,
        agent_id: str | None = None,
        dag_id: str | None = None,
        tags: list[str] | None = None,
    ) -> TaskList:
        """List tasks with optional filtering."""
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
        """Get a task by ID."""
        data = await self._request("GET", f"/tasks/{task_id}")
        return Task(**data)

    async def create_task(self, task: TaskCreate) -> Task:
        """Create a new task."""
        data = await self._request(
            "POST",
            "/tasks",
            json_data=task.model_dump(by_alias=True, exclude_none=True),
        )
        return Task(**data)

    async def update_task(self, task_id: str, task: TaskUpdate) -> Task:
        """Update an existing task."""
        data = await self._request(
            "PATCH",
            f"/tasks/{task_id}",
            json_data=task.model_dump(by_alias=True, exclude_none=True),
        )
        return Task(**data)

    async def delete_task(self, task_id: str) -> None:
        """Delete a task."""
        await self._request("DELETE", f"/tasks/{task_id}")

    async def cancel_task(self, task_id: str) -> Task:
        """Cancel a running task."""
        data = await self._request("POST", f"/tasks/{task_id}/cancel")
        return Task(**data)

    async def retry_task(self, task_id: str) -> Task:
        """Retry a failed task."""
        data = await self._request("POST", f"/tasks/{task_id}/retry")
        return Task(**data)

    # Agents

    async def list_agents(
        self,
        page: int = 1,
        per_page: int = 20,
        status: str | None = None,
        tags: list[str] | None = None,
    ) -> AgentList:
        """List agents with optional filtering."""
        params: dict[str, Any] = {}
        if status:
            params["status"] = status
        if tags:
            params["tags"] = ",".join(tags)
        data = await self._paginated_request("/agents", params, page, per_page)
        return AgentList(**data)

    async def get_agent(self, agent_id: str) -> Agent:
        """Get an agent by ID."""
        data = await self._request("GET", f"/agents/{agent_id}")
        return Agent(**data)

    async def create_agent(self, agent: AgentCreate) -> Agent:
        """Create a new agent."""
        data = await self._request(
            "POST",
            "/agents",
            json_data=agent.model_dump(by_alias=True, exclude_none=True),
        )
        return Agent(**data)

    async def update_agent(self, agent_id: str, agent: AgentUpdate) -> Agent:
        """Update an existing agent."""
        data = await self._request(
            "PATCH",
            f"/agents/{agent_id}",
            json_data=agent.model_dump(by_alias=True, exclude_none=True),
        )
        return Agent(**data)

    async def delete_agent(self, agent_id: str) -> None:
        """Delete an agent."""
        await self._request("DELETE", f"/agents/{agent_id}")

    # DAGs

    async def list_dags(
        self,
        page: int = 1,
        per_page: int = 20,
        status: str | None = None,
        tags: list[str] | None = None,
    ) -> DAGList:
        """List DAGs with optional filtering."""
        params: dict[str, Any] = {}
        if status:
            params["status"] = status
        if tags:
            params["tags"] = ",".join(tags)
        data = await self._paginated_request("/dags", params, page, per_page)
        return DAGList(**data)

    async def get_dag(self, dag_id: str) -> DAG:
        """Get a DAG by ID."""
        data = await self._request("GET", f"/dags/{dag_id}")
        return DAG(**data)

    async def create_dag(self, dag: DAGCreate) -> DAG:
        """Create a new DAG."""
        data = await self._request(
            "POST",
            "/dags",
            json_data=dag.model_dump(by_alias=True, exclude_none=True),
        )
        return DAG(**data)

    async def update_dag(self, dag_id: str, dag: DAGUpdate) -> DAG:
        """Update an existing DAG."""
        data = await self._request(
            "PATCH",
            f"/dags/{dag_id}",
            json_data=dag.model_dump(by_alias=True, exclude_none=True),
        )
        return DAG(**data)

    async def delete_dag(self, dag_id: str) -> None:
        """Delete a DAG."""
        await self._request("DELETE", f"/dags/{dag_id}")

    async def start_dag(self, dag_id: str, input_data: dict[str, Any] | None = None) -> DAG:
        """Start a DAG execution."""
        data = await self._request(
            "POST",
            f"/dags/{dag_id}/start",
            json_data={"input": input_data} if input_data else None,
        )
        return DAG(**data)

    async def cancel_dag(self, dag_id: str) -> DAG:
        """Cancel a running DAG."""
        data = await self._request("POST", f"/dags/{dag_id}/cancel")
        return DAG(**data)

    async def pause_dag(self, dag_id: str) -> DAG:
        """Pause a running DAG."""
        data = await self._request("POST", f"/dags/{dag_id}/pause")
        return DAG(**data)

    async def resume_dag(self, dag_id: str) -> DAG:
        """Resume a paused DAG."""
        data = await self._request("POST", f"/dags/{dag_id}/resume")
        return DAG(**data)

    # Approvals

    async def list_approvals(
        self,
        page: int = 1,
        per_page: int = 20,
        status: str | None = None,
        task_id: str | None = None,
    ) -> ApprovalList:
        """List approvals with optional filtering."""
        params: dict[str, Any] = {}
        if status:
            params["status"] = status
        if task_id:
            params["taskId"] = task_id
        data = await self._paginated_request("/approvals", params, page, per_page)
        return ApprovalList(**data)

    async def get_approval(self, approval_id: str) -> Approval:
        """Get an approval by ID."""
        data = await self._request("GET", f"/approvals/{approval_id}")
        return Approval(**data)

    async def create_approval(self, approval: ApprovalCreate) -> Approval:
        """Create a new approval request."""
        data = await self._request(
            "POST",
            "/approvals",
            json_data=approval.model_dump(by_alias=True, exclude_none=True),
        )
        return Approval(**data)

    async def decide_approval(self, approval_id: str, decision: ApprovalDecision) -> Approval:
        """Make a decision on an approval."""
        data = await self._request(
            "POST",
            f"/approvals/{approval_id}/decide",
            json_data=decision.model_dump(by_alias=True, exclude_none=True),
        )
        return Approval(**data)
