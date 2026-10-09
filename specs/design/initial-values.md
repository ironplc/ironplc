# Design: Initial Values Completed by the Analyzer

status: implemented
date: 2026-10-09

## Overview

What value a variable starts with is a language rule. This design moves the
rule out of code generation and into the analyzer: the analyzer resolves the
starting value of every declaration and completes the declaration's
initializer with it, in place. After analysis an initializer is no longer
only what the program wrote: it is the whole starting value, every field and
element listed, every scalar a literal of the type it is stored as. A back
end reads the value from the initializer and decides nothing, so every back
end starts every variable at the same value by construction (ADR-0056 states
the principle; ADR-0045 records its application to initial values).

A declaration holds one answer to what it starts with. The value is not kept
beside the initializer, where the two could disagree; the initializer *is*
the value, and a later lowering works from it.

The design builds on:

- **[ADR-0045](../adrs/0045-variable-initialization-in-bytecode.md)**:
  initialization runs once, in the init function, before the first scan.
- **[ADR-0024](../adrs/0024-function-local-reinit-via-bytecode-prologue.md)**:
  a function's locals are set again by a bytecode prologue on every call.
- **[Enumeration Code Generation](enumeration-codegen.md)**: an enumeration's
  default comes from the analyzer by `TypeId`, the model this design extends
  to every type.
- **[Subrange Code Generation](subrange-codegen.md)** and
  **[Structure Code Generation](structure-codegen-memory-layout.md)**: the
  storage of the values this design resolves.

## The rule

The starting value of a variable of type `T` is, in order:

1. the declaration's own initializer;
2. else the default `T` declares, through every alias and subrange layer
   (`TYPE MYINT : INT := 7`, `TYPE R : DINT(10..100) := 50`);
3. else `T`'s implicit default: an enumeration's default member, a
   subrange's lower bound, an empty string, `NULL`, or zero;

recursively for every field and element. The default of a structure is the
value of each of its fields by this rule. The default of an array is its
element default in every element. A function block instance starts with the
value its block declares for each input, output and internal variable, with
the instance's own initializer applied over them.

An initializer may state less than the whole value:

- a partial structure initializer (`(z := 4)`) leaves the fields it does not
  name at their defaults;
- an array initializer is expanded: a repetition `2(5)` is two elements, and
  `n()` is `n` elements at the element default;
- an array initializer with fewer values than elements leaves the rest at
  the element default; one with more is a `check` error.

The rule is the same for program variables, globals, function and method
locals (`VAR` and `VAR_TEMP`), the result of a function or method, function
block fields, structure fields and array elements.

```
TYPE
    MYINT : INT := 7;
    R2 : DINT(10..100);
    P : STRUCT x : R2; y : INT := 3; END_STRUCT;
END_TYPE
VAR
    m : MYINT;                               (* 7 *)
    r : R2;                                  (* 10 *)
    p : P := (y := 4);                       (* (x := 10, y := 4) *)
    a : ARRAY[1..4] OF MYINT := [1, 2(5)];   (* [1, 5, 5, 7] *)
END_VAR
```

## 1. The completed initializer

The analyzer completes the value slot of each declaration's initializer, and
keeps the kind of initializer and the type it names:

| Type | Completed initializer |
|---|---|
| elementary, or an alias of one | `Simple` with its constant: `TRUE`/`FALSE` for a `BOOL`, an integer literal for an integer or bit string, a real literal for a `REAL` or `LREAL`, a duration, date or time-of-day literal for those types |
| enumeration | `EnumeratedType` or `EnumeratedValues` with its member |
| subrange | `Subrange` with its value |
| string | `String` with its literal, of the string's width and never longer than it |
| reference | `Reference` with `NULL` or `REF(x)` |
| array | `Array` with every element, listed flat in storage order (the last subscript varies fastest), whatever the array's shape |
| structure | `Structure` with every field, in declaration order |
| function block | `FunctionBlock` with every input, output and internal variable, in declaration order |

An element or field holds a value of the same form: a literal, a member, an
`Expression` (`NULL`, or a member expression, which is evaluated when the
variable is initialized; `--allow-struct-initializer-expressions`), a list of
elements, or a list of fields. `ArrayInitialElementKind::Structure` holds an
element that is a structure. A literal that initializes a type of another
form is rewritten as one of that type (`r : REAL := 2` holds `2.0`), at the
span it was written at.

Every part the program did not write at the declaration -- the type's
default, a field a partial initializer leaves out, the elements an array
initializer does not reach -- has a synthesized span
(`SourceSpan::synthesized`). A rendering of the program as written leaves
those parts out (section 4).

```
p : P := (y := 4);       (* completed: (x := ~10, y := 4), ~ synthesized *)
a : ARRAY[1..4] OF MYINT := [1, 2(5)];   (* [1, 5, 5, ~7] *)
```

A function's or method's result, which the program declares only by its
return type, gets a `VarDecl` of its own (`FunctionDeclaration::result`,
`MethodDeclaration::result`) whose initializer is completed the same way.

A value is range-checked before it is written: a constant outside its type
is reported by a rule, and the declaration's initializer is left as written.

## 2. Analyzer

The defaults of declared types are resolved once per `TypeId`
(`TypeDefaults`) right after declaration types are resolved, while each
`TYPE` declaration still names the types it was written with. They are kept
on the `SemanticContext`; they are analyzer-internal, and no back end reads
them. Each declaration's initializer is completed after implicit conversions
are recorded and after the range rule checks the literals the program wrote
(`xform_resolve_initial_values`), so that an expression member carries its
conversions. The value is built from the same leaves an initializer holds
and written back as the initializer; it is not kept anywhere else.

### Completed initializers

**REQ-IV-analyzer-001** The analyzer completes the initializer of every declaration that holds a value so that its value slot holds the declaration's starting value, and leaves the initializer of a `VAR_IN_OUT` or `VAR_EXTERNAL`, which names another variable, as written.

**REQ-IV-analyzer-002** Every part of a completed initializer the program did not write at the declaration -- a default, a field the initializer leaves out, an element it does not reach -- has a synthesized span, and every part it wrote keeps its own.

**REQ-IV-analyzer-003** A scalar value is a literal of the kind its type stores, at the span the program wrote it: an integer literal that initializes a `REAL` is rewritten as a real literal, and one that initializes a `BOOL` as `TRUE` or `FALSE`.

**REQ-IV-analyzer-004** A function's or method's result, which the program declares only by its return type, gets a `VarDecl` of its own (`FunctionDeclaration::result`, `MethodDeclaration::result`) whose initializer holds the default of the return type.

**REQ-IV-analyzer-005** A function local and a global of the same name get the value their own declarations give: values are recorded on declaration nodes, never looked up by scope and name.

**REQ-IV-analyzer-006** A declaration whose initializer states no value and is of a kind that cannot hold its type's value (a top-level `VAR_GLOBAL` of a structure type, which the type resolver does not classify) is completed in the kind its type takes.

### Structures

**REQ-IV-analyzer-010** A structure field declared with an initializer (`a : INT := 5`) starts at it in every variable of the structure type.

**REQ-IV-analyzer-011** A structure field of an alias type (`b : MYINT` with `MYINT : INT := 7`) starts at the alias's declared default.

**REQ-IV-analyzer-012** A structure field of a subrange type with a declared default (`r : RNG` with `RNG : INT(1..10) := 5`) starts at that default.

**REQ-IV-analyzer-013** A `STRING` field starts at its declared default, and a structure initializer's value for it replaces the default.

**REQ-IV-analyzer-014** An `ARRAY` field starts at its declared default, and a structure initializer's value for it replaces the default.

**REQ-IV-analyzer-015** A partial structure initializer leaves the fields it does not name at their defaults.

**REQ-IV-analyzer-016** Structures nested in structures, and structures in arrays in structures, start at the defaults of their innermost fields.

**REQ-IV-analyzer-017** A structure field of an inline subrange type that states a value (`level : INT (0..15) := 3`) starts at it, and one that states none at the lower bound.

### Scalars

**REQ-IV-analyzer-020** A program variable of an alias type with a declared default (`m : MYINT`) starts at the default.

**REQ-IV-analyzer-021** A variable of a subrange type with a declared default (`R : DINT(10..100) := 50`) starts at the default.

**REQ-IV-analyzer-022** A program variable, a function local and a function block output of a subrange type without a declared default start at its lower bound.

**REQ-IV-analyzer-023** A function's `VAR_TEMP` declared with an initializer starts at it and is flagged to start again on every call.

**REQ-IV-analyzer-024** The result of a function returning an enumeration with a declared default starts at that default.

**REQ-IV-analyzer-025** The result of a function returning a subrange starts at its lower bound.

### Arrays

**REQ-IV-analyzer-030** An array initializer's repetitions are expanded, and an empty repetition `n()` gives `n` elements at the element default.

**REQ-IV-analyzer-031** An array initializer with fewer values than elements leaves the rest at the element default, which is the element type's declared default when it has one.

**REQ-IV-analyzer-032** Every element of an array of structures starts at the structure's default.

**REQ-IV-analyzer-033** An array initializer with more values than the array has elements is reported as P4077, and the declaration's initializer is left as written.

**REQ-IV-analyzer-034** An array's value lists every element flat, in storage order, whatever the array's shape: an array of arrays lists the elements of its innermost arrays, as an initializer does.

### Function block instances and references

**REQ-IV-analyzer-040** An instance of a standard function block starts with its member initializer applied over the block's defaults.

**REQ-IV-analyzer-041** An instance of a user-defined function block starts with its member initializer applied over the values the block declares for its variables.

**REQ-IV-analyzer-042** A reference starts at `NULL` unless its initializer names a variable (`REF(x)`), which it then refers to.

### Re-initialization

**REQ-IV-analyzer-050** The `VAR` and `VAR_TEMP` declarations of a function or method, and its result, are flagged (`reset_on_call`) to start again on every call; a program's and a function block's variables, and a function's inputs, are not.

## 3. Code generation

Codegen reads each value from the completed initializer
(`codegen/src/initial_value.rs`), against the declared type, and decides only
where each value is stored and which stores to emit.
The variable table and the data region start cleared, so the init function
omits a store that would leave storage as it starts: an elementary scalar of
value zero, an array element or a field of an array element that is zero.
An enumeration, a subrange and every field of a structure variable are
stored whatever their value, as the init function has always stored them.
The prologue of a function or method stores every flagged value whatever it
is, since it overwrites what the last call left.

**REQ-IV-codegen-001** A declaration whose initializer reaches code generation without a complete value is reported as an internal error rather than given a default.

**REQ-IV-codegen-010** A structure variable's fields start at the defaults the structure declares.

**REQ-IV-codegen-011** A structure field of an alias type starts at the alias's declared default.

**REQ-IV-codegen-012** A structure field of a subrange type starts at the subrange's declared default.

**REQ-IV-codegen-013** A `STRING` field starts at its declared default, or at the value a structure initializer gives it.

**REQ-IV-codegen-014** An `ARRAY` field's elements start at its declared default, or at the values a structure initializer gives them.

**REQ-IV-codegen-015** A structure field of an inline subrange type starts at the value its declaration states, and at the lower bound when it states none.

**REQ-IV-codegen-020** A program variable of an alias type starts at the alias's declared default.

**REQ-IV-codegen-021** A variable of a subrange type with a declared default starts at the default.

**REQ-IV-codegen-022** A program variable, a function local and a function block output of a subrange type start at its lower bound.

**REQ-IV-codegen-023** A function's `VAR_TEMP` is reset to its initial value on every call.

**REQ-IV-codegen-024** A function returning an enumeration that assigns no result returns the enumeration's declared default.

**REQ-IV-codegen-025** A function returning a subrange that assigns no result returns its lower bound.

**REQ-IV-codegen-030** An array of strings initialized with an empty repetition holds empty strings in the elements it covers.

**REQ-IV-codegen-031** An array initializer with fewer values than elements leaves the rest at the element type's default.

**REQ-IV-codegen-032** Every element of an array of structures starts at the structure's default.

**REQ-IV-codegen-033** A program whose array initializer has more values than elements does not reach code generation: analysis reports P4077.

**REQ-IV-codegen-040** A function block instance starts with the values its block declares for its variables, with the instance's member initializer applied over them.

**REQ-IV-codegen-041** A function block member initialized by an expression (an extension) is evaluated when the instance is initialized.

## 4. Rendering

**REQ-IV-plc2plc-001** Rendering an analyzed library writes each initializer as the program wrote it: a value, field or element with a synthesized span is left out, and so is the `:=` of an initializer the program wrote no part of.

## 5. What changes for an existing program

A program whose variables started at the right values keeps the same
container bytes, with three exceptions:

- An initializer that states the value storage already starts at (`:= 0`,
  `:= FALSE`, `:= ''`, a zero array element) is no longer stored, because
  codegen sees the value and not whether the program wrote it.
- Function block fields and array elements of a reference type are now
  stored `NULL`, where they started at variable-table index 0, a reference
  to an unrelated variable.
- A function's `VAR_TEMP` is reset on every call, as its `VAR` is; it kept
  the value the last call left.

A program that states a subrange value its subrange cannot hold -- a
subrange type's declared default (`TYPE R : DINT(10..100) := 500`) or a
structure field's value (`x : INT (0..15) := 20`) -- is now reported as a
constant overflow. Both values were ignored, the variable starting at the
lower bound.

## Out of scope

- Where storage lives: slot and data-region layout, and descriptors.
- `RETAIN` and `PERSISTENT` semantics.
- The call-style function block initializer `name : FB(args)`, which stays
  refused by `rule_function_block_call_unsupported`.
- An expression initializer for a structure field, which codegen refuses as
  not implemented, and a non-empty initial value for a `STRING` nested inside
  an array element, whose header codegen does not write.
- Function block fields of a string, array or structure type, which have no
  storage of their own in an instance; a non-empty starting value for one is
  refused as not implemented.
