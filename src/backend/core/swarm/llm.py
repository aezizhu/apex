"""LLM client for swarm agents using the Anthropic Claude API."""

from __future__ import annotations

import asyncio
import json
import logging
from typing import AsyncGenerator

import httpx

from ..config import ANTHROPIC_API_KEY, ANTHROPIC_BASE_URL, MODEL_NAME
from .models import AgentRole
from .roles import ROLE_CONFIGS

logger = logging.getLogger("apex.swarm.llm")

# Simple rate-limit guard: minimum seconds between API calls
_call_lock = asyncio.Lock()
_MIN_CALL_GAP = 0.5
_last_call_time: float = 0.0


async def _rate_limit_wait() -> None:
    """Ensure at least _MIN_CALL_GAP seconds between consecutive API calls."""
    global _last_call_time
    async with _call_lock:
        now = asyncio.get_running_loop().time()
        wait = _MIN_CALL_GAP - (now - _last_call_time)
        if wait > 0:
            await asyncio.sleep(wait)
        _last_call_time = asyncio.get_running_loop().time()


def _to_anthropic_messages(messages: list[dict]) -> tuple[str, list[dict]]:
    """Convert OpenAI-style messages to Anthropic format.

    Returns (system_prompt, messages) where system is extracted
    and messages only contain user/assistant roles.
    """
    system = ""
    anthropic_msgs = []
    for msg in messages:
        if msg["role"] == "system":
            system = msg["content"]
        else:
            anthropic_msgs.append({"role": msg["role"], "content": msg["content"]})
    return system, anthropic_msgs


class LLMClient:
    """Thin async wrapper around the Anthropic Messages API."""

    MAX_RETRIES = 4
    BACKOFF_BASE = 5  # seconds: 5, 10, 20, 40

    def __init__(self) -> None:
        self._url = f"{ANTHROPIC_BASE_URL}/messages"
        self._headers = {
            "Content-Type": "application/json",
            "x-api-key": ANTHROPIC_API_KEY,
            "anthropic-version": "2023-06-01",
        }

    def _build_payload(self, messages: list[dict], role: AgentRole, *, stream: bool) -> dict:
        cfg = ROLE_CONFIGS[role]
        system, msgs = _to_anthropic_messages(messages)
        payload: dict = {
            "model": MODEL_NAME,
            "messages": msgs,
            "max_tokens": cfg["max_tokens"],
            "temperature": cfg["temperature"],
        }
        if system:
            payload["system"] = system
        if stream:
            payload["stream"] = True
        return payload

    async def call(self, messages: list[dict], role: AgentRole) -> str:
        """Send a non-streaming request and return the full response text."""
        payload = self._build_payload(messages, role, stream=False)
        for attempt in range(self.MAX_RETRIES + 1):
            await _rate_limit_wait()
            async with httpx.AsyncClient(timeout=120.0) as client:
                resp = await client.post(self._url, json=payload, headers=self._headers)
                if resp.status_code == 429 and attempt < self.MAX_RETRIES:
                    wait = self.BACKOFF_BASE * (2 ** attempt)
                    logger.warning("Rate limited (429), retrying in %ds (%d/%d)", wait, attempt + 1, self.MAX_RETRIES)
                    await asyncio.sleep(wait)
                    continue
                if resp.status_code == 529 and attempt < self.MAX_RETRIES:
                    wait = self.BACKOFF_BASE * (2 ** attempt)
                    logger.warning("API overloaded (529), retrying in %ds (%d/%d)", wait, attempt + 1, self.MAX_RETRIES)
                    await asyncio.sleep(wait)
                    continue
                if resp.status_code != 200:
                    logger.error("Claude API error %s: %s", resp.status_code, resp.text)
                    return f"[LLM Error {resp.status_code}]"
                data = resp.json()
                # Extract text from content blocks
                content_blocks = data.get("content", [])
                return "".join(
                    block.get("text", "") for block in content_blocks if block.get("type") == "text"
                )
        return "[LLM Error: max retries exceeded]"

    async def stream(self, messages: list[dict], role: AgentRole) -> AsyncGenerator[str, None]:
        """Send a streaming request and yield content chunks as they arrive."""
        payload = self._build_payload(messages, role, stream=True)

        for attempt in range(self.MAX_RETRIES + 1):
            await _rate_limit_wait()

            retry = False
            async with httpx.AsyncClient(timeout=120.0) as client:
                async with client.stream(
                    "POST", self._url, json=payload, headers=self._headers
                ) as resp:
                    if resp.status_code in (429, 529) and attempt < self.MAX_RETRIES:
                        await resp.aread()
                        retry = True
                    elif resp.status_code != 200:
                        error_body = await resp.aread()
                        logger.error(
                            "Claude API stream error %s: %s",
                            resp.status_code,
                            error_body.decode("utf-8", errors="replace"),
                        )
                        yield f"[LLM Error {resp.status_code}]"
                        return
                    else:
                        # Stream Anthropic SSE events
                        async for line in resp.aiter_lines():
                            if not line:
                                continue
                            if line.startswith("data:"):
                                data_str = line[len("data:"):].strip()
                                try:
                                    event = json.loads(data_str)
                                    event_type = event.get("type", "")
                                    if event_type == "content_block_delta":
                                        delta = event.get("delta", {})
                                        if delta.get("type") == "text_delta":
                                            text = delta.get("text", "")
                                            if text:
                                                yield text
                                    elif event_type == "message_stop":
                                        break
                                except (json.JSONDecodeError, KeyError):
                                    continue
                        return  # done streaming

            # Retry on 429/529 (after exiting the context managers)
            if retry:
                wait = self.BACKOFF_BASE * (2 ** attempt)
                logger.warning("Rate limited/overloaded, retrying in %ds (%d/%d)", wait, attempt + 1, self.MAX_RETRIES)
                await asyncio.sleep(wait)

        # All retries exhausted
        yield "[LLM Error: rate limit exceeded after retries]"
