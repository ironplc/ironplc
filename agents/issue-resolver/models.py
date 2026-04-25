"""Core data types for the issue-resolver orchestrator."""

from __future__ import annotations

from dataclasses import dataclass, field
from datetime import datetime, timezone
from enum import Enum


class Stage(str, Enum):
    TRIAGE = "triage"
    REQUIREMENTS = "requirements"
    DESIGN = "design"
    PLAN = "plan"
    CODE = "code"
    PR_OPEN = "pr_open"
    CLOSED = "closed"

    def next(self) -> "Stage":
        order = [
            Stage.TRIAGE,
            Stage.REQUIREMENTS,
            Stage.DESIGN,
            Stage.PLAN,
            Stage.CODE,
            Stage.PR_OPEN,
            Stage.CLOSED,
        ]
        idx = order.index(self)
        if idx == len(order) - 1:
            return self
        return order[idx + 1]


class BlockReason(str, Enum):
    AGENT_ERROR = "agent_error"
    REVISION_LIMIT = "revision_limit"
    NEEDS_INFO = "needs_info"


@dataclass
class WorkItem:
    issue_number: int
    stage: Stage = Stage.TRIAGE
    revision_counts: dict[Stage, int] = field(default_factory=dict)
    artifact_ids: dict[Stage, int] = field(default_factory=dict)
    blocked_on: BlockReason | None = None


@dataclass
class WorkItemEvent:
    issue_number: int
    event_type: str
    action: str
    label: str | None = None
    actor: str | None = None
    comment_body: str | None = None
    received_at: datetime = field(
        default_factory=lambda: datetime.now(timezone.utc)
    )
