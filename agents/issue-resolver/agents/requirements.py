"""Requirements-stage agent: validate an issue, then draft requirements.

Two LLM calls in sequence:

1. **Validate.** Ask Claude whether the issue contains enough substance
   — an ST program or snippet, a description of what IronPLC does now,
   and a description of what it should do instead. Structure is
   ignored; we care about content, not headers.
2. **Generate.** If validation passes, ask Claude to emit a structured
   requirements document whose requirement IDs use the
   ``REQ-TBD-NNN`` placeholder prefix. The future Design stage picks
   the real prefix (``REQ-CF-``, ``REQ-TH-``, …) once it knows which
   design doc the new requirements belong in.

Both calls are logged via :class:`Ledger` with a prompt hash and token
counts — never the prompt body.

The orchestrator distinguishes three outcomes:

- happy path → returns the requirements markdown,
- ``IncompleteIssueError`` → validation said the issue is missing
  information; orchestrator asks the reporter for more detail,
- ``AgentError`` → the Anthropic SDK itself failed; orchestrator raises
  the agent-error flag for a human.
"""

from __future__ import annotations

import json
from dataclasses import dataclass
from typing import Any

import anthropic

from ledger import Ledger


class IncompleteIssueError(Exception):
    def __init__(self, missing: str) -> None:
        super().__init__(missing)
        self.missing = missing


class AgentError(RuntimeError):
    def __init__(self, stage: str, message: str) -> None:
        super().__init__(f"[{stage}] {message}")
        self.stage = stage
        self.message = message


_VALIDATION_SYSTEM = """\
You are a triage assistant for the IronPLC compiler project. Your job is
to decide whether a GitHub issue contains enough information to generate
requirements.

An issue has enough information when it contains ALL of the following:
1. An ST (Structured Text) program or code snippet.
2. A description of what currently happens in IronPLC (actual behavior).
3. A description of what should happen instead (expected behavior).

The issue does NOT need to follow any template. Assess substance, not
structure. Information can appear anywhere in the title, body, or
follow-up comments.

Respond with a JSON object and nothing else:
{"sufficient": true}
or
{"sufficient": false, "missing": "Brief explanation of what is missing"}
"""


_REQUIREMENTS_SYSTEM = """\
You are a requirements engineer for the IronPLC compiler project. IronPLC
is an open-source IEC 61131-3 PLC runtime. Given a compatibility-gap
issue (title, body, and comment thread), produce a structured
requirements document.

Format each requirement as:
**REQ-TBD-NNN** The compiler SHALL ...

Where NNN is a sequential zero-padded number starting at 001. The
`REQ-TBD-` prefix is a placeholder — the Design stage will reassign it
to match the target design document (for example REQ-CF-, REQ-TH-, or
REQ-CG-). Do not invent a different prefix.

Guidelines:
- Use SHALL for mandatory behavior. Keep each requirement specific,
  testable, and traceable to what the issue describes.
- Stay inside the scope of the reported gap. Do not expand into
  unrelated features.
- Ground requirements in IEC 61131-3 clauses or tables when the issue
  cites one; otherwise leave spec references out.
- Where existing compiler problem codes (P####) are relevant, you may
  reference them, but do not invent new codes.

End the document with an "## Open Questions" section listing any
ambiguities that need maintainer input. If there are none, include the
heading with "- None identified." underneath.

Begin the output with this one-line note on a line by itself:
> IDs are placeholders — the Design stage will reassign prefixes to
> match the target design document (e.g. REQ-CF-, REQ-TH-, REQ-CG-).

Then a blank line, then the first requirement. No other preamble.
"""


@dataclass
class RequirementsAgent:
    client: anthropic.Anthropic
    ledger: Ledger
    model: str = "claude-sonnet-4-20250514"

    PLACEHOLDER_REQ_PREFIX = "REQ-TBD"
    STAGE = "requirements"

    async def run(
        self, context: dict[str, Any], _work_item: Any = None
    ) -> str:
        validation = self._validate(context)
        if not validation["sufficient"]:
            raise IncompleteIssueError(
                missing=validation.get("missing") or "unspecified"
            )
        return self._generate(context)

    def _validate(self, context: dict[str, Any]) -> dict[str, Any]:
        user_prompt = _format_user_prompt(context)
        try:
            resp = self.client.messages.create(
                model=self.model,
                max_tokens=500,
                system=_VALIDATION_SYSTEM,
                messages=[{"role": "user", "content": user_prompt}],
            )
        except anthropic.AnthropicError as exc:
            raise AgentError(self.STAGE, f"validation failed: {exc}") from exc
        except Exception as exc:  # network, unexpected
            raise AgentError(self.STAGE, f"validation failed: {exc}") from exc

        text = resp.content[0].text if resp.content else ""
        self.ledger.log_llm_call(
            issue_number=context["issue_number"],
            stage=self.STAGE,
            prompt=user_prompt,
            model=self.model,
            input_tokens=getattr(resp.usage, "input_tokens", 0),
            output_tokens=getattr(resp.usage, "output_tokens", 0),
            phase="validate",
        )
        return _parse_validation(text)

    def _generate(self, context: dict[str, Any]) -> str:
        user_prompt = _format_user_prompt(context)
        try:
            resp = self.client.messages.create(
                model=self.model,
                max_tokens=2000,
                system=_REQUIREMENTS_SYSTEM,
                messages=[{"role": "user", "content": user_prompt}],
            )
        except anthropic.AnthropicError as exc:
            raise AgentError(self.STAGE, f"generation failed: {exc}") from exc
        except Exception as exc:
            raise AgentError(self.STAGE, f"generation failed: {exc}") from exc

        text = resp.content[0].text if resp.content else ""
        self.ledger.log_llm_call(
            issue_number=context["issue_number"],
            stage=self.STAGE,
            prompt=user_prompt,
            model=self.model,
            input_tokens=getattr(resp.usage, "input_tokens", 0),
            output_tokens=getattr(resp.usage, "output_tokens", 0),
            phase="generate",
        )
        return text.strip()


def _format_user_prompt(context: dict[str, Any]) -> str:
    parts = [f"Issue #{context.get('issue_number')}: {context.get('issue_title', '')}"]
    body = (context.get("issue_body") or "").strip()
    if body:
        parts.append(f"Issue body:\n{body}")
    comments = context.get("comments") or []
    if comments:
        rendered_comments = []
        for c in comments:
            rendered_comments.append(
                f"[{c.get('created_at', '')}] @{c.get('author', '')}:\n"
                f"{c.get('body', '')}"
            )
        parts.append(
            "Follow-up comments (oldest first):\n\n"
            + "\n\n---\n\n".join(rendered_comments)
        )
    return "\n\n".join(parts)


def _parse_validation(text: str) -> dict[str, Any]:
    try:
        data = json.loads(text.strip())
    except (json.JSONDecodeError, AttributeError):
        return {"sufficient": False, "missing": "Unable to parse validation response"}
    return {
        "sufficient": bool(data.get("sufficient", False)),
        "missing": data.get("missing") or "",
    }
