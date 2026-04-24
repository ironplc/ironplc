# Plan: Agentic Issue-Resolver Orchestrator (initial build)

## Goal

Build the first stage of an agentic system that takes compatibility-gap
GitHub issues and walks them through
Requirements → Design → Plan → Code → PR. This task covers:

- A long-running FastAPI webhook orchestrator with a state machine and
  event ledger (`agents/issue-resolver/`).
- The **Requirements** stage agent (only stage implemented; later stages
  stubbed).
- Terraform configuration that provisions the 14 workflow labels and the
  GitHub repository webhook (`infrastructure/`).

This supersedes the existing `agents/compatibility_resolver/`
GitHub-Actions batch script, which is deleted as part of this change.

## Architecture

```
GitHub issue event  ──webhook──▶  FastAPI (main.py)
                                        │  verify HMAC signature
                                        ▼
                                Orchestrator.handle_event
                                        │  state machine
                                        ▼
                         RequirementsAgent  (LLM validate → LLM generate)
                                        │
                                        ▼
                 post comment + transition labels  ──▶  ledger.db
```

- **State** lives in two places: labels on the GitHub issue (source of
  truth for humans) and `ledger.db` (SQLite append-only audit log).
- **Requirement IDs** are emitted as `**REQ-TBD-NNN**` placeholders. The
  future Design stage rewrites the prefix when it picks a target design
  doc (e.g. `REQ-CF-`, `REQ-TH-`).
- **Validation** that an issue has enough info is an LLM call, not a
  header parser — the Requirements agent owns both the validation and
  the generation calls.

## File Map

| File | Change |
|------|--------|
| `agents/issue-resolver/main.py` | FastAPI app + `/webhook` route |
| `agents/issue-resolver/orchestrator.py` | Event routing + state machine |
| `agents/issue-resolver/context.py` | `build_requirements_context` (AI-ready packaging) |
| `agents/issue-resolver/agents/requirements.py` | `RequirementsAgent` with two-pass LLM flow |
| `agents/issue-resolver/github_client.py` | REST client + HMAC signature verification |
| `agents/issue-resolver/ledger.py` | SQLite event ledger (dual-sink stdout + DB) |
| `agents/issue-resolver/models.py` | `Stage`, `BlockReason`, `WorkItem`, `WorkItemEvent` |
| `agents/issue-resolver/config.py` | `.env` loader + `ConfigError` |
| `agents/issue-resolver/requirements.txt` | Pinned dependencies |
| `agents/issue-resolver/.env.example` | Environment template |
| `agents/issue-resolver/README.md` | Local run instructions + how to add a stage |
| `agents/issue-resolver/tests/test_*.py` | 5 test modules |
| `infrastructure/main.tf` | 14 labels + 1 webhook resource |
| `infrastructure/variables.tf` | Variable definitions |
| `infrastructure/outputs.tf` | `webhook_url`, `webhook_id` |
| `infrastructure/terraform.tfvars.example` | Variable template |
| `infrastructure/README.md` | Terraform usage |
| `.gitignore` | Append `terraform.tfvars`, `*.tfstate*`, `.terraform/`, `.env` |
| `agents/justfile` | Replace `compatibility_resolver` recipes with `issue-resolver` |
| `agents/compatibility_resolver/**` | **Delete** (superseded) |
| `.github/workflows/agent-triage.yaml` | **Delete** (invoked the deleted script) |

## Tasks

- [x] Write plan
- [ ] Scaffold `agents/issue-resolver/` package (models, config, ledger)
- [ ] Implement `github_client.py` with HMAC signature verification
- [ ] Implement `context.py` (AI-ready packaging; no header parsing)
- [ ] Implement `agents/requirements.py` with two-pass validate → generate
- [ ] Implement `orchestrator.py` and `main.py`
- [ ] Write unit tests (signature verify, context packaging, orchestrator
      routing with mocked agent, ledger roundtrip)
- [ ] Create `infrastructure/` Terraform files
- [ ] Update `.gitignore` and `agents/justfile`; delete
      `compatibility_resolver/` and its workflow
- [ ] Write READMEs and `.env.example`
- [ ] Run tests, commit, push feature branch

## Out of scope

- Design / Plan / Code stage agents (later stage handlers are orchestrator
  stubs only).
- Persistent `WorkItem` state beyond process memory (the ledger is the
  replay log; re-deriving `WorkItem` from the ledger is a future task).
- Changes to `compiler/**` — none.
