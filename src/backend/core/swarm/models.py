"""Data models for the Apex swarm system."""

from __future__ import annotations

import uuid
from dataclasses import dataclass, field
from datetime import datetime, timezone
from enum import Enum


class SwarmPhase(str, Enum):
    PLANNING = "planning"
    RESEARCHING = "researching"
    ANALYZING = "analyzing"
    FACT_CHECKING = "fact_checking"
    WRITING = "writing"
    # Report pipeline sub-phases (within WRITING)
    TEMPLATE_SELECTION = "template_selection"
    LAYOUT_DESIGN = "layout_design"
    BUDGETING = "budgeting"
    CHAPTER_WRITING = "chapter_writing"
    STITCHING = "stitching"
    RENDERING = "rendering"
    COMPLETE = "complete"


class AgentRole(str, Enum):
    COORDINATOR = "coordinator"
    RESEARCHER = "researcher"
    ANALYST = "analyst"
    FACT_CHECKER = "fact_checker"
    WRITER = "writer"
    TEMPLATE_SELECTOR = "template_selector"
    LAYOUT_DESIGNER = "layout_designer"
    BUDGET_PLANNER = "budget_planner"
    CHAPTER_WRITER = "chapter_writer"


@dataclass
class AgentState:
    id: str = field(default_factory=lambda: uuid.uuid4().hex[:12])
    role: AgentRole = AgentRole.COORDINATOR
    name: str = ""
    status: str = "idle"  # idle | working | done | error
    created_at: str = field(default_factory=lambda: datetime.now(timezone.utc).isoformat())


@dataclass
class Task:
    id: str = field(default_factory=lambda: uuid.uuid4().hex[:12])
    description: str = ""
    assigned_to: str = ""
    status: str = "pending"  # pending | running | done | error
    result: str = ""


@dataclass
class AgentMessage:
    id: str = field(default_factory=lambda: uuid.uuid4().hex[:12])
    from_agent: str = ""
    to_agent: str = ""
    content: str = ""
    timestamp: str = field(default_factory=lambda: datetime.now(timezone.utc).isoformat())
    msg_type: str = "info"  # info | task | result | error


@dataclass
class SwarmSession:
    id: str = field(default_factory=lambda: uuid.uuid4().hex[:12])
    query: str = ""
    phase: SwarmPhase = SwarmPhase.PLANNING
    agents: dict[str, AgentState] = field(default_factory=dict)
    tasks: list[Task] = field(default_factory=list)
    messages: list[AgentMessage] = field(default_factory=list)
    created_at: str = field(default_factory=lambda: datetime.now(timezone.utc).isoformat())
    final_output: str = ""
    cancelled: bool = False
