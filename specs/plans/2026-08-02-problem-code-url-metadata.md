# Plan: Problem-code URL channel metadata + PostHog reach dashboard

## Context

When a user follows a problem-code link (e.g. `P0001`) the client appends the
current version to the URL (`?version=<v>`) so `docs/_static/version-check.js`
can show an "out-of-date" banner. Those URLs carry no signal about *where* the
user came from, and the query strings make PostHog treat each variant as a
distinct page.

This plan adds channel metadata to every problem-code URL and a version-control
managed PostHog dashboard that turns the resulting `$pageview` stream into an
adoption-by-channel picture — without any new client telemetry.

## The URL scheme

Every problem-code URL gains a single plain `channel` parameter (in addition to
the existing, untouched `version`, `file`, and `line`):

```
…/problems/<CODE>.html?version=<v>&channel=<channel>[&file=…&line=…]
```

- `version` — unchanged; still the only param `version-check.js` reads.
- `channel` — the origin: `playground`, `cli`, `extension`, or `mcp`.
- `file` / `line` — kept as-is on the LSP and CLI paths. They carry the Rust
  source location that raised the diagnostic, which is how a maintainer sees what
  a remote user hit.

Plain names (`channel`, `version`) are chosen over `utm_source`/`utm_medium`/
`utm_campaign` deliberately: utm_* names read as marketing tracking (users
hesitate to click them) and are stripped by ad-blockers (losing data). PostHog
still captures `channel` and `version` natively because
`docs/_static/posthog-init.js` lists them in `custom_campaign_params`, which
extends PostHog's campaign-param set — so they become event properties and
`$initial_*` person properties exactly like utm_* would, with no per-insight
mapping. No `utm_medium` marker is needed: the presence of `channel` already
means the arrival came from a client link (organic navigation has none). No
`utm_campaign` mirror is needed: `version` is captured directly.

The apparent-unique-page problem is handled on the dashboard side by breaking
down on low-cardinality properties (`channel`, `version`, `$pathname`), not the
full URL, so `version`/`file`/`line` never fragment the tiles.

### Channel taxonomy

| Channel      | Emission site                                                        |
|--------------|----------------------------------------------------------------------|
| `playground` | `playground/src/app.ts` diagnostic chips                             |
| `cli`        | `compiler/ironplc-cli/src/cli.rs` terminal diagnostic notes (new)    |
| `extension`  | LSP compiler diagnostics **and** the extension's own E-code help     |
| `mcp`        | `compiler/mcp` `explain_diagnostic` response `doc_url` (new)         |

Both editor emission sites use `extension` (not `lsp`/`vscode`): the code is
surfaced through the editor integration, and we cannot assume the editor is VS
Code.

## Code changes

1. **`compiler/ironplc-cli/src/lsp_project.rs`** (`map_diagnostic`, ~line 659) —
   append `&channel=extension` to the existing URL, before the conditional
   `&file`/`&line`.
2. **`compiler/ironplc-cli/src/cli.rs`** (`map_diagnostic`, ~line 360) — build
   the same URL (with `channel=cli`, plus `&file`/`&line` when present) and
   append it as a trailing `Learn more: <url>` note, so CLI diagnostics carry a
   clickable link. New `section_for_code` + `problem_help_url` helpers.
3. **`integrations/vscode/src/problemUrl.ts`** (new, pure, no `vscode` import) —
   `problemHelpUrl(code, version)` deriving section from the code prefix
   (`E→editor`, `P→compiler`, `V→runtime`) and emitting `channel=extension`.
   `extension.ts` imports it in `openProblemInBrowser`.
4. **`playground/src/app.ts`** (`renderDiagnostics`, ~line 998) — append
   `&channel=playground`.
5. **`compiler/mcp/src/tools/explain_diagnostic.rs`** — add a `doc_url` field to
   `ExplainDiagnosticResponse`, populated for found codes with `channel=mcp`;
   new `section_for_code` helper.
6. **`docs/_static/posthog-init.js`** — add `custom_campaign_params: ['channel',
   'version']` so PostHog captures the two plain params as event properties.

The section mapping lives once, as `ironplc_dsl::diagnostic::docs_section`
(`P→compiler`, `V→runtime`, `E→editor`, else `unknown`), shared by `cli.rs` and
`explain_diagnostic.rs`. It returns `unknown` rather than defaulting to a real
section, so a new code family produces an honest 404 instead of a wrong page,
and a docs-tree-walking test (`docs_section_covers_every_documented_code`, mirrored
in the TS `problemUrls` suite) fails if any documented code's prefix is left
unmapped. The LSP path keeps its hardcoded `compiler` section (its diagnostics
are P-codes) to avoid widening scope.

## Tests

- `lsp_project.rs`: extend the two existing URL tests to assert `channel=extension`.
- `cli.rs`: new test — `map_diagnostic` notes contain the URL with `channel=cli`.
- `problemUrls.test.ts`: new unit test for `problemHelpUrl()` (asserts `channel`,
  no `utm_`).
- `playground/tests/e2e.spec.ts`: update the href regex to require `channel=playground`.
- `explain_diagnostic.rs`: new test — `doc_url` contains `channel=mcp`.

## PostHog dashboard (as code)

New `infrastructure/posthog-problem-code.tf` — a `posthog_dashboard`
"IronPLC — Problem-code reach" plus one `posthog_insight` per tile. Tiles filter
`$pageview` where `$pathname` contains `/problems/`; the `channel` breakdown then
splits client-link arrivals from organic (the null bucket):

1. Total problem-code arrivals (BoldNumber).
2. Reach by channel — breakdown `channel` (table). Headline adoption tile.
3. Channel trend — weekly unique visitors, breakdown `channel` (line).
4. Top problem codes — breakdown `$pathname` (bar).
5. Version freshness — breakdown `version`, scoped to arrivals with a `version`
   (table).
6. Referrers to problem pages — breakdown `$referring_domain` (table).

Follows the existing `infrastructure/posthog.tf` pattern (raw query-node
`query_json` via `jsonencode`). Cannot be `terraform apply`-ed from CI (no
personal API key here, same as the existing module); HCP Terraform applies it.
Definitions are public and reveal only which metrics are tracked, no data.

## Validation

- `cd compiler && just` (compile, coverage, clippy, fmt).
- `cd playground && just ci` (or the playground test target) for the e2e change.
- `cd integrations/vscode` unit tests for the new pure module.
- `terraform fmt`/`validate` on `infrastructure/` if the toolchain is available.

## Out of scope

- Fixing the LSP path's hardcoded `compiler` section for non-P codes.
- Any change to `version-check.js` (still reads `version`).
- CLI/MCP do not themselves emit `$pageview`; they only surface the URL. Data
  flows only when a human follows the link, and only after the next release
  ships these client changes.
