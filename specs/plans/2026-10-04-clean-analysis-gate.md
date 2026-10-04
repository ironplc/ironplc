# Clean-Analysis Gate in Front of Codegen

Plan for [#2047](https://github.com/ironplc/ironplc/issues/2047), one of the
`lowering-pre-factor` issues for
[#2011](https://github.com/ironplc/ironplc/pull/2011).

## Goal

Make "codegen runs only on a clean analysis" something the type system
enforces. `ironplc_codegen::compile` takes a `CleanAnalysis`, a value that
can be built only from a semantic context that holds no diagnostics, instead
of a separate `&Library` and `&SemanticContext`. No generated code and no
diagnostic changes.

## What the code does today

Production callers already check before they call codegen, each in its own
way:

| Caller | Check today |
|---|---|
| `project/src/compile.rs` `compile` (also the MCP server and the playground) | `diagnostics.is_empty()`: seeded, parse and analysis |
| `ironplc-cli/src/lsp_runner.rs` | `context.has_diagnostics()` |
| `benchmarks/src/lib.rs` `compile_st` | asserts no diagnostics |

Test callers mostly do not check. The issue lists the ones in `vm-cli` and
`codegen/tests/it/common/mod.rs`; there are more:

- `codegen/src/compile.rs` tests: 13 calls through a local `parse`.
- `codegen/src/spec_conformance*.rs`: 6 files, 8 calls.
- `codegen/tests/it/end_to_end_tc2_math.rs`, `end_to_end_tc2_utilities.rs`:
  assert no diagnostics.
- `codegen/tests/it/end_to_end_debug_line_map.rs`: 2 calls on the output of
  `stages::resolve_types`, so the rules never run.
- The `ignore`d doc example in `codegen/src/lib.rs`.

### Which tests compile an analysis that reported problems

To find out, `ironplc_codegen::compile` was made to panic when
`context.has_diagnostics()`, and `cargo test --workspace --no-fail-fast` was
run. Every target passed except two: `ironplc-codegen`'s own unit tests
(3 failures) and its `it` integration suite (122 failures). **125 codegen
tests run codegen on a program that analysis rejects.** Nothing outside
codegen does: the production paths, `vm-cli`, `benchmarks`, `project`, `mcp`
and `playground` already compile only clean analyses.

So the issue's constraint "every existing end-to-end test passes unchanged"
cannot hold for the gate alone. Per
[Prefactoring](../steering/development-standards.md#prefactoring), the test
changes go in their own PRs, before the gate. The 125 tests fall into four
groups:

**A. Testing codegen's defensive paths (32 tests).** These feed codegen a
program that the rules reject on purpose, to pin codegen's own error or its
behaviour on that input.

| Tests | Analysis reports |
|---|---|
| `compile::tests::compile_when_exit_outside_loop_then_p4021_error` | P4021 |
| `end_to_end_continue::compile_when_continue_outside_loop_then_p4065_error` | P4065 |
| `compile_case::compile_when_case_label_does_not_fit_selector_width_then_constant_overflow` (7 cases) | P2026 |
| `compile_case::compile_when_case_selector_is_real_then_internal_error_at_selector` (2) | P4053 |
| `end_to_end_date::compile_when_count_exceeds_its_storage_then_internal_error` (5) | P2038, P2039 |
| `end_to_end_date::compile_when_time_literal_is_float_width_then_internal_error` (6) | P4035 |
| `compile_const_trunc::compile_when_constant_out_of_range_then_folded_not_truncated`, `end_to_end_const_trunc` (4) | P2026 |
| `end_to_end_types::*_overflow_then_wraps` (4), `end_to_end_type_alias::end_to_end_when_type_alias_int_overflow_then_truncated` | P2026 |

All 32 still pass when the library and context come from
`stages::resolve_types` (type resolution only, no rules): that context holds
no diagnostics for them, so they can reach codegen through the gate the way
a caller that skips the rules would.

**A′. Unreachable through any gate (2 tests).**
`compile_this_super::compile_when_self_ref_then_not_implemented` (2 cases):
type resolution itself reports P9999 for `THIS^` and `SUPER^`, so even the
`resolve_types` context holds a diagnostic. The analyzer already tests that
P9999 (`stages.rs` `analyze_when_self_ref_then_rejected_with_not_implemented`,
`rule_unsupported_extension.rs` `apply_when_self_ref_then_p9999`). These two
codegen tests are deleted; the
codegen arm stays, as a defensive error for a caller that builds a context
by hand.

**B. Ill-typed test sources (87 tests).** The test is about valid code, but
its source has a type error the analyzer rightly reports. The fix keeps the
property under test and makes the source valid, preferring a vendor flag
that admits the source unchanged over rewriting it.

| Tests | Analysis reports | Fix |
|---|---|---|
| `compile_bool` (7), `compile_cmp` (6), `end_to_end_bool` (8), `end_to_end_cmp` (6), `end_to_end_func_forms::end_to_end_when_not_parens_then_returns_negation` | P4035: a BOOL result into a DINT | declare the result `BOOL` |
| `compile_func_forms` (6), `end_to_end_func_forms` (9 more) | P4027: `EQ(...)` etc. into a DINT | declare the result `BOOL` |
| `end_to_end_bit_access` (9), `end_to_end_bitstring` (15 of 17), `end_to_end_type_alias` (2), `compile::tests::compile_when_byte_variable_then_produces_container` | P4035, P4049: an integer literal into, or arithmetic on, a bit string | `allow_int_literal_to_bit_string` and `allow_bit_string_arithmetic` (ADR-0031, ADR-0053); a run with only these two set clears all 27 and changes no other result |
| `end_to_end_bitstring::end_to_end_when_byte_not_in_if_then_correct`, `..._byte_not_zero_in_if_then_enters_body` | P4072: `IF NOT x` on a BYTE | test `(NOT x) <> BYTE#0`, which still needs the truncation the tests are about |
| `end_to_end_sel` (3), `end_to_end_sel_float` (5), `end_to_end_sel_lint` (2) | P4026: `SEL(G := 1, ...)` | `G := TRUE`/`FALSE`, a `BOOL` selector variable |
| `end_to_end_shift::end_to_end_when_shr_with_abs_then_computes_correctly`, `end_to_end_user_function::..._uses_in_builtin_then_correct` | P4026: `SHR` on DINT/INT | shift a `DWORD`/`WORD` |
| `end_to_end_ltime` `case_4_comparison` | P4035: integer into an `LTIME` result | declare the result `DINT` |
| `end_to_end_wstring::wstring_when_struct_wstring_array_field_written_then_reads_back` | P4035: `'...'` into a WSTRING | `"..."` |
| `end_to_end_arithmetic_overloads::end_to_end_req_ao_010_...` | P4035: UDINT quotient into a DINT | assign to a `LINT` (signed, and a widening the analyzer accepts) |

REQ-AO-codegen-010 in `specs/design/arithmetic-operator-overloads.md` itself
says "`UDINT / UDINT` assigned to `DINT`", a program the analyzer rejects.
Its wording changes with the test; the ID stays.

Where a `compile_*` bytecode assertion changes because the result variable's
type changed, only the store of the result changes; the PR description says
so test by test.

**C. Analyzer false positives (4 tests).** The program is valid, codegen
compiles it correctly, and the analyzer rejects it. `ironplcc check`
reproduces both on `main`:

- [#2056](https://github.com/ironplc/ironplc/issues/2056):
  `ARRAY[0..2] OF REF_TO INT` (Edition 3) or `OF REFERENCE TO INT`:
  `refs[0] := REF(val)` reports P2032 and `refs[0]^` reports P2031.
  Tests: `end_to_end_array_ref_to::end_to_end_when_array_of_ref_to_store_ref_then_roundtrips`,
  `end_to_end_reference_to::end_to_end_when_array_of_reference_to_element_bound_then_reads`,
  `spec_conformance::codegen_spec_req_rto_420_array_of_reference_element_access`.
- [#2057](https://github.com/ironplc/ironplc/issues/2057):
  `SIZEOF(arr)` with `arr : ARRAY[1..10] OF INT` and `allow_sizeof`:
  P4026, "expected=ANY, actual=ARRAY[1..10] OF INT". Structures and named
  array types are rejected the same way.
  Test: `end_to_end_sizeof::end_to_end_when_sizeof_array_of_int_then_returns_total_bytes`.

Fixing these changes diagnostics, so it is not part of this work. Route
the four tests the same way as group A, with a comment naming the issue, so
codegen stays covered and the gate does not wait on the analyzer. Each
issue's fix moves its tests back.

## Architecture

### The gate (`analyzer/src/clean_analysis.rs`, new)

```rust
#[derive(Clone, Copy, Debug)]
pub struct CleanAnalysis<'a> {
    library: &'a Library,
    context: &'a SemanticContext,
}

impl<'a> CleanAnalysis<'a> {
    pub fn new(library: &'a Library, context: &'a SemanticContext) -> Option<Self> {
        (!context.has_diagnostics()).then_some(Self { library, context })
    }
    pub fn library(&self) -> &'a Library { self.library }
    pub fn context(&self) -> &'a SemanticContext { self.context }
}
```

Re-exported from `ironplc_analyzer` beside `SemanticContext`. The shape is
the issue's sketch. `new` returns `Option` rather than the diagnostics:
every caller already holds the context, so `context.diagnostics()` is at
hand when `new` fails.

The doc comment says what the gate guarantees and what it does not:

- It guarantees that the context held no diagnostics when the pair was
  made. The shared borrow means the context cannot gain one while the pair
  exists.
- It does not guarantee that analysis ran, or that the two belong together.
  `SemanticContextBuilder` and `stages::resolve_types` both produce
  contexts with no rule diagnostics, and nothing ties a context to the
  library it is paired with. The gate stops a caller from forgetting the
  check, not one that sets out to skip it.

### Codegen

```rust
pub fn compile(
    analysis: CleanAnalysis<'_>,
    options: &CodegenOptions,
    sources: &dyn SourceLookup,
) -> Result<Container, Diagnostic>
```

The body starts with `let library = analysis.library();` and
`let context = analysis.context();`, and is otherwise unchanged. The
defensive arms in codegen stay: a caller can still build a gate from a
context that the rules never checked.

### Callers

- **`ironplc_project::compile`.** Keeps its `diagnostics.is_empty()` check,
  which also covers seeded and parse diagnostics. The `let`-`else` that
  fetches the cached library and context now also builds the gate; when it
  fails, the result is the existing `Diagnostic::internal_error()`. Every
  context diagnostic was reported by `project.semantic()`, so a failure
  there is a compiler defect, the same as a missing cache.
- **`lsp_runner.rs`.** `if context.has_diagnostics() { return Err(...) }`
  becomes `let Some(analysis) = CleanAnalysis::new(..) else { return Err(...) }`,
  with the same error text.
- **`benchmarks::compile_st`.** The assertion becomes
  `unwrap_or_else(|| panic!(...))` with the same message listing the codes.
- **Tests.** Each shared helper builds the gate and panics with the codes
  if it cannot. The group A and C helpers build it from `resolve_types`
  output.

## Prefactoring

This whole change is a prefactor for lowering. It needs two prefactors of
its own, because 125 codegen tests compile programs that analysis rejects
and would otherwise have to change in the gate PR:

1. Route groups A and C through a "before the rules" helper, and delete A′.
2. Make the group B sources valid.

They are independent and touch only tests and one requirement's wording.
After both land, the gate PR changes only test helpers and call sites, and
those changes are mechanical.

## Design doc reference

- No design document covers the analyzer–codegen boundary. The reasoning
  is local to `CleanAnalysis` and goes in its doc comment. No ADR: the
  issue settles the decision, and nothing here chooses between
  alternatives someone is likely to reopen.
- `specs/steering/compiler-architecture.md`, "Compiler Pipeline": stage 3
  still reads "Code Generation (future)". Name `codegen/` and say that it
  takes a `CleanAnalysis`. Update the structural test template in the same
  file if the helper names change.
- `specs/design/arithmetic-operator-overloads.md`: REQ-AO-codegen-010
  wording (prefactor 2).
- Do not cite the lowering design document from code or docs; it is not on
  `main`.

## File map

**Prefactor 1: defensive tests reach codegen before the rules**

- `codegen/tests/it/common/mod.rs`: `try_compile_before_rules` and
  `run_before_rules` (or macro forms for the `e2e_i32!` callers), built on
  `resolve_types`, with a doc comment saying they model a caller that skips
  the rules and that production never does.
- `codegen/src/compile.rs` (test module): an in-crate equivalent for
  `compile_when_exit_outside_loop_then_p4021_error`.
- Group A: `compile_case.rs`, `end_to_end_continue.rs`, `end_to_end_date.rs`,
  `compile_const_trunc.rs`, `end_to_end_const_trunc.rs`,
  `end_to_end_types.rs`, `end_to_end_type_alias.rs` (the overflow case).
- Group C: `end_to_end_array_ref_to.rs`, `end_to_end_reference_to.rs`,
  `end_to_end_sizeof.rs`, `codegen/src/spec_conformance.rs`, each with the
  issue link.
- Group A′: delete `codegen/tests/it/compile_this_super.rs` and its `mod`
  line.

**Prefactor 2: valid sources in codegen tests**

- `compile_bool.rs`, `compile_cmp.rs`, `compile_func_forms.rs`,
  `end_to_end_bool.rs`, `end_to_end_cmp.rs`, `end_to_end_func_forms.rs`,
  `end_to_end_bit_access.rs`, `end_to_end_bitstring.rs`,
  `end_to_end_type_alias.rs`, `end_to_end_sel.rs`, `end_to_end_sel_float.rs`,
  `end_to_end_sel_lint.rs`, `end_to_end_shift.rs`,
  `end_to_end_user_function.rs`, `end_to_end_ltime.rs`,
  `end_to_end_wstring.rs`, `end_to_end_arithmetic_overloads.rs` (all under
  `codegen/tests/it/`), and `codegen/src/compile.rs` (the BYTE test).
- `specs/design/arithmetic-operator-overloads.md`.

**Gate**

- `analyzer/src/clean_analysis.rs` (new), `analyzer/src/lib.rs`.
- `codegen/src/compile.rs` (signature and tests), `codegen/src/lib.rs` (doc
  example), `codegen/src/spec_conformance*.rs`.
- `codegen/tests/it/common/mod.rs`, `end_to_end_tc2_math.rs`,
  `end_to_end_tc2_utilities.rs`, `end_to_end_debug_line_map.rs`.
- `project/src/compile.rs`, `ironplc-cli/src/lsp_runner.rs`,
  `benchmarks/src/lib.rs`, `vm-cli/tests/dap.rs`, `vm-cli/tests/cli.rs`.
- `specs/steering/compiler-architecture.md`.

## Coordination

- [#2034](https://github.com/ironplc/ironplc/pull/2034) (open) splits
  `codegen/tests/it/common/mod.rs` into `assert.rs`, `bc.rs` and `run.rs`.
  All three PRs here edit the helpers that move to `run.rs`. Whichever lands
  second rebases; the edits move with the functions.
- [#1998](https://github.com/ironplc/ironplc/pull/1998) (plan, open) gives
  `THIS^`/`SUPER^` types. If it lands first, the A′ tests may become
  reachable through `resolve_types`; move them to group A instead of
  deleting them.

## Tasks

### Prefactor 1: defensive tests reach codegen before the rules

- [x] Open the two group C issues: #2056, #2057.
- [ ] Add the before-the-rules helpers (integration suite and in-crate).
- [ ] Move the 32 group A tests and 4 group C tests onto them. No
      assertion changes.
- [ ] Delete `compile_this_super.rs`.
- [ ] Verify with the temporary panic probe that none of these tests now
      compiles an analysis with diagnostics. Do not commit the probe.
- [ ] `cd compiler && just`.

### Prefactor 2: valid sources in codegen tests

- [ ] Fix the 87 group B tests as in the table. Prefer a flag over a
      rewrite, and keep each test's subject.
- [ ] Reword REQ-AO-codegen-010.
- [ ] Verify with the probe that the whole workspace now compiles only
      clean analyses.
- [ ] `cd compiler && just`, `cd specs && just`.

### Gate (after both prefactors)

- [ ] Tests first: `clean_analysis::tests`:
      `new_when_context_has_diagnostic_then_none` and
      `new_when_context_has_no_diagnostics_then_returns_library_and_context`
      (the accessors return the same references, `std::ptr::eq`).
- [ ] Add `CleanAnalysis` and its doc comment. Re-export it.
- [ ] Change `ironplc_codegen::compile` to take `CleanAnalysis<'_>`, and
      update the doc example.
- [ ] Move every caller listed above behind the gate. Keep
      `ironplc_project::compile`'s check of seeded and parse diagnostics.
- [ ] Update the "Compiler Pipeline" section of `compiler-architecture.md`.
- [ ] Confirm that no test's assertions changed. The diff to tests is
      helper and call-site plumbing only.
- [ ] `cd compiler && just`, `cd specs && just`.

### Cleanup

- [ ] Close this plan PR unmerged and close #2047. #2056 and #2057 stay
      open until their analyzer fixes land.
