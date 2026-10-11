# Name Resolution Is an Analyzer Function, Not an AST Annotation

status: accepted
date: 2026-10-11

## Context and Problem Statement

A body refers to a variable by name, and more than one stage needs to know
which declaration the name means: the semantic rules, to type and check the
reference; lowering, to give it a variable id; and today's code generation,
to find its storage.

Four places answered that question, each with its own rules. The symbol
environment's `find` returned a symbol of any kind. One semantic rule alone
refused a global the body does not reach through `VAR_EXTERNAL`. The write
collector alone tied a `VAR_EXTERNAL` to its global. Code generation kept
name-keyed maps of its own. They disagreed, and a program that analysis
accepted could be compiled against the wrong declaration (see the postscript
to [ADR-0051](0051-reject-variable-redeclaration-without-an-executable-path.md)).

[Pull request 2175](https://github.com/ironplc/ironplc/pull/2175) fixed the
disagreement by giving every declaration an id, a `DeclId`, and recording
that id on every reference in the AST. It was closed without merging.

Where is the answer made, and in what form is it given?

## Decision Drivers

* **One answer** to which declaration a name means, which every stage reads.
* **The AST holds the program as written**, and decisions recorded in the
  language's own vocabulary, such as a declaration's type or an implicit
  conversion ([ADR-0056](0056-analyzer-records-implicit-conversions-in-the-ast.md)).
* **Variable identity for storage is lowering's.** The lowered program names
  a variable by a `VarId` it allocates
  ([Lowered Program](../design/lowered-program.md), section 3.1).
* **A problem with a program is reported by analysis**
  ([ADR-0058](0058-the-analyzer-decides-and-lowering-translates.md)). An
  undefined variable, a global the body does not reach and a `VAR_EXTERNAL`
  with no global are all problems of this kind.

## Considered Options

* An analyzer function that names the declaration by its scope and its name
* An id on every declaration and every reference in the AST
* A table the analyzer builds, keyed by scope and name
* Each later stage resolves names itself

## Decision Outcome

Chosen option: "An analyzer function that names the declaration by its scope
and its name", because it gives every stage one answer without putting an
identity into the AST that the source does not have.

* **`SymbolEnvironment::resolve_variable(name, from)`** is the one place that
  decides which declaration a variable name means from a scope. It applies
  the lookup order, whether a global is reachable, and what a `VAR_EXTERNAL`
  stands for.
* **The answer is a `DeclRef`**: the scope the declaration is in, and its
  name. That is how the program itself names a declaration. It is unique
  because a name is declared at most once in a scope, and a library that
  repeats one does not pass analysis.
* **A `VAR_EXTERNAL` resolves to its global's `DeclRef`.** An external is not
  a variable of its own.
* **The semantic rules ask the function.** A rule that needs another kind of
  symbol still calls `find`.
* **Lowering allocates one `VarId` for each `DeclRef`.** It decides nothing
  about which declaration a name means.
* **The AST is not changed.**

[Variable Binding](../design/variable-binding.md) specifies the function.

### Consequences

* Good, because the rules and lowering cannot disagree about a name.
* Good, because the refusal of an unreachable global and the tie between a
  `VAR_EXTERNAL` and its global are each stated once.
* Good, because the language server can ask what a name means without
  running lowering.
* Good, because nothing in the AST has to be kept in step with the
  declarations when a transform moves or adds one.
* Bad, because a reader looks a name up each time it meets a reference, where
  an annotation would be read from the node.
* Bad, because a `DeclRef` is unique only in a library that analysis accepts.
  A stage that runs on a library with a repeated name cannot rely on it.
* Bad, because the passes that run before the symbol environment is built
  still keep name tables of their own.
* Bad, because code generation keeps its own tables until a construct is
  generated from the lowered program, so it can still disagree with the
  analyzer.

## Pros and Cons of the Options

### An analyzer function that names the declaration by its scope and its name (chosen)

* Good, because the symbol environment already holds every declaration by
  scope and name; the function adds rules, not a second store.
* Good, because the answer is in terms a diagnostic and the language server
  can show.
* Bad, because every reader pays for a lookup.

### An id on every declaration and every reference in the AST

Pull request 2175 took this option.

* Good, because a reader gets the declaration from the node, without a
  lookup.
* Good, because the id is unique whatever the library contains.
* Bad, because the id means nothing in the source. It exists to key storage,
  and the lowered program already has an id for that.
* Bad, because every transform that builds or copies a reference has to carry
  the id, and one that forgets leaves a reference bound to nothing.
* Bad, because lowering would translate one identity into another.

### A table the analyzer builds, keyed by scope and name

A pass would resolve every name from every scope once, and readers would
look the result up.

* Good, because each name is resolved once.
* Bad, because the table restates what the symbol environment holds, and the
  two can drift apart.
* Bad, because the pass has to know every scope a name can be asked from
  before any reader asks.

### Each later stage resolves names itself

This is what code generation does today.

* Good, because nothing changes.
* Bad, because every stage repeats the scope rules, and two stages can bind
  one name to two declarations. That is the defect this decision removes.
* Bad, because a stage after analysis cannot report that a program is
  invalid (ADR-0058), so a name it cannot resolve is reported as a defect of
  the compiler.

## More Information

* [Variable Binding](../design/variable-binding.md)
* [Lowered Program](../design/lowered-program.md), sections 3.1 and 4
* `compiler/analyzer/src/symbol_environment.rs`
* [Pull request 2175](https://github.com/ironplc/ironplc/pull/2175), the
  closed attempt to record a `DeclId` in the AST
