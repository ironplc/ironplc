# infrastructure

Terraform that provisions:

- 14 GitHub workflow labels (`status/*`, `review/*`, `flag/*`) — `main.tf`.
- The PostHog **"IronPLC — Adoption & Success"** product-analytics dashboard
  and its insights, and the project setting that makes it the **default
  (primary) dashboard** — `posthog.tf`.

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
- A PostHog **personal API key** with `insight:write` + `dashboard:write` +
  `project:write` scopes (Settings → Personal API keys). This is *not* the
  public `phc_…` ingestion key — that one cannot create dashboards.
  `project:write` is what lets Terraform set the default dashboard (see below);
  without it dashboards/insights still apply but that PATCH is rejected.

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
| `posthog_api_key` | ✅ yes | `phx_…` (personal key) |
| `posthog_project_id` | no | `12345` |
| `posthog_host` | no | `https://us.posthog.com` (default) |

Then run the plan + apply from your laptop — it executes remotely in
HCP, output streams back to your terminal:

```bash
terraform plan      # 14 labels + 1 dashboard + its insights the first time
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

## PostHog dashboard

`posthog.tf` defines the **"IronPLC — Adoption & Success"** dashboard and one
insight per tile, using only events already flowing into PostHog (website
`$pageview` and the playground's `compile_finished` / `run_started` /
`example_loaded` / … events). Tiles are grouped as acquisition, interest,
activation, product-health, a visitor→success funnel, and compile retention.
Product-health includes a **Compile success rate** tile (successful compiles ÷
all compiles) and a **Top compile error codes** tile that ranks the diagnostic
codes of failed compiles — the latter surfaces *why* compiles fail using only
the `error_codes` property, never the program source.

Install-adoption tiles (`install_completed`, `release_downloads`, Open VSX)
are left as commented stubs at the bottom of `posthog.tf`; they light up once
the collectors that emit those events exist.

### Default (primary) dashboard

`posthog.tf` also makes this dashboard the project's **default** — the one
PostHog opens on the project home. The Terraform provider does not expose that
setting, so a `null_resource` (`null_resource.primary_dashboard`) PATCHes
`/api/projects/{id}/` with `{"primary_dashboard": <id>}` using the same
credentials. Two things to know:

- **It enforces but does not reconcile.** There is no read for this field, so
  Terraform can't detect drift. The PATCH re-runs only when the dashboard id,
  project id, or host changes. To re-assert it after a manual change in the UI:

  ```bash
  terraform apply -replace=null_resource.primary_dashboard
  ```

- It runs `curl` on the executing runner (present on HCP's remote image; needs
  to be on `PATH` for local execution mode).

Each insight's `query_json` is the raw PostHog query node. Exact field values
(boolean property filters, `breakdownFilter` shape, display enums) can vary by
PostHog version, so if `terraform plan`/`apply` reports a rejected query,
adjust the offending field and re-apply — the resources are additive and do
not affect the GitHub labels.

## What is NOT managed here

- Application code, deployment, or process supervision.
- The issue template (checked in at
  `.github/ISSUE_TEMPLATE/compatibility_gap.md`).

## Who can apply these labels?

Adding labels requires write or triage permission on the repo, so
random issue reporters cannot apply them. A maintainer adds the
relevant label after a quick sanity check of the filed issue.
