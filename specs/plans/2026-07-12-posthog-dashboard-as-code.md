# Plan: PostHog "Adoption & Success" dashboard as code

## Goal

Manage the IronPLC product-analytics dashboard as version-controlled code instead
of hand-built UI clicks, so the "Adoption & Success" dashboard (acquisition →
interest → activation → retention, plus product-health tiles) is reproducible and
reviewable. This turns the pageview and playground event streams already flowing
into PostHog into a coherent set of insights.

## Approach

**Extend the existing `infrastructure/` Terraform module** rather than create a
new one. That module already runs on HCP Terraform (workspace
`ironplc-infrastructure`, remote state + remote execution) and manages the GitHub
workflow labels via the `integrations/github` provider. Adding PostHog is one more
provider in the same module:

- Add the official `PostHog/posthog` provider to `required_providers` and a
  `provider "posthog"` block.
- Add a `posthog_dashboard` resource plus one `posthog_insight` per tile, linked
  via `dashboard_ids`. Each insight's `query_json` is the raw PostHog query node
  (`InsightVizNode` wrapping `TrendsQuery` / `FunnelsQuery` / `RetentionQuery`).

Terraform (not OpenTofu) because HCP Terraform is already wired up; reusing it is
simpler than a parallel toolchain and state.

## Scope

In scope (all inside `infrastructure/`):
- `main.tf` — add the posthog provider + provider block.
- `variables.tf` — `posthog_api_key` (sensitive), `posthog_project_id`,
  `posthog_host`.
- `posthog.tf` — the dashboard and one insight per **data-available-today** tile.
- `terraform.tfvars.example` and `README.md` — document the new workspace
  variables and the dashboard.

Out of scope (documented as stubs / follow-ups):
- The install-adoption tiles (`install_completed`, `release_downloads`, Open VSX)
  — they depend on collectors not yet built (Tier 1 / Tier 2). Left as commented
  stubs.
- Any change to the PostHog SDK init files (`posthog-init.js`) — untouched.

## Credentials & safety

- Requires a **personal API key** with `insight:write` + `dashboard:write`
  scopes, supplied as the **sensitive HCP workspace variable** `posthog_api_key`
  — the same pattern the module already uses for `github_token`. Never committed.
  The public `phc_…` ingestion key cannot create dashboards.
- API host is the app host (`https://us.posthog.com`), not the ingestion host.
- The dashboard/insight *definitions* are public (they live in the public repo),
  which is fine: they reveal which metrics are tracked, not any data and no
  secrets.

## Tiles (data available today)

Acquisition: weekly visitors; top pages; traffic sources.
Interest: install-page reach; playground reach.
Activation: successful compiles; compile success rate (formula); programs run.
Health: broken docs examples (by `host_page`); top error codes; dialect
adoption; example popularity.
Funnel: `$pageview` → `playground_loaded` → `compile_finished{success}` →
`run_started`.
Retention: weekly retention on compiles.

## Validation

The `query_json` payloads follow PostHog's query-node schema, but exact field
values (notably boolean property filters, `breakdownFilter` shape, and display
enums) can vary by PostHog version. This module cannot be applied from CI (no
personal API key here), so it is validated by running `terraform plan` /
`terraform apply` against the live project and adjusting any field the API
rejects. The change is purely additive (a new dashboard + insights); it does not
touch the existing GitHub-label resources, and `terraform plan` shows exactly
what will be created before apply. No IronPLC build/CI step is affected (the
module lives outside `compiler/`, `docs/`, and `playground/`).
