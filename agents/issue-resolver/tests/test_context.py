"""Tests for context.build_requirements_context (typed packaging)."""

from __future__ import annotations

import unittest

from context import build_requirements_context
from schemas import IssueContext


class TestBuildRequirementsContext(unittest.TestCase):
    def test_when_body_and_comments_present_then_typed_context(self) -> None:
        issue = {
            "number": 42,
            "title": "MOD not supported",
            "body": "```st\nx:=10 MOD 3;\n```",
        }
        comments = [
            {
                "user": {"login": "alice"},
                "body": "Here's more info",
                "created_at": "2026-04-20T10:00:00Z",
            }
        ]
        context = build_requirements_context(issue, comments)
        self.assertIsInstance(context, IssueContext)
        self.assertEqual(context.issue_number, 42)
        self.assertEqual(context.issue_title, "MOD not supported")
        self.assertIn("MOD", context.issue_body)
        self.assertEqual(len(context.comments), 1)
        self.assertEqual(context.comments[0].author, "alice")
        self.assertEqual(context.comments[0].body, "Here's more info")

    def test_when_body_none_then_empty_string(self) -> None:
        context = build_requirements_context(
            {"number": 1, "title": "t", "body": None}, []
        )
        self.assertEqual(context.issue_body, "")

    def test_when_bot_comments_then_filtered(self) -> None:
        issue = {"number": 1, "title": "t", "body": ""}
        comments = [
            {
                "user": {"login": "alice"},
                "body": "question",
                "created_at": "2026-04-20T10:00:00Z",
            },
            {
                "user": {"login": "ironplc-bot"},
                "body": "Auto-generated requirements ...",
                "created_at": "2026-04-20T10:01:00Z",
            },
            {
                "user": {"login": "bob"},
                "body": "follow-up",
                "created_at": "2026-04-20T10:02:00Z",
            },
        ]
        context = build_requirements_context(
            issue, comments, bot_login="ironplc-bot"
        )
        self.assertEqual(
            [c.author for c in context.comments], ["alice", "bob"]
        )

    def test_when_comment_body_missing_then_tolerated(self) -> None:
        issue = {"number": 1, "title": "t", "body": "b"}
        comments = [{"user": {"login": "alice"}, "body": None}]
        context = build_requirements_context(issue, comments)
        self.assertEqual(context.comments[0].body, "")

    def test_when_issue_number_missing_then_raises(self) -> None:
        with self.assertRaises(ValueError):
            build_requirements_context({"title": "t", "body": "b"}, [])


if __name__ == "__main__":
    unittest.main()
