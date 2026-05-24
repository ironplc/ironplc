# Playground Analytics Instrumentation

## Context

The IronPLC playground gets regular traffic, but we have no visibility into what users actually do once they land. Today only Clicky pageviews are recorded, and the single `clicky.goal` wrapper (`trackGoal`) fires only two unstructured names (`embed_compile_error`/`playground_compile_error` and `embed_run`/`playground_run`). Without event-level data and properties we cannot answer basic operational questions:

- Are the docs-embedded playgrounds compiling cleanly, or is one of our own examples broken on a specific docs page?
- Which built-in examples do people actually try?
- When users run programs, do they crash (VM trap) or get stopped voluntarily — and which compiler/runtime error codes dominate?
- Does the 2013 dialect see real use, and which `--allow-*` feature flags get exercised?
- Do users edit the program after it loads, or just run what they were given?

This plan adds product-analytics instrumentation that captures session-level provenance (where the program came from, which dialect, embedded vs standalone, which docs page hosts the embed) plus discrete events for the compile/run lifecycle. Clicky stays for pageviews; PostHog is added for events with property bags. The Sphinx playground extension is extended so docs-hosted iframes self-identify their host page.

## Analytics backend

- **Keep Clicky** for pageviews. Leave the script tag in `playground/index.html` (line 16) and `trackPageview()` in `app.js` (lines 139-143) untouched. The example-selection pageview at `app.js:247` stays.
- **Drop the `clicky.goal` path.** Remove `trackGoal()` (lines 133-137) and both call sites (lines 482 and 505).
- **Add PostHog** for events with properties. The public project key is embedded in the playground bundle (PostHog "public" project keys are designed for browser use and are not a secret). Snippet goes into `playground/index.html` `<head>`.

## Event schema

Super-properties — attached to every PostHog event via `posthog.register()`:

| Property            | Type                                         | Notes |
|---------------------|----------------------------------------------|-------|
| `embed`             | bool                                         | True when `?embed=true` |
| `dialect`           | `"2003"` \| `"2013"`                         | From `getDialect()`; re-registered on dialect change |
| `allows`            | string                                       | Sorted CSV (e.g. `"c-style-comments,sizeof"`), `""` if none |
| `program_origin`    | `"docs"` \| `"url_shared"` \| `"example"` \| `"user_defined"` | See taxonomy below |
| `host_page`         | string \| null                               | Docs page slug; only when `origin="docs"` |
| `example_name`      | string \| null                               | Only when `origin="example"` |
| `program_modified`  | bool                                         | Flips true on first keystroke after a program is loaded |

Events:

```
playground_loaded   { }
compile_attempted   { trigger: "manual" }
compile_finished    { success, error_codes[], error_count,
                      program_lines, duration_ms }
run_started         { cycle_interval_ms }
run_stopped         { reason: "user" | "error" | "reload",
                      duration_ms, cycle_count,
                      error_codes[]   # only when reason="error"
                    }
example_loaded      { }   # example_name lives in super-props
```

Note: `trigger` is always `"manual"` today (the Start button is the only entry point). The property is included for forward compatibility with an auto-compile-on-type mode.

### `program_origin` taxonomy

Set once at load (and re-set when the user picks an example):

| Load condition                                          | `program_origin` | `host_page`   |
|---------------------------------------------------------|------------------|---------------|
| `?source=ironplc-docs&host=<slug>&code=…`               | `docs`           | `<slug>`      |
| `?code=…` without `source=ironplc-docs`                 | `url_shared`     | null          |
| User picks from examples dropdown                       | `example`        | null          |
| Anything else (default Counter template, scratch)       | `user_defined`   | null          |

`program_modified` distinguishes an untouched default template (`user_defined` + `program_modified=false`) from one the user typed/edited (`program_modified=true`). It also flags edits to examples and docs-loaded programs.

## Files to modify

### 1. `playground/index.html`
- Add PostHog snippet to `<head>` (just before `</head>`). Standard PostHog JS init pattern with the public project key.
- Leave the Clicky script tag (line 16) as-is.

### 2. `playground/app.js`
- **Remove** `trackGoal()` (lines 133-137) and both call sites (`trackGoal(... "compile_error")` at line 482, `trackGoal(... "run")` at line 505). Leave `trackPageview()` and its existing call at line 247.
- **Add** an inline analytics module at the top of `app.js` (keeps the diff focused — no new file):
  - `initAnalytics()`: reads URL params (`embed`, `source`, `host`, `code`, `dialect`, `allows`), computes initial super-properties, calls `posthog.register({...})`, fires `playground_loaded`.
  - `setProgramOrigin({ program_origin, example_name, host_page })`: updates the three provenance super-props and resets `program_modified` to false. Called from the examples dropdown handler.
  - `markModified()`: bound to the textarea `input` event (idempotent); sets `program_modified=true` via `posthog.register({ program_modified: true })`.
  - `capture(event, props)`: thin wrapper around `posthog.capture()` that no-ops if PostHog is absent (ad-blocked, offline).
- **Wire events at existing call sites:**
  - `compile_attempted` — fires inside the Start button handler before the `load_program` postCommand (line 465). Stash `performance.now()` for the duration.
  - `compile_finished` — fires when the worker responds to `load_program` (success and failure branches). `error_codes` from the diagnostic JSON (`d.code` field, e.g. `P0042`); `program_lines` from `source.split("\n").length`; `duration_ms` from the stashed timestamp.
  - `run_started` — fires immediately after a successful compile, just before `startStepLoop()` (around line 507). `cycle_interval_ms` from `intervalMs`.
  - `run_stopped` — fires on user Stop (line 513 handler), on VM trap (the failure branches in the step loop at lines 372-391), and on `pagehide` (reason `"reload"`). Track `cycle_count` from `cycleCount`; `duration_ms` from a `runStartTime` stashed at `run_started`; `error_codes` extracted from `result.diagnostics`.
  - `example_loaded` — fires inside the examples dropdown change handler (line 235). Calls `setProgramOrigin({ program_origin: "example", example_name: selected.name, host_page: null })` first, then `capture("example_loaded")`. Replaces the existing `trackPageview` line.
- **Re-register `dialect` super-prop** in the dialect-change handler (line 218), so subsequent events reflect the new dialect.

### 3. `docs/extensions/ironplc_playground.py`
- Add a `host` parameter to `_build_playground_url()` and `_build_iframe()`. When set, append `source=ironplc-docs&host=<host>` to the URL params.
- In each `Directive.run()`, derive the docname from `self.state.document.settings.env.docname` and pass it through. The link-only `PlaygroundLinkDirective` also receives a host so we can attribute landings from docs links.

### 4. Existing Clicky pageview at `app.js:247`
- Leave as-is. The PostHog `example_loaded` event is the structured replacement; the Clicky pageview remains for legacy continuity until the Clicky dashboards are retired separately.

## Things explicitly NOT in scope

- No cookie/consent banner. Current site has none; adding one is a separate policy decision.
- No replacement of Clicky pageviews.
- No new unit tests for the WASM compiler (no Rust changes in this PR).
- No backfilling: events start flowing only after deploy.
- No PII. PostHog `distinct_id` stays anonymous (PostHog default); program source code is **never** sent.

## Verification

1. **Local playground (standalone)**:
   - `cd playground && python3 -m http.server 8080`.
   - Open `http://localhost:8080/` with PostHog debug enabled (`localStorage.setItem('ph_debug', true)` then reload).
   - Devtools network tab: one `playground_loaded` event with `embed=false`, `program_origin="user_defined"`, `program_modified=false`, `dialect="2003"`, `host_page=null`.
   - Type one character → no event fires immediately, but the next `compile_attempted` carries `program_modified=true`.
   - Pick the "Sine Wave" example → `example_loaded` fires with `example_name="Sine Wave"`, super-prop `program_origin="example"`, `program_modified=false`.
   - Click Start → `compile_attempted` then `compile_finished{success:true, error_count:0}` then `run_started`.
   - Click Stop → `run_stopped{reason:"user"}`.
   - Edit code to introduce a syntax error and Start → `compile_finished{success:false, error_codes:["P####", ...]}`.
   - Force a VM trap (e.g., divide by zero) → `run_stopped{reason:"error", error_codes:[...]}`.

2. **Docs embed**:
   - `cd docs && make html`.
   - Open a generated page containing a `.. playground::` directive in the browser.
   - Inspect the iframe `src`: confirm it contains `source=ironplc-docs&host=<docname>`.
   - Inside the iframe (devtools → frame), confirm `playground_loaded` fires with `program_origin="docs"`, `host_page="<docname>"`, `embed=true`.

3. **URL share**:
   - Visit `http://localhost:8080/?code=<base64-of-anything>` (no `source` param).
   - Confirm `playground_loaded` fires with `program_origin="url_shared"`, `host_page=null`.

4. **Regressions**:
   - Clicky network beacons still fire on page load and on example selection (confirms the pageview path is intact).
   - `cd compiler && just` passes (this PR touches no Rust, but CI is the gate).
   - PostHog dashboard receives events within ~1 minute of testing.

## Out-of-band follow-up (not part of this PR)

- After 1-2 weeks of data, build PostHog insights for: (a) `compile_finished{success:false}` grouped by `host_page` to surface broken docs examples, (b) `run_stopped{reason:"error"}` error-code distribution, (c) example popularity, (d) dialect adoption.
