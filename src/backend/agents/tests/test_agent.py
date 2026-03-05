"""Tests for the Agent class."""

import pytest
from unittest.mock import AsyncMock, MagicMock, patch
from apex_agents.agent import Agent, AgentConfig, AgentStatus, TaskInput, TaskOutput
from apex_agents.llm import LLMClient, LLMResponse, LLMUsage
from apex_agents.tools import ToolRegistry, Tool, ToolParameter


@pytest.fixture
def mock_llm_client():
    """Create a mock LLM client."""
    client = AsyncMock(spec=LLMClient)
    return client


@pytest.fixture
def mock_tool_registry():
    """Create a mock tool registry with a test tool."""
    registry = ToolRegistry()

    async def test_tool_func(query: str) -> str:
        return f"Result for: {query}"

    test_tool_obj = Tool(
        name="test_tool",
        description="A test tool",
        parameters=[ToolParameter("query", "string", "The query")],
        func=test_tool_func,
    )
    registry.register(test_tool_obj)
    return registry


@pytest.fixture
def agent_config():
    """Create a test agent configuration."""
    return AgentConfig(
        name="test-agent",
        model="gpt-4o-mini",
        system_prompt="You are a helpful assistant.",
        tools=["test_tool"],
        max_iterations=5,
    )


@pytest.fixture
def agent(agent_config, mock_llm_client, mock_tool_registry):
    """Create a test agent."""
    return Agent(
        config=agent_config,
        llm_client=mock_llm_client,
        tool_registry=mock_tool_registry,
    )


class TestAgent:
    """Tests for Agent class."""

    def test_agent_creation(self, agent, agent_config):
        """Test that agent is created with correct configuration."""
        assert agent.config.name == "test-agent"
        assert agent.config.model == "gpt-4o-mini"
        assert agent.status == AgentStatus.IDLE

    def test_agent_available_tools(self, agent):
        """Test that agent has access to configured tools."""
        tools = agent.available_tools
        assert len(tools) == 1
        assert tools[0].name == "test_tool"

    @pytest.mark.asyncio
    async def test_agent_run_simple(self, agent, mock_llm_client):
        """Test simple agent execution without tool calls."""
        # Mock LLM response without tool calls
        mock_response = LLMResponse(
            content="Hello! How can I help you?",
            tool_calls=[],
            usage=LLMUsage(prompt_tokens=50, completion_tokens=20, total_tokens=70),
            model="gpt-4o-mini",
            cost=0.001,
            finish_reason="stop",
        )
        mock_llm_client.create.return_value = mock_response

        task = TaskInput(instruction="Say hello")
        result = await agent.run(task)

        assert result.result == "Hello! How can I help you?"
        assert agent.status == AgentStatus.IDLE
        assert agent.metrics.tokens_used == 70
        mock_llm_client.create.assert_called_once()

    @pytest.mark.asyncio
    async def test_agent_run_with_tool_call(self, agent, mock_llm_client):
        """Test agent execution with tool calls."""
        # First response requests tool use
        tool_response = LLMResponse(
            content="Let me search for that.",
            tool_calls=[
                {
                    "id": "call_123",
                    "function": {
                        "name": "test_tool",
                        "arguments": {"query": "test query"},
                    },
                }
            ],
            usage=LLMUsage(prompt_tokens=50, completion_tokens=30, total_tokens=80),
            model="gpt-4o-mini",
            cost=0.001,
            finish_reason="tool_calls",
        )

        # Second response after tool result
        final_response = LLMResponse(
            content="Based on the search, here is the answer.",
            tool_calls=[],
            usage=LLMUsage(prompt_tokens=100, completion_tokens=40, total_tokens=140),
            model="gpt-4o-mini",
            cost=0.002,
            finish_reason="stop",
        )

        mock_llm_client.create.side_effect = [tool_response, final_response]

        task = TaskInput(instruction="Search for something")
        result = await agent.run(task)

        assert "answer" in result.result
        assert agent.metrics.tool_calls == 1
        assert mock_llm_client.create.call_count == 2

    @pytest.mark.asyncio
    async def test_agent_max_iterations(self, agent, mock_llm_client):
        """Test that agent respects max iterations limit.

        With loop detection enabled, repeated identical outputs get caught
        by the loop detector before max iterations is reached. To test
        pure max-iterations, we generate unique content each call and
        disable loop detection thresholds.
        """
        call_count = 0

        async def unique_tool_response(*args, **kwargs):
            nonlocal call_count
            call_count += 1
            return LLMResponse(
                content=f"Working on step {call_count} with unique details about topic {call_count * 7}",
                tool_calls=[
                    {
                        "id": f"call_{call_count}",
                        "function": {
                            "name": "test_tool",
                            "arguments": {"query": f"query_{call_count}"},
                        },
                    }
                ],
                usage=LLMUsage(prompt_tokens=50, completion_tokens=30, total_tokens=80),
                model="gpt-4o-mini",
                cost=0.001,
                finish_reason="tool_calls",
            )

        mock_llm_client.create.side_effect = unique_tool_response

        agent.config.max_iterations = 3
        # Disable loop detection to test pure max-iterations behavior
        from apex_agents.loop_detector import LoopDetector, CostPerInsightTracker
        agent.loop_detector = LoopDetector(
            hash_threshold=999, similarity_threshold=1.0, length_stagnation_window=999
        )
        agent.cost_tracker = CostPerInsightTracker(min_iterations=999)

        task = TaskInput(instruction="Do something")
        result = await agent.run(task)

        assert "Max iterations reached" in result.result
        assert agent.metrics.iterations == 3

    @pytest.mark.asyncio
    async def test_agent_error_handling(self, agent, mock_llm_client):
        """Test that agent handles errors gracefully."""
        mock_llm_client.create.side_effect = Exception("API error")

        task = TaskInput(instruction="Do something")

        with pytest.raises(Exception) as exc_info:
            await agent.run(task)

        assert "API error" in str(exc_info.value)
        assert agent.status == AgentStatus.ERROR

    def test_build_messages_with_context(self, agent):
        """Test message building with context."""
        task = TaskInput(
            instruction="Analyze this data",
            context={"key1": "value1", "key2": "value2"},
        )

        messages = agent._build_initial_messages(task)

        assert len(messages) == 2  # system + user
        assert messages[0]["role"] == "system"
        assert "helpful assistant" in messages[0]["content"]
        assert messages[1]["role"] == "user"
        assert "Context:" in messages[1]["content"]
        assert "key1: value1" in messages[1]["content"]


class TestAgentConfig:
    """Tests for AgentConfig."""

    def test_default_values(self):
        """Test default configuration values."""
        config = AgentConfig(name="test", model="gpt-4o")

        assert config.system_prompt == ""
        assert config.tools == []
        assert config.max_iterations == 10
        assert config.temperature == 0.7

    def test_custom_values(self):
        """Test custom configuration values."""
        config = AgentConfig(
            name="custom",
            model="claude-3-sonnet",
            system_prompt="Custom prompt",
            tools=["tool1", "tool2"],
            max_iterations=20,
            temperature=0.5,
        )

        assert config.name == "custom"
        assert config.model == "claude-3-sonnet"
        assert len(config.tools) == 2


class TestTaskInput:
    """Tests for TaskInput."""

    def test_minimal_input(self):
        """Test task input with minimal fields."""
        task = TaskInput(instruction="Do something")

        assert task.instruction == "Do something"
        assert task.context == {}
        assert task.parameters == {}

    def test_full_input(self):
        """Test task input with all fields."""
        task = TaskInput(
            instruction="Analyze data",
            context={"source": "database"},
            parameters={"limit": 100},
        )

        assert task.instruction == "Analyze data"
        assert task.context["source"] == "database"
        assert task.parameters["limit"] == 100


class TestTaskOutput:
    """Tests for TaskOutput."""

    def test_output_creation(self):
        """Test task output creation."""
        output = TaskOutput(
            result="Analysis complete",
            data={"count": 42},
            reasoning="Step-by-step analysis...",
        )

        assert output.result == "Analysis complete"
        assert output.data["count"] == 42
        assert output.reasoning is not None
