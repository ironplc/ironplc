# infrastructure

Terraform that provisions the GitHub-side pieces the agent
orchestrator depends on:

- 14 workflow labels (`status/*`, `review/*`, `flag/*`).
- A repository webhook pointing at the orchestrator, with HMAC
  secret + JSON payload + `issues`/`issue_comment` events.

The orchestrator itself runs from `agents/issue-resolver/` — this
directory does **not** deploy app code.

## State

State and plan/apply execution both live in [HCP
Terraform](https://app.terraform.io). The workspace is named
`ironplc-infrastructure` and is created automatically on first
`terraform init`. Local `.tfstate` files are gitignored and not
used.

## Prerequisites

- Terraform >= 1.5
- An HCP Terraform account and organization
- `TF_CLOUD_ORGANIZATION` exported in your shell:

  ```bash
  export TF_CLOUD_ORGANIZATION=your-hcp-org
  ```

- A fine-grained GitHub PAT with `admin:webhooks` on the repo plus
  `issues: write`.
- A stable public HTTPS URL for the orchestrator (ngrok reserved
  domain for local dev, or a real host for production).
- A random high-entropy string to use as the webhook HMAC secret.
  The **same** value must also go in
  `agents/issue-resolver/.env` as `GITHUB_WEBHOOK_SECRET`.

## First apply

```bash
cd infrastructure
terraform login           # browser flow; one-time per machine
terraform init            # creates the workspace in your HCP org
```

Then set the five input variables on the workspace. Two options:

### Option 1 — Remote execution (recommended)

Variables live in HCP. Open
`https://app.terraform.io/app/<your-org>/workspaces/ironplc-infrastructure/variables`
and add each one as a **Terraform variable**:

| Variable | Sensitive? | Example |
|---|---|---|
| `github_token` | ✅ yes | `github_pat_…` |
| `github_owner` | no | `ironplc` |
| `github_repo` | no | `ironplc` |
| `webhook_url` | no | `https://you.ngrok-free.app/webhook` |
| `webhook_secret` | ✅ yes | random high-entropy string |

Then run the plan + apply from your laptop — it executes remotely in
HCP, output streams back to your terminal:

```bash
terraform plan      # 14 labels + 1 webhook the first time
terraform apply
```

### Option 2 — Local execution

If you'd rather keep using a local `terraform.tfvars`, set the
workspace's **Execution Mode** to *Local* in HCP
(Settings → General). Then:

```bash
cp terraform.tfvars.example terraform.tfvars   # gitignored
$EDITOR terraform.tfvars
terraform plan
terraform apply
```

State is still stored remotely in HCP; only the plan/apply runs
locally.

## Label catalogue

| Category | Labels |
|---|---|
| Stage status | `status/triage`, `status/requirements`, `status/design`, `status/plan`, `status/code`, `status/pr-open`, `status/closed`, `status/needs-info` |
| Review | `review/requested`, `review/approved`, `review/changes-requested` |
| Flags | `flag/agent-error`, `flag/revision-limit`, `flag/blocked` |

## Rotating the ngrok URL

Update `webhook_url` (in HCP variables for remote execution, or in
`terraform.tfvars` for local) and run `terraform apply`. Terraform
updates the webhook in place — no destroy/recreate.

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
