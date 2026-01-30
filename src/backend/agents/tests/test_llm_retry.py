"""Tests for LLM client retry logic and edge cases."""

import pytest
from unittest.mock import AsyncMock, MagicMock, patch

import httpx
from tenacity import RetryError

from apex_agents.llm import (
    LLMClient,
    LLMResponse,
    LLMUsage,
    calculate_cost,
    count_tokens,
)


class TestLLMClientRetry:
    """Tests for LLM client retry logic via tenacity."""

    @pytest.fixture
    def client(self):
        """Create a test client."""
        return LLMClient(
            openai_api_key="test-openai-key",
            anthropic_api_key="test-anthropic-key",
        )

    @pytest.mark.asyncio
    async def test_retries_on_http_error(self, client):
        """Test that transient HTTP errors trigger retries."""
        mock_success = {
            "choices": [
                {
                    "message": {"content": "OK", "role": "assistant"},
                    "finish_reason": "stop",
                }
            ],
            "usage": {"prompt_tokens": 5, "completion_tokens": 2, "total_tokens": 7},
        }

        call_count = 0

        async def mock_post(*args, **kwargs):
            nonlocal call_count
            call_count += 1
            if call_count < 3:
                raise httpx.HTTPStatusError(
                    "Server Error",
                    request=MagicMock(),
                    response=MagicMock(status_code=500),
                )
            return MagicMock(
                json=lambda: mock_success,
                raise_for_status=lambda: None,
            )

        with patch("httpx.AsyncClient.post", side_effect=mock_post):
            response = await client.create(
                model="gpt-4o-mini",
                messages=[{"role": "user", "content": "Hi"}],
            )

        assert response.content == "OK"
        assert call_count == 3

    @pytest.mark.asyncio
    async def test_no_retry_on_value_error(self):
        """Test that ValueError is not retried (retry_if_not_exception_type)."""
        bare_client = LLMClient()

        with pytest.raises(ValueError):
            await bare_client.create(
                model="gpt-4o-mini",
                messages=[{"role": "user", "content": "Hi"}],
            )

    @pytest.mark.asyncio
    async def test_retries_exhaust_raises(self, client):
        """Test that when all retries are exhausted, the error propagates."""
        async def always_fail(*args, **kwargs):
            raise httpx.HTTPStatusError(
                "Server Error",
                request=MagicMock(),
                response=MagicMock(status_code=500),
            )

        with patch("httpx.AsyncClient.post", side_effect=always_fail):
            with pytest.raises(RetryError):
                await client.create(
                    model="gpt-4o-mini",
                    messages=[{"role": "user", "content": "Hi"}],
                )

    @pytest.mark.asyncio
    async def test_anthropic_missing_key_raises_immediately(self):
        """Test that missing Anthropic key raises ValueError without retry."""
        client = LLMClient(openai_api_key="test-key")

        with pytest.raises(ValueError):
            await client.create(
                model="claude-3.5-sonnet",
                messages=[{"role": "user", "content": "Hi"}],
            )

    @pytest.mark.asyncio
    async def test_unknown_provider_in_create(self):
        """Test that unknown model raises ValueError through create."""
        client = LLMClient(openai_api_key="test")

        with pytest.raises(ValueError):
            await client.create(
                model="llama-70b",
                messages=[{"role": "user", "content": "Hi"}],
            )


class TestLLMClientAnthropicDetails:
    """Tests for Anthropic-specific message conversion."""

    @pytest.fixture
    def client(self):
        """Create client with Anthropic key."""
        return LLMClient(anthropic_api_key="test-key")

    @pytest.mark.asyncio
    async def test_anthropic_system_message_extraction(self, client):
        """Test that system messages are extracted for Anthropic."""
        mock_response = {
            "content": [{"type": "text", "text": "Hello!"}],
            "usage": {"input_tokens": 10, "output_tokens": 5},
            "stop_reason": "end_turn",
        }

        with patch("httpx.AsyncClient.post") as mock_post:
            mock_post.return_value = MagicMock(
                json=lambda: mock_response,
                raise_for_status=lambda: None,
            )

            await client.create(
                model="claude-3-haiku",
                messages=[
                    {"role": "system", "content": "You are helpful."},
                    {"role": "user", "content": "Hi"},
                ],
            )

            call_args = mock_post.call_args
            assert "system" in str(call_args)

    @pytest.mark.asyncio
    async def test_anthropic_tool_use_response(self, client):
        """Test parsing Anthropic tool_use response blocks."""
        mock_response = {
            "content": [
                {"type": "text", "text": "Let me search."},
                {
                    "type": "tool_use",
                    "id": "toolu_123",
                    "name": "web_search",
                    "input": {"query": "test"},
                },
            ],
            "usage": {"input_tokens": 20, "output_tokens": 15},
            "stop_reason": "tool_use",
        }

        with patch("httpx.AsyncClient.post") as mock_post:
            mock_post.return_value = MagicMock(
                json=lambda: mock_response,
                raise_for_status=lambda: None,
            )

            response = await client.create(
                model="claude-3-haiku",
                messages=[{"role": "user", "content": "Search"}],
                tools=[{"name": "web_search", "description": "Search", "parameters": {}}],
            )

        assert response.content == "Let me search."
        assert len(response.tool_calls) == 1
        assert response.tool_calls[0]["id"] == "toolu_123"
        assert response.tool_calls[0]["function"]["name"] == "web_search"
        assert response.tool_calls[0]["function"]["arguments"] == {"query": "test"}


class TestCountTokensFallback:
    """Tests for count_tokens fallback encoding."""

    def test_unknown_model_uses_fallback(self):
        """Test that an unknown model falls back to cl100k_base."""
        tokens = count_tokens("Hello world", model="totally-unknown-model")
        assert tokens > 0

    def test_empty_string(self):
        """Test token count for empty string."""
        tokens = count_tokens("")
        assert tokens == 0


class TestCostCalculationEdgeCases:
    """Tests for edge cases in cost calculation."""

    def test_zero_tokens(self):
        """Test cost with zero tokens."""
        cost = calculate_cost("gpt-4o", 0, 0)
        assert cost == 0.0

    def test_all_known_models(self):
        """Test that all models in MODEL_PRICING produce non-negative costs."""
        from apex_agents.llm import MODEL_PRICING

        for model_name in MODEL_PRICING:
            cost = calculate_cost(model_name, 1000, 500)
            assert cost >= 0.0
