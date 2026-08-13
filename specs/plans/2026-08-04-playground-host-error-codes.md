# Playground host/embedding-layer errors: reuse P9998

Closes #1201. Follow-up to #1200.

## Goal

Give every host/embedding-layer error surfaced by the playground WASM wrapper
(`compiler/playground/src/lib.rs`) a stable code so that every UI-visible error
carries one, and the empty-code special-case in `renderDiagnostics` can be
removed.

## Decision (revised)

The issue proposed a new `H####` family with a code + doc page per error. That
premise is wrong for these errors: **they are all illegal states (bugs), not
distinct user conditions.** In real playground use the bytecode base64 never
leaves the WASM boundary — the user edits *source*, which yields `P####`/`V####`
codes — so `Invalid base64`, `Invalid container`, `No program loaded`,
`Session is faulted`, and the serialization fallbacks are all "should never
happen" host bugs.

Bugs do not each deserve a stable code and a doc page. They should share **one**
internal-error code and be told apart by a **source-location reference**, which
is exactly the mechanism the compiler already has:

- `P9998 InternalError` — "Internal error indicating a bug in the compiler".
- `Diagnostic::internal_error(file, line)` stamps `file#Lline` into
  `source_file`/`source_line`, surfaced across the boundary as
  `compiler_file`/`compiler_line` and ranked on the existing P9xxx path.

So the whole `H####` family is dropped and every host illegal state reuses
`P9998`, carrying the WASM host `file`/`line` of the call site.

## Architecture

- **No new family**: no CSV, no `build.rs`, no docs section, no new `docs_section`
  / `sectionForCode` / `renderDiagnostics` arms. `P9998` already maps to the
  `compiler` docs section everywhere.
- Two `#[track_caller]` helpers in `lib.rs`:
  - `internal_run_error(message) -> RunError` — for the run/step path.
  - `internal_diagnostic(message) -> DiagnosticInfo` — for the compile path.
  Both derive the code from `Diagnostic::internal_error(loc.file(), loc.line())`
  (no hard-coded `"P9998"`) and record the location.
- `RunError` gains `compiler_file` / `compiler_line` (mirroring `DiagnosticInfo`)
  so runtime internal errors rank by location like compile-time ones; VM-trap
  literals fill them via `..Default::default()`.
- Front end: `RunError` type + `runErrorToDiagnostic` carry the location through;
  the empty-code special-case in `renderDiagnostics` is dropped (every error now
  carries a code). This is the one surviving piece of #1201.

## File map

- modify `compiler/playground/src/lib.rs` (helpers + all seven sites + tests)
- modify `playground/src/app.ts` (`runErrorToDiagnostic`, `renderDiagnostics`)
- modify `playground/src/types/messages.d.ts` (`RunError` location fields)

(The earlier `H####` scaffolding — CSV, `build.rs`, `docs/reference/playground/`,
and the `H`/`playground` arms in `docs_section`, `sectionForCode`, the Sphinx
extension, and `renderDiagnostics` — is reverted.)

## Validation

- `cd compiler && just` (build, coverage ≥85%, clippy, fmt)
- `cd docs && just ci` (Sphinx `-W -n`)
- VS Code extension `just ci` (unit tests, lint)
