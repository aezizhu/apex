"""
Agent state, task result, and conversation persistence.

Provides stores backed by PostgreSQL (via asyncpg) for:
- Saving / loading agent state across restarts
- Persisting task outputs for auditing and replay
- Storing multi-turn conversation history per session
"""

from __future__ import annotations

import json
import uuid
from dataclasses import asdict, dataclass, field
from datetime import UTC, datetime
from typing import Any

import structlog

logger = structlog.get_logger()


# ---------------------------------------------------------------------------
# SQL schemas – apply via migration tooling or execute manually
# ---------------------------------------------------------------------------

PERSISTENCE_TABLE_SQL = """
-- Serialised agent state snapshots
CREATE TABLE IF NOT EXISTS agent_state (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    agent_id        TEXT NOT NULL,
    agent_name      TEXT NOT NULL,
    session_id      TEXT NOT NULL,
    state_data      JSONB NOT NULL DEFAULT '{}'::jsonb,
    status          TEXT NOT NULL DEFAULT 'idle',
    saved_at        TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_agent_state_agent_id ON agent_state (agent_id);
CREATE INDEX IF NOT EXISTS idx_agent_state_session_id ON agent_state (session_id);
CREATE INDEX IF NOT EXISTS idx_agent_state_saved_at ON agent_state (saved_at DESC);

-- Persisted task results
CREATE TABLE IF NOT EXISTS task_results (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    task_id         TEXT NOT NULL,
    agent_id        TEXT NOT NULL,
    session_id      TEXT NOT NULL,
    status          TEXT NOT NULL,
    result_text     TEXT,
    result_data     JSONB NOT NULL DEFAULT '{}'::jsonb,
    error           TEXT,
    tokens_used     BIGINT NOT NULL DEFAULT 0,
    cost_dollars    DOUBLE PRECISION NOT NULL DEFAULT 0.0,
    duration_ms     INTEGER NOT NULL DEFAULT 0,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_task_results_task_id ON task_results (task_id);
CREATE INDEX IF NOT EXISTS idx_task_results_agent_id ON task_results (agent_id);
CREATE INDEX IF NOT EXISTS idx_task_results_session_id ON task_results (session_id);
CREATE INDEX IF NOT EXISTS idx_task_results_created_at ON task_results (created_at DESC);

-- Multi-turn conversation history
CREATE TABLE IF NOT EXISTS conversation_history (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    session_id      TEXT NOT NULL,
    agent_id        TEXT NOT NULL,
    role            TEXT NOT NULL,
    content         TEXT NOT NULL DEFAULT '',
    tool_calls      JSONB,
    tool_call_id    TEXT,
    message_index   INTEGER NOT NULL DEFAULT 0,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_conversation_session_id ON conversation_history (session_id);
CREATE INDEX IF NOT EXISTS idx_conversation_agent_id ON conversation_history (agent_id);
CREATE INDEX IF NOT EXISTS idx_conversation_ordering ON conversation_history (session_id, message_index);
"""


# ---------------------------------------------------------------------------
# Data transfer objects
# ---------------------------------------------------------------------------

@dataclass
class AgentStateRecord:
    agent_id: str
    agent_name: str
    session_id: str
    state_data: dict[str, Any] = field(default_factory=dict)
    status: str = "idle"
    saved_at: datetime | None = None


@dataclass
class TaskResultRecord:
    task_id: str
    agent_id: str
    session_id: str
    status: str
    result_text: str | None = None
    result_data: dict[str, Any] = field(default_factory=dict)
    error: str | None = None
    tokens_used: int = 0
    cost_dollars: float = 0.0
    duration_ms: int = 0
    created_at: datetime | None = None


@dataclass
class ConversationMessage:
    session_id: str
    agent_id: str
    role: str
    content: str = ""
    tool_calls: list[dict[str, Any]] | None = None
    tool_call_id: str | None = None
    message_index: int = 0
    created_at: datetime | None = None


# ---------------------------------------------------------------------------
# Base store with shared pool management
# ---------------------------------------------------------------------------

class _BaseStore:
    """Shared helpers for persistence stores."""

    def __init__(self, database_url: str):
        self.database_url = database_url
        self._pool: Any = None  # asyncpg.Pool
        self._logger = logger.bind(component=self.__class__.__name__)

    async def connect(self) -> None:
        try:
            import asyncpg  # type: ignore[import-untyped]
        except ImportError:
            self._logger.warning("asyncpg not installed; persistence disabled")
            return

        self._pool = await asyncpg.create_pool(self.database_url, min_size=1, max_size=5)
        async with self._pool.acquire() as conn:
            await conn.execute(PERSISTENCE_TABLE_SQL)
        self._logger.info("Persistence store connected")

    async def close(self) -> None:
        if self._pool:
            await self._pool.close()
            self._pool = None


# ---------------------------------------------------------------------------
# AgentStateStore
# ---------------------------------------------------------------------------

class AgentStateStore(_BaseStore):
    """Save and load serialised agent state to PostgreSQL."""

    async def save(self, record: AgentStateRecord) -> str:
        """Persist agent state. Returns the generated row id."""
        if not self._pool:
            self._logger.warning("No DB pool; skipping state save")
            return ""

        row = await self._pool.fetchrow(
            """
            INSERT INTO agent_state
                (agent_id, agent_name, session_id, state_data, status, saved_at)
            VALUES ($1, $2, $3, $4::jsonb, $5, $6)
            RETURNING id
            """,
            record.agent_id,
            record.agent_name,
            record.session_id,
            json.dumps(record.state_data),
            record.status,
            record.saved_at or datetime.now(UTC),
        )
        self._logger.debug("Saved agent state", agent_id=record.agent_id)
        return str(row["id"])

    async def load_latest(self, agent_id: str) -> AgentStateRecord | None:
        """Load the most recent state snapshot for an agent."""
        if not self._pool:
            return None

        row = await self._pool.fetchrow(
            """
            SELECT agent_id, agent_name, session_id, state_data, status, saved_at
            FROM agent_state
            WHERE agent_id = $1
            ORDER BY saved_at DESC
            LIMIT 1
            """,
            agent_id,
        )
        if row is None:
            return None

        return AgentStateRecord(
            agent_id=row["agent_id"],
            agent_name=row["agent_name"],
            session_id=row["session_id"],
            state_data=json.loads(row["state_data"]) if isinstance(row["state_data"], str) else row["state_data"],
            status=row["status"],
            saved_at=row["saved_at"],
        )

    async def list_sessions(self, agent_id: str, limit: int = 20) -> list[AgentStateRecord]:
        """List recent state snapshots for an agent."""
        if not self._pool:
            return []

        rows = await self._pool.fetch(
            """
            SELECT agent_id, agent_name, session_id, state_data, status, saved_at
            FROM agent_state
            WHERE agent_id = $1
            ORDER BY saved_at DESC
            LIMIT $2
            """,
            agent_id,
            limit,
        )
        return [
            AgentStateRecord(
                agent_id=r["agent_id"],
                agent_name=r["agent_name"],
                session_id=r["session_id"],
                state_data=json.loads(r["state_data"]) if isinstance(r["state_data"], str) else r["state_data"],
                status=r["status"],
                saved_at=r["saved_at"],
            )
            for r in rows
        ]


# ---------------------------------------------------------------------------
# TaskResultStore
# ---------------------------------------------------------------------------

class TaskResultStore(_BaseStore):
    """Persist task execution outputs."""

    async def save(self, record: TaskResultRecord) -> str:
        if not self._pool:
            self._logger.warning("No DB pool; skipping task result save")
            return ""

        row = await self._pool.fetchrow(
            """
            INSERT INTO task_results
                (task_id, agent_id, session_id, status,
                 result_text, result_data, error,
                 tokens_used, cost_dollars, duration_ms, created_at)
            VALUES ($1, $2, $3, $4, $5, $6::jsonb, $7, $8, $9, $10, $11)
            RETURNING id
            """,
            record.task_id,
            record.agent_id,
            record.session_id,
            record.status,
            record.result_text,
            json.dumps(record.result_data),
            record.error,
            record.tokens_used,
            record.cost_dollars,
            record.duration_ms,
            record.created_at or datetime.now(UTC),
        )
        self._logger.debug("Saved task result", task_id=record.task_id)
        return str(row["id"])

    async def get_by_task_id(self, task_id: str) -> TaskResultRecord | None:
        if not self._pool:
            return None

        row = await self._pool.fetchrow(
            """
            SELECT task_id, agent_id, session_id, status,
                   result_text, result_data, error,
                   tokens_used, cost_dollars, duration_ms, created_at
            FROM task_results
            WHERE task_id = $1
            ORDER BY created_at DESC
            LIMIT 1
            """,
            task_id,
        )
        if row is None:
            return None

        return TaskResultRecord(
            task_id=row["task_id"],
            agent_id=row["agent_id"],
            session_id=row["session_id"],
            status=row["status"],
            result_text=row["result_text"],
            result_data=json.loads(row["result_data"]) if isinstance(row["result_data"], str) else row["result_data"],
            error=row["error"],
            tokens_used=row["tokens_used"],
            cost_dollars=row["cost_dollars"],
            duration_ms=row["duration_ms"],
            created_at=row["created_at"],
        )

    async def list_by_session(
        self, session_id: str, limit: int = 50
    ) -> list[TaskResultRecord]:
        if not self._pool:
            return []

        rows = await self._pool.fetch(
            """
            SELECT task_id, agent_id, session_id, status,
                   result_text, result_data, error,
                   tokens_used, cost_dollars, duration_ms, created_at
            FROM task_results
            WHERE session_id = $1
            ORDER BY created_at DESC
            LIMIT $2
            """,
            session_id,
            limit,
        )
        return [
            TaskResultRecord(
                task_id=r["task_id"],
                agent_id=r["agent_id"],
                session_id=r["session_id"],
                status=r["status"],
                result_text=r["result_text"],
                result_data=json.loads(r["result_data"]) if isinstance(r["result_data"], str) else r["result_data"],
                error=r["error"],
                tokens_used=r["tokens_used"],
                cost_dollars=r["cost_dollars"],
                duration_ms=r["duration_ms"],
                created_at=r["created_at"],
            )
            for r in rows
        ]


# ---------------------------------------------------------------------------
# ConversationHistory
# ---------------------------------------------------------------------------

class ConversationHistory(_BaseStore):
    """Save and replay multi-turn conversations."""

    async def append(self, message: ConversationMessage) -> str:
        if not self._pool:
            self._logger.warning("No DB pool; skipping conversation append")
            return ""

        row = await self._pool.fetchrow(
            """
            INSERT INTO conversation_history
                (session_id, agent_id, role, content,
                 tool_calls, tool_call_id, message_index, created_at)
            VALUES ($1, $2, $3, $4, $5::jsonb, $6, $7, $8)
            RETURNING id
            """,
            message.session_id,
            message.agent_id,
            message.role,
            message.content,
            json.dumps(message.tool_calls) if message.tool_calls else None,
            message.tool_call_id,
            message.message_index,
            message.created_at or datetime.now(UTC),
        )
        return str(row["id"])

    async def append_many(self, messages: list[ConversationMessage]) -> None:
        """Batch-insert multiple messages."""
        if not self._pool or not messages:
            return

        async with self._pool.acquire() as conn:
            async with conn.transaction():
                for msg in messages:
                    await conn.execute(
                        """
                        INSERT INTO conversation_history
                            (session_id, agent_id, role, content,
                             tool_calls, tool_call_id, message_index, created_at)
                        VALUES ($1, $2, $3, $4, $5::jsonb, $6, $7, $8)
                        """,
                        msg.session_id,
                        msg.agent_id,
                        msg.role,
                        msg.content,
                        json.dumps(msg.tool_calls) if msg.tool_calls else None,
                        msg.tool_call_id,
                        msg.message_index,
                        msg.created_at or datetime.now(UTC),
                    )
        self._logger.debug(
            "Saved conversation batch",
            session_id=messages[0].session_id,
            count=len(messages),
        )

    async def load_session(
        self,
        session_id: str,
        agent_id: str | None = None,
        limit: int = 200,
    ) -> list[ConversationMessage]:
        """Load conversation messages for a session, ordered by index."""
        if not self._pool:
            return []

        if agent_id:
            rows = await self._pool.fetch(
                """
                SELECT session_id, agent_id, role, content,
                       tool_calls, tool_call_id, message_index, created_at
                FROM conversation_history
                WHERE session_id = $1 AND agent_id = $2
                ORDER BY message_index ASC
                LIMIT $3
                """,
                session_id,
                agent_id,
                limit,
            )
        else:
            rows = await self._pool.fetch(
                """
                SELECT session_id, agent_id, role, content,
                       tool_calls, tool_call_id, message_index, created_at
                FROM conversation_history
                WHERE session_id = $1
                ORDER BY message_index ASC
                LIMIT $2
                """,
                session_id,
                limit,
            )

        return [
            ConversationMessage(
                session_id=r["session_id"],
                agent_id=r["agent_id"],
                role=r["role"],
                content=r["content"],
                tool_calls=json.loads(r["tool_calls"]) if r["tool_calls"] else None,
                tool_call_id=r["tool_call_id"],
                message_index=r["message_index"],
                created_at=r["created_at"],
            )
            for r in rows
        ]

    async def delete_session(self, session_id: str) -> int:
        """Delete all messages for a session. Returns rows deleted."""
        if not self._pool:
            return 0

        result = await self._pool.execute(
            "DELETE FROM conversation_history WHERE session_id = $1",
            session_id,
        )
        # asyncpg returns e.g. "DELETE 42"
        count = int(result.split()[-1])
        self._logger.debug("Deleted conversation", session_id=session_id, rows=count)
        return count
