"""
CNP (Contract Net Protocol) bidding logic for agents.

This module implements the agent-side of the Contract Net Protocol:
- Listen for task announcements from the orchestrator.
- Evaluate whether the agent can handle the announced task.
- Compute a marginal cost bid using load-aware pricing.
- Submit bids and handle award notifications.
- Send execution heartbeats so the orchestrator can monitor progress.
"""

from __future__ import annotations

import asyncio
import json
import time
import uuid
from dataclasses import dataclass, field
from typing import Any

import redis.asyncio as redis
import structlog

logger = structlog.get_logger()


# ─────────────────────────────────────────────────────────────────────────────
# Data Models
# ─────────────────────────────────────────────────────────────────────────────


@dataclass
class TaskAnnouncement:
    """A task announcement from the orchestrator."""

    task_id: str
    description: str
    requirements: list[str]
    deadline_secs: int
    min_bid_count: int
    metadata: dict[str, Any] = field(default_factory=dict)

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> TaskAnnouncement:
        return cls(
            task_id=data["task_id"],
            description=data.get("description", ""),
            requirements=data.get("requirements", []),
            deadline_secs=data.get("deadline_secs", 30),
            min_bid_count=data.get("min_bid_count", 1),
            metadata=data.get("metadata", {}),
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "task_id": self.task_id,
            "description": self.description,
            "requirements": self.requirements,
            "deadline_secs": self.deadline_secs,
            "min_bid_count": self.min_bid_count,
            "metadata": self.metadata,
        }


@dataclass
class AgentBid:
    """A bid submitted by an agent for a task."""

    agent_id: str
    task_id: str
    estimated_cost: float
    estimated_duration: float
    confidence: float
    capabilities: list[str]

    def to_dict(self) -> dict[str, Any]:
        return {
            "agent_id": self.agent_id,
            "task_id": self.task_id,
            "estimated_cost": self.estimated_cost,
            "estimated_duration": self.estimated_duration,
            "confidence": self.confidence,
            "capabilities": self.capabilities,
        }


@dataclass
class AwardDecision:
    """An award decision from the orchestrator."""

    task_id: str
    winning_bid: dict[str, Any]
    runner_up: dict[str, Any] | None
    total_bids: int

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> AwardDecision:
        return cls(
            task_id=data["task_id"],
            winning_bid=data["winning_bid"],
            runner_up=data.get("runner_up"),
            total_bids=data.get("total_bids", 0),
        )


# ─────────────────────────────────────────────────────────────────────────────
# Bidding Agent
# ─────────────────────────────────────────────────────────────────────────────


class BiddingAgent:
    """
    Agent-side implementation of the Contract Net Protocol.

    Listens for task announcements, computes bids using load-aware marginal
    cost pricing, and manages heartbeats during task execution.

    Args:
        agent_id: Unique identifier for this agent.
        capabilities: List of capability tags this agent supports.
        redis_url: Redis connection URL.
        base_cost: Base cost per task in dollars.
        complexity_premium: Multiplier applied based on task complexity.
        heartbeat_interval: Seconds between heartbeats during execution.
        heartbeat_ttl: TTL for the heartbeat key in seconds.
    """

    # Redis key patterns (must match the Rust side)
    ANNOUNCEMENTS_CHANNEL = "apex:cnp:announcements"
    BIDS_QUEUE_PREFIX = "apex:cnp:bids:"
    AWARDS_QUEUE_PREFIX = "apex:cnp:awards:"
    HEARTBEAT_PREFIX = "apex:cnp:heartbeat:"

    def __init__(
        self,
        agent_id: str | None = None,
        capabilities: list[str] | None = None,
        redis_url: str = "redis://localhost:6379",
        base_cost: float = 0.01,
        complexity_premium: float = 0.005,
        heartbeat_interval: float = 5.0,
        heartbeat_ttl: int = 15,
    ):
        self.agent_id = agent_id or str(uuid.uuid4())
        self.capabilities = capabilities or []
        self.redis_url = redis_url
        self.base_cost = base_cost
        self.complexity_premium = complexity_premium
        self.heartbeat_interval = heartbeat_interval
        self.heartbeat_ttl = heartbeat_ttl

        # Runtime state
        self._redis: redis.Redis | None = None
        self._current_queue_depth: int = 0
        self._active_tasks: set[str] = set()
        self._shutdown_event = asyncio.Event()
        self._heartbeat_tasks: dict[str, asyncio.Task[Any]] = {}
        self._logger = logger.bind(
            component="bidding_agent",
            agent_id=self.agent_id,
        )

    @property
    def current_queue_depth(self) -> int:
        """Current number of tasks in the agent's queue."""
        return self._current_queue_depth

    @current_queue_depth.setter
    def current_queue_depth(self, value: int) -> None:
        self._current_queue_depth = max(0, value)

    # ─────────────────────────────────────────────────────────────────────
    # Connection Management
    # ─────────────────────────────────────────────────────────────────────

    async def connect(self) -> None:
        """Establish the Redis connection."""
        if self._redis is None:
            self._redis = redis.from_url(
                self.redis_url,
                encoding="utf-8",
                decode_responses=True,
            )
            self._logger.debug("Connected to Redis")

    async def close(self) -> None:
        """Close the Redis connection and cancel heartbeats."""
        self._shutdown_event.set()

        # Cancel all heartbeat tasks
        for task_id, task in self._heartbeat_tasks.items():
            task.cancel()
            try:
                await task
            except asyncio.CancelledError:
                pass
        self._heartbeat_tasks.clear()

        if self._redis:
            await self._redis.aclose()
            self._redis = None

    # ─────────────────────────────────────────────────────────────────────
    # Step 1: Listen for Announcements
    # ─────────────────────────────────────────────────────────────────────

    async def listen_for_announcements(
        self,
        callback: Any | None = None,
    ) -> None:
        """
        Subscribe to the CNP announcements channel.

        For each announcement received, evaluates the task and optionally
        submits a bid if the agent has matching capabilities.

        Args:
            callback: Optional async callback invoked with each TaskAnnouncement.
                      If None, the agent auto-evaluates and bids.
        """
        await self.connect()
        assert self._redis is not None

        pubsub = self._redis.pubsub()
        await pubsub.subscribe(self.ANNOUNCEMENTS_CHANNEL)

        self._logger.info("Listening for task announcements")

        try:
            async for message in pubsub.listen():
                if self._shutdown_event.is_set():
                    break

                if message["type"] != "message":
                    continue

                try:
                    data = json.loads(message["data"])
                    announcement = TaskAnnouncement.from_dict(data)

                    if callback:
                        await callback(announcement)
                    else:
                        await self._auto_evaluate_and_bid(announcement)

                except (json.JSONDecodeError, KeyError) as e:
                    self._logger.warning(
                        "Ignoring malformed announcement",
                        error=str(e),
                    )
        finally:
            await pubsub.unsubscribe(self.ANNOUNCEMENTS_CHANNEL)
            await pubsub.aclose()

    async def _auto_evaluate_and_bid(self, announcement: TaskAnnouncement) -> None:
        """Evaluate a task and submit a bid if capable."""
        bid = self.evaluate_task(announcement)
        if bid is not None:
            await self.submit_bid(bid)

    # ─────────────────────────────────────────────────────────────────────
    # Step 2: Evaluate Task
    # ─────────────────────────────────────────────────────────────────────

    def evaluate_task(self, announcement: TaskAnnouncement) -> AgentBid | None:
        """
        Evaluate whether this agent should bid on a task.

        Returns a bid if the agent has at least partial capability match,
        or None if the agent cannot handle the task at all.
        """
        # Check capability overlap
        matched = [
            cap for cap in self.capabilities
            if cap in announcement.requirements
        ]

        if announcement.requirements and not matched:
            self._logger.debug(
                "Skipping task — no capability match",
                task_id=announcement.task_id,
                required=announcement.requirements,
            )
            return None

        # Compute capability confidence
        if announcement.requirements:
            match_ratio = len(matched) / len(announcement.requirements)
        else:
            match_ratio = 1.0

        # Compute cost
        cost = self.marginal_cost(announcement)

        # Estimate duration (heuristic: 10s base + 5s per requirement)
        estimated_duration = 10.0 + 5.0 * len(announcement.requirements)

        # Confidence = capability match ratio * load penalty
        load_penalty = max(0.5, 1.0 - 0.1 * self._current_queue_depth)
        confidence = min(1.0, match_ratio * load_penalty)

        bid = AgentBid(
            agent_id=self.agent_id,
            task_id=announcement.task_id,
            estimated_cost=cost,
            estimated_duration=estimated_duration,
            confidence=confidence,
            capabilities=matched if announcement.requirements else list(self.capabilities),
        )

        self._logger.debug(
            "Computed bid",
            task_id=announcement.task_id,
            cost=cost,
            confidence=confidence,
        )

        return bid

    # ─────────────────────────────────────────────────────────────────────
    # Step 3: Marginal Cost
    # ─────────────────────────────────────────────────────────────────────

    def marginal_cost(self, task: TaskAnnouncement) -> float:
        """
        Compute the marginal cost of executing a task.

        Formula:
            cost = base_cost + load_factor * current_queue_depth + complexity_premium * num_requirements

        The load factor increases cost when the agent is already busy,
        discouraging overbidding. The complexity premium scales with the
        number of required capabilities.
        """
        load_factor = 0.002  # cost increase per queued task
        cost = (
            self.base_cost
            + load_factor * self._current_queue_depth
            + self.complexity_premium * len(task.requirements)
        )
        return round(cost, 6)

    # ─────────────────────────────────────────────────────────────────────
    # Step 4: Submit Bid
    # ─────────────────────────────────────────────────────────────────────

    async def submit_bid(self, bid: AgentBid) -> None:
        """
        Submit a bid to the per-task bid queue.

        The bid is serialized as JSON and pushed to the Redis list
        ``apex:cnp:bids:{task_id}``.
        """
        await self.connect()
        assert self._redis is not None

        key = f"{self.BIDS_QUEUE_PREFIX}{bid.task_id}"
        payload = json.dumps(bid.to_dict())

        await self._redis.rpush(key, payload)

        self._logger.info(
            "Bid submitted",
            task_id=bid.task_id,
            cost=bid.estimated_cost,
            confidence=bid.confidence,
        )

    # ─────────────────────────────────────────────────────────────────────
    # Step 5: Handle Award
    # ─────────────────────────────────────────────────────────────────────

    async def handle_award(self, award: AwardDecision) -> None:
        """
        Handle a task award from the orchestrator.

        Begins task execution tracking and starts the heartbeat loop.
        """
        task_id = award.task_id
        self._active_tasks.add(task_id)
        self._current_queue_depth += 1

        self._logger.info(
            "Task awarded — starting execution",
            task_id=task_id,
        )

        # Start heartbeat for this task
        heartbeat_task = asyncio.create_task(self._heartbeat_loop(task_id))
        self._heartbeat_tasks[task_id] = heartbeat_task

    async def wait_for_award(self, timeout: float = 30.0) -> AwardDecision | None:
        """
        Wait for an award decision on this agent's award queue.

        Args:
            timeout: Maximum time to wait in seconds.

        Returns:
            The AwardDecision if received, or None on timeout.
        """
        await self.connect()
        assert self._redis is not None

        key = f"{self.AWARDS_QUEUE_PREFIX}{self.agent_id}"
        result = await self._redis.blpop(key, timeout=int(timeout))

        if result is None:
            return None

        _key, value = result
        data = json.loads(value)
        return AwardDecision.from_dict(data)

    # ─────────────────────────────────────────────────────────────────────
    # Step 6: Heartbeat
    # ─────────────────────────────────────────────────────────────────────

    async def send_heartbeat(self, task_id: str) -> None:
        """
        Send a single heartbeat for a task.

        Sets the key ``apex:cnp:heartbeat:{task_id}`` with a TTL so the
        orchestrator can detect when the agent stops reporting.
        """
        await self.connect()
        assert self._redis is not None

        key = f"{self.HEARTBEAT_PREFIX}{task_id}"
        heartbeat_data = json.dumps({
            "agent_id": self.agent_id,
            "task_id": task_id,
            "timestamp": time.time(),
        })

        await self._redis.setex(key, self.heartbeat_ttl, heartbeat_data)

    async def _heartbeat_loop(self, task_id: str) -> None:
        """Continuously send heartbeats for a task until cancelled."""
        self._logger.debug("Starting heartbeat", task_id=task_id)

        try:
            while not self._shutdown_event.is_set():
                await self.send_heartbeat(task_id)
                await asyncio.sleep(self.heartbeat_interval)
        except asyncio.CancelledError:
            self._logger.debug("Heartbeat cancelled", task_id=task_id)

    def complete_task(self, task_id: str) -> None:
        """
        Mark a task as completed and stop its heartbeat.

        Call this when the agent finishes executing an awarded task.
        """
        self._active_tasks.discard(task_id)
        self._current_queue_depth = max(0, self._current_queue_depth - 1)

        if task_id in self._heartbeat_tasks:
            self._heartbeat_tasks[task_id].cancel()
            del self._heartbeat_tasks[task_id]

        self._logger.info("Task completed", task_id=task_id)
