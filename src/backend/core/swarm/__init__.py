"""Apex swarm — multi-agent research pipeline."""

from .bus import MessageBus
from .engine import SwarmEngine
from .events import EventEmitter
from .llm import LLMClient
from .models import (
    AgentMessage,
    AgentRole,
    AgentState,
    SwarmPhase,
    SwarmSession,
    Task,
)
from .roles import ROLE_CONFIGS
from .search import WebSearchClient

__all__ = [
    "AgentMessage",
    "AgentRole",
    "AgentState",
    "EventEmitter",
    "LLMClient",
    "SwarmEngine",
    "MessageBus",
    "ROLE_CONFIGS",
    "SwarmPhase",
    "SwarmSession",
    "Task",
]
