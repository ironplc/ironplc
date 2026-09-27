# Apply declared initial values to function block instance fields

Issues: #1355, #1524 (the same defect). #1753 (STRING fields) is related but
needs per-instance string storage and is not part of this change.

## Goal

`VAR a : INT := 5; END_VAR` in a `FUNCTION_BLOCK` gives every instance of
that block `a = 5` before its first call, as the same declaration does in a
`PROGRAM`. An instance initializer (`inst : FB := (a := 9)`) still wins.

## Architecture

Instance fields live in the instance's data region and are copied in and
out on `FB_CALL`. The setup block initializes a user instance by storing its
data-region offset and then its member initializers
(`emit_fb_instance_member_initializers`), but never the defaults its type
declares.

- The FB pre-scan in `compile.rs` records, in `UserFbTypeInfo`, each field's
  declared initial value as an expression (`field_defaults`).
- The `FunctionBlock` arm of `emit_initial_values` stores those defaults with
  `compile_fb_field_store` (the sequence an assignment `inst.a := 5` uses)
  before the instance's own member initializers, so those override them.

## Prefactoring

The order of an FB's fields (inputs, outputs, locals), which *is* the
data-region layout, is computed by the same three loops in `compile.rs` (FB
pre-scan) and `compile_fn.rs` (`compile_user_function_block`). Extract it into
one function first, in its own behaviour-preserving commit, so the defaults
are taken in the layout order the rest of codegen uses.

## Design doc reference

None covers FB instance initialization; the rule is the one a PROGRAM
variable already follows.

## File map

- `compiler/codegen/src/compile_fn.rs` (shared field order), `compile.rs`
- `compiler/codegen/src/compile_setup.rs` (instance init)
- `compiler/codegen/tests/it/end_to_end_fb_field_initial_values.rs` (new)

## Tasks

- [ ] Prefactor: one function for the FB field order
- [ ] Tests: #1355 and #1524 reproductions, an instance initializer
      overriding a default, REAL/BOOL/enum defaults, VAR_INPUT and
      VAR_OUTPUT defaults, a global instance
- [ ] Record defaults; emit them in instance init
- [ ] `cd compiler && just`
- [ ] Delete this plan
