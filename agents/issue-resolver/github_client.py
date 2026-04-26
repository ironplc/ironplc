"""Thin synchronous GitHub REST client scoped to what the orchestrator needs.

Only four actions matter at this stage: read an issue and its comments,
post a comment, and manipulate labels. Plus one webhook-side concern:
verifying the ``X-Hub-Signature-256`` HMAC GitHub sends with every
delivery.

Error handling is deliberately small:

- ``GitHubAPIError`` carries the HTTP status and a short body excerpt.
- 5xx responses are retried **once** with a 2 s backoff (GitHub is
  usually back within a second or two; more aggressive retries risk
  duplicate comments).
- 4xx responses never retry — the orchestrator decides whether to treat
  e.g. a 404 on ``remove_label`` as benign.
"""

from __future__ import annotations

import hmac
import sys
import time
from dataclasses import dataclass
from hashlib import sha256

import requests


GITHUB_API = "https://api.github.com"
_RETRY_BACKOFF_SECONDS = 2


class GitHubAPIError(RuntimeError):
    def __init__(self, status: int, body: str) -> None:
        super().__init__(f"GitHub API error {status}: {body[:200]}")
        self.status = status
        self.body = body


@dataclass(frozen=True)
class GitHubClient:
    token: str
    repo: str

    def _headers(self) -> dict[str, str]:
        return {
            "Authorization": f"token {self.token}",
            "Accept": "application/vnd.github+json",
            "X-GitHub-Api-Version": "2022-11-28",
        }

    def _request(
        self,
        method: str,
        path: str,
        *,
        json_body: dict | None = None,
        allow_404: bool = False,
    ) -> requests.Response:
        url = f"{GITHUB_API}{path}"
        for attempt in (1, 2):
            resp = requests.request(
                method,
                url,
                headers=self._headers(),
                json=json_body,
                timeout=30,
            )
            if 500 <= resp.status_code < 600 and attempt == 1:
                time.sleep(_RETRY_BACKOFF_SECONDS)
                continue
            break

        if allow_404 and resp.status_code == 404:
            return resp
        if resp.status_code >= 400:
            raise GitHubAPIError(resp.status_code, resp.text)
        return resp

    def get_issue(self, issue_number: int) -> dict:
        resp = self._request("GET", f"/repos/{self.repo}/issues/{issue_number}")
        return resp.json()

    def get_issue_comments(self, issue_number: int) -> list[dict]:
        resp = self._request(
            "GET", f"/repos/{self.repo}/issues/{issue_number}/comments"
        )
        return resp.json()

    def post_comment(self, issue_number: int, body: str) -> dict:
        resp = self._request(
            "POST",
            f"/repos/{self.repo}/issues/{issue_number}/comments",
            json_body={"body": body},
        )
        return resp.json()

    def add_label(self, issue_number: int, label: str) -> None:
        self._request(
            "POST",
            f"/repos/{self.repo}/issues/{issue_number}/labels",
            json_body={"labels": [label]},
        )

    def remove_label(self, issue_number: int, label: str) -> None:
        self._request(
            "DELETE",
            f"/repos/{self.repo}/issues/{issue_number}/labels/{label}",
            allow_404=True,
        )

    def get_labels(self, issue_number: int) -> list[str]:
        resp = self._request(
            "GET", f"/repos/{self.repo}/issues/{issue_number}/labels"
        )
        return [item["name"] for item in resp.json()]

    def get_authenticated_user_login(self) -> str:
        resp = self._request("GET", "/user")
        return resp.json()["login"]


@dataclass(frozen=True)
class DryRunGitHubClient:
    """Wrapper that delegates reads but turns writes into stdout logs.

    Lets you point the orchestrator at a real repo with a read-only PAT
    and watch what it *would* post without touching the issue. Reads
    (``get_issue``, ``get_issue_comments``, ``get_labels``,
    ``get_authenticated_user_login``) hit the real API; writes
    (``post_comment``, ``add_label``, ``remove_label``) print and no-op.
    """

    inner: GitHubClient

    def get_issue(self, issue_number: int) -> dict:
        return self.inner.get_issue(issue_number)

    def get_issue_comments(self, issue_number: int) -> list[dict]:
        return self.inner.get_issue_comments(issue_number)

    def get_labels(self, issue_number: int) -> list[str]:
        return self.inner.get_labels(issue_number)

    def get_authenticated_user_login(self) -> str:
        return self.inner.get_authenticated_user_login()

    def post_comment(self, issue_number: int, body: str) -> dict:
        print(
            f"\n[DRY RUN] post_comment(#{issue_number}):\n{body}\n",
            file=sys.stdout,
            flush=True,
        )
        return {"id": 0}

    def add_label(self, issue_number: int, label: str) -> None:
        print(
            f"[DRY RUN] add_label(#{issue_number}, {label!r})",
            file=sys.stdout,
            flush=True,
        )

    def remove_label(self, issue_number: int, label: str) -> None:
        print(
            f"[DRY RUN] remove_label(#{issue_number}, {label!r})",
            file=sys.stdout,
            flush=True,
        )


def verify_webhook_signature(
    secret: str, body: bytes, signature_header: str | None
) -> bool:
    """Constant-time verification of the ``X-Hub-Signature-256`` header.

    Returns True iff the header is present, correctly formatted, and
    matches the HMAC-SHA256 of ``body`` under ``secret``.
    """
    if not signature_header or not signature_header.startswith("sha256="):
        return False
    expected = hmac.new(
        secret.encode("utf-8"), msg=body, digestmod=sha256
    ).hexdigest()
    provided = signature_header[len("sha256="):]
    return hmac.compare_digest(expected, provided)
