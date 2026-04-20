"""Tests for the SQLite event ledger."""

from __future__ import annotations

import os
import tempfile
import unittest

from ledger import Ledger


class TestLedger(unittest.TestCase):
    def setUp(self) -> None:
        self._tmp = tempfile.NamedTemporaryFile(
            suffix=".db", delete=False
        )
        self._tmp.close()
        self.path = self._tmp.name
        self.ledger = Ledger(self.path)

    def tearDown(self) -> None:
        os.unlink(self.path)

    def test_log_when_called_then_roundtrips(self) -> None:
        self.ledger.log(
            "WEBHOOK_RECEIVED",
            issue_number=42,
            stage="requirements",
            action="labeled",
        )
        history = self.ledger.get_history(42)
        self.assertEqual(len(history), 1)
        self.assertEqual(history[0]["event_type"], "WEBHOOK_RECEIVED")
        self.assertEqual(history[0]["stage"], "requirements")
        self.assertEqual(history[0]["details"]["action"], "labeled")

    def test_log_when_multiple_events_then_history_ordered(self) -> None:
        self.ledger.log("FIRST", issue_number=1)
        self.ledger.log("SECOND", issue_number=1)
        self.ledger.log("THIRD", issue_number=2)
        one = [e["event_type"] for e in self.ledger.get_history(1)]
        two = [e["event_type"] for e in self.ledger.get_history(2)]
        self.assertEqual(one, ["FIRST", "SECOND"])
        self.assertEqual(two, ["THIRD"])

    def test_log_llm_call_when_called_then_stores_hash_not_prompt(self) -> None:
        self.ledger.log_llm_call(
            issue_number=99,
            stage="requirements",
            prompt="very secret prompt text",
            model="claude-sonnet-4",
            input_tokens=120,
            output_tokens=340,
            phase="validate",
        )
        history = self.ledger.get_history(99)
        self.assertEqual(len(history), 1)
        details = history[0]["details"]
        self.assertIn("prompt_sha256", details)
        self.assertNotIn("prompt", details)
        self.assertEqual(details["input_tokens"], 120)
        self.assertEqual(details["output_tokens"], 340)
        self.assertEqual(details["phase"], "validate")


if __name__ == "__main__":
    unittest.main()
