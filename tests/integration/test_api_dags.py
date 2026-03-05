"""Integration tests for the DAG API endpoints."""

from __future__ import annotations

import asyncio
from typing import Any

import asyncpg
import pytest
import pytest_asyncio

from apex_sdk import AsyncApexClient
from apex_sdk.exceptions import ApexNotFoundError, ApexValidationError
from apex_sdk.models import (
    DAG,
    DAGCreate,
    DAGEdge,
    DAGNode,
    DAGStatus,
    DAGUpdate,
    TaskCreate,
    TaskStatus,
)


class TestDAGCRUD:
    """Tests for DAG CRUD operations."""

    @pytest.mark.asyncio
    async def test_create_dag_minimal(
        self,
        api_client: AsyncApexClient,
        cleanup_dags: list[str],
    ) -> None:
        """Test creating a DAG with minimal required fields."""
        # Arrange
        dag_data = DAGCreate(
            name="minimal-dag",
            nodes=[
                DAGNode(
                    id="node-1",
                    task_template=TaskCreate(name="task-1"),
                ),
            ],
        )

        # Act
        dag = await api_client.create_dag(dag_data)
        cleanup_dags.append(dag.id)

        # Assert
        assert dag.id is not None
        assert dag.name == "minimal-dag"
        assert dag.status == DAGStatus.PENDING
        assert len(dag.nodes) == 1
        assert dag.created_at is not None

    @pytest.mark.asyncio
    async def test_create_dag_with_dependencies(
        self,
        api_client: AsyncApexClient,
        cleanup_dags: list[str],
    ) -> None:
        """Test creating a DAG with node dependencies."""
        # Arrange
        dag_data = DAGCreate(
            name="dependency-dag",
            description="DAG with node dependencies",
            nodes=[
                DAGNode(
                    id="start",
                    task_template=TaskCreate(name="start-task"),
                    depends_on=[],
                ),
                DAGNode(
                    id="middle",
                    task_template=TaskCreate(name="middle-task"),
                    depends_on=["start"],
                ),
                DAGNode(
                    id="end",
                    task_template=TaskCreate(name="end-task"),
                    depends_on=["middle"],
                ),
            ],
        )

        # Act
        dag = await api_client.create_dag(dag_data)
        cleanup_dags.append(dag.id)

        # Assert
        assert len(dag.nodes) == 3
        node_map = {n.id: n for n in dag.nodes}
        assert node_map["start"].depends_on == []
        assert node_map["middle"].depends_on == ["start"]
        assert node_map["end"].depends_on == ["middle"]

    @pytest.mark.asyncio
    async def test_create_dag_with_edges(
        self,
        api_client: AsyncApexClient,
        cleanup_dags: list[str],
    ) -> None:
        """Test creating a DAG with explicit edges."""
        # Arrange
        dag_data = DAGCreate(
            name="edge-dag",
            nodes=[
                DAGNode(id="a", task_template=TaskCreate(name="task-a")),
                DAGNode(id="b", task_template=TaskCreate(name="task-b")),
                DAGNode(id="c", task_template=TaskCreate(name="task-c")),
            ],
            edges=[
                DAGEdge(source="a", target="b"),
                DAGEdge(source="a", target="c"),
                DAGEdge(source="b", target="c"),
            ],
        )

        # Act
        dag = await api_client.create_dag(dag_data)
        cleanup_dags.append(dag.id)

        # Assert
        assert len(dag.edges) == 3

    @pytest.mark.asyncio
    async def test_create_dag_full(
        self,
        api_client: AsyncApexClient,
        cleanup_dags: list[str],
    ) -> None:
        """Test creating a DAG with all fields populated."""
        # Arrange
        dag_data = DAGCreate(
            name="full-dag",
            description="A fully specified test DAG",
            nodes=[
                DAGNode(
                    id="node-1",
                    task_template=TaskCreate(name="task-1", retries=2),
                    retry_policy={"max_retries": 3, "delay": 10},
                ),
                DAGNode(
                    id="node-2",
                    task_template=TaskCreate(name="task-2"),
                    depends_on=["node-1"],
                    condition="node-1.output.success == true",
                ),
            ],
            input={"param1": "value1", "param2": 42},
            tags=["integration", "test", "full"],
            metadata={"source": "test", "version": "1.0"},
            schedule="0 */6 * * *",  # Every 6 hours
        )

        # Act
        dag = await api_client.create_dag(dag_data)
        cleanup_dags.append(dag.id)

        # Assert
        assert dag.name == "full-dag"
        assert dag.description == "A fully specified test DAG"
        assert dag.input["param1"] == "value1"
        assert set(dag.tags) == {"integration", "test", "full"}
        assert dag.schedule == "0 */6 * * *"

    @pytest.mark.asyncio
    async def test_get_dag(
        self,
        api_client: AsyncApexClient,
        cleanup_dags: list[str],
        dag_factory: callable,
    ) -> None:
        """Test retrieving a DAG by ID."""
        # Arrange
        created = await api_client.create_dag(dag_factory(name="get-test-dag"))
        cleanup_dags.append(created.id)

        # Act
        retrieved = await api_client.get_dag(created.id)

        # Assert
        assert retrieved.id == created.id
        assert retrieved.name == created.name
        assert retrieved.status == created.status

    @pytest.mark.asyncio
    async def test_get_dag_not_found(
        self,
        api_client: AsyncApexClient,
    ) -> None:
        """Test retrieving a non-existent DAG."""
        # Act & Assert
        with pytest.raises(ApexNotFoundError):
            await api_client.get_dag("non-existent-dag-id")

    @pytest.mark.asyncio
    async def test_update_dag(
        self,
        api_client: AsyncApexClient,
        cleanup_dags: list[str],
        dag_factory: callable,
    ) -> None:
        """Test updating a DAG."""
        # Arrange
        dag = await api_client.create_dag(
            dag_factory(name="update-test", description="Original")
        )
        cleanup_dags.append(dag.id)

        # Act
        updated = await api_client.update_dag(
            dag.id,
            DAGUpdate(
                description="Updated description",
                tags=["updated"],
                schedule="0 0 * * *",
            ),
        )

        # Assert
        assert updated.id == dag.id
        assert updated.description == "Updated description"
        assert "updated" in updated.tags
        assert updated.schedule == "0 0 * * *"

    @pytest.mark.asyncio
    async def test_delete_dag(
        self,
        api_client: AsyncApexClient,
        dag_factory: callable,
    ) -> None:
        """Test deleting a DAG."""
        # Arrange
        dag = await api_client.create_dag(dag_factory(name="delete-test"))

        # Act
        await api_client.delete_dag(dag.id)

        # Assert
        with pytest.raises(ApexNotFoundError):
            await api_client.get_dag(dag.id)


class TestDAGListing:
    """Tests for DAG listing and filtering."""

    @pytest.mark.asyncio
    async def test_list_dags_empty(
        self,
        api_client: AsyncApexClient,
    ) -> None:
        """Test listing DAGs with a filter that returns none."""
        # Act
        result = await api_client.list_dags(
            tags=["nonexistent-unique-dag-tag-xyz123"]
        )

        # Assert
        assert result.items == []
        assert result.total == 0

    @pytest.mark.asyncio
    async def test_list_dags_pagination(
        self,
        api_client: AsyncApexClient,
        cleanup_dags: list[str],
        dag_factory: callable,
    ) -> None:
        """Test DAG listing with pagination."""
        # Arrange - Create 5 DAGs
        for i in range(5):
            dag = await api_client.create_dag(
                dag_factory(name=f"pagination-dag-{i}", tags=["pagination-test-dag"])
            )
            cleanup_dags.append(dag.id)

        # Act - Get first page
        page1 = await api_client.list_dags(
            page=1, per_page=2, tags=["pagination-test-dag"]
        )

        # Act - Get second page
        page2 = await api_client.list_dags(
            page=2, per_page=2, tags=["pagination-test-dag"]
        )

        # Assert
        assert len(page1.items) == 2
        assert len(page2.items) == 2
        assert page1.total == 5
        page1_ids = {d.id for d in page1.items}
        page2_ids = {d.id for d in page2.items}
        assert page1_ids.isdisjoint(page2_ids)

    @pytest.mark.asyncio
    async def test_list_dags_filter_by_status(
        self,
        api_client: AsyncApexClient,
        cleanup_dags: list[str],
        dag_factory: callable,
    ) -> None:
        """Test filtering DAGs by status."""
        # Arrange
        dag = await api_client.create_dag(
            dag_factory(name="status-filter-dag", tags=["status-filter-dag"])
        )
        cleanup_dags.append(dag.id)

        # Act
        pending_dags = await api_client.list_dags(
            status="pending", tags=["status-filter-dag"]
        )

        # Assert
        assert all(d.status == DAGStatus.PENDING for d in pending_dags.items)


class TestDAGExecution:
    """Tests for DAG execution lifecycle."""

    @pytest.mark.asyncio
    async def test_start_dag(
        self,
        api_client: AsyncApexClient,
        cleanup_dags: list[str],
        dag_factory: callable,
    ) -> None:
        """Test starting a DAG execution."""
        # Arrange
        dag = await api_client.create_dag(dag_factory(name="start-test"))
        cleanup_dags.append(dag.id)

        # Act
        started = await api_client.start_dag(dag.id)

        # Assert
        assert started.status in [DAGStatus.RUNNING, DAGStatus.PENDING]

    @pytest.mark.asyncio
    async def test_start_dag_with_input(
        self,
        api_client: AsyncApexClient,
        cleanup_dags: list[str],
        dag_factory: callable,
    ) -> None:
        """Test starting a DAG with runtime input."""
        # Arrange
        dag = await api_client.create_dag(dag_factory(name="input-test"))
        cleanup_dags.append(dag.id)

        # Act
        started = await api_client.start_dag(
            dag.id,
            input_data={"runtime_param": "runtime_value"},
        )

        # Assert
        assert started.status in [DAGStatus.RUNNING, DAGStatus.PENDING]

    @pytest.mark.asyncio
    async def test_cancel_dag(
        self,
        api_client: AsyncApexClient,
        cleanup_dags: list[str],
        dag_factory: callable,
    ) -> None:
        """Test cancelling a DAG."""
        # Arrange
        dag = await api_client.create_dag(dag_factory(name="cancel-test"))
        cleanup_dags.append(dag.id)
        await api_client.start_dag(dag.id)

        # Act
        cancelled = await api_client.cancel_dag(dag.id)

        # Assert
        assert cancelled.status == DAGStatus.CANCELLED

    @pytest.mark.asyncio
    async def test_pause_dag(
        self,
        api_client: AsyncApexClient,
        cleanup_dags: list[str],
        dag_factory: callable,
    ) -> None:
        """Test pausing a running DAG."""
        # Arrange
        dag = await api_client.create_dag(dag_factory(name="pause-test"))
        cleanup_dags.append(dag.id)
        await api_client.start_dag(dag.id)

        # Act
        paused = await api_client.pause_dag(dag.id)

        # Assert
        assert paused.status == DAGStatus.PAUSED

    @pytest.mark.asyncio
    async def test_resume_dag(
        self,
        api_client: AsyncApexClient,
        cleanup_dags: list[str],
        dag_factory: callable,
    ) -> None:
        """Test resuming a paused DAG."""
        # Arrange
        dag = await api_client.create_dag(dag_factory(name="resume-test"))
        cleanup_dags.append(dag.id)
        await api_client.start_dag(dag.id)
        await api_client.pause_dag(dag.id)

        # Act
        resumed = await api_client.resume_dag(dag.id)

        # Assert
        assert resumed.status == DAGStatus.RUNNING


class TestDAGStructure:
    """Tests for DAG structure validation and processing."""

    @pytest.mark.asyncio
    async def test_create_dag_parallel_nodes(
        self,
        api_client: AsyncApexClient,
        cleanup_dags: list[str],
    ) -> None:
        """Test creating a DAG with parallel nodes."""
        # Arrange - Diamond pattern: A -> (B, C) -> D
        dag_data = DAGCreate(
            name="parallel-dag",
            nodes=[
                DAGNode(id="a", task_template=TaskCreate(name="task-a")),
                DAGNode(id="b", task_template=TaskCreate(name="task-b"), depends_on=["a"]),
                DAGNode(id="c", task_template=TaskCreate(name="task-c"), depends_on=["a"]),
                DAGNode(id="d", task_template=TaskCreate(name="task-d"), depends_on=["b", "c"]),
            ],
        )

        # Act
        dag = await api_client.create_dag(dag_data)
        cleanup_dags.append(dag.id)

        # Assert
        assert len(dag.nodes) == 4
        node_map = {n.id: n for n in dag.nodes}
        assert node_map["d"].depends_on == ["b", "c"]

    @pytest.mark.asyncio
    async def test_create_dag_conditional_edges(
        self,
        api_client: AsyncApexClient,
        cleanup_dags: list[str],
    ) -> None:
        """Test creating a DAG with conditional edges."""
        # Arrange
        dag_data = DAGCreate(
            name="conditional-dag",
            nodes=[
                DAGNode(id="check", task_template=TaskCreate(name="check-task")),
                DAGNode(
                    id="success",
                    task_template=TaskCreate(name="success-task"),
                    depends_on=["check"],
                    condition="check.output.result == 'success'",
                ),
                DAGNode(
                    id="failure",
                    task_template=TaskCreate(name="failure-task"),
                    depends_on=["check"],
                    condition="check.output.result == 'failure'",
                ),
            ],
        )

        # Act
        dag = await api_client.create_dag(dag_data)
        cleanup_dags.append(dag.id)

        # Assert
        node_map = {n.id: n for n in dag.nodes}
        assert node_map["success"].condition is not None
        assert node_map["failure"].condition is not None


class TestDAGDatabaseState:
    """Tests that verify database state after DAG operations."""

    @pytest.mark.asyncio
    @pytest.mark.database
    async def test_dag_creation_persists_to_database(
        self,
        api_client: AsyncApexClient,
        cleanup_dags: list[str],
        db_connection: asyncpg.Connection,
        dag_factory: callable,
    ) -> None:
        """Verify DAG creation persists correctly to the database."""
        # Arrange & Act
        dag = await api_client.create_dag(
            dag_factory(name="db-persist-dag", description="Testing DB persistence")
        )
        cleanup_dags.append(dag.id)

        # Assert - Query database directly
        row = await db_connection.fetchrow(
            "SELECT * FROM dags WHERE id = $1",
            dag.id,
        )

        assert row is not None
        assert row["name"] == "db-persist-dag"
        assert row["status"] == "pending"

    @pytest.mark.asyncio
    @pytest.mark.database
    async def test_dag_execution_creates_tasks(
        self,
        api_client: AsyncApexClient,
        cleanup_dags: list[str],
        db_connection: asyncpg.Connection,
        dag_factory: callable,
    ) -> None:
        """Verify starting a DAG creates tasks in the database."""
        # Arrange
        dag = await api_client.create_dag(dag_factory(name="task-creation-dag"))
        cleanup_dags.append(dag.id)

        # Act
        await api_client.start_dag(dag.id)

        # Wait a bit for task creation
        await asyncio.sleep(0.5)

        # Assert - Check for created tasks
        rows = await db_connection.fetch(
            "SELECT * FROM tasks WHERE dag_id = $1",
            dag.id,
        )

        # Should have created tasks for each node
        assert len(rows) >= 1


class TestDAGValidation:
    """Tests for DAG input validation."""

    @pytest.mark.asyncio
    async def test_create_dag_empty_name_fails(
        self,
        api_client: AsyncApexClient,
    ) -> None:
        """Test that creating a DAG with empty name fails."""
        # Act & Assert
        with pytest.raises(ApexValidationError):
            await api_client.create_dag(
                DAGCreate(
                    name="",
                    nodes=[DAGNode(id="n", task_template=TaskCreate(name="t"))],
                )
            )

    @pytest.mark.asyncio
    async def test_create_dag_no_nodes_fails(
        self,
        api_client: AsyncApexClient,
    ) -> None:
        """Test that creating a DAG with no nodes fails."""
        # Act & Assert
        with pytest.raises(ApexValidationError):
            await api_client.create_dag(
                DAGCreate(name="empty-dag", nodes=[])
            )

    @pytest.mark.asyncio
    async def test_create_dag_duplicate_node_ids_fails(
        self,
        api_client: AsyncApexClient,
    ) -> None:
        """Test that creating a DAG with duplicate node IDs fails."""
        # Act & Assert
        with pytest.raises(ApexValidationError):
            await api_client.create_dag(
                DAGCreate(
                    name="duplicate-nodes",
                    nodes=[
                        DAGNode(id="same", task_template=TaskCreate(name="t1")),
                        DAGNode(id="same", task_template=TaskCreate(name="t2")),
                    ],
                )
            )

    @pytest.mark.asyncio
    async def test_create_dag_invalid_dependency_fails(
        self,
        api_client: AsyncApexClient,
    ) -> None:
        """Test that creating a DAG with invalid dependencies fails."""
        # Act & Assert
        with pytest.raises(ApexValidationError):
            await api_client.create_dag(
                DAGCreate(
                    name="invalid-dep",
                    nodes=[
                        DAGNode(
                            id="node-1",
                            task_template=TaskCreate(name="t1"),
                            depends_on=["nonexistent"],
                        ),
                    ],
                )
            )

    @pytest.mark.asyncio
    async def test_create_dag_cyclic_dependency_fails(
        self,
        api_client: AsyncApexClient,
    ) -> None:
        """Test that creating a DAG with cyclic dependencies fails."""
        # Act & Assert
        with pytest.raises(ApexValidationError):
            await api_client.create_dag(
                DAGCreate(
                    name="cyclic-dag",
                    nodes=[
                        DAGNode(id="a", task_template=TaskCreate(name="t1"), depends_on=["c"]),
                        DAGNode(id="b", task_template=TaskCreate(name="t2"), depends_on=["a"]),
                        DAGNode(id="c", task_template=TaskCreate(name="t3"), depends_on=["b"]),
                    ],
                )
            )


class TestDAGConcurrency:
    """Tests for concurrent DAG operations."""

    @pytest.mark.asyncio
    @pytest.mark.slow
    async def test_concurrent_dag_creation(
        self,
        api_client: AsyncApexClient,
        cleanup_dags: list[str],
        dag_factory: callable,
    ) -> None:
        """Test creating multiple DAGs concurrently."""
        # Arrange
        num_dags = 5
        dag_data = [
            dag_factory(name=f"concurrent-dag-{i}", tags=["concurrent-dag-test"])
            for i in range(num_dags)
        ]

        # Act
        dags = await asyncio.gather(
            *[api_client.create_dag(data) for data in dag_data]
        )

        # Cleanup
        for dag in dags:
            cleanup_dags.append(dag.id)

        # Assert
        assert len(dags) == num_dags
        assert len(set(d.id for d in dags)) == num_dags

    @pytest.mark.asyncio
    @pytest.mark.slow
    async def test_concurrent_dag_starts(
        self,
        api_client: AsyncApexClient,
        cleanup_dags: list[str],
        dag_factory: callable,
    ) -> None:
        """Test starting multiple DAGs concurrently."""
        # Arrange - Create multiple DAGs first
        dags = []
        for i in range(3):
            dag = await api_client.create_dag(
                dag_factory(name=f"concurrent-start-{i}")
            )
            dags.append(dag)
            cleanup_dags.append(dag.id)

        # Act - Start all concurrently
        results = await asyncio.gather(
            *[api_client.start_dag(dag.id) for dag in dags]
        )

        # Assert
        assert all(r.status in [DAGStatus.RUNNING, DAGStatus.PENDING] for r in results)
