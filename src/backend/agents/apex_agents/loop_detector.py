"""
Loop detection and cost-per-insight tracking for agent execution.

Detects when an agent is stuck in a loop by comparing recent outputs
using multiple detection strategies: exact hash matching, Jaccard
similarity, oscillation detection, and length-based analysis.

Also tracks cost-per-insight to detect diminishing returns.
"""

from __future__ import annotations

import hashlib
import time
from collections import deque
from dataclasses import dataclass, field
from enum import Enum
from typing import Optional

import structlog

logger = structlog.get_logger()


class LoopType(str, Enum):
    """Type of loop detected."""

    EXACT_REPEAT = "exact_repeat"
    SEMANTIC_LOOP = "semantic_loop"
    OSCILLATION = "oscillation"
    LENGTH_STAGNATION = "length_stagnation"


@dataclass(frozen=True)
class LoopDetectionResult:
    """Result of a loop detection check."""

    is_loop: bool
    confidence: float
    loop_type: Optional[LoopType]
    suggestion: str

    def __str__(self) -> str:
        if not self.is_loop:
            return "No loop detected"
        return (
            f"Loop detected ({self.loop_type.value}, "
            f"confidence={self.confidence:.2f}): {self.suggestion}"
        )


@dataclass
class LoopDetector:
    """Detects when an agent is stuck in a loop by comparing recent outputs.

    Uses four detection strategies:
    1. Exact hash matching - catches identical repeated outputs
    2. Jaccard similarity - catches near-duplicate outputs at word level
    3. Oscillation detection - catches A-B-A-B alternating patterns
    4. Length stagnation - catches outputs stuck at identical lengths

    Example:
        detector = LoopDetector()
        for output in agent_outputs:
            result = detector.check(output)
            if result.is_loop:
                print(f"Loop detected: {result}")
                break
    """

    window_size: int = 10
    similarity_threshold: float = 0.85
    hash_threshold: int = 3
    length_stagnation_window: int = 5
    _recent_outputs: deque = field(default_factory=lambda: deque(maxlen=10))
    _output_hashes: deque = field(default_factory=lambda: deque(maxlen=20))
    _output_lengths: deque = field(default_factory=lambda: deque(maxlen=10))

    def __post_init__(self) -> None:
        # Ensure deque maxlens match configured window sizes
        self._recent_outputs = deque(maxlen=self.window_size)
        self._output_hashes = deque(maxlen=self.window_size * 2)
        self._output_lengths = deque(maxlen=self.window_size)

    def check(self, output: str) -> LoopDetectionResult:
        """Check if the output indicates a loop.

        Args:
            output: The latest agent output text.

        Returns:
            LoopDetectionResult indicating whether a loop was detected,
            with confidence score, loop type, and suggested action.
        """
        output_hash = hashlib.sha256(output.encode()).hexdigest()[:16]

        # Method 1: Exact hash matching (highest priority)
        result = self._check_exact_repeat(output_hash)
        if result is not None:
            self._record(output, output_hash)
            return result

        # Method 2: Oscillation detection (A-B-A-B pattern)
        # Record first so oscillation can see the current hash in history
        self._record(output, output_hash)
        result = self._check_oscillation()
        if result is not None:
            return result

        # Method 3: Jaccard similarity (word-level)
        result = self._check_similarity(output)
        if result is not None:
            return result

        # Method 4: Length stagnation
        result = self._check_length_stagnation(output)
        if result is not None:
            return result

        return LoopDetectionResult(
            is_loop=False,
            confidence=0.0,
            loop_type=None,
            suggestion="",
        )

    def _record(self, output: str, output_hash: str) -> None:
        """Record an output for future comparison."""
        self._recent_outputs.append(output)
        self._output_hashes.append(output_hash)
        self._output_lengths.append(len(output))

    def _check_exact_repeat(self, output_hash: str) -> Optional[LoopDetectionResult]:
        """Check for exact repeated outputs via hash matching."""
        hash_count = sum(1 for h in self._output_hashes if h == output_hash)
        if hash_count >= self.hash_threshold:
            confidence = min(1.0, hash_count / (self.hash_threshold + 2))
            return LoopDetectionResult(
                is_loop=True,
                confidence=confidence,
                loop_type=LoopType.EXACT_REPEAT,
                suggestion=(
                    f"Agent has produced the exact same output {hash_count + 1} times. "
                    "Consider changing the prompt, increasing temperature, or terminating."
                ),
            )
        return None

    def _check_similarity(self, output: str) -> Optional[LoopDetectionResult]:
        """Check for high Jaccard similarity with recent outputs.

        Note: This is called after the current output has been recorded,
        so we compare against all entries except the last (which is the
        current output itself).
        """
        # Need at least 2 entries (current + 1 previous)
        if len(self._recent_outputs) < 2:
            return None

        current_tokens = set(output.lower().split())
        if not current_tokens:
            return None

        max_similarity = 0.0
        similar_count = 0
        # Compare against all previous outputs (skip the last which is current)
        previous = list(self._recent_outputs)[:-1]

        for prev_output in previous:
            prev_tokens = set(prev_output.lower().split())
            if not prev_tokens:
                continue

            intersection = current_tokens & prev_tokens
            union = current_tokens | prev_tokens
            similarity = len(intersection) / len(union) if union else 0.0

            max_similarity = max(max_similarity, similarity)
            if similarity >= self.similarity_threshold:
                similar_count += 1

        # Need at least 2 similar outputs in the window to flag
        if similar_count >= 2:
            confidence = min(1.0, max_similarity * (similar_count / len(previous)))
            return LoopDetectionResult(
                is_loop=True,
                confidence=confidence,
                loop_type=LoopType.SEMANTIC_LOOP,
                suggestion=(
                    f"Agent outputs are highly similar (Jaccard={max_similarity:.2f}, "
                    f"{similar_count} similar in window). "
                    "The agent may be rephrasing the same response. "
                    "Consider injecting new context or terminating."
                ),
            )
        return None

    def _check_oscillation(self) -> Optional[LoopDetectionResult]:
        """Check for oscillation between 2-3 states (A-B-A-B pattern)."""
        hashes = list(self._output_hashes)
        if len(hashes) < 4:
            return None

        # Check for period-2 oscillation: A-B-A-B
        recent = hashes[-6:] if len(hashes) >= 6 else hashes
        if len(recent) >= 4:
            period_2_match = all(
                recent[i] == recent[i + 2]
                for i in range(len(recent) - 2)
            )
            if period_2_match and recent[-1] != recent[-2]:
                return LoopDetectionResult(
                    is_loop=True,
                    confidence=0.9,
                    loop_type=LoopType.OSCILLATION,
                    suggestion=(
                        "Agent is oscillating between two states (A-B-A-B pattern). "
                        "This typically indicates conflicting instructions or tool results. "
                        "Consider adding a tie-breaking instruction or terminating."
                    ),
                )

        # Check for period-3 oscillation: A-B-C-A-B-C
        if len(recent) >= 6:
            period_3_match = all(
                recent[i] == recent[i + 3]
                for i in range(len(recent) - 3)
            )
            if period_3_match:
                unique = len(set(recent[:3]))
                if unique >= 2:
                    return LoopDetectionResult(
                        is_loop=True,
                        confidence=0.85,
                        loop_type=LoopType.OSCILLATION,
                        suggestion=(
                            "Agent is oscillating between three states (A-B-C-A-B-C pattern). "
                            "Consider simplifying the task or terminating."
                        ),
                    )

        return None

    def _check_length_stagnation(self, output: str) -> Optional[LoopDetectionResult]:
        """Check if outputs are stuck at identical lengths.

        Note: Called after the current output is already recorded in
        _output_lengths via _record().
        """
        if len(self._output_lengths) < self.length_stagnation_window:
            return None

        recent_lengths = list(self._output_lengths)[-self.length_stagnation_window:]

        # Check if all lengths are identical
        if len(set(recent_lengths)) == 1:
            return LoopDetectionResult(
                is_loop=True,
                confidence=0.6,
                loop_type=LoopType.LENGTH_STAGNATION,
                suggestion=(
                    f"Last {len(recent_lengths)} outputs all have identical length "
                    f"({recent_lengths[0]} chars). Agent may be stuck generating templated responses. "
                    "Consider varying the prompt or terminating."
                ),
            )

        return None

    def reset(self) -> None:
        """Clear all detection state."""
        self._recent_outputs.clear()
        self._output_hashes.clear()
        self._output_lengths.clear()


@dataclass
class InsightRecord:
    """A single iteration's cost and insight data."""

    tokens_used: int
    cost: float
    state_changed: bool
    output_novelty: float
    timestamp: float


@dataclass
class CostPerInsightTracker:
    """Track state mutations vs token cost to detect diminishing returns.

    Monitors the ratio of useful work (state changes, novel outputs) to
    resource consumption (tokens, cost) over a rolling window. When the
    ratio drops below a threshold, recommends termination.

    Example:
        tracker = CostPerInsightTracker()
        for iteration in agent_iterations:
            tracker.record_iteration(
                tokens_used=response.usage.total_tokens,
                cost=response.cost,
                state_changed=has_new_data,
                output_novelty=novelty_score,
            )
            should_stop, reason = tracker.should_terminate()
            if should_stop:
                break
    """

    window_size: int = 10
    min_iterations: int = 3
    cost_threshold: float = 0.05
    novelty_floor: float = 0.1
    _history: list[InsightRecord] = field(default_factory=list)

    def record_iteration(
        self,
        tokens_used: int,
        cost: float,
        state_changed: bool,
        output_novelty: float,
    ) -> None:
        """Record an agent iteration's cost and value.

        Args:
            tokens_used: Number of tokens consumed in this iteration.
            cost: Dollar cost of this iteration.
            state_changed: Whether this iteration produced a meaningful state change.
            output_novelty: Score from 0-1 indicating how novel the output was.
        """
        record = InsightRecord(
            tokens_used=tokens_used,
            cost=cost,
            state_changed=state_changed,
            output_novelty=output_novelty,
            timestamp=time.monotonic(),
        )
        self._history.append(record)

        # Keep only the last N * 2 records to bound memory
        max_records = self.window_size * 2
        if len(self._history) > max_records:
            self._history = self._history[-max_records:]

    def should_terminate(self) -> tuple[bool, str]:
        """Check if the agent should be terminated due to diminishing returns.

        Returns:
            Tuple of (should_terminate, reason).
        """
        if len(self._history) < self.min_iterations:
            return False, ""

        window = self._history[-self.window_size:]

        # Check 1: No state changes in the window
        state_changes = sum(1 for r in window if r.state_changed)
        if state_changes == 0 and len(window) >= self.min_iterations:
            total_cost = sum(r.cost for r in window)
            return True, (
                f"No state changes in last {len(window)} iterations "
                f"(cost: ${total_cost:.4f}). Agent is not making progress."
            )

        # Check 2: Average novelty below floor
        avg_novelty = sum(r.output_novelty for r in window) / len(window)
        if avg_novelty < self.novelty_floor and len(window) >= self.min_iterations:
            total_cost = sum(r.cost for r in window)
            return True, (
                f"Average output novelty ({avg_novelty:.2f}) below threshold "
                f"({self.novelty_floor}) over last {len(window)} iterations "
                f"(cost: ${total_cost:.4f}). Diminishing returns detected."
            )

        # Check 3: Cost increasing but insight decreasing
        if len(window) >= 4:
            mid = len(window) // 2
            first_half = window[:mid]
            second_half = window[mid:]

            first_cost = sum(r.cost for r in first_half)
            second_cost = sum(r.cost for r in second_half)
            first_insight = sum(r.output_novelty for r in first_half) / len(first_half)
            second_insight = sum(r.output_novelty for r in second_half) / len(second_half)

            if (
                second_cost > first_cost * 1.5
                and second_insight < first_insight * 0.5
            ):
                return True, (
                    f"Cost increased by {((second_cost / first_cost) - 1) * 100:.0f}% "
                    f"but insight decreased by {(1 - (second_insight / max(first_insight, 0.001))) * 100:.0f}% "
                    f"in the second half of the window. Escalating cost with diminishing returns."
                )

        return False, ""

    def get_efficiency_score(self) -> float:
        """Get a 0-1 efficiency score for current execution.

        Returns:
            Float from 0 (wasteful) to 1 (efficient).
        """
        if not self._history:
            return 1.0

        window = self._history[-self.window_size:]
        total_cost = sum(r.cost for r in window)
        avg_novelty = sum(r.output_novelty for r in window) / len(window)
        state_change_rate = sum(1 for r in window if r.state_changed) / len(window)

        if total_cost == 0:
            return 1.0

        # Weighted combination of novelty and state change rate
        insight_score = 0.6 * avg_novelty + 0.4 * state_change_rate
        return min(1.0, insight_score)

    def reset(self) -> None:
        """Clear tracking history."""
        self._history.clear()


def compute_output_novelty(current: str, previous_outputs: list[str]) -> float:
    """Compute a novelty score for an output compared to previous outputs.

    Uses Jaccard distance at the word level. Returns 1.0 for completely
    novel output and 0.0 for an exact duplicate.

    Args:
        current: The current output text.
        previous_outputs: List of previous output texts to compare against.

    Returns:
        Float from 0.0 (duplicate) to 1.0 (completely novel).
    """
    if not previous_outputs:
        return 1.0

    current_tokens = set(current.lower().split())
    if not current_tokens:
        return 0.0

    max_similarity = 0.0
    for prev in previous_outputs:
        prev_tokens = set(prev.lower().split())
        if not prev_tokens:
            continue
        intersection = current_tokens & prev_tokens
        union = current_tokens | prev_tokens
        similarity = len(intersection) / len(union) if union else 0.0
        max_similarity = max(max_similarity, similarity)

    return 1.0 - max_similarity
