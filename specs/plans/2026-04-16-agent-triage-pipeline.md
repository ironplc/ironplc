# Agent Triage Pipeline — Stage 1

## Context

IronPLC needs an automated pipeline that detects new compatibility-gap issues
and generates requirements documents. This task covers the trigger (issue
template + workflow) and the first stage (requirements generation script).

**Flow:** A user files a compatibility-gap issue (no labels applied). A
maintainer reviews it and manually adds `status/triage`. This triggers a
GitHub Actions workflow that runs a Python script to read the issue, call
Claude, and post a requirements document as a comment.

## Security Model

**Primary control (GitHub-enforced):** Only users with write or triage
permission on the repository can add labels. The workflow fires on
`issues: [labeled]` with a guard on `status/triage`. A random user who opens
an issue cannot add this label — GitHub's permission model prevents it.

**Additional hardening:**
- `permissions` block scoped to minimum needed (`issues: write`, `contents: read`)
- `ANTHROPIC_API_KEY` stored as a repository secret
- The script never echoes or logs secret values
- Workflow only runs on `issues` events (not `pull_request` or
  `pull_request_target`)

## Architecture

1. **Issue template** (`.github/ISSUE_TEMPLATE/compatibility_gap.md`)
2. **GitHub Actions workflow** (`.github/workflows/agent-triage.yaml`)
3. **Python script** (`agents/issue_triage/requirements.py`)

## Label Lifecycle

```
New issue (from template) → no labels
  ↓ maintainer manually adds status/triage
  ↓ workflow fires
  ├── Valid fields   → status/requirements (removes status/triage)
  ├── Missing fields → status/needs-info   (removes status/triage)
  └── Script error   → flag/agent-error    (keeps status/triage for retry)
```
