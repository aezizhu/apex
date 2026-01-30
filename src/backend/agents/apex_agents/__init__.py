"""
Apex Agents - Python Agent Runtime

The agent execution layer for Project Apex.

This module provides:
- Agent: AI agent that executes tasks using LLMs and tools
- AgentExecutor: Manages agent pool and coordinates task execution
- Worker: Process that runs agents in a loop with heartbeat
- Configuration and tracing utilities
"""

__version__ = "0.1.0"

from apex_agents.agent import Agent, AgentConfig, AgentStatus, TaskInput, TaskOutput
from apex_agents.config import Settings, get_settings
from apex_agents.executor import AgentExecutor, QueuedTask, TaskResult
from apex_agents.llm import LLMClient, LLMProvider
from apex_agents.loop_detector import (
    CostPerInsightTracker,
    LoopDetectionResult,
    LoopDetector,
    LoopType,
    compute_output_novelty,
)
from apex_agents.routing import ModelRouter, RoutingConfig, RoutingResult
from apex_agents.tools import Tool, ToolError, ToolRegistry, ToolResult, create_default_registry
from apex_agents.tracing import (
    TaskSpanContext,
    get_tracer,
    init_tracing,
    shutdown_tracing,
    traced,
    traced_async,
)
from apex_agents.bidding import AgentBid, AwardDecision, BiddingAgent, TaskAnnouncement
from apex_agents.worker import Worker, WorkerPool, WorkerState

__all__ = [
    # Agent
    "Agent",
    "AgentConfig",
    "AgentStatus",
    "TaskInput",
    "TaskOutput",
    # Bidding (CNP)
    "BiddingAgent",
    "TaskAnnouncement",
    "AgentBid",
    "AwardDecision",
    # Executor
    "AgentExecutor",
    "QueuedTask",
    "TaskResult",
    # Worker
    "Worker",
    "WorkerPool",
    "WorkerState",
    # Tools
    "Tool",
    "ToolError",
    "ToolRegistry",
    "ToolResult",
    "create_default_registry",
    # LLM
    "LLMClient",
    "LLMProvider",
    # Loop Detection
    "LoopDetector",
    "LoopDetectionResult",
    "LoopType",
    "CostPerInsightTracker",
    "compute_output_novelty",
    # Routing
    "ModelRouter",
    "RoutingConfig",
    "RoutingResult",
    # Config
    "Settings",
    "get_settings",
    # Tracing
    "init_tracing",
    "shutdown_tracing",
    "get_tracer",
    "traced",
    "traced_async",
    "TaskSpanContext",
]
