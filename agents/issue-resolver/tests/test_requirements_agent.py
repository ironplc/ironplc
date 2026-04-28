"""Tests for RequirementsAgent (Anthropic tool-use + Pydantic path).

The Anthropic client is fully mocked — no network calls. The goal is
to exercise:
- tool definitions include the Pydantic JSON schemas,
- tool-use blocks are extracted into validated schema objects,
- validation failure and malformed tool input both surface as
  ``AgentError``.
"""

from __future__ import annotations

import asyncio
import tempfile
import unittest
from unittest.mock import MagicMock

import anthropic

from agents.requirements import (
    AgentError,
    IncompleteIssueError,
    RequirementsAgent,
    _REQUIREMENTS_SYSTEM,
    _VALIDATION_SYSTEM,
)
from ledger import Ledger
from schemas import IssueContext, RequirementsDocument, ValidationResult


def _context() -> IssueContext:
    return IssueContext(
        issue_number=99,
        issue_title="MOD operator not supported",
        issue_body="```st\nx:=10 MOD 3;\n```\nExpected 1, got an error.",
    )


def _tool_use_block(name: str, input_payload: dict) -> MagicMock:
    block = MagicMock()
    block.type = "tool_use"
    block.name = name
    block.input = input_payload
    return block


def _anthropic_response(blocks: list, in_tokens: int = 10, out_tokens: int = 20) -> MagicMock:
    resp = MagicMock()
    resp.content = blocks
    resp.usage.input_tokens = in_tokens
    resp.usage.output_tokens = out_tokens
    return resp


def _make_agent() -> tuple[RequirementsAgent, MagicMock]:
    tmp = tempfile.NamedTemporaryFile(suffix=".db", delete=False)
    tmp.close()
    ledger = Ledger(tmp.name)
    client = MagicMock()
    return (
        RequirementsAgent(client=client, ledger=ledger),
        client,
    )


class TestRequirementsAgent(unittest.TestCase):
    def test_run_when_validation_sufficient_then_returns_requirements_document(self) -> None:
        agent, client = _make_agent()
        client.messages.create.side_effect = [
            _anthropic_response([
                _tool_use_block("report_validation", {"sufficient": True, "missing": ""})
            ]),
            _anthropic_response([
                _tool_use_block(
                    "report_requirements",
                    {
                        "requirements": [
                            {"id": "REQ-TBD-001", "statement": "The compiler SHALL support MOD."},
                        ],
                        "open_questions": [],
                    },
                )
            ]),
        ]

        doc = asyncio.run(agent.run(_context()))
        self.assertIsInstance(doc, RequirementsDocument)
        self.assertEqual(len(doc.requirements), 1)
        self.assertEqual(doc.requirements[0].id, "REQ-TBD-001")
        self.assertEqual(client.messages.create.call_count, 2)

    def test_run_when_validation_insufficient_then_raises_incomplete(self) -> None:
        agent, client = _make_agent()
        client.messages.create.return_value = _anthropic_response([
            _tool_use_block(
                "report_validation",
                {"sufficient": False, "missing": "No ST program provided"},
            )
        ])

        with self.assertRaises(IncompleteIssueError) as ctx:
            asyncio.run(agent.run(_context()))
        self.assertEqual(ctx.exception.missing, "No ST program provided")
        # Only the validation call should have been made.
        self.assertEqual(client.messages.create.call_count, 1)

    def test_run_when_sdk_raises_then_agent_error(self) -> None:
        agent, client = _make_agent()
        client.messages.create.side_effect = anthropic.AnthropicError("rate limit")

        with self.assertRaises(AgentError) as ctx:
            asyncio.run(agent.run(_context()))
        self.assertIn("validate failed", str(ctx.exception))

    def test_run_when_tool_input_invalid_then_agent_error(self) -> None:
        agent, client = _make_agent()
        client.messages.create.side_effect = [
            _anthropic_response([
                _tool_use_block("report_validation", {"sufficient": True, "missing": ""})
            ]),
            _anthropic_response([
                _tool_use_block(
                    "report_requirements",
                    # Missing required "requirements" field → schema violation.
                    {"open_questions": ["a"]},
                )
            ]),
        ]
        with self.assertRaises(AgentError) as ctx:
            asyncio.run(agent.run(_context()))
        self.assertIn("invalid requirements payload", str(ctx.exception))

    def test_run_when_no_tool_use_block_then_agent_error(self) -> None:
        agent, client = _make_agent()
        text_block = MagicMock()
        text_block.type = "text"
        text_block.text = "I refuse to call the tool."
        client.messages.create.return_value = _anthropic_response([text_block])

        with self.assertRaises(AgentError) as ctx:
            asyncio.run(agent.run(_context()))
        self.assertIn("no report_validation tool_use", str(ctx.exception))

    def test_tool_choice_forces_correct_tool_name(self) -> None:
        agent, client = _make_agent()
        client.messages.create.side_effect = [
            _anthropic_response([
                _tool_use_block("report_validation", {"sufficient": True, "missing": ""})
            ]),
            _anthropic_response([
                _tool_use_block(
                    "report_requirements",
                    {"requirements": [{"id": "REQ-TBD-001", "statement": "SHALL x."}], "open_questions": []},
                )
            ]),
        ]
        asyncio.run(agent.run(_context()))
        first, second = client.messages.create.call_args_list
        self.assertEqual(
            first.kwargs["tool_choice"],
            {"type": "tool", "name": "report_validation"},
        )
        self.assertEqual(
            second.kwargs["tool_choice"],
            {"type": "tool", "name": "report_requirements"},
        )


class TestValidationResultSchema(unittest.TestCase):
    def test_validation_result_when_missing_defaults_to_empty(self) -> None:
        v = ValidationResult.model_validate({"sufficient": True})
        self.assertEqual(v.missing, "")


class TestSystemPrompts(unittest.TestCase):
    def test_validation_prompt_when_loaded_then_non_empty_and_mentions_tool(
        self,
    ) -> None:
        self.assertTrue(_VALIDATION_SYSTEM.strip())
        self.assertIn("report_validation", _VALIDATION_SYSTEM)

    def test_requirements_prompt_when_loaded_then_non_empty_and_mentions_tool(
        self,
    ) -> None:
        self.assertTrue(_REQUIREMENTS_SYSTEM.strip())
        self.assertIn("report_requirements", _REQUIREMENTS_SYSTEM)
        self.assertIn("REQ-TBD", _REQUIREMENTS_SYSTEM)


if __name__ == "__main__":
    unittest.main()
