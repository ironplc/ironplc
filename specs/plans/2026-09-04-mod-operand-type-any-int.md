# Plan: MOD accepts ANY_INT, in both spellings

Fixes [#1619](https://github.com/ironplc/ironplc/issues/1619).

## Goal

`MOD` on a `REAL` operand is reported by `check` instead of surfacing as an
internal error (P9998) from `compile`. IEC 61131-3 defines `MOD` over
`ANY_INT`; IronPLC declares its function form `ANY_NUM`, so `MOD(r1, r2)`
passes analysis, and the operator spelling `r1 MOD r2` is not type-checked
at all. Codegen has no floating-point `MOD` opcode, so both reach the
bytecode verifier with an unbalanced stack.

After this change:

- `MOD(r1, r2)` on `REAL` arguments is P4026 (argument type mismatch),
  because the row for `MOD` in the operator-form table says `ANY_INT`.
- `r1 MOD r2` on a `REAL` operand is P4049 (a new problem code: operator
  operand type mismatch), reported by a new semantic rule.
- `7.5 MOD 2.0` on two literals is also P4049: the constant folder no
  longer folds a real `MOD`, so the rule sees it.
- The `__MOD` compiler intrinsic keeps being the floating-point remainder
  for the compatibility libraries. It is a separate function and is not
  touched.

## Architecture

The function form and the operator are two spellings of one operation, and
the operator-form table (`operator_function_form.rs`) already states what
each function form accepts, once per row. The fix makes both spellings read
that row:

- **Function form.** The `MOD` row's operand cell changes from `ANY_NUM` to
  `ANY_INT`. The signature is derived from the row, so the existing P4026
  check rejects a real argument with no further change.
- **Operator form.** A new rule, `rule_operator_operand_type_check`, visits
  every `BinaryExpr` whose operator is `MOD`, looks up the row for that
  operator, and checks each operand's resolved type against the row's
  operand type using the same predicate the function-call rule uses
  (`type_compat::are_types_compatible`). The two spellings therefore agree
  by construction: whatever `MOD(a, b)` accepts, `a MOD b` accepts.

  Only `MOD` is checked. The other arithmetic operators are declared
  `ANY_NUM` in the table, but their operator spellings also compile today
  for `TIME` and bit-string operands (`t1 + t2`, `b1 + 1`), and IEC 61131-3
  Table 30 defines `ADD`/`SUB` on `TIME` too, so applying the table to them
  would turn working programs into errors. That is a separate decision
  and is tracked in a follow-up issue (see Tasks).

  An operand whose resolved type the predicate cannot judge (a subrange,
  an enumeration, a structure) is skipped rather than reported, as the
  assignment check does. `p MOD 2` on a subrange of `INT` compiles today
  and this rule does not change that.
- **Constant folder.** `fold_real_binary` returns `Ok(None)` for `MOD`
  rather than computing `left % right`, so a real `MOD` on literals is
  left unfolded for the rule to reject. Integer folding is unchanged.
- **Codegen.** `emit_mod` keeps its no-op float arms; the comment saying
  the analyzer should reject float `MOD` becomes a statement of which rule
  does.

## Prefactoring

Done, as its own pull request (`claude/mod-argument-type-sx16zk-prefactor`),
before this one:

- The type-compatibility predicate (`are_types_compatible` and the helpers
  that define the `ANY_*` relation, literal inference and widening) moved
  out of `rule_function_call_type_check.rs` into a crate-visible module,
  `type_compat.rs`, so a second rule can ask the same question without
  copying it. This also brought the function-call rule module back under
  the 1000-line limit (1801 to 1459 lines).

No further prefactoring is needed: the operator-form table already exists
and already states the operand category once per row (PR #1617). The one
addition the rule needs, a lookup of a row by operator rather than by
function name, is added here where its first caller is.

## Design doc reference

No design document covers this rule. The durable record is the rule
module's doc comment (why only `MOD`, why user-defined operand types are
skipped) and the problem documentation for P4049.

## File map

| File | Change |
|---|---|
| `compiler/analyzer/src/intermediates/operator_function_form.rs` | `MOD` row: `ANY_NUM` → `ANY_INT`; add `form_of_operator` lookup and `operand_type` accessor; update the pinned row test |
| `compiler/analyzer/src/rule_operator_operand_type_check.rs` | New rule: `MOD` operands must be in the row's operand category (P4049) |
| `compiler/analyzer/src/lib.rs` | Register the module and export the lookup |
| `compiler/analyzer/src/stages.rs` | Run the rule |
| `compiler/analyzer/src/constant_folding.rs` | Real `MOD` is not folded |
| `compiler/codegen/src/compile_expr.rs` | `emit_mod` comment names the rule |
| `compiler/problems/resources/problem-codes.csv` | Add P4049 |
| `docs/reference/compiler/problems/P4049.rst` | Document P4049 |

## Tasks

- [x] Prefactor: extract `type_compat.rs` (separate PR)
- [ ] Add P4049 to the problem-code registry and document it
- [ ] Change the `MOD` row to `ANY_INT` and update the pinned row test
- [ ] Add a lookup of the operator-form row by operator
- [ ] Add `rule_operator_operand_type_check` with tests: `REAL` operand,
      real literal, mixed `DINT MOD 2.0`, `DINT MOD 2` ok, integer literal
      ok, subrange operand skipped, nested expression reported
- [ ] Constant folder leaves real `MOD` unfolded, with a test
- [ ] Update the `emit_mod` comment
- [ ] Open a follow-up issue for operand checks on `+`, `-`, `*`, `/` and
      the `ADD(TIME, TIME)` function-form false positive
- [ ] `cd compiler && just`, `cd docs && just`, `cd specs && just`
- [ ] Delete this plan before merge
