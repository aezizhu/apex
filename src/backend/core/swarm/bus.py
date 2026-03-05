"""Message bus for inter-agent communication within a swarm session."""

from __future__ import annotations

from .models import AgentMessage, SwarmSession


class MessageBus:
    """Simple in-memory message bus scoped to a single SwarmSession."""

    def __init__(self, session: SwarmSession) -> None:
        self._session = session

    def post(self, message: AgentMessage) -> None:
        """Add a message to the session's message list."""
        self._session.messages.append(message)

    def get_messages(
        self,
        agent_id: str | None = None,
        msg_type: str | None = None,
    ) -> list[AgentMessage]:
        """Return messages filtered by recipient agent and/or type."""
        results = self._session.messages
        if agent_id is not None:
            results = [m for m in results if m.to_agent == agent_id]
        if msg_type is not None:
            results = [m for m in results if m.msg_type == msg_type]
        return results
