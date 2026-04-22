"""Requirements-stage agent: validate an issue, then draft requirements.

Two LLM calls in sequence, both driven by Anthropic's tool-use API so
we get JSON-schema-validated structured output instead of ad-hoc string
parsing:

1. **Validate.** Ask Claude whether the issue contains enough
   substance — a code snippet, a description of actual behavior, and
   a description of expected behavior. Structure is ignored; only
   content matters. Claude reports the answer via a
   ``report_validation`` tool whose schema is
   :class:`ValidationResult`.
2. **Generate.** If validation passes, ask Claude to emit a
   ``RequirementsDocument`` via a ``report_requirements`` tool. IDs use
   the ``REQ-TBD-NNN`` placeholder prefix; the future Design stage
   picks the real prefix (``REQ-CF-``, ``REQ-TH-``, …) once it knows
   which design doc the new requirements belong in.

Outcomes the orchestrator distinguishes:

- happy path → returns a ``RequirementsDocument``,
- ``IncompleteIssueError`` → validation says the issue is missing
  information; orchestrator asks the reporter for more detail,
- ``AgentError`` → the Anthropic SDK itself failed or the tool-use
  payload didn't validate; orchestrator raises the agent-error flag.

Both LLM calls are logged via :class:`Ledger` with a prompt hash and
token counts — never the prompt body.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any

import anthropic
from pydantic import ValidationError

from ledger import Ledger
from models import WorkItem
from schemas import IssueContext, RequirementsDocument, ValidationResult


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
You are a triage assistant for the IronPLC compiler project. Your job
is to decide whether a GitHub issue contains enough information for a
requirements engineer to draft requirements.

An issue has enough information when it contains ALL of the following:
1. A code snippet, typically IEC 61131-3 Structured Text.
2. A description of what currently happens in IronPLC (actual
   behavior).
3. A description of what should happen instead (expected behavior).

The issue does NOT need to follow any template. Assess substance, not
structure. Relevant information may appear anywhere in the title,
body, or follow-up comments.

Call the `report_validation` tool exactly once with your answer.
"""


_REQUIREMENTS_SYSTEM = """\
You are a requirements engineer for the IronPLC compiler project.
IronPLC is an open-source runtime for the IEC 61131-3:2013 PLC
programming languages. Given a GitHub issue (title, body, and comment
thread), produce a structured requirements document.

Each requirement must:
- Use a placeholder ID of the form `REQ-TBD-NNN`, sequential, zero-
  padded, starting at 001. The Design stage will reassign the prefix
  later; do not invent another prefix here.
- Contain one SHALL sentence describing required behavior.
- Be specific, testable, and traceable to the reported problem.
- Stay inside the scope of the reported issue; do not expand into
  unrelated features.
- When the issue cites a clause or table from the standard, ground
  the requirement by referring to IEC 61131-3:2013 and the specific
  clause or table number.

Follow the requirement conventions that already exist in the IronPLC
repository; do not invent new formatting rules.

Use the `open_questions` field for ambiguities that need maintainer
input, and leave it empty when there are none.

Call the `report_requirements` tool exactly once with the finished
document.
"""


def _tool_from_schema(name: str, description: str, schema: dict[str, Any]) -> dict[str, Any]:
    # Anthropic's input_schema must be a JSON Schema object. Pydantic's
    # model_json_schema() already returns one; we just wrap it as the
    # tool definition expected by messages.create(tools=...).
    return {"name": name, "description": description, "input_schema": schema}


_VALIDATION_TOOL = _tool_from_schema(
    name="report_validation",
    description="Report whether the issue is ready for requirements drafting.",
    schema=ValidationResult.model_json_schema(),
)

_REQUIREMENTS_TOOL = _tool_from_schema(
    name="report_requirements",
    description="Report the structured requirements document for the issue.",
    schema=RequirementsDocument.model_json_schema(),
)


@dataclass
class RequirementsAgent:
    """Runs the validate → generate pair for one issue.

    ``model`` defaults to Claude Sonnet 4.6, the current production
    Sonnet. Sonnet is the right capability tier for this task — it
    produces the SHALL-style structured output we need at materially
    lower cost than Opus, and its long-context handling covers lengthy
    comment threads. Override this to pin an older model for
    reproducibility, or to upgrade once a newer model has been
    validated against a small set of real issues.
    """

    client: anthropic.Anthropic
    ledger: Ledger
    model: str = "claude-sonnet-4-6"

    PLACEHOLDER_REQ_PREFIX = "REQ-TBD"
    STAGE = "requirements"

    async def run(
        self,
        context: IssueContext,
        _work_item: WorkItem | None = None,
    ) -> RequirementsDocument:
        validation = self._validate(context)
        if not validation.sufficient:
            raise IncompleteIssueError(
                missing=validation.missing or "unspecified"
            )
        return self._generate(context)

    def _validate(self, context: IssueContext) -> ValidationResult:
        payload = self._tool_call(
            context=context,
            user_prompt=_format_user_prompt(context),
            system=_VALIDATION_SYSTEM,
            tool=_VALIDATION_TOOL,
            max_tokens=500,
            phase="validate",
        )
        try:
            return ValidationResult.model_validate(payload)
        except ValidationError as exc:
            raise AgentError(
                self.STAGE, f"invalid validation payload: {exc}"
            ) from exc

    def _generate(self, context: IssueContext) -> RequirementsDocument:
        payload = self._tool_call(
            context=context,
            user_prompt=_format_user_prompt(context),
            system=_REQUIREMENTS_SYSTEM,
            tool=_REQUIREMENTS_TOOL,
            max_tokens=2000,
            phase="generate",
        )
        try:
            return RequirementsDocument.model_validate(payload)
        except ValidationError as exc:
            raise AgentError(
                self.STAGE, f"invalid requirements payload: {exc}"
            ) from exc

    def _tool_call(
        self,
        *,
        context: IssueContext,
        user_prompt: str,
        system: str,
        tool: dict[str, Any],
        max_tokens: int,
        phase: str,
    ) -> dict[str, Any]:
        try:
            resp = self.client.messages.create(
                model=self.model,
                max_tokens=max_tokens,
                system=system,
                messages=[{"role": "user", "content": user_prompt}],
                tools=[tool],
                tool_choice={"type": "tool", "name": tool["name"]},
            )
        except anthropic.AnthropicError as exc:
            raise AgentError(self.STAGE, f"{phase} failed: {exc}") from exc
        except Exception as exc:
            raise AgentError(self.STAGE, f"{phase} failed: {exc}") from exc

        self.ledger.log_llm_call(
            issue_number=context.issue_number,
            stage=self.STAGE,
            prompt=user_prompt,
            model=self.model,
            input_tokens=getattr(resp.usage, "input_tokens", 0),
            output_tokens=getattr(resp.usage, "output_tokens", 0),
            phase=phase,
        )

        for block in resp.content:
            if (
                getattr(block, "type", None) == "tool_use"
                and getattr(block, "name", None) == tool["name"]
            ):
                tool_input = getattr(block, "input", None)
                if isinstance(tool_input, dict):
                    return tool_input
                raise AgentError(
                    self.STAGE,
                    f"{phase} tool_use had non-dict input: {type(tool_input)}",
                )
        raise AgentError(
            self.STAGE, f"no {tool['name']} tool_use block in {phase} response"
        )


def _format_user_prompt(context: IssueContext) -> str:
    parts = [f"Issue #{context.issue_number}: {context.issue_title}"]
    body = (context.issue_body or "").strip()
    if body:
        parts.append(f"Issue body:\n{body}")
    if context.comments:
        rendered = [
            f"[{c.created_at}] @{c.author}:\n{c.body}"
            for c in context.comments
        ]
        parts.append(
            "Follow-up comments (oldest first):\n\n"
            + "\n\n---\n\n".join(rendered)
        )
    return "\n\n".join(parts)
