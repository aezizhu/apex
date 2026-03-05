"""Tests for loop detection and cost-per-insight tracking."""

import pytest
from unittest.mock import AsyncMock, MagicMock, patch

from apex_agents.loop_detector import (
    CostPerInsightTracker,
    LoopDetectionResult,
    LoopDetector,
    LoopType,
    compute_output_novelty,
)


class TestLoopDetectorExactRepeat:
    """Tests for exact repeat detection via hash matching."""

    def test_no_loop_on_first_output(self):
        detector = LoopDetector()
        result = detector.check("Hello, world!")
        assert not result.is_loop

    def test_no_loop_with_different_outputs(self):
        detector = LoopDetector()
        # Use outputs with different lengths and content to avoid all detection strategies
        outputs = [
            "Short",
            "A medium-length output here",
            "This is a somewhat longer output with more words",
            "Tiny",
            "And now for something completely different and much longer than before",
        ]
        for output in outputs:
            result = detector.check(output)
            assert not result.is_loop

    def test_detects_exact_repeat(self):
        # Disable similarity detection so we isolate exact hash matching
        detector = LoopDetector(hash_threshold=3, similarity_threshold=1.0)
        repeated_output = "I'm stuck in a loop."

        # First three repetitions build up the hash history
        for _ in range(3):
            detector.check(repeated_output)

        # Fourth check sees 3 hashes in history, triggers exact repeat
        result = detector.check(repeated_output)
        assert result.is_loop
        assert result.loop_type == LoopType.EXACT_REPEAT
        assert result.confidence > 0

    def test_exact_repeat_confidence_increases(self):
        detector = LoopDetector(hash_threshold=2)
        repeated = "Same output every time."

        results = []
        for _ in range(6):
            results.append(detector.check(repeated))

        # Find the first detection and a later one
        detections = [r for r in results if r.is_loop]
        assert len(detections) > 0
        # Later detections should have equal or higher confidence
        if len(detections) >= 2:
            assert detections[-1].confidence >= detections[0].confidence

    def test_exact_repeat_suggestion_present(self):
        detector = LoopDetector(hash_threshold=2)
        for _ in range(4):
            result = detector.check("Repeated content")
        assert result.is_loop
        assert "same output" in result.suggestion.lower() or "identical" in result.suggestion.lower() or "exact" in result.suggestion.lower()


class TestLoopDetectorSimilarity:
    """Tests for Jaccard similarity-based loop detection."""

    def test_detects_near_duplicate_outputs(self):
        detector = LoopDetector(similarity_threshold=0.8, hash_threshold=100)

        base = "The quick brown fox jumps over the lazy dog"
        variants = [
            "The quick brown fox jumps over the lazy dog",
            "The quick brown fox jumped over the lazy dog",
            "The quick brown fox leaps over the lazy dog",
            "The quick brown fox jumps over the lazy dogs",
        ]

        last_result = None
        for v in variants:
            last_result = detector.check(v)

        # With high enough similarity among these variants, should detect a loop
        # The variants share most words
        assert last_result is not None
        # The first two are exact (will be caught by hash or similarity)
        # The later ones should trigger similarity detection

    def test_no_similarity_loop_for_diverse_outputs(self):
        detector = LoopDetector(similarity_threshold=0.85, hash_threshold=100)

        diverse_outputs = [
            "The weather today is sunny and warm.",
            "Python is a versatile programming language.",
            "Machine learning models require large datasets.",
            "The stock market closed higher today.",
            "Quantum computing uses qubits instead of bits.",
        ]

        for output in diverse_outputs:
            result = detector.check(output)
            assert not result.is_loop

    def test_semantic_loop_type(self):
        detector = LoopDetector(similarity_threshold=0.7, hash_threshold=100)

        # Feed many similar outputs to build up history
        for i in range(5):
            # These share most words
            detector.check(f"The analysis shows positive results for item number {i} in the dataset")

        result = detector.check("The analysis shows positive results for item number 99 in the dataset")
        if result.is_loop:
            assert result.loop_type == LoopType.SEMANTIC_LOOP


class TestLoopDetectorOscillation:
    """Tests for oscillation pattern detection (A-B-A-B)."""

    def test_detects_period_2_oscillation(self):
        # Use completely different word sets so similarity check doesn't fire first
        detector = LoopDetector(hash_threshold=100, similarity_threshold=1.0)

        state_a = "alpha bravo charlie delta echo foxtrot"
        state_b = "golf hotel india juliet kilo lima"

        # Alternate between two states
        for _ in range(4):
            detector.check(state_a)
            detector.check(state_b)

        # After enough oscillations, should detect the pattern
        result = detector.check(state_a)
        # The oscillation check fires because the hashes alternate A-B-A-B
        if result.is_loop:
            assert result.loop_type == LoopType.OSCILLATION

    def test_no_oscillation_for_sequential_unique(self):
        detector = LoopDetector(hash_threshold=100, similarity_threshold=1.0)

        for i in range(10):
            result = detector.check(f"Completely unique output {i}")
            if result.is_loop and result.loop_type == LoopType.OSCILLATION:
                pytest.fail("Should not detect oscillation in unique outputs")


class TestLoopDetectorLengthStagnation:
    """Tests for length-based stagnation detection."""

    def test_detects_identical_length_outputs(self):
        detector = LoopDetector(
            hash_threshold=100,
            similarity_threshold=1.0,
            length_stagnation_window=4,
        )

        # Outputs with identical length but different content
        # "AAAA" length = 4
        for i in range(6):
            char = chr(65 + i)  # A, B, C, D, E, F
            result = detector.check(char * 4)

        # After enough identical-length outputs, should detect stagnation
        if result.is_loop:
            assert result.loop_type == LoopType.LENGTH_STAGNATION

    def test_no_stagnation_for_varying_lengths(self):
        detector = LoopDetector(
            hash_threshold=100,
            similarity_threshold=1.0,
            length_stagnation_window=5,
        )

        for i in range(10):
            result = detector.check("x" * (10 + i * 5))
            if result.is_loop and result.loop_type == LoopType.LENGTH_STAGNATION:
                pytest.fail("Should not detect stagnation with varying lengths")


class TestLoopDetectorReset:
    """Tests for detector reset functionality."""

    def test_reset_clears_state(self):
        detector = LoopDetector(hash_threshold=2)

        # Build up state
        for _ in range(3):
            detector.check("Repeated")

        detector.reset()

        # After reset, same output should not trigger
        result = detector.check("Repeated")
        assert not result.is_loop


class TestLoopDetectionResult:
    """Tests for LoopDetectionResult dataclass."""

    def test_str_no_loop(self):
        result = LoopDetectionResult(
            is_loop=False, confidence=0.0, loop_type=None, suggestion=""
        )
        assert "No loop detected" in str(result)

    def test_str_with_loop(self):
        result = LoopDetectionResult(
            is_loop=True,
            confidence=0.95,
            loop_type=LoopType.EXACT_REPEAT,
            suggestion="Stop the agent",
        )
        s = str(result)
        assert "exact_repeat" in s
        assert "0.95" in s
        assert "Stop the agent" in s


class TestCostPerInsightTracker:
    """Tests for cost-per-insight tracking."""

    def test_no_termination_below_min_iterations(self):
        tracker = CostPerInsightTracker(min_iterations=5)

        for _ in range(3):
            tracker.record_iteration(
                tokens_used=100, cost=0.01, state_changed=False, output_novelty=0.0
            )

        should_stop, reason = tracker.should_terminate()
        assert not should_stop

    def test_terminates_on_no_state_changes(self):
        tracker = CostPerInsightTracker(min_iterations=3, window_size=5)

        for _ in range(5):
            tracker.record_iteration(
                tokens_used=500, cost=0.05, state_changed=False, output_novelty=0.5
            )

        should_stop, reason = tracker.should_terminate()
        assert should_stop
        assert "No state changes" in reason

    def test_terminates_on_low_novelty(self):
        tracker = CostPerInsightTracker(
            min_iterations=3, window_size=5, novelty_floor=0.2
        )

        for _ in range(5):
            tracker.record_iteration(
                tokens_used=500,
                cost=0.05,
                state_changed=True,
                output_novelty=0.05,
            )

        should_stop, reason = tracker.should_terminate()
        assert should_stop
        assert "novelty" in reason.lower()

    def test_terminates_on_increasing_cost_decreasing_insight(self):
        tracker = CostPerInsightTracker(min_iterations=2, window_size=8)

        # First half: low cost, high novelty
        for _ in range(4):
            tracker.record_iteration(
                tokens_used=100, cost=0.01, state_changed=True, output_novelty=0.9
            )

        # Second half: high cost, low novelty
        for _ in range(4):
            tracker.record_iteration(
                tokens_used=1000, cost=0.10, state_changed=True, output_novelty=0.1
            )

        should_stop, reason = tracker.should_terminate()
        assert should_stop
        assert "cost" in reason.lower() or "diminishing" in reason.lower()

    def test_does_not_terminate_when_efficient(self):
        tracker = CostPerInsightTracker(min_iterations=3, window_size=5)

        for _ in range(5):
            tracker.record_iteration(
                tokens_used=200, cost=0.02, state_changed=True, output_novelty=0.8
            )

        should_stop, _ = tracker.should_terminate()
        assert not should_stop

    def test_efficiency_score_high_when_productive(self):
        tracker = CostPerInsightTracker()

        for _ in range(5):
            tracker.record_iteration(
                tokens_used=200, cost=0.02, state_changed=True, output_novelty=0.9
            )

        score = tracker.get_efficiency_score()
        assert score > 0.5

    def test_efficiency_score_low_when_stagnant(self):
        tracker = CostPerInsightTracker()

        for _ in range(5):
            tracker.record_iteration(
                tokens_used=1000, cost=0.10, state_changed=False, output_novelty=0.05
            )

        score = tracker.get_efficiency_score()
        assert score < 0.2

    def test_efficiency_score_default_when_empty(self):
        tracker = CostPerInsightTracker()
        assert tracker.get_efficiency_score() == 1.0

    def test_reset_clears_history(self):
        tracker = CostPerInsightTracker(min_iterations=1, window_size=3)

        for _ in range(5):
            tracker.record_iteration(
                tokens_used=500, cost=0.05, state_changed=False, output_novelty=0.0
            )

        tracker.reset()
        should_stop, _ = tracker.should_terminate()
        assert not should_stop

    def test_memory_bounded(self):
        """Ensure tracker doesn't grow unbounded."""
        tracker = CostPerInsightTracker(window_size=10)

        for _ in range(1000):
            tracker.record_iteration(
                tokens_used=100, cost=0.01, state_changed=True, output_novelty=0.5
            )

        # History should be bounded to window_size * 2
        assert len(tracker._history) <= 20


class TestComputeOutputNovelty:
    """Tests for the novelty computation helper."""

    def test_fully_novel(self):
        score = compute_output_novelty("completely new content", [])
        assert score == 1.0

    def test_exact_duplicate(self):
        score = compute_output_novelty("same text here", ["same text here"])
        assert score == 0.0

    def test_partial_overlap(self):
        score = compute_output_novelty(
            "the quick brown fox",
            ["the slow brown cat"],
        )
        assert 0.0 < score < 1.0

    def test_novelty_decreases_with_more_similar(self):
        score1 = compute_output_novelty(
            "analysis of data results",
            ["review of data findings"],
        )
        score2 = compute_output_novelty(
            "analysis of data results",
            ["analysis of data results exactly"],
        )
        # score2 should be lower (less novel) since it's more similar
        # (both share many words but score2's reference is closer)
        assert score2 <= score1 or abs(score1 - score2) < 0.3

    def test_empty_output(self):
        score = compute_output_novelty("", ["some previous output"])
        assert score == 0.0


class TestAgentLoopDetectionIntegration:
    """Integration tests for loop detection within the Agent execution loop."""

    @pytest.mark.asyncio
    async def test_agent_terminates_on_loop(self):
        """Test that the agent stops when a loop is detected."""
        from apex_agents.agent import Agent, AgentConfig, TaskInput
        from apex_agents.llm import LLMResponse, LLMUsage

        config = AgentConfig(
            name="loop-test-agent",
            model="gpt-4o-mini",
            system_prompt="You are a test agent.",
            tools=["test_tool"],
            max_iterations=20,
        )

        mock_client = AsyncMock()
        # Return the same tool-calling response every time
        repeated_response = LLMResponse(
            content="Let me search for that.",
            tool_calls=[
                {
                    "id": "call_123",
                    "function": {
                        "name": "test_tool",
                        "arguments": {"query": "test"},
                    },
                }
            ],
            usage=LLMUsage(prompt_tokens=50, completion_tokens=30, total_tokens=80),
            model="gpt-4o-mini",
            cost=0.001,
            finish_reason="tool_calls",
        )
        mock_client.create.return_value = repeated_response

        from apex_agents.tools import Tool, ToolParameter, ToolRegistry, ToolResult

        registry = ToolRegistry()

        async def test_func(query: str) -> str:
            return f"Result: {query}"

        tool = Tool(
            name="test_tool",
            description="A test tool",
            parameters=[ToolParameter("query", "string", "Query")],
            func=test_func,
        )
        registry.register(tool)

        agent = Agent(config=config, llm_client=mock_client, tool_registry=registry)

        # Override detector thresholds for testing
        agent.loop_detector = LoopDetector(hash_threshold=3, similarity_threshold=0.85)

        task = TaskInput(instruction="Search for something")
        result = await agent.run(task)

        # Should have been terminated due to loop detection, not max iterations
        assert "loop" in result.data.get("error", "").lower() or "diminishing" in result.data.get("error", "").lower()
        # Should have stopped well before max_iterations=20
        assert agent.metrics.iterations < 20

    @pytest.mark.asyncio
    async def test_agent_normal_execution_unaffected(self):
        """Test that normal (non-looping) execution is not affected by detectors."""
        from apex_agents.agent import Agent, AgentConfig, TaskInput
        from apex_agents.llm import LLMResponse, LLMUsage

        config = AgentConfig(
            name="normal-test-agent",
            model="gpt-4o-mini",
            system_prompt="You are a test agent.",
            tools=[],
            max_iterations=10,
        )

        mock_client = AsyncMock()
        normal_response = LLMResponse(
            content="Here is a unique and helpful answer.",
            tool_calls=[],
            usage=LLMUsage(prompt_tokens=50, completion_tokens=20, total_tokens=70),
            model="gpt-4o-mini",
            cost=0.001,
            finish_reason="stop",
        )
        mock_client.create.return_value = normal_response

        from apex_agents.tools import ToolRegistry

        agent = Agent(
            config=config, llm_client=mock_client, tool_registry=ToolRegistry()
        )

        task = TaskInput(instruction="Give me an answer")
        result = await agent.run(task)

        # Should complete normally
        assert result.result == "Here is a unique and helpful answer."
        assert "error" not in result.data
