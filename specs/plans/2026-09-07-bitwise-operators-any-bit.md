# Hold the bit-string operators to ANY_BIT

## Goal

`AND`, `OR`, `XOR` and `NOT` in their **operator** spelling accept `ANY_INT`
operands and compile to the *logical* opcodes, silently returning the wrong
value with no diagnostic. Hold them to `ANY_BIT` — the type IEC 61131-3
defines them over — so such a program is rejected with P4049 rather than
miscompiled.

Measured on `main` (`x := 10`, right operand `3`):

| operand type       | `x AND 3` | `x OR 3` | `x XOR 3` | `NOT x`         |
| ------------------ | --------- | -------- | --------- | --------------- |
| `BYTE` (`ANY_BIT`) | 2         | 11       | 9         | correct         |
| `DINT` (`ANY_INT`) | **1**     | **1**    | **0**     | **0** (want -11) |

The `DINT` row is the result of treating each operand as truthy — logical, not
bitwise. `BYTE` is correct throughout, so the defect is confined to operands
outside `ANY_BIT`.

This is the mirror of issue #1567, which fixed the *functional* spelling
`AND(w1, w2)` wrongly rejecting `WORD`. That was resolved by holding the family
to `ANY_BIT`; this holds the operator spelling to the same row, so the two
spellings agree in both directions.

## Architecture

Three facts make this small:

1. The operator-form table already carries the rows — `AND`, `OR`, `XOR` and
   `NOT`, each with operand type `ANY_BIT` (`intermediates/operator_function_form.rs`).
   `FormOf::Not` and `Arity::Unary` already exist for the unary row.
2. `rule_operator_operand_type_check` already reads that table and checks an
   operand against a row via `are_types_compatible`, reporting P4049. It just
   never asks the question for anything but `MOD`: it implements only
   `visit_binary_expr`.
3. `visit_compare_expr` and `visit_unary_expr` exist on the generated `Visitor`
   trait, so reaching the other two node kinds needs no DSL change.

So the change is to ask the existing question at two more node kinds. Nothing
in codegen changes: once the analyzer rejects `ANY_INT` operands, the
`_ => emit_bool_not()` arm in `compile_expr.rs` and the logical `AND`/`OR`/`XOR`
opcodes only ever see `BOOL`, which is what they are correct for.

**Deliberately left alone**, each matching the rule's existing conservative
posture:

- The relational rows (`GT`, `EQ`, …) are `ANY_ELEMENTARY`. Holding them to
  that is a separate decision, like `ADD`/`SUB` on `TIME` (issue #1621).
- `AND_THEN` / `OR_ELSE` have no table row, so they skip naturally. They are a
  CODESYS/TwinCAT short-circuit extension, not IEC operators.
- An operand whose type the predicate cannot judge (subrange, enumeration,
  structure) is skipped, as it is for `MOD` today.

## Prefactoring

`visit_binary_expr` inlines the whole per-node body: look up the row, then call
`check_operand` once per operand. Adding two more node kinds would copy that
shape twice more.

Prefactor first: extract it into one helper taking the operator label, the row
and the operands, so all three visitors share a single path. Committed on its
own, with no behaviour change, before any new check is added.

`checked_form` also needs generalising — it takes `&Operator` (arithmetic only)
and its doc says "Only `MOD` is checked". One lookup per node kind, each
returning `None` for the rows this rule deliberately does not check, keeps the
"which operators are checked, and why" decision in one readable place.

## Design doc reference

None exists for this rule; the operator-form table is self-documenting and
pinned by its own row-by-row test. Issue #1567 is the precedent for the
direction.

## File map

| File | Change |
| ---- | ------ |
| `compiler/analyzer/src/rule_operator_operand_type_check.rs` | Prefactor the shared check; add compare + unary visitors; update the module doc |
| `docs/reference/compiler/problems/P4049.rst` | Extend beyond `MOD` to the bit-string operators |
| `compiler/codegen/tests/it/end_to_end_bool.rs` | Three `NOT <DINT>` fixtures encode the old behaviour |
| `compiler/codegen/tests/it/compile_bool.rs` | One `NOT <DINT>` fixture asserts `BOOL_NOT` |
| `compiler/analyzer/src/rule_operator_operand_type_check.rs` (tests) | Accept/reject cases per operator |

## Tasks

- [ ] Prefactor: extract the shared "look up row, check operands" helper out of
      `visit_binary_expr`; no behaviour change
- [ ] Generalise the row lookup to the three node kinds, `None` for rows left
      unchecked, with the reasoning in the doc comment
- [ ] Add `visit_compare_expr`, checking `AND`/`OR`/`XOR` operands
- [ ] Add `visit_unary_expr`, checking `NOT`'s operand
- [ ] Update the module doc: no longer "only `MOD`"
- [ ] Rule tests: `BYTE`/`WORD` accepted, `DINT` rejected, `BOOL` accepted, for
      each of the four operators; subrange still skipped
- [ ] Update the four codegen fixtures that encode the miscompile
- [ ] Extend `P4049.rst` with a bit-string example and the fix
- [ ] Full CI: `cd compiler && just` and `cd specs && just`
- [ ] `git rm` this plan
