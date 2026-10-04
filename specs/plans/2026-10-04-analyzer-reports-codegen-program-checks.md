# Report codegen's program checks in the analyzer (#2051)

## Goal

Analysis reports every problem with the program as written, so `ironplcc
check` and the language server show it beside every other problem. Today a
problem that only codegen finds appears only from `compile`. For each codegen
site that raises a user-facing problem code, either show that analysis already
reports it, or add an analyzer rule that does. The codegen checks stay as
fallbacks; turning them into internal errors is later work (#2011).

Where codegen's check is looser than the language, the new rule follows the
language. Correctness comes before keeping today's programs compiling, as
decided in review of #2055; this replaces #2051's criterion that `compile`
output stays unchanged for programs that compile today. Each PR lists every
program it newly rejects.

## Site inventory

Recounted against `main` at 74b9571. Non-test code makes 23
`Diagnostic::problem` calls with a user-facing code, 14 of them
`ConstantOverflow`, matching the count in #2011's design. `encoding_mismatch` is one of
the 23 but has three callers, so codegen can raise a code from 25 places.
`RecursiveCycle` is not one of them: `call_graph.rs` already reports a cycle as
an internal error, and the name appears only in comments.

Each (b) entry was checked with `ironplcc check` and `ironplcc compile` on the
program shown: `check` is silent and `compile` reports the code.

Groups: **(a)** analysis already reports the problem for every program that
can reach the site. **(b)** it does not. **(c)** programs that analysis accepts
as valid reach the site, so it is a gap in codegen rather than a problem with
the program. **(t)** a target limit, which stays in codegen. **(–)** no
diagnostic from it reaches the user.

| # | Site (`compiler/codegen/src/`) | Code | Program that reaches it | Group |
|---|---|---|---|---|
| 1 | `compile_array.rs` `try_constant_flat_index` | P2027 | `a[6]` on `ARRAY[1..5]`; also through a named array type, a structure field and a folded `a[3 + 3]` | (b) |
| 2–3 | `compile_expr.rs` `compile_constant`, integer literal at 32-bit signed width (negative, positive) | P2026 | `ADD(d, 5000000000)` (generic parameter); `FOR i := 0 TO 5000000000`, and the same in `BY`; `a[5000000000]`; `WHILE (x + 5000000000) > 0`; `SHL(d, 5000000000)` | (b) |
| 4–5 | same, 32-bit unsigned (negative, too large) | P2026 | `d : DWORD := -1`; `d : DWORD := 16#1FFFFFFFF` | (b) |
| 6–7 | same, 64-bit signed (negative, positive) | P2026 | `ADD(l, 10000000000000000000)` on a `LINT` | (b) |
| 8 | same, 64-bit unsigned, negative | P2026 | `ADD(u, -1)` on a `ULINT`; `d : LWORD := -1` | (b) |
| 9 | same, bit-string literal at 32-bit width | P2026 | `d := DWORD#16#1FFFFFFFF` | (b) |
| 10 | same, bit-string literal at 64-bit width | P2026 | `d := LWORD#16#1FFFFFFFFFFFFFFFF` | (b) |
| 11–12 | `compile_expr.rs` `signed_integer_to_i64` | P2026 | none: all four callers (`constant_i64`, `compile_loop.rs` `try_constant_i64`) discard the error with `.ok()` | (–) |
| 13 | `compile_stmt.rs` `CaseLabelValue::overflow` | P2026 | a label outside the selector's width. `rule_constant_range` checks every label against the selector's own (narrower) range, and `rule_case_selector_type` (P4053) admits only integer and enumerated selectors. Checked for named, alias, subrange, array element, structure field, function result, arithmetic, `p^`, function block output and `VAR_EXTERNAL` selectors | (a) |
| 14–15 | `compile_stmt.rs` `signed_integer_to_i32`, from `compile_array.rs` `array_spec_from_inline` | P2026 | `VAR a : ARRAY[2147483648..2147483650] OF BOOL`. The same bounds in a `TYPE` declaration are P2024 in analysis (`intermediates/array.rs`) | (b) |
| 16 | `compile_stmt.rs` `StmtKind::Exit` | P4021 | `rule_loop_control_inside_loop` | (a) |
| 17 | `compile_stmt.rs` `StmtKind::Continue` | P4065 | `rule_loop_control_inside_loop` | (a) |
| 18 | `compile.rs` `CompileContext::var_index` | P4007 | an undeclared name is reported by `rule_use_declared_symbolic_var`, but valid programs also reach it: a variable inherited through `EXTENDS` (documented in `extends.rst`, asserted in `codegen/tests/it/compile_extends_inheritance.rs`), and a `RESOURCE`'s `VAR_GLOBAL` used through `VAR_EXTERNAL` (#1930, fixed by the open #1938) | (c) |
| 19 | `string_width.rs` `resolve_operand_char_width` | P4034 | `w = 'abc'`; `CONCAT(s, w)`; `FIND(w, 'cd')`; `CONCAT(CONCAT(w, w), s)`; `CONCAT(w, w) = 'abab'` | (b) |
| 20 | `string_width.rs` `compile_string_value` | P4034 | `a[1] := w` into an `ARRAY OF STRING`; `s.f := w` into a `STRING` field; `a[1] := CONCAT(w, w)` | (b) |
| 21 | `compile_string.rs` `resolve_string_arg` | P4034 | only a named `WSTRING` passed where a `STRING` is required (`F(w)`, `STRING_TO_INT(w)`), which `rule_function_call_type_check` reports first (P4026) | (a) |
| 22 | `compile.rs` `apply_task_configuration` | P4047 | a `SINGLE` task parameter | (t) |
| 23–24 | `compile.rs` `apply_task_configuration`, `task_interval_us` | P4048 | a task `INTERVAL` or `PRIORITY` outside what the bytecode holds | (t) |
| 25 | `compile.rs` `find_program` | P4020 | no `PROGRAM` to compile | (t) |

Two of the issue's assumptions do not hold. The analyzer checks
`StringEncodingMismatch` only between two named variables, so literals,
string function results and element or field stores reach codegen unchecked
(sites 19–20). And valid programs reach `VariableUndefined` (site 18), through
`EXTENDS` and through resource globals.

## Architecture

**Arrays (sites 1, 14–15).**

- A new rule, `rule_array_index_range`, reports P2027 for a subscript that is
  an integer literal outside its dimension. It reads the dimensions from the
  subscripted variable's `IntermediateType::Array` (the analyzer stores the
  same `lower`/`upper` that codegen's `DimensionInfo` is built from) and labels
  the subscript, not the whole access. Rules run on the folded library, so it
  sees the same literals as `try_constant_flat_index`. It compares at `i128`,
  so `a[5000000000]` is P2027 too, which also covers that subscript case of
  site 2.
- A subscript that names a constant (`a[K]`) is out of bounds just as surely,
  but codegen does not check it either, and the analyzer has no shared lookup
  for a constant's value (the only table is private to
  `xform_fold_initializer_expressions`). Checking it is a follow-up issue, not
  part of moving codegen's checks.
- `rule_range_limits` also requires each inline array bound to fit a `DINT`,
  and reports P2024 for one that does not. That is the code
  `intermediates/array.rs` already gives a `TYPE`-declared array, keeping its
  stated rule that an array dimension gets one code wherever it is declared.
  Codegen's fallback stays P2026.

**String encodings (sites 19–20).**

- Extend `rule_string_encoding_compat` from named variables to every string
  operand:
  - the two sides of a comparison;
  - the string arguments of one standard string function (`CONCAT`, `INSERT`,
    `REPLACE`, `FIND`, and so on);
  - the value stored into an array element or a structure field.
- An operand's encoding comes from its resolved type, which already carries
  one for a literal (by its delimiter), a variable, and a string function's
  result: `xform_resolve_expr_types` gives a generic return type the type of
  the first matching argument. That is why `F(CONCAT(w, w))` is already P4026
  with `actual=wstring`.
- The rule's module doc says the analyzer collapses string function results
  to `STRING` and leaves them to codegen. That is out of date, so it is
  rewritten.

**Integer and bit-string literals (sites 2–10).** These wait for #2050
("finish ADR-0056"), as #2051 asks. After #2050's last step every literal in
the analyzed `Library` has a concrete type. `rule_constant_range` then checks
every integer and bit-string literal against the range of that type: `SINT`
holds -128 to 127, `BYTE` 0 to 255, `DWORD` 0 to 4294967295. A literal's value
must be one its type can hold.

- **Stricter than codegen.** Codegen checks only the 32- or 64-bit slot a
  value is stored in (`value_range::fits`). These programs compile today and
  will be rejected: `FOR s := 0 TO 300` on a `SINT`, `ADD(s, 300)`,
  `b : BYTE := 256`, `BYTE#256`, `d : DWORD := -1`.
- **Bit strings are checked.** `rule_constant_range` and `P2026.rst` exempt
  them today, on the grounds that wrapping a bit pattern is legitimate.
  Wrapping a value at run time stays legitimate and is not affected. A
  constant is not a run-time value, though, and `BYTE#256` is not a byte. The
  exemption goes, and so does `P2026.rst`'s `pattern := 255 + 1` example of an
  accepted wrap.
- **Generic parameters are checked.** An argument to `ADD`'s `ANY_NUM` inputs
  is checked against the type the call resolves to, which #2050 records on the
  literal. `P2026.rst` says such arguments are not checked; that changes.
- **Same change as #2050's last step.** That step switches
  `rule_constant_range` from predicting literal types to reading them. This
  lands as part of it or right after it. Either way, the comparison and
  `CASE` checks keep reading the program as written, because they ask whether
  the other operand can ever equal the literal, which is a different question
  from the literal's own type (ADR-0056's `DINT#300 < s` case).
- **Not before #2050.** Doing it earlier would mean predicting the operation
  type of each context (generic arguments, `FOR` bounds, comparisons), which
  #2050 removes from `rule_constant_range`.
- **Codegen keeps its looser check** as the fallback.

**(a) sites (13, 16, 17, 21).** Each codegen test that reaches the fallback (for
example `compile_when_case_label_does_not_fit_selector_width_then_constant_overflow`,
`compile_when_exit_outside_loop_then_p4021_error`,
`compile_when_continue_outside_loop_then_p4065_error`) gets a matching rule
test on the same program, written with the rule macros or the shared rule
test helpers. Where the rule's tests
already contain that program, the plan reuses them and adds nothing: most of
the `CASE` cases are already in `rule_constant_range/tests.rs`, but `ULINT`
`-1` and a `DINT` subrange ending at 4294967295 are not. Each codegen site
gets a doc comment naming the rule that reports the problem first, as
`compile_stmt.rs` already has for `CASE` labels.

**Site 18.** No analyzer rule: the programs that reach it are valid. Today
codegen tells the user that a declared variable is undefined. Instead, its
lookup failure reports P9999 (`Diagnostic::not_implemented`), saying the
variable is one codegen cannot yet store. An undeclared name never reaches
that point through `compile`, because `rule_use_declared_symbolic_var` reports
it first. The PR adds that rule test next to the codegen fallback.
`compile_extends_inheritance.rs` and `extends.rst` change from P4007 to P9999.
#1938 then removes the resource-global path, and once codegen stores every
variable the analyzer accepts, the site becomes an internal error with the
others.

**Sites 11–12.** `signed_integer_to_i64` becomes `Option<i64>`, and the two
identical callers (`compile_expr::constant_i64`, `compile_loop::try_constant_i64`)
become one function. No diagnostic is built only to be thrown away, and
lowering has two fewer sites to translate.

## Prefactoring

- **Prefactor PR:** sites 11–12, above. This is behaviour-preserving, since
  every caller already discards the error.
- No prefactor is needed for the rules. `rule_range_limits` already visits
  inline array subranges. `rule_string_encoding_compat` already has the scope
  tracking and variable typing it needs. The new array rule follows the
  existing rule shape (`run_rule`, `DiagnosticVisitor`).

## Design doc reference

- The lowering design on #2011 (not on `main`; not cited from code or docs).
- [ADR-0056](../adrs/0056-analyzer-records-implicit-conversions-in-the-ast.md)
  and #2050, which the literal step depends on.

## PRs and tracking

#2051 tracks these PRs:

1. Prefactor: `signed_integer_to_i64` returns `Option`, with one
   constant-literal helper.
2. Core, arrays: `rule_array_index_range` (P2027) and the `DINT` bound check
   in `rule_range_limits` (P2024).
3. Core, string encodings: `rule_string_encoding_compat` covers operands and
   element or field stores.
4. Core, (a) sites: matching rule tests and doc comments on the codegen
   fallbacks.
5. Core, site 18: codegen's variable lookup reports P9999 instead of P4007.
6. Core, literals, **with or after #2050's last step**: every literal is
   checked against its type's range.

PR 1 lands first. PRs 2–5 are independent of each other and of #2050. PR 6
belongs with #2050's last step.

## Decisions

Settled in review of #2055, on the principle that correctness comes before
keeping today's programs compiling:

1. A literal is checked against its type's own range, bit strings and generic
   parameters included, not against codegen's storage width.
2. An inline array bound outside `DINT` is P2024, the code a `TYPE` array
   already gets, not codegen's P2026.
3. Codegen's variable lookup reports P9999 now, rather than waiting for #1938.

## File map

- PR 1: `codegen/src/compile_expr.rs`, `codegen/src/compile_loop.rs`
- PR 2: `analyzer/src/rule_array_index_range.rs` (new), `analyzer/src/lib.rs`,
  `analyzer/src/stages.rs`, `analyzer/src/rule_range_limits.rs`,
  `codegen/src/compile_array.rs` and `codegen/src/compile_stmt.rs` (fallback
  comments), `docs/reference/compiler/problems/P2027.rst` (today a bare
  summary; add a description and example), `docs/reference/compiler/problems/P2024.rst`
- PR 3: `analyzer/src/rule_string_encoding_compat.rs`,
  `codegen/src/string_width.rs` (fallback comments). `P4034.rst` already
  describes these cases, so it needs no change.
- PR 4: rule tests in `analyzer/src/rule_constant_range/tests.rs`,
  `rule_loop_control_inside_loop.rs` and the `rule_function_call_type_check`
  tests; fallback comments in `codegen/src/compile_stmt.rs` and
  `compile_string.rs`
- PR 5: `codegen/src/compile.rs`, `codegen/tests/it/compile_extends_inheritance.rs`,
  `analyzer/src/rule_use_declared_symbolic_var.rs` (rule test),
  `docs/reference/language/object-orientation/extends.rst`
- PR 6: `analyzer/src/rule_constant_range.rs` and its tests (including the
  module doc's bit-string exemption); `docs/reference/compiler/problems/P2026.rst`
  (the "Bit strings are not checked" section and the sentence on generic
  parameters)

## Tasks

Prefactor PR (1):
- [ ] `signed_integer_to_i64` returns `Option<i64>`
- [ ] One constant-literal helper replaces `constant_i64` and `try_constant_i64`
- [ ] `cd compiler && just`

Core PR (2), arrays:
- [ ] `rule_array_index_range`, registered in `stages.rs`, with rule tests:
      in range, each bound, below and above, multi-dimensional, negative
      bounds, named array type, structure field, folded subscript, beyond
      `i64`, and a non-literal subscript (not checked)
- [ ] `rule_range_limits`: inline bound outside `DINT` is P2024; tests for
      each bound and for the `DINT` limits themselves
- [ ] `P2027.rst` and `P2024.rst`
- [ ] Doc comments on the two codegen fallbacks
- [ ] Open a follow-up issue for subscripts that name a constant (`a[K]`)

Core PR (3), string encodings:
- [ ] Comparison operands, string-function operands (nested too), and stores
      into array elements and structure fields; tests for each program in
      sites 19–20, plus the matching-encoding cases that must stay clean
- [ ] Rewrite the rule's module doc
- [ ] Doc comments on the two codegen fallbacks

Core PR (4), (a) sites:
- [ ] Rule tests for each codegen fallback test program not already covered
- [ ] Doc comments on sites 13, 16, 17, 21

Core PR (5), site 18:
- [ ] `var_index` reports `Diagnostic::not_implemented` for a name it has no
      slot for
- [ ] `compile_extends_inheritance.rs` expects P9999; `extends.rst` says so
- [ ] Rule test in `rule_use_declared_symbolic_var` for the undeclared-name
      program that codegen's fallback was written for

Core PR (6), literals (with or after #2050's last step):
- [ ] `rule_constant_range` checks every literal against its recorded type's
      range, bit strings and generic parameters included; tests for each
      program in sites 2–10 and for each newly rejected program listed above
- [ ] Remove the bit-string exemption from the rule's module doc
- [ ] `P2026.rst`

Every core PR that adds or extends a rule:
- [ ] Find every program the rule newly rejects. The end-to-end harness
      ignores analysis diagnostics (`parse` keeps them in the context), so
      run the codegen suite once with `parse` asserting that the rule reports
      nothing, and check the documentation's runnable examples. Fix each
      fixture that relied on an invalid program, or turn it into a test of
      the new diagnostic, and list them all in the PR.

Every PR:
- [ ] The PR description lists its sites and their group.
- [ ] `cd compiler && just`
