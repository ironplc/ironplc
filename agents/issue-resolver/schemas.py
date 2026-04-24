"""Pydantic schemas for LLM input and output payloads.

Deliberately kept separate from ``models.py`` (which holds the
orchestrator's state-machine dataclasses). Instances of the classes
here flow *into* LLM calls (``IssueContext``) and *out of* them
(``ValidationResult``, ``RequirementsDocument``). Their JSON schemas
are handed to Anthropic as tool definitions so Claude returns payloads
the Python side can round-trip through ``model_validate`` — no more
hand-rolled JSON parsing.
"""

from __future__ import annotations

from pydantic import BaseModel, ConfigDict, Field


class IssueComment(BaseModel):
    """One follow-up comment on a GitHub issue."""

    model_config = ConfigDict(frozen=True)

    author: str = ""
    body: str = ""
    created_at: str = ""


class IssueContext(BaseModel):
    """Everything a stage agent sees about an issue.

    Produced by ``context.build_requirements_context``; consumed by
    ``RequirementsAgent.run``. The issue body is preserved verbatim;
    comments are chronological with bot-authored entries already
    filtered out.
    """

    model_config = ConfigDict(frozen=True)

    issue_number: int
    issue_title: str = ""
    issue_body: str = ""
    comments: list[IssueComment] = Field(default_factory=list)


class ValidationResult(BaseModel):
    """Outcome of the Requirements-stage pre-check LLM call."""

    model_config = ConfigDict(frozen=True)

    sufficient: bool = Field(
        description=(
            "True when the issue contains a code snippet, a description "
            "of actual behavior, and a description of expected behavior."
        )
    )
    missing: str = Field(
        default="",
        description=(
            "When sufficient is false, a short human-readable explanation "
            "of what is missing. Empty string otherwise."
        ),
    )


class Requirement(BaseModel):
    """One SHALL-style requirement statement."""

    model_config = ConfigDict(frozen=True)

    id: str = Field(
        description=(
            "Placeholder ID in the form REQ-TBD-NNN with zero-padded "
            "three-digit sequential suffix starting at 001. The Design "
            "stage reassigns the prefix later."
        )
    )
    statement: str = Field(
        description="A single SHALL sentence describing the required behavior."
    )


class RequirementsDocument(BaseModel):
    """Structured output of the Requirements agent.

    The orchestrator renders this into a Markdown comment when posting
    back to GitHub.
    """

    model_config = ConfigDict(frozen=True)

    requirements: list[Requirement]
    open_questions: list[str] = Field(
        default_factory=list,
        description=(
            "Ambiguities or decisions that need maintainer input. Empty "
            "list when nothing remains."
        ),
    )
