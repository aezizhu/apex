"""Event emitter for pushing real-time updates to WebSocket clients."""

from __future__ import annotations

import asyncio
import logging
from typing import Any

logger = logging.getLogger("apex.swarm.events")

# Recognised event types
EVENT_TYPES = {
    "agent_spawned",
    "agent_status",
    "phase_change",
    "agent_output",
    "task_update",
    "swarm_complete",
    "swarm_cancelled",
    "error",
    # Report pipeline events
    "report_stage",       # stage started/completed (template_selection, layout_design, etc.)
    "report_progress",    # overall progress percentage + message
    "chapter_status",     # per-chapter generation status (started, retry, completed, failed)
}

# Maximum number of events to buffer per session (prevents unbounded memory growth)
MAX_BUFFER_SIZE = 1000


class EventEmitter:
    """Fan-out event emitter backed by asyncio.Queues (one per subscriber).

    All emitted events are buffered per session so that late-joining
    subscribers (e.g. a WebSocket that connects after the engine has
    already started) receive the full event history on subscribe.
    """

    def __init__(self) -> None:
        # session_id → set of asyncio.Queue
        self._subscribers: dict[str, set[asyncio.Queue]] = {}
        # session_id → ordered list of past events
        self._buffers: dict[str, list[dict]] = {}

    def subscribe(self, session_id: str, queue: asyncio.Queue) -> None:
        """Register a queue to receive events for *session_id*.

        Any events emitted before this call are replayed into the queue
        so the subscriber sees the complete history.
        """
        self._subscribers.setdefault(session_id, set()).add(queue)
        for event in self._buffers.get(session_id, []):
            queue.put_nowait(event)

    def unsubscribe(self, session_id: str, queue: asyncio.Queue) -> None:
        """Remove a previously registered queue."""
        queues = self._subscribers.get(session_id)
        if queues:
            queues.discard(queue)
            if not queues:
                del self._subscribers[session_id]

    async def emit(self, session_id: str, event_type: str, data: Any) -> None:
        """Push an event to every subscriber of *session_id*."""
        event = {"type": event_type, **data}

        # Get or create buffer for this session
        buffer = self._buffers.setdefault(session_id, [])

        # Implement eviction policy: remove oldest events when buffer is full
        if len(buffer) >= MAX_BUFFER_SIZE:
            # Remove oldest 10% of events to make room for new ones
            evict_count = max(1, MAX_BUFFER_SIZE // 10)
            buffer = buffer[evict_count:]
            self._buffers[session_id] = buffer

        buffer.append(event)

        for queue in list(self._subscribers.get(session_id, [])):
            try:
                await queue.put(event)
            except Exception:
                logger.exception("Failed to enqueue event for session %s", session_id)

    def clear_buffer(self, session_id: str) -> None:
        """Remove buffered events for a completed/cancelled session."""
        self._buffers.pop(session_id, None)
