# Design: Variable Binding

status: partially implemented
date: 2026-10-11

## Overview

A body refers to a variable by name. Which declaration that name means is a
language rule: the scopes it is looked up in, in what order, whether a global
is reachable, and what a `VAR_EXTERNAL` stands for.

One analyzer function decides it, `SymbolEnvironment::resolve_variable`, and
it states its answer in the language's own terms: the scope the declaration
is in, and its name. The semantic rules ask that function, and lowering
allocates its variable ids from the same answer
([Lowered Program](lowered-program.md), section 3.1), so the stages cannot
disagree about which variable a name is.

[ADR-0066](../adrs/0066-name-resolution-is-an-analyzer-function.md) records
why the answer is a function of the analyzer and not an annotation on the
AST.

**Status.** Every requirement in this document is implemented and has a
conformance test. Two readers of the function are not built:

- **Lowering** does not exist yet. It will allocate one `VarId` for each
  declaration this design names (see [Readers](#readers)).
- **Code generation** still resolves names with name-keyed tables of its own
  (`codegen/src/scope.rs`), and disagrees with the analyzer where two scopes
  declare one name. The postscript to
  [ADR-0051](../adrs/0051-reject-variable-redeclaration-without-an-executable-path.md)
  records what that gets wrong.

## Problem

Before this design, four places each answered "which variable is this name",
and no two with the same rules:

| Where | What it did |
|---|---|
| `SymbolEnvironment::find` | Looked through the enclosing scopes, the `EXTENDS` chain and the global scope, and returned a symbol of any kind. |
| `rule_use_declared_symbolic_var` | Called `find`, then refused a global the body does not reach through `VAR_EXTERNAL`. No other caller made that refusal. |
| `write_collector` | Mapped a write through a `VAR_EXTERNAL` to the global of the same name. No other caller tied an external to its global. |
| Code generation | Kept one name-keyed map for each kind of variable. |

Nothing checked that a `VAR_EXTERNAL` named a global at all, or that it
declared the global's type. A structure field shared the global scope's map
with the global variables, so a field and a global of one name replaced each
other.

## 1. Naming a Declaration

```rust
/// A variable declaration, named by the scope it is declared in and its name.
pub struct DeclRef {
    pub scope: ScopeKind,
    pub name: Id,
}
```

A `DeclRef` is not an id the analyzer allocates. It is how the program names
a declaration. It is unique because a name is declared at most once in a
scope (P4014), and a library with a repeated name does not reach lowering.

**REQ-VB-analyzer-001** The `DeclRef` of a variable holds the scope that
declares the variable, and the name as that declaration spells it, whatever
case the reference uses.

## 2. Resolution

```rust
impl SymbolEnvironment {
    /// The declaration `name` refers to as a variable from the scope `from`.
    pub fn resolve_variable(&self, name: &Id, from: &ScopeKind) -> Resolution<'_>;
}

pub enum Resolution<'a> {
    /// A variable the reference may use.
    Variable { decl: DeclRef, info: &'a SymbolInfo },
    /// A global the scope does not reach.
    GlobalNotVisible { decl: DeclRef, info: &'a SymbolInfo },
    /// The name is declared, but not as a variable.
    NotAVariable,
    /// Nothing visible declares the name.
    Undeclared,
}
```

`decl` names the variable. `info` describes the declaration the name reaches
from where it is used: its section, its qualifier and its type. The two are
the same declaration except through a `VAR_EXTERNAL` (see
[A VAR_EXTERNAL is its global](#23-a-var_external-is-its-global)).

### 2.1 Lookup order

**REQ-VB-analyzer-010** A name resolves to its nearest declaration: the scope
the reference is in, then each scope that encloses it, innermost first. A
method's variable is found before a field of its function block, and a field
of the block is found from the method.

**REQ-VB-analyzer-011** After the enclosing scopes come the function blocks
the outermost scope `EXTENDS`, nearest first. The `DeclRef` of an inherited
field names the scope of the block that declares it.

**REQ-VB-analyzer-012** After those comes the global scope. A variable
declared in a function, function block or method is found before a global
variable of the same name.

**REQ-VB-analyzer-013** The own name of a function, or of a method that
declares a return type, resolves to its result variable, declared in the
scope of that function or method.

**REQ-VB-analyzer-014** A name that the nearest declaration declares as
something other than a variable, such as a type or a program, is
`NotAVariable`. A name nothing visible declares is `Undeclared`.

### 2.2 Global visibility

A body reaches a `VAR_GLOBAL` through a `VAR_EXTERNAL`, which is a
declaration in the body's own scope. The dialects that declare globals in
top-level lists let a body use a global directly.

**REQ-VB-analyzer-020** A name whose nearest declaration is a `VAR_GLOBAL` is
`GlobalNotVisible`, and `rule_use_declared_symbolic_var` reports it as P4007.

**REQ-VB-analyzer-021** With `allow_top_level_var_global` set, that name is
`Variable`.

**REQ-VB-analyzer-022** A global the compiler provides, such as
`__SYSTEM_UP_TIME`, is `Variable` on every dialect.

The symbol environment holds the setting
(`SymbolEnvironment::with_bare_globals`), so every caller of
`resolve_variable` gets the same answer for the same name.

A caller that types or checks a variable accepts `GlobalNotVisible` as well
as `Variable` (`Resolution::declared`). Reporting the unreachable global is
the one rule's job; a second pass that refused it would only lose what is
known about the variable.

### 2.3 A VAR_EXTERNAL is its global

A `VAR_EXTERNAL` declares no variable. It gives a body access to a global:
a read or a write through it reaches the global's storage.

**REQ-VB-analyzer-030** A name that reaches a `VAR_EXTERNAL` declaration
resolves to the `DeclRef` of the global variable of that name: the global
scope, and the name as the global's declaration spells it.

**REQ-VB-analyzer-031** Every unit that names one global through a
`VAR_EXTERNAL` resolves to the same `DeclRef`.

**REQ-VB-analyzer-032** Through a `VAR_EXTERNAL`, `info` describes the
`VAR_EXTERNAL` declaration, not the global: its qualifier is the one the body
sees. A write through a `VAR_EXTERNAL CONSTANT` is checked against that
declaration
([Constant Variable Inference](constant-variable-inference.md),
REQ-CVI-analyzer-051).

**REQ-VB-analyzer-033** A `VAR_EXTERNAL` may name a global the compiler
provides, and resolves to it.

**REQ-VB-analyzer-034** A `VAR_EXTERNAL` that names no global variable
resolves to its own declaration, so that the rest of the body is still
checked. It is an error (REQ-VB-analyzer-040).

## 3. Checking a VAR_EXTERNAL

An external with no global to name has nothing to reach. One that declares
another type would read the global's storage as something it is not. Both
can make a program invalid, so the analyzer reports them
([ADR-0058](../adrs/0058-the-analyzer-decides-and-lowering-translates.md)).
`rule_var_external_matches_global` reports both as P4082.

**REQ-VB-analyzer-040** A `VAR_EXTERNAL` declaration whose name no global
variable has is P4082. A type, a program or another unit's variable of that
name is not a global variable.

**REQ-VB-analyzer-041** A `VAR_EXTERNAL` declaration whose type is not the
type of the global variable it names is P4082.

Two declarations have the same type when they name the same type. A type is
identified by its id
([ADR-0055](../adrs/0055-concrete-type-ids-numbered-by-debug-tag.md)),
which settles two cases:

**REQ-VB-analyzer-042** A type and an alias of it are different types: an
external declared `INT` does not match a global declared with an alias of
`INT`. Two structure types with the same fields are different types.

**REQ-VB-analyzer-043** Two arrays spelled out in place, each a type of its
own, are the same type when they have the same bounds in every dimension and
the same element type.

An array is the only type a `VAR_GLOBAL` or a `VAR_EXTERNAL` can spell in
place today.

**REQ-VB-analyzer-044** P4082 is reported on every dialect. No compiler
option relaxes it.

A declaration whose type did not resolve is left to the rule that reports
the undeclared type.

The compiler checks a set of files as one unit, so the global an external
names has to be among the files checked. A file that declares only a function
block with a `VAR_EXTERNAL` is reported when it is checked without the file
that declares the global, in the same way that a file is reported when it
uses a type another file declares.

## 4. Structure Fields

**REQ-VB-analyzer-050** A field of a structure is declared in the scope of
its structure type, not in the scope the type is declared in. A global
variable and a structure field of the same name are both declared, whichever
comes first in the source, and the name resolves to the global variable.

Which field a field access names is decided from the type of the record
(`TypeEnvironment::field_type`), not by this function.

**REQ-VB-analyzer-051** A field access has the type its field is declared
with, as `TypeEnvironment::field_type` records it, whatever that type is: the
output `p` of a function block instance `f`, declared as a structure, makes
`f.p` that structure, and a `STRING` output `s` makes `f.s` a `STRING`.

`variable_type::of` walks a reference by type id, through
`TypeEnvironment::field_type` for a field, and never reads a field's type from
the fields a representation lists (see
[Expression Type Resolution](expression-type-resolution.md)).

## 5. Properties

A `PROPERTY` of a function block is not a variable: reading or writing it
calls its `GET` or `SET` accessor ([Lowered Program](lowered-program.md),
section 4). Its name still takes part in the lookup, so that it hides what a
variable of the same name farther out would have bound to. Each accessor is a
method named after the property, with a scope of its own.

**REQ-VB-analyzer-060** A property is declared in the scope of its function
block. A name whose nearest declaration is a property is `NotAVariable`, and
`rule_use_declared_symbolic_var` reports its use as not implemented (P9999).

**REQ-VB-analyzer-061** A property is found before a global variable of the
same name, and a property a block inherits through `EXTENDS` is found from the
derived block, as a field is.

**REQ-VB-analyzer-062** A property and a variable of the same name in one
function block are P4014.

**REQ-VB-analyzer-063** The `GET` and `SET` accessors of a property each have
a scope of their own, `<property>.GET` and `<property>.SET` inside the block's
scope, so a variable each declares under one name is two declarations. Inside
`GET` the property's name is its result variable; inside `SET` it is the
implicit input that holds the assigned value.

**REQ-VB-analyzer-064** A variable that a function block declares with the
name of a property one of the blocks it `EXTENDS` declares is P4044, as one
with the name of an inherited field is.

## Readers

**The semantic rules.** Each of these asks `resolve_variable` which variable
a name is:

- `rule_use_declared_symbolic_var` (P4007 and its suggestions);
- `variable_type::declared` and `variable_type::of`;
- `xform_resolve_expr_types`;
- `rule_ref_to`;
- `rule_function_call_in_out_argument`;
- `rule_function_call_type_check`;
- `rule_constant_not_written`;
- `write_collector`, for the variable a write through a `VAR_EXTERNAL`
  changes.

Code that asks about another kind of symbol calls `SymbolEnvironment::find`.

**Lowering** (not built). It allocates one `VarId` for each `DeclRef`.
Because a `VAR_EXTERNAL` resolves to its global's `DeclRef`, an external gets
no variable of its own, which is what REQ-LOW-lowering-022 requires, and two
variables of one name in different scopes get two ids, which is what
REQ-LOW-lowering-020 requires. Lowering reads the function from the
`SemanticContext` that analysis returns.

## Out of Scope

- **Passes that run before the symbol environment exists** keep name tables
  of their own: `xform_resolve_constant_expressions`,
  `xform_resolve_late_bound_expr_kind` and
  `xform_fold_initializer_expressions`. Moving them onto the environment means
  building it earlier in `stages::resolve_types`.
- **Field identity.** Lowering names a field by position (`FieldIdx`).
- **Function block member visibility**: reading or writing a block's `VAR`
  from outside the block. That needs only the type environment.
- **Code generation.** It keeps its own name tables until a construct is
  generated from the lowered program.
