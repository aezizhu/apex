"""Integration tests for the Task API endpoints."""

from __future__ import annotations

import asyncio
from typing import Any

import asyncpg
import pytest
import pytest_asyncio

from apex_sdk import AsyncApexClient
from apex_sdk.exceptions import ApexNotFoundError, ApexValidationError
from apex_sdk.models import (
    Task,
    TaskCreate,
    TaskInput,
    TaskPriority,
    TaskStatus,
    TaskUpdate,
)


class TestTaskCRUD:
    """Tests for Task CRUD operations."""

    @pytest.mark.asyncio
    async def test_create_task_minimal(
        self,
        api_client: AsyncApexClient,
        cleanup_tasks: list[str],
    ) -> None:
        """Test creating a task with minimal required fields."""
        # Arrange
        task_data = TaskCreate(name="minimal-task")

        # Act
        task = await api_client.create_task(task_data)
        cleanup_tasks.append(task.id)

        # Assert
        assert task.id is not None
        assert task.name == "minimal-task"
        assert task.status == TaskStatus.PENDING
        assert task.priority == TaskPriority.NORMAL
        assert task.created_at is not None

    @pytest.mark.asyncio
    async def test_create_task_full(
        self,
        api_client: AsyncApexClient,
        cleanup_tasks: list[str],
        sample_task_input: dict[str, Any],
        sample_task_metadata: dict[str, Any],
    ) -> None:
        """Test creating a task with all fields populated."""
        # Arrange
        task_data = TaskCreate(
            name="full-task",
            description="A fully specified test task",
            priority=TaskPriority.HIGH,
            input=TaskInput(**sample_task_input),
            timeout_seconds=300,
            retries=3,
            tags=["integration", "test", "full"],
            metadata=sample_task_metadata,
        )

        # Act
        task = await api_client.create_task(task_data)
        cleanup_tasks.append(task.id)

        # Assert
        assert task.name == "full-task"
        assert task.description == "A fully specified test task"
        assert task.priority == TaskPriority.HIGH
        assert task.timeout_seconds == 300
        assert task.retries == 3
        assert set(task.tags) == {"integration", "test", "full"}
        assert task.metadata["source"] == "integration-test"

    @pytest.mark.asyncio
    async def test_get_task(
        self,
        api_client: AsyncApexClient,
        cleanup_tasks: list[str],
    ) -> None:
        """Test retrieving a task by ID."""
        # Arrange
        created = await api_client.create_task(TaskCreate(name="get-test-task"))
        cleanup_tasks.append(created.id)

        # Act
        retrieved = await api_client.get_task(created.id)

        # Assert
        assert retrieved.id == created.id
        assert retrieved.name == created.name
        assert retrieved.status == created.status

    @pytest.mark.asyncio
    async def test_get_task_not_found(
        self,
        api_client: AsyncApexClient,
    ) -> None:
        """Test retrieving a non-existent task."""
        # Act & Assert
        with pytest.raises(ApexNotFoundError):
            await api_client.get_task("non-existent-task-id")

    @pytest.mark.asyncio
    async def test_update_task(
        self,
        api_client: AsyncApexClient,
        cleanup_tasks: list[str],
    ) -> None:
        """Test updating a task."""
        # Arrange
        task = await api_client.create_task(
            TaskCreate(name="update-test", description="Original")
        )
        cleanup_tasks.append(task.id)

        # Act
        updated = await api_client.update_task(
            task.id,
            TaskUpdate(
                description="Updated description",
                priority=TaskPriority.CRITICAL,
                tags=["updated"],
            ),
        )

        # Assert
        assert updated.id == task.id
        assert updated.description == "Updated description"
        assert updated.priority == TaskPriority.CRITICAL
        assert "updated" in updated.tags

    @pytest.mark.asyncio
    async def test_delete_task(
        self,
        api_client: AsyncApexClient,
    ) -> None:
        """Test deleting a task."""
        # Arrange
        task = await api_client.create_task(TaskCreate(name="delete-test"))

        # Act
        await api_client.delete_task(task.id)

        # Assert
        with pytest.raises(ApexNotFoundError):
            await api_client.get_task(task.id)


class TestTaskListing:
    """Tests for task listing and filtering."""

    @pytest.mark.asyncio
    async def test_list_tasks_empty(
        self,
        api_client: AsyncApexClient,
    ) -> None:
        """Test listing tasks when none exist (or filtered to none)."""
        # Act
        result = await api_client.list_tasks(
            tags=["nonexistent-unique-tag-xyz123"]
        )

        # Assert
        assert result.items == []
        assert result.total == 0

    @pytest.mark.asyncio
    async def test_list_tasks_pagination(
        self,
        api_client: AsyncApexClient,
        cleanup_tasks: list[str],
    ) -> None:
        """Test task listing with pagination."""
        # Arrange - Create 5 tasks
        for i in range(5):
            task = await api_client.create_task(
                TaskCreate(name=f"pagination-test-{i}", tags=["pagination-test"])
            )
            cleanup_tasks.append(task.id)

        # Act - Get first page
        page1 = await api_client.list_tasks(
            page=1, per_page=2, tags=["pagination-test"]
        )

        # Act - Get second page
        page2 = await api_client.list_tasks(
            page=2, per_page=2, tags=["pagination-test"]
        )

        # Assert
        assert len(page1.items) == 2
        assert len(page2.items) == 2
        assert page1.total == 5
        assert page1.total_pages == 3
        # Ensure different items on different pages
        page1_ids = {t.id for t in page1.items}
        page2_ids = {t.id for t in page2.items}
        assert page1_ids.isdisjoint(page2_ids)

    @pytest.mark.asyncio
    async def test_list_tasks_filter_by_status(
        self,
        api_client: AsyncApexClient,
        cleanup_tasks: list[str],
    ) -> None:
        """Test filtering tasks by status."""
        # Arrange
        task = await api_client.create_task(
            TaskCreate(name="status-filter-test", tags=["status-filter"])
        )
        cleanup_tasks.append(task.id)

        # Act
        pending_tasks = await api_client.list_tasks(
            status="pending", tags=["status-filter"]
        )

        # Assert
        assert all(t.status == TaskStatus.PENDING for t in pending_tasks.items)

    @pytest.mark.asyncio
    async def test_list_tasks_filter_by_tags(
        self,
        api_client: AsyncApexClient,
        cleanup_tasks: list[str],
    ) -> None:
        """Test filtering tasks by tags."""
        # Arrange
        unique_tag = "unique-tag-abc123"
        task = await api_client.create_task(
            TaskCreate(name="tag-filter-test", tags=[unique_tag, "common"])
        )
        cleanup_tasks.append(task.id)

        # Also create a task without the unique tag
        other_task = await api_client.create_task(
            TaskCreate(name="other-task", tags=["common"])
        )
        cleanup_tasks.append(other_task.id)

        # Act
        filtered = await api_client.list_tasks(tags=[unique_tag])

        # Assert
        assert len(filtered.items) == 1
        assert filtered.items[0].id == task.id


class TestTaskLifecycle:
    """Tests for task lifecycle operations."""

    @pytest.mark.asyncio
    async def test_cancel_pending_task(
        self,
        api_client: AsyncApexClient,
        cleanup_tasks: list[str],
    ) -> None:
        """Test cancelling a pending task."""
        # Arrange
        task = await api_client.create_task(TaskCreate(name="cancel-test"))
        cleanup_tasks.append(task.id)

        # Act
        cancelled = await api_client.cancel_task(task.id)

        # Assert
        assert cancelled.status == TaskStatus.CANCELLED

    @pytest.mark.asyncio
    async def test_retry_failed_task(
        self,
        api_client: AsyncApexClient,
        cleanup_tasks: list[str],
        db_connection: asyncpg.Connection,
    ) -> None:
        """Test retrying a failed task."""
        # Arrange - Create and manually fail a task
        task = await api_client.create_task(
            TaskCreate(name="retry-test", retries=3)
        )
        cleanup_tasks.append(task.id)

        # Simulate task failure by updating status directly in DB
        await db_connection.execute(
            "UPDATE tasks SET status = 'failed' WHERE id = $1",
            task.id,
        )

        # Act
        retried = await api_client.retry_task(task.id)

        # Assert
        assert retried.status in [TaskStatus.PENDING, TaskStatus.QUEUED]
        assert retried.retry_count > 0


class TestTaskDatabaseState:
    """Tests that verify database state after task operations."""

    @pytest.mark.asyncio
    @pytest.mark.database
    async def test_task_creation_persists_to_database(
        self,
        api_client: AsyncApexClient,
        cleanup_tasks: list[str],
        db_connection: asyncpg.Connection,
    ) -> None:
        """Verify task creation persists correctly to the database."""
        # Arrange & Act
        task = await api_client.create_task(
            TaskCreate(
                name="db-persist-test",
                description="Testing DB persistence",
                tags=["db-test"],
            )
        )
        cleanup_tasks.append(task.id)

        # Assert - Query database directly
        row = await db_connection.fetchrow(
            "SELECT * FROM tasks WHERE id = $1",
            task.id,
        )

        assert row is not None
        assert row["name"] == "db-persist-test"
        assert row["description"] == "Testing DB persistence"
        assert row["status"] == "pending"

    @pytest.mark.asyncio
    @pytest.mark.database
    async def test_task_update_persists_to_database(
        self,
        api_client: AsyncApexClient,
        cleanup_tasks: list[str],
        db_connection: asyncpg.Connection,
    ) -> None:
        """Verify task updates persist correctly to the database."""
        # Arrange
        task = await api_client.create_task(TaskCreate(name="db-update-test"))
        cleanup_tasks.append(task.id)

        # Act
        await api_client.update_task(
            task.id,
            TaskUpdate(description="Updated via API"),
        )

        # Assert - Query database directly
        row = await db_connection.fetchrow(
            "SELECT description, updated_at FROM tasks WHERE id = $1",
            task.id,
        )

        assert row["description"] == "Updated via API"
        assert row["updated_at"] > task.created_at

    @pytest.mark.asyncio
    @pytest.mark.database
    async def test_task_deletion_removes_from_database(
        self,
        api_client: AsyncApexClient,
        db_connection: asyncpg.Connection,
    ) -> None:
        """Verify task deletion removes the record from database."""
        # Arrange
        task = await api_client.create_task(TaskCreate(name="db-delete-test"))

        # Act
        await api_client.delete_task(task.id)

        # Assert - Query database directly
        row = await db_connection.fetchrow(
            "SELECT * FROM tasks WHERE id = $1",
            task.id,
        )

        assert row is None


class TestTaskValidation:
    """Tests for task input validation."""

    @pytest.mark.asyncio
    async def test_create_task_empty_name_fails(
        self,
        api_client: AsyncApexClient,
    ) -> None:
        """Test that creating a task with empty name fails."""
        # Act & Assert
        with pytest.raises(ApexValidationError):
            await api_client.create_task(TaskCreate(name=""))

    @pytest.mark.asyncio
    async def test_create_task_long_name_fails(
        self,
        api_client: AsyncApexClient,
    ) -> None:
        """Test that creating a task with too long name fails."""
        # Act & Assert
        with pytest.raises(ApexValidationError):
            await api_client.create_task(TaskCreate(name="x" * 300))

    @pytest.mark.asyncio
    async def test_create_task_negative_timeout_fails(
        self,
        api_client: AsyncApexClient,
    ) -> None:
        """Test that creating a task with negative timeout fails."""
        # Act & Assert
        with pytest.raises(ApexValidationError):
            await api_client.create_task(
                TaskCreate(name="negative-timeout", timeout_seconds=-1)
            )

    @pytest.mark.asyncio
    async def test_create_task_excessive_retries_fails(
        self,
        api_client: AsyncApexClient,
    ) -> None:
        """Test that creating a task with too many retries fails."""
        # Act & Assert
        with pytest.raises(ApexValidationError):
            await api_client.create_task(
                TaskCreate(name="too-many-retries", retries=100)
            )


class TestTaskConcurrency:
    """Tests for concurrent task operations."""

    @pytest.mark.asyncio
    @pytest.mark.slow
    async def test_concurrent_task_creation(
        self,
        api_client: AsyncApexClient,
        cleanup_tasks: list[str],
    ) -> None:
        """Test creating multiple tasks concurrently."""
        # Arrange
        num_tasks = 10
        task_data = [
            TaskCreate(name=f"concurrent-{i}", tags=["concurrent-test"])
            for i in range(num_tasks)
        ]

        # Act
        tasks = await asyncio.gather(
            *[api_client.create_task(data) for data in task_data]
        )

        # Cleanup
        for task in tasks:
            cleanup_tasks.append(task.id)

        # Assert
        assert len(tasks) == num_tasks
        assert len(set(t.id for t in tasks)) == num_tasks  # All unique IDs

    @pytest.mark.asyncio
    @pytest.mark.slow
    async def test_concurrent_task_updates(
        self,
        api_client: AsyncApexClient,
        cleanup_tasks: list[str],
    ) -> None:
        """Test updating a task concurrently (optimistic locking)."""
        # Arrange
        task = await api_client.create_task(TaskCreate(name="concurrent-update"))
        cleanup_tasks.append(task.id)

        # Act - Multiple concurrent updates
        updates = [
            api_client.update_task(
                task.id,
                TaskUpdate(description=f"Update {i}"),
            )
            for i in range(5)
        ]

        results = await asyncio.gather(*updates, return_exceptions=True)

        # Assert - All should succeed (last write wins) or handle conflicts
        successful = [r for r in results if isinstance(r, Task)]
        assert len(successful) >= 1
