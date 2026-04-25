"""Append-only SQLite event ledger with stdout mirror.

Every operation the orchestrator takes — webhook received, agent
dispatched, LLM response returned, comment posted, label transitioned,
error raised — is recorded here. The ledger is the system's replay log
and blast-radius bound: lose the ``WorkItem`` dict in memory and we can
still rebuild what happened by walking ``ledger.db``.

Two sinks are written to on every call:

1. **stdout** in a column-aligned format humans can scan while tailing
   ``uvicorn`` logs.
2. **SQLite** at ``./ledger.db`` via a fresh connection per call (safe
   under FastAPI's async handlers without an external connection pool).

Sensitive values are never stored. LLM prompts are written as
``sha256(prompt)`` only; we record token counts so we can price calls
without keeping the text.
"""

from __future__ import annotations

import hashlib
import json
import sqlite3
import sys
import threading
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

_SCHEMA = """
CREATE TABLE IF NOT EXISTS events (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    ts            TEXT    NOT NULL,
    issue_number  INTEGER,
    event_type    TEXT    NOT NULL,
    stage         TEXT,
    details_json  TEXT
);
CREATE INDEX IF NOT EXISTS events_issue_idx ON events(issue_number);
"""

_init_lock = threading.Lock()
_initialized: set[str] = set()


class Ledger:
    def __init__(self, db_path: str | Path = "ledger.db") -> None:
        self.db_path = str(db_path)
        self._ensure_schema()

    def _ensure_schema(self) -> None:
        with _init_lock:
            if self.db_path in _initialized:
                return
            conn = sqlite3.connect(self.db_path)
            try:
                conn.executescript(_SCHEMA)
                conn.commit()
            finally:
                conn.close()
            _initialized.add(self.db_path)

    def log(
        self,
        event_type: str,
        *,
        issue_number: int | None = None,
        stage: str | None = None,
        **details: Any,
    ) -> None:
        ts = datetime.now(timezone.utc).isoformat()
        payload = json.dumps(details, default=str) if details else None

        issue_col = f"#{issue_number}" if issue_number is not None else "-"
        stage_col = stage or "-"
        detail_str = json.dumps(details, default=str) if details else ""
        print(
            f"{ts}  {issue_col:>6}  {stage_col:<13}  "
            f"{event_type:<20}  {detail_str}",
            file=sys.stdout,
            flush=True,
        )

        conn = sqlite3.connect(self.db_path)
        try:
            conn.execute(
                "INSERT INTO events "
                "(ts, issue_number, event_type, stage, details_json) "
                "VALUES (?, ?, ?, ?, ?)",
                (ts, issue_number, event_type, stage, payload),
            )
            conn.commit()
        finally:
            conn.close()

    def log_llm_call(
        self,
        *,
        issue_number: int,
        stage: str,
        prompt: str,
        model: str,
        input_tokens: int,
        output_tokens: int,
        phase: str = "",
    ) -> None:
        prompt_hash = hashlib.sha256(prompt.encode("utf-8")).hexdigest()
        self.log(
            "LLM_RESPONSE",
            issue_number=issue_number,
            stage=stage,
            phase=phase,
            model=model,
            prompt_sha256=prompt_hash,
            input_tokens=input_tokens,
            output_tokens=output_tokens,
        )

    def get_history(self, issue_number: int) -> list[dict[str, Any]]:
        conn = sqlite3.connect(self.db_path)
        conn.row_factory = sqlite3.Row
        try:
            rows = conn.execute(
                "SELECT ts, event_type, stage, details_json "
                "FROM events WHERE issue_number = ? ORDER BY id",
                (issue_number,),
            ).fetchall()
        finally:
            conn.close()
        return [
            {
                "ts": r["ts"],
                "event_type": r["event_type"],
                "stage": r["stage"],
                "details": (
                    json.loads(r["details_json"]) if r["details_json"] else {}
                ),
            }
            for r in rows
        ]
