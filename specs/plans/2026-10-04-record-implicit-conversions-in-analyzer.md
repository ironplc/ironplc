# Record Every Implicit Conversion and Literal Type in the Analyzer

Tracking issue: https://github.com/ironplc/ironplc/issues/2050

## Goal

Codegen never chooses an implicit conversion or the type of an untyped
literal. The analyzer records both in the analyzed `Library`, as ADR-0056
already does for the operands of a comparison, and codegen compiles what is
recorded. This completes the plan ADR-0056 states: "Arithmetic operands,
assignments and function arguments are still converted by codegen; each moves
to the pass in its own change."

## Architecture

`xform_insert_implicit_conversions` (`compiler/analyzer/src/`) already wraps a
converted comparison operand in `ExprKind::ImplicitConversion` and gives an
untyped comparison literal a concrete `expr_type`. The pass is extended, one
construct per PR, to record what codegen does today for:

1. arithmetic operands (the operator `a + b` and the folds `ADD(a, b, ...)`),
2. the value of an assignment, converted to the target's type,
3. arguments to functions, methods and function blocks,
4. every remaining literal.

Each PR first records *exactly* what codegen does today, so that introducing it
changes no generated code. Codegen then compiles the recorded nodes and the
code that made the decision is deleted in the same PR (a recording that codegen
ignores would be a second copy of the rule).

The conversions codegen chooses today, and so what each PR must reproduce:

| Decision | Where codegen makes it today |
|---|---|
| Arithmetic: each numeric operand at its own type, converted to the result type; result converted to the enclosing operation type | `compile_arith.rs`: `compile_binary_arith`, `numeric_steps`, `compile_numeric_fold`, `compile_at`, `convert` |
| Arithmetic overload resolved a second time | `compile_arith.rs` calls `resolve_arithmetic_overload` with `ctx.compiler_options` |
| Variable read converted to the operation type | `compile_expr.rs`, `ExprKind::Variable` arm |
| Argument converted to the parameter's width | `compile_call.rs`: `compile_value_arg` |
| Literal takes the operation type passed down | `compile_expr.rs`: `compile_expr(.., op_type)`, `compile_constant` |
| Literal range checked against a predicted type | `rule_constant_range.rs`: `check_expr` pushes `expected` down |

## Prefactoring

- **Share the "needs a conversion" predicate.** `needs_conversion`,
  `is_scalar` and the literal-typing step in the pass are private to the
  comparison code. PR 1 begins with a behaviour-preserving PR that moves them
  from the comparison walker to a small `conversion_target` helper module in
  the analyzer that every later PR calls, so arithmetic, assignment and
  argument recording do not each copy them. This is the prefactor (PR A).
- **Keep modules under 1000 lines.** `compile_call.rs` is already 970 lines;
  deleting `compile_value_arg` and its conversion shrinks it, and no PR adds
  to it. If the pass module nears the limit, split per construct
  (`xform_insert_implicit_conversions/{compare,arith,assign,call}.rs`).
- No prefactoring of codegen is needed first: each PR deletes the code it
  replaces.

## Design doc reference

- `specs/adrs/0056-analyzer-records-implicit-conversions-in-the-ast.md`
- `specs/design/implicit-conversions.md`
- `specs/design/arithmetic-operator-overloads.md`
- `specs/design/comparison-operand-type.md`

Both ADR-0056 and `implicit-conversions.md` are updated by each PR to describe
the construct it adds (new `REQ-IC-*` IDs with `#[spec_test]` tests), and
`implicit-conversions.md` states the final invariants in PR 5.

## Constraints and risks

- **No diagnostic may be lost.** The pass runs after the semantic rules, so
  rules keep checking the program as written. ADR-0056 found that running it
  before them lost P2026 for `DINT#300 < s` on a `SINT`.
- **Byte-for-byte output.** Every existing end-to-end test passes unchanged.
  Any bytecode difference is explained in the PR that causes it, and a
  difference that is a bug fix goes in its own PR.
- **Function block outputs.** Codegen stores a function block output at the
  field's operation type with no conversion (`compile_stmt.rs`). Only that is
  recorded; adding a conversion there changes behaviour and is a separate
  correction.
- **Merge conflicts.** This work touches `compile_expr.rs`, as the bit and
  partial-access issue does; land one before starting the other where possible.
- `specs/design/lowered-program.md` is not cited from code or docs until it is
  on `main`.

## File map

| File | Change |
|---|---|
| `compiler/analyzer/src/xform_insert_implicit_conversions.rs` | extended per construct (possibly split into a directory module) |
| `compiler/analyzer/src/intermediates/conversion_target.rs` | new: shared predicate (PR A) |
| `compiler/analyzer/src/rule_constant_range.rs` | PR 5: read recorded literal types |
| `compiler/analyzer/src/lib.rs` | export what codegen still needs, drop `resolve_arithmetic_overload` use from codegen |
| `compiler/codegen/src/compile_arith.rs` | delete `convert` callers, `numeric_steps`, `compile_numeric_fold`'s conversions |
| `compiler/codegen/src/compile_call.rs` | delete the conversion in `compile_value_arg` |
| `compiler/codegen/src/compile_expr.rs` | stop passing literal types through `compile_expr` |
| `compiler/codegen/src/compile_stmt.rs` | compile the recorded assignment conversion |
| `compiler/codegen/tests/` | analyzed-tree invariant test (PR 5); existing end-to-end tests unchanged |
| `specs/adrs/0056-*.md`, `specs/design/implicit-conversions.md` | describe each extension |

## Tasks

Each group is its own PR branched from `main`; none includes this plan.

### Prefactor PR A: share the conversion predicate

- [ ] Move `needs_conversion`, `is_scalar` and the literal-typing step out of
      the comparison walker into a shared analyzer helper.
- [ ] Existing analyzer tests pass unchanged.

### PR 1: arithmetic operands

- [ ] Record, for `a op b` and `OP(a, b, ...)`, the result type of the numeric
      overload (resolved once, with the analyzer's `CompilerOptions`) and wrap
      each operand of another width in an `ImplicitConversion` to it; record
      the conversion of the result to the enclosing operation type where
      codegen converts it.
- [ ] Leave typed time/date overloads (`t1 + t2`, `dt + t`, `d1 - d2`) alone:
      they compile through their typed routine.
- [ ] Codegen compiles the recorded nodes; delete `compile_at`,
      `numeric_steps`, the five `convert` call sites, and the second
      `resolve_arithmetic_overload` call.
- [ ] Tests: `REQ-IC-analyzer-*` for `INT + REAL`, `DINT * DINT` assigned to
      `LINT`, a three-input fold, and a time pair left unconverted; compare
      generated bytecode before and after for the end-to-end corpus.

### PR 2: assignments

- [ ] Wrap the assigned value in a conversion to the target's type where
      codegen converts it today.
- [ ] Codegen compiles the recorded node; delete its own conversion.
- [ ] Record function block outputs at the field's operation type only.

### PR 3: arguments to functions, methods and function blocks

- [ ] Wrap an argument whose operation width differs from its parameter's.
- [ ] Delete the conversion in `compile_value_arg` and
      `emit_conversion_opcode`'s call there.
- [ ] Cover positional, named, and function block input arguments.

### PR 4: literal types everywhere

- [ ] Give every literal in the analyzed `Library` a concrete `expr_type`
      (initializers, array and structure elements, subscripts, bounds,
      arguments, conditions).
- [ ] Stop passing a literal's type down through `compile_expr`; codegen reads
      the literal's recorded type.

### PR 5: constant range reads the recorded types, and the invariant test

- [ ] Switch `rule_constant_range` to the recorded literal types, check by
      check: each check chooses between the recorded tree and the program as
      written, and the choice is stated in a comment. No P2026 (or other)
      diagnostic is lost; a regression test per check.
- [ ] Add a test that walks the analyzed `Library` for a corpus (the
      end-to-end sources) and finds no literal at a generic type such as
      `ANY_INT`, and no operand whose type differs from the type its operation
      computes at unless an `ImplicitConversion` wraps it.
- [ ] Update ADR-0056 (record what landed) and
      `specs/design/implicit-conversions.md` to describe the extended pass.

## Verification (every PR)

- [ ] `cd compiler && just` passes.
- [ ] `cd specs && just` passes.
- [ ] Every existing end-to-end test passes unchanged, and the PR explains any
      bytecode difference.
