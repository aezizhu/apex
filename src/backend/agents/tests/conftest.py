"""Shared pytest fixtures for Apex Agents tests."""

import os
from unittest.mock import AsyncMock, MagicMock, patch

import pytest


@pytest.fixture(autouse=True)
def mock_env():
    """Set up test environment variables."""
    env_vars = {
        "APEX_LLM_OPENAI_API_KEY": "sk-test-key-12345",
        "APEX_ENVIRONMENT": "development",
        "APEX_DEBUG": "false",
        "APEX_LOG_LEVEL": "INFO",
        "APEX_LOG_JSON": "false",
        "APEX_BACKEND_HOST": "localhost",
        "APEX_BACKEND_HTTP_PORT": "8080",
        "APEX_REDIS_URL": "redis://localhost:6379",
        "APEX_TRACING_ENABLED": "false",
    }

    with patch.dict(os.environ, env_vars, clear=False):
        yield


@pytest.fixture
def mock_llm_response():
    """Create a mock LLM response."""
    from apex_agents.llm import LLMResponse, LLMUsage

    return LLMResponse(
        content="This is a test response.",
        tool_calls=[],
        usage=LLMUsage(prompt_tokens=50, completion_tokens=20, total_tokens=70),
        model="gpt-4o-mini",
        cost=0.001,
        finish_reason="stop",
    )


@pytest.fixture
def mock_llm_client(mock_llm_response):
    """Create a mock LLM client."""
    from apex_agents.llm import LLMClient

    client = AsyncMock(spec=LLMClient)
    client.create.return_value = mock_llm_response
    return client


@pytest.fixture
def sample_agent_config():
    """Create a sample agent configuration."""
    from apex_agents.agent import AgentConfig

    return AgentConfig(
        name="test-agent",
        model="gpt-4o-mini",
        system_prompt="You are a helpful assistant for testing.",
        tools=["web_search", "read_file"],
        max_iterations=5,
        temperature=0.7,
    )


@pytest.fixture
def mock_tool_registry():
    """Create a mock tool registry."""
    from apex_agents.tools import Tool, ToolParameter, ToolRegistry

    registry = ToolRegistry()

    # Add some test tools
    async def search_func(query: str, limit: int = 5) -> str:
        return f"Search results for: {query}"

    search_tool = Tool(
        name="web_search",
        description="Search the web",
        parameters=[
            ToolParameter("query", "string", "The search query"),
            ToolParameter("limit", "number", "Number of results", required=False),
        ],
        func=search_func,
    )
    registry.register(search_tool)

    async def read_func(path: str) -> str:
        return f"Contents of {path}"

    read_tool = Tool(
        name="read_file",
        description="Read a file",
        parameters=[ToolParameter("path", "string", "File path")],
        func=read_func,
    )
    registry.register(read_tool)

    return registry


@pytest.fixture
def sample_task_input():
    """Create a sample task input."""
    from apex_agents.agent import TaskInput

    return TaskInput(
        instruction="Analyze the following data and provide insights.",
        context={"source": "test", "timestamp": "2024-01-01"},
        parameters={"format": "json", "verbose": True},
    )


@pytest.fixture
def sample_queued_task():
    """Create a sample queued task."""
    from apex_agents.executor import QueuedTask

    return QueuedTask(
        id="test-task-123",
        name="test-task",
        instruction="Perform a test operation",
        context={"key": "value"},
        parameters={"param1": "value1"},
        priority=5,
        max_retries=3,
        retry_count=0,
        trace_id="abc123def456",
        span_id="789012345678",
    )


@pytest.fixture
def mock_redis():
    """Create a mock Redis client."""
    redis = AsyncMock()
    redis.brpop.return_value = None
    redis.lpush.return_value = None
    redis.setex.return_value = None
    redis.aclose.return_value = None
    return redis


@pytest.fixture
def mock_httpx_client():
    """Create a mock httpx client."""
    import httpx

    client = AsyncMock(spec=httpx.AsyncClient)
    response = MagicMock()
    response.json.return_value = {"success": True, "data": {}}
    response.raise_for_status.return_value = None
    client.request.return_value = response
    return client
