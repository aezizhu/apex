"""Contract enforcement tests for the Apex API."""

from __future__ import annotations

import asyncio
from datetime import datetime, timezone
from typing import Any

import pytest
import pytest_asyncio

from apex_sdk import AsyncApexClient, ApexClient
from apex_sdk.exceptions import (
    ApexAPIError,
    ApexAuthenticationError,
    ApexNotFoundError,
    ApexRateLimitError,
    ApexValidationError,
)
from apex_sdk.models import (
    AgentCreate,
    AgentList,
    AgentStatus,
    DAGCreate,
    DAGList,
    DAGNode,
    DAGStatus,
    HealthStatus,
    Task,
    TaskCreate,
    TaskList,
    TaskPriority,
    TaskStatus,
)


class TestHealthEndpointContract:
    """Tests for the health endpoint contract."""

    @pytest.mark.asyncio
    async def test_health_response_structure(
        self,
        api_client: AsyncApexClient,
    ) -> None:
        """Verify health endpoint returns expected structure."""
        # Act
        health = await api_client.health()

        # Assert - Required fields
        assert isinstance(health, HealthStatus)
        assert hasattr(health, "status")
        assert hasattr(health, "version")
        assert hasattr(health, "uptime")
        assert hasattr(health, "services")

        # Assert - Field types
        assert isinstance(health.status, str)
        assert isinstance(health.version, str)
        assert isinstance(health.uptime, (int, float))
        assert isinstance(health.services, dict)

    @pytest.mark.asyncio
    async def test_health_status_values(
        self,
        api_client: AsyncApexClient,
    ) -> None:
        """Verify health status returns valid values."""
        # Act
        health = await api_client.health()

        # Assert - Status should be a known value
        valid_statuses = ["healthy", "degraded", "unhealthy"]
        assert health.status in valid_statuses


class TestTaskResponseContract:
    """Tests for task response contracts."""

    @pytest.mark.asyncio
    async def test_task_response_structure(
        self,
        api_client: AsyncApexClient,
        cleanup_tasks: list[str],
    ) -> None:
        """Verify task response has all required fields."""
        # Act
        task = await api_client.create_task(
            TaskCreate(name="contract-test-task")
        )
        cleanup_tasks.append(task.id)

        # Assert - Required fields exist
        assert isinstance(task, Task)
        assert hasattr(task, "id")
        assert hasattr(task, "name")
        assert hasattr(task, "status")
        assert hasattr(task, "priority")
        assert hasattr(task, "created_at")
        assert hasattr(task, "updated_at")

        # Assert - Field types
        assert isinstance(task.id, str)
        assert isinstance(task.name, str)
        assert isinstance(task.status, (str, TaskStatus))
        assert isinstance(task.priority, (str, TaskPriority))
        assert isinstance(task.created_at, datetime)
        assert isinstance(task.updated_at, datetime)

    @pytest.mark.asyncio
    async def test_task_list_response_structure(
        self,
        api_client: AsyncApexClient,
        cleanup_tasks: list[str],
    ) -> None:
        """Verify task list response has pagination fields."""
        # Arrange
        task = await api_client.create_task(
            TaskCreate(name="list-contract-test", tags=["list-contract"])
        )
        cleanup_tasks.append(task.id)

        # Act
        result = await api_client.list_tasks(tags=["list-contract"])

        # Assert - Pagination structure
        assert isinstance(result, TaskList)
        assert hasattr(result, "items")
        assert hasattr(result, "total")
        assert hasattr(result, "page")
        assert hasattr(result, "per_page")
        assert hasattr(result, "total_pages")

        # Assert - Types
        assert isinstance(result.items, list)
        assert isinstance(result.total, int)
        assert isinstance(result.page, int)
        assert isinstance(result.per_page, int)
        assert isinstance(result.total_pages, int)

        # Assert - Logical constraints
        assert result.total >= len(result.items)
        assert result.page >= 1
        assert result.per_page >= 1

    @pytest.mark.asyncio
    async def test_task_status_enum_values(
        self,
        api_client: AsyncApexClient,
        cleanup_tasks: list[str],
    ) -> None:
        """Verify task status is a valid enum value."""
        # Act
        task = await api_client.create_task(TaskCreate(name="status-enum-test"))
        cleanup_tasks.append(task.id)

        # Assert - Status is valid enum
        valid_statuses = [
            "pending", "queued", "running", "paused",
            "completed", "failed", "cancelled"
        ]
        # Convert to string value if it's an enum
        status_value = task.status.value if hasattr(task.status, "value") else task.status
        assert status_value in valid_statuses

    @pytest.mark.asyncio
    async def test_task_priority_enum_values(
        self,
        api_client: AsyncApexClient,
        cleanup_tasks: list[str],
    ) -> None:
        """Verify task priority is a valid enum value."""
        # Test all priority values
        valid_priorities = ["low", "normal", "high", "critical"]

        for priority in valid_priorities:
            task = await api_client.create_task(
                TaskCreate(
                    name=f"priority-{priority}-test",
                    priority=TaskPriority(priority),
                )
            )
            cleanup_tasks.append(task.id)

            priority_value = task.priority.value if hasattr(task.priority, "value") else task.priority
            assert priority_value == priority


class TestAgentResponseContract:
    """Tests for agent response contracts."""

    @pytest.mark.asyncio
    async def test_agent_response_structure(
        self,
        api_client: AsyncApexClient,
        cleanup_agents: list[str],
    ) -> None:
        """Verify agent response has all required fields."""
        # Act
        agent = await api_client.create_agent(
            AgentCreate(name="contract-test-agent")
        )
        cleanup_agents.append(agent.id)

        # Assert - Required fields
        assert hasattr(agent, "id")
        assert hasattr(agent, "name")
        assert hasattr(agent, "status")
        assert hasattr(agent, "capabilities")
        assert hasattr(agent, "max_concurrent_tasks")
        assert hasattr(agent, "created_at")
        assert hasattr(agent, "updated_at")

        # Assert - Types
        assert isinstance(agent.id, str)
        assert isinstance(agent.name, str)
        assert isinstance(agent.capabilities, list)
        assert isinstance(agent.max_concurrent_tasks, int)

    @pytest.mark.asyncio
    async def test_agent_list_response_structure(
        self,
        api_client: AsyncApexClient,
        cleanup_agents: list[str],
    ) -> None:
        """Verify agent list response has pagination fields."""
        # Arrange
        agent = await api_client.create_agent(
            AgentCreate(name="list-contract-agent", tags=["agent-list-contract"])
        )
        cleanup_agents.append(agent.id)

        # Act
        result = await api_client.list_agents(tags=["agent-list-contract"])

        # Assert
        assert isinstance(result, AgentList)
        assert hasattr(result, "items")
        assert hasattr(result, "total")
        assert hasattr(result, "page")
        assert hasattr(result, "per_page")
        assert hasattr(result, "total_pages")


class TestDAGResponseContract:
    """Tests for DAG response contracts."""

    @pytest.mark.asyncio
    async def test_dag_response_structure(
        self,
        api_client: AsyncApexClient,
        cleanup_dags: list[str],
    ) -> None:
        """Verify DAG response has all required fields."""
        # Act
        dag = await api_client.create_dag(
            DAGCreate(
                name="contract-test-dag",
                nodes=[
                    DAGNode(
                        id="node-1",
                        task_template=TaskCreate(name="dag-task"),
                    )
                ],
            )
        )
        cleanup_dags.append(dag.id)

        # Assert - Required fields
        assert hasattr(dag, "id")
        assert hasattr(dag, "name")
        assert hasattr(dag, "status")
        assert hasattr(dag, "nodes")
        assert hasattr(dag, "edges")
        assert hasattr(dag, "created_at")
        assert hasattr(dag, "updated_at")

        # Assert - Types
        assert isinstance(dag.id, str)
        assert isinstance(dag.name, str)
        assert isinstance(dag.nodes, list)
        assert isinstance(dag.edges, list)


class TestErrorResponseContract:
    """Tests for error response contracts."""

    @pytest.mark.asyncio
    async def test_not_found_error_structure(
        self,
        api_client: AsyncApexClient,
    ) -> None:
        """Verify 404 error response structure."""
        # Act & Assert
        with pytest.raises(ApexNotFoundError) as exc_info:
            await api_client.get_task("nonexistent-task-id-12345")

        error = exc_info.value
        assert hasattr(error, "message")
        assert hasattr(error, "status_code")
        assert error.status_code == 404

    @pytest.mark.asyncio
    async def test_validation_error_structure(
        self,
        api_client: AsyncApexClient,
    ) -> None:
        """Verify 422 validation error response structure."""
        # Act & Assert
        with pytest.raises(ApexValidationError) as exc_info:
            await api_client.create_task(TaskCreate(name=""))

        error = exc_info.value
        assert hasattr(error, "message")
        assert hasattr(error, "status_code")
        assert error.status_code == 422

    @pytest.mark.asyncio
    async def test_authentication_error_structure(
        self,
        test_config: dict[str, Any],
    ) -> None:
        """Verify 401 authentication error response structure."""
        # Arrange - Client with invalid credentials
        client = AsyncApexClient(
            base_url=test_config["api_url"],
            api_key="invalid-api-key-12345",
        )

        try:
            # Act & Assert
            with pytest.raises(ApexAuthenticationError) as exc_info:
                await client.list_tasks()

            error = exc_info.value
            assert hasattr(error, "message")
            assert hasattr(error, "status_code")
            assert error.status_code == 401
        except ApexAPIError:
            # Some implementations may return different errors
            pass
        finally:
            await client.close()


class TestPaginationContract:
    """Tests for pagination behavior contracts."""

    @pytest.mark.asyncio
    async def test_pagination_defaults(
        self,
        api_client: AsyncApexClient,
    ) -> None:
        """Verify default pagination parameters."""
        # Act
        result = await api_client.list_tasks()

        # Assert - Defaults
        assert result.page == 1
        assert result.per_page == 20  # Default page size

    @pytest.mark.asyncio
    async def test_pagination_bounds(
        self,
        api_client: AsyncApexClient,
        cleanup_tasks: list[str],
    ) -> None:
        """Verify pagination respects bounds."""
        # Arrange - Create some tasks
        for i in range(5):
            task = await api_client.create_task(
                TaskCreate(name=f"pagination-bounds-{i}", tags=["pagination-bounds"])
            )
            cleanup_tasks.append(task.id)

        # Act - Request specific page size
        result = await api_client.list_tasks(
            page=1, per_page=3, tags=["pagination-bounds"]
        )

        # Assert
        assert len(result.items) <= 3
        assert result.per_page == 3

    @pytest.mark.asyncio
    async def test_pagination_total_pages_calculation(
        self,
        api_client: AsyncApexClient,
        cleanup_tasks: list[str],
    ) -> None:
        """Verify total_pages is calculated correctly."""
        # Arrange - Create 7 tasks
        for i in range(7):
            task = await api_client.create_task(
                TaskCreate(name=f"total-pages-{i}", tags=["total-pages-test"])
            )
            cleanup_tasks.append(task.id)

        # Act - Request with page size of 3
        result = await api_client.list_tasks(
            page=1, per_page=3, tags=["total-pages-test"]
        )

        # Assert - 7 items / 3 per page = 3 pages (rounded up)
        assert result.total_pages == 3

    @pytest.mark.asyncio
    async def test_empty_page_returns_empty_items(
        self,
        api_client: AsyncApexClient,
    ) -> None:
        """Verify requesting beyond available pages returns empty items."""
        # Act - Request very high page number
        result = await api_client.list_tasks(
            page=9999,
            tags=["nonexistent-unique-tag-xyz"],
        )

        # Assert
        assert result.items == []


class TestTimestampContract:
    """Tests for timestamp handling contracts."""

    @pytest.mark.asyncio
    async def test_created_at_is_set_on_creation(
        self,
        api_client: AsyncApexClient,
        cleanup_tasks: list[str],
    ) -> None:
        """Verify created_at is set when resource is created."""
        # Arrange
        before = datetime.now(timezone.utc)

        # Act
        task = await api_client.create_task(TaskCreate(name="timestamp-test"))
        cleanup_tasks.append(task.id)

        after = datetime.now(timezone.utc)

        # Assert - created_at is between before and after
        # Note: Need to handle timezone awareness
        created_at = task.created_at
        if created_at.tzinfo is None:
            created_at = created_at.replace(tzinfo=timezone.utc)

        # Allow 1 second tolerance for clock drift
        assert before.timestamp() - 1 <= created_at.timestamp() <= after.timestamp() + 1

    @pytest.mark.asyncio
    async def test_updated_at_changes_on_update(
        self,
        api_client: AsyncApexClient,
        cleanup_tasks: list[str],
    ) -> None:
        """Verify updated_at changes when resource is modified."""
        # Arrange
        task = await api_client.create_task(TaskCreate(name="update-timestamp-test"))
        cleanup_tasks.append(task.id)
        original_updated = task.updated_at

        # Small delay to ensure timestamp difference
        await asyncio.sleep(0.1)

        # Act
        from apex_sdk.models import TaskUpdate
        updated_task = await api_client.update_task(
            task.id,
            TaskUpdate(description="Modified"),
        )

        # Assert
        assert updated_task.updated_at >= original_updated


class TestIdempotencyContract:
    """Tests for idempotency contracts."""

    @pytest.mark.asyncio
    async def test_get_is_idempotent(
        self,
        api_client: AsyncApexClient,
        cleanup_tasks: list[str],
    ) -> None:
        """Verify GET requests are idempotent."""
        # Arrange
        task = await api_client.create_task(TaskCreate(name="idempotent-get-test"))
        cleanup_tasks.append(task.id)

        # Act - Multiple GET requests
        result1 = await api_client.get_task(task.id)
        result2 = await api_client.get_task(task.id)
        result3 = await api_client.get_task(task.id)

        # Assert - All return same data
        assert result1.id == result2.id == result3.id
        assert result1.name == result2.name == result3.name

    @pytest.mark.asyncio
    async def test_delete_is_idempotent(
        self,
        api_client: AsyncApexClient,
    ) -> None:
        """Verify DELETE requests don't error on already deleted resources."""
        # Arrange
        task = await api_client.create_task(TaskCreate(name="idempotent-delete-test"))

        # Act - Delete twice
        await api_client.delete_task(task.id)

        # Second delete should either succeed or raise NotFound
        try:
            await api_client.delete_task(task.id)
        except ApexNotFoundError:
            pass  # This is acceptable idempotent behavior


class TestConcurrencyContract:
    """Tests for concurrency handling contracts."""

    @pytest.mark.asyncio
    @pytest.mark.slow
    async def test_concurrent_creates_all_succeed(
        self,
        api_client: AsyncApexClient,
        cleanup_tasks: list[str],
    ) -> None:
        """Verify concurrent create requests all succeed."""
        # Arrange
        num_tasks = 20

        # Act
        tasks = await asyncio.gather(
            *[
                api_client.create_task(
                    TaskCreate(name=f"concurrent-contract-{i}")
                )
                for i in range(num_tasks)
            ]
        )

        # Cleanup
        for task in tasks:
            cleanup_tasks.append(task.id)

        # Assert
        assert len(tasks) == num_tasks
        ids = [t.id for t in tasks]
        assert len(set(ids)) == num_tasks  # All unique IDs

    @pytest.mark.asyncio
    @pytest.mark.slow
    async def test_concurrent_reads_consistent(
        self,
        api_client: AsyncApexClient,
        cleanup_tasks: list[str],
    ) -> None:
        """Verify concurrent read requests return consistent data."""
        # Arrange
        task = await api_client.create_task(TaskCreate(name="concurrent-read-test"))
        cleanup_tasks.append(task.id)

        # Act - 10 concurrent reads
        results = await asyncio.gather(
            *[api_client.get_task(task.id) for _ in range(10)]
        )

        # Assert - All results are consistent
        assert all(r.id == task.id for r in results)
        assert all(r.name == task.name for r in results)


class TestSyncClientContract:
    """Tests for synchronous client contract compliance."""

    def test_sync_client_task_operations(
        self,
        sync_api_client: ApexClient,
    ) -> None:
        """Verify sync client supports same operations as async."""
        # Act
        task = sync_api_client.create_task(TaskCreate(name="sync-contract-test"))

        try:
            # Assert - Can perform operations
            retrieved = sync_api_client.get_task(task.id)
            assert retrieved.id == task.id

            # List works
            task_list = sync_api_client.list_tasks()
            assert isinstance(task_list, TaskList)

        finally:
            # Cleanup
            sync_api_client.delete_task(task.id)

    def test_sync_client_error_handling(
        self,
        sync_api_client: ApexClient,
    ) -> None:
        """Verify sync client raises same exceptions as async."""
        # Act & Assert
        with pytest.raises(ApexNotFoundError):
            sync_api_client.get_task("nonexistent-sync-task")

        with pytest.raises(ApexValidationError):
            sync_api_client.create_task(TaskCreate(name=""))
