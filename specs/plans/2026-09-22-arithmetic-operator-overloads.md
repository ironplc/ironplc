# Plan: Arithmetic Operator Overloads

## Read this first: where the design cannot be built as written

The design is buildable, but reading the code turned up twelve places where
its text is wrong or leaves a decision open. None of them changes the
architecture. Each one needs either a small amendment to the design or a
decision recorded in this plan. PR 0 below makes the amendments before any
code lands, so every later PR builds against a design that is correct.

1. **The long forms do not exist.** The design says `ADD_LTIME`,
   `SUB_LDATE_LDATE` and the other long forms are "registered alongside the
   short one in `get_time_functions`". They are not. `get_time_functions`
   (`compiler/analyzer/src/intermediates/stdlib_function.rs:720`) registers
   only the eleven short forms plus `CONCAT_DATE_TOD` and the decomposition
   functions. `compile_function_call` has no arm for a long name, and
   `docs/reference/standard-library/functions/` has no page for one. A grep
   for `ADD_LTIME|SUB_LDATE|MUL_LTIME` across the compiler and the docs finds
   nothing. REQ-AO-analyzer-006, -007, -014 and REQ-AO-codegen-005 all depend
   on the long forms, so registering and compiling them is a PR of its own
   (Core 1). The eleven doc pages the design lists are therefore pages for
   functions this work adds, not for functions that already exist. The design
   also says that `lt1 + lt2` "compiles correctly today". That is true of the
   operator: `end_to_end_ltime.rs` case 3 runs it through the generic
   `BinaryOp` path at W64. It is not true of any long-form call.
2. **REQ-AO-codegen-005 says a short operand of a long form is
   "sign-extended".** That is right for `TIME`. It is wrong for `DATE`,
   `TIME_OF_DAY` and `DATE_AND_TIME`, which are `(W32, Unsigned)` in
   `codegen/src/type_info.rs:134-139` (ADR-0025). Sign-extending a date past
   2038 into an `LDATE` operation gives a negative number of seconds. The
   amendment says the short operand is widened according to its own
   signedness: `TIME` is sign-extended, and the three date types are
   zero-extended (`CONV_U32_TO_I64`, which `emit_conversion_opcode` already
   emits). The widths share a unit per ADR-0021 and the ADR-0025 amendment, so
   no unit conversion is needed.
3. **REQ-AO-codegen-006 ("same bytecode as before when operands and target
   share an operation width") is false when only the signedness differs.**
   Take `d := u1 / u2` with `UDINT` operands and a `DINT` target. The
   operation width is W32 on both sides, but today the expression compiles at
   the target's `(W32, Signed)` and emits `DIV_I32`, while the resolved type
   `UDINT` gives `DIV_U32`. Division, `MOD`, and any conversion to floating
   point differ. The amendment rewords the requirement to "share an operation
   width and signedness", and adds this case as a row to *Programs whose value
   changes*. `4000000000 / 2` changes from -147483648 to 2000000000.
4. **P4049 has a second family the design does not mention.** `AND`, `OR`,
   `XOR` and `NOT` report P4049 today, one diagnostic per failing operand,
   with `expected=ANY_BIT` and `actual=` contexts. `NOT` has only one operand,
   so it cannot take `left`/`right` contexts. **Decision:** the bit-string
   family keeps its per-operand `expected`/`actual` shape unchanged. The five
   arithmetic operators and the four overloaded function names report one
   P4049 per failing expression (or per failing fold step), with `operator`,
   `left` and `right` contexts. The new message, "Operator is not defined for
   the operand types", reads correctly for both shapes. `P4049.rst` documents
   both. The amendment says this in the design's *Rules* section.
5. **The two behaviour-change tables are incomplete.** The resolver as
   specified also rejects the following programs, which analyze cleanly today:
   - `d1 + u1` on `DINT` and `UDINT`, or any signed/unsigned pair where
     neither type widens to the other (REQ-AO-analyzer-004);
   - `d * 1.5`, a `DINT` times a real literal: `ANY_REAL` is not acceptable
     where `DINT` is expected, nor the other way round;
   - `2 * t`, `ANY_NUM` times `TIME` (REQ-AO-analyzer-008 says so, but the
     table does not list it);
   - `t1 / t2` on `TIME`, and `d1 + d2` on `DATE`: no Table 30 row covers
     either;
   - `w + i` with `w : WORD` and `i : INT`, and `b + s` with `b : BYTE` and
     `s : SINT`, **even in the `codesys`, `twincat` and `rusty` dialects**.
     Judged as `UINT` and `USINT`, the bit string widens to neither `INT` nor
     `SINT`, and the signed type does not widen to it either. The design's own
     example, `b + i` with `i : INT`, is accepted only because `USINT` widens
     to `INT`.

   Because the result type is now the wider operand rather than the left one,
   one more kind of program goes from clean to a different code:
   `i := i + d` with `i : INT` and `d : DINT` now resolves to `DINT`, and the
   assignment is reported as P4035 (narrowing).

   Finally, `r MOD r2` and `7.5 MOD 2.0` go from two P4049s (one per operand)
   to one. The amendment adds every one of these rows. Each row gets a test
   (see *Behaviour that changes* below).
6. **REQ-KF-analyzer-010 also has to be narrowed.** The design names only
   REQ-KF-analyzer-001 and REQ-KF-analyzer-005. But REQ-KF-analyzer-010
   ("an input beyond the second outside the category is P4026") covers
   `ADD(a, a, s)` and `MUL(a, a, s)`. Once the fold runs through the
   resolver, those calls report P4049. The amendment narrows REQ-KF-analyzer-010
   to `AND`, `OR` and `XOR`. Its test
   (`analyzer_spec_req_kf_010_third_input_outside_category_is_p4026`) drops
   `ADD` and `MUL`, and a new REQ-AO test covers them.
7. **Bit-string arithmetic and `MOD`.** The design applies the bit-string
   rule inside "step 2", which also runs for `MOD`. Only the four overloaded
   names skip the call rule, though. So with the flag on, `b MOD 2` would be
   accepted while `MOD(b, 2)` is still P4026. That breaks the Keyword
   Function Forms invariant that a function form accepts exactly what its
   operator accepts. **Decision:** the bit-string rule applies to the `ADD`,
   `SUB`, `MUL` and `DIV` rows only. ADR-0053 names only those four, and
   `b MOD 2` is rejected in every dialect today, so nothing regresses. The
   tracking issue records `MOD` on bit strings as a possible follow-up.
8. **`P4026.rst` does not list the arithmetic function forms today.** The
   design says the page "no longer lists" them, but it never did. The only
   change there is one sentence sending `ADD`/`SUB`/`MUL`/`DIV` operand
   mismatches to P4049.
9. **Some operands have no resolved type at all.** The probe below found
   four codegen tests (the array-of-struct field sums in
   `end_to_end_array_of_struct.rs` and `end_to_end_struct.rs`) whose
   operands have `resolved_type == None`. Step 1 of the design covers types
   the predicate "cannot judge", but not a missing type. **Decision:** a
   missing operand type is treated as `Unchecked`, keeping today's rule. The
   operator rule already skips such operands.
10. **Codegen's numeric path needs a rule for types it cannot place.**
    `op_type()` in `compile_expr.rs:34` falls back to the enum width (W32) for
    any name `resolve_type_name` does not know, including a subrange of
    `LINT`. **Decision:** codegen compiles an expression at its own resolved
    width only when that resolved type is a concrete elementary numeric or
    bit-string type. Any other expression (generic, unknown, subrange,
    unresolved) compiles at the enclosing operation type, as it does today.
11. **The function form needs the numeric-width rule too.** Otherwise
    `ADD(i, r)` into `REAL` still loads the `INT` at F32 (today's 1.5 bug),
    `ADD(i, r)` and `i + r` compute different values, and REQ-KF-codegen-001
    and REQ-KF-codegen-002 would hold only by accident. The design states the
    rule for "a numeric binary expression" only. **Decision:** each fold step
    of `compile_operator_form`'s arithmetic arm goes through the same
    per-step routine as `BinaryOp`. The amendment says so under *Codegen*.
    Codegen gets each step's result type from the resolver, called with the
    options the analyzer ran with: codegen already receives them through
    `SemanticContext::compiler_options()`, so the two passes cannot disagree.
    (An earlier version of this plan gave codegen a fixed option set and a
    test pinning that it agreed with the analyzer; reading the context showed
    neither is needed. PR 0, #1779, records this in the design.) The design's
    "the typed step takes no options" still holds: the typed step is a
    separate, public, option-free function.
12. **The documentation list misses one page, and a steering pointer is
    stale.** `docs/reference/language/structured-text/arithmetic-operators.rst`
    says the arithmetic operators "apply to integer types ... and
    floating-point types". It needs the time and date overloads and the
    mixed-type rule, and it is added to the file map. Separately, the
    `CLAUDE.md` pointer to
    `compiler/sources/resources/compat-libraries/` names a directory that
    does not exist; the bundled libraries live in
    `compiler/sources/resources/libs/`. That pointer is outside this work and
    goes in the tracking issue.

## Goal

Implement [Arithmetic Operator Overloads](../design/arithmetic-operator-overloads.md)
and close [#1621](https://github.com/ironplc/ironplc/issues/1621):

- `+ - * /` and `ADD SUB MUL DIV` accept exactly the IEC 61131-3 Table 24
  numeric overload and the Table 30 time/date overloads. The two spellings
  are checked by one rule (P4049) and compiled by one dispatch.
- Mixed-width numeric expressions compute at their resolved width
  (ADR-0001).
- Bit-string arithmetic sits behind `--allow-bit-string-arithmetic`, which
  the `rusty`, `codesys` and `twincat` dialects enable (ADR-0053).

## Architecture

The design's architecture is unchanged. One pure resolver,
`compiler/analyzer/src/intermediates/arithmetic_overload.rs`, answers
`Unchecked`, `Numeric` or `Typed`, or `None` when no overload applies:

- `xform_resolve_expr_types` asks it for the type of an arithmetic `BinaryOp`
  and of a call to one of the four overloaded names.
- `rule_operator_operand_type_check` asks it to report P4049.
- Codegen asks it, through the option-free typed step and the fixed-option
  numeric step, which routine and which width to compile.

The tree is never rewritten (REQ-AO-analyzer-023). The typed routines move to
their own codegen module and take `(left: Operand, right: &Expr, width)`.
`Operand` is either an expression to compile or a value already on the stack,
because an extensible fold carries its accumulated left operand on the stack.
Both spellings call these routines, and so do the direct calls to typed names
at both widths.

Pass order is unchanged. `compiler/analyzer/src/stages.rs` runs
`xform_resolve_expr_types` (line 280) and then
`xform_fold_constant_expressions` (line 285) inside `resolve_types`.
`semantic()` runs later: `rule_function_call_type_check` is at line 346 and
`rule_operator_operand_type_check` at line 351 of its function list. The
resolver needs resolved operand types (available after line 280). Folding
keeps each node's `resolved_type` (`xform_fold_constant_expressions.rs:69`,
`..node`), so the rules see the folded tree with types intact. The rules only
read the tree, so their relative order does not matter. **No change to
`stages.rs`.**

## Prefactoring

Each candidate the task named, with a verdict:

| Candidate | Verdict | Why |
|---|---|---|
| `xform_resolve_expr_types.rs` is 1660 lines | **Do first (P1)** | The production code is lines 1–632. The test module is lines 633–1660, 1027 lines. Moving the tests out, following the `xform_mark_unwritten_constants.rs` → `xform_mark_unwritten_constants/tests.rs` precedent, leaves 632 lines. Core 3 adds about 50: an `options` field on `ExprTypeResolver`, plus the `BinaryOp` and overloaded-`Function` arms calling the resolver. That lands near 690. The resolver itself goes in `intermediates/arithmetic_overload.rs` as the design says, so **nothing else has to move out of the pass**. The moved tests are themselves over 1000 lines, so they split in two. `tests.rs` keeps the helpers and the individual tests. `tests/single_assignment.rs` takes the `apply_when_single_assignment_then_resolves_expected_type` rstest case table (about 420 lines). |
| Typed time routines in `compile_call.rs` take `&Function` and hard-code W32 | **Do first (P2)** | This matches the "copy a function and change a few lines" signal: without it, Core 2 needs a second copy of each routine for the operator path and Core 1 needs a third for W64. P2 moves `compile_dt_time_add_sub`, `compile_sub_to_time` and `compile_mul_div_time`, plus the time use of `compile_two_arg_operator`, into a new `codegen/src/compile_time_arith.rs` with the signature `(emitter, ctx, left: Operand<'_>, right: &Expr, width: OpWidth)`. The call sites in `compile_function_call` extract the two arguments and pass `Operand::Expr(in1)` and `OpWidth::W32`, so the bytecode is identical and the existing tests pass unchanged. `compile_two_arg_operator` has no caller left once the time arms move, so it is deleted. `CONCAT_DATE_TOD` shares `compile_dt_time_add_sub` and moves with it. `compile_call.rs` is 1363 lines (production 1273), so this also shrinks a module that is already over the limit by about 150 lines. |
| `compile_expr.rs` `BinaryOp` arm (2003-line module) | **Do first (P2)** | The core change to that arm is about 40 lines of per-operand conversion. P2 moves the arm's body unchanged into `compile_arith::compile_binary_arith(emitter, ctx, binary, op_type)`. It also moves the arithmetic branch of `compile_operator_form` into `compile_arith::compile_arith_fold`. Core 2 and Core 3 then edit only the new module, and `compile_expr.rs` shrinks rather than grows. |
| `checked_form` in `rule_operator_operand_type_check.rs` | **Core 3, not a prefactor** | Deleting it changes behaviour: every arithmetic operator becomes checked. One behaviour-preserving step does go first, in P1. The rule moves its arithmetic check from `visit_binary_expr` to `visit_expr` so it has the whole expression's span for the one-per-expression label Core 3 needs. The diagnostics stay identical, because the label is still attached per operand in P1. |
| `rule_function_call_type_check` binds the four names to ANY_NUM | **Core 3, not a prefactor** | Stopping the input check is a behaviour change (REQ-AO-analyzer-032). It is one early `continue` in `visit_function` (`rule_function_call_type_check.rs:268`), and there is nothing to reshape first. |
| `OperatorFunctionForm` gains a typed-names column; its pinned-row test changes | **Core 2, not a prefactor** | The pinned test `operator_function_form_when_row_then_signature_is_derived_from_it` has to gain a `typed` case column, so the existing tests do not pass unchanged. That fails the prefactoring bar. The column and the test change land together with REQ-AO-analyzer-014. The other ten rows get `&[]`. |
| P4049 contexts `expected`/`actual` → `left`/`right` | **Core 3** | This is a message change. The tests that assert the old shape (found by grep over `P4049|OperatorOperandTypeMismatch|expected=|actual=`): `rule_operator_operand_type_check::tests::apply_when_mod_of_real_then_diagnostic_names_operator_and_types` (asserts `expected=ANY_INT` and `actual=real`); `apply_when_mod_of_real_variables_then_p4049_per_operand` and `apply_when_mod_of_real_literals_then_p4049_per_operand` (count goes 2 → 1); and `apply_when_add_of_time_variables_then_ok` (still clean, but its doc comment "Only MOD is checked" becomes false). The only non-test files naming P4049 are `P4049.rst`, `problem-codes.csv`, the KF and AO designs, ADR-0053 and a comment at `compile_expr.rs:1979`. No CLI, LSP, VS Code or snapshot test contains the message text: a grep for "not defined for the operand" finds only the CSV. The bit-string-family tests keep their shape (decision 4). |
| `get_time_functions` in `stdlib_function.rs` (1455 lines) | **Do first (P1)** | Core 1 adds eleven signatures to this function. P1 moves `get_time_functions` and its tests, unchanged, into `intermediates/stdlib_time_function.rs`. Core 1 then derives each long signature from its short row in that module. The shape is still stated once, and `stdlib_function.rs` shrinks. |
| `parser/src/options.rs` is 1013 lines | **Do first (P1)** | Core 3 adds a flag entry and three dialect-test lines. P1 moves the test module (lines 547–1013) to `parser/src/options/tests.rs`, following the same precedent. |
| `codegen/tests/it/common/mod.rs` is 999 lines | **No prefactor; constraint** | No new helper goes into it. The existing `assert_run_*_with` and `e2e_*_with!` helpers cover every new end-to-end test. |

`rule_function_call_type_check.rs` (1791 lines, production 295) and
`ironplc-cli/src/lsp.rs` (1590 lines) are also over the limit, but this work
adds no lines to either one, so they are left alone.

## Design doc reference

- [specs/design/arithmetic-operator-overloads.md](../design/arithmetic-operator-overloads.md), amended by PR 0
- [specs/adrs/0053-bit-string-arithmetic-behind-its-own-flag.md](../adrs/0053-bit-string-arithmetic-behind-its-own-flag.md), which flips to `accepted` in Core 3
- [specs/design/keyword-function-forms.md](../design/keyword-function-forms.md), updated in Core 3
- Issue [#1621](https://github.com/ironplc/ironplc/issues/1621), closed by Core 3

## PR sequence

Every PR comes from its own feature branch off `main` and is squash-merged.
None is pushed to `main`. Branch names are proposals.

| # | Branch | Kind | Lands | Requirement IDs |
|---|---|---|---|---|
| — | `claude/arithmetic-operator-overloads-plan-ls0ko6` | plan review (this PR) | nothing; never merged | — |
| 0 | `claude/ao-design-amendment` | specs only | the twelve amendments above, in the design and (for decision 7) in ADR-0053's *More Information* | — |
| P1 | `claude/ao-prefactor-module-sizes` | prefactor | the test-module moves for `xform_resolve_expr_types` and `options`, the `stdlib_time_function` move, and `visit_expr` in the operator rule | — (existing tests pass unchanged) |
| P2 | `claude/ao-prefactor-typed-routines` | prefactor | `compile_time_arith.rs`, `compile_arith.rs` | — (existing tests pass unchanged; bytecode identical) |
| — | — | **open the tracking issue** | — | — |
| C1 | `claude/ao-long-typed-functions` | core | the eleven long forms: registered, compiled at W64 by name, documented | none tagged (plain tests; see *Requirement tags* below) |
| C2 | `claude/ao-resolver-and-typed-dispatch` | core | the resolver, the table column, codegen typed dispatch for both spellings | tests for analyzer 001–014 and codegen 001–005, 009, written as plain `#[test]` |
| C3 | `claude/ao-check-and-numeric-width` | core | analyzer adoption, the rules, P4049, the flag, numeric width, docs, both `build.rs` entries | every REQ-AO-analyzer and REQ-AO-codegen ID becomes `#[spec_test]`; adds 020–035 and codegen 006–008 |

**Why this order.** Core 2 lands the codegen typed dispatch before Core 3
lets the analyzer accept `d1 - d2` on `DATE`. The other order would leave a
window in which `t := d1 - d2` goes from P4035 to silently wrong: the generic
path subtracts seconds and never multiplies by 1000. In Core 2 the analyzer
still rejects that program, so the new codegen path is reachable only through
the typed-name calls, through `ADD(t1, t2)`-style forms of pairs the analyzer
already accepts, and through tests that skip the semantic rules. Numeric width
(codegen 006–008) lands in Core 3, not Core 2, because it reads the
expression's resolved type, and that type is still "the left operand" until
Core 3 changes it. With the old rule, `i + r` would compile at `INT` width and
convert `r` to an integer.

**Requirement tags.** A crate's completeness meta-test fails as soon as its
`build.rs` lists a design with an untested owned requirement, and
`#[spec_test(REQ_AO_…)]` does not compile until the constant is generated. So
both `build.rs` entries, `compiler/analyzer/build.rs` and
`compiler/codegen/build.rs`, land in **Core 3**, the first PR in which every
REQ-AO requirement can be tested. Core 2 writes its conformance tests as
plain `#[test]` functions whose names carry the requirement number
(`…_req_ao_005_…`). Core 3 changes the attribute to `#[spec_test(…)]`, a
mechanical edit, and adds the rest.

## Tracking issue

Open it after P2 merges and before the Core 1 branch is created, titled
"Arithmetic operator overloads (design implementation)". It should contain:

- A link to the design, ADR-0053 and #1621. Say that #1621 closes with Core 3.
- The three core PRs as a checklist with their scope and requirement IDs (the
  table above), and links to the merged PR 0, P1 and P2.
- The behaviour changes users will see, copied from the amended design's
  tables, so the release notes can quote them. In particular: `b + 1` needs a
  dialect or `--allow-bit-string-arithmetic`; `w + i` (`WORD` + `INT`) is
  rejected even in the vendor dialects; `i := i + d` becomes P4035; and
  mixed-width values change.
- Follow-ups this work does not deliver, one line each:
  - `MOD` on bit strings under the flag (decision 7);
  - suppressing a second P4049 on the enclosing expression when an inner
    arithmetic expression already failed (see Core 3);
  - the stale `compat-libraries` path in `CLAUDE.md` (decision 12);
  - the modules that remain over the size limit (`compile_call.rs`,
    `compile_expr.rs`, `stdlib_function.rs`, `rule_function_call_type_check.rs`,
    `lsp.rs`);
  - the design's out-of-scope list (`**`, unary `-` on `TIME`, a strict
    same-type mode, `MULTIME`/`DIVTIME`), so it is not lost when this plan is
    deleted.
- A note that a program containing a user `FUNCTION ADD_LTIME` (or another
  long name) now collides with the standard library (P4016,
  `FunctionDeclNameDuplicated`), because the long forms are registered in
  every dialect, as the design specifies.

## File map

### specs/

- `specs/design/arithmetic-operator-overloads.md` — PR 0: the amendments. C3:
  a *Status* note if the design uses one; otherwise no change.
- `specs/adrs/0053-bit-string-arithmetic-behind-its-own-flag.md` — PR 0: a
  dated *More Information* postscript saying the rule excludes `MOD`
  (decision 7). C3: `status: proposed` → `accepted`.
- `specs/design/keyword-function-forms.md` — C3: rewrite the "only MOD"
  paragraph (lines 52–59); reword REQ-KF-analyzer-001 to "the numeric
  overload or one of the typed overloads of Arithmetic Operator Overloads";
  narrow REQ-KF-analyzer-005 to forms without overloads (`MOD`, the
  comparisons, `AND`, `OR`, `XOR`, `NOT`); narrow REQ-KF-analyzer-010 to
  `AND`, `OR` and `XOR` (decision 6).
- `specs/plans/2026-09-22-arithmetic-operator-overloads.md` — this file. It
  exists only on the plan branch and is never merged.

### compiler/parser

- `src/options.rs` — P1: test module out. C3: a `define_compiler_options!`
  entry, "Allow arithmetic on bit-string operands (BYTE + 1), treating a bit
  string as the unsigned integer of its width", `--allow-bit-string-arithmetic`,
  `[Rusty, Codesys, TwinCat]`, `allow_bit_string_arithmetic`.
- `src/options/tests.rs` — P1: new, the moved tests. C3: add
  `"allow_bit_string_arithmetic"` to the three `*_dialect_enables_exactly_these_flags`
  lists.

### compiler/ironplc-cli

- `bin/main.rs` — C3: a clap field `allow_bit_string_arithmetic` and its
  `|=` overlay in `compiler_options()`, next to lines 215 and 329.
- `src/lsp.rs` — **no change.** `extract_compiler_options` (line 66) iterates
  `CompilerOptions::FEATURE_DESCRIPTORS`, so the new flag is wired in
  automatically (verified). Its existing every-descriptor tests cover the new
  flag.
- `tests/cli.rs` — C3: `check_when_bit_string_arithmetic_and_default_dialect_then_err`,
  `check_when_bit_string_arithmetic_and_codesys_dialect_then_ok`, and
  `check_when_bit_string_arithmetic_and_allow_flag_then_ok`.
- `compiler/resources/test/bit_string_arithmetic.st` — C3: new shared
  resource, `b := b + 1;` on a `BYTE`.

### compiler/playground, compiler/mcp, integrations/vscode

- `compiler/playground/src/lib.rs` — **no change.** It builds options from
  `CompilerOptions::from_dialect` and looks flags up in `FEATURE_DESCRIPTORS`
  (line 72) (verified).
- `compiler/mcp/src/feature_flag_conformance.rs` — C3: a `FlagFixture` for
  `allow_bit_string_arithmetic`, with the source `b := b + 1;` on a `BYTE`.
  The `every_feature_flag_has_a_fixture` and
  `each_feature_flag_gates_its_example_source_off_then_on` tests require it
  (verified at lines 313 and 356).
- `integrations/vscode/` — **no change.** A grep finds no enumerated flag
  names in the extension.

### compiler/problems

- `resources/problem-codes.csv` — C3: P4049 message becomes "Operator is not
  defined for the operand types".

### compiler/analyzer

- `build.rs` — C3: add `"arithmetic-operator-overloads.md"` with a comment
  `// Arithmetic operator overloads (REQ-AO-analyzer-*).`
- `src/xform_resolve_expr_types.rs` — P1: `#[cfg(test)] mod tests;`. C3: an
  `options` field; the `BinaryOp` arm and the overloaded-`Function` arm
  ask the resolver and fall back to today's rule on `Unchecked` or `None`.
- `src/xform_resolve_expr_types/tests.rs` — P1: new, moved. C3: rename
  `case_13_mixed_type_binary_op_inherits_left_operand_type` to
  `…_resolves_to_wider_operand_type` (the expected type `DINT` is unchanged),
  and add an `INT + DINT` case expecting `DINT`.
- `src/xform_resolve_expr_types/tests/single_assignment.rs` — P1: new, moved.
- `src/intermediates/stdlib_time_function.rs` — P1: new, moved
  `get_time_functions` and its tests. C1: an `OVERLOAD_ROWS` data table of the
  eleven short rows, from which both widths' signatures are derived, plus
  `long_form(short: &str) -> Option<&'static str>`, an explicit eleven-arm
  match.
- `src/intermediates/stdlib_function.rs` — P1: the moved code removed; calls
  `stdlib_time_function::get_time_functions()`.
- `src/intermediates/mod.rs` — P1: `pub mod stdlib_time_function;`. C2:
  `pub mod arithmetic_overload;`.
- `src/intermediates/arithmetic_overload.rs` — C2: new.
  `pub enum Overload { Unchecked { result }, Numeric { result }, Typed { name: &'static str, long: bool, result } }`;
  `pub fn resolve_arithmetic_overload(op: &Operator, left: Option<&TypeName>, right: Option<&TypeName>, options: &CompilerOptions) -> Option<Overload>`;
  `pub fn typed_overload(op, left, right) -> Option<Overload>` (option-free);
  `pub fn resolve_arithmetic_fold(op, inputs: &[Option<&TypeName>], options) -> Result<Overload, FoldFailure>`,
  where `FoldFailure` names the failing step's left and right types for the
  diagnostic. Unit tests cover every rule branch; the
  REQ-tagged tests live in the spec conformance module.
- `src/intermediates/operator_function_form.rs` — C2: a `typed:
  &'static [&'static str]` column, a `form()` argument, a
  `pub fn typed_overloads(&self)` accessor, and the pinned-row test gaining a
  `typed` case column.
- `src/lib.rs` — C2: re-export `resolve_arithmetic_overload`,
  `typed_overload`, `Overload` beside the existing
  `operator_function_form` re-export (line 107).
- `src/rule_operator_operand_type_check.rs` — P1: `visit_expr`. C3: delete
  `checked_form`; check `+ - * / MOD` and calls to `ADD SUB MUL DIV` through
  the resolver; one P4049 per failing expression (or fold step) with
  `operator`/`left`/`right` contexts, labelled at the expression span (or the
  call name); update the module doc (drop the #1621 citation) and the four
  tests listed under *Prefactoring*.
- `src/rule_function_call_type_check.rs` — C3: skip the input-type loop
  (lines 268–286) when `operator_function_form(name)` has a non-empty
  `typed` column, that is, for the four overloaded names. Arity (P4018) is
  checked by `rule_function_call_declared` and is untouched.
- `src/spec_conformance_keyword_function_forms.rs` — C3: `req_kf_005` iterates
  the forms without overloads; `req_kf_010` iterates `AND`, `OR`, `XOR`;
  `req_kf_001` gains the Table 30 rows in function form.
- `src/spec_conformance_arithmetic_operator_overloads.rs` — C2: new (plain
  tests); C3: `#[spec_test]` plus 020–035.
- `src/lib.rs` (module list) — C2: `mod spec_conformance_arithmetic_operator_overloads;`
  under `#[cfg(test)]`, following the KF module.

### compiler/codegen

- `build.rs` — C3: add `"arithmetic-operator-overloads.md"` with a comment
  `// Arithmetic operator overloads (REQ-AO-codegen-*).`
- `src/compile_time_arith.rs` — P2: new, the moved routines with
  `(left: Operand, right: &Expr, width: OpWidth)`, and `pub(crate) enum
  Operand<'a> { Expr(&'a Expr), Stack(OpType) }`. C1: no change beyond
  receiving W64. C2: `compile_typed_overload(name, long, left, right)`, the
  one name→routine table both spellings and direct calls go through.
- `src/compile_arith.rs` — P2: new, `compile_binary_arith` and
  `compile_arith_fold` holding today's code. C2: ask `typed_overload` first.
  C3: the numeric-width rule (operands at their own width plus
  `emit_conversion_opcode`, the operation at the resolved width, the result
  converted to the enclosing width), per step, for both spellings.
- `src/compile_call.rs` — P2: the time arms call `compile_time_arith`, and
  the moved routines and `compile_two_arg_operator` are removed. C1: arms for
  `add_ltime`, `add_ltod_ltime`, `add_ldt_ltime`, `sub_ltime`,
  `sub_ldate_ldate`, `sub_ltod_ltime`, `sub_ltod_ltod`, `sub_ldt_ltime`,
  `sub_ldt_ldt`, `mul_ltime` and `div_ltime`, calling the same routines at
  W64.
- `src/compile_expr.rs` — P2: the `BinaryOp` arm delegates to
  `compile_arith::compile_binary_arith`.
- `src/lib.rs` — P2: `mod compile_arith; mod compile_time_arith;`.
- `src/spec_conformance_arithmetic_operator_overloads.rs` — C2: new
  (bytecode-equality tests, plain `#[test]`); C3: `#[spec_test]`.
- `tests/it/end_to_end_long_time_functions.rs` — C1: new, one VM case per
  long function.
- `tests/it/end_to_end_arithmetic_overloads.rs` — C2: new, VM value tests
  (plain); C3: `#[spec_test]` and the numeric value-change rows.
- `tests/it/main.rs` — C1 and C2: `mod` lines.

### docs/

- `docs/reference/standard-library/functions/add_ltime.rst`,
  `add_ltod_ltime.rst`, `add_ldt_ltime.rst`, `sub_ltime.rst`,
  `sub_ldate_ldate.rst`, `sub_ltod_ltime.rst`, `sub_ltod_ltod.rst`,
  `sub_ldt_ltime.rst`, `sub_ldt_ldt.rst`, `mul_ltime.rst`, `div_ltime.rst` —
  C1: new, each mirroring its short-form page (the "parallel reference
  pages" exemption in the duplication rule).
- `docs/reference/standard-library/functions/index.rst` — C1: table rows and
  toctree entries for the eleven.
- `docs/reference/standard-library/functions/add.rst`, `sub.rst`, `mul.rst`,
  `div.rst` — C3: an *Overloads* section listing the Table 30 rows, each
  linking to the typed page; replace "All inputs must share the same type"
  with the widening rule, linking to `type-conversions`.
- `docs/reference/compiler/problems/P4049.rst` — C3: rewritten for all
  arithmetic operators, both spellings, the two-operand message, and the
  bit-string family's unchanged per-operand shape.
- `docs/reference/compiler/problems/P4026.rst` — C3: one sentence sending
  arithmetic-form operand mismatches to P4049 (decision 8).
- `docs/reference/compiler/ironplcc.rst` — C3: an `--allow-bit-string-arithmetic`
  entry beside `--allow-cross-family-widening` (line 198).
- `docs/explanation/enabling-dialects-and-features.rst` — C3: the flag's
  section, and the three dialects' `**Enables:**` lists (lines 58, 93, 134).
  `docs/extensions/ironplc_flags.py` fails the docs build if these drift.
- `docs/explanation/type-conversions.rst` — C3: a bit-string arithmetic case
  after the cross-family tables (around line 297), plus the mixed-type
  arithmetic result rule.
- `docs/reference/language/structured-text/arithmetic-operators.rst` — C3:
  time and date overloads and the result-type rule (decision 12).

## Requirement coverage

**Analyzer.** Every REQ-AO-analyzer test lives in
`compiler/analyzer/src/spec_conformance_arithmetic_operator_overloads.rs`,
which follows the KF module's helpers (`analyze_codes`, `program`). Resolver
tests call `resolve_arithmetic_overload` directly on `TypeName`s, and
pipeline tests go through `stages::analyze` or `stages::resolve_types`.

| Req | PR | Test shape |
|---|---|---|
| 001 | C2 | rstest over the ten ANY_NUM types × four operators: `Numeric { result: T }` for `(T, T)` |
| 002 | C2 | rstest cases `(INT, DINT)→DINT`, `(DINT, INT)→DINT`, `(INT, REAL)→REAL`, `(UDINT, LINT)→LINT`, `(REAL, LREAL)→LREAL`, `(USINT, INT)→INT` |
| 003 | C2 | `(DINT, ANY_INT)→DINT`, `(ANY_INT, REAL)→REAL`, `(LREAL, ANY_INT)→LREAL` |
| 004 | C2 | `(DINT, REAL)`, `(DINT, UDINT)`, `(DINT, ANY_REAL)` → `None` (rows from decision 5) |
| 005 | C2 | rstest over the eleven Table 30 rows: `Typed { name, long: false, result }` equals the registered signature's return type |
| 006 | C2 | the same eleven rows at long width → the long name and not the short one |
| 007 | C2 | `(TIME, LTIME)`, `(LTIME, TIME)`, `(DATE_AND_TIME, LTIME)` → long form with long result |
| 008 | C2 | `MUL`/`DIV` of `TIME` by each ANY_NUM type and by `ANY_INT`/`ANY_REAL` resolve; `(ANY_INT, TIME)` and `(REAL, TIME)` do not |
| 009 | C2 | `(STRING, STRING)`, `(BOOL, BOOL)`, `(TIME, DATE)`, `(DATE, DATE)` for ADD, `(TIME, TIME)` for MUL/DIV → `None` |
| 010 | C2 | with the flag: `(BYTE, BYTE)→BYTE`, `(BYTE, WORD)→WORD`, `(BYTE, ANY_INT)→BYTE`, `(BYTE, INT)→INT`, `(BYTE, REAL)→REAL`; `(WORD, INT)` → `None`; `BOOL` still `None`; `MOD` on `BYTE` → `None` (decision 7) |
| 011 | C2 | the 010 pairs without the flag → `None` |
| 012 | C2 | a subrange name, an enum name, and a missing type (decision 9) → `Unchecked { result: left }`; asserts `!matches!(…, Numeric { .. })` |
| 013 | C2 | `resolve_arithmetic_fold(Add, [TIME, TIME, TIME])` is `Typed`; `[TIME, TIME, REAL]` fails at step 2 with `left=TIME, right=REAL` |
| 014 | C2 | for each row and each typed name, the name and `long_form(name)` are in `get_all_stdlib_functions()` with two inputs |
| 020 | C3 | `resolve_types` then read the assignment value's `resolved_type`: `i + r`→REAL, `t1 + t2`→TIME, `d1 - d2`→TIME, `lt + t`→LTIME |
| 021 | C3 | `SUB(d1, d2)` on DATE→TIME; `ADD(t1, t2, t3)`→TIME; `MUL(t, 2)`→TIME |
| 022 | C3 | `s1 + s2`→STRING; `p + 1` on a subrange → the subrange's elementary type, as today |
| 023 | C3 | a visitor over the resolved tree for `t1 + t2`, `ADD(t1, t2)`, `d1 - d2`: exactly one `BinaryOp` and one `Function` named `ADD`, and no `Function` named `ADD_TIME` or `SUB_DATE_DATE` |
| 024 | C3 | `2 + 3` still folds to 5 and `1 + 2.5` to 3.5, each with the same `resolved_type` as before this change |
| 030 | C3 | `t * r`: exactly one diagnostic, code P4049, described contains `operator=*`, `left=TIME`, `right=REAL` |
| 031 | C3 | rstest over `r MOD 2.0`, `t1 * 1.5`, `s1 + s2`, `x * x` on BOOL → contains P4049 |
| 032 | C3 | `MUL(t, r)` → P4049 with `operator=MUL`, `left=TIME`, `right=REAL`, and no P4026; `ADD(t1, t2, r)` → P4049 naming step 2 |
| 033 | C3 | rstest over `t1 + t2`, `tod + t`, `d1 - d2`, `t + lt`, `INT + DINT` → clean |
| 034 | C3 | rstest over the eleven rows in function form, both widths (long ones under `allow_long_time_types`) → clean |
| 035 | C3 | `ADD_TIME(t1, t2)` clean; `ADD_TIME(t1, r)` → P4026 |

**Codegen.** Bytecode-equality tests live in
`compiler/codegen/src/spec_conformance_arithmetic_operator_overloads.rs`,
following the KF module's `program_bytecode` helper. VM-value tests live in
`compiler/codegen/tests/it/end_to_end_arithmetic_overloads.rs`, using
`assert_run_*_with` and following the `REQ_PAB_codegen_*` tests in
`tests/it/end_to_end_partial_access.rs`, which carry `#[spec_test]` in the
integration crate through the `spec_requirements` module in `tests/it/main.rs`.

| Req | PR | Test shape |
|---|---|---|
| 001 | C2 | rstest over eleven rows × two widths: bytecode of `result := a OP b` equals bytecode of `result := TYPED(a, b)` |
| 002 | C2 | VM: `DT#2000-01-01-00:00:00 + T#1h` equals `ADD_DT_TIME(...)` equals the epoch seconds of 01:00 |
| 003 | C2 | VM: `T#1s * r` with `r := 1.5` → 1500 ms, equal to `MUL_TIME` |
| 004 | C2 | VM: `D#2000-01-02 - D#2000-01-01` → 86 400 000 ms |
| 005 | C2 | VM: `lt1 + lt2` past 2³¹ ms; `lt + t` with a negative `t` (sign-extension); `ldt - dt` with a date after 2038 (zero-extension, decision 2) |
| 006 | C3 | rstest over every numeric type `T`: the bytecode of `x : T := a + b` with `T` operands is byte-identical to a golden captured on `main` before C3 (the golden is committed in the test as `assert_bytecode!` output) |
| 007 | C3 | VM: `x : REAL := i + r` → 4.5; `x : LINT := u + l` → 4000000001 |
| 008 | C3 | VM: `l : LINT := d1 * d2` with `d1 = d2 = 100000` → the 32-bit wrapped product 1410065408, widened |
| 009 | C2 | bytecode of `ADD(t1, t2, t3)` equals that of `t1 + t2 + t3`; VM value equal |

PR 0 (#1779) added requirements while amending the design. Their tests:

| Req | PR | Test shape |
|---|---|---|
| analyzer-015 | C2 | with the flag: `(BYTE, ANY_INT)` for `MOD` → `None`; C3 adds a pipeline case under `Dialect::Codesys` asserting P4049 for `b MOD 2` and P4026 for `MOD(b, 2)` |
| analyzer-036 | C3 | `t * r` gives exactly one P4049 with `left`/`right`; `d1 AND d2` on `DINT` still gives two with `expected=ANY_BIT` |
| codegen-010 | C3 | VM: `d : DINT := u1 / u2` with `4000000000` and `2` → 2000000000 |
| codegen-011 | C3 | VM: `x : REAL := ADD(i, r)` → 4.5; bytecode of `ADD(i, r)` equals that of `i + r` |
| codegen-012 | C3 | bytecode of `p + 1` on a subrange of `LINT` is byte-identical to a golden captured before C3 |

## Behaviour that changes: where each row is tested

Each clean → reported row is tested through the full `stages::analyze`
pipeline in the analyzer spec module. Codegen integration tests use
`resolve_types` and skip the semantic rules, so they cannot see P4049. Value
rows are VM tests in `end_to_end_arithmetic_overloads.rs`.

| Row (amended design table) | Test |
|---|---|
| `b + 1` on BYTE, strict dialect → P4049 | analyzer REQ-011 pipeline case; `cli.rs` `…_default_dialect_then_err` |
| `b + 1` under `rusty`, `codesys`, `twincat` → clean, correct | analyzer rstest over the three `Dialect`s; `cli.rs` `…_codesys_dialect_then_ok`; VM `BYTE#255 + 1` → 0 under `Dialect::Codesys` |
| `s1 + s2` on STRING → P4049 | REQ-031 |
| `x * x` on BOOL → P4049 | REQ-031 |
| `t1 * t2` on TIME → P4049 | REQ-009 pipeline case |
| `r + d` on REAL, DINT → P4049 | REQ-004 pipeline case |
| `d1 + u1` (signed/unsigned) → P4049 *(added)* | REQ-004 pipeline case |
| `d * 1.5` → P4049 *(added)* | REQ-004 pipeline case |
| `2 * t`, `t1 / t2`, `d1 + d2` → P4049 *(added)* | REQ-008 and REQ-009 pipeline cases |
| `w + i` (WORD, INT) under `codesys` → P4049 *(added)* | REQ-010 pipeline case under `Dialect::Codesys` |
| `i := i + d` (INT, DINT) → P4035 *(added)* | `apply_when_narrower_target_of_widened_sum_then_p4035` in the analyzer spec module |
| `r MOD r2` → one P4049 instead of two *(added)* | the updated `apply_when_mod_of_real_variables_then_p4049` in `rule_operator_operand_type_check.rs` |
| `x : REAL := i + r` → 4.5 | codegen REQ-007 |
| `x : LINT := u + l` → 4000000001 | codegen REQ-007 |
| `l : LINT := d1 * d2` → 32-bit product | codegen REQ-008 |
| `d := u1 / u2` (UDINT into DINT) → 2000000000 *(added, decision 3)* | VM case in `end_to_end_arithmetic_overloads.rs` |
| keeps working: `t1 + t2`, `t + lt`, `lt + LTIME#1s`, `dt + t`, `tod - tod`, `ADD_TIME(t1, t2)` | REQ-033/035 (analysis) and codegen REQ-002/005 (value) |

### Existing tests and libraries affected in a strict dialect (measured, not guessed)

A throwaway worktree built with a probe logged the operator and operand types
of every arithmetic `BinaryOp` and every `ADD/SUB/MUL/DIV/MOD` call as
`xform_resolve_expr_types` resolved it. The probe sat in that pass because
the codegen tests call `resolve_types` and skip the semantic rules. The whole
workspace was then run: `cargo test --workspace`, 3613 expressions. This
checkout is unmodified. The results:

- **Bundled compatibility libraries** (`compiler/sources/resources/libs/`,
  exercised by `end_to_end_tc2_utilities` and `end_to_end_tc2_math`, 2572
  expressions): **none affected.** All arithmetic is same-type
  (`LREAL`/`LINT`/`INT` with integer literals). There is no
  `compiler/sources/resources/compat-libraries/` directory.
- **Bit-string arithmetic:**
  `codegen/tests/it/end_to_end_bitstring.rs::end_to_end_when_byte_arithmetic_then_truncates`
  (`BYTE + ANY_INT`). It is unaffected, because it runs through
  `resolve_types`, which does not run P4049. C3 adds a sibling test through
  the full pipeline under the default dialect that asserts P4049 (the table
  above). The rest are the KF conformance matrices (`req_kf_005`,
  `req_kf_007`) over `BYTE`/`WORD`/`DWORD`/`LWORD`, handled in the KF test
  changes.
- **Temporal operators:**
  `rule_operator_operand_type_check::tests::apply_when_add_of_time_variables_then_ok`
  (stays clean) and `end_to_end_ltime::case_3_addition` (`LTIME + LTIME`,
  which now goes through `ADD_LTIME` and gives the same value). The rest are
  the KF 005 and 007 matrices.
- **Mixed numeric types:**
  `xform_resolve_expr_types` `case_13_mixed_type_binary_op_inherits_left_operand_type`
  (`DINT + INT`, expected type unchanged, renamed),
  `end_to_end_global::end_to_end_when_multiple_globals_then_all_accessible`
  and `end_to_end_struct::end_to_end_when_struct_field_write_int_then_correct_value`
  (`INT + DINT` into `DINT`). The last two share op width and signedness, so
  their values and bytecode are unchanged. The operator-rule MOD tests with
  `DINT`/`REAL` are already P4049.
- **Unchecked or missing types:** `end_to_end_subrange` (subrange + literal),
  and four array-of-struct sums with unresolved operand types (decision 9).
  All are unaffected.
- **Docs examples:** a grep for assignments with a binary arithmetic operator
  over `docs/**/*.rst` found 72 lines, all same-type. None is affected.
  `ltime.rst`'s `c := a + b` on `LTIME` keeps its value.

The probe could not see the assignment target, so it cannot rule out an
existing `x : DINT := u1 / u2`-style signedness case (decision 3). C3's full
`cd compiler && just` is the check for that. Any test that fails there is
listed in the C3 PR description with the reason its expected value changed.

## Tasks

### Plan review (this PR, never merged)

- [ ] Commit this plan as the only commit on
      `claude/arithmetic-operator-overloads-plan-ls0ko6` and open a review PR.
- [ ] Revise until approved.

### PR 0 — design amendment (`claude/ao-design-amendment`)

Opened as #1779.

- [x] Amend `specs/design/arithmetic-operator-overloads.md` for decisions 1–12:
      the long-form registration sentence (now "this change registers"),
      REQ-AO-codegen-005 wording (widening by signedness), REQ-AO-codegen-006
      wording (width and signedness), the P4049 bit-string-family paragraph,
      the added behaviour-change rows, the REQ-KF-analyzer-010 narrowing, the
      `MOD` exclusion, the P4026 sentence, missing types as `Unchecked`, the
      codegen conditions for the numeric path, the per-step function-form
      rule with the analyzer's options, the `Operand` left-on-stack shape, and
      `arithmetic-operators.rst`. Keep the requirement IDs stable and add new
      ones only in the gaps.
- [x] Add the dated postscript to ADR-0053 (the `MOD` exclusion).
- [x] `cd specs && just`. `cd compiler && just`: no crate lists the design
      yet, so this is a sanity run.

### PR P1 — module-size prefactor (`claude/ao-prefactor-module-sizes`)

Merged as #1775.

- [x] Move the `xform_resolve_expr_types` test module to
      `xform_resolve_expr_types/tests.rs` and
      `xform_resolve_expr_types/tests/single_assignment.rs`, both under 1000
      lines.
- [x] Move `get_time_functions` and its tests to
      `intermediates/stdlib_time_function.rs`.
- [x] Move the `options.rs` tests to `parser/src/options/tests.rs`.
- [x] Move the operator rule's arithmetic and compare checks to `visit_expr`,
      keeping the diagnostics byte-identical.
- [x] Each move is its own commit. The tests pass unchanged; only `use`
      paths may change.
- [x] `cd compiler && just`, `cd specs && just`.

### PR P2 — codegen prefactor (`claude/ao-prefactor-typed-routines`)

Merged as #1776. One departure: the routines take `(in1, in2)` with no
`width` and no `Operand` yet. Every caller would have passed W32 and the W64
sequences do not exist, so C1 adds `width` with the W64 code, and C2 adds
`Operand` with the fold that needs it. A name-to-routine table,
`time_arith_for`, landed here instead of in C2.

- [x] Create `compile_time_arith.rs` with `Operand` and the four routines
      taking `(left, right, width)`. Point `compile_function_call` at them
      with `Operand::Expr(in1)` and `W32`. Delete `compile_two_arg_operator`.
- [x] Create `compile_arith.rs` with `compile_binary_arith` (the `BinaryOp`
      arm body) and `compile_arith_fold` (the arithmetic arm of
      `compile_operator_form`).
- [x] Verify that the bytecode is unchanged: every existing
      `assert_bytecode!` and `compile_*` test passes unmodified.
- [x] `cd compiler && just`, `cd specs && just`.

### Tracking issue

- [ ] Open it with the contents listed under *Tracking issue*, and link it
      from the C1, C2 and C3 PR descriptions.

### PR C1 — long typed functions (`claude/ao-long-typed-functions`)

- [ ] In `stdlib_time_function.rs`, turn the eleven short signatures into an
      `OVERLOAD_ROWS` table and derive both widths from it. Add `long_form()`.
- [ ] Add the eleven `compile_call.rs` arms at W64, and make
      `compile_time_arith` handle W64. In `compile_mul_div_time` at W64 the
      float path converts with `CONV_I64_TO_F64` and `CONV_F64_TO_I64`, which
      already exist.
- [ ] `tests/it/end_to_end_long_time_functions.rs`: one VM case per long
      function under `allow_long_time_types`, including a short-width second
      operand and a date after 2038.
- [ ] Analyzer unit test: each long name is registered with the long-typed
      parameters and return type.
- [ ] The eleven doc pages and `functions/index.rst`.
- [ ] `cd compiler && just`, `cd specs && just`, `cd docs && just`.

### PR C2 — resolver and typed dispatch (`claude/ao-resolver-and-typed-dispatch`)

- [ ] `arithmetic_overload.rs`: `Overload`, `resolve_arithmetic_overload`,
      `typed_overload`, `resolve_arithmetic_fold`, and unit tests.
- [ ] Add the typed-names column to `operator_function_form.rs` and update
      the pinned-row test.
- [ ] Re-export the resolver from `lib.rs`.
- [ ] `compile_time_arith::compile_typed_overload`. `compile_binary_arith`
      and `compile_arith_fold` ask `typed_overload` first, fold with the
      accumulated type as `Operand::Stack`, and otherwise keep today's path.
- [ ] Analyzer conformance module with plain tests for 001–014, and codegen
      conformance and VM tests for 001–005 and 009.
- [ ] Codegen reads the analyzer's options from the `SemanticContext` it is
      given, for the numeric step.
- [ ] `cd compiler && just`, `cd specs && just`.

### PR C3 — analyzer adoption, flag, numeric width (`claude/ao-check-and-numeric-width`)

- [ ] Flag: `options.rs` entry, dialect lists in `options/tests.rs`,
      `bin/main.rs`, MCP fixture, CLI tests and resource.
- [ ] `xform_resolve_expr_types`: the `options` field and the resolver in
      the `BinaryOp` and overloaded-`Function` arms. Update the tests.
- [ ] `rule_operator_operand_type_check`: delete `checked_form`; resolver
      checks for both spellings; the new contexts; one diagnostic per
      expression; module doc; the four existing tests. A nested failure can
      produce a second P4049 on its enclosing expression (for example
      `(s1 + s2) * 2`). This is accepted and noted in the tracking issue.
- [ ] `rule_function_call_type_check`: skip the input check for the four
      names.
- [ ] `compile_arith`: the numeric-width rule per step, under the conditions
      of decision 10.
- [ ] `problem-codes.csv` message.
- [ ] Add the analyzer tests for 020–035 and the codegen tests for 006–008.
      Switch every REQ-AO test to `#[spec_test]`. Add
      `"arithmetic-operator-overloads.md"` to both `build.rs` files.
- [ ] KF: the design text and `spec_conformance_keyword_function_forms.rs`
      changes (005, 010, 001).
- [ ] Docs: P4049, P4026, `ironplcc.rst`, `enabling-dialects-and-features.rst`,
      `type-conversions.rst`, `add/sub/mul/div.rst`, `arithmetic-operators.rst`.
- [ ] ADR-0053 → `accepted`. The PR body says "Closes #1621".
- [ ] `cd compiler && just`, `cd specs && just`, `cd docs && just`.

### Cleanup

- [ ] Close the tracking issue once C3 merges, and discard the plan branch.

## Verification

Every PR ends with `cd compiler && just` and `cd specs && just` passing.
Beyond that:

- **`cd docs && just`** for C1 and C3. C1 adds eleven pages and toctree
  entries, and a missing entry fails Sphinx. C3 adds a flag: the
  `ironplc_flags.py` extension fails the build if a flag is missing from
  `ironplcc.rst` or `enabling-dialects-and-features.rst`, or if a dialect's
  `**Enables:**` list disagrees with `options.rs`. C3 also rewrites P4049,
  whose `problem-summary` directive reads `problem-codes.csv`. PR 0, P1 and
  P2 change no docs.
- **No playground build.** `compiler/playground` is a workspace member, so
  `cd compiler && just` already compiles and tests it. It reads flags from
  `FEATURE_DESCRIPTORS` and needs no code change. The `playground/` frontend
  is untouched.
- **No VS Code extension checks.** No extension file changes, and the
  problem-code list it syncs against gains no new code (P4049 already
  exists).
- **C3 only:** read the diff of every changed expected value from the full
  run. Any test whose expected number changed must match a row of the
  amended behaviour-change tables. A change that matches no row stops the
  PR until the design is amended.
