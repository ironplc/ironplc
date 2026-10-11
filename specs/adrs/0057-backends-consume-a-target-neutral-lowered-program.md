# Backends Consume a Target-Neutral Lowered Program

status: proposed
date: 2026-10-07

## Context and Problem Statement

Code generation compiles the analyzed syntax tree. That is the tree the parser
built, annotated in place by analysis, and it serves three consumers with
different needs:

* **Rendering a program back to source** needs it faithful to the source.
* **The language server** needs it to hold a broken program.
* **Code generation** needs every name resolved and every type known.

A tree that can hold a broken program cannot also promise a resolved one, so
code generation checks at run time. It looks names up, keeps an error path for
each state analysis has ruled out, and makes some of the language's decisions
itself.

A WebAssembly backend is planned, and an LLVM backend is possible. Each would
have to make those decisions again, and two implementations of one decision can
disagree. A program that behaves differently on two targets is a safety defect
in a PLC.

What should a backend consume?

## Decision Drivers

* **One implementation of each language decision**, made before any backend.
* **States that cannot occur cannot be written** in what a backend consumes,
  so a backend has no error paths for them.
* **Target neutral:** nothing a backend consumes is specific to one target.
* **The analyzed tree stays faithful to the source**, and able to hold a broken
  program.

## Considered Options

* A separate, target-neutral lowered program
* One tree, with a type for each phase of the compiler
* Side tables of analysis results, keyed by node
* A control-flow graph
* A tree in the bytecode VM's own terms
* Decisions written into the syntax tree, with no lowered program
* A validator in front of code generation

## Decision Outcome

Chosen option: "A separate, target-neutral lowered program", because it is the
only option that removes both the error paths and the repeated decisions from
every backend.

A seam goes between analysis and code generation. A new stage, **lowering**,
turns a program analysis accepted into a **lowered program**: a target-neutral
intermediate representation in which every name is resolved and every decision
the language makes has been made. It is a structured tree rather than a
control-flow graph. Every backend consumes the lowered program and nothing
else; no backend reads the syntax tree.

### Consequences

* Good, because each language decision is made once, before any backend, so two
  backends cannot make it differently.
* Good, because a backend handles only what can occur.
* Good, because a second backend implements the lowered program, not the
  language's decisions.
* Good, because the analyzed tree stays as it is, for rendering source and for
  the language server.
* Bad, because there is a second representation of the program, and a stage to
  maintain.
* Bad, because a source language with jumps must be structured by lowering
  before it fits a tree.
* Bad, because a backend cannot see the whole syntax tree, so an optimization
  can only work on what the lowered program holds.
* Bad, because some information, such as the configuration, is repeated in the
  lowered program, since by design no backend may read the syntax tree.

## Pros and Cons of the Options

### A separate, target-neutral lowered program (chosen)

* Good, because a backend has nothing to resolve and nothing to decide.
* Good, because the lowered program can be written out as text, so tests of
  lowering can compare against readable output.
* Bad, because it is the largest change of the options.

### One tree, with a type for each phase of the compiler

The syntax tree is generic over the phase, so a checked tree has no optional
types and no unresolved names, as GHC ("Trees That Grow") and Scala 3 do.

* Good, because it removes the leftovers of earlier phases.
* Bad, because the tree still refers to things by name, so none of the name,
  call or typing work leaves code generation.
* Bad, because it changes every type of the syntax tree and every pass over
  it.

### Side tables of analysis results, keyed by node

The tree stays syntactic, and analysis results live in maps beside it, as in
rustc's type-check results and RuSTy's annotation map.

* Good, because the tree needs no change.
* Bad, because a lookup in a map can fail, which is what this decision sets out
  to remove. rustc itself builds a typed tree from its tables before it
  generates code.

### A control-flow graph

Typed instructions over basic blocks, as in rustc's MIR.

* Good, because LLVM takes a graph, and jumps need no structuring.
* Bad, because the bytecode VM and WebAssembly have structured control flow, so
  a graph would have to be structured again for them. Building a graph from a
  tree is the easy direction.

### A tree in the bytecode VM's own terms

Variables as VM slots, calls as VM function ids.

* Good, because it is simpler for one backend.
* Bad, because a second backend would make every decision again.

### Decisions written into the syntax tree, with no lowered program

Analysis already records implicit conversions this way.

* Good, because the language server can show each decision.
* Bad, because the tree still refers to things by name, still holds broken
  programs and still has forms that exist only in the source, so every backend
  keeps the name, call and desugaring work, and its error paths.

### A validator in front of code generation

One pass that checks what code generation needs, with accessors that assume
it.

* Good, because the checks are in one place.
* Bad, because every state stays representable, so a backend still handles
  states that cannot occur.
