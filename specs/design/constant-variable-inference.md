# Design: Constant Variable Inference

## Overview

A variable the program never writes holds its initial value for the whole
run. It is a constant in fact, whether or not the source says `CONSTANT`.
The analyzer makes that fact explicit: the transform
`xform_mark_unwritten_constants` gives the `CONSTANT` qualifier to every
declaration it can prove is never written, so that every later stage --
the semantic rules, and above all code generation -- sees one notion of
"constant variable" instead of two.

The motivation is constant folding
([#1612](https://github.com/ironplc/ironplc/issues/1612)). `LEN('Hello')`
and `LEN(msg)` for a `msg : STRING := 'Hello'` that nothing assigns should
both fold to `5` at compile time. Code generation can fold the first from
the literal alone. For the second it needs to know the variable cannot
change, and the cleanest place to establish that is a single analyzer pass
rather than a write search inside each folding site in the code generator.
With this transform in place, code generation only has to understand two
cases -- a literal, and a `CONSTANT`-qualified variable -- and every
future folding opportunity benefits, not only `LEN`.

## Contract for later stages

After `stages::resolve_types` returns, a `VarDecl` whose qualifier is
`DeclarationQualifier::Constant` holds its initial value for the whole
run, whether the qualifier came from the source or from this transform.
Code generation may read the initializer instead of the variable's storage
and may skip the storage altogether when nothing else needs it.

Two consequences follow, and both are deliberate:

- A tool that lets the operator change a variable while the program runs
  (the debugger's `setVariable`, which the current server refuses) must
  refuse an inferred constant just as it would refuse a declared one, or
  the folded reads and the stored value will disagree. The transform makes
  no distinction between the two, and neither should such a tool.
- The transform runs on the whole library as one unit. A variable is only
  constant with respect to the program that was analyzed with it; the
  qualifier is a property of the analyzed library, not of the declaration
  in isolation, and is never written back to source.

## What counts as a write

The transform is **conservative**: it may fail to mark a variable that is
in fact never written, but it never marks a variable that is. Every
construct through which a program can change a variable is a write, and a
write to any part of a variable is a write to the whole variable.

| Requirement | Construct | Written variable |
|-------------|-----------|------------------|
| **REQ-CVI-analyzer-010** | Assignment `v := e`, including to an array element, structure field, bit or partial access of `v` | The root variable `v` of the access chain |
| **REQ-CVI-analyzer-011** | `FOR i := ...` | The control variable `i` |
| **REQ-CVI-analyzer-012** | Output binding `Q => v` on a function, function block or method call | `v` |
| **REQ-CVI-analyzer-013** | A variable argument bound to a `VAR_IN_OUT` parameter of a function, function block or method: by name, or by position for a function (a positional function-block argument occupies a `VAR_INPUT` only) | The argument |
| **REQ-CVI-analyzer-014** | `REF(v)` or `ADR(v)`, including the `REF=` binding | `v` (its address is taken, so it may be written through the reference) |
| **REQ-CVI-analyzer-015** | A variable argument to a callee whose parameter directions cannot be determined (an undeclared function, an instance of an unknown type, an argument the callee declares no parameter for) | The argument |
| **REQ-CVI-analyzer-016** | A function-block instance initializer or configuration `VAR_CONFIG` that sets a member value: `inst : FB := (count := 5)` | The member `count` |
| **REQ-CVI-analyzer-017** | A configuration program connection sink `prog(Q => g)` | The global `g` |
| **REQ-CVI-analyzer-018** | A `VAR_ACCESS` path with direction `READ_WRITE` or no direction | The variable at the end of the path |
| **REQ-CVI-analyzer-019** | An SFC action association `A(N, ind)` | The action name `A` (which may be a Boolean variable) and each indicator `ind` |

A variable argument bound to a `VAR_INPUT` parameter is **not** a write
(part of REQ-CVI-analyzer-013): `LEN(msg)` and `timer(PT := delay)` leave
`msg` and `delay` constant. A standard-library function block declares no
`VAR_IN_OUT`, so every argument to one is a read.

The callee is found the way the invocation rules find it: the instance's
declared type from the declarations of the unit being walked, the block or
method from the library (`callee_resolution`), and the argument-to-parameter
binding from `call_assignment_check`. The transform adds no lookup of its
own.

**REQ-CVI-analyzer-020** An assignment to a member of a function-block
instance from outside the block, `inst.count := 5`, writes the member
`count` as well as the instance `inst`, so the block's own declaration of
`count` is not marked.

### Writes are tracked by name, not by scope

**REQ-CVI-analyzer-021** A write to a variable named `x` anywhere in the
library prevents every declaration named `x` in the library from being
marked, in every program organization unit.

This is coarser than scope-aware tracking, and it is chosen on purpose.
The name-only rule is sound by construction: it cannot be wrong about
which declaration a write reaches, because it does not try to decide.
Inheritance (a derived block writing a field of its base), methods writing
the fields of their block, and `VAR_EXTERNAL` aliasing of a `VAR_GLOBAL`
all fall out without any scope modelling. The cost is that two unrelated
units reusing a name share one verdict. That precision can be bought
later without changing the contract above; nothing downstream depends on
how the set of written names is computed.

## Which declarations are marked

**REQ-CVI-analyzer-001** A `VAR` or `VAR_TEMP` declaration is marked
`CONSTANT` when it has no qualifier, a symbolic identifier, an initializer,
and its name is never written.

The following are never marked, each for a reason the semantic rules or
the runtime already enforce:

| Requirement | Declaration | Reason |
|-------------|-------------|--------|
| **REQ-CVI-analyzer-002** | No initializer (`x : INT;`) | P4008 requires a `CONSTANT` to have an initializer; the default value is constant but the rule would reject the declaration |
| **REQ-CVI-analyzer-003** | Located (`x AT %IX0.0 : BOOL := TRUE`) | Hardware writes it |
| **REQ-CVI-analyzer-004** | `VAR_INPUT`, `VAR_OUTPUT`, `VAR_IN_OUT` | The caller writes an input or in-out; the body writes an output the caller reads |
| **REQ-CVI-analyzer-005** | Already `CONSTANT`, `RETAIN` or `NON_RETAIN` | The qualifier is one field; replacing `RETAIN` would lose it, and a declared `CONSTANT` needs nothing |
| **REQ-CVI-analyzer-006** | A function-block instance | P4010 forbids a `CONSTANT` function block, and calling the instance writes its state |

The initializers accepted are exactly the ones `rule_var_decl_const_initialized`
accepts without further checking: a simple, string or enumerated initial
value that is present, or an array initializer with at least one element.
A structure initializer is not marked, because the rule additionally
requires every field without a type default to be initialized, and a
variable that is never written may legitimately leave such a field at
zero.

### Globals and externals

**REQ-CVI-analyzer-030** A `VAR_GLOBAL` declaration is marked when every
global declaration of that name qualifies under REQ-CVI-analyzer-001 and
the name is never written; every `VAR_EXTERNAL` declaration of that name
is marked with it.

**REQ-CVI-analyzer-031** When the global is not marked -- because it is
written, has no initializer, or carries another qualifier -- its
`VAR_EXTERNAL` declarations are left unchanged, and so is any local
declaration of the same name.

The pair is required by P4009 (`VariableMustBeConst`): a constant global
must be declared constant in every unit that references it. Marking one
side without the other would introduce that diagnostic into a program
that had none. The rule that enforces P4009 collects the name of every
`CONSTANT` declaration, local ones included, which is why a local
declaration shares the verdict of a same-named global that cannot be
marked.

## Pipeline position

**REQ-CVI-analyzer-040** The transform runs inside `stages::resolve_types`,
after every other transform, so the library that `analyze` returns and
that code generation receives carries the inferred qualifiers.

**REQ-CVI-analyzer-041** The transform introduces no diagnostics: a
program that analyzes cleanly before marking analyzes cleanly after it.

It runs last because it needs bare identifiers already resolved to
variables (`xform_resolve_late_bound_expr_kind`), `ADR` already rewritten
to `ExprKind::Ref` (`xform_resolve_adr`), user functions already in the
function environment and named arguments already positional
(`xform_resolve_symbol_and_function_environment`,
`xform_named_to_positional_args`). It runs before the semantic rules so
that the rules see the same library code generation will, and so that
REQ-CVI-analyzer-041 is checked by the rules themselves rather than
assumed. The transform is infallible; there is no fallback to revert to.

## Out of scope

- Folding reads of constant variables in code generation (the `LEN` fold
  of #1612 and any other). This document only establishes the qualifier.
- Scope-aware write tracking (see REQ-CVI-analyzer-021).
- Structure-typed variables (see the initializer note above).
- Reference-typed variables (`REF_TO`, `POINTER TO`, `REFERENCE TO`): the
  constancy of a reference says nothing about its target, and no folding
  opportunity depends on it.
