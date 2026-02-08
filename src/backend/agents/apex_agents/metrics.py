"""
Agent metrics collection and persistence.

Provides per-agent and per-session metrics tracking with periodic
flush to PostgreSQL for cost analysis and operational visibility.
"""

from __future__ import annotations

import asyncio
import time
import uuid
from collections import defaultdict
from dataclasses import dataclass, field
from datetime import UTC, datetime
from typing import Any

import structlog

logger = structlog.get_logger()


# ---------------------------------------------------------------------------
# Data structures
# ---------------------------------------------------------------------------

@dataclass
class ModelCostEntry:
    """Accumulated cost data for a single model."""
    model: str
    total_tokens: int = 0
    prompt_tokens: int = 0
    completion_tokens: int = 0
    total_cost: float = 0.0
    call_count: int = 0


@dataclass
class MetricsSnapshot:
    """Point-in-time snapshot of an agent's metrics."""
    agent_id: str
    session_id: str
    timestamp: datetime
    total_tokens: int
    total_cost: float
    tasks_completed: int
    tasks_failed: int
    avg_latency_ms: float
    tool_calls: int
    cost_by_model: dict[str, ModelCostEntry]


# ---------------------------------------------------------------------------
# MetricsCollector – in-memory accumulator attached to a single agent
# ---------------------------------------------------------------------------

class MetricsCollector:
    """
    Collects runtime metrics for a single agent instance.

    Thread-safe for asyncio (single-threaded event loop).
    Call ``record_task`` after each task execution to update counters,
    and ``snapshot`` to obtain a serialisable point-in-time view.
    """

    def __init__(self, agent_id: str, session_id: str | None = None):
        self.agent_id = agent_id
        self.session_id = session_id or str(uuid.uuid4())

        # Counters
        self.total_tokens: int = 0
        self.total_cost: float = 0.0
        self.tasks_completed: int = 0
        self.tasks_failed: int = 0
        self.tool_calls: int = 0

        # Latency tracking
        self._latencies_ms: list[float] = []

        # Per-model cost breakdown
        self._cost_by_model: dict[str, ModelCostEntry] = defaultdict(
            lambda: ModelCostEntry(model="")
        )

        self._logger = logger.bind(
            component="metrics_collector",
            agent_id=agent_id,
            session_id=self.session_id,
        )

    # -- recording helpers ---------------------------------------------------

    def record_task(
        self,
        *,
        success: bool,
        tokens_used: int,
        cost: float,
        latency_ms: float,
        tool_calls: int,
        model: str,
        prompt_tokens: int = 0,
        completion_tokens: int = 0,
    ) -> None:
        """Record metrics from a completed task execution."""
        if success:
            self.tasks_completed += 1
        else:
            self.tasks_failed += 1

        self.total_tokens += tokens_used
        self.total_cost += cost
        self.tool_calls += tool_calls
        self._latencies_ms.append(latency_ms)

        entry = self._cost_by_model[model]
        entry.model = model
        entry.total_tokens += tokens_used
        entry.prompt_tokens += prompt_tokens
        entry.completion_tokens += completion_tokens
        entry.total_cost += cost
        entry.call_count += 1

        self._logger.debug(
            "Recorded task metrics",
            success=success,
            tokens=tokens_used,
            cost=cost,
            latency_ms=latency_ms,
        )

    def record_llm_call(
        self,
        *,
        model: str,
        prompt_tokens: int,
        completion_tokens: int,
        cost: float,
    ) -> None:
        """Record a single LLM call (finer-grained than per-task)."""
        total = prompt_tokens + completion_tokens
        self.total_tokens += total
        self.total_cost += cost

        entry = self._cost_by_model[model]
        entry.model = model
        entry.total_tokens += total
        entry.prompt_tokens += prompt_tokens
        entry.completion_tokens += completion_tokens
        entry.total_cost += cost
        entry.call_count += 1

    # -- queries -------------------------------------------------------------

    @property
    def avg_latency_ms(self) -> float:
        if not self._latencies_ms:
            return 0.0
        return sum(self._latencies_ms) / len(self._latencies_ms)

    def snapshot(self) -> MetricsSnapshot:
        """Return an immutable snapshot of the current metrics."""
        return MetricsSnapshot(
            agent_id=self.agent_id,
            session_id=self.session_id,
            timestamp=datetime.now(UTC),
            total_tokens=self.total_tokens,
            total_cost=self.total_cost,
            tasks_completed=self.tasks_completed,
            tasks_failed=self.tasks_failed,
            avg_latency_ms=self.avg_latency_ms,
            tool_calls=self.tool_calls,
            cost_by_model=dict(self._cost_by_model),
        )

    def reset(self) -> None:
        """Reset all counters (e.g. after a successful flush)."""
        self.total_tokens = 0
        self.total_cost = 0.0
        self.tasks_completed = 0
        self.tasks_failed = 0
        self.tool_calls = 0
        self._latencies_ms.clear()
        self._cost_by_model.clear()


# ---------------------------------------------------------------------------
# MetricsPersister – periodic background flush to PostgreSQL
# ---------------------------------------------------------------------------

# SQL table definitions – apply via migration tooling or execute manually.
METRICS_TABLE_SQL = """
-- Agent metrics snapshots
CREATE TABLE IF NOT EXISTS agent_metrics (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    agent_id        TEXT NOT NULL,
    session_id      TEXT NOT NULL,
    recorded_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    total_tokens    BIGINT NOT NULL DEFAULT 0,
    total_cost      DOUBLE PRECISION NOT NULL DEFAULT 0.0,
    tasks_completed INTEGER NOT NULL DEFAULT 0,
    tasks_failed    INTEGER NOT NULL DEFAULT 0,
    avg_latency_ms  DOUBLE PRECISION NOT NULL DEFAULT 0.0,
    tool_calls      INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_agent_metrics_agent_id ON agent_metrics (agent_id);
CREATE INDEX IF NOT EXISTS idx_agent_metrics_session_id ON agent_metrics (session_id);
CREATE INDEX IF NOT EXISTS idx_agent_metrics_recorded_at ON agent_metrics (recorded_at);

-- Per-model cost breakdown rows linked to a snapshot
CREATE TABLE IF NOT EXISTS agent_model_costs (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    metrics_id          UUID NOT NULL REFERENCES agent_metrics (id) ON DELETE CASCADE,
    model               TEXT NOT NULL,
    total_tokens        BIGINT NOT NULL DEFAULT 0,
    prompt_tokens       BIGINT NOT NULL DEFAULT 0,
    completion_tokens   BIGINT NOT NULL DEFAULT 0,
    total_cost          DOUBLE PRECISION NOT NULL DEFAULT 0.0,
    call_count          INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_agent_model_costs_metrics_id ON agent_model_costs (metrics_id);
CREATE INDEX IF NOT EXISTS idx_agent_model_costs_model ON agent_model_costs (model);
"""


class MetricsPersister:
    """
    Periodically flushes ``MetricsCollector`` snapshots to PostgreSQL.

    Uses ``asyncpg`` for async database access.  Call ``start`` to begin
    the background flush loop and ``stop`` to drain and shut down.

    Args:
        database_url: PostgreSQL connection string.
        flush_interval_seconds: How often to flush (default 60s).
    """

    def __init__(
        self,
        database_url: str,
        flush_interval_seconds: float = 60.0,
    ):
        self.database_url = database_url
        self.flush_interval = flush_interval_seconds
        self._collectors: list[MetricsCollector] = []
        self._pool: Any = None  # asyncpg.Pool
        self._flush_task: asyncio.Task[Any] | None = None
        self._stop_event = asyncio.Event()
        self._logger = logger.bind(component="metrics_persister")

    # -- lifecycle -----------------------------------------------------------

    def register(self, collector: MetricsCollector) -> None:
        """Register a collector whose snapshots will be flushed."""
        self._collectors.append(collector)

    async def start(self) -> None:
        """Open the connection pool and start the flush loop."""
        try:
            import asyncpg  # type: ignore[import-untyped]
        except ImportError:
            self._logger.warning(
                "asyncpg is not installed; metrics will NOT be persisted to PostgreSQL"
            )
            return

        self._pool = await asyncpg.create_pool(self.database_url, min_size=1, max_size=5)
        self._logger.info("Metrics persister connected to PostgreSQL")

        # Ensure tables exist
        async with self._pool.acquire() as conn:
            await conn.execute(METRICS_TABLE_SQL)

        self._stop_event.clear()
        self._flush_task = asyncio.create_task(self._flush_loop())

    async def stop(self) -> None:
        """Flush remaining metrics and shut down."""
        self._stop_event.set()

        if self._flush_task and not self._flush_task.done():
            try:
                await asyncio.wait_for(self._flush_task, timeout=10.0)
            except asyncio.TimeoutError:
                self._flush_task.cancel()

        # Final flush
        await self._flush_all()

        if self._pool:
            await self._pool.close()
            self._pool = None

        self._logger.info("Metrics persister stopped")

    # -- internal ------------------------------------------------------------

    async def _flush_loop(self) -> None:
        while not self._stop_event.is_set():
            try:
                await asyncio.sleep(self.flush_interval)
                await self._flush_all()
            except asyncio.CancelledError:
                break
            except Exception:
                self._logger.exception("Error during metrics flush")

    async def _flush_all(self) -> None:
        if not self._pool:
            return

        for collector in self._collectors:
            snap = collector.snapshot()
            if snap.total_tokens == 0 and snap.tasks_completed == 0 and snap.tasks_failed == 0:
                continue  # nothing to persist

            try:
                await self._write_snapshot(snap)
                collector.reset()
                self._logger.debug(
                    "Flushed metrics",
                    agent_id=snap.agent_id,
                    tokens=snap.total_tokens,
                    cost=snap.total_cost,
                )
            except Exception:
                self._logger.exception(
                    "Failed to flush metrics for agent",
                    agent_id=snap.agent_id,
                )

    async def _write_snapshot(self, snap: MetricsSnapshot) -> None:
        async with self._pool.acquire() as conn:
            async with conn.transaction():
                row = await conn.fetchrow(
                    """
                    INSERT INTO agent_metrics
                        (agent_id, session_id, recorded_at,
                         total_tokens, total_cost,
                         tasks_completed, tasks_failed,
                         avg_latency_ms, tool_calls)
                    VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                    RETURNING id
                    """,
                    snap.agent_id,
                    snap.session_id,
                    snap.timestamp,
                    snap.total_tokens,
                    snap.total_cost,
                    snap.tasks_completed,
                    snap.tasks_failed,
                    snap.avg_latency_ms,
                    snap.tool_calls,
                )
                metrics_id = row["id"]

                for entry in snap.cost_by_model.values():
                    await conn.execute(
                        """
                        INSERT INTO agent_model_costs
                            (metrics_id, model, total_tokens,
                             prompt_tokens, completion_tokens,
                             total_cost, call_count)
                        VALUES ($1, $2, $3, $4, $5, $6, $7)
                        """,
                        metrics_id,
                        entry.model,
                        entry.total_tokens,
                        entry.prompt_tokens,
                        entry.completion_tokens,
                        entry.total_cost,
                        entry.call_count,
                    )
