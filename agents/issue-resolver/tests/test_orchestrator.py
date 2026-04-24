"""Tests for orchestrator routing, with the agent mocked out."""

from __future__ import annotations

import asyncio
import tempfile
import unittest
from unittest.mock import AsyncMock, MagicMock

from agents.requirements import AgentError, IncompleteIssueError
from ledger import Ledger
from models import BlockReason, Stage, WorkItemEvent
from orchestrator import Orchestrator
from schemas import Requirement, RequirementsDocument


def _event(label: str = "status/triage") -> WorkItemEvent:
    return WorkItemEvent(
        issue_number=42,
        event_type="issues",
        action="labeled",
        label=label,
        actor="reporter",
    )


def _happy_doc() -> RequirementsDocument:
    return RequirementsDocument(
        requirements=[
            Requirement(id="REQ-TBD-001", statement="The compiler SHALL do the thing."),
            Requirement(id="REQ-TBD-002", statement="The compiler SHALL do another thing."),
        ],
        open_questions=["Does the second thing apply to LREAL too?"],
    )


def _make_orchestrator() -> tuple[Orchestrator, MagicMock, AsyncMock]:
    tmp = tempfile.NamedTemporaryFile(suffix=".db", delete=False)
    tmp.close()
    ledger = Ledger(tmp.name)
    github = MagicMock()
    github.get_issue.return_value = {
        "number": 42,
        "title": "MOD not supported",
        "body": "something",
    }
    github.get_issue_comments.return_value = []
    github.post_comment.return_value = {"id": 1234}
    agent = AsyncMock()
    orchestrator = Orchestrator(
        github=github,
        ledger=ledger,
        requirements_agent=agent,
        bot_login="ironplc-bot",
    )
    return orchestrator, github, agent


class TestOrchestrator(unittest.TestCase):
    def test_triage_happy_path_posts_comment_and_transitions_labels(self) -> None:
        orch, github, agent = _make_orchestrator()
        agent.run.return_value = _happy_doc()

        asyncio.run(orch.handle_event(_event()))

        github.post_comment.assert_called_once()
        args, _ = github.post_comment.call_args
        self.assertEqual(args[0], 42)
        body = args[1]
        self.assertIn("REQ-TBD-001", body)
        self.assertIn("REQ-TBD-002", body)
        self.assertIn("Does the second thing apply", body)
        github.add_label.assert_any_call(42, "status/requirements")
        github.remove_label.assert_any_call(42, "status/triage")

        wi = orch._work_item(42)
        self.assertEqual(wi.stage, Stage.REQUIREMENTS)
        self.assertEqual(wi.revision_counts[Stage.REQUIREMENTS], 1)
        self.assertEqual(wi.artifact_ids[Stage.REQUIREMENTS], 1234)

    def test_happy_path_with_no_open_questions_then_renders_none_identified(self) -> None:
        orch, github, agent = _make_orchestrator()
        agent.run.return_value = RequirementsDocument(
            requirements=[
                Requirement(id="REQ-TBD-001", statement="SHALL x.")
            ],
            open_questions=[],
        )

        asyncio.run(orch.handle_event(_event()))

        body = github.post_comment.call_args.args[1]
        self.assertIn("None identified", body)

    def test_incomplete_issue_posts_needs_info_and_swaps_labels(self) -> None:
        orch, github, agent = _make_orchestrator()
        agent.run.side_effect = IncompleteIssueError(missing="No ST program")

        asyncio.run(orch.handle_event(_event()))

        github.post_comment.assert_called_once()
        body = github.post_comment.call_args.args[1]
        self.assertIn("No ST program", body)
        github.add_label.assert_any_call(42, "status/needs-info")
        github.remove_label.assert_any_call(42, "status/triage")

        self.assertEqual(orch._work_item(42).blocked_on, BlockReason.NEEDS_INFO)

    def test_agent_error_flags_and_blocks(self) -> None:
        orch, github, agent = _make_orchestrator()
        agent.run.side_effect = AgentError("requirements", "API timed out")

        asyncio.run(orch.handle_event(_event()))

        github.post_comment.assert_called_once()
        github.add_label.assert_any_call(42, "flag/agent-error")
        self.assertEqual(
            orch._work_item(42).blocked_on, BlockReason.AGENT_ERROR
        )

    def test_revision_limit_trips_after_third_pass(self) -> None:
        orch, github, agent = _make_orchestrator()
        agent.run.return_value = _happy_doc()

        for _ in range(3):
            asyncio.run(orch.handle_event(_event()))
        # Fourth triage attempt trips the revision limit.
        asyncio.run(orch.handle_event(_event()))

        github.add_label.assert_any_call(42, "flag/revision-limit")
        self.assertEqual(
            orch._work_item(42).blocked_on, BlockReason.REVISION_LIMIT
        )

    def test_unknown_label_logs_and_ignores(self) -> None:
        orch, github, agent = _make_orchestrator()
        asyncio.run(orch.handle_event(_event(label="area/runtime")))
        github.post_comment.assert_not_called()
        agent.run.assert_not_called()

    def test_bot_comment_is_ignored(self) -> None:
        orch, github, agent = _make_orchestrator()
        event = WorkItemEvent(
            issue_number=42,
            event_type="issue_comment",
            action="created",
            actor="ironplc-bot",
            comment_body="Auto-generated ...",
        )
        asyncio.run(orch.handle_event(event))
        github.post_comment.assert_not_called()
        agent.run.assert_not_called()


if __name__ == "__main__":
    unittest.main()
