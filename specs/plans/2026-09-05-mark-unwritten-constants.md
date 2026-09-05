# Plan: Mark never-written variables as `CONSTANT` (issue #1612, case 3)

## Goal

Add an analyzer transform that gives the `CONSTANT` qualifier to every
variable the program never writes, so that code generation only ever has to
reason about two kinds of constant (literals and `CONSTANT`-qualified
variables) and can fold reads of them -- starting with `LEN` of a constant
string, per issue #1612.

This pull request delivers the transform and its design document only. The
`LEN` fold in `compile_string.rs` (cases 1 and 2 of the issue) is a separate
change that builds on it.

## Architecture

A new pass, `xform_mark_unwritten_constants`, runs at the end of
`stages::resolve_types`, after every other transform has resolved names and
types and before the semantic rules run. It has two phases:

1. **Collect writes.** A `Visitor` walks the whole library and records, by
   name, every variable the program can write: assignment targets, `FOR`
   control variables, `=>` output bindings, `REF()`/`ADR()` operands,
   arguments bound to `VAR_IN_OUT` parameters (functions, function blocks
   and methods), function-block instance initializers that override a member
   value, configuration-level writes (`VAR_CONFIG`, program connection sinks,
   `VAR_ACCESS` paths that are not `READ_ONLY`), and SFC action-association
   names and indicators. Writes are keyed by the *root* variable of an access
   chain: `s.field[i].0 := x` writes `s`.
2. **Mark declarations.** A `Fold` sets `qualifier = Constant` on every
   `VAR`/`VAR_TEMP` declaration that has an unqualified declaration, a
   symbolic (not located) identifier, an initializer the const rules accept,
   and whose name was never written. A `VAR_GLOBAL` is marked when every
   global of that name qualifies, and its `VAR_EXTERNAL` declarations are
   marked with it so P4009 (`VariableMustBeConst`) is never introduced.

**Write tracking is by name only, not by scope.** A write to `x` anywhere in
the library stops every `x` from being marked. This is sound (it can only
fail to mark, never mark wrongly) and it makes inheritance, methods and
`VAR_EXTERNAL` aliasing fall out for free. The cost is precision when two
POUs reuse a name, which is acceptable for a first version and recorded in
the design document.

**Callee lookups are conservative.** When a callee's parameter directions
cannot be determined (undeclared function, instance of unknown type), every
variable argument is treated as written.

## Prefactoring

None needed. The transform is a new, self-contained module wired into
`stages.rs` in the same shape as the existing `xform_*` passes. The helpers
it needs already exist: `IntermediateFunctionParameter::is_inout`,
`FunctionSignature::input_parameters`, `IntermediateStructField::var_type`
and `intermediates::inherited_fields::collect_inherited_fields`. No existing
code gains a new `match` arm, and no existing test setup is duplicated.

## Design doc reference

`specs/design/constant-variable-inference.md` (new in this change), with
`REQ-CVI-analyzer-*` requirements and conformance tests.

## File map

- `compiler/analyzer/src/xform_mark_unwritten_constants.rs` -- new transform
  (write collection, eligibility, marking, unit tests)
- `compiler/analyzer/src/stages.rs` -- wire the pass at the end of
  `resolve_types`
- `compiler/analyzer/src/lib.rs` -- register the module and the conformance
  test module
- `compiler/analyzer/build.rs` -- list the new design document
- `compiler/analyzer/src/spec_conformance_constant_inference.rs` -- new
  requirement conformance tests
- `specs/design/constant-variable-inference.md` -- new design document

## Tasks

- [ ] Write and commit this plan
- [ ] Add the design document with requirement IDs
- [ ] Implement the transform with unit tests for every write kind and every
      exclusion (located, retain, input/output/in-out, function block,
      no initializer, already constant)
- [ ] Wire into `stages.rs`; confirm `analyze` introduces no diagnostics for
      a marked program
- [ ] Add the conformance tests and register the design document
- [ ] Run `cd compiler && just`
- [ ] `git rm` this plan
