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
- [`just`](https://github.com/casey/just) (used to drive every
  workflow in this directory).
- A fine-grained GitHub PAT with `issues: write` on `ironplc/ironplc`.
- An Anthropic API key.
- For full end-to-end testing only: an
  [ngrok reserved domain](https://dashboard.ngrok.com/domains) (or
  other stable public HTTPS endpoint) and Terraform applied from the
  sibling `infrastructure/` directory (creates the 14 labels and the
  webhook).

## Setup

The only manual step is creating `.env` — `just` will not overwrite
it. Copy the template and fill in your values:

```bash
cd agents/issue-resolver
cp .env.example .env
# Fill in ANTHROPIC_API_KEY, GITHUB_TOKEN, GITHUB_WEBHOOK_SECRET.
# GITHUB_WEBHOOK_SECRET must match infrastructure/terraform.tfvars.
```

For local smoke testing without real credentials, any non-empty
string works for each of the three secrets — the Anthropic SDK and
GitHub client don't validate at construction. Calls that actually
hit the APIs will fail, which is the expected behavior.

Then create the venv and install dependencies:

```bash
just setup
```

## Available recipes

| Recipe | What it does |
|---|---|
| `just` (or `just ci`) | `setup` then `test` |
| `just setup` | Create `.venv` and install deps (idempotent) |
| `just test` | Run the unit test suite |
| `just serve` | Start `uvicorn` on port 8000 with hot-reload |
| `just serve-dryrun` | Like `serve`, but comment posts and label changes are printed to stdout instead of sent to GitHub |
| `just health` | `GET /health` against a running `serve` |
| `just webhook` | Send a properly-signed webhook (default action `opened`, ignored by orchestrator) |
| `just webhook-bogus` | Send a webhook with an invalid signature; expect 401 |
| `just ledger` | Tail the 20 most recent `ledger.db` rows |

All recipes auto-load `.env`, so you don't have to export anything.

## Local smoke test (no GitHub, no LLM)

Two terminals:

```bash
# Terminal A
cd agents/issue-resolver
just serve

# Terminal B
cd agents/issue-resolver
just health         # → 200 {"ok":"true"}
just webhook-bogus  # → 401, ledger row WEBHOOK_UNAUTHORIZED
just webhook        # → 200, ledger row WEBHOOK_IGNORED (action "opened")
just ledger         # see what got written
```

Drive the orchestrator into the requirements stage (will fail at the
LLM call without a real Anthropic key, which is fine):

```bash
just webhook ACTION=labeled LABEL=status/triage NUMBER=42
```

## Simulate a real issue without writing back (dry run)

Useful when you want to see how the orchestrator handles a real issue
that already exists in GitHub, without it actually posting comments or
moving labels. Reads still hit real GitHub, so use a fine-grained PAT
with **read-only** access (`metadata: read`, `issues: read`) as a
safety net in case the wrapper is bypassed.

```bash
# .env: GITHUB_REPO points at the real repo, GITHUB_TOKEN is read-only.
just serve-dryrun

# In another terminal, fabricate the trigger for a real issue number.
just webhook ACTION=labeled LABEL=status/triage NUMBER=<real issue #>
```

The would-be comment body and label changes print to the `serve-dryrun`
console, prefixed with `[DRY RUN]`. The ledger still records
`COMMENT_POSTED` and `LABEL_TRANSITION` events.

## Full end-to-end (requires real credentials + ngrok + Terraform)

```bash
# Terminal A: the app
just serve

# Terminal B: expose it on a public HTTPS URL
ngrok http --domain your-reserved-name.ngrok-free.app 8000
```

Point the webhook (managed by Terraform) at
`https://your-reserved-name.ngrok-free.app/webhook`. Then:

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
just test
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
