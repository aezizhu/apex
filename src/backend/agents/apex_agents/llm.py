"""
LLM client abstraction supporting multiple providers.
"""

from __future__ import annotations

import json
from dataclasses import dataclass
from enum import Enum
from typing import Any

import httpx
import structlog
import tiktoken
from opentelemetry import trace
from tenacity import retry, retry_if_not_exception_type, stop_after_attempt, wait_exponential

logger = structlog.get_logger()
tracer = trace.get_tracer(__name__)


class LLMProvider(str, Enum):
    """Supported LLM providers."""
    OPENAI = "openai"
    ANTHROPIC = "anthropic"


@dataclass
class LLMUsage:
    """Token usage statistics."""
    prompt_tokens: int
    completion_tokens: int
    total_tokens: int


@dataclass
class LLMResponse:
    """Response from an LLM call."""
    content: str
    tool_calls: list[dict[str, Any]]
    usage: LLMUsage
    model: str
    cost: float
    finish_reason: str


# Pricing per 1K tokens (input, output)
MODEL_PRICING: dict[str, tuple[float, float]] = {
    # OpenAI
    "gpt-4o": (0.005, 0.015),
    "gpt-4o-mini": (0.00015, 0.0006),
    "gpt-4-turbo": (0.01, 0.03),
    "gpt-3.5-turbo": (0.0005, 0.0015),
    # Anthropic
    "claude-3-opus": (0.015, 0.075),
    "claude-3-sonnet": (0.003, 0.015),
    "claude-3.5-sonnet": (0.003, 0.015),
    "claude-3-haiku": (0.00025, 0.00125),
    "claude-3.5-haiku": (0.00025, 0.00125),
}


def calculate_cost(model: str, prompt_tokens: int, completion_tokens: int) -> float:
    """Calculate cost for a model call."""
    pricing = MODEL_PRICING.get(model, (0.01, 0.03))  # Default to expensive
    input_cost = (prompt_tokens / 1000) * pricing[0]
    output_cost = (completion_tokens / 1000) * pricing[1]
    return input_cost + output_cost


class LLMClient:
    """
    Unified LLM client supporting multiple providers.

    Example:
        client = LLMClient(
            openai_api_key="sk-...",
            anthropic_api_key="sk-ant-..."
        )

        response = await client.create(
            model="gpt-4o",
            messages=[{"role": "user", "content": "Hello!"}]
        )
    """

    def __init__(
        self,
        openai_api_key: str | None = None,
        anthropic_api_key: str | None = None,
        timeout: float = 60.0,
    ):
        self.openai_api_key = openai_api_key
        self.anthropic_api_key = anthropic_api_key
        self.timeout = timeout
        self._logger = logger.bind(component="llm_client")

    def _get_provider(self, model: str) -> LLMProvider:
        """Determine provider from model name."""
        if model.startswith("gpt") or model.startswith("o1"):
            return LLMProvider.OPENAI
        elif model.startswith("claude"):
            return LLMProvider.ANTHROPIC
        else:
            raise ValueError(f"Unknown model provider for: {model}")

    @retry(
        stop=stop_after_attempt(3),
        wait=wait_exponential(multiplier=1, min=1, max=10),
        retry=retry_if_not_exception_type(ValueError),
    )
    async def create(
        self,
        model: str,
        messages: list[dict[str, Any]],
        tools: list[dict[str, Any]] | None = None,
        temperature: float = 0.7,
        max_tokens: int | None = None,
    ) -> LLMResponse:
        """
        Create a completion.

        Args:
            model: Model name
            messages: Conversation messages
            tools: Tool definitions
            temperature: Sampling temperature
            max_tokens: Maximum tokens to generate

        Returns:
            LLMResponse with the result
        """
        provider = self._get_provider(model)

        with tracer.start_as_current_span(
            "llm_create",
            attributes={
                "llm.provider": provider.value,
                "llm.model": model,
            }
        ) as span:
            if provider == LLMProvider.OPENAI:
                response = await self._openai_create(
                    model, messages, tools, temperature, max_tokens
                )
            elif provider == LLMProvider.ANTHROPIC:
                response = await self._anthropic_create(
                    model, messages, tools, temperature, max_tokens
                )
            else:
                raise ValueError(f"Unsupported provider: {provider}")

            span.set_attributes({
                "llm.tokens.prompt": response.usage.prompt_tokens,
                "llm.tokens.completion": response.usage.completion_tokens,
                "llm.cost": response.cost,
            })

            return response

    async def _openai_create(
        self,
        model: str,
        messages: list[dict[str, Any]],
        tools: list[dict[str, Any]] | None,
        temperature: float,
        max_tokens: int | None,
    ) -> LLMResponse:
        """Call OpenAI API."""
        if not self.openai_api_key:
            raise ValueError("OpenAI API key not configured")

        async with httpx.AsyncClient(timeout=self.timeout) as client:
            payload: dict[str, Any] = {
                "model": model,
                "messages": messages,
                "temperature": temperature,
            }

            if tools:
                payload["tools"] = [
                    {"type": "function", "function": t} for t in tools
                ]

            if max_tokens:
                payload["max_tokens"] = max_tokens

            response = await client.post(
                "https://api.openai.com/v1/chat/completions",
                headers={
                    "Authorization": f"Bearer {self.openai_api_key}",
                    "Content-Type": "application/json",
                },
                json=payload,
            )
            response.raise_for_status()
            data = response.json()

        choice = data["choices"][0]
        message = choice["message"]
        usage = data["usage"]

        tool_calls = []
        if "tool_calls" in message:
            for tc in message["tool_calls"]:
                tool_calls.append({
                    "id": tc["id"],
                    "function": {
                        "name": tc["function"]["name"],
                        "arguments": json.loads(tc["function"]["arguments"]),
                    }
                })

        llm_usage = LLMUsage(
            prompt_tokens=usage["prompt_tokens"],
            completion_tokens=usage["completion_tokens"],
            total_tokens=usage["total_tokens"],
        )

        return LLMResponse(
            content=message.get("content", ""),
            tool_calls=tool_calls,
            usage=llm_usage,
            model=model,
            cost=calculate_cost(model, llm_usage.prompt_tokens, llm_usage.completion_tokens),
            finish_reason=choice["finish_reason"],
        )

    async def _anthropic_create(
        self,
        model: str,
        messages: list[dict[str, Any]],
        tools: list[dict[str, Any]] | None,
        temperature: float,
        max_tokens: int | None,
    ) -> LLMResponse:
        """Call Anthropic API."""
        if not self.anthropic_api_key:
            raise ValueError("Anthropic API key not configured")

        # Convert messages format for Anthropic
        system_content = ""
        anthropic_messages = []

        for msg in messages:
            if msg["role"] == "system":
                system_content = msg["content"]
            else:
                anthropic_messages.append({
                    "role": msg["role"],
                    "content": msg["content"],
                })

        async with httpx.AsyncClient(timeout=self.timeout) as client:
            payload: dict[str, Any] = {
                "model": model,
                "messages": anthropic_messages,
                "max_tokens": max_tokens or 4096,
            }

            if system_content:
                payload["system"] = system_content

            if tools:
                payload["tools"] = [
                    {
                        "name": t["name"],
                        "description": t.get("description", ""),
                        "input_schema": t.get("parameters", {}),
                    }
                    for t in tools
                ]

            response = await client.post(
                "https://api.anthropic.com/v1/messages",
                headers={
                    "x-api-key": self.anthropic_api_key,
                    "anthropic-version": "2023-06-01",
                    "Content-Type": "application/json",
                },
                json=payload,
            )
            response.raise_for_status()
            data = response.json()

        # Extract content and tool calls
        content = ""
        tool_calls = []

        for block in data["content"]:
            if block["type"] == "text":
                content = block["text"]
            elif block["type"] == "tool_use":
                tool_calls.append({
                    "id": block["id"],
                    "function": {
                        "name": block["name"],
                        "arguments": block["input"],
                    }
                })

        usage = data["usage"]
        llm_usage = LLMUsage(
            prompt_tokens=usage["input_tokens"],
            completion_tokens=usage["output_tokens"],
            total_tokens=usage["input_tokens"] + usage["output_tokens"],
        )

        return LLMResponse(
            content=content,
            tool_calls=tool_calls,
            usage=llm_usage,
            model=model,
            cost=calculate_cost(model, llm_usage.prompt_tokens, llm_usage.completion_tokens),
            finish_reason=data["stop_reason"],
        )


def count_tokens(text: str, model: str = "gpt-4") -> int:
    """Count tokens in text using tiktoken."""
    try:
        encoding = tiktoken.encoding_for_model(model)
    except KeyError:
        encoding = tiktoken.get_encoding("cl100k_base")
    return len(encoding.encode(text))
