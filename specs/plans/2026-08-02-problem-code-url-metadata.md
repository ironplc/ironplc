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

Every problem-code URL gains three UTM parameters (in addition to the existing,
untouched `version`, `file`, and `line`):

```
…/problems/<CODE>.html?version=<v>&utm_source=<channel>&utm_medium=problem-code&utm_campaign=<v>[&file=…&line=…]
```

- `version` — unchanged; still the only param `version-check.js` reads.
- `utm_source` — the channel: `playground`, `cli`, `extension`, or `mcp`.
- `utm_medium=problem-code` — constant, so diagnostic-link arrivals separate
  cleanly from organic docs navigation to the same page.
- `utm_campaign=<version>` — mirrors the version. PostHog auto-extracts only the
  `utm_*` params (not arbitrary params like `?version`), so this makes the
  client version a native breakdown dimension. Intentional, minor duplication of
  `version`.
- `file` / `line` — kept as-is on the LSP and CLI paths. They carry the Rust
  source location that raised the diagnostic, which is how a maintainer sees what
  a remote user hit.

`utm_*` is chosen over a custom `?source=` because PostHog parses `utm_*` into
first-class event and person properties automatically, driving the built-in
acquisition reports with no extra configuration. The apparent-unique-page
problem is handled on the dashboard side by breaking down on `$pathname` (path
only), which collapses every query-string variant to one row per code.

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
   append `&utm_source=extension&utm_medium=problem-code&utm_campaign=<v>` to the
   existing URL, before the conditional `&file`/`&line`.
2. **`compiler/ironplc-cli/src/cli.rs`** (`map_diagnostic`, ~line 360) — build
   the same URL (with `utm_source=cli`, plus `&file`/`&line` when present) and
   append it as a trailing `Learn more: <url>` note, so CLI diagnostics carry a
   clickable link. New `section_for_code` + `problem_help_url` helpers.
3. **`integrations/vscode/src/problemUrl.ts`** (new, pure, no `vscode` import) —
   `problemHelpUrl(code, version)` deriving section from the code prefix
   (`E→editor`, `P→compiler`, `V→runtime`) and emitting `utm_source=extension`.
   `extension.ts` imports it in `openProblemInBrowser`.
4. **`playground/src/app.ts`** (`renderDiagnostics`, ~line 998) — append the utm
   params with `utm_source=playground`.
5. **`compiler/mcp/src/tools/explain_diagnostic.rs`** — add a `doc_url` field to
   `ExplainDiagnosticResponse`, populated for found codes with
   `utm_source=mcp`; new `section_for_code` helper.

`section_for_code` is duplicated in `cli.rs` and `explain_diagnostic.rs` (a
four-arm match in two different crates); the LSP path keeps its hardcoded
`compiler` section (its diagnostics are P-codes) to avoid widening scope.

## Tests

- `lsp_project.rs`: extend the two existing URL tests to assert `utm_source=extension`
  and `utm_medium=problem-code`.
- `cli.rs`: new test — `map_diagnostic` notes contain the URL with `utm_source=cli`.
- `problemUrls.test.ts`: new unit test for `problemHelpUrl()`.
- `playground/tests/e2e.spec.ts`: update the href regex to require `utm_source=playground`.
- `explain_diagnostic.rs`: new test — `doc_url` contains `utm_source=mcp`.

## PostHog dashboard (as code)

New `infrastructure/posthog-problem-code.tf` — a `posthog_dashboard`
"IronPLC — Problem-code reach" plus one `posthog_insight` per tile. All tiles
filter `$pageview` where `$pathname` contains `/problems/` and
`utm_medium = problem-code`:

1. Total problem-code arrivals (BoldNumber).
2. Reach by channel — breakdown `utm_source` (table). Headline adoption tile.
3. Channel trend — weekly unique visitors, breakdown `utm_source` (line).
4. Top problem codes — breakdown `$pathname` (bar).
5. Version freshness — breakdown `utm_campaign` (table).
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
