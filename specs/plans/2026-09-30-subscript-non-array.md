# Reject a subscript on a variable that is not an array

Issue: #1470

## Goal

`ironplcc check` accepts `x[1]` when `x` is a `DINT`, and `h.n[1]` when the
field `n` is a `DINT`; `compile` then fails in codegen with P9999, which tells
the user the compiler is incomplete when the program is wrong. The analyzer
reports these programs, and the codegen sites that reported them become
internal errors (P9998) guarding the invariant.

## Architecture

A new semantic rule, `rule_subscript_operand_array`, visits every subscript
chain once, at its outermost `ArrayVariable`, and walks it from the base
variable outwards with `variable_type::of` resolving the base:

- A bracket that starts on a value that is not an array reports
  **P4070 SubscriptNotArray**: a scalar, a `STRING`, a structure, a function
  block instance, a scalar structure field, `p^` on a `REF_TO DINT`, or the
  element reached by one bracket too many (`v[1][2]` on a one-dimensional
  array).
- A bracket that gives more subscripts than the array has dimensions left,
  or a chain that stops before every dimension has one, reports
  **P4071 ArraySubscriptCountMismatch**.

The walk keeps codegen's accepted shapes: `m[1][2]` on a two-dimensional
array (codegen flattens the chain), and `pa[1]` on a `REF_TO ARRAY` or a
`REFERENCE TO ARRAY` (codegen indexes the referenced array). An unresolved
type is left alone (another rule reports an undeclared name), as is a
dimension list the analyzer does not know.

`variable_type::of` needs two corrections to answer these questions, the
same two open PR #1911 makes, in identical hunks so the branches merge
cleanly: a named variable resolves through its declaration's type id, so an
inline array keeps its dimensions, and `p^` has the referenced type rather
than `REF_TO`.

Problem codes P4064 and P4066 to P4069 are taken by open pull requests, so
the new codes are P4070 and P4071.

## Prefactoring

`value_type::describe_representation` names a type as diagnostics show it;
make it `pub(crate)` so the new rule describes the operand's type the same
way P4035 does instead of carrying a second copy.

## Codegen

- `compile_array.rs` `resolve_struct_field_array` "Field is not an array
  type": becomes `internal_error_at`.
- `compile_array_struct.rs`, both "Field is not an array type" sites:
  become `internal_error_at`.
- `compile_array.rs` `emit_flat_index` "Wrong number of array subscripts":
  becomes `internal_error_at`.
- `compile_array.rs` `resolve_access`, a subscripted name in neither
  `array_vars` nor `struct_array_vars`: stays P9999. It is still reached by
  a real array codegen does not register -- a `CONFIGURATION` `VAR_GLOBAL`
  of a named array type -- so it is not an invariant yet. Its label says so,
  and an issue records the gap.

## File map

- `compiler/analyzer/src/rule_subscript_operand_array.rs` (new)
- `compiler/analyzer/src/lib.rs`, `stages.rs` (register the rule)
- `compiler/analyzer/src/variable_type.rs` (type id, dereference)
- `compiler/analyzer/src/value_type.rs` (visibility)
- `compiler/problems/resources/problem-codes.csv` (P4070, P4071)
- `docs/reference/compiler/problems/P4070.rst`, `P4071.rst` (new)
- `docs/reference/language/data-types/derived/array-types.rst` (subscripts)
- `compiler/codegen/src/compile_array.rs`, `compile_array_struct.rs`

## Tasks

- [ ] Prefactor: `describe_representation` visible to the crate
- [ ] Tests first: rule tests for every shape above, failing
- [ ] `variable_type::of` corrections
- [ ] Rule, problem codes, docs
- [ ] Codegen sites to internal errors
- [ ] Issue for the named-type global array gap
- [ ] `git rm` this plan; `cd compiler && just`; `cd specs && just`; docs build
