# Plan: One-click P9999 "Submit Code" reporting from the playground

## Goal

P9999 ("capability not implemented yet") is the second most common compile
error, yet the project has never received a single report. The only documented
path is a high-friction GitHub issue that asks users to build a minimal repro —
and it is not surfaced at the point of failure. Meanwhile almost all P9999
volume originates in the browser playground, which today has no report
affordance at all.

Add a low-friction, **consent-forward** "Submit Code" affordance to the
playground diagnostics panel that appears whenever a P9999 diagnostic is shown.
Submitting sends the program source (the thing we actually need) to the IronPLC
team **without requiring a GitHub account**, via an explicit PostHog event, and
also offers a prefilled GitHub issue for users who want to track it.

## Consent / privacy

This is the load-bearing requirement. Users must clearly understand, before they
act, that:

- they are submitting the **program in the editor**, and
- **their code may become public** (e.g. in a GitHub issue) — this applies to
  the PostHog path *and* the GitHub path.

Design consequences:

- The action verb is on the button itself: **"Submit Code"** (not "Report").
- A plain-language consent line sits directly above the button.
- Source is **only** ever transmitted on this explicit click. The existing
  automatic `compile_finished` event stays source-free (its documented promise in
  `infrastructure/posthog.tf` is preserved and clarified).

## Architecture

Client-only change in the playground (TypeScript + CSS + HTML), plus docs and
an analytics dashboard tile. No Rust/WASM changes are needed: the playground
`Diagnostic` already carries the compiler `file#Lline` inside `d.label` (from the
`Diagnostic::todo` helper), and the WASM `compile` path already returns P9999 as
a normal diagnostic (`compiler/playground/src/lib.rs`).

- `renderDiagnostics()` appends a report panel when any reportable code
  (P9999) is present.
- The panel's **Submit Code** button fires a new explicit PostHog event
  `todo_report_submitted` carrying the compiled source (truncated), the error
  codes, the diagnostic labels (which include `file#Lline`), program size,
  dialect/allows, and compiler version. Super-properties (`program_origin`,
  `embed`, `host_page`, …) attach automatically.
- The panel also offers a secondary **"open a GitHub issue"** link, prefilled
  with a `P9999` label, a title, and a body containing the diagnostics and —
  for small programs that fit under the browser URL limit — the source in a
  fenced block. Larger programs get an "attach the file" note instead.
- After a successful submit the panel is replaced with a thank-you confirmation
  and the button cannot be fired again (idempotent per render).

## Reliable P9999 trigger for tests

`%QX0.0 := TRUE;` (a direct hardware-address write) yields P9999 from
`codegen/src/compile_expr.rs` and is genuinely out of scope for a software
playground VM, so it is a stable e2e trigger. Verified against the built CLI.

## File map

- `playground/src/app.ts` — capture compiled source; render + wire the report
  panel; PostHog `todo_report_submitted`; GitHub link builder.
- `playground/style.css` — styles for the report panel, consent text, button,
  confirmation.
- `playground/index.html` — (only if a static hook is needed; prefer
  dynamic rendering, so likely unchanged).
- `playground/tests/e2e.spec.ts` — new e2e tests.
- `infrastructure/posthog.tf` — new "Todo (P9999) code submissions" tile;
  clarify the privacy comment to note this event intentionally carries source
  on explicit user action.
- `docs/reference/compiler/problems/P9999.rst` — mention the one-click
  playground path alongside the existing GitHub link.

## Tasks

- [ ] Commit this plan.
- [ ] app.ts: capture `lastCompiledSource`; add `REPORTABLE_CODES`; render the
      report panel in `renderDiagnostics`; wire the Submit Code button to a new
      `todo_report_submitted` capture; build the prefilled GitHub link; show the
      confirmation and prevent double-submit.
- [ ] style.css: style the report panel, consent line, button, and confirmation
      (standalone + embed + light of existing dark theme vars).
- [ ] e2e tests: panel appears on P9999; not on ordinary syntax error; Submit
      Code fires the event and shows the confirmation; GitHub link is prefilled.
- [ ] posthog.tf: add the submissions tile; clarify the privacy comment.
- [ ] docs: update P9999.rst.
- [ ] Verify: `tsc` typecheck; run playground e2e if the toolchain is available.
