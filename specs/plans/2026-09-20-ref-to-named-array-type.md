# Plan: REF_TO a Named Array Type

Tracking issue: [#1580](https://github.com/ironplc/ironplc/issues/1580).
The prefactor landed in #1735; this plan is the fix.

## Goal

`pt : REF_TO ARR4` (where `TYPE ARR4 : ARRAY[0..3] OF INT; END_TYPE`) compiles
and `pt^[i]` reads and writes the referenced array, exactly as the inline
spelling `REF_TO ARRAY[0..3] OF INT` already does. The alias form
`TYPE ArrRef : REF_TO ARR4; END_TYPE` used as `pt : ArrRef` is covered by the
same change.

## Architecture

Codegen registers `REF_TO` variables through one function,
`register_reference_variable` in `compiler/codegen/src/compile_reference.rs`.
Today it builds array metadata only when the AST target is
`ReferenceTarget::Array`, so a named target registers nothing and the later
`pt^[i]` lowering fails with P9999.

The analyzer already answers "what does this REF_TO point at?" for both
spellings: `TypeEnvironment::resolve_reference_target` returns an
`IntermediateType`. The fix is to call it from `register_reference_variable`
and, when the result is `IntermediateType::Array`, build the `ArraySpec` via
the existing `array_spec_from_named`. That is the same path a plain
`arr : ARR4` declaration takes, so both spellings share one representation
and the `ReferenceTarget` enum is no longer inspected by codegen.

Threading the `TypeEnvironment` into `register_reference_variable` is the only
signature change. It is already in scope at all four call sites.

## Prefactoring

Done in #1735 (one resolver in the analyzer, one registration path in
codegen, dead parser arm removed). One small reshaping remains and is folded
into the fix commit because it only touches the function the fix reaches for:
`array_spec_from_named` reports an unsupported element type with a bare
`Diagnostic::todo()`, which has no span. It gains a `span` parameter so the
P9999 points at the declaration, matching #1740.

## Design doc reference

`specs/design/reference-to-twincat.md`, section "Execution (codegen)". Adds
**REQ-RTO-codegen-421** for a reference to a named array type.

## File map

- `compiler/codegen/src/compile_reference.rs` — resolve the target through
  the type environment; register array metadata from the resolved type
- `compiler/codegen/src/compile_array.rs` — `array_spec_from_named` takes a
  span for its not-implemented diagnostic
- `compiler/codegen/src/compile_setup.rs`, `compiler/codegen/src/compile_fn.rs`
  — pass `types` at the four call sites
- `compiler/codegen/src/spec_conformance.rs` — conformance test for the new
  requirement
- `compiler/codegen/tests/it/end_to_end_ref_to_array.rs` — end-to-end tests
- `specs/design/reference-to-twincat.md` — requirement text and table row

## Tasks

- [ ] `register_reference_variable` resolves the target via
      `TypeEnvironment::resolve_reference_target` and registers array metadata
      for any `IntermediateType::Array`
- [ ] `array_spec_from_named` reports unsupported element types with a span
- [ ] Pass the type environment at the four call sites
- [ ] Requirement REQ-RTO-codegen-421 and its conformance test
- [ ] End-to-end tests: program-level read (the issue program), write through
      the reference, function parameter, function-block local, alias type,
      two-dimensional named type
- [ ] `cd compiler && just` passes
- [ ] Remove this plan before merge

## Out of scope

A `REF_TO` whose target is an array of structures still reports P9999. That
already holds for the inline spelling, and the shared path now reports it
the same way for both.
