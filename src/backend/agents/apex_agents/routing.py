"""
FrugalGPT cascade model routing.

Routes LLM calls through a cascade of models from cheapest to most expensive,
escalating only when the cheaper model's response confidence is too low.
"""

from __future__ import annotations

import re
from dataclasses import dataclass, field
from typing import Any

import structlog
from opentelemetry import trace
from pydantic import BaseModel, Field

from apex_agents.llm import LLMClient, LLMResponse, calculate_cost

logger = structlog.get_logger()
tracer = trace.get_tracer(__name__)


# Default cascade order: cheapest to most expensive
DEFAULT_CASCADE = [
    "gpt-4o-mini",       # $0.15/$0.60 per 1M tokens
    "claude-3-haiku",    # $0.25/$1.25 per 1M tokens
    "gpt-4o",            # $2.50/$10.00 per 1M tokens
    "claude-3.5-sonnet", # $3.00/$15.00 per 1M tokens
]

# Hedging language patterns indicating low confidence
_HEDGING_PATTERNS: list[re.Pattern[str]] = [
    re.compile(r"\bI'?m not sure\b", re.IGNORECASE),
    re.compile(r"\bmaybe\b", re.IGNORECASE),
    re.compile(r"\bI think\b", re.IGNORECASE),
    re.compile(r"\bpossibly\b", re.IGNORECASE),
    re.compile(r"\bperhaps\b", re.IGNORECASE),
    re.compile(r"\bit seems\b", re.IGNORECASE),
    re.compile(r"\bI believe\b", re.IGNORECASE),
    re.compile(r"\bnot entirely clear\b", re.IGNORECASE),
    re.compile(r"\bI'?m uncertain\b", re.IGNORECASE),
]

# Refusal patterns indicating the model cannot complete the task
_REFUSAL_PATTERNS: list[re.Pattern[str]] = [
    re.compile(r"\bI cannot\b", re.IGNORECASE),
    re.compile(r"\bI can'?t\b", re.IGNORECASE),
    re.compile(r"\bI'?m unable\b", re.IGNORECASE),
    re.compile(r"\bI'?m not able\b", re.IGNORECASE),
    re.compile(r"\bI don'?t have the ability\b", re.IGNORECASE),
    re.compile(r"\bI'?m sorry,? but I\b", re.IGNORECASE),
    re.compile(r"\bunable to (assist|help|provide|complete)\b", re.IGNORECASE),
]


class RoutingConfig(BaseModel):
    """Configuration for cascade model routing."""

    enabled: bool = Field(default=False, description="Enable cascade routing")
    cascade: list[str] = Field(
        default_factory=lambda: list(DEFAULT_CASCADE),
        description="Ordered list of models from cheapest to most expensive",
    )
    confidence_threshold: float = Field(
        default=0.7,
        ge=0.0,
        le=1.0,
        description="Minimum confidence score to accept a response",
    )
    max_escalations: int = Field(
        default=3,
        ge=0,
        description="Maximum number of escalations allowed (0 = use only first model)",
    )


@dataclass
class RoutingResult:
    """Result from cascade routing."""

    response: LLMResponse
    model_used: str
    models_tried: list[str]
    confidence: float
    total_cost: float
    cost_saved: float


class ModelRouter:
    """
    FrugalGPT cascade model router.

    Routes LLM calls through a cascade of models from cheapest to most
    expensive. Tries the cheapest model first and escalates only when the
    response confidence is below the configured threshold.

    Example:
        config = RoutingConfig(
            enabled=True,
            cascade=["gpt-4o-mini", "claude-3-haiku", "gpt-4o", "claude-3.5-sonnet"],
            confidence_threshold=0.7,
            max_escalations=3,
        )
        router = ModelRouter(llm_client=client, config=config)

        result = await router.route(
            messages=[{"role": "user", "content": "Explain quantum computing"}]
        )
        print(f"Used {result.model_used}, saved ${result.cost_saved:.4f}")
    """

    def __init__(self, llm_client: LLMClient, config: RoutingConfig):
        self.llm_client = llm_client
        self.config = config
        self.cascade = config.cascade
        self.confidence_threshold = config.confidence_threshold
        self._logger = logger.bind(component="model_router")

    async def route(
        self,
        messages: list[dict[str, Any]],
        tools: list[dict[str, Any]] | None = None,
        temperature: float = 0.7,
    ) -> RoutingResult:
        """
        Try cheapest model first, escalate if confidence is low.

        Args:
            messages: Conversation messages.
            tools: Optional tool definitions.
            temperature: Sampling temperature.

        Returns:
            RoutingResult with the best response found.
        """
        with tracer.start_as_current_span(
            "model_router_route",
            attributes={
                "router.cascade_length": len(self.cascade),
                "router.confidence_threshold": self.confidence_threshold,
            },
        ) as span:
            # Limit cascade length by max_escalations
            max_models = self.config.max_escalations + 1
            effective_cascade = self.cascade[:max_models]

            accumulated_cost = 0.0
            models_tried: list[str] = []

            most_expensive_model = self.cascade[-1] if self.cascade else effective_cascade[-1]

            for i, model in enumerate(effective_cascade):
                is_last = i == len(effective_cascade) - 1

                self._logger.debug(
                    "Trying model in cascade",
                    model=model,
                    attempt=i + 1,
                    total_models=len(effective_cascade),
                )

                with tracer.start_as_current_span(
                    f"cascade_attempt_{i}",
                    attributes={"router.model": model, "router.attempt": i},
                ):
                    response = await self.llm_client.create(
                        model=model,
                        messages=messages,
                        tools=tools,
                        temperature=temperature,
                    )

                    accumulated_cost += response.cost
                    models_tried.append(model)

                    confidence = self._evaluate_confidence(
                        response, tools_expected=tools is not None and len(tools) > 0
                    )

                    self._logger.debug(
                        "Model response evaluated",
                        model=model,
                        confidence=confidence,
                        threshold=self.confidence_threshold,
                        is_last=is_last,
                    )

                    if confidence >= self.confidence_threshold or is_last:
                        # Estimate cost if we had used the most expensive model directly
                        premium_cost = calculate_cost(
                            most_expensive_model,
                            response.usage.prompt_tokens,
                            response.usage.completion_tokens,
                        )

                        result = RoutingResult(
                            response=response,
                            model_used=model,
                            models_tried=models_tried,
                            confidence=confidence,
                            total_cost=accumulated_cost,
                            cost_saved=max(0.0, premium_cost - accumulated_cost),
                        )

                        span.set_attributes({
                            "router.model_used": model,
                            "router.models_tried": len(models_tried),
                            "router.confidence": confidence,
                            "router.total_cost": accumulated_cost,
                            "router.cost_saved": result.cost_saved,
                        })

                        self._logger.info(
                            "Routing complete",
                            model_used=model,
                            models_tried=len(models_tried),
                            confidence=round(confidence, 3),
                            total_cost=round(accumulated_cost, 6),
                            cost_saved=round(result.cost_saved, 6),
                        )

                        return result

            # Should never reach here due to is_last check, but satisfy type checker
            raise RuntimeError("Cascade exhausted without returning a result")  # pragma: no cover

    def _evaluate_confidence(
        self,
        response: LLMResponse,
        tools_expected: bool = False,
    ) -> float:
        """
        Evaluate output quality using heuristics.

        Scoring (each factor contributes to a 0.0-1.0 score):
          1. Response length -- very short responses score lower
          2. Hedging language -- "I'm not sure", "maybe", etc.
          3. Refusal patterns -- "I cannot", "I'm unable", etc.
          4. Tool call validity -- if tools were expected, did it call them?
          5. Finish reason -- abnormal stop reasons reduce confidence

        Args:
            response: The LLM response to evaluate.
            tools_expected: Whether tools were provided and a tool call is expected.

        Returns:
            Confidence score between 0.0 and 1.0.
        """
        content = response.content or ""

        # Start with base score of 1.0 and apply multiplicative penalties.
        # This ensures any single bad signal can drag confidence below the
        # threshold, unlike a weighted average where good scores dilute bad ones.

        score = 1.0

        # 1. Response length -- very short or empty responses get penalized
        length = len(content.strip())
        if length == 0:
            if response.tool_calls:
                score *= 0.95  # Empty content with tool calls is fine
            else:
                score *= 0.15  # Empty content without tool calls is very bad
        elif length < 10:
            score *= 0.40
        elif length < 30:
            score *= 0.60
        elif length < 100:
            score *= 0.85
        # else: no penalty

        # 2. Hedging language
        hedging_count = sum(1 for p in _HEDGING_PATTERNS if p.search(content))
        if hedging_count == 1:
            score *= 0.75
        elif hedging_count == 2:
            score *= 0.55
        elif hedging_count >= 3:
            score *= 0.35

        # 3. Refusal patterns (strongest signal)
        refusal_count = sum(1 for p in _REFUSAL_PATTERNS if p.search(content))
        if refusal_count == 1:
            score *= 0.35
        elif refusal_count >= 2:
            score *= 0.15

        # 4. Tool call validity
        if tools_expected:
            if response.tool_calls:
                pass  # Good -- no penalty
            else:
                score *= 0.75  # Tools available but not used

        # 5. Finish reason
        normal_reasons = {"stop", "end_turn", "tool_calls", "tool_use"}
        if response.finish_reason in normal_reasons:
            pass  # No penalty
        elif response.finish_reason == "length":
            score *= 0.65  # Response was truncated
        else:
            score *= 0.80  # Unknown finish reason

        return max(0.0, min(1.0, score))
