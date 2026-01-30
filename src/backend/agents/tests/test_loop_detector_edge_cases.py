"""Edge case tests for loop detection and cost-per-insight tracking."""

import pytest

from apex_agents.loop_detector import (
    CostPerInsightTracker,
    LoopDetector,
    LoopType,
    compute_output_novelty,
)


class TestLoopDetectorCustomWindowSize:
    """Tests for LoopDetector with custom window sizes."""

    def test_custom_window_size(self):
        """Test that custom window size limits hash history."""
        detector = LoopDetector(window_size=3, hash_threshold=2)
        for i in range(10):
            detector.check(f"output {i}")
        # Internal hash history should be bounded by window_size * 2
        assert len(detector._output_hashes) <= 6

    def test_hash_threshold_1_triggers_immediately(self):
        """Test hash_threshold=1 triggers on first repeat."""
        detector = LoopDetector(hash_threshold=1, similarity_threshold=1.0)
        detector.check("repeat me")
        result = detector.check("repeat me")
        assert result.is_loop
        assert result.loop_type == LoopType.EXACT_REPEAT

    def test_high_similarity_threshold_avoids_false_positives(self):
        """Test that similarity_threshold=1.0 never fires semantic loop."""
        detector = LoopDetector(hash_threshold=100, similarity_threshold=1.0)
        for i in range(20):
            result = detector.check(f"The quick brown fox jumps {i}")
            if result.is_loop and result.loop_type == LoopType.SEMANTIC_LOOP:
                pytest.fail("Should not fire semantic loop with threshold=1.0")


class TestLoopDetectorOscillationPeriod3:
    """Tests for oscillation with period > 2."""

    def test_period_3_oscillation(self):
        """Test detection of A-B-C-A-B-C oscillation pattern."""
        detector = LoopDetector(hash_threshold=100, similarity_threshold=1.0)
        a = "alpha bravo charlie"
        b = "delta echo foxtrot"
        c = "golf hotel india"

        for _ in range(4):
            detector.check(a)
            detector.check(b)
            detector.check(c)

        result = detector.check(a)
        # May or may not detect -- this tests the code path runs without error
        if result.is_loop:
            assert result.loop_type in (LoopType.OSCILLATION, LoopType.EXACT_REPEAT)


class TestCostPerInsightTrackerEdgeCases:
    """Edge case tests for CostPerInsightTracker."""

    def test_zero_cost_efficiency(self):
        """Test efficiency calculation with zero cost."""
        tracker = CostPerInsightTracker(min_iterations=1, window_size=3)
        for _ in range(3):
            tracker.record_iteration(
                tokens_used=0, cost=0.0, state_changed=True, output_novelty=0.5
            )
        score = tracker.get_efficiency_score()
        assert score >= 0.0

    def test_single_iteration(self):
        """Test behavior with only one recorded iteration."""
        tracker = CostPerInsightTracker(min_iterations=5)
        tracker.record_iteration(
            tokens_used=100, cost=0.01, state_changed=True, output_novelty=0.9
        )
        should_stop, _ = tracker.should_terminate()
        assert not should_stop

    def test_window_sizing(self):
        """Test that window_size parameter controls history."""
        tracker = CostPerInsightTracker(window_size=5)
        for i in range(100):
            tracker.record_iteration(
                tokens_used=100, cost=0.01, state_changed=True, output_novelty=0.5
            )
        assert len(tracker._history) <= 10  # window * 2

    def test_cost_increasing_insight_not_decreasing_enough(self):
        """Test case where cost increases but insight doesn't decrease enough to trigger."""
        tracker = CostPerInsightTracker(min_iterations=2, window_size=4)
        for _ in range(4):
            tracker.record_iteration(
                tokens_used=100, cost=0.01, state_changed=True, output_novelty=0.8
            )
        for _ in range(4):
            tracker.record_iteration(
                tokens_used=200, cost=0.02, state_changed=True, output_novelty=0.7
            )
        # Novelty only slightly decreased, may or may not trigger
        should_stop, reason = tracker.should_terminate()
        # Just verify it doesn't crash
        assert isinstance(should_stop, bool)


class TestComputeOutputNoveltyEdgeCases:
    """Edge case tests for compute_output_novelty."""

    def test_empty_previous_outputs(self):
        """Test novelty with no previous outputs is 1.0."""
        score = compute_output_novelty("brand new content", [])
        assert score == 1.0

    def test_all_empty_previous(self):
        """Test novelty when all previous outputs are empty strings."""
        score = compute_output_novelty("some content", ["", "", ""])
        assert score > 0.0

    def test_current_empty(self):
        """Test novelty when current output is empty."""
        score = compute_output_novelty("", ["previous output"])
        assert score == 0.0

    def test_identical_to_multiple(self):
        """Test novelty when current matches multiple previous outputs."""
        score = compute_output_novelty(
            "same text", ["same text", "same text", "same text"]
        )
        assert score == 0.0

    def test_single_word_overlap(self):
        """Test novelty with minimal word overlap."""
        score = compute_output_novelty(
            "hello world foo bar baz",
            ["hello universe qux quux corge"],
        )
        assert 0.0 < score < 1.0
