# Plan: Focused Test Modules to Reduce Merge Conflicts

## Problem

Feature PRs repeatedly conflict in a handful of large, append-only test
files. The concrete trigger was PR #1243
(`feat(twincat): accept hex/binary/octal bit-string literals as CASE
labels`), whose test changes landed in:

- `compiler/parser/src/tests.rs` — one `mod test { … }`, ~3060 lines, 141 tests
- `compiler/plc2plc/src/tests.rs` — one `mod test { … }`, ~678 lines, 44 tests
- `compiler/codegen/tests/it/end_to_end_case.rs` — a focused file, but PRs
  *append* to it

Every feature PR does the same two things in each monolith:

1. adds names to the single shared `use { … }` block at the top, and
2. appends `#[test]` fns just before the closing `}`.

Those are the exact lines every other in-flight PR also touches, so Git
conflicts are effectively guaranteed. The git history shows the pressure —
a steady stream of parser/TwinCAT feature PRs (#1236, #1253, #1254, #1252,
#1241, #1224, #1214, #1207 …) all funnel through `parser/src/tests.rs`.

## The pattern already exists in-repo

`compiler/codegen/tests/it/` is already ~150 focused files, one per feature
(`end_to_end_case.rs`, `compile_add.rs`, …), compiled into a **single test
binary** via `mod foo;` lines in `main.rs`. Its header documents the
rationale (one binary → low link time / `target/` size). Adding a feature
there is a **new file + one `mod` line** — two PRs adding different files
never conflict on content; the only shared line is the sorted `mod` list,
which auto-merges for non-adjacent insertions.

This plan brings `parser` and `plc2plc` up to the same structure.

## Design

Convert each monolith `src/tests.rs` into a `src/tests/` directory:

```
parser/src/tests/
  mod.rs      # declares submodules only
  common.rs   # shared imports + helper fns (pub(crate))
  case.rs     # CASE labels (incl. the new bit-string labels)
  enums.rs
  reference_to.rs
  duration.rs
  …
```

- `lib.rs` keeps `#[cfg(test)] mod tests;` unchanged — it resolves to
  `tests/mod.rs`.
- `common.rs` holds the shared `use` imports (re-exported `pub(crate) use`)
  and every helper fn (`pub(crate)`), so no per-file import maintenance is
  needed. Each thematic file starts with `use super::common::*;`.
- Each thematic file contains only `#[test]`/`#[rstest]` fns for one
  feature area. The giant shared `use` block and the shared "end of file"
  both disappear as collision points.

Residual conflict surface: just the sorted `mod` list in `mod.rs` — one
line per PR, trivially resolvable.

### Behavior preservation

This is a pure move: no test body is edited. Test functions are relocated
verbatim; only module visibility wrappers change. `cargo fmt` normalizes
indentation (string-literal contents are untouched by rustfmt, so embedded
ST sources are unchanged). The full suite must run identically
before/after.

## Thematic grouping

Feature areas that have been active conflict sources each get their own
file: `case`, `enums`, `reference_to`, `duration`, `dialect_flags`,
`short_circuit`, `constant_initializers`, `partial_access`. The stable
"corpus round-trip / smoke" tests are grouped together.

## Convention going forward

Add to `specs/steering/syntax-support-guide.md`: new syntax → a new focused
test file + one `mod` line; do not append tests for an unrelated feature to
an existing file. This also covers the codegen `it/` suite (PR #1243 should
have added `end_to_end_case_bit_string.rs`, not appended to
`end_to_end_case.rs`).

## Tasks

- [x] Write plan (this document)
- [ ] Split `compiler/parser/src/tests.rs` into `tests/` submodules
- [ ] Split `compiler/plc2plc/src/tests.rs` into `tests/` submodules
- [ ] Document the convention in `syntax-support-guide.md`
- [ ] Run full CI (`cd compiler && just`) — build, coverage, clippy, fmt
- [ ] Push branch
