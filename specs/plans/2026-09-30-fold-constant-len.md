# Fold LEN of a Constant String at Compile Time

Issue: [#1612](https://github.com/ironplc/ironplc/issues/1612)

## Goal

`LEN` of a string whose value cannot change compiles to a single integer
constant instead of allocating a data region slot, filling it through a temp
buffer and reading `cur_length` back with `LEN_STR`:

1. a character string literal, `LEN('Hello')`, including escapes (`$'`,
   `$N`, `$41`) and `WSTRING` literals;
2. a `CONCAT` built from such operands, `LEN(CONCAT('ab', 'cde'))`;
3. a named `CONSTANT` string variable with an initial value, whether the
   qualifier was written in the source or inferred by
   `xform_mark_unwritten_constants` (case 3 of the issue, already merged).

## Architecture

The fold lives in code generation, in `compile_len`, not in the analyzer's
`xform_fold_constant_expressions`:

- `specs/design/constant-variable-inference.md` already assigns it there:
  the analyzer establishes the `CONSTANT` qualifier and "code generation only
  has to understand two cases -- a literal, and a `CONSTANT`-qualified
  variable".
- The analyzer fold runs before the semantic rules. Replacing `LEN('...')`
  there with an integer literal would remove the literal before
  `rule_string_literal_char_range` (P4052) and the call before
  `rule_function_call_type_check` see them, and would change what the
  language server reports.
- What the value is at run time is a codegen fact: the literal is clamped to
  the slot capacity (a `u16`, ADR-0035), a variable's initial value is
  truncated to its declared capacity, and `CONCAT` needs its operands to
  share an encoding (P4034). Codegen already computes these in
  `string_width.rs`.

The folded constant is pushed with `LOAD_CONST_I32`, the same stack type
`LEN_STR` leaves, so the call's result type and every consumer of it
(assignment to `INT`/`DINT`/`LINT`, arithmetic) are unchanged.

A string's length is counted in code units: one per `char` for both
`STRING` (Latin-1) and `WSTRING` (UCS-2); P4052 rejects characters outside
the type's range, and the parser has already decoded `$` escapes.

When an operand's constant length cannot be established (a written
variable, an array element, a structure field, a call other than `CONCAT`,
`CONCAT` operands of different encodings) `compile_len` compiles as today,
so every existing diagnostic is still reported.

## Prefactoring

The code that registers a `STRING`/`WSTRING` declaration (resolve capacity
and width, reserve the data region, raise `max_string_capacity`, insert
`StringVarInfo`) is repeated four times: program/global variables in
`compile_setup::assign_variables`, and function parameters, function locals
and function block fields in `compile_fn.rs`. Adding the constant length to
`StringVarInfo` would need a fifth change in each copy. Extract it into one
`register_string_variable` in `compile_setup.rs`, behaviour unchanged, in its
own commit.

## Design doc reference

`specs/design/constant-variable-inference.md`: replace the "Out of scope"
bullet about the `LEN` fold with a "Folding in code generation" section
carrying `REQ-CVI-codegen-*` requirements, and add the doc to
`codegen/build.rs`.

## File map

- `compiler/codegen/src/compile_setup.rs` -- `register_string_variable`
- `compiler/codegen/src/compile_fn.rs` -- use it
- `compiler/codegen/src/compile.rs` -- `StringVarInfo::constant_length`
- `compiler/codegen/src/string_constant.rs` (new) -- constant length of a
  string expression and of a declaration
- `compiler/codegen/src/compile_string.rs` -- fold in `compile_len`
- `compiler/codegen/src/spec_conformance_constant_len.rs` (new)
- `compiler/codegen/build.rs`, `compiler/codegen/src/lib.rs`
- `compiler/codegen/tests/it/compile_len.rs` (new) -- bytecode structure
- `compiler/codegen/tests/it/end_to_end_len.rs` -- values unchanged
- `specs/design/constant-variable-inference.md`

## Tasks

- [ ] Plan (this file)
- [ ] Prefactor: `register_string_variable`
- [ ] Tests first: bytecode tests (no `STR_INIT`/`LOAD_CONST_STR`/`LEN_STR`,
      no data region bytes, no temp buffer) and end-to-end values for
      escapes, `WSTRING`, `CONCAT`, declared and inferred `CONSTANT`,
      truncation to the declared capacity, `DINT`/`LINT` targets, function
      and function block locals; a written variable still uses `LEN_STR`
- [ ] Implement the fold
- [ ] Design doc requirements and conformance tests
- [ ] `git rm` this plan
- [ ] `cd compiler && just`, `cd specs && just`
