"""FastAPI entrypoint for the issue-resolver webhook orchestrator.

Accepts ``POST /webhook`` from GitHub, verifies the HMAC signature, and
hands supported events off to the orchestrator on a background task so
the HTTP response returns immediately. A failed signature verification
returns 401 without revealing why.

Startup eagerly loads configuration and initializes the ledger so that
a misconfigured deployment fails fast rather than at the first webhook.
"""

from __future__ import annotations

import json
import logging
import os

import anthropic
from fastapi import BackgroundTasks, FastAPI, Header, Request, Response

from agents.requirements import RequirementsAgent
from config import load_config
from github_client import (
    DryRunGitHubClient,
    GitHubClient,
    verify_webhook_signature,
)
from ledger import Ledger
from models import WorkItemEvent
from orchestrator import Orchestrator, safe_handle_event


logger = logging.getLogger("issue_resolver")
logging.basicConfig(level=logging.INFO)


def create_app() -> FastAPI:
    app = FastAPI(title="ironplc-issue-resolver", version="0.1.0")

    config = load_config()
    ledger = Ledger()
    real_github = GitHubClient(
        token=config.github_token, repo=config.github_repo
    )
    github: GitHubClient | DryRunGitHubClient = real_github
    if os.environ.get("DRY_RUN") == "true":
        github = DryRunGitHubClient(real_github)
        logger.warning(
            "DRY_RUN=true: comments and label changes will be printed, "
            "not sent to GitHub"
        )
    anthropic_client = anthropic.Anthropic(api_key=config.anthropic_api_key)

    bot_login: str | None
    try:
        bot_login = github.get_authenticated_user_login()
    except Exception:  # noqa: BLE001 - startup best-effort
        bot_login = None
        logger.warning("Could not resolve bot login at startup")

    orchestrator = Orchestrator(
        github=github,
        ledger=ledger,
        requirements_agent=RequirementsAgent(
            client=anthropic_client, ledger=ledger
        ),
        bot_login=bot_login,
    )

    app.state.config = config
    app.state.ledger = ledger
    app.state.orchestrator = orchestrator

    @app.get("/health")
    async def health() -> dict[str, str]:
        return {"ok": "true"}

    @app.post("/webhook")
    async def webhook(
        request: Request,
        background_tasks: BackgroundTasks,
        x_hub_signature_256: str | None = Header(default=None),
        x_github_event: str | None = Header(default=None),
    ) -> Response:
        body = await request.body()

        if not verify_webhook_signature(
            config.github_webhook_secret, body, x_hub_signature_256
        ):
            ledger.log(
                "WEBHOOK_UNAUTHORIZED", source_event=x_github_event or "?"
            )
            return Response(status_code=401)

        try:
            payload = json.loads(body.decode("utf-8")) if body else {}
        except json.JSONDecodeError:
            ledger.log("WEBHOOK_BAD_JSON")
            return Response(status_code=400)

        action = payload.get("action", "")
        issue = payload.get("issue") or {}
        issue_number = issue.get("number")
        if not isinstance(issue_number, int):
            ledger.log(
                "WEBHOOK_IGNORED",
                source_event=x_github_event,
                reason="no issue number",
            )
            return Response(status_code=200, content='{"ok":true}')

        label_name: str | None = None
        actor: str | None = None
        comment_body: str | None = None

        if x_github_event == "issues" and action == "labeled":
            label_name = (payload.get("label") or {}).get("name")
            actor = (payload.get("sender") or {}).get("login")
        elif x_github_event == "issue_comment" and action == "created":
            comment = payload.get("comment") or {}
            comment_body = comment.get("body")
            actor = (comment.get("user") or {}).get("login")
        else:
            ledger.log(
                "WEBHOOK_IGNORED",
                issue_number=issue_number,
                source_event=x_github_event,
                action=action,
            )
            return Response(status_code=200, content='{"ok":true}')

        event = WorkItemEvent(
            issue_number=issue_number,
            event_type=x_github_event or "",
            action=action,
            label=label_name,
            actor=actor,
            comment_body=comment_body,
        )
        background_tasks.add_task(safe_handle_event, orchestrator, event)
        return Response(
            status_code=200,
            content='{"ok":true}',
            media_type="application/json",
        )

    return app


app = create_app()
