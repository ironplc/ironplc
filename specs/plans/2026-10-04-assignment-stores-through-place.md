# Write scalar assignment targets through `Place`

## Goal

Make `Place` (`compiler/codegen/src/compile_place.rs`) the one way an
assignment statement writes a single-slot target. Six arms of the
`StmtKind::Assignment` dispatch each repeat "compile the value, truncate,
address, store". They become one sequence: resolve a `Place`, `compile_expr` at
`place.op_type()`, then `place.emit_store`. This is a prefactor for later work
that hands the backend a place directly, so the change must preserve behaviour
and the emitted bytecode must not change.

## Architecture

Line numbers are as of `30dc88a`. Since `7951b20`, the only change to these
arms is the `IntermediateType` to `SemanticType` rename.

| Assignment arm (`compile_stmt.rs`) | `Place` address |
|---|---|
| Fixed-offset struct field, after the FB-field and STRING-field checks (222–229) | `Element`, `Slot` |
| `ResolvedAccess::Scalar` (254–264) | `Variable` |
| `ResolvedAccess::InOut` (265–277) | `InOut` |
| `ResolvedAccess::ArrayElement`, non-string (315–331) | `Element`, `through_ref: false`, `Subscripts { offset: None }` |
| `ResolvedAccess::DerefArrayElement` (333–362) | `Element`, `through_ref: true` |
| `ResolvedAccess::StructFieldArrayElement` (363–386). Covers `s.arr[i]` and `a[i].f`. | `Element`, `Subscripts { offset: Some(..) }` |

Today each arm does value, truncate, address, store. `Place::emit_store` does
truncate, address, store, with the value already on the stack. The order is the
same, and constants enter the pool in the same order: the value's constants
first, then the index's.

- **Resolve once.** Factor `Place::from_access(ctx, access, name, span)` out of
  `Place::resolve`. It builds a place from a `ResolvedAccess` that has already
  been computed. `resolve` keeps its fixed-offset-field branch and then calls
  `from_access(resolve_symbolic_access(..))`, so partial access is unaffected.
  The assignment's generic section still calls `resolve_access` first. That
  keeps the existing `todo` error for `Variable::Direct`. It then matches the
  two string-element variants into the existing string code and passes every
  other variant to `from_access`. The name and span passed in come from
  `resolve_variable_name` and `variable_span`, which are the values the arms
  use today.
- **One store helper** in the new module:
  `compile_expr(value, place.op_type())` followed by `place.emit_store`. All
  six arms call it.
- **Dispatch order unchanged**: set/reset, `p^ :=`, partial access, FB field,
  STRING field, whole aggregate, STRING variable, then the generic
  `resolve_access` match.

## Prefactoring

The whole change is a prefactor. Inside its PR, the first refactor commit only
moves code: the assignment arm's body moves unchanged into
`compile_assignment` in a new `compile_assign.rs`. Then `compile_stmt.rs`
shrinks by about 290 lines, and the `Place` diff is reviewed apart from the
move.

## Design doc reference

None. `Place`'s rationale lives in the doc comment of `compile_place.rs`. That
comment gains a sentence saying assignment targets use it too.

## Equivalence checks

1. **Value operation type: always equal.**
   `var_type_info_for_field` begins with
   `let (op_width, signedness) = resolve_field_op_type(field_type)?;` and has
   a storage-width arm for every type that `resolve_field_op_type` maps to
   `Some`. Both struct arms reach the store only when `resolve_field_op_type`
   is `Some`; otherwise `resolve_struct_field_access`,
   `resolve_struct_field_array` or `struct_array_element_field` returns
   `not_implemented` first. So `Place::op_type()` returns the same pair and
   never falls back to `DEFAULT_OP_TYPE`. Both constructors of
   `StructFieldArrayElement` derive `element_op_type` and `element_type` from
   the same type. `Place` therefore does not need to carry an op type. A unit
   test will pin the agreement for every leaf `SemanticType`.
2. **No type info: identical.** `Scalar` and `InOut` come only from
   `SymbolicVariableKind::Named`. Both paths call `ctx.var_type_info(name)` on
   that name. When it returns `None`, both use `DEFAULT_OP_TYPE` and emit no
   truncation.
3. **Truncation: identical.** For the struct arms, both paths use
   `var_type_info_for_field` (`emit_truncation_for_field` wraps it). For the
   array arms, both use `info.element_var_type_info`. For the scalar arms,
   both use `ctx.var_type_info`.
4. **Unresolvable targets: unchanged.** `Variable::Direct` still fails in
   `resolve_access` with its `todo` error.
5. **Difference found: a STRING element reached through `^`.** `Place`
   rejects any array element with `is_string_element` set, including through
   a reference (`array_element` in `compile_place.rs`). Today's
   `DerefArrayElement` arm has no such check, and the shape is reachable
   because the analyzer accepts `^` on an array that is not a reference:

   ```
   VAR a : ARRAY[1..3] OF STRING; END_VAR
   a^[1] := 'x';
   ```

   On `main` this compiles to `LOAD_CONST_STR; LOAD_CONST_I32;
   STORE_ARRAY_DEREF`. Through `Place` it would fail with P9999 instead.
   **Proposal:** keep that one shape on today's code, in a guarded arm
   `DerefArrayElement { info, .. } if info.is_string_element` with a comment,
   so the bytecode stays identical. The arm can go once the analyzer rejects
   `^` on a non-reference. The alternative, accepting P9999, changes
   behaviour and would need its own PR.

### Bugs to describe in the PR, not fix

- `a^[i] := v` where `a` is an array but not a reference compiles to
  `STORE_ARRAY_DEREF` on `a`'s slot. That slot holds a data-region offset,
  not a variable index. Verified for `ARRAY OF DINT`. The bytecode is the
  same before and after this change.
- `p^[i] := 'x'` with `p : REF_TO ARRAY[..] OF STRING` stores the string as a
  single slot. `compile_reference.rs` registers a reference-to-array with
  `is_string_element: false`, so neither today's code nor `Place` treats the
  element as a STRING. The bytecode is the same before and after.

## Characterization tests

Existing coverage of a narrow target that overflows:

| Shape | Covered by |
|---|---|
| Named variable | `end_to_end_const_trunc.rs` (`SINT` `a + a` reads -56; `USINT`, `INT`, `UINT`) |
| `VAR_IN_OUT` in a function | `end_to_end_user_function_in_out.rs`, `…_when_in_out_int_overflows_then_caller_variable_wraps_at_int_width` |
| Array element | Bytecode only (`compile_array.rs`, `compile_when_array_sint_store_then_emits_truncation`). **Missing.** |
| Element through `REF_TO ARRAY` | **Missing.** `end_to_end_array_ref_to.rs` covers `ARRAY OF REF_TO`, a different shape. |
| Struct field `s.f` | **Missing.** |
| `s.arr[i]` | **Missing.** |
| `a[i].f` | Only a `BOOL` set to `TRUE`, which cannot overflow. **Missing.** |

A new `tests/it/end_to_end_assignment_targets.rs` adds the five missing
shapes, following `end_to_end_partial_access_bases.rs`. Each shape has a
`SINT` target, assigns it `a + a` with `a = 100`, and reads back -56. A
runtime `i` makes the subscripts dynamic. The element through a reference is
written inside a function that takes `REF_TO ARRAY[..] OF SINT`.

## Byte-identity check (not committed)

Add a temporary hook to `ironplc_codegen::compile`. When `IRONPLC_DUMP_DIR`
is set, it writes `Container::write_to` bytes to a file named by a hash of the
input library's `Debug` text and the options. The hook sits in `compile`
itself, so it also catches tests that call it without the shared helpers.
Apply the hook on both `main` (plus the test commit) and the branch, run
`cargo test -p ironplc-codegen` with a fixed `PROPTEST_RNG_SEED`, and
`diff -r` the two directories. The PR reports the count of containers
compared.

## File map

- `compiler/codegen/src/compile_assign.rs`: new, holds `compile_assignment`.
- `compiler/codegen/src/compile_stmt.rs`: the assignment arm becomes one
  call, and unused imports are removed.
- `compiler/codegen/src/compile_place.rs`: gains `from_access`, and its
  module doc mentions assignments.
- `compiler/codegen/src/compile_struct.rs`: gains the unit test from
  equivalence check 1. `emit_truncation_for_field` stays because
  `compile_struct_init.rs` still calls it.
- `compiler/codegen/src/lib.rs`: adds `mod compile_assign;`.
- `compiler/codegen/tests/it/end_to_end_assignment_targets.rs` and
  `tests/it/main.rs`: the new tests.

None of `compile.rs`, `compile_fn.rs`, `compile_method.rs` or
`compile_setup.rs` changes.

## Tasks

### Prefactor PR: write scalar assignment targets through `Place`

- [ ] Commit 1: characterization tests for the five missing shapes. They pass
      on `main`.
- [ ] Commit 2: move the `StmtKind::Assignment` body unchanged into
      `compile_assign.rs::compile_assignment`.
- [ ] Commit 3: factor `Place::from_access` out of `Place::resolve`. Route the
      six arms through `Place` and the store helper. Keep the guarded
      `DerefArrayElement` STRING arm. Delete the code this leaves unused. Add
      the op-type agreement unit test.
- [ ] Run the byte-identity check against `main` and record the result in the
      PR.
- [ ] `cd compiler && just`, including 85% coverage. Confirm that
      `compile_stmt.rs` has fewer lines than it does on `main` and that no
      module exceeds 1000 lines.
- [ ] PR description: the equivalence findings above, what was done about
      check 5, and the bugs found but not fixed.

## Out of scope

STRING stores, FB-instance field stores, whole-aggregate assignment, `p^ := v`,
`S=` and `R=`, the `FOR` control variable, initial-value stores, FB-call input
and output stores, the read side, renaming, dispatch-order changes, and fixing
the bugs listed above.
