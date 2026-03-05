"""Integration tests for the Agent API endpoints."""

from __future__ import annotations

import asyncio
from typing import Any

import asyncpg
import pytest
import pytest_asyncio

from apex_sdk import AsyncApexClient
from apex_sdk.exceptions import ApexNotFoundError, ApexValidationError
from apex_sdk.models import (
    Agent,
    AgentCapability,
    AgentCreate,
    AgentStatus,
    AgentUpdate,
)


class TestAgentCRUD:
    """Tests for Agent CRUD operations."""

    @pytest.mark.asyncio
    async def test_create_agent_minimal(
        self,
        api_client: AsyncApexClient,
        cleanup_agents: list[str],
    ) -> None:
        """Test creating an agent with minimal required fields."""
        # Arrange
        agent_data = AgentCreate(name="minimal-agent")

        # Act
        agent = await api_client.create_agent(agent_data)
        cleanup_agents.append(agent.id)

        # Assert
        assert agent.id is not None
        assert agent.name == "minimal-agent"
        assert agent.status == AgentStatus.IDLE
        assert agent.max_concurrent_tasks == 1
        assert agent.created_at is not None

    @pytest.mark.asyncio
    async def test_create_agent_full(
        self,
        api_client: AsyncApexClient,
        cleanup_agents: list[str],
        sample_agent_capabilities: list[dict[str, Any]],
    ) -> None:
        """Test creating an agent with all fields populated."""
        # Arrange
        capabilities = [
            AgentCapability(**cap) for cap in sample_agent_capabilities
        ]
        agent_data = AgentCreate(
            name="full-agent",
            description="A fully specified test agent",
            capabilities=capabilities,
            max_concurrent_tasks=5,
            tags=["integration", "test", "full"],
            metadata={"team": "platform", "tier": "standard"},
        )

        # Act
        agent = await api_client.create_agent(agent_data)
        cleanup_agents.append(agent.id)

        # Assert
        assert agent.name == "full-agent"
        assert agent.description == "A fully specified test agent"
        assert len(agent.capabilities) == 3
        assert agent.max_concurrent_tasks == 5
        assert set(agent.tags) == {"integration", "test", "full"}
        assert agent.metadata["team"] == "platform"

    @pytest.mark.asyncio
    async def test_get_agent(
        self,
        api_client: AsyncApexClient,
        cleanup_agents: list[str],
    ) -> None:
        """Test retrieving an agent by ID."""
        # Arrange
        created = await api_client.create_agent(AgentCreate(name="get-test-agent"))
        cleanup_agents.append(created.id)

        # Act
        retrieved = await api_client.get_agent(created.id)

        # Assert
        assert retrieved.id == created.id
        assert retrieved.name == created.name
        assert retrieved.status == created.status

    @pytest.mark.asyncio
    async def test_get_agent_not_found(
        self,
        api_client: AsyncApexClient,
    ) -> None:
        """Test retrieving a non-existent agent."""
        # Act & Assert
        with pytest.raises(ApexNotFoundError):
            await api_client.get_agent("non-existent-agent-id")

    @pytest.mark.asyncio
    async def test_update_agent(
        self,
        api_client: AsyncApexClient,
        cleanup_agents: list[str],
    ) -> None:
        """Test updating an agent."""
        # Arrange
        agent = await api_client.create_agent(
            AgentCreate(name="update-test", description="Original")
        )
        cleanup_agents.append(agent.id)

        # Act
        updated = await api_client.update_agent(
            agent.id,
            AgentUpdate(
                description="Updated description",
                max_concurrent_tasks=10,
                tags=["updated"],
            ),
        )

        # Assert
        assert updated.id == agent.id
        assert updated.description == "Updated description"
        assert updated.max_concurrent_tasks == 10
        assert "updated" in updated.tags

    @pytest.mark.asyncio
    async def test_update_agent_status(
        self,
        api_client: AsyncApexClient,
        cleanup_agents: list[str],
    ) -> None:
        """Test updating agent status."""
        # Arrange
        agent = await api_client.create_agent(AgentCreate(name="status-update-test"))
        cleanup_agents.append(agent.id)
        assert agent.status == AgentStatus.IDLE

        # Act
        updated = await api_client.update_agent(
            agent.id,
            AgentUpdate(status=AgentStatus.BUSY),
        )

        # Assert
        assert updated.status == AgentStatus.BUSY

    @pytest.mark.asyncio
    async def test_delete_agent(
        self,
        api_client: AsyncApexClient,
    ) -> None:
        """Test deleting an agent."""
        # Arrange
        agent = await api_client.create_agent(AgentCreate(name="delete-test"))

        # Act
        await api_client.delete_agent(agent.id)

        # Assert
        with pytest.raises(ApexNotFoundError):
            await api_client.get_agent(agent.id)


class TestAgentListing:
    """Tests for agent listing and filtering."""

    @pytest.mark.asyncio
    async def test_list_agents_empty(
        self,
        api_client: AsyncApexClient,
    ) -> None:
        """Test listing agents with a filter that returns none."""
        # Act
        result = await api_client.list_agents(
            tags=["nonexistent-unique-agent-tag-xyz123"]
        )

        # Assert
        assert result.items == []
        assert result.total == 0

    @pytest.mark.asyncio
    async def test_list_agents_pagination(
        self,
        api_client: AsyncApexClient,
        cleanup_agents: list[str],
    ) -> None:
        """Test agent listing with pagination."""
        # Arrange - Create 5 agents
        for i in range(5):
            agent = await api_client.create_agent(
                AgentCreate(name=f"pagination-test-{i}", tags=["pagination-test-agent"])
            )
            cleanup_agents.append(agent.id)

        # Act - Get first page
        page1 = await api_client.list_agents(
            page=1, per_page=2, tags=["pagination-test-agent"]
        )

        # Act - Get second page
        page2 = await api_client.list_agents(
            page=2, per_page=2, tags=["pagination-test-agent"]
        )

        # Assert
        assert len(page1.items) == 2
        assert len(page2.items) == 2
        assert page1.total == 5
        assert page1.total_pages == 3
        # Ensure different items on different pages
        page1_ids = {a.id for a in page1.items}
        page2_ids = {a.id for a in page2.items}
        assert page1_ids.isdisjoint(page2_ids)

    @pytest.mark.asyncio
    async def test_list_agents_filter_by_status(
        self,
        api_client: AsyncApexClient,
        cleanup_agents: list[str],
    ) -> None:
        """Test filtering agents by status."""
        # Arrange - Create an agent and set to busy
        agent = await api_client.create_agent(
            AgentCreate(name="status-filter-test", tags=["status-filter-agent"])
        )
        cleanup_agents.append(agent.id)
        await api_client.update_agent(agent.id, AgentUpdate(status=AgentStatus.BUSY))

        # Act
        busy_agents = await api_client.list_agents(
            status="busy", tags=["status-filter-agent"]
        )

        # Assert
        assert all(a.status == AgentStatus.BUSY for a in busy_agents.items)
        assert agent.id in [a.id for a in busy_agents.items]

    @pytest.mark.asyncio
    async def test_list_agents_filter_by_tags(
        self,
        api_client: AsyncApexClient,
        cleanup_agents: list[str],
    ) -> None:
        """Test filtering agents by tags."""
        # Arrange
        unique_tag = "unique-agent-tag-def456"
        agent = await api_client.create_agent(
            AgentCreate(name="tag-filter-test", tags=[unique_tag, "common-agent"])
        )
        cleanup_agents.append(agent.id)

        # Also create an agent without the unique tag
        other_agent = await api_client.create_agent(
            AgentCreate(name="other-agent", tags=["common-agent"])
        )
        cleanup_agents.append(other_agent.id)

        # Act
        filtered = await api_client.list_agents(tags=[unique_tag])

        # Assert
        assert len(filtered.items) == 1
        assert filtered.items[0].id == agent.id


class TestAgentCapabilities:
    """Tests for agent capability management."""

    @pytest.mark.asyncio
    async def test_create_agent_with_capabilities(
        self,
        api_client: AsyncApexClient,
        cleanup_agents: list[str],
    ) -> None:
        """Test creating an agent with multiple capabilities."""
        # Arrange
        capabilities = [
            AgentCapability(name="web_search", version="1.0"),
            AgentCapability(name="code_execution", version="2.0", parameters={"timeout": 30}),
        ]
        agent_data = AgentCreate(
            name="capable-agent",
            capabilities=capabilities,
        )

        # Act
        agent = await api_client.create_agent(agent_data)
        cleanup_agents.append(agent.id)

        # Assert
        assert len(agent.capabilities) == 2
        cap_names = {cap.name for cap in agent.capabilities}
        assert cap_names == {"web_search", "code_execution"}

    @pytest.mark.asyncio
    async def test_update_agent_capabilities(
        self,
        api_client: AsyncApexClient,
        cleanup_agents: list[str],
    ) -> None:
        """Test updating agent capabilities."""
        # Arrange
        agent = await api_client.create_agent(
            AgentCreate(
                name="update-cap-test",
                capabilities=[AgentCapability(name="original_cap")],
            )
        )
        cleanup_agents.append(agent.id)

        # Act
        updated = await api_client.update_agent(
            agent.id,
            AgentUpdate(
                capabilities=[
                    AgentCapability(name="new_cap_1"),
                    AgentCapability(name="new_cap_2"),
                ]
            ),
        )

        # Assert
        assert len(updated.capabilities) == 2
        cap_names = {cap.name for cap in updated.capabilities}
        assert cap_names == {"new_cap_1", "new_cap_2"}


class TestAgentDatabaseState:
    """Tests that verify database state after agent operations."""

    @pytest.mark.asyncio
    @pytest.mark.database
    async def test_agent_creation_persists_to_database(
        self,
        api_client: AsyncApexClient,
        cleanup_agents: list[str],
        db_connection: asyncpg.Connection,
    ) -> None:
        """Verify agent creation persists correctly to the database."""
        # Arrange & Act
        agent = await api_client.create_agent(
            AgentCreate(
                name="db-persist-agent",
                description="Testing DB persistence",
                max_concurrent_tasks=3,
            )
        )
        cleanup_agents.append(agent.id)

        # Assert - Query database directly
        row = await db_connection.fetchrow(
            "SELECT * FROM agents WHERE id = $1",
            agent.id,
        )

        assert row is not None
        assert row["name"] == "db-persist-agent"
        assert row["description"] == "Testing DB persistence"
        assert row["status"] == "idle"
        assert row["max_concurrent_tasks"] == 3

    @pytest.mark.asyncio
    @pytest.mark.database
    async def test_agent_status_change_persists(
        self,
        api_client: AsyncApexClient,
        cleanup_agents: list[str],
        db_connection: asyncpg.Connection,
    ) -> None:
        """Verify agent status changes persist to database."""
        # Arrange
        agent = await api_client.create_agent(AgentCreate(name="db-status-test"))
        cleanup_agents.append(agent.id)

        # Act
        await api_client.update_agent(
            agent.id,
            AgentUpdate(status=AgentStatus.OFFLINE),
        )

        # Assert - Query database directly
        row = await db_connection.fetchrow(
            "SELECT status FROM agents WHERE id = $1",
            agent.id,
        )

        assert row["status"] == "offline"

    @pytest.mark.asyncio
    @pytest.mark.database
    async def test_agent_deletion_removes_from_database(
        self,
        api_client: AsyncApexClient,
        db_connection: asyncpg.Connection,
    ) -> None:
        """Verify agent deletion removes the record from database."""
        # Arrange
        agent = await api_client.create_agent(AgentCreate(name="db-delete-agent"))

        # Act
        await api_client.delete_agent(agent.id)

        # Assert - Query database directly
        row = await db_connection.fetchrow(
            "SELECT * FROM agents WHERE id = $1",
            agent.id,
        )

        assert row is None


class TestAgentValidation:
    """Tests for agent input validation."""

    @pytest.mark.asyncio
    async def test_create_agent_empty_name_fails(
        self,
        api_client: AsyncApexClient,
    ) -> None:
        """Test that creating an agent with empty name fails."""
        # Act & Assert
        with pytest.raises(ApexValidationError):
            await api_client.create_agent(AgentCreate(name=""))

    @pytest.mark.asyncio
    async def test_create_agent_long_name_fails(
        self,
        api_client: AsyncApexClient,
    ) -> None:
        """Test that creating an agent with too long name fails."""
        # Act & Assert
        with pytest.raises(ApexValidationError):
            await api_client.create_agent(AgentCreate(name="x" * 300))

    @pytest.mark.asyncio
    async def test_create_agent_invalid_max_concurrent_tasks(
        self,
        api_client: AsyncApexClient,
    ) -> None:
        """Test that creating an agent with invalid max_concurrent_tasks fails."""
        # Act & Assert - Zero tasks
        with pytest.raises(ApexValidationError):
            await api_client.create_agent(
                AgentCreate(name="zero-tasks", max_concurrent_tasks=0)
            )

        # Act & Assert - Too many tasks
        with pytest.raises(ApexValidationError):
            await api_client.create_agent(
                AgentCreate(name="too-many-tasks", max_concurrent_tasks=1000)
            )


class TestAgentConcurrency:
    """Tests for concurrent agent operations."""

    @pytest.mark.asyncio
    @pytest.mark.slow
    async def test_concurrent_agent_creation(
        self,
        api_client: AsyncApexClient,
        cleanup_agents: list[str],
    ) -> None:
        """Test creating multiple agents concurrently."""
        # Arrange
        num_agents = 10
        agent_data = [
            AgentCreate(name=f"concurrent-agent-{i}", tags=["concurrent-agent-test"])
            for i in range(num_agents)
        ]

        # Act
        agents = await asyncio.gather(
            *[api_client.create_agent(data) for data in agent_data]
        )

        # Cleanup
        for agent in agents:
            cleanup_agents.append(agent.id)

        # Assert
        assert len(agents) == num_agents
        assert len(set(a.id for a in agents)) == num_agents  # All unique IDs

    @pytest.mark.asyncio
    @pytest.mark.slow
    async def test_concurrent_agent_status_updates(
        self,
        api_client: AsyncApexClient,
        cleanup_agents: list[str],
    ) -> None:
        """Test updating agent status concurrently."""
        # Arrange - Create multiple agents
        agents = []
        for i in range(5):
            agent = await api_client.create_agent(
                AgentCreate(name=f"concurrent-status-{i}")
            )
            agents.append(agent)
            cleanup_agents.append(agent.id)

        # Act - Update all to busy concurrently
        updates = [
            api_client.update_agent(
                agent.id,
                AgentUpdate(status=AgentStatus.BUSY),
            )
            for agent in agents
        ]

        results = await asyncio.gather(*updates)

        # Assert
        assert all(r.status == AgentStatus.BUSY for r in results)


class TestAgentTaskRelationships:
    """Tests for agent-task relationships."""

    @pytest.mark.asyncio
    async def test_agent_task_count_tracking(
        self,
        api_client: AsyncApexClient,
        cleanup_agents: list[str],
        cleanup_tasks: list[str],
    ) -> None:
        """Test that agent tracks current and completed task counts."""
        # Arrange
        agent = await api_client.create_agent(
            AgentCreate(name="task-count-agent", max_concurrent_tasks=5)
        )
        cleanup_agents.append(agent.id)

        # Create a task assigned to this agent
        from apex_sdk.models import TaskCreate
        task = await api_client.create_task(
            TaskCreate(name="agent-task", agent_id=agent.id)
        )
        cleanup_tasks.append(task.id)

        # Act - Get agent after task assignment
        updated_agent = await api_client.get_agent(agent.id)

        # Assert
        assert updated_agent.current_tasks >= 0
        assert updated_agent.total_tasks_completed >= 0
