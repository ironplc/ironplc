# Plan: Set the PostHog default (primary) dashboard from Terraform

## Goal

Make the "IronPLC — Adoption & Success" dashboard the project's **default
(primary) dashboard** — the one PostHog shows on the project home — through
Terraform, so this setting is version-controlled alongside the dashboard
definition instead of being a manual UI click.

## Problem

The official `PostHog/posthog` Terraform provider does **not** expose the
primary-dashboard setting. Its `posthog_project` resource only manages `name`,
`organization_id`, and `timezone`; there is no `primary_dashboard` attribute
and no project-settings resource that carries it.

The PostHog REST API *does* support it:

```
PATCH {host}/api/projects/{project_id}/
{ "primary_dashboard": <dashboard_id> }
```

## Approach

Add a `null_resource` in `infrastructure/posthog.tf` whose `local-exec`
provisioner issues that PATCH, wiring `posthog_dashboard.adoption.id` into the
body. The provisioner reuses the credentials the module already has
(`posthog_api_key`, `posthog_project_id`, `posthog_host`) — no new variables.

- `triggers` include the dashboard id, project id, and host, so the PATCH
  re-runs whenever any of them change (and on first apply).
- The API key is passed via a redacted `environment` entry, never interpolated
  into the command string, so it does not leak into run logs.
- `curl --fail` makes an HTTP error fail the apply.

This adds the `hashicorp/null` provider to `required_providers`.

## Limitations (documented in README)

- **Enforce, not reconcile.** There is no read/data-source for
  `primary_dashboard`, so Terraform does not detect drift. A manual change in
  the UI is only corrected on the next apply whose trigger changes. Force a
  re-run with `terraform apply -replace=null_resource.primary_dashboard`.
- **API key scope.** PATCHing the project needs the personal API key to also
  carry `project:write` (in addition to `insight:write` + `dashboard:write`).
- **Runner needs `curl`.** HCP Terraform's remote execution image ships curl;
  local execution mode needs it on PATH.

## Scope

- `infrastructure/posthog.tf` — the `null_resource` + explanatory header.
- `infrastructure/main.tf` — add the `null` provider.
- `infrastructure/README.md` — document the behavior, the extra key scope, and
  the drift caveat.

Out of scope: any change to dashboards/insights themselves, and any new
input variable.
