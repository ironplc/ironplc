# infrastructure

Terraform that provisions the GitHub-side workflow labels:

- 14 workflow labels (`status/*`, `review/*`, `flag/*`).

This directory does **not** deploy app code.

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

- A fine-grained GitHub PAT with `issues: write` on the repo.

## First apply

```bash
cd infrastructure
terraform login           # browser flow; one-time per machine
terraform init            # creates the workspace in your HCP org
```

Then set the three input variables on the workspace. Two options:

### Option 1 — Remote execution (recommended)

Variables live in HCP. Open
`https://app.terraform.io/app/<your-org>/workspaces/ironplc-infrastructure/variables`
and add each one as a **Terraform variable**:

| Variable | Sensitive? | Example |
|---|---|---|
| `github_token` | ✅ yes | `github_pat_…` |
| `github_owner` | no | `ironplc` |
| `github_repo` | no | `ironplc` |

Then run the plan + apply from your laptop — it executes remotely in
HCP, output streams back to your terminal:

```bash
terraform plan      # 14 labels the first time
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

## What is NOT managed here

- Application code, deployment, or process supervision.
- The issue template (checked in at
  `.github/ISSUE_TEMPLATE/compatibility_gap.md`).

## Who can apply these labels?

Adding labels requires write or triage permission on the repo, so
random issue reporters cannot apply them. A maintainer adds the
relevant label after a quick sanity check of the filed issue.
