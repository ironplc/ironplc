"""Builders that package raw GitHub data into LLM-ready context dicts.

Deliberately *not* a parser. Earlier iterations tried to split the
issue body on ``##`` or ``**Field**`` headers and enforce that each
expected field was present — which either rejected valid issues that
didn't follow the template or silently accepted broken ones. The
Requirements agent now does the extraction itself via an LLM call, so
this module's job is just to hand the model a clean, chronological view
of what the user wrote.

Later stages will add ``build_design_context`` etc. Those are stubbed
with ``NotImplementedError`` so the module clearly enumerates the stages
that remain to be built.
"""

from __future__ import annotations

from typing import Any


def build_requirements_context(
    issue: dict[str, Any],
    comments: list[dict[str, Any]],
    *,
    bot_login: str | None = None,
) -> dict[str, Any]:
    """Package an issue plus its comments for the Requirements agent.

    The issue body is preserved verbatim (``None`` becomes ``""``).
    Comments are included chronologically with the bot's own comments
    filtered out — without that filter the agent would loop on its own
    prior requirement drafts.
    """
    packaged_comments: list[dict[str, Any]] = []
    for comment in comments:
        user = comment.get("user") or {}
        author = user.get("login", "")
        if bot_login and author == bot_login:
            continue
        packaged_comments.append(
            {
                "author": author,
                "body": comment.get("body") or "",
                "created_at": comment.get("created_at", ""),
            }
        )

    return {
        "issue_number": issue.get("number"),
        "issue_title": issue.get("title", ""),
        "issue_body": issue.get("body") or "",
        "comments": packaged_comments,
    }


def build_design_context(*_args: Any, **_kwargs: Any) -> dict[str, Any]:
    raise NotImplementedError("Design stage is not implemented yet")


def build_plan_context(*_args: Any, **_kwargs: Any) -> dict[str, Any]:
    raise NotImplementedError("Plan stage is not implemented yet")


def build_code_context(*_args: Any, **_kwargs: Any) -> dict[str, Any]:
    raise NotImplementedError("Code stage is not implemented yet")
