# infrastructure

Terraform that provisions the GitHub-side pieces the agent
orchestrator depends on:

- 14 workflow labels (`status/*`, `review/*`, `flag/*`).
- A repository webhook pointing at the orchestrator, with HMAC
  secret + JSON payload + `issues`/`issue_comment` events.

The orchestrator itself runs from `agents/issue-resolver/` — this
directory does **not** deploy app code.

## Prerequisites

- Terraform >= 1.5
- A fine-grained GitHub PAT with `admin:webhooks` on the repo plus
  `issues: write`.
- A stable public HTTPS URL for the orchestrator (ngrok reserved
  domain for local dev, or a real host for production).
- A random high-entropy string to use as the webhook HMAC secret.
  Put the **same** value in `agents/issue-resolver/.env` as
  `GITHUB_WEBHOOK_SECRET`.

## First apply

```bash
cd infrastructure
cp terraform.tfvars.example terraform.tfvars
# Edit terraform.tfvars. It is gitignored.
terraform init
terraform plan      # confirm 14 labels + 1 webhook will be created
terraform apply
```

Terraform will print the webhook URL and id when done. GitHub may
show a red ping for a moment until the orchestrator is up and
returning 200 on `/webhook`.

## Label catalogue

| Category | Labels |
|---|---|
| Stage status | `status/triage`, `status/requirements`, `status/design`, `status/plan`, `status/code`, `status/pr-open`, `status/closed`, `status/needs-info` |
| Review | `review/requested`, `review/approved`, `review/changes-requested` |
| Flags | `flag/agent-error`, `flag/revision-limit`, `flag/blocked` |

## Rotating the ngrok URL

Edit `webhook_url` in `terraform.tfvars` and run `terraform apply`.
Terraform updates the webhook in place.

## What is NOT managed here

- `.env` secrets for the orchestrator.
- Application code, deployment, or process supervision.
- The issue template (checked in at
  `.github/ISSUE_TEMPLATE/compatibility_gap.md`).

## Who can trigger the agent?

The agent runs on `labeled` events with `status/triage`. Adding
labels requires write or triage permission on the repo, so random
issue reporters cannot self-trigger. A maintainer adds the label
after a quick sanity check of the filed issue.
