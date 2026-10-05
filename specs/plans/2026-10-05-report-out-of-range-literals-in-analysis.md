# Report every out-of-range literal in analysis

Plan for #2071.

## Goal

Analysis reports every integer or bit-string literal that does not fit its
type (P2026), so `ironplcc check` and the language server show what today only
`ironplcc compile` finds. Codegen's ten `Problem::ConstantOverflow` sites then
become internal errors (P9998), and P2026 is no longer a check only codegen
makes.

## Why

`rule_constant_range` cannot see the type a literal is compiled at, so it
predicts it: it pushes the destination's type down through operators. The
prediction misses generic arguments, `FOR` bounds, conditions, shift counts
and an untyped `CASE` selector. #2113 and #2119 record that type on every
literal, so the rule can read it instead.

Every program in the issue's table passes `check` and fails `compile` on
`main` at `e5c4218`. A prototype of the checks below, not committed, makes
each of them report P2026 from `check`, and `DINT#300 < s` still reports. The
prototype also gave the list of newly rejected programs below.

## Findings that shape the design

Each was confirmed with `ironplcc check`, `compile` and `echo --types` on
`main`.

1. **The recorded type is sometimes wider than the type as written.** Reading
   only the recorded type would lose these P2026 reports, each of which
   `check` makes today:
   - `x := 20` with `x : Small` and `Small : INT(0..10)`. The literal is
     recorded as `INT`, the subrange's base type (REQ-IC-analyzer-056).
   - `c(CU := TRUE, PV := 40000)` with `c : CTU`. A standard block's input is
     recorded as `DINT`, the default slot type (REQ-IC-analyzer-054), but `PV`
     is an `INT`.
   - `h(300, 1.0)`, a positional function block call. Its literals have no
     recorded type yet (`? ANY_INT`); #2050 is still recording function block
     and method arguments.
   - `s := DINT#300` with `s : SINT`. The literal keeps its own type, `DINT`,
     and the pass wraps it in a conversion to `SINT` (REQ-IC-analyzer-066).
   - `DINT#300 < s` with `s : SINT` (ADR-0056, REQ-IC-analyzer-009). The pass
     wraps `s` in a conversion to `DINT`.
2. **A bitwise operator narrows a wider literal without recording a
   conversion.** `AND`, `OR` and `XOR` take the type of their left operand, so
   `w XOR LWORD#16#FFFFFFFFF` on a `DWORD` is typed `DWORD`. The `LWORD`
   literal is not wrapped in a conversion, and codegen compiles it at 32 bits.
   `check` accepts this and `compile` fails with P2026. The literal fits its
   own type, `LWORD`, so once codegen's check is an internal error the program
   would get P9998. So a literal operand of a bitwise operator is also checked
   against the operator's type. Once #2050 records conversions for bitwise
   operands, the conversion check below covers this and the special case goes.
3. **The rule would report most literals twice.** In `x := 300` on a `USINT`,
   the recorded type and the destination are both `USINT`. The prototype ran
   both checks and reported it twice. Decision 1 makes it once.
4. **A real literal passed to an `LREAL` parameter is recorded as `REAL`.**
   With `p : LREAL`, `WIDE(1.0E300)` records `1.0E300` as a `REAL` converted
   to `LREAL` (the Arguments postscript of ADR-0056), and compiles to
   infinity today. This plan does not change which real literals are checked,
   so that program stays accepted. The PR reports it for a follow-up issue
   rather than fixing it, because the fix changes bytecode.
5. **The codegen test harness already refuses a program analysis rejects.**
   `compile_analyzed` goes through `CleanAnalysis` (#2117) and panics on any
   diagnostic. The issue's step of running the suite once with `parse`
   asserting that the rule reports nothing is therefore done by every run,
   and each newly rejected workspace program shows up as a failing test.
6. **A literal with no recorded type does not reach an overflow site.** A
   positional or named function block argument and a function block instance
   initializer each compile `5000000000` into a `LINT` input today. So after
   core PR 1, every literal that reaches `compile_constant` either fits its
   recorded type or has none and is checked against its destination.

## Architecture

### Where the rule runs

`rule_constant_range` leaves the list in `stages::semantic`. `stages::analyze`
runs it right after `xform_insert_implicit_conversions`, on the library the
pass returns, where every recorded type exists. Review of #2071 accepted
running a rule after the pass. Each check reads one of two things:

- **The recorded type.** This is a literal's `expr_type`: the type #2113
  recorded for an untyped integer literal, or the prefix of a prefixed or
  bit-string literal. When the pass wraps a literal in a conversion, the
  conversion's type is recorded too.
- **The type as written.** This is the declared type of a destination or of
  the other operand. It is read through an `ImplicitConversion`, whose inner
  expression keeps its own type (ADR-0056), and an untyped literal has no type
  as written.

Reading through conversions keeps the rule correct if the pass later moves
before the rules, which review also suggested. ADR-0056 found that running the
pass first lost `DINT#300 < s`. Reading through the conversion fixes that.

### The checks

| Check | Reads | Example | Today |
|---|---|---|---|
| **Own type**: an integer or bit-string literal in an expression is a value of its recorded type | recorded | `ADD(d, 5000000000)`, `FOR i := 0 TO 5000000000`, `SHL(w, 5000000000)`, `x := 300` on `USINT` | push-down, partly |
| **Conversion**: a literal the pass wrapped fits the conversion's type | recorded | `s := DINT#300` on `SINT`; `d < UDINT#4000000000` | push-down |
| **Bitwise operand**: a literal operand of `AND`, `OR` or `XOR` fits the operator's type (finding 2) | resolved | `w XOR LWORD#16#FFFFFFFFF` on `DWORD` | not checked |
| **Prefix**: a prefixed literal anywhere, including an initializer, fits its prefix | as written | `d : DINT := INT#40000`; newly `BYTE#256` | integer prefixes only |
| **Destination**: a constant stored directly (an initializer, an assignment's value, a function or function block argument) fits the declared type of the place it is stored in | as written | `x := 20` on `INT(0..10)`; `PV := 40000` on `CTU`; `h(300, ...)` | push-down |
| **Comparison**: a literal compared with an operand of another type fits that operand's type | as written | `DINT#300 < s` on `SINT` | unchanged |
| **`CASE`**: each label fits the selector's type | as written; recorded for an untyped literal selector | `CASE x OF 200:` on `SINT`; `CASE 5 OF 4294967295:` | untyped selector not checked |
| **Real**: an untyped real literal recorded as `REAL` is a `REAL`, unless the pass wrapped it in a conversion (finding 4) | recorded | `x := y + 1.0E300` | push-down |

**The push-down goes.** A destination check no longer descends through
operators, because each operand's recorded type now says what the operator
computes at. Constant folding still turns `255 + 1` into `256` before any rule
runs, so `x := 255 + 1` on a `USINT` is still caught, by the destination and
own-type checks. The push-down did find one P2026 the new checks do not: an
operand of a type other than the destination's, such as `s := x + 300` with
`x : INT` and `s : SINT`. The operation computes at `INT`, and 300 is an
`INT`. That program is still rejected, with P4035 for the narrowing
assignment.

### Bit strings

`value_range::of` returns the unsigned range of a bit string's width, reversing
the exemption in the rule's module doc and in `P2026.rst`. A constant is not a
run-time value: `BYTE#256` is not a byte. Wrapping at run time (`b := b + 1`
with `allow_bit_string_arithmetic`) is unaffected. `rule_constant_range` is the
only caller of `value_range::of`.

## Decisions for review

1. **One report per literal.** The issue says no literal is reported twice. One
   existing test, `apply_when_prefixed_literal_fits_neither_then_err_for_each`,
   expects two reports for `x : SINT := INT#40000`: one for the prefix and one
   for the destination. **Proposal:** report a literal once, for the first
   check it fails, checking its own type before the type as written. That test
   then expects one report, against `INT`, and `P2026.rst` drops "reported once
   for each". **Alternative:** report once per distinct range. That keeps the
   test, and still removes the duplicates of finding 3.
2. **Run the whole rule after the pass, rather than split it across the
   pass.** The issue proposes keeping the as-written checks before the pass.
   Those checks and the recorded-type checks would then report independently,
   and neither could tell whether the other had already reported a literal
   (finding 3). Running the whole rule after the pass, with the as-written
   checks reading through conversions, keeps one place that knows what was
   reported.
3. **An ADR for the bit-string reversal.** **Proposal:** a new ADR-0057,
   "Constants of bit-string types are range-checked". **Alternative:** a
   postscript on ADR-0053 (bit-string arithmetic). That ADR is about run-time
   arithmetic, which does not change, so a new ADR reads better.

## Programs newly rejected

These all compile today and are rejected with P2026 after core PR 1:

- **Bit-string constants:** `b : BYTE := 256`, `b := BYTE#256`, and, with
  `allow_int_literal_to_bit_string`, `x := 255 + 1` on a `BYTE`.
- **A literal outside the type it is computed at:** `FOR s := 0 TO 300`,
  `ADD(s, 300)`, `MAX(300, 1)`, `MOVE(300)`, `ABS(300)` and `MUX(0, 300, 1)`,
  each on a `SINT`. Each stores a truncated value today.
- **A literal operand of a bitwise operator outside the operator's type:**
  `w := w AND DWORD#16#1FFFF` on a `WORD`, and `b := b AND WORD#16#1FF` on a
  `BYTE`. Each silently drops the high bits today.

The rest of the issue's table already fails `compile` today, so analysis
reports it earlier rather than rejecting something new. That includes
`d : DWORD := -1`, which the issue lists among programs that compile today:
on `main` it fails `compile` with P2026.

In the workspace, the prototype ran `cargo test --workspace` and `ironplcc
check` on every `.st` file in the repository, under the default, `twincat` and
`codesys` dialects. It also checked all 186 `playground` and
`playground-with-program` examples in `docs/`. It newly rejects only these
tests' programs:

| Test | Program | Rework |
|---|---|---|
| `compile_case.rs` `compile_when_case_label_does_not_fit_literal_selector_then_constant_overflow` | `CASE 5 OF 4294967295` | Delete; its program becomes a rule test |
| `compile_const_trunc.rs` `compile_when_constant_out_of_range_then_folded_not_truncated` | `x := BYTE#300` | Delete; the in-range fold tests beside it remain |
| `end_to_end_bitstring.rs` `end_to_end_when_byte_overflow_then_wraps` | `x := 255 + 1` on `BYTE` | Wrap at run time: `a := 255; x := a + 1` |
| `end_to_end_bitstring.rs` `end_to_end_when_word_overflow_then_wraps` | `x := 65535 + 1` on `WORD` | Same |
| `end_to_end_const_trunc.rs` `end_to_end_when_byte_overflow_then_folded_matches_computed` | `folded := 300` on `BYTE` | Drop the folded half; keep the run-time `live := a + a` |
| `end_to_end_const_trunc.rs` `end_to_end_when_word_overflow_then_folded_matches_computed` | `folded := 80000` on `WORD` | Same |
| `end_to_end_type_alias.rs` `end_to_end_when_type_alias_byte_truncation_then_correct` | `x := 300` on `MyByte : BYTE` | Wrap at run time: `a := 150; x := a + a` |

Four analyzer tests state the old exemption and flip:
`apply_when_bit_string_overflows_then_ok` (three cases) and
`value_range::of_when_bit_string_then_none`. No `.st` file and no
documentation example is newly rejected.

## Prefactoring

**Prefactor PR: run `rule_constant_range` on the library the pass returns.**
No diagnostic changes:

- `stages::analyze` runs the rule after `xform_insert_implicit_conversions`.
- Every read of a type as written goes through one helper. It unwraps an
  `ImplicitConversion` and gives an untyped literal no type. The push-down
  descends through a conversion as it does through parentheses.
- `test_helpers` gains a variant of `rule_diagnostics` that lowers the library
  before running the rule. `rule_constant_range`'s tests use it and are
  otherwise unchanged.
- REQ-IC-analyzer-009 and "The pass" in `specs/design/implicit-conversions.md`
  say that this rule runs after the pass and reads the operands as written
  through a conversion. ADR-0056 gains a postscript saying the same. The
  REQ-IC-analyzer-009 test runs `analyze`, so it passes unchanged.

One thing can change: the rule's reports now come after those of every other
rule. Nothing sorts diagnostics, so any test that asserts an order across rules
is updated in this PR and named in its description.

## Design doc reference

`specs/design/implicit-conversions.md` (The pass; Literals) and ADR-0056. No new
design document: the rule's module doc and
`docs/reference/compiler/problems/P2026.rst` describe what it checks.

## File map

- `compiler/analyzer/src/stages.rs`: run the rule after the pass.
- `compiler/analyzer/src/rule_constant_range.rs`: the checks above, the
  as-written helper, one report per literal, and the module doc.
- `compiler/analyzer/src/rule_constant_range/tests.rs`: tests for every program
  in the issue's table, every newly rejected program, and the findings.
- `compiler/analyzer/src/value_range.rs`: bit-string ranges and their tests.
- `compiler/analyzer/src/test_helpers.rs`: the lowering variant of
  `rule_diagnostics`.
- `compiler/codegen/src/compile_expr.rs`: the nine sites in `compile_constant`.
- `compiler/codegen/src/compile_stmt.rs`: `CaseLabelValue::overflow`.
- `compiler/codegen/tests/it/`: the seven tests in the table above.
- `docs/reference/compiler/problems/P2026.rst`.
- `specs/design/implicit-conversions.md`, ADR-0056, and the new ADR-0057.

## Tasks

### Prefactor PR: run `rule_constant_range` after the conversions pass

- [ ] Move the rule from `stages::semantic` to after
      `xform_insert_implicit_conversions::apply` in `stages::analyze`.
- [ ] Route every as-written type read through one helper that unwraps an
      `ImplicitConversion` and gives an untyped literal no type.
- [ ] Add the lowering variant of `rule_diagnostics` and use it in the rule's
      tests.
- [ ] Update REQ-IC-analyzer-009, "The pass" and ADR-0056.
- [ ] `cd compiler && just`. Every existing test passes unchanged, apart from
      any that assert diagnostic order across rules, which the PR names.

### Core PR 1: check each literal against its recorded type

- [ ] `value_range::of` gives bit strings their range.
- [ ] Add the own-type, conversion and bitwise-operand checks. Extend the
      prefix check to bit-string prefixes. Check the labels of an untyped
      literal `CASE` selector against its recorded type. Move the real check to
      the recorded type.
- [ ] Stop destination checks descending through operators.
- [ ] Report each literal once (decision 1).
- [ ] Rule tests (`rule_err!`, `rule_err_at!`, `rule_ok!`) for every program in
      the issue's table, every newly rejected program, the cases in finding 1,
      `w XOR LWORD#16#FFFFFFFFF`, and `WIDE(1.0E300)` staying accepted.
- [ ] Rework the seven codegen tests above.
- [ ] Module doc, `P2026.rst`, the design doc and ADR-0057.
- [ ] `cd compiler && just` and `cd specs && just`. The PR lists every newly
      rejected program and reports finding 4 as a follow-up issue.

### Core PR 2: make codegen's `ConstantOverflow` sites internal errors

- [ ] Make each of the ten sites a `Diagnostic::internal_error_at`, with a
      comment naming `rule_constant_range`, as #2081 did for `EXIT` and
      `CONTINUE`.
- [ ] `git grep "Problem::ConstantOverflow" -- compiler/codegen/src` finds
      nothing.
- [ ] `cd compiler && just`. Only error paths change, so the bytecode of every
      program that compiles is unchanged.

## Out of scope

- Which real literals are checked, and the `REAL`-to-`LREAL` argument
  (finding 4).
- The bitwise operator's type (finding 2). Recording its operands'
  conversions belongs to #2050.
- Codegen's compile-time truncation fold. Once every constant is checked, its
  wrapping branch is unreachable from an analyzed program. Removing it is a
  separate change.
- #2129, which changes the types the pass records for negation, `NOT`, `ABS`
  and the shifts. The own-type check reads whatever is recorded, so expect a
  merge conflict but no change of design.
