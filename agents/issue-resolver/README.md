# issue-resolver

A long-running FastAPI webhook orchestrator that drives
compatibility-gap GitHub issues through
**Requirements → Design → Plan → Code → PR**. This first build ships
the Requirements stage; later stages are stubbed in
`orchestrator.py` and will be filled in separately.

This directory supersedes the earlier
`agents/compatibility_resolver/` GitHub-Actions one-shot script.

## Architecture at a glance

```
GitHub issue event  ──webhook──▶  FastAPI (main.py)
                                        │  verify HMAC
                                        ▼
                                Orchestrator.handle_event
                                        │  state machine
                                        ▼
                         RequirementsAgent  (LLM validate → LLM generate)
                                        │
                                        ▼
                 post comment + transition labels  ──▶  ledger.db
```

- **Labels** on the issue are the human-facing source of truth.
- **`ledger.db`** (SQLite) is an append-only audit log of every
  webhook, LLM call, comment, and label change. LLM prompts are
  hashed, never stored.
- **`WorkItem` state** is in-memory for now — re-deriving it from the
  ledger after a restart is a future task.

## Prerequisites

- Python 3.11+
- An [ngrok reserved domain](https://dashboard.ngrok.com/domains) or
  other stable public HTTPS endpoint (GitHub needs a stable URL).
- A fine-grained GitHub PAT with `issues: write` on `ironplc/ironplc`.
- An Anthropic API key.
- Terraform has already been applied from the sibling
  `infrastructure/` directory (creates the 14 labels and the webhook).

## Setup

```bash
cd agents/issue-resolver
cp .env.example .env
# Fill in ANTHROPIC_API_KEY, GITHUB_TOKEN, GITHUB_WEBHOOK_SECRET.
# GITHUB_WEBHOOK_SECRET must match infrastructure/terraform.tfvars.
pip install -r requirements.txt
```

## Run locally

Two terminals:

```bash
# Terminal A: the app
cd agents/issue-resolver
uvicorn main:app --port 8000 --reload

# Terminal B: the tunnel
ngrok http --domain your-reserved-name.ngrok-free.app 8000
```

Point the webhook (managed by Terraform) at
`https://your-reserved-name.ngrok-free.app/webhook`.

## End-to-end smoke test

1. File a new issue using the *Compatibility Gap* template (see
   `.github/ISSUE_TEMPLATE/compatibility_gap.md`).
2. A maintainer adds the `status/triage` label. (The template does
   not auto-apply labels — only users with write/triage permission can
   do this, which is our spam gate.)
3. Within ~30 s the app should log:

   ```
   ...  #123  triage       WEBHOOK_RECEIVED  ...
   ...  #123  requirements AGENT_DISPATCH
   ...  #123  requirements LLM_RESPONSE     {"phase": "validate", ...}
   ...  #123  requirements LLM_RESPONSE     {"phase": "generate", ...}
   ...  #123  requirements COMMENT_POSTED
   ...  #123  requirements LABEL_TRANSITION
   ```

4. The issue now has a draft requirements comment and the
   `status/requirements` label.

## Outcomes the orchestrator handles

| Situation | Comment | Label change | Blocked reason |
|---|---|---|---|
| Happy path | Auto-generated requirements doc | `+status/requirements`, `-status/triage` | — |
| Issue missing info | "What's missing: …" | `+status/needs-info`, `-status/triage` | `NEEDS_INFO` |
| Agent/API failure | Error notice | `+flag/agent-error` | `AGENT_ERROR` |
| 4th retry at one stage | Revision-limit notice | `+flag/revision-limit` | `REVISION_LIMIT` |

## Tests

```bash
cd agents/issue-resolver
python -m pytest tests/ -v
```

Tests cover signature verification, context packaging, ledger
roundtrip, and orchestrator routing (agent mocked). They do **not**
touch the LLM or GitHub — those are tested by the smoke test above.

## Adding a new stage

1. **Context builder** — replace the `NotImplementedError` stub in
   `context.py` (e.g. `build_design_context`) with the fields the new
   stage needs from the issue, prior comments, and prior stage
   artifacts.
2. **Agent** — add a module under `agents/` with a class exposing an
   `async run(context, work_item)` method. Follow the two-pass
   validate-then-generate pattern in `agents/requirements.py` if the
   stage has its own preconditions. Log each LLM call via
   `ledger.log_llm_call`.
3. **Routing** — extend `Orchestrator._on_labeled` / `handle_event`
   with the new branch. Decide which label triggers it and which
   labels it transitions to. Route `IncompleteIssueError` and
   `AgentError` through the existing handlers.
4. **Labels** — if you introduce new labels, add them to
   `infrastructure/main.tf`.
5. **Tests** — add an orchestrator-level routing test with the new
   agent mocked, plus any agent-internal logic tests.

## Relationship to `infrastructure/`

`infrastructure/` owns the 14 GitHub labels and the repository
webhook. This directory owns the runtime app behind the webhook.
Secrets (`.env`, `terraform.tfvars`) are **not** managed by either —
keep them out of version control.
