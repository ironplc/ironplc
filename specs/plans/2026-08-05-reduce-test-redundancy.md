# Plan: Reduce Test Redundancy

## Context

The test suite carries a large volume of near-identical, templated test code.
Running `cargo dupes` with tests *included* (the CI `dupes` recipe excludes them
via `--exclude-tests`) reports **~29% exact duplication**, and the large majority
of the redundant lines are in test code. The duplication is structural, not
accidental copy-paste of logic: the same test body repeated with a different input
string and expected value.

Two hotspots dominate:

1. **codegen `compiler/codegen/tests/it/end_to_end_*.rs`** — the single largest
   mass. One dupes group has 133 identical function bodies. A fix mechanism
   **already exists** in `compiler/codegen/tests/it/common/mod.rs`
   (`assert_run_i32/i64/f32/f64[_near][_with]` helpers and the declarative
   `e2e_i32!/i64!/f32!/f64!/…` macros, wired in via `#[macro_use] mod common;` in
   `tests/it/main.rs`). It is only partially adopted — the raw `parse_and_run`
   scaffold still appears in ~90 files.
2. **analyzer `compiler/analyzer/src/rule_*.rs`** inline `#[cfg(test)]` modules —
   the `apply_when_<cond>_then_(ok|error)` scaffold recurs 233 times across
   35 files, with no helper/macro yet.

## Goal

Cut redundant and complex test *code* while holding line coverage ≥ 85% and losing
no test signal (no feature coverage removed, no `#[spec_test]` tests deleted). Do
this by reusing the existing macro/helper patterns, not by inventing new structure
or collapsing the deliberately-granular per-feature test files.

## Constraints

- BDD test names: `function_when_condition_then_result`.
- Keep the granular per-feature file structure (`syntax-support-guide.md` 26–46).
  Reduce redundancy *within* files; do not merge feature files.
- Never delete/rename `#[spec_test(REQ_…)]` tests — the build enforces them
  bidirectionally. The `e2e_*!` macros forward `$(#[$meta])*`, so a REQ id or `///`
  doc comment on a converted test survives.
- Coverage floor 85% via `just coverage`.
- No branching/loops in tests; self-contained; one assertion per logical concept.

## Approach

Pilot-first. The codegen e2e migration is the committed first deliverable because
it is the biggest mass and its tooling already exists and is proven. Later stages
are gated on the pilot's measured results (dupes % drop, coverage held, no lost
tests).

### Stage 0 — Baseline (no source changes)
- Record `cargo dupes check --min-lines 10` (without `--exclude-tests`), current
  `just coverage` %, and `cargo test -- --list | wc -l`.
- Add a non-blocking informational `dupes-tests` recipe to `compiler/justfile`
  (NOT in `default`/`ci`); leave the blocking `dupes` recipe unchanged.

### Stage 1 — codegen e2e migration (pilot / committed)
Finish the `e2e_*!` migration. Per file:
- Pure single-scan (`let (_c,bufs)=parse_and_run(src,&opts); assert_eq!(bufs.vars[i].as_*(), N)`)
  → `e2e_i32!/i64!/f32!/f64!` (`_near!` for tolerance, `_with!` for a dialect flag).
- Scalar-varying family (one program template, only a literal/expected differs)
  → `rstest` `#[case]` table with a `format!` template (mirror
  `end_to_end.rs::end_to_end_single_var_scalar`).
- Multi-scan / custom-VM-driving tests → leave hand-written.

Convert in alphabetical file batches, one commit per batch; run `just test` +
`just coverage` after each. Reference already-migrated file:
`compiler/codegen/tests/it/end_to_end_bit_access.rs`.

### Stage 1 gate
Record codegen dupes exact% drop, coverage %, test count. Proceed to later stages
only if green, coverage ≥ 85%, exact% materially dropped, no test count lost beyond
intended rstest parametrization.

### Stage 2 — analyzer rule tests
Add `resolve_default(program)` to `compiler/analyzer/src/test_helpers.rs` and
crate-root `#[cfg(test)]` macros `rule_ok!`, `rule_err!(…, problem)`,
`rule_ok_with!/rule_err_with!` (dialect-gated). Pilot on
`rule_enumeration_values_unique.rs`, then roll across the ~35 rule files. Keep
exact-diagnostic-count assertions explicit (or add `rule_err_count!`).

### Stage 3 — corpus dedup
Parametrize `compiler/plc2plc/src/tests/corpus.rs` (fully) and the trivial
`parse_resource; assert is_ok()` block of `compiler/parser/src/tests/corpus.rs`
with `rstest` `#[case::name]`. Keep the parser AST-fixture assertions as-is.

### Stage 4 — property tests (targeted, additive)
Add proptest only where an independent oracle exists; keep every deterministic
example test. Candidates: VM value/slot conversions (`compiler/vm/src/value.rs`),
VM arithmetic/logic opcodes vs a Rust oracle (near
`compiler/vm/tests/it/proptest_robustness.rs`), duration/integer-literal
round-trips (near `compiler/parser/src/tests/duration.rs`). Out of scope:
generative whole-program e2e, generative analyzer inputs, whole-grammar round-trip.

### Stage 5 — CI guardrail (measure-only)
Keep `dupes-tests` informational this round. Revisit a blocking test-duplication
gate in a later effort once the number stabilizes and a convention-safe threshold
is known.

## File map

- `compiler/justfile` — add informational `dupes-tests` recipe (Stage 0)
- `compiler/codegen/tests/it/common/mod.rs` — existing `assert_run_*` + `e2e_*!` (Stage 1 reference; extend only if a variant is missing)
- `compiler/codegen/tests/it/end_to_end_*.rs` — Stage 1 conversion targets (batch by filename)
- `compiler/analyzer/src/test_helpers.rs` — add `resolve_default` + `rule_ok!/rule_err!` macros (Stage 2)
- `compiler/analyzer/src/rule_*.rs` — Stage 2 conversion targets (pilot on `rule_enumeration_values_unique.rs`)
- `compiler/plc2plc/src/tests/corpus.rs`, `compiler/parser/src/tests/corpus.rs` — Stage 3
- `compiler/vm/src/value.rs`, `compiler/vm/tests/it/proptest_robustness.rs`, `compiler/parser/src/tests/duration.rs` — Stage 4

## Tasks

- [ ] Stage 0: baseline metrics + `dupes-tests` recipe
- [ ] Stage 1: migrate codegen `end_to_end_*.rs` to `e2e_*!` (batched)
- [ ] Stage 1 gate: record deltas, confirm coverage ≥ 85% and no lost tests
- [ ] Stage 2: analyzer `rule_ok!/rule_err!` macros + roll-out
- [ ] Stage 3: corpus dedup (plc2plc + parser)
- [ ] Stage 4: targeted additive proptests
- [ ] Stage 5: keep `dupes-tests` measure-only; revisit gating later

## Verification

- Per batch: `cd compiler && just test` green; `just coverage` ≥ 85 (watch
  `--show-missing-lines`).
- Dupes movement: `cargo dupes check --min-lines 10` (no `--exclude-tests`)
  before/after per crate.
- Test-count guard: diff `cargo test -- --list | wc -l` before/after; should drop
  only by the intended rstest-parametrization delta.
- Before any PR: `cd compiler && just` (compile + coverage + lint + dupes) passes.
