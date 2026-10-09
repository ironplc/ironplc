# Design: Variable Binding

status: partially implemented
date: 2026-10-09

## Overview

Which declaration a name refers to is a language rule: a local hides a
global of the same name, a method's local hides its function block's field, a
reference through `VAR_EXTERNAL` is the global it names. The analyzer applies
those rules (`SymbolEnvironment::find`) and records the answer on every
reference, and code generation allocates storage per declaration and looks a
reference up by the declaration it records (ADR-0058). A back end never
resolves a name, so it cannot resolve one differently from the analyzer.

Before this design, codegen built its own name-keyed maps per program
organization unit and copied the program's globals into each function and
function block body by name. A local only replaced the map for its own kind of
variable, so a function local `x : BIG` hiding a global `x : INT` kept the
global's type and a 70000 stored in it was truncated to 4464.

What is built, and what is not yet:

| Section | State |
|---|---|
| [Declaration identity](#declaration-identity) | Implemented |
| [Named references](#named-references) | Implemented |
| [`VAR_EXTERNAL`](#var_external) | References implemented; the declaration does not yet record the global it names |
| [`VAR_IN_OUT`](#var_in_out) | Implemented |
| [Late-bound names](#late-bound-names) | Implemented |
| [Storage](#storage) | Implemented for every variable; fields are still matched by name |
| [Fields](#fields) | Not implemented |
| [Function block member visibility](#function-block-member-visibility) | Not implemented |

## Declaration identity

```rust
// compiler/dsl/src/decl_id.rs
pub struct DeclId(u32);
```

`xform_assign_decl_ids` runs in `stages::resolve_types` before the symbol
environment is built and gives each declaration a `DeclId` from one allocator.
The id is stored on the declaration, set by the analyzer and, like `type_id`,
left out of `PartialEq`: a declaration built by hand equals a parsed one by
what it declares.

**REQ-VB-analyzer-001** Every variable declaration has a `DeclId` no other declaration has: a top-level, configuration or resource `VAR_GLOBAL`, a program, function or function block variable of every section, an edge variable, and a method variable.

**REQ-VB-analyzer-002** A function's implicit result variable has a `DeclId` (`FunctionDeclaration::result_decl_id`), and so does a method's when the method declares a return type (`MethodDeclaration::result_decl_id`); a method without one has none.

**REQ-VB-analyzer-003** Each compiler-provided uptime global has a `DeclId`, and `system_globals::declarations` returns the global's declaration carrying it.

**REQ-VB-analyzer-004** The symbol of each variable records the `DeclId` of its declaration (`SymbolInfo::decl_id`), so the symbol environment and the AST agree.

**REQ-VB-analyzer-005** Two declarations, and two references, that differ only by their recorded `DeclId` compare equal.

## Named references

`xform_bind_variables` runs right after the symbol environment is built. For
each reference it records the `DeclId` of the symbol `SymbolEnvironment::find`
resolves the name to from the scope the reference is written in: the enclosing
scopes innermost first, then the function blocks the outermost one `EXTENDS`,
then the global scope. The binding is stored on the reference node and left
out of `PartialEq`:

| Reference | Field |
|---|---|
| A named variable, including the record of `s.f`, the array of `a[i]` and the variable of `p^` | `NamedVariable::decl_id` |
| The control variable of a `FOR` | `For::control_decl_id` |
| The instance of a function block call | `FbCall::instance_decl_id` |
| The instance a method is called on | `MethodCall::receiver_decl_id` |

Every later pass keeps the binding: a fold that rebuilds a node carries the
field over. A name that declares no variable is left unbound, and the rules
that check references report it.

**REQ-VB-analyzer-010** A reference to a function local that hides a global of the same name binds to the local, whatever the kinds of the two: elementary, subrange, `STRING`, structure, array or function block instance.

**REQ-VB-analyzer-011** A reference in a method to a name its own local declares binds to the local, not to the function block's field of the same name; a reference to a field it does not hide binds to the field.

**REQ-VB-analyzer-012** A reference in a derived function block to a field it inherits through `EXTENDS` binds to the base block's field.

**REQ-VB-analyzer-013** An assignment to a function's own name binds to the function's result variable.

**REQ-VB-analyzer-014** The control variable of a `FOR`, the instance of a function block call and the instance a method is called on each bind to the declaration they name.

**REQ-VB-analyzer-015** The variable at the root of a field path, an array element or a dereference (`s.inner.v`, `s.items[i].v`, `a[i].inner.v`, `p^.px`) binds to its declaration.

**REQ-VB-analyzer-016** A name that declares no variable is left unbound.

## `VAR_EXTERNAL`

Aliasing is the analyzer's decision. A reference through a `VAR_EXTERNAL`
declaration binds to the global of the same name, so every reference to one
global carries one `DeclId`, and a back end gives the external no storage of its
own. Without a global of that name the reference keeps the external's own
declaration, which has no storage. A function cannot declare `VAR_EXTERNAL`
(IEC 61131-3 B.1.5.1).

Not yet built: the `VAR_EXTERNAL` declaration does not record the `DeclId` of the
global it names.

**REQ-VB-analyzer-020** A reference through `VAR_EXTERNAL` in a program or a function block binds to the global the external names, not to the external declaration.

## `VAR_IN_OUT`

A reference to a `VAR_IN_OUT` parameter binds to the parameter's own `DeclId`.
Codegen learns that the parameter is passed by reference from that declaration
(it records the declaration in `in_out_params` when it lays the parameter out),
not from a set of names.

**REQ-VB-analyzer-030** A reference to a `VAR_IN_OUT` parameter binds to the parameter.

## Late-bound names

The parser records a bare name whose meaning depends on the declarations, a
variable or an enumerated value, as `LateBound`.
`xform_resolve_late_bound_expr_kind` rewrites every one into a variable
reference or an enumerated value, so none reaches code generation, and codegen's
`LateBound` arm is an internal error rather than a variable read by name.

**REQ-VB-analyzer-040** After `resolve_types`, no expression is `LateBound`: a bare name is a variable reference or an enumerated value.

**REQ-VB-codegen-040** Compiling a `LateBound` expression is an internal error.

## Storage

Codegen keys every map of a variable's storage by `DeclId`: its variable table
slot, its type information, and the data region metadata of a `STRING`,
structure, array, array of structures or function block instance, and the set
of `VAR_IN_OUT` parameters. What kind of storage a declaration has follows from
its own declaration, never from which map a name happened to land in.

No body copies another's names. A body's storage is added when it is laid out
and released when it, and for a function block its methods, is compiled. A
function block's fields live in its own type's frame, which a block that
`EXTENDS` it cannot address yet; releasing them after the type is compiled
keeps a derived body from reaching a base field's slot even when the base type
is compiled first.

A reference whose analyzer binding is missing is an internal error. A
declaration with no storage is reported as not implemented: analysis has
already reported an undeclared name, so the gap is in code generation.

Programs that compiled before this design compile to the same container bytes,
variable indexes and debug variable names included.

**REQ-VB-codegen-001** A function local of a subrange type that hides a global `INT` of the same name keeps its own type: storing 70000 in it and returning it returns 70000.

**REQ-VB-codegen-002** A function local `DINT` that hides a global structure of the same name compiles and is the local.

**REQ-VB-codegen-003** A function local `DINT` that hides a global `STRING` of the same name compiles and is the local.

**REQ-VB-codegen-004** A function local structure that hides a program's function block instance of the same name is never the instance: until a function-local structure is given storage, reading its field is reported as not implemented.

**REQ-VB-codegen-005** Locals of the same name in two functions each have their own storage.

**REQ-VB-codegen-010** A function block reading a global array through `VAR_EXTERNAL` reads the global.

**REQ-VB-codegen-011** A write through a function block's `VAR_EXTERNAL` is seen through the program's `VAR_EXTERNAL` of the same global.

**REQ-VB-codegen-020** A derived function block's reference to an inherited field is reported as not implemented, even when the base type is compiled first.

**REQ-VB-codegen-030** A variable reference the analyzer did not bind is an internal error.

**REQ-VB-codegen-031** Two declarations of the same name have their own storage, and a `VAR_IN_OUT` parameter's slot is reached only as the reference it holds.

## Fields

Not yet built. A field access (`s.f`, `inst.x`, nested, through an array
element or a dereference, and the members `THIS^` and `SUPER^` reach) will
record the field it resolves to: a structure field as `(TypeId, field index)`, a
function block field by its `DeclId`. Codegen then uses the recorded field and
matches no name. Until then structure fields are matched by lower-cased name
(`compile_struct::walk_struct_chain`) and function block fields by name in
`FbInstanceInfo::field_indices`, and a miss in the latter is how codegen tells a
function block field from a structure field.

## Function block member visibility

Not yet built. Once a member access records its field, the analyzer decides
which members are visible from outside the block:

* Reading or writing a function block's internal `VAR` or `VAR_TEMP` from
  outside the block is rejected.
* Writing a `VAR_OUTPUT` from outside the block is rejected on the strict
  dialects. IEC 61131-3 does not permit an output assignment from outside a
  function block (Figure 13 of Edition 3, as vendor compliance statements cite
  it); reading one is permitted. CODESYS and TwinCAT accept the write, so it is
  allowed behind a dialect flag that those dialects enable.
