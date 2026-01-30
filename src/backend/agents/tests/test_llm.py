"""Tests for the LLM client."""

import pytest
from unittest.mock import AsyncMock, patch, MagicMock
import httpx
from apex_agents.llm import (
    LLMClient,
    LLMProvider,
    LLMResponse,
    LLMUsage,
    calculate_cost,
    count_tokens,
)


class TestLLMProvider:
    """Tests for LLM provider detection."""

    def test_openai_models(self):
        """Test OpenAI model detection."""
        client = LLMClient(openai_api_key="test")

        assert client._get_provider("gpt-4o") == LLMProvider.OPENAI
        assert client._get_provider("gpt-4o-mini") == LLMProvider.OPENAI
        assert client._get_provider("gpt-3.5-turbo") == LLMProvider.OPENAI

    def test_anthropic_models(self):
        """Test Anthropic model detection."""
        client = LLMClient(anthropic_api_key="test")

        assert client._get_provider("claude-3-opus") == LLMProvider.ANTHROPIC
        assert client._get_provider("claude-3.5-sonnet") == LLMProvider.ANTHROPIC
        assert client._get_provider("claude-3-haiku") == LLMProvider.ANTHROPIC

    def test_unknown_model(self):
        """Test unknown model raises error."""
        client = LLMClient()

        with pytest.raises(ValueError) as exc_info:
            client._get_provider("unknown-model")

        assert "Unknown model provider" in str(exc_info.value)


class TestCostCalculation:
    """Tests for cost calculation."""

    def test_gpt4o_cost(self):
        """Test GPT-4o cost calculation."""
        cost = calculate_cost("gpt-4o", 1000, 500)
        expected = (1000 / 1000 * 0.005) + (500 / 1000 * 0.015)
        assert abs(cost - expected) < 0.0001

    def test_gpt4o_mini_cost(self):
        """Test GPT-4o-mini cost calculation."""
        cost = calculate_cost("gpt-4o-mini", 1000, 500)
        expected = (1000 / 1000 * 0.00015) + (500 / 1000 * 0.0006)
        assert abs(cost - expected) < 0.0001

    def test_claude_sonnet_cost(self):
        """Test Claude Sonnet cost calculation."""
        cost = calculate_cost("claude-3.5-sonnet", 1000, 500)
        expected = (1000 / 1000 * 0.003) + (500 / 1000 * 0.015)
        assert abs(cost - expected) < 0.0001

    def test_unknown_model_default_pricing(self):
        """Test unknown model uses default pricing."""
        cost = calculate_cost("unknown-model", 1000, 500)
        expected = (1000 / 1000 * 0.01) + (500 / 1000 * 0.03)
        assert abs(cost - expected) < 0.0001


class TestTokenCounting:
    """Tests for token counting."""

    def test_simple_text(self):
        """Test token count for simple text."""
        tokens = count_tokens("Hello, world!")
        assert tokens > 0
        assert tokens < 10

    def test_longer_text(self):
        """Test token count for longer text."""
        text = "This is a longer piece of text that should have more tokens. " * 10
        tokens = count_tokens(text)
        assert tokens > 50


class TestLLMClient:
    """Tests for the LLM client."""

    @pytest.fixture
    def client(self):
        """Create a test client."""
        return LLMClient(
            openai_api_key="test-openai-key",
            anthropic_api_key="test-anthropic-key",
        )

    @pytest.mark.asyncio
    async def test_openai_create(self, client):
        """Test OpenAI API call."""
        mock_response = {
            "choices": [
                {
                    "message": {
                        "content": "Hello!",
                        "role": "assistant",
                    },
                    "finish_reason": "stop",
                }
            ],
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 5,
                "total_tokens": 15,
            },
        }

        with patch("httpx.AsyncClient.post") as mock_post:
            mock_post.return_value = MagicMock(
                json=lambda: mock_response,
                raise_for_status=lambda: None,
            )

            response = await client.create(
                model="gpt-4o-mini",
                messages=[{"role": "user", "content": "Hi"}],
            )

            assert response.content == "Hello!"
            assert response.usage.total_tokens == 15
            assert response.tool_calls == []

    @pytest.mark.asyncio
    async def test_openai_with_tools(self, client):
        """Test OpenAI API call with tool calls."""
        mock_response = {
            "choices": [
                {
                    "message": {
                        "content": "Let me search.",
                        "role": "assistant",
                        "tool_calls": [
                            {
                                "id": "call_123",
                                "function": {
                                    "name": "search",
                                    "arguments": '{"query": "test"}',
                                },
                            }
                        ],
                    },
                    "finish_reason": "tool_calls",
                }
            ],
            "usage": {
                "prompt_tokens": 20,
                "completion_tokens": 15,
                "total_tokens": 35,
            },
        }

        with patch("httpx.AsyncClient.post") as mock_post:
            mock_post.return_value = MagicMock(
                json=lambda: mock_response,
                raise_for_status=lambda: None,
            )

            response = await client.create(
                model="gpt-4o-mini",
                messages=[{"role": "user", "content": "Search for something"}],
                tools=[{"name": "search", "parameters": {}}],
            )

            assert len(response.tool_calls) == 1
            assert response.tool_calls[0]["function"]["name"] == "search"
            assert response.finish_reason == "tool_calls"

    @pytest.mark.asyncio
    async def test_anthropic_create(self, client):
        """Test Anthropic API call."""
        mock_response = {
            "content": [{"type": "text", "text": "Hello!"}],
            "usage": {
                "input_tokens": 10,
                "output_tokens": 5,
            },
            "stop_reason": "end_turn",
        }

        with patch("httpx.AsyncClient.post") as mock_post:
            mock_post.return_value = MagicMock(
                json=lambda: mock_response,
                raise_for_status=lambda: None,
            )

            response = await client.create(
                model="claude-3.5-sonnet",
                messages=[{"role": "user", "content": "Hi"}],
            )

            assert response.content == "Hello!"
            assert response.usage.total_tokens == 15

    @pytest.mark.asyncio
    async def test_missing_api_key(self):
        """Test error when API key is missing."""
        client = LLMClient()  # No keys

        with pytest.raises(ValueError) as exc_info:
            await client.create(
                model="gpt-4o",
                messages=[{"role": "user", "content": "Hi"}],
            )

        assert "API key not configured" in str(exc_info.value)


class TestLLMUsage:
    """Tests for LLMUsage."""

    def test_usage_creation(self):
        """Test usage object creation."""
        usage = LLMUsage(
            prompt_tokens=100,
            completion_tokens=50,
            total_tokens=150,
        )

        assert usage.prompt_tokens == 100
        assert usage.completion_tokens == 50
        assert usage.total_tokens == 150


class TestLLMResponse:
    """Tests for LLMResponse."""

    def test_response_creation(self):
        """Test response object creation."""
        response = LLMResponse(
            content="Test response",
            tool_calls=[],
            usage=LLMUsage(10, 5, 15),
            model="gpt-4o-mini",
            cost=0.001,
            finish_reason="stop",
        )

        assert response.content == "Test response"
        assert response.tool_calls == []
        assert response.cost == 0.001
