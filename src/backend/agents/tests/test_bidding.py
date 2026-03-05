"""Tests for the CNP bidding module."""

from __future__ import annotations

import asyncio
import json
from unittest.mock import AsyncMock, MagicMock, patch

import pytest

from apex_agents.bidding import (
    AgentBid,
    AwardDecision,
    BiddingAgent,
    TaskAnnouncement,
)


# ─────────────────────────────────────────────────────────────────────────────
# Fixtures
# ─────────────────────────────────────────────────────────────────────────────


@pytest.fixture
def sample_announcement() -> TaskAnnouncement:
    """Create a sample task announcement."""
    return TaskAnnouncement(
        task_id="task-001",
        description="Analyze a dataset",
        requirements=["python", "pandas"],
        deadline_secs=30,
        min_bid_count=1,
    )


@pytest.fixture
def bidding_agent() -> BiddingAgent:
    """Create a bidding agent with known capabilities."""
    return BiddingAgent(
        agent_id="agent-test-1",
        capabilities=["python", "pandas", "rust"],
        redis_url="redis://localhost:6379",
        base_cost=0.01,
        complexity_premium=0.005,
        heartbeat_interval=5.0,
        heartbeat_ttl=15,
    )


@pytest.fixture
def mock_redis():
    """Create a mock async Redis client."""
    r = AsyncMock()
    r.rpush = AsyncMock(return_value=1)
    r.setex = AsyncMock(return_value=True)
    r.blpop = AsyncMock(return_value=None)
    r.aclose = AsyncMock()
    return r


# ─────────────────────────────────────────────────────────────────────────────
# TaskAnnouncement Tests
# ─────────────────────────────────────────────────────────────────────────────


class TestTaskAnnouncement:
    """Tests for TaskAnnouncement data class."""

    def test_from_dict(self):
        data = {
            "task_id": "t1",
            "description": "Test",
            "requirements": ["python"],
            "deadline_secs": 60,
            "min_bid_count": 2,
            "metadata": {"priority": "high"},
        }
        ann = TaskAnnouncement.from_dict(data)
        assert ann.task_id == "t1"
        assert ann.requirements == ["python"]
        assert ann.deadline_secs == 60
        assert ann.min_bid_count == 2
        assert ann.metadata["priority"] == "high"

    def test_to_dict_roundtrip(self, sample_announcement):
        data = sample_announcement.to_dict()
        restored = TaskAnnouncement.from_dict(data)
        assert restored.task_id == sample_announcement.task_id
        assert restored.requirements == sample_announcement.requirements

    def test_from_dict_defaults(self):
        data = {"task_id": "t2"}
        ann = TaskAnnouncement.from_dict(data)
        assert ann.description == ""
        assert ann.requirements == []
        assert ann.deadline_secs == 30
        assert ann.min_bid_count == 1


# ─────────────────────────────────────────────────────────────────────────────
# AgentBid Tests
# ─────────────────────────────────────────────────────────────────────────────


class TestAgentBid:
    """Tests for AgentBid data class."""

    def test_to_dict(self):
        bid = AgentBid(
            agent_id="a1",
            task_id="t1",
            estimated_cost=0.05,
            estimated_duration=20.0,
            confidence=0.9,
            capabilities=["python"],
        )
        d = bid.to_dict()
        assert d["agent_id"] == "a1"
        assert d["estimated_cost"] == 0.05
        assert d["capabilities"] == ["python"]

    def test_json_serialization(self):
        bid = AgentBid(
            agent_id="a1",
            task_id="t1",
            estimated_cost=0.05,
            estimated_duration=20.0,
            confidence=0.9,
            capabilities=["python"],
        )
        json_str = json.dumps(bid.to_dict())
        parsed = json.loads(json_str)
        assert parsed["agent_id"] == "a1"


# ─────────────────────────────────────────────────────────────────────────────
# Marginal Cost Tests
# ─────────────────────────────────────────────────────────────────────────────


class TestMarginalCost:
    """Tests for the marginal cost calculation."""

    def test_base_cost_no_load_no_requirements(self, bidding_agent):
        """With no load and no requirements, cost equals base_cost."""
        task = TaskAnnouncement(
            task_id="t1",
            description="Simple task",
            requirements=[],
            deadline_secs=30,
            min_bid_count=1,
        )
        cost = bidding_agent.marginal_cost(task)
        assert cost == bidding_agent.base_cost

    def test_cost_increases_with_requirements(self, bidding_agent):
        """More requirements should increase the cost."""
        task_simple = TaskAnnouncement(
            task_id="t1",
            description="Simple",
            requirements=["python"],
            deadline_secs=30,
            min_bid_count=1,
        )
        task_complex = TaskAnnouncement(
            task_id="t2",
            description="Complex",
            requirements=["python", "pandas", "rust", "docker"],
            deadline_secs=30,
            min_bid_count=1,
        )
        cost_simple = bidding_agent.marginal_cost(task_simple)
        cost_complex = bidding_agent.marginal_cost(task_complex)
        assert cost_complex > cost_simple

    def test_cost_increases_with_queue_depth(self, bidding_agent, sample_announcement):
        """Higher queue depth should increase cost."""
        bidding_agent.current_queue_depth = 0
        cost_idle = bidding_agent.marginal_cost(sample_announcement)

        bidding_agent.current_queue_depth = 10
        cost_busy = bidding_agent.marginal_cost(sample_announcement)

        assert cost_busy > cost_idle

    def test_cost_formula_exact(self, bidding_agent):
        """Verify the exact formula: base + load_factor * depth + premium * len(reqs)."""
        bidding_agent.current_queue_depth = 5
        task = TaskAnnouncement(
            task_id="t1",
            description="Test",
            requirements=["a", "b", "c"],
            deadline_secs=30,
            min_bid_count=1,
        )
        # base_cost=0.01, load_factor=0.002, depth=5, premium=0.005, reqs=3
        expected = 0.01 + 0.002 * 5 + 0.005 * 3
        cost = bidding_agent.marginal_cost(task)
        assert abs(cost - round(expected, 6)) < 1e-9


# ─────────────────────────────────────────────────────────────────────────────
# Task Evaluation Tests
# ─────────────────────────────────────────────────────────────────────────────


class TestEvaluateTask:
    """Tests for task evaluation and bidding decisions."""

    def test_returns_bid_when_capable(self, bidding_agent, sample_announcement):
        """Agent with matching capabilities should produce a bid."""
        bid = bidding_agent.evaluate_task(sample_announcement)
        assert bid is not None
        assert bid.agent_id == bidding_agent.agent_id
        assert bid.task_id == sample_announcement.task_id

    def test_returns_none_when_no_match(self, sample_announcement):
        """Agent with no matching capabilities should return None."""
        agent = BiddingAgent(
            agent_id="no-match",
            capabilities=["javascript", "react"],
        )
        bid = agent.evaluate_task(sample_announcement)
        assert bid is None

    def test_confidence_decreases_with_load(self, bidding_agent, sample_announcement):
        """Confidence should decrease as queue depth increases."""
        bidding_agent.current_queue_depth = 0
        bid_idle = bidding_agent.evaluate_task(sample_announcement)

        bidding_agent.current_queue_depth = 5
        bid_busy = bidding_agent.evaluate_task(sample_announcement)

        assert bid_idle is not None
        assert bid_busy is not None
        assert bid_busy.confidence < bid_idle.confidence

    def test_no_requirements_matches_all(self, bidding_agent):
        """Task with no requirements should accept any agent."""
        task = TaskAnnouncement(
            task_id="t1",
            description="Open task",
            requirements=[],
            deadline_secs=30,
            min_bid_count=1,
        )
        bid = bidding_agent.evaluate_task(task)
        assert bid is not None
        assert bid.confidence > 0


# ─────────────────────────────────────────────────────────────────────────────
# Bid Submission Tests
# ─────────────────────────────────────────────────────────────────────────────


class TestSubmitBid:
    """Tests for bid submission to Redis."""

    @pytest.mark.asyncio
    async def test_submit_bid_calls_rpush(self, bidding_agent, mock_redis):
        """Submitting a bid should RPUSH to the correct Redis key."""
        bidding_agent._redis = mock_redis

        bid = AgentBid(
            agent_id=bidding_agent.agent_id,
            task_id="task-001",
            estimated_cost=0.02,
            estimated_duration=20.0,
            confidence=0.85,
            capabilities=["python"],
        )

        await bidding_agent.submit_bid(bid)

        mock_redis.rpush.assert_called_once()
        call_args = mock_redis.rpush.call_args
        assert call_args[0][0] == "apex:cnp:bids:task-001"

        # Verify the payload is valid JSON with correct fields
        payload = json.loads(call_args[0][1])
        assert payload["agent_id"] == bidding_agent.agent_id
        assert payload["task_id"] == "task-001"
        assert payload["estimated_cost"] == 0.02


# ─────────────────────────────────────────────────────────────────────────────
# Heartbeat Tests
# ─────────────────────────────────────────────────────────────────────────────


class TestHeartbeat:
    """Tests for heartbeat sending."""

    @pytest.mark.asyncio
    async def test_send_heartbeat_calls_setex(self, bidding_agent, mock_redis):
        """Heartbeat should SETEX with correct key and TTL."""
        bidding_agent._redis = mock_redis

        await bidding_agent.send_heartbeat("task-001")

        mock_redis.setex.assert_called_once()
        call_args = mock_redis.setex.call_args
        assert call_args[0][0] == "apex:cnp:heartbeat:task-001"
        assert call_args[0][1] == bidding_agent.heartbeat_ttl

        # Payload should contain agent_id and task_id
        payload = json.loads(call_args[0][2])
        assert payload["agent_id"] == bidding_agent.agent_id
        assert payload["task_id"] == "task-001"
        assert "timestamp" in payload

    @pytest.mark.asyncio
    async def test_complete_task_stops_tracking(self, bidding_agent):
        """Completing a task should remove it from active tracking."""
        bidding_agent._active_tasks.add("task-001")
        bidding_agent._current_queue_depth = 1

        # Create a mock heartbeat task
        mock_task = AsyncMock()
        mock_task.cancel = MagicMock()
        bidding_agent._heartbeat_tasks["task-001"] = mock_task

        bidding_agent.complete_task("task-001")

        assert "task-001" not in bidding_agent._active_tasks
        assert bidding_agent._current_queue_depth == 0
        mock_task.cancel.assert_called_once()


# ─────────────────────────────────────────────────────────────────────────────
# Award Handling Tests
# ─────────────────────────────────────────────────────────────────────────────


class TestHandleAward:
    """Tests for award handling."""

    @pytest.mark.asyncio
    async def test_handle_award_starts_tracking(self, bidding_agent, mock_redis):
        """Handling an award should add the task to active tracking."""
        bidding_agent._redis = mock_redis

        award = AwardDecision(
            task_id="task-001",
            winning_bid={"bid": {"agent_id": bidding_agent.agent_id}},
            runner_up=None,
            total_bids=3,
        )

        await bidding_agent.handle_award(award)

        assert "task-001" in bidding_agent._active_tasks
        assert bidding_agent._current_queue_depth == 1
        assert "task-001" in bidding_agent._heartbeat_tasks

        # Cleanup
        bidding_agent.complete_task("task-001")

    @pytest.mark.asyncio
    async def test_wait_for_award_returns_none_on_timeout(self, bidding_agent, mock_redis):
        """Waiting for an award should return None when BLPOP times out."""
        bidding_agent._redis = mock_redis
        mock_redis.blpop.return_value = None

        result = await bidding_agent.wait_for_award(timeout=1.0)
        assert result is None


# ─────────────────────────────────────────────────────────────────────────────
# AwardDecision Tests
# ─────────────────────────────────────────────────────────────────────────────


class TestAwardDecision:
    """Tests for AwardDecision data class."""

    def test_from_dict(self):
        data = {
            "task_id": "t1",
            "winning_bid": {"bid": {"agent_id": "a1"}, "score": 0.95},
            "runner_up": {"bid": {"agent_id": "a2"}, "score": 0.80},
            "total_bids": 5,
        }
        award = AwardDecision.from_dict(data)
        assert award.task_id == "t1"
        assert award.total_bids == 5
        assert award.runner_up is not None

    def test_from_dict_no_runner_up(self):
        data = {
            "task_id": "t1",
            "winning_bid": {"bid": {"agent_id": "a1"}, "score": 0.95},
            "total_bids": 1,
        }
        award = AwardDecision.from_dict(data)
        assert award.runner_up is None


# ─────────────────────────────────────────────────────────────────────────────
# Queue Depth Property Tests
# ─────────────────────────────────────────────────────────────────────────────


class TestQueueDepth:
    """Tests for queue depth management."""

    def test_queue_depth_cannot_go_negative(self, bidding_agent):
        """Setting queue depth to negative should clamp to zero."""
        bidding_agent.current_queue_depth = -5
        assert bidding_agent.current_queue_depth == 0

    def test_queue_depth_tracks_correctly(self, bidding_agent):
        bidding_agent.current_queue_depth = 3
        assert bidding_agent.current_queue_depth == 3
