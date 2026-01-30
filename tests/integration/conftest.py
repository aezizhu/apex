"""Pytest fixtures and configuration for Apex integration tests."""

from __future__ import annotations

import asyncio
import os
import uuid
from collections.abc import AsyncGenerator, Generator
from datetime import datetime, timezone
from typing import Any

import asyncpg
import pytest
import pytest_asyncio
import redis.asyncio as aioredis

# Import from the Apex SDK
# Note: Ensure apex_sdk is installed or PYTHONPATH includes sdk/python
import sys
sys.path.insert(0, os.path.join(os.path.dirname(__file__), '..', '..', 'sdk', 'python'))

from apex_sdk import ApexClient, AsyncApexClient
from apex_sdk.models import (
    Agent,
    AgentCreate,
    AgentStatus,
    Approval,
    ApprovalCreate,
    DAG,
    DAGCreate,
    DAGNode,
    Task,
    TaskCreate,
    TaskPriority,
    TaskStatus,
    WebSocketEventType,
)
from apex_sdk.websocket import ApexWebSocketClient


# ══════════════════════════════════════════════════════════════════════════════
# Configuration
# ══════════════════════════════════════════════════════════════════════════════

def get_test_config() -> dict[str, Any]:
    """Get test configuration from environment variables."""
    return {
        "api_url": os.environ.get("TEST_API_URL", "http://localhost:8081"),
        "ws_url": os.environ.get("TEST_WS_URL", "ws://localhost:8081/ws"),
        "api_key": os.environ.get("TEST_API_KEY", "test-api-key"),
        "db_url": os.environ.get(
            "TEST_DB_URL",
            "postgres://apex:apex_test@localhost:5433/apex_test"
        ),
        "redis_url": os.environ.get("TEST_REDIS_URL", "redis://localhost:6380"),
        "timeout": int(os.environ.get("TEST_TIMEOUT", "30")),
    }


# ══════════════════════════════════════════════════════════════════════════════
# Session-scoped fixtures
# ══════════════════════════════════════════════════════════════════════════════

@pytest.fixture(scope="session")
def event_loop() -> Generator[asyncio.AbstractEventLoop, None, None]:
    """Create an event loop for the test session."""
    loop = asyncio.new_event_loop()
    yield loop
    loop.close()


@pytest.fixture(scope="session")
def test_config() -> dict[str, Any]:
    """Provide test configuration."""
    return get_test_config()


@pytest_asyncio.fixture(scope="session")
async def db_pool(test_config: dict[str, Any]) -> AsyncGenerator[asyncpg.Pool, None]:
    """Create a database connection pool for the test session."""
    pool = await asyncpg.create_pool(
        test_config["db_url"],
        min_size=1,
        max_size=5,
        command_timeout=30,
    )
    yield pool
    await pool.close()


@pytest_asyncio.fixture(scope="session")
async def redis_pool(test_config: dict[str, Any]) -> AsyncGenerator[aioredis.Redis, None]:
    """Create a Redis connection for the test session."""
    redis_client = aioredis.from_url(
        test_config["redis_url"],
        encoding="utf-8",
        decode_responses=True,
    )
    yield redis_client
    await redis_client.aclose()


# ══════════════════════════════════════════════════════════════════════════════
# Function-scoped fixtures
# ══════════════════════════════════════════════════════════════════════════════

@pytest_asyncio.fixture
async def api_client(test_config: dict[str, Any]) -> AsyncGenerator[AsyncApexClient, None]:
    """Provide an async API client for each test."""
    client = AsyncApexClient(
        base_url=test_config["api_url"],
        api_key=test_config["api_key"],
        timeout=float(test_config["timeout"]),
    )
    yield client
    await client.close()


@pytest.fixture
def sync_api_client(test_config: dict[str, Any]) -> Generator[ApexClient, None, None]:
    """Provide a sync API client for each test."""
    client = ApexClient(
        base_url=test_config["api_url"],
        api_key=test_config["api_key"],
        timeout=float(test_config["timeout"]),
    )
    yield client
    client.close()


@pytest_asyncio.fixture
async def ws_client(test_config: dict[str, Any]) -> AsyncGenerator[ApexWebSocketClient, None]:
    """Provide a WebSocket client for each test."""
    client = ApexWebSocketClient(
        base_url=test_config["api_url"],
        api_key=test_config["api_key"],
        reconnect=False,  # Disable auto-reconnect for tests
    )
    yield client
    await client.disconnect()


@pytest_asyncio.fixture
async def db_connection(
    db_pool: asyncpg.Pool,
) -> AsyncGenerator[asyncpg.Connection, None]:
    """Provide a database connection for each test."""
    async with db_pool.acquire() as connection:
        yield connection


@pytest_asyncio.fixture
async def redis_client(
    redis_pool: aioredis.Redis,
) -> AsyncGenerator[aioredis.Redis, None]:
    """Provide a Redis client for each test."""
    yield redis_pool


# ══════════════════════════════════════════════════════════════════════════════
# Cleanup fixtures
# ══════════════════════════════════════════════════════════════════════════════

@pytest_asyncio.fixture
async def cleanup_tasks(
    api_client: AsyncApexClient,
) -> AsyncGenerator[list[str], None]:
    """Track and cleanup created tasks after test."""
    task_ids: list[str] = []
    yield task_ids
    for task_id in task_ids:
        try:
            await api_client.delete_task(task_id)
        except Exception:
            pass  # Ignore cleanup errors


@pytest_asyncio.fixture
async def cleanup_agents(
    api_client: AsyncApexClient,
) -> AsyncGenerator[list[str], None]:
    """Track and cleanup created agents after test."""
    agent_ids: list[str] = []
    yield agent_ids
    for agent_id in agent_ids:
        try:
            await api_client.delete_agent(agent_id)
        except Exception:
            pass  # Ignore cleanup errors


@pytest_asyncio.fixture
async def cleanup_dags(
    api_client: AsyncApexClient,
) -> AsyncGenerator[list[str], None]:
    """Track and cleanup created DAGs after test."""
    dag_ids: list[str] = []
    yield dag_ids
    for dag_id in dag_ids:
        try:
            await api_client.delete_dag(dag_id)
        except Exception:
            pass  # Ignore cleanup errors


# ══════════════════════════════════════════════════════════════════════════════
# Factory fixtures
# ══════════════════════════════════════════════════════════════════════════════

@pytest.fixture
def task_factory() -> callable:
    """Factory for creating TaskCreate objects."""
    def _create_task(
        name: str | None = None,
        description: str | None = None,
        priority: TaskPriority = TaskPriority.NORMAL,
        **kwargs: Any,
    ) -> TaskCreate:
        return TaskCreate(
            name=name or f"test-task-{uuid.uuid4().hex[:8]}",
            description=description or "Integration test task",
            priority=priority,
            **kwargs,
        )
    return _create_task


@pytest.fixture
def agent_factory() -> callable:
    """Factory for creating AgentCreate objects."""
    def _create_agent(
        name: str | None = None,
        description: str | None = None,
        **kwargs: Any,
    ) -> AgentCreate:
        return AgentCreate(
            name=name or f"test-agent-{uuid.uuid4().hex[:8]}",
            description=description or "Integration test agent",
            capabilities=[],
            max_concurrent_tasks=1,
            **kwargs,
        )
    return _create_agent


@pytest.fixture
def dag_factory(task_factory: callable) -> callable:
    """Factory for creating DAGCreate objects."""
    def _create_dag(
        name: str | None = None,
        description: str | None = None,
        nodes: list[DAGNode] | None = None,
        **kwargs: Any,
    ) -> DAGCreate:
        if nodes is None:
            # Create a simple 2-node DAG
            nodes = [
                DAGNode(
                    id="node-1",
                    task_template=task_factory(name="dag-task-1"),
                    depends_on=[],
                ),
                DAGNode(
                    id="node-2",
                    task_template=task_factory(name="dag-task-2"),
                    depends_on=["node-1"],
                ),
            ]
        return DAGCreate(
            name=name or f"test-dag-{uuid.uuid4().hex[:8]}",
            description=description or "Integration test DAG",
            nodes=nodes,
            **kwargs,
        )
    return _create_dag


# ══════════════════════════════════════════════════════════════════════════════
# Helper fixtures
# ══════════════════════════════════════════════════════════════════════════════

@pytest.fixture
def unique_id() -> str:
    """Generate a unique identifier for test isolation."""
    return uuid.uuid4().hex[:12]


@pytest.fixture
def current_timestamp() -> datetime:
    """Provide the current UTC timestamp."""
    return datetime.now(timezone.utc)


# ══════════════════════════════════════════════════════════════════════════════
# Database helper fixtures
# ══════════════════════════════════════════════════════════════════════════════

@pytest_asyncio.fixture
async def clear_test_data(db_connection: asyncpg.Connection) -> AsyncGenerator[None, None]:
    """Clear test data before and after test (use with caution)."""
    # Clean before test
    await _cleanup_test_tables(db_connection)
    yield
    # Clean after test
    await _cleanup_test_tables(db_connection)


async def _cleanup_test_tables(conn: asyncpg.Connection) -> None:
    """Clean up test data from tables."""
    # Delete in order to respect foreign key constraints
    tables = ["approvals", "dag_task_statuses", "tasks", "dags", "agents"]
    for table in tables:
        try:
            await conn.execute(f"DELETE FROM {table} WHERE true")
        except Exception:
            pass  # Table might not exist or other issues


# ══════════════════════════════════════════════════════════════════════════════
# Async utilities
# ══════════════════════════════════════════════════════════════════════════════

@pytest.fixture
def wait_for() -> callable:
    """Utility to wait for a condition with timeout."""
    async def _wait_for(
        condition: callable,
        timeout: float = 10.0,
        interval: float = 0.1,
    ) -> bool:
        """
        Wait for a condition to become true.

        Args:
            condition: Async callable that returns True when condition is met
            timeout: Maximum time to wait in seconds
            interval: Time between checks in seconds

        Returns:
            True if condition was met, False if timeout
        """
        import asyncio
        start_time = asyncio.get_event_loop().time()
        while asyncio.get_event_loop().time() - start_time < timeout:
            if await condition():
                return True
            await asyncio.sleep(interval)
        return False
    return _wait_for


# ══════════════════════════════════════════════════════════════════════════════
# Markers
# ══════════════════════════════════════════════════════════════════════════════

def pytest_configure(config: pytest.Config) -> None:
    """Configure custom pytest markers."""
    config.addinivalue_line(
        "markers", "slow: marks tests as slow (deselect with '-m \"not slow\"')"
    )
    config.addinivalue_line(
        "markers", "websocket: marks tests that require WebSocket connections"
    )
    config.addinivalue_line(
        "markers", "database: marks tests that directly interact with the database"
    )
    config.addinivalue_line(
        "markers", "workflow: marks end-to-end workflow tests"
    )


# ══════════════════════════════════════════════════════════════════════════════
# Test data fixtures
# ══════════════════════════════════════════════════════════════════════════════

@pytest.fixture
def sample_task_input() -> dict[str, Any]:
    """Provide sample task input data."""
    return {
        "data": {"key": "value", "number": 42},
        "parameters": {"format": "json", "verbose": True},
    }


@pytest.fixture
def sample_task_metadata() -> dict[str, Any]:
    """Provide sample task metadata."""
    return {
        "source": "integration-test",
        "environment": "test",
        "version": "1.0.0",
    }


@pytest.fixture
def sample_agent_capabilities() -> list[dict[str, Any]]:
    """Provide sample agent capabilities."""
    return [
        {"name": "web_search", "version": "1.0"},
        {"name": "code_execution", "version": "2.1"},
        {"name": "file_operations", "version": "1.5"},
    ]
