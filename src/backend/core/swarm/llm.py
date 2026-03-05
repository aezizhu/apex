"""LLM client for swarm agents using the MiniMax API."""

from __future__ import annotations

import asyncio
import json
import logging
from typing import AsyncGenerator

import httpx

from ..config import MINIMAX_API_KEY, MINIMAX_BASE_URL, MODEL_NAME
from .models import AgentRole
from .roles import ROLE_CONFIGS

logger = logging.getLogger("apex.swarm.llm")

# Simple rate-limit guard: minimum seconds between API calls
_call_lock = asyncio.Lock()
_MIN_CALL_GAP = 1.0
_last_call_time: float = 0.0


async def _rate_limit_wait() -> None:
    """Ensure at least _MIN_CALL_GAP seconds between consecutive API calls."""
    global _last_call_time
    async with _call_lock:
        now = asyncio.get_event_loop().time()
        wait = _MIN_CALL_GAP - (now - _last_call_time)
        if wait > 0:
            await asyncio.sleep(wait)
        _last_call_time = asyncio.get_event_loop().time()


class LLMClient:
    """Thin async wrapper around the MiniMax chat-completions endpoint."""

    MAX_RETRIES = 4
    BACKOFF_BASE = 5  # seconds: 5, 10, 20, 40

    def __init__(self) -> None:
        self._url = f"{MINIMAX_BASE_URL}/chat/completions"
        self._headers = {
            "Content-Type": "application/json",
            "Authorization": f"Bearer {MINIMAX_API_KEY}",
        }

    def _build_payload(self, messages: list[dict], role: AgentRole, *, stream: bool) -> dict:
        cfg = ROLE_CONFIGS[role]
        return {
            "model": MODEL_NAME,
            "messages": messages,
            "stream": stream,
            "temperature": cfg["temperature"],
            "max_tokens": cfg["max_tokens"],
        }

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
                if resp.status_code != 200:
                    logger.error("MiniMax API error %s: %s", resp.status_code, resp.text)
                    return f"[LLM Error {resp.status_code}]"
                data = resp.json()
                return data["choices"][0]["message"]["content"]
        return "[LLM Error: max retries exceeded]"

    async def stream(self, messages: list[dict], role: AgentRole) -> AsyncGenerator[str, None]:
        """Send a streaming request and yield content chunks as they arrive."""
        payload = self._build_payload(messages, role, stream=True)

        for attempt in range(self.MAX_RETRIES + 1):
            await _rate_limit_wait()

            got_429 = False
            async with httpx.AsyncClient(timeout=120.0) as client:
                async with client.stream(
                    "POST", self._url, json=payload, headers=self._headers
                ) as resp:
                    if resp.status_code == 429 and attempt < self.MAX_RETRIES:
                        await resp.aread()
                        got_429 = True
                    elif resp.status_code != 200:
                        error_body = await resp.aread()
                        logger.error(
                            "MiniMax API stream error %s: %s",
                            resp.status_code,
                            error_body.decode("utf-8", errors="replace"),
                        )
                        yield f"[LLM Error {resp.status_code}]"
                        return
                    else:
                        # Success — stream the response
                        async for line in resp.aiter_lines():
                            if not line:
                                continue
                            if line.startswith("data:"):
                                data_str = line[len("data:"):].strip()
                                if data_str == "[DONE]":
                                    break
                                try:
                                    chunk = json.loads(data_str)
                                    delta = chunk["choices"][0].get("delta", {})
                                    content = delta.get("content", "")
                                    if content:
                                        yield content
                                except (json.JSONDecodeError, KeyError, IndexError):
                                    continue
                        return  # done streaming

            # Retry on 429 (after exiting the context managers)
            if got_429:
                wait = self.BACKOFF_BASE * (2 ** attempt)
                logger.warning("Rate limited (429), retrying in %ds (%d/%d)", wait, attempt + 1, self.MAX_RETRIES)
                await asyncio.sleep(wait)

        # All retries exhausted
        yield "[LLM Error: rate limit exceeded after retries]"
