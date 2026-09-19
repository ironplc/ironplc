# Prefactor: Resolve `REF_TO` Targets Through One Path

Prefactor for [#1580](https://github.com/ironplc/ironplc/issues/1580):
`pt : REF_TO ARR4` (where `TYPE ARR4 : ARRAY[0..3] OF INT`) fails codegen
with P9999, while the inline spelling `REF_TO ARRAY[0..3] OF INT` works. The
work spans two pull requests, so #1580 is the tracking issue. This pull
request is the prefactor only; the fix lands in a separate pull request.

## Goal

Reshape the code so the #1580 fix drops in at one place. Behaviour is
unchanged: after this change, `REF_TO <named array type>` still fails
exactly as it does today, and every existing test passes without edits
beyond mechanical renames.

## Architecture

`ReferenceTarget` in `compiler/dsl/src/common.rs` stays a distinct enum
(`Named(TypeName) | Array(ArraySubranges)`). `Named` may be *any* type, not
only an array, so replacing it with `ArraySpecificationKind` (or an alias of
it) was considered and rejected: codegen could then pass a `REF_TO INT`
target into array-only helpers that raise an internal error on a non-array
name.

What is wrong is that three places each hand-write the inline-vs-named
branching:

1. The parser rule `ref_to_target` matches on the result of
   `array_specification()` to convert it, but that rule only ever produces
   `SpecificationKind::Inline`, so the `Named` arm is dead.
2. `fold_reference_declaration` in the analyzer resolves the target itself,
   re-wrapping the inline subranges in a `SpecificationKind` to call
   `intermediates::array::try_from`.
3. `register_ref_to_array_metadata` in codegen matches the enum and only
   handles `Array`. That is the bug, and it is reached from four copied
   call sites.

## Prefactoring

This whole change is prefactoring. Each step is one commit and is
behaviour-preserving on its own.

1. **One resolver in the analyzer.** Add
   `TypeEnvironment::resolve_reference_target(declaring, target)` returning
   the `IntermediateType` a `REF_TO` points at, whether the target is named
   or an inline array. The body moves verbatim from
   `fold_reference_declaration`, which becomes a call to the method followed
   by the existing `IntermediateType::Reference` wrap and `insert_type`.
2. **Remove the dead parser conversion.** Split the inline body of
   `array_specification()` into `array_subranges() -> ArraySubranges`; have
   `array_specification()` wrap it in `SpecificationKind::Inline`; make
   `ref_to_target` build `ReferenceTarget::Array` from `array_subranges()`
   directly. The `match` disappears.
3. **One reference registration path in codegen.** The
   `InitialValueAssignmentKind::Reference(ref_init)` arm is copied four
   times with the same body. Extract `register_reference_variable` and call
   it from all four. Also extract the dimension/stride computation that
   `register_ref_to_array_metadata` duplicates from
   `register_array_variable` into one shared function. Move the reference
   helpers into a new `compile_reference.rs` so `compile_array.rs` stays
   well under the 1000-line limit. The `if let ReferenceTarget::Array(..)`
   gate stays, with a comment referencing #1580.

## Design doc reference

None. The reasoning that must outlive this plan (why `ReferenceTarget` stays
an enum, why the codegen gate is still there) lives in doc comments on the
code it constrains.

## File map

Modified:

- `compiler/analyzer/src/type_environment.rs` — add
  `resolve_reference_target` and its unit tests
- `compiler/analyzer/src/xform_resolve_type_decl_environment.rs` — call it
  from `fold_reference_declaration`
- `compiler/parser/src/parser.rs` — `array_subranges` rule; simplify
  `ref_to_target`
- `compiler/codegen/src/compile_array.rs` — shared dimension helper; move
  reference helpers out
- `compiler/codegen/src/compile_setup.rs`,
  `compiler/codegen/src/compile_fn.rs` — call `register_reference_variable`
- `compiler/codegen/src/lib.rs` — register the new module

Created:

- `compiler/codegen/src/compile_reference.rs` — `register_reference_variable`
  and `register_ref_to_array_metadata`

## Tasks

- [ ] Step 1: `resolve_reference_target` in `TypeEnvironment`, with unit
      tests (named elementary, named array, inline array, undeclared named
      target keeps the same problem code, inline array with undeclared
      element type errors)
- [ ] Step 2: `array_subranges` parser rule; `ref_to_target` without the
      `match`
- [ ] Step 3: `register_reference_variable` and the shared dimension helper
      in codegen; `compile_reference.rs`
- [ ] Verify: `cd compiler && just` passes; `cd specs && just` passes;
      no test outside step 1's new unit tests changed beyond renames; the
      #1580 repro still fails with P9999
- [ ] `git rm` this plan before opening the pull request
