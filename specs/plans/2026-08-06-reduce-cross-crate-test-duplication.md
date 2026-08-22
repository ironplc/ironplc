# Plan: Reduce Cross-Crate Test Duplication

## Context

The prior test-redundancy effort (`2026-08-05-reduce-test-redundancy.md`, PRs
#1327/#1328) reduced duplication *within* crates (rstest tables, `e2e_*!`
macros, shared drive helpers). This plan addresses the next layer: tests in
different crates that exercise the same code paths. `cargo dupes` finds only
5 token-exact cross-crate groups, but a semantic survey of all 16 crates finds
roughly **250–300 test instances** whose assertion is substantially already
made by the crate that owns the behavior. The duplication is not accidental
copy-paste; it is mostly *structural* — several crates wrap the same pipeline
and each re-tests it — so the fix is a mix of test deletion, test re-homing,
and refactoring the crates so a behavior has exactly one owner.

## Findings

### F1. Four front ends re-test the compile pipeline (~130–150 tests, largest cluster)

There is no single owned "compile" function. `project::run_semantic_analysis`
covers parse→analyze only; codegen is re-composed independently by
`ironplc-cli/src/cli.rs:115-212`, `mcp/src/tools/compile.rs:66-244`, and
`playground/src/lib.rs:625-719` (playground does not depend on `project` at
all, and the CLI's `compile` bypasses `project` while its `check` uses it).
Because nobody owns "compile", everybody tests it:

- The valid/syntax-error/semantic-error triple is restated at every layer:
  ~40 "happy path compiles" assertions, ~17 "parser rejects bad syntax"
  assertions, and ~25 "analyzer reports semantic errors" assertions across
  `project`, `mcp`, `playground`, `ironplc-cli` — all behaviors owned and
  far more thoroughly tested by `parser` (294 tests) and `analyzer` (505).
- `mcp` per-tool boilerplate: `build_response_when_valid_program_then_ok_true`
  ×5, `..._invalid_sources_then_error_diagnostic` ×9,
  `..._invalid_options_then_error_diagnostic` ×9, `..._semantic_error` ×7,
  `..._syntax_error` ×3 across `mcp/src/tools/*.rs`. The invalid-sources /
  invalid-options copies re-test `tools/common.rs::validate_sources` /
  `parse_options`, which already has 16 exhaustive tests of its own. Test
  scaffolding is also copy-pasted: `fn ed2_options()` ×11, a 6-line
  `fn build(src)` wrapper ×4.
- `mcp/tests/cli.rs`: ~40 of 57 subprocess rstest cases re-run the same
  tool × valid/syntax/semantic/empty-name/missing-dialect matrix as the unit
  tests; ~17 cases assert genuinely new wire-protocol facts.
- Dialect/feature flags are owned by `parser` (`options.rs` +
  `tests/dialect_flags.rs`) but re-verified behaviorally in `playground`
  (11 tests) and `mcp/src/feature_flag_conformance.rs` (full flag table
  through `check::build_response`). The CLI/LSP `extract_compiler_options_*`
  and `mcp` `list_options` tests assert only string→flag *mapping*, which is
  legitimately wrapper-owned.
- Fixture text is inlined everywhere instead of shared: the literal
  `"PROGRAM p\nEND_PROGRAM"` appears 17×, the semantic-error snippet
  `x := y;` 17× in `mcp` alone, and the `INVALID_RANGE : INT(10..-10)`
  fixture is inlined in `project` (3×) and `ironplc-cli/src/lsp_project.rs`
  (2×) although `analyzer/src/rule_decl_subrange_limits.rs` owns that rule
  and `resources/test/first_steps_semantic_error.st` exists. `ironplc-test`
  (`read_shared_resource`/`shared_resource_path`) is used by
  parser/analyzer/sources/plc2plc/ironplc-cli but **not** by
  project/mcp/playground.

### F2. parser ↔ plc2plc: round-trip subsumes parse-ok (~31 tests)

A plc2plc round-trip test parses the same file with the same default options
(`plc2plc/src/tests/common.rs:20-24`), so it strictly subsumes a parser
"parses ok" assertion on the same source:

- 12 of the 13 `parser/src/tests/corpus.rs` `parse_when_corpus_resource_then_ok`
  cases are subsumed by plc2plc's 17-case round-trip table (only `oscat.st`
  is parser-only). 9 of the same files are covered a *third* time by
  `parser/src/lexer.rs` tokenize-only tests.
- `parser/src/tests/reference_to.rs:160-436` and
  `plc2plc/src/tests/reference_to.rs:32-139` share 10 verbatim snippets;
  6 of the parser members assert nothing beyond `body.len() == 1`.
- ~13 feature areas (struct-init, AND_THEN, CASE labels, enums, OOP
  inheritance, STRING(n), partial access, TIME-as-name, constant
  initializers, …) have both legs on essentially the same snippet — 3 of
  them byte-identical source strings. Where the parser leg asserts real AST
  shape this is complementary; where it is pure `is_ok()` it is redundant.
- The analyzer is clean: no analyzer test re-asserts parse outcomes.

### F3. codegen ↔ vm: ~50 near-1:1 pairs, against an existing convention

Ten `vm/tests/it/execute_*.rs` files already carry a header naming the
codegen `end_to_end_*.rs` file that owns the *basic* case and restricting the
VM file to edge/trap cases. Where that convention is followed, there is no
duplication (arithmetic, conversions). Where it is not:

- **Timers** (largest pair-cluster, ~15 pairs): `execute_fb_ton/tof/tp.rs`
  drive the same `intrinsic.rs` state machines over the same time grids as
  the `end_to_end_fb_ton/tof/tp.rs` rstest cases.
- **Strings**: ~10 pairs (CONCAT/FIND/LEFT/MID narrow and wide) between
  `execute_string_ops.rs` and `end_to_end_concat/find/left/wstring.rs`.
- Two explicit convention violations: `execute_sub_i32.rs:23` and
  `execute_mul_i32.rs:23` keep nominal cases their own headers say live in
  codegen.
- The "steel thread" bytecode sequence is hand-assembled **7×** across
  `vm`, `vm-cli`, `container`, and `project/src/disassemble.rs`;
  `vm-cli/tests/cli.rs` duplicates the container-builder scaffolding that
  `vm/tests/it/common/mod.rs` already provides.
- Counterpoint to note: `vm/src/builtin.rs` (743 LOC) and
  `vm/src/intrinsic.rs` (411 LOC) have **zero** unit tests — codegen e2e is
  currently the *only* test of every conversion/MIN/MAX/LIMIT/SEL/MUX/BCD
  builtin and of CTU/CTD/CTUD/SR/RS/R_TRIG/F_TRIG. The convention (codegen
  e2e owns basics) is therefore load-bearing; VM-side deletions must keep
  every trap/edge/encoding case.

### F4. Duplicated production code causing duplicated tests

- `vm-cli/src/cli.rs:354-448` contains a full copy of the
  `format_variable_value` test module from `container/src/debug_format.rs:137-208`.
  The function lives in `container` (vm-cli imports it). The vm-cli copies
  carry `#[spec_test(REQ_VC_vm_cli_009)]`; that requirement
  (`specs/design/vm-cli.md`) is about vm-cli's *dump output*, so the spec
  coverage belongs on `write_variable_line`, not on a re-test of container's
  function.
- `ironplc-cli/src/logger.rs` and `vm-cli/src/logger.rs` are near-identical
  production modules (differ only in error type), each with duplicated tests.
- `project/src/project.rs` re-tests `sources`-owned behavior through thin
  delegation: `xml_file_returns_empty_library` (near-verbatim copy of
  `sources/src/source.rs:34`), `file_backed_initialize_many_*` ×2
  (duplicate `sources/src/project.rs::initialize_from_directories` tests).
- `ironplc-cli/src/cli.rs:548,:570` duplicate
  `sources/src/discovery/mod.rs:111,:138` (identical `.plcproj` +
  `MISSING.TcPOU` fixtures).

### F5. Copy-paste bugs found during the survey (fix regardless)

- `ironplc-cli/src/cli.rs:607` `tokenize_first_steps_when_valid_syntax_then_ok`
  calls `echo`, not `tokenize`.
- `ironplc-cli/src/cli.rs:593` `echo_first_steps_when_invalid_syntax_then_error`
  calls `check`, not `echo`.

## Constraints

- `#[spec_test(REQ_…)]` tests are enforced bidirectionally; never delete —
  **re-home** the requirement onto the owning crate's own surface instead
  (as with REQ_VC_vm_cli_009 above).
- Coverage floor 85% (`just coverage`). Deleting wrapper-level tests can drop
  *wrapper-crate* line coverage even when the tested behavior is covered
  elsewhere; check per-batch and keep one wiring test per code path.
- Keep known-high-value unique tests: mcp boolean-schema regression guard
  (`mcp/tests/cli.rs:390`), CLI output-clobber guards, encoding tests,
  playground session-state tests, `wire_format.rs` as canonical byte truth,
  vm-cli frozen golden `.iplc` files.
- BDD names; per-feature file granularity stays (syntax-support-guide).

## Approach

Ordered by risk. Stages A–B are test-only. Stage C refactors crates and needs
its own plan/PR per item.

### Stage A — Mechanical dedup + re-homing (test-only, low risk)

1. **vm-cli formatting tests**: add one `#[spec_test(REQ_VC_vm_cli_009)]`
   (rstest over type-tags) through `write_variable_line`/dump output; delete
   the 7 copied `format_variable_value_*` tests (`container` owns them).
   Also fix the two miswired CLI tests (F5).
2. **project**: delete `xml_file_returns_empty_library` and the two
   `file_backed_initialize_many_*` duplicates; keep one delegation test and
   the sort-order regression test (`project.rs:556`).
3. **vm**: delete the two nominal tests violating their own file headers
   (`execute_sub_i32.rs:23`, `execute_mul_i32.rs:23`).
4. **mcp**: add a `#[cfg(test)]` `tools/test_support.rs` with `ed2_options()`,
   the canonical valid/syntax-error/semantic-error fixtures, and the generic
   `build` wrapper. Collapse the 9× invalid-sources and 9× invalid-options
   copies to one wiring test per tool (single rstest each if practical);
   keep exactly one valid/syntax/semantic test per tool as the wiring proof.
   Trim `tests/cli.rs` to the ~17 wire-protocol-value-add cases plus one
   end-to-end scenario per tool.
5. **ironplc-cli**: drop the two `.plcproj MISSING.TcPOU` re-tests of
   `sources::discovery` (keep one asserting `check`'s Err wiring), and the
   in-process/subprocess double-coverage where the subprocess test asserts
   nothing beyond the in-process one (~9 of 18 in `tests/cli.rs`).

### Stage B — Apply/extend the ownership conventions (test-only, judgment required)

1. **Timers/strings (codegen ↔ vm)**: extend the existing header convention
   to `execute_fb_ton/tof/tp.rs` and `execute_string_ops.rs`: codegen e2e
   owns the nominal behavior matrix; VM keeps reset paths, ET clamping,
   encoding-mismatch traps, and anything not reachable from ST. Delete only
   VM cases with an exact codegen twin (~15 timer + ~6 string pairs); add
   the convention header comment to these files.
2. **parser corpus**: reduce `parser/src/tests/corpus.rs` `is_ok` table to
   files with no plc2plc round-trip (keeps `oscat`); keep all 7 AST-fixture
   tests. Reduce the 9 lexer corpus tokenize tests to one smoke case.
3. **reference_to**: delete the 6 parser tests asserting only
   `body.len() == 1` whose snippets are in plc2plc's round-trip table; keep
   every parser test with a real AST assertion.
4. **syntax-support-guide update**: the parser leg of a new feature must
   assert AST shape; "parses ok" alone is delegated to the plc2plc
   round-trip leg. This prevents the F2 pattern from regrowing.
5. **playground dialect tests**: keep `dialect_from_when_*`/`allows`-mapping
   tests (wrapper-owned), drop the behavioral ltime/sizeof re-verification
   (~6 tests) that `parser/tests/dialect_flags.rs` and
   `mcp/feature_flag_conformance.rs` already pin. (Keep one behavioral
   smoke test that a flag actually reaches the compiler.)

### Stage C — Crate refactors (each its own plan + PR)

1. **Single compile owner** (root cause of F1): add a full
   parse→analyze→codegen `compile()` to `project` (or a dedicated function
   alongside `run_semantic_analysis`), used by `ironplc-cli`, `mcp`, and
   `playground`. Wrappers then keep only response-shape/exit-code/binding
   tests. Playground currently avoids `project`; feature-gate the
   file-backed parts (`FileBackedProject` uses `fs`) so the
   `MemoryBackedProject` path is wasm-clean, or extract the pure pipeline
   into a `sources`-level function both can share.
2. **Shared fixtures for wrappers**: add `include_str!`-backed `pub const`
   fixtures to `ironplc-test` (valid / syntax-error / semantic-error
   programs, steel thread) so `mcp`, `project`, and (wasm-safe via
   `include_str!`) `playground` stop inlining them. Replaces the 17×/17×/5×
   literals.
3. **Steel-thread container builder**: extend `vm`'s `test-support` feature
   with the shared container builders (`single_function_container*`,
   steel-thread sequence) and use it from `vm-cli/tests/cli.rs`,
   `container`, and `project/src/disassemble.rs` tests instead of 7 hand
   copies.
4. **Logger unification**: extract the shared logger-configure logic from
   `ironplc-cli/src/logger.rs` / `vm-cli/src/logger.rs` (generic over the
   error mapping) into a small shared module/crate; keep one test suite.

## What NOT to do

- Do not delete `compile_*.rs` / `end_to_end_*.rs` twins in codegen: same
  inputs, complementary assertions (bytecode vs run result) — that split is
  deliberate.
- Do not remove codegen e2e conversion/builtin/FB coverage in favor of VM
  unit tests that do not exist (`builtin.rs`/`intrinsic.rs` have none).
  Adding VM unit tests for those is a possible *future* inversion, but until
  then codegen e2e is the sole owner.
- Do not touch `mcp/src/spec_conformance.rs`/`feature_flag_conformance.rs`
  REQ tests except to swap inlined fixtures for shared ones.
- Do not merge per-feature test files or rename `#[spec_test]` tests.

## Tasks

- [x] Stage A1–A3: vm-cli re-home + project/vm deletions (one PR)
- [x] Stage A4: mcp test_support + boilerplate collapse (one PR)
- [x] Stage A5: ironplc-cli dedup + miswired-test fixes (one PR)
- [x] Stage B1: codegen↔vm timer/string ownership pass
- [x] Stage B2–B4: parser corpus/reference_to reduction + guide update
- [x] Stage B5: playground dialect trim
- [x] Stage C1: plan for single compile owner
- [ ] Stage C2: shared fixtures in ironplc-test
- [x] Stage C3: shared container test builders
- [ ] Stage C4: logger unification

## Outcome of Stages A and B

Where a per-test check disagreed with an estimate above, the check won. The
estimates were derived from a survey; each deletion was then verified against
its claimed owner individually, and several claims did not hold:

| Cluster | Estimated | Actually deleted | Why the difference |
|---|---|---|---|
| codegen↔vm timers | ~15 | 9 | ET clamping, reset-from-expired, cold-start (IN never TRUE) and TP retrigger have no codegen twin |
| codegen↔vm strings | ~10 | 11 | Traps, the STR_INIT header contract and a narrow-stride wide array descriptor stay; the function cases went |
| parser `reference_to` | 6 | 3 | The `XOR` and `<> NULL` cases guard operator disambiguation and are not round-tripped |
| playground dialect | ~6 | 1 | The off/on contrast pairs are the only proof the dialect/allows strings reach the compiler |
| ironplc-cli `.plcproj` | 2 | 1 | Main's #1360 rewrote the second one to assert analysis still runs — a CLI-owned contract `sources` cannot test |

Coverage held throughout. Stage A moved the total 91.549% → 91.530% purely by
removing covered *test* lines; no production line lost coverage (uncovered
counts per touched file were identical before and after). For Stage B the
decisive check was that `vm/src/intrinsic.rs` stayed at 186/186 and
`vm/src/string_ops.rs` at 225/225 after deleting 20 VM tests, confirming the
codegen e2e suite genuinely exercises those paths.

Lesson for Stage C: a "this is redundant" claim from a survey is a hypothesis.
Before deleting, read both sides and name the specific assertion that survives.

## Verification

- Per PR: `cd compiler && just` (compile, coverage ≥ 85%, lint) green.
- Test-count diff (`cargo test -- --list | wc -l`) matches the intended
  deletions; no `#[spec_test]` lost (build enforces).
- `cargo dupes check --min-lines 10` (no `--exclude-tests`) before/after;
  baseline: 17.9% exact / 5.5% near.
- For each deleted wrapper test, name the owning test that keeps the
  behavior covered (in the PR description).
