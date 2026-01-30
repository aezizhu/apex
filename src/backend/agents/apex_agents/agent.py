"""
Agent definition and execution logic.
"""

from __future__ import annotations

import asyncio
import uuid
from dataclasses import dataclass, field
from datetime import UTC, datetime
from enum import Enum
from typing import Any

import structlog
from opentelemetry import trace
from pydantic import BaseModel

from apex_agents.llm import LLMClient
from apex_agents.loop_detector import (
    CostPerInsightTracker,
    LoopDetector,
    compute_output_novelty,
)
from apex_agents.routing import ModelRouter
from apex_agents.bidding import BiddingAgent
from apex_agents.tools import Tool, ToolRegistry

logger = structlog.get_logger()
tracer = trace.get_tracer(__name__)


class AgentStatus(str, Enum):
    """Agent status."""
    IDLE = "idle"
    BUSY = "busy"
    ERROR = "error"
    PAUSED = "paused"


class TaskInput(BaseModel):
    """Input for a task."""
    instruction: str
    context: dict[str, Any] = {}
    parameters: dict[str, Any] = {}


class TaskOutput(BaseModel):
    """Output from a task."""
    result: str
    data: dict[str, Any] = {}
    reasoning: str | None = None


class AgentConfig(BaseModel):
    """Configuration for an agent."""
    name: str
    model: str
    system_prompt: str = ""
    tools: list[str] = []
    max_iterations: int = 10
    temperature: float = 0.7


@dataclass
class AgentMetrics:
    """Runtime metrics for an agent."""
    tokens_used: int = 0
    cost_dollars: float = 0.0
    iterations: int = 0
    tool_calls: int = 0
    start_time: datetime | None = None
    end_time: datetime | None = None

    @property
    def duration_ms(self) -> int:
        """Get duration in milliseconds."""
        if self.start_time and self.end_time:
            return int((self.end_time - self.start_time).total_seconds() * 1000)
        return 0


class Agent:
    """
    An AI agent that can execute tasks using LLMs and tools.

    Example:
        agent = Agent(
            config=AgentConfig(
                name="researcher",
                model="gpt-4o",
                system_prompt="You are a research assistant.",
                tools=["web_search", "read_file"]
            ),
            llm_client=llm_client,
            tool_registry=tool_registry
        )

        result = await agent.run(TaskInput(instruction="Research AI trends"))
    """

    def __init__(
        self,
        config: AgentConfig,
        llm_client: LLMClient,
        tool_registry: ToolRegistry,
        model_router: ModelRouter | None = None,
        bidding_agent: BiddingAgent | None = None,
    ):
        self.id = uuid.uuid4()
        self.config = config
        self.llm_client = llm_client
        self.tool_registry = tool_registry
        self.model_router = model_router
        self.bidding_agent = bidding_agent
        self.status = AgentStatus.IDLE
        self.metrics = AgentMetrics()
        self.loop_detector = LoopDetector()
        self.cost_tracker = CostPerInsightTracker()
        self._previous_outputs: list[str] = []
        self._logger = logger.bind(agent_id=str(self.id), agent_name=config.name)

    @property
    def available_tools(self) -> list[Tool]:
        """Get tools available to this agent."""
        tools = []
        for name in self.config.tools:
            t = self.tool_registry.get(name)
            if t is not None:
                tools.append(t)
        return tools

    async def run(self, task: TaskInput, trace_id: str | None = None) -> TaskOutput:
        """
        Execute a task.

        Args:
            task: The task input
            trace_id: Optional trace ID for distributed tracing

        Returns:
            TaskOutput with the result
        """
        with tracer.start_as_current_span(
            f"agent_{self.config.name}_run",
            attributes={
                "agent.id": str(self.id),
                "agent.name": self.config.name,
                "agent.model": self.config.model,
            }
        ) as span:
            self.status = AgentStatus.BUSY
            self.metrics = AgentMetrics(start_time=datetime.now(UTC))

            self._logger.info(
                "Starting task execution",
                instruction=task.instruction[:100],
            )

            try:
                result = await self._execute_loop(task, span)
                self.status = AgentStatus.IDLE
                return result

            except Exception as e:
                self.status = AgentStatus.ERROR
                self._logger.error("Task execution failed", error=str(e))
                span.record_exception(e)
                raise

            finally:
                self.metrics.end_time = datetime.now(UTC)
                self._logger.info(
                    "Task execution completed",
                    tokens_used=self.metrics.tokens_used,
                    cost=self.metrics.cost_dollars,
                    duration_ms=self.metrics.duration_ms,
                    iterations=self.metrics.iterations,
                )

    async def _execute_loop(self, task: TaskInput, span: trace.Span) -> TaskOutput:
        """Execute the agent loop with tool use and loop detection."""
        messages = self._build_initial_messages(task)
        tools_schema = self._build_tools_schema()

        # Reset detectors for this execution
        self.loop_detector.reset()
        self.cost_tracker.reset()
        self._previous_outputs.clear()

        for iteration in range(self.config.max_iterations):
            self.metrics.iterations = iteration + 1

            with tracer.start_as_current_span(f"iteration_{iteration}"):
                # Call LLM (via router if available)
                if self.model_router is not None:
                    routing_result = await self.model_router.route(
                        messages=messages,
                        tools=tools_schema if tools_schema else None,
                        temperature=self.config.temperature,
                    )
                    response = routing_result.response
                    iteration_cost = routing_result.total_cost
                else:
                    response = await self.llm_client.create(
                        model=self.config.model,
                        messages=messages,
                        tools=tools_schema if tools_schema else None,
                        temperature=self.config.temperature,
                    )
                    iteration_cost = response.cost

                self.metrics.cost_dollars += iteration_cost

                # Update metrics
                self.metrics.tokens_used += response.usage.total_tokens

                # --- Loop detection ---
                output_text = response.content or ""
                loop_result = self.loop_detector.check(output_text)
                if loop_result.is_loop:
                    self._logger.warning(
                        "Loop detected",
                        loop_type=loop_result.loop_type.value if loop_result.loop_type else None,
                        confidence=loop_result.confidence,
                        iteration=iteration,
                    )
                    span.set_attribute("agent.loop_detected", True)
                    span.set_attribute("agent.loop_type", str(loop_result.loop_type))
                    return TaskOutput(
                        result=f"Agent terminated: {loop_result.suggestion}",
                        data={
                            "error": "loop_detected",
                            "loop_type": loop_result.loop_type.value if loop_result.loop_type else None,
                            "confidence": loop_result.confidence,
                            "iteration": iteration,
                        },
                    )

                # --- Cost-per-insight tracking ---
                novelty = compute_output_novelty(output_text, self._previous_outputs)
                state_changed = bool(response.tool_calls)
                self.cost_tracker.record_iteration(
                    tokens_used=response.usage.total_tokens,
                    cost=iteration_cost,
                    state_changed=state_changed,
                    output_novelty=novelty,
                )
                self._previous_outputs.append(output_text)

                should_terminate, reason = self.cost_tracker.should_terminate()
                if should_terminate:
                    self._logger.warning(
                        "Diminishing returns detected",
                        reason=reason,
                        iteration=iteration,
                    )
                    span.set_attribute("agent.diminishing_returns", True)
                    return TaskOutput(
                        result=f"Agent terminated due to diminishing returns: {reason}",
                        data={
                            "error": "diminishing_returns",
                            "reason": reason,
                            "iteration": iteration,
                            "efficiency_score": self.cost_tracker.get_efficiency_score(),
                        },
                    )

                # Check if done (no tool calls)
                if not response.tool_calls:
                    return TaskOutput(
                        result=response.content,
                        data={},
                        reasoning=None,
                    )

                # Execute tool calls
                messages.append({
                    "role": "assistant",
                    "content": response.content,
                    "tool_calls": response.tool_calls,
                })

                tool_results = await self._execute_tools(response.tool_calls)
                messages.extend(tool_results)

        # Max iterations reached
        self._logger.warning("Max iterations reached", max=self.config.max_iterations)
        return TaskOutput(
            result="Max iterations reached without completing the task.",
            data={"error": "max_iterations_exceeded"},
        )

    def _build_initial_messages(self, task: TaskInput) -> list[dict[str, Any]]:
        """Build the initial message list."""
        messages = []

        if self.config.system_prompt:
            messages.append({
                "role": "system",
                "content": self.config.system_prompt,
            })

        # Build user message with context
        user_content = task.instruction
        if task.context:
            context_str = "\n".join(f"- {k}: {v}" for k, v in task.context.items())
            user_content = f"Context:\n{context_str}\n\nTask: {task.instruction}"

        messages.append({
            "role": "user",
            "content": user_content,
        })

        return messages

    def _build_tools_schema(self) -> list[dict[str, Any]]:
        """Build the tools schema for the LLM."""
        return [tool.to_schema() for tool in self.available_tools]

    async def _execute_tools(
        self, tool_calls: list[dict[str, Any]]
    ) -> list[dict[str, Any]]:
        """Execute tool calls and return results."""
        results = []

        for call in tool_calls:
            tool_name = call["function"]["name"]
            tool_args = call["function"]["arguments"]
            call_id = call["id"]

            self.metrics.tool_calls += 1

            with tracer.start_as_current_span(
                f"tool_{tool_name}",
                attributes={"tool.name": tool_name}
            ):
                self._logger.debug("Executing tool", tool=tool_name)

                try:
                    tool = self.tool_registry.get(tool_name)
                    if tool is None:
                        raise KeyError(f"Tool not found: {tool_name}")
                    tool_result = await tool.execute(**tool_args)

                    results.append({
                        "role": "tool",
                        "tool_call_id": call_id,
                        "content": tool_result.output if tool_result.success else f"Error: {tool_result.error}",
                    })

                except Exception as e:
                    self._logger.error("Tool execution failed", tool=tool_name, error=str(e))
                    results.append({
                        "role": "tool",
                        "tool_call_id": call_id,
                        "content": f"Error: {str(e)}",
                    })

        return results
