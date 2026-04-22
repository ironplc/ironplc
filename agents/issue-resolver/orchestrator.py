"""Event router and state machine for GitHub-issue work items.

The orchestrator is the only place that knows how an incoming webhook
event should affect labels, comments, and the ``WorkItem`` state
machine. Individual stage agents don't touch GitHub directly.

Rules baked into the routing table:

- A ``labeled`` event with ``status/triage`` dispatches the Requirements
  agent. All other stage-advancing label events are stubbed for now
  and simply recorded in the ledger.
- An ``IncompleteIssueError`` from any agent triggers the
  ``status/needs-info`` path: the orchestrator posts a comment listing
  what the agent flagged missing and hands the issue back to a human.
- Any ``AgentError`` raises the ``flag/agent-error`` label and records
  the failure in both the ledger and a human-readable comment.
- A per-stage revision cap (3) prevents infinite loops when a reviewer
  keeps asking for changes; the 4th attempt sets
  ``flag/revision-limit`` instead of re-running the agent.
- ``WorkItem`` state lives in-process for now. The ledger is the
  source of truth; rebuilding ``WorkItem`` from the ledger after a
  restart is a future task.
"""

from __future__ import annotations

import traceback
from dataclasses import dataclass
from typing import Any

from agents.requirements import (
    AgentError,
    IncompleteIssueError,
    RequirementsAgent,
)
from context import build_requirements_context
from github_client import GitHubClient, GitHubAPIError
from ledger import Ledger
from models import BlockReason, Stage, WorkItem, WorkItemEvent
from schemas import RequirementsDocument


_MAX_REVISIONS_PER_STAGE = 3

_LABEL_STATUS_TRIAGE = "status/triage"
_LABEL_STATUS_REQUIREMENTS = "status/requirements"
_LABEL_STATUS_NEEDS_INFO = "status/needs-info"
_LABEL_REVIEW_APPROVED = "review/approved"
_LABEL_FLAG_AGENT_ERROR = "flag/agent-error"
_LABEL_FLAG_REVISION_LIMIT = "flag/revision-limit"


@dataclass
class Orchestrator:
    github: GitHubClient
    ledger: Ledger
    requirements_agent: RequirementsAgent
    bot_login: str | None = None
    _work_items: dict[int, WorkItem] | None = None

    def __post_init__(self) -> None:
        if self._work_items is None:
            self._work_items = {}

    def _work_item(self, issue_number: int) -> WorkItem:
        if issue_number not in self._work_items:
            self._work_items[issue_number] = WorkItem(issue_number=issue_number)
        return self._work_items[issue_number]

    async def handle_event(self, event: WorkItemEvent) -> None:
        self.ledger.log(
            "WEBHOOK_RECEIVED",
            issue_number=event.issue_number,
            source_event=event.event_type,
            action=event.action,
            label=event.label,
            actor=event.actor,
        )

        if event.event_type == "issues" and event.action == "labeled":
            await self._on_labeled(event)
            return

        if event.event_type == "issue_comment" and event.action == "created":
            if self.bot_login and event.actor == self.bot_login:
                return
            self.ledger.log(
                "EVENT_IGNORED",
                issue_number=event.issue_number,
                reason="issue_comment handler not implemented",
            )
            return

        self.ledger.log(
            "EVENT_IGNORED",
            issue_number=event.issue_number,
            reason=f"unhandled {event.event_type}/{event.action}",
        )

    async def _on_labeled(self, event: WorkItemEvent) -> None:
        label = event.label or ""
        if label == _LABEL_STATUS_TRIAGE:
            await self._run_requirements(event)
            return
        if label == _LABEL_REVIEW_APPROVED:
            self.ledger.log(
                "STAGE_STUB",
                issue_number=event.issue_number,
                reason="review/approved handler not implemented",
            )
            return
        if label == _LABEL_STATUS_NEEDS_INFO:
            self.ledger.log(
                "STAGE_STUB",
                issue_number=event.issue_number,
                reason="status/needs-info handler not implemented",
            )
            return
        self.ledger.log(
            "EVENT_IGNORED",
            issue_number=event.issue_number,
            reason=f"label not routed: {label}",
        )

    async def _run_requirements(self, event: WorkItemEvent) -> None:
        work_item = self._work_item(event.issue_number)
        stage = Stage.REQUIREMENTS

        if work_item.revision_counts.get(stage, 0) >= _MAX_REVISIONS_PER_STAGE:
            self._trip_revision_limit(work_item, stage)
            return

        self.ledger.log(
            "AGENT_DISPATCH",
            issue_number=event.issue_number,
            stage=stage.value,
        )

        try:
            issue = self.github.get_issue(event.issue_number)
            comments = self.github.get_issue_comments(event.issue_number)
            context = build_requirements_context(
                issue, comments, bot_login=self.bot_login
            )
            requirements_doc = await self.requirements_agent.run(
                context, work_item
            )
        except IncompleteIssueError as exc:
            self._handle_incomplete_issue(work_item, exc)
            return
        except AgentError as exc:
            self._handle_agent_error(work_item, stage, exc)
            return
        except GitHubAPIError as exc:
            self._handle_agent_error(
                work_item,
                stage,
                AgentError(stage.value, f"GitHub API: {exc}"),
            )
            return

        body = _format_requirements_comment(requirements_doc)
        posted = self.github.post_comment(event.issue_number, body)
        comment_id = posted.get("id")
        if isinstance(comment_id, int):
            work_item.artifact_ids[stage] = comment_id

        self.github.add_label(event.issue_number, _LABEL_STATUS_REQUIREMENTS)
        self.github.remove_label(event.issue_number, _LABEL_STATUS_TRIAGE)
        work_item.stage = stage
        work_item.revision_counts[stage] = (
            work_item.revision_counts.get(stage, 0) + 1
        )

        self.ledger.log(
            "COMMENT_POSTED",
            issue_number=event.issue_number,
            stage=stage.value,
            comment_id=comment_id,
        )
        self.ledger.log(
            "LABEL_TRANSITION",
            issue_number=event.issue_number,
            stage=stage.value,
            added=_LABEL_STATUS_REQUIREMENTS,
            removed=_LABEL_STATUS_TRIAGE,
        )

    def _handle_incomplete_issue(
        self, work_item: WorkItem, exc: IncompleteIssueError
    ) -> None:
        issue_number = work_item.issue_number
        body = (
            "This issue doesn't yet have enough information to generate "
            "requirements.\n\n"
            f"**What's missing:** {exc.missing}\n\n"
            "Please update the issue with the missing details. A "
            "maintainer can re-trigger the agent by re-applying the "
            "`status/triage` label."
        )
        try:
            self.github.post_comment(issue_number, body)
            self.github.add_label(issue_number, _LABEL_STATUS_NEEDS_INFO)
            self.github.remove_label(issue_number, _LABEL_STATUS_TRIAGE)
        except GitHubAPIError as api_exc:
            self.ledger.log(
                "GITHUB_ERROR",
                issue_number=issue_number,
                reason=str(api_exc),
            )
        work_item.blocked_on = BlockReason.NEEDS_INFO
        self.ledger.log(
            "NEEDS_INFO",
            issue_number=issue_number,
            stage=Stage.REQUIREMENTS.value,
            missing=exc.missing,
        )

    def _handle_agent_error(
        self, work_item: WorkItem, stage: Stage, exc: AgentError
    ) -> None:
        issue_number = work_item.issue_number
        body = (
            f"The {stage.value} agent failed: `{exc.message}`.\n\n"
            "A maintainer should investigate and retry by re-adding the "
            "`status/triage` label once the root cause is fixed."
        )
        try:
            self.github.post_comment(issue_number, body)
            self.github.add_label(issue_number, _LABEL_FLAG_AGENT_ERROR)
        except GitHubAPIError as api_exc:
            self.ledger.log(
                "GITHUB_ERROR",
                issue_number=issue_number,
                reason=str(api_exc),
            )
        work_item.blocked_on = BlockReason.AGENT_ERROR
        self.ledger.log(
            "AGENT_ERROR",
            issue_number=issue_number,
            stage=stage.value,
            error=exc.message,
        )

    def _trip_revision_limit(self, work_item: WorkItem, stage: Stage) -> None:
        issue_number = work_item.issue_number
        body = (
            f"The {stage.value} stage has hit the revision limit "
            f"({_MAX_REVISIONS_PER_STAGE}). A maintainer should step in "
            "to resolve the blocking concern before the agent runs again."
        )
        try:
            self.github.post_comment(issue_number, body)
            self.github.add_label(issue_number, _LABEL_FLAG_REVISION_LIMIT)
        except GitHubAPIError as api_exc:
            self.ledger.log(
                "GITHUB_ERROR",
                issue_number=issue_number,
                reason=str(api_exc),
            )
        work_item.blocked_on = BlockReason.REVISION_LIMIT
        self.ledger.log(
            "REVISION_LIMIT",
            issue_number=issue_number,
            stage=stage.value,
            count=work_item.revision_counts.get(stage, 0),
        )


def _format_requirements_comment(doc: RequirementsDocument) -> str:
    header = (
        "> **Auto-generated requirements** — review and edit before "
        "accepting. This was produced by an AI assistant and may "
        "contain errors.\n>\n"
        "> Requirement IDs use the `REQ-TBD-` placeholder prefix; the "
        "Design stage will reassign prefixes to match the target design "
        "document.\n"
    )
    body_lines = [f"**{req.id}** {req.statement}" for req in doc.requirements]

    questions_heading = "## Open Questions"
    if doc.open_questions:
        questions = "\n".join(f"- {q}" for q in doc.open_questions)
    else:
        questions = "- None identified."

    return "\n\n".join(
        [header.rstrip(), "\n\n".join(body_lines), questions_heading, questions]
    )


async def safe_handle_event(
    orchestrator: Orchestrator, event: WorkItemEvent
) -> None:
    """Top-level background-task wrapper that never propagates exceptions.

    Any failure inside ``handle_event`` is logged to the ledger and, when
    possible, surfaced as a comment on the issue. We never want an
    unhandled exception to kill the background worker.
    """
    try:
        await orchestrator.handle_event(event)
    except Exception as exc:  # noqa: BLE001 - explicit top-level guard
        orchestrator.ledger.log(
            "UNHANDLED_EXCEPTION",
            issue_number=event.issue_number,
            error=repr(exc),
            traceback=traceback.format_exc(),
        )
        try:
            orchestrator.github.post_comment(
                event.issue_number,
                f"Internal orchestrator error: `{exc}`.",
            )
            orchestrator.github.add_label(
                event.issue_number, _LABEL_FLAG_AGENT_ERROR
            )
        except Exception as inner_exc:  # noqa: BLE001
            orchestrator.ledger.log(
                "UNHANDLED_EXCEPTION_FOLLOWUP_FAILED",
                issue_number=event.issue_number,
                error=repr(inner_exc),
            )
