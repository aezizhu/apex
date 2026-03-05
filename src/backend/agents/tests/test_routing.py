"""Tests for FrugalGPT cascade model routing."""

import pytest
from unittest.mock import AsyncMock

from apex_agents.llm import LLMClient, LLMResponse, LLMUsage
from apex_agents.routing import (
    DEFAULT_CASCADE,
    ModelRouter,
    RoutingConfig,
    RoutingResult,
)


def _make_response(
    content: str = "This is a clear, detailed, and helpful response to your question.",
    model: str = "gpt-4o-mini",
    tool_calls: list | None = None,
    finish_reason: str = "stop",
    prompt_tokens: int = 100,
    completion_tokens: int = 50,
    cost: float = 0.001,
) -> LLMResponse:
    """Helper to build an LLMResponse."""
    return LLMResponse(
        content=content,
        tool_calls=tool_calls or [],
        usage=LLMUsage(
            prompt_tokens=prompt_tokens,
            completion_tokens=completion_tokens,
            total_tokens=prompt_tokens + completion_tokens,
        ),
        model=model,
        cost=cost,
        finish_reason=finish_reason,
    )


@pytest.fixture
def routing_config() -> RoutingConfig:
    """Standard routing config for tests."""
    return RoutingConfig(
        enabled=True,
        cascade=list(DEFAULT_CASCADE),
        confidence_threshold=0.7,
        max_escalations=3,
    )


@pytest.fixture
def mock_llm_client() -> AsyncMock:
    """Create a mock LLM client."""
    return AsyncMock(spec=LLMClient)


@pytest.fixture
def router(mock_llm_client: AsyncMock, routing_config: RoutingConfig) -> ModelRouter:
    """Create a ModelRouter with mocked LLM client."""
    return ModelRouter(llm_client=mock_llm_client, config=routing_config)


class TestRoutingConfig:
    """Tests for RoutingConfig."""

    def test_default_values(self):
        """Test default routing configuration."""
        config = RoutingConfig()

        assert config.enabled is False
        assert config.cascade == list(DEFAULT_CASCADE)
        assert config.confidence_threshold == 0.7
        assert config.max_escalations == 3

    def test_custom_values(self):
        """Test custom routing configuration."""
        config = RoutingConfig(
            enabled=True,
            cascade=["gpt-4o-mini", "gpt-4o"],
            confidence_threshold=0.8,
            max_escalations=1,
        )

        assert config.enabled is True
        assert len(config.cascade) == 2
        assert config.confidence_threshold == 0.8
        assert config.max_escalations == 1

    def test_confidence_threshold_validation(self):
        """Test that confidence threshold is bounded 0.0-1.0."""
        from pydantic import ValidationError

        with pytest.raises(ValidationError):
            RoutingConfig(confidence_threshold=-0.1)

        with pytest.raises(ValidationError):
            RoutingConfig(confidence_threshold=1.5)

        # Boundary values should be fine
        config = RoutingConfig(confidence_threshold=0.0)
        assert config.confidence_threshold == 0.0

        config = RoutingConfig(confidence_threshold=1.0)
        assert config.confidence_threshold == 1.0


class TestModelRouterCheapModelSucceeds:
    """Tests for cheap model succeeding on first try."""

    @pytest.mark.asyncio
    async def test_cheap_model_high_confidence(self, router, mock_llm_client):
        """Cheap model returns a high-confidence response, no escalation."""
        mock_llm_client.create.return_value = _make_response(
            content="Here is a comprehensive and detailed answer to your question about machine learning.",
            model="gpt-4o-mini",
            cost=0.0005,
        )

        result = await router.route(
            messages=[{"role": "user", "content": "Explain ML"}],
        )

        assert result.model_used == "gpt-4o-mini"
        assert len(result.models_tried) == 1
        assert result.confidence >= 0.7
        assert result.total_cost == 0.0005
        mock_llm_client.create.assert_called_once()

    @pytest.mark.asyncio
    async def test_cheap_model_with_tool_calls(self, router, mock_llm_client):
        """Cheap model correctly uses tools, high confidence."""
        mock_llm_client.create.return_value = _make_response(
            content="Let me search for that information using the available search tool.",
            model="gpt-4o-mini",
            tool_calls=[{"id": "call_1", "function": {"name": "search", "arguments": {"q": "test"}}}],
            finish_reason="tool_calls",
            cost=0.0005,
        )

        result = await router.route(
            messages=[{"role": "user", "content": "Search for something"}],
            tools=[{"name": "search", "parameters": {}}],
        )

        assert result.model_used == "gpt-4o-mini"
        assert len(result.models_tried) == 1


class TestModelRouterEscalation:
    """Tests for escalation from cheap to mid-tier model."""

    @pytest.mark.asyncio
    async def test_escalate_to_mid_tier(self, router, mock_llm_client):
        """Cheap model hedges, escalates to mid-tier."""
        low_confidence = _make_response(
            content="I'm not sure, maybe the answer is something related to that topic.",
            model="gpt-4o-mini",
            cost=0.0005,
        )
        high_confidence = _make_response(
            content="Here is a comprehensive and detailed answer to your question about this topic.",
            model="claude-3-haiku",
            cost=0.001,
        )

        mock_llm_client.create.side_effect = [low_confidence, high_confidence]

        result = await router.route(
            messages=[{"role": "user", "content": "Complex question"}],
        )

        assert result.model_used == "claude-3-haiku"
        assert len(result.models_tried) == 2
        assert result.models_tried == ["gpt-4o-mini", "claude-3-haiku"]
        assert result.total_cost == pytest.approx(0.0015)
        assert mock_llm_client.create.call_count == 2

    @pytest.mark.asyncio
    async def test_escalate_on_refusal(self, router, mock_llm_client):
        """Cheap model refuses, escalates."""
        refusal = _make_response(
            content="I cannot help with that request. I'm unable to provide this information.",
            model="gpt-4o-mini",
            cost=0.0003,
        )
        success = _make_response(
            content="Here is the detailed information you requested about the topic.",
            model="claude-3-haiku",
            cost=0.001,
        )

        mock_llm_client.create.side_effect = [refusal, success]

        result = await router.route(
            messages=[{"role": "user", "content": "Difficult question"}],
        )

        assert result.model_used == "claude-3-haiku"
        assert len(result.models_tried) == 2

    @pytest.mark.asyncio
    async def test_escalate_on_empty_response(self, router, mock_llm_client):
        """Empty response with no tool calls triggers escalation."""
        empty = _make_response(
            content="",
            model="gpt-4o-mini",
            cost=0.0001,
        )
        good = _make_response(
            content="Here is the answer you were looking for with plenty of detail.",
            model="claude-3-haiku",
            cost=0.001,
        )

        mock_llm_client.create.side_effect = [empty, good]

        result = await router.route(
            messages=[{"role": "user", "content": "Question"}],
        )

        assert result.model_used == "claude-3-haiku"
        assert len(result.models_tried) == 2


class TestModelRouterFullCascade:
    """Tests for full cascade escalation to most expensive model."""

    @pytest.mark.asyncio
    async def test_full_cascade_to_premium(self, router, mock_llm_client):
        """All cheaper models fail, falls back to most expensive."""
        bad1 = _make_response(content="I'm not sure, maybe perhaps.", model="gpt-4o-mini", cost=0.0005)
        bad2 = _make_response(content="I cannot help with that. I'm unable to assist.", model="claude-3-haiku", cost=0.001)
        bad3 = _make_response(content="I think possibly perhaps maybe.", model="gpt-4o", cost=0.01)
        # Last model is always accepted regardless of confidence
        final = _make_response(content="Short.", model="claude-3.5-sonnet", cost=0.02)

        mock_llm_client.create.side_effect = [bad1, bad2, bad3, final]

        result = await router.route(
            messages=[{"role": "user", "content": "Very hard question"}],
        )

        assert result.model_used == "claude-3.5-sonnet"
        assert len(result.models_tried) == 4
        assert result.models_tried == list(DEFAULT_CASCADE)
        assert mock_llm_client.create.call_count == 4

    @pytest.mark.asyncio
    async def test_last_model_always_accepted(self, router, mock_llm_client):
        """The last model in cascade is accepted even with low confidence."""
        bad = _make_response(
            content="I'm not sure, maybe perhaps I think possibly.",
            model="gpt-4o-mini",
            cost=0.0005,
        )
        # All models return low confidence
        mock_llm_client.create.return_value = bad

        # Use a tiny cascade
        router.config.cascade = ["gpt-4o-mini", "gpt-4o"]
        router.cascade = router.config.cascade
        router.config.max_escalations = 1

        result = await router.route(
            messages=[{"role": "user", "content": "Question"}],
        )

        # Should accept gpt-4o as the last model
        assert result.model_used == "gpt-4o"
        assert len(result.models_tried) == 2


class TestCostSavingsCalculation:
    """Tests for cost savings calculation."""

    @pytest.mark.asyncio
    async def test_cost_saved_when_cheap_model_succeeds(self, router, mock_llm_client):
        """Cost savings when cheap model handles the request."""
        mock_llm_client.create.return_value = _make_response(
            content="Here is a clear, thorough, and detailed answer to your question.",
            model="gpt-4o-mini",
            prompt_tokens=100,
            completion_tokens=50,
            cost=0.0005,
        )

        result = await router.route(
            messages=[{"role": "user", "content": "Simple question"}],
        )

        assert result.total_cost == 0.0005
        # cost_saved should be positive (premium model would cost more)
        assert result.cost_saved > 0
        # Savings = premium_model_cost - actual_cost
        # Premium model (claude-3.5-sonnet): (100/1000)*0.003 + (50/1000)*0.015 = 0.0003 + 0.00075 = 0.00105
        # Actual: 0.0005
        # Saved: 0.00105 - 0.0005 = 0.00055
        assert result.cost_saved == pytest.approx(0.00055, abs=0.0001)

    @pytest.mark.asyncio
    async def test_no_savings_when_all_models_tried(self, router, mock_llm_client):
        """Accumulated costs may exceed premium cost after full cascade."""
        responses = [
            _make_response(content="Maybe.", model="gpt-4o-mini", cost=0.001),
            _make_response(content="I think.", model="claude-3-haiku", cost=0.002),
            _make_response(content="Perhaps.", model="gpt-4o", cost=0.01),
            _make_response(content="Here is your detailed answer.", model="claude-3.5-sonnet", cost=0.02),
        ]
        mock_llm_client.create.side_effect = responses

        result = await router.route(
            messages=[{"role": "user", "content": "Hard question"}],
        )

        assert result.total_cost == pytest.approx(0.033)
        # cost_saved is max(0, premium - actual), so could be 0 if actual > premium
        assert result.cost_saved >= 0.0

    @pytest.mark.asyncio
    async def test_cost_accumulation(self, router, mock_llm_client):
        """Total cost includes all attempted models."""
        bad = _make_response(content="I cannot help.", model="gpt-4o-mini", cost=0.001)
        good = _make_response(
            content="Here is a very detailed and comprehensive answer to your question.",
            model="claude-3-haiku",
            cost=0.002,
        )

        mock_llm_client.create.side_effect = [bad, good]

        result = await router.route(
            messages=[{"role": "user", "content": "Question"}],
        )

        assert result.total_cost == pytest.approx(0.003)


class TestConfidenceEvaluation:
    """Tests for confidence evaluation heuristics."""

    @pytest.fixture
    def router_for_eval(self, mock_llm_client):
        """Create a router just for _evaluate_confidence calls."""
        config = RoutingConfig(enabled=True)
        return ModelRouter(llm_client=mock_llm_client, config=config)

    def test_high_confidence_response(self, router_for_eval):
        """Long, clear response with no hedging scores high."""
        response = _make_response(
            content="Machine learning is a branch of artificial intelligence that enables systems "
            "to learn and improve from experience without being explicitly programmed. "
            "It focuses on developing algorithms that can access data and use it to learn for themselves.",
        )

        confidence = router_for_eval._evaluate_confidence(response)
        assert confidence >= 0.8

    def test_low_confidence_hedging(self, router_for_eval):
        """Response with hedging language scores lower."""
        response = _make_response(
            content="I'm not sure about this, but maybe the answer is related to that. "
            "I think possibly it could be something else perhaps.",
        )

        confidence = router_for_eval._evaluate_confidence(response)
        assert confidence < 0.5

    def test_low_confidence_refusal(self, router_for_eval):
        """Refusal response scores very low."""
        response = _make_response(
            content="I cannot help with that request. I'm unable to provide this information.",
        )

        confidence = router_for_eval._evaluate_confidence(response)
        assert confidence < 0.2

    def test_empty_response_no_tools(self, router_for_eval):
        """Empty response with no tool calls has very low confidence."""
        response = _make_response(content="", tool_calls=[])

        confidence = router_for_eval._evaluate_confidence(response)
        assert confidence < 0.2

    def test_empty_response_with_tools(self, router_for_eval):
        """Empty content but with tool calls is acceptable."""
        response = _make_response(
            content="",
            tool_calls=[{"id": "call_1", "function": {"name": "search", "arguments": {}}}],
            finish_reason="tool_calls",
        )

        confidence = router_for_eval._evaluate_confidence(response)
        assert confidence >= 0.7

    def test_short_response(self, router_for_eval):
        """Very short response gets lower score."""
        response = _make_response(content="Yes.")

        confidence = router_for_eval._evaluate_confidence(response)
        # Short but no hedging/refusal -- low-moderate confidence
        assert 0.3 <= confidence <= 0.5

    def test_tool_expected_but_not_called(self, router_for_eval):
        """When tools are expected but not called, slight penalty."""
        response = _make_response(
            content="Here is a clear, thorough, and detailed answer to your question.",
            tool_calls=[],
        )

        confidence_with_tools = router_for_eval._evaluate_confidence(response, tools_expected=True)
        confidence_without_tools = router_for_eval._evaluate_confidence(response, tools_expected=False)

        assert confidence_with_tools < confidence_without_tools

    def test_length_finish_reason(self, router_for_eval):
        """Truncated response (finish_reason='length') reduces confidence."""
        full = _make_response(
            content="A decent answer with enough content to be reasonable for evaluation purposes.",
            finish_reason="stop",
        )
        truncated = _make_response(
            content="A decent answer with enough content to be reasonable for evaluation purposes.",
            finish_reason="length",
        )

        conf_full = router_for_eval._evaluate_confidence(full)
        conf_truncated = router_for_eval._evaluate_confidence(truncated)

        assert conf_full > conf_truncated


class TestMaxEscalations:
    """Tests for max_escalations limiting the cascade."""

    @pytest.mark.asyncio
    async def test_max_escalations_limits_cascade(self, mock_llm_client):
        """max_escalations=1 means at most 2 models tried."""
        config = RoutingConfig(
            enabled=True,
            cascade=list(DEFAULT_CASCADE),
            confidence_threshold=0.99,  # Very high so it always escalates
            max_escalations=1,
        )
        router = ModelRouter(llm_client=mock_llm_client, config=config)

        bad = _make_response(content="Maybe.", model="gpt-4o-mini", cost=0.0005)
        ok = _make_response(content="OK.", model="claude-3-haiku", cost=0.001)
        mock_llm_client.create.side_effect = [bad, ok]

        result = await router.route(
            messages=[{"role": "user", "content": "Question"}],
        )

        # Should stop at 2 models (1 + 1 escalation)
        assert len(result.models_tried) == 2
        assert result.model_used == "claude-3-haiku"
        assert mock_llm_client.create.call_count == 2

    @pytest.mark.asyncio
    async def test_zero_escalations(self, mock_llm_client):
        """max_escalations=0 means only the first model is tried."""
        config = RoutingConfig(
            enabled=True,
            cascade=list(DEFAULT_CASCADE),
            confidence_threshold=0.99,
            max_escalations=0,
        )
        router = ModelRouter(llm_client=mock_llm_client, config=config)

        response = _make_response(content="Short.", model="gpt-4o-mini", cost=0.0005)
        mock_llm_client.create.return_value = response

        result = await router.route(
            messages=[{"role": "user", "content": "Question"}],
        )

        assert len(result.models_tried) == 1
        assert result.model_used == "gpt-4o-mini"
        mock_llm_client.create.assert_called_once()
