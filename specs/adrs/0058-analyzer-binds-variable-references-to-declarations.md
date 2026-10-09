# The Analyzer Binds Variable References to Declarations

status: accepted
date: 2026-10-09

## Context and Problem Statement

Which declaration a name refers to is a language rule. A local hides a global
of the same name, a method's local hides its function block's field, a derived
block sees the fields it inherits, and a reference through `VAR_EXTERNAL` is
the global it names. The analyzer applies those rules
(`SymbolEnvironment::find`), but it recorded nothing, so code generation
applied its own.

Codegen built name-keyed maps per program organization unit: one map from name
to variable table slot, one from name to type information, and one per kind of
aggregate (`STRING`, structure, array, array of structures, function block
instance) from name to its data region metadata. A function or function block
body was compiled against a copy of the program's maps, filtered to the
globals. A local only replaced the entry in the map for its own kind, so an
entry of another kind for the same name survived, and every lookup tried the
maps in its own order. The analyzer and codegen disagreed, and programs
miscompiled:

| Program | Before |
|---|---|
| Global `x : INT`; function local `x : BIG` (`DINT(0..100000)`); `x := 70000` | Returned 4464: the local kept the global's `INT` type |
| Program `t : TON := (PT := T#5s)`; function local `t : REC`; `F := t.PT` | Returned `T#5000ms`, the program's instance's field |
| Global `p : POINT`; function local `p : DINT` | P9998 internal error |
| Global array read in a function block through `VAR_EXTERNAL` | P9999: arrays were not copied into a body |

ADR-0056 set the principle that the analyzer records the language's decisions
in the AST and a backend lowers them. Where should the decision of which
declaration a name refers to live?

## Decision Drivers

* **One answer per reference**, made once by the rules the analyzer already
  applies, that every backend reads.
* **Backends lower; they do not decide** what a name means.
* **Programs that compile correctly today compile to the same bytes**,
  variable indexes and debug names included.
* **No side table keyed by scope and name.** Such a table is the shape of the
  bug: it can be keyed differently from how the analyzer resolves.

## Considered Options

* Give every declaration an identity and record it on every reference
* Keep a side table from (scope, name) to declaration in the semantic context
* Keep name resolution in codegen and fix its scoping

## Decision Outcome

Chosen option: "Give every declaration an identity and record it on every
reference".

* **`DeclId`** (`ironplc_dsl::decl_id`) identifies one variable declaration.
  `xform_assign_decl_ids` gives one to every `VarDecl` and `EdgeVarDecl`, to the
  implicit result variable of a function and of a method with a return type,
  and to each compiler-provided global. The symbol environment records the same
  id (`SymbolInfo::decl_id`).
* **`xform_bind_variables`** records on every reference the `DeclId` that
  `SymbolEnvironment::find` resolves it to from where it is written:
  `NamedVariable::decl_id`, `For::control_decl_id`, `FbCall::instance_decl_id`
  and `MethodCall::receiver_decl_id`. A reference through `VAR_EXTERNAL` binds
  to the global it names. Like `type_id`, the ids are left out of `PartialEq`.
* **Codegen keys storage by `DeclId`.** Every per-variable map is keyed by
  declaration, a body's storage is released once the body is compiled, and no
  scope is copied into another. A reference without a binding is an internal
  error. `scope.rs` is gone.

Field accesses still match names, and function block member visibility is not
yet decided by the analyzer; `specs/design/variable-binding.md` records how
both are to be built.

### Consequences

* Good, because the analyzer and codegen cannot disagree about which
  declaration a name means: codegen no longer resolves names.
* Good, because a local that hides a global has its own storage and its own
  type whatever their kinds, and a global array is reachable through
  `VAR_EXTERNAL` in a function block.
* Good, because the language server and a second backend can read the same
  bindings.
* Good, because every program the codegen test suite and the repository's
  examples compile produces the same container bytes as before.
* Bad, because the reference nodes carry a field a node built by hand leaves
  `None`, and a pass that builds a reference after binding must bind it.
  Every pass that builds one runs before the binder today.
* Neutral, because a function-local structure still has no storage: the
  second row above is now reported as not implemented instead of reading the
  program's instance, and giving it storage is a separate change.

### Confirmation

The `REQ-VB-analyzer-*` and `REQ-VB-codegen-*` requirements in
`specs/design/variable-binding.md` have conformance tests: the bindings the
analyzer records for each kind of reference and each hiding combination, and
end-to-end tests of each row above. The codegen suite, including the wire
format and debugger tests, passes unchanged.

## Pros and Cons of the Options

### Give every declaration an identity and record it on every reference (chosen)

* Good, because the binding sits on the reference, where every consumer that
  walks the tree finds it.
* Good, because storage keyed by identity cannot collide the way storage keyed
  by name does.
* Bad, because several AST nodes gain a field and a manual `PartialEq`.

### Keep a side table from (scope, name) to declaration in the semantic context

* Good, because the AST does not change.
* Bad, because every consumer has to track scopes the way the analyzer does to
  key into the table, which is the disagreement this decision removes.

### Keep name resolution in codegen and fix its scoping

* Good, because it is a smaller change to the analyzer.
* Bad, because every backend repeats the language's scope rules, and nothing
  checks that they match the analyzer's.
