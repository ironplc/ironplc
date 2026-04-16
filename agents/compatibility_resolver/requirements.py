"""Generate requirements from a compatibility-gap GitHub issue.

Reads an issue via the GitHub API, validates it has enough information
using Claude, then generates a structured requirements document and
posts it as an issue comment.

Designed to run as a GitHub Actions step. Required env vars:
  GITHUB_TOKEN, ANTHROPIC_API_KEY, ISSUE_NUMBER
Optional:
  GITHUB_REPOSITORY (defaults to ironplc/ironplc)
"""

import json
import os
import sys

import anthropic
import requests

GITHUB_API = "https://api.github.com"
MODEL = "claude-sonnet-4-20250514"

LABEL_COLORS = {
    "status/requirements": "0e8a16",  # green
    "status/needs-info": "fbca04",    # yellow
    "flag/agent-error": "d93f0b",     # red
}


# ---------------------------------------------------------------------------
# GitHub helpers
# ---------------------------------------------------------------------------

def fetch_issue(repo, issue_number, token):
    """Fetch issue title and body from GitHub."""
    url = f"{GITHUB_API}/repos/{repo}/issues/{issue_number}"
    resp = requests.get(url, headers=_gh_headers(token), timeout=30)
    resp.raise_for_status()
    data = resp.json()
    return data["title"], data.get("body") or ""


def post_comment(repo, issue_number, body, token):
    """Post a comment on a GitHub issue."""
    url = f"{GITHUB_API}/repos/{repo}/issues/{issue_number}/comments"
    resp = requests.post(
        url,
        headers=_gh_headers(token),
        json={"body": body},
        timeout=30,
    )
    resp.raise_for_status()


def add_label(repo, issue_number, label, token):
    """Add a label to an issue, creating it first if needed."""
    ensure_label_exists(repo, label, token)
    url = f"{GITHUB_API}/repos/{repo}/issues/{issue_number}/labels"
    resp = requests.post(
        url,
        headers=_gh_headers(token),
        json={"labels": [label]},
        timeout=30,
    )
    resp.raise_for_status()


def remove_label(repo, issue_number, label, token):
    """Remove a label from an issue (ignores 404)."""
    url = f"{GITHUB_API}/repos/{repo}/issues/{issue_number}/labels/{label}"
    resp = requests.delete(url, headers=_gh_headers(token), timeout=30)
    if resp.status_code != 404:
        resp.raise_for_status()


def ensure_label_exists(repo, label, token):
    """Create a label if it doesn't already exist."""
    url = f"{GITHUB_API}/repos/{repo}/labels/{label}"
    resp = requests.get(url, headers=_gh_headers(token), timeout=30)
    if resp.status_code == 200:
        return
    color = LABEL_COLORS.get(label, "ededed")
    requests.post(
        f"{GITHUB_API}/repos/{repo}/labels",
        headers=_gh_headers(token),
        json={"name": label, "color": color},
        timeout=30,
    ).raise_for_status()


def _gh_headers(token):
    return {
        "Authorization": f"token {token}",
        "Accept": "application/vnd.github.v3+json",
    }


# ---------------------------------------------------------------------------
# Prompt builders
# ---------------------------------------------------------------------------

VALIDATION_SYSTEM = """\
You are a triage assistant for the IronPLC compiler project. Your job is to
assess whether a GitHub issue contains enough information to generate
requirements.

An issue has enough information when it contains ALL of the following:
1. An ST (Structured Text) program or code snippet
2. A description of what currently goes wrong (the actual behavior)
3. A description of what should happen instead (the expected behavior)

The issue does NOT need to follow any particular template format. Assess the
substance, not the structure.

Respond with a JSON object and nothing else:
{"sufficient": true}
or
{"sufficient": false, "missing": "Brief explanation of what is missing"}
"""


def build_validation_prompt(title, body):
    """Build the user message for the validation LLM call."""
    parts = [f"Issue title: {title}"]
    if body.strip():
        parts.append(f"Issue body:\n{body}")
    return "\n\n".join(parts)


def parse_validation_response(text):
    """Parse the JSON validation response from the LLM.

    Returns (sufficient: bool, missing: str | None).
    """
    try:
        data = json.loads(text.strip())
        sufficient = data.get("sufficient", False)
        missing = data.get("missing")
        return bool(sufficient), missing
    except (json.JSONDecodeError, AttributeError):
        return False, "Unable to parse validation response"


REQUIREMENTS_SYSTEM = """\
You are a requirements engineer for the IronPLC compiler project. Given a
compatibility-gap issue, produce a structured requirements document.

Format each requirement as:
**REQ-CG-NNN** The system SHALL ...

Where NNN is a sequential number starting at 001. Requirements should be
specific, testable, and traceable to the issue.

After listing all requirements, add an "Open Questions" section listing any
ambiguities or decisions that need maintainer input.

Do not include any preamble — start directly with the first requirement.
"""


def build_requirements_prompt(title, body):
    """Build the messages for the requirements generation LLM call."""
    user_content = f"Issue title: {title}\n\nIssue body:\n{body}"
    return user_content


# ---------------------------------------------------------------------------
# Comment formatting
# ---------------------------------------------------------------------------

def format_comment(content):
    """Wrap generated requirements in a comment with disclaimer.

    If the content doesn't already contain an Open Questions section,
    append a placeholder one.
    """
    header = (
        "> **Auto-generated requirements** — review and edit before "
        "accepting. This was produced by an AI assistant and may contain "
        "errors.\n\n"
    )
    result = header + content.strip()
    if "open questions" not in content.lower():
        result += "\n\n## Open Questions\n\n- _None identified — please verify._"
    return result


# ---------------------------------------------------------------------------
# Main pipeline
# ---------------------------------------------------------------------------

def main():
    token = os.environ["GITHUB_TOKEN"]
    api_key = os.environ["ANTHROPIC_API_KEY"]
    issue_number = os.environ["ISSUE_NUMBER"]
    repo = os.environ.get("GITHUB_REPOSITORY", "ironplc/ironplc")

    client = anthropic.Anthropic(api_key=api_key)

    try:
        title, body = fetch_issue(repo, issue_number, token)

        # Stage 1: Validate issue has enough information
        validation_user = build_validation_prompt(title, body)
        validation_resp = client.messages.create(
            model=MODEL,
            max_tokens=500,
            system=VALIDATION_SYSTEM,
            messages=[{"role": "user", "content": validation_user}],
        )
        validation_text = validation_resp.content[0].text
        sufficient, missing = parse_validation_response(validation_text)

        if not sufficient:
            comment = (
                "This issue doesn't have enough information to generate "
                f"requirements yet.\n\n**What's missing:** {missing}\n\n"
                "Please update the issue with the missing details and a "
                "maintainer will re-trigger triage."
            )
            post_comment(repo, issue_number, comment, token)
            add_label(repo, issue_number, "status/needs-info", token)
            remove_label(repo, issue_number, "status/triage", token)
            return

        # Stage 2: Generate requirements
        req_user = build_requirements_prompt(title, body)
        req_resp = client.messages.create(
            model=MODEL,
            max_tokens=2000,
            system=REQUIREMENTS_SYSTEM,
            messages=[{"role": "user", "content": req_user}],
        )
        requirements_text = req_resp.content[0].text

        comment = format_comment(requirements_text)
        post_comment(repo, issue_number, comment, token)
        add_label(repo, issue_number, "status/requirements", token)
        remove_label(repo, issue_number, "status/triage", token)

    except Exception as exc:
        print(f"Error in requirements pipeline: {exc}", file=sys.stderr)
        try:
            post_comment(
                repo,
                issue_number,
                f"⚠️ Requirements generation failed: `{exc}`\n\n"
                "A maintainer should investigate and retry by re-adding "
                "the `status/triage` label.",
                token,
            )
            add_label(repo, issue_number, "flag/agent-error", token)
        except Exception as label_exc:
            print(f"Failed to post error comment: {label_exc}", file=sys.stderr)


if __name__ == "__main__":
    main()
