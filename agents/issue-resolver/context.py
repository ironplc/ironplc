"""Builders that package raw GitHub REST data into typed LLM contexts.

Deliberately *not* a parser. Earlier iterations tried to split the
issue body on ``##`` or ``**Field**`` headers and enforce each expected
field was present, which either rejected valid issues that didn't
follow the template or silently accepted broken ones. The Requirements
agent now does content extraction itself via an LLM call, so this
module's job is to hand the model a clean chronological view of what
the user wrote — typed into ``IssueContext`` so downstream code never
sees a loose dict.

Later stages will add ``build_design_context`` etc. Those are stubbed
with ``NotImplementedError`` so the module clearly enumerates what
still needs to be built.
"""

from __future__ import annotations

from typing import Any

from schemas import IssueComment, IssueContext


def build_requirements_context(
    issue: dict[str, Any],
    comments: list[dict[str, Any]],
    *,
    bot_login: str | None = None,
) -> IssueContext:
    """Package a GitHub issue + its comments into an ``IssueContext``.

    ``issue`` and ``comments`` arrive as raw GitHub REST JSON; this
    function is the one place the untyped external shape is tolerated.
    Comments authored by the bot itself are filtered out — without the
    filter the agent would loop on its own prior requirement drafts.
    """
    packaged: list[IssueComment] = []
    for raw_comment in comments:
        user = raw_comment.get("user") or {}
        author = user.get("login", "")
        if bot_login and author == bot_login:
            continue
        packaged.append(
            IssueComment(
                author=author,
                body=raw_comment.get("body") or "",
                created_at=raw_comment.get("created_at", ""),
            )
        )

    issue_number = issue.get("number")
    if not isinstance(issue_number, int):
        raise ValueError(f"issue has no integer 'number' field: {issue_number!r}")

    return IssueContext(
        issue_number=issue_number,
        issue_title=issue.get("title") or "",
        issue_body=issue.get("body") or "",
        comments=packaged,
    )


def build_design_context(*_args: Any, **_kwargs: Any) -> IssueContext:
    raise NotImplementedError("Design stage is not implemented yet")


def build_plan_context(*_args: Any, **_kwargs: Any) -> IssueContext:
    raise NotImplementedError("Plan stage is not implemented yet")


def build_code_context(*_args: Any, **_kwargs: Any) -> IssueContext:
    raise NotImplementedError("Code stage is not implemented yet")
