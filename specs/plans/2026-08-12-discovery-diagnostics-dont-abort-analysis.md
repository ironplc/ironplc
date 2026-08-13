# Plan: don't let discovery-time diagnostics skip analysis entirely

## Problem

`create_project` in `compiler/ironplc-cli/src/cli.rs` treats any
discovery-time diagnostic (a `.plcproj` `<Compile Include="...">` entry
that doesn't resolve, or -- newly, since #1320 -- a project-referenced
compatibility library IronPLC doesn't bundle, `P6011`) as immediately
fatal for the whole command:

```rust
if had_error {
    return Err(String::from("Error enumerating or reading source files"));
}
```

Every caller (`check`, `echo`, `tokenize`, `compile`) does
`create_project(...)?`, so this `Err` propagates before the caller ever
runs `project.semantic()` (or the equivalent per-command work). The
already-resolved files are fully loaded into `project` at that point --
they're just discarded, unused, un-analyzed.

**Real-world impact**: `ironplc-cli/src/cli.rs:1215`'s original PR
(`afefbc566`, "don't abort project discovery on one unresolvable
.plcproj entry") intended the opposite -- its own commit message says
`had_error` should be set "**without dropping the already-resolved
files from the check**". That intent was never actually implemented:
`check()`'s `create_project(...)?` drops the whole project, so the
"still checked" files never actually get checked. The one test covering
this (`check_when_plcproj_has_valid_and_missing_entries_then_error_but_valid_file_still_checked`)
only asserts `result.is_err()` -- it never asserts the valid file's own
diagnostics actually surfaced, so the gap was never caught.

This was a latent, low-impact bug when the only source was a single
`.plcproj` missing-file entry (rare). It became a real-world blocker
once `P6011` (referenced-but-unbundled library) started flowing through
the same path (#1320, 2026-08-04): IronPLC bundles only four
compatibility libraries so far (`Tc2_System`, `Tc2_Math`,
`Tc2_Utilities`, `Tc2_BuiltIns`), so any real TwinCAT solution
referencing a vendor library outside that set (visualization libraries,
motion control, IoT, ...) -- which is most of them -- now gets zero
analysis output at all, just the library-not-found diagnostics.
Verified directly: all 8 of a real private TwinCAT-solution corpus fail
`ironplcc check --dialect twincat` this way, where before #1320 they at
least ran full analysis.

## Fix

Change `create_project` to still return the built project when
discovery produced diagnostics, instead of discarding it:

```rust
fn create_project(...) -> Result<(FileBackedProject, bool), String>
```

The `bool` (`had_discovery_error`) is `true` when any diagnostic was
collected during enumeration/file-loading (unresolvable `.plcproj`
entry, unbundled library reference, unreadable file). Those diagnostics
are still printed via `handle_diagnostics` exactly as today -- only the
control flow changes: the caller gets the project either way and can
still do its real work.

Each of the four callers destructures the tuple, does its normal work
against the returned project, and folds `had_discovery_error` into the
final result **after** that work runs, so discovery diagnostics still
fail the overall command (matching the safety-first precedent from
`afefbc566`'s review: a partial project must not silently look clean)
without hiding real syntax/semantic diagnostics behind them:

- `check`: run `project.semantic()` as today; if that also errors,
  report and return `Err` (unchanged). Only if semantic analysis is
  clean, check `had_discovery_error` and return
  `Err("Error enumerating or reading source files")` if set.
- `tokenize`: run tokenization over all sources as today (still
  short-circuits via `?` inside the loop on the first per-source
  tokenize failure, unchanged); check `had_discovery_error` after the
  loop.
- `echo`: run the existing per-source render loop as today; combine
  `has_error` (its own existing flag) with `had_discovery_error` when
  deciding the final `Result`.
- `compile`: run the existing parse -> load-libraries -> analyze ->
  codegen -> write pipeline unchanged; check `had_discovery_error`
  immediately before the final `Ok(())`. Writing the `.iplc` on a
  discovery-diagnostic project is intentional and consistent with the
  other three commands: if a missing/unbundled reference's symbols are
  actually used, `analyze()` already fails compilation earlier via its
  own undeclared-symbol diagnostics (e.g. `P4017`); if unused, the
  output is genuinely correct and the command still reports overall
  failure via its exit code.

No change to `enumerate_files`, `LibraryRegistry::resolve_references`,
or discovery itself -- they already do the right thing (collect,
don't abort). This is purely a control-flow fix in `create_project` and
its four callers.

## Files

- `compiler/ironplc-cli/src/cli.rs` -- `create_project` return type and
  final block; `check`, `echo`, `tokenize`, `compile`.

## Tests

- Strengthen the existing
  `check_when_plcproj_has_valid_and_missing_entries_then_error_but_valid_file_still_checked`
  to actually prove the claim in its own name: make the valid entry
  contain a real semantic error (e.g. an undeclared variable) and
  assert that error's diagnostic is present in the output, not just
  that `check()` returns `Err`. This is the test that should have
  caught the original gap.
- New: `check_when_plcproj_references_unbundled_library_then_still_runs_analysis`
  -- a `.plcproj` referencing an unbundled library plus a source file
  with a real, unrelated semantic error (e.g. calling an undeclared
  function); asserts `check()` returns `Err` (command still fails) but
  the real semantic diagnostic is present in the reported output, not
  just the `P6011`.
- New (compile): `compile_when_plcproj_references_unbundled_library_and_unused_then_still_produces_container`
  -- a `.plcproj` referencing an unbundled library that the program
  never actually calls anything from; asserts `compile()` still writes
  a valid `.iplc` container (proving the pipeline doesn't stop early)
  while still returning `Err` overall.
- Existing `check_when_plcproj_references_missing_file_then_error` must
  keep passing unchanged (still `Err`, now for the right reason: ran
  successfully but discovery had diagnostics, not "aborted before
  running").

## Verification

After the fix, re-run the real-corpus regression check that surfaced
this: `ironplcc check --dialect twincat <path-to-a-real-solution-that-
references-an-unbundled-library>` should now print both the `P6011`
library diagnostics *and* any real syntax/semantic diagnostics from the
project's own files, and exit non-zero -- not just the `P6011` noise
with no other output.
