# Interface types: declare, hold and convert (#1891, part 1)

## Goal

An `INTERFACE` is accepted by `ironplcc check` as a type. A variable can be
declared with an interface type, and a function block instance that
`IMPLEMENTS` the interface can be assigned to it or passed to it as an
argument. Calls through an interface and code generation for interface values
stay unsupported, and say so.

This is part 1 of #1891 (items 1, 2 and 4). Part 2, the `IMPLEMENTS`
conformance check (item 3), is a separate PR. Until it lands, `IMPLEMENTS`
is accepted without checking that the function block provides the members.

Stacked on #1892 (interface member prototypes), which is stacked on #1871.

## Current behaviour

For

```
INTERFACE I_Comm ... END_INTERFACE
FUNCTION_BLOCK FB_NullComm IMPLEMENTS I_Comm ... END_FUNCTION_BLOCK
FUNCTION_BLOCK FB_User
VAR comm : I_Comm; impl : FB_NullComm; END_VAR
comm := impl;
END_FUNCTION_BLOCK
```

`check --dialect twincat` reports:

- P2008 on `comm : I_Comm`: `xform_resolve_late_bound_type_initializer`
  classifies type names through its own `TypeDefinitionKind` table, which has
  no entry for an interface.
- P2037 on `comm := impl`: `xform_resolve_type_decl_environment` registers
  an interface as an empty `IntermediateType::Structure`, so the struct
  assignment rule applies.
- P9999 on `INTERFACE` and on `IMPLEMENTS`, from `rule_unsupported_extension`.

## Architecture

- **Type:** a new `IntermediateType::Interface { name, extends }` replaces
  the empty-structure placeholder. An interface value is a reference to a
  function block instance (ADR-0041 Phase 2 calls it a "fat reference"). Its
  runtime representation is not decided here.
- **Declaration:** `TypeDefinitionKind::Interface`, and a new
  `InitialValueAssignmentKind::Interface(InterfaceInitializer { type_name })`
  for `x : I_X`. An initializer on an interface variable (`x : I_X := ...`)
  is not supported. It reports P9999.
- **Conversion:** a function block type converts to an interface type if the
  block, or a base it `EXTENDS`, lists the interface or an interface that
  `EXTENDS` it (transitively) in `IMPLEMENTS`. An interface converts to
  itself and to the interfaces it extends. That applies to `:=`, to input and
  in-out arguments, and to output arguments assigned to an interface
  variable. Anything else is a new problem code, "value does not implement
  the interface".
- **Calls through an interface** (`comm.Send(1)`) report P9999 with a label
  that points at the dynamic dispatch design (#1870), in
  `rule_method_call_declared`, instead of falling through to P4012.
- **Code generation:** an interface-typed variable reports P9999 in
  `compile_setup`. `check` passes, `compile` refuses.
- **P9999 removed** for `INTERFACE` and for `IMPLEMENTS`. `EXTENDS` and
  `ABSTRACT` on a function block keep their current handling.

## Prefactoring

`xform_resolve_late_bound_type_initializer.rs` is at 927 lines, and the new
tests would take it over the 1000-line limit. Move its test module to
`xform_resolve_late_bound_type_initializer/tests.rs` first, with no change to
the tests.

No other prefactor is planned. The new `Interface` arms are needed wherever
the enums are matched exhaustively. That is the enum doing its job, not
repeated branching over a flag.

## Design doc reference

- ADR-0041, "Phase 2": it describes interface values and refers to an
  "existing `IMPLEMENTS` conformance rule". No such rule exists in
  `ironplc-analyzer`. Correct that sentence in this PR.
- `specs/design/beckhoff-twincat-dialect.md` §1.3: update what is supported.

## File map

- `analyzer/src/xform_resolve_late_bound_type_initializer.rs` (+ `tests.rs`)
- `analyzer/src/xform_resolve_type_decl_environment.rs`
- `analyzer/src/intermediate_type.rs` and the exhaustive matches the compiler
  finds
- `analyzer/src/rule_unsupported_extension.rs`
- `analyzer/src/rule_assignment_aggregate_type_compat.rs` or a new
  `rule_interface_conversion.rs`, depending on where argument checks live
- `analyzer/src/rule_method_call_declared.rs`
- `dsl/src/common.rs`, `fold.rs`, `visitor.rs`: `InterfaceInitializer`
- `plc2plc/src/renderer.rs`: render `x : I_X`
- `codegen/src/compile_setup.rs`: P9999
- `problems/resources/problem-codes.csv`, `docs/reference/compiler/problems/P####.rst`
- `docs/reference/language/object-orientation/interface.rst`
- `specs/adrs/0041-staged-method-and-interface-dispatch.md`,
  `specs/design/beckhoff-twincat-dialect.md`

## Tasks

- [ ] Prefactor: move the late-bound type initializer tests into `tests.rs`
- [ ] `IntermediateType::Interface`, registered by `fold_interface_declaration`
- [ ] `TypeDefinitionKind::Interface` and `InitialValueAssignmentKind::Interface`;
      `x : I_X` resolves, `x : I_X := ...` is P9999
- [ ] Remove the P9999 for `INTERFACE` and `IMPLEMENTS`
- [ ] Conversion check for `:=` and call arguments, new problem code and docs page
- [ ] P9999 for a call through an interface variable
- [ ] P9999 in codegen for an interface-typed variable
- [ ] plc2plc round trip for interface-typed declarations
- [ ] Docs and design doc updates, ADR-0041 correction
- [ ] Corpus: rerun per project, record the change on #1199
- [ ] Open or update the issue for part 2 (conformance)
- [ ] `git rm` this plan; `cd compiler && just`
