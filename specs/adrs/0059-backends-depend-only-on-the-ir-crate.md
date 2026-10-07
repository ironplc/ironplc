# Backends Depend Only on the IR Crate

status: proposed
date: 2026-10-07

## Context and Problem Statement

A backend that can reach the analyzer or the AST will, at some convenient call
site, and then decide something again that lowering already decided
([ADR-0057](0057-backends-consume-a-target-neutral-lowered-program.md)). A rule
in a review cannot stop that for long. A crate boundary can: Rust lets a crate
name only its direct dependencies.

Built-in functions need a name on both sides of that boundary:

* The analyzer resolves each call of a built-in function to one function and
  records it on the `FunctionSignature`, with an enum it calls `Intrinsic`
  today. It knows which function as soon as it resolves the call.
* The IR needs an enum with one variant per operation and operand type
  ([ADR-0062](0062-one-intrinsic-variant-per-operation-and-operand-type.md)).
  The operand types are known only once the analyzer has recorded them.

If the analyzer named the IR's enum on its signatures, the analyzer would
depend on the IR.

How are the crates arranged?

## Decision Drivers

* **A backend cannot name the analyzer or the AST**, and the compiler enforces
  it.
* **A backend does not compile the analyzer.**
* **The analyzer does not depend on the IR**, so its dispatch does not change
  when an operation gains an operand type.
* **No stage after the analyzer matches a function's name.**

## Considered Options

* A separate IR crate; a backend depends on it alone; the analyzer and the IR
  each have their own enum of built-in functions
* The analyzer depends on the IR and names the IR's enum on its signatures
* One crate for the IR and lowering
* A backend may depend on the analyzer, kept apart by review

## Decision Outcome

Chosen option: "A separate IR crate; a backend depends on it alone; the
analyzer and the IR each have their own enum of built-in functions", because it
is the only arrangement in which the compiler stops a backend from reaching the
analyzer and the analyzer does not depend on the IR.

| Crate | Holds | Depends on |
|---|---|---|
| `ironplc-ir` | The IR's data model, its checking constructors, the `Intrinsic` and `StandardBlock` enums, and the expansions a backend may call instead of implementing a node | `ironplc-dsl` only, for ids, source spans and diagnostics, which it re-exports |
| `ironplc-analyzer` | As today | As today |
| `ironplc-lowering` | The pass | `ironplc-dsl`, `ironplc-analyzer`, `ironplc-ir` |
| A backend | Its emitter | `ironplc-ir` only |

* **The analyzer's `BuiltinFunction`** says which function a call resolved to:
  `SQRT`, `CONCAT`, `INT_TO_REAL`. Every built-in function signature carries
  one. The analyzer's enum is renamed from `Intrinsic` to `BuiltinFunction` in
  a mechanical prefactor, so that `Intrinsic` names only the IR's enum.
* **The IR's `Intrinsic`** says which operation at which operand types, such as
  `SqrtF32` or `SqrtF64`.
* **Lowering maps one to the other** with a match that has no wildcard arm. A
  combination with no `Intrinsic` variant is P9999.
* **`ironplc-codegen` drops its dependencies** on the analyzer and the DSL when
  the route that reads the AST is deleted.

### Consequences

* Good, because the compiler, not a reviewer, keeps a backend away from the
  analyzer and the AST.
* Good, because a backend does not compile the analyzer at all.
* Good, because a new operand type for an operation changes the IR and
  lowering's mapping, not the analyzer.
* Bad, because there are two enums of built-in functions and one mapping
  between them to keep in step.
* Bad, because the rename is a prefactor of its own.

### Confirmation

The requirements of `specs/design/lowered-program.md` confirm this decision:

* A test over the crate manifest asserts that `ironplc-codegen` has no
  dependency on `ironplc-analyzer`, `ironplc-parser`, `ironplc-lowering` or
  `ironplc-dsl` outside `[dev-dependencies]` (REQ-LOW-codegen-002).
* Every built-in function signature names its `BuiltinFunction`
  (REQ-LOW-analyzer-070).
* Lowering's mapping to `Intrinsic` has no wildcard arm
  (REQ-LOW-lowering-147).

## Pros and Cons of the Options

### A separate IR crate; a backend depends on it alone; two enums (chosen)

* Good, because each crate names only what it needs.
* Bad, because of the second enum and the mapping.

### The analyzer depends on the IR and names the IR's enum on its signatures

* Good, because there is one enum and no mapping.
* Bad, because the analyzer would depend on the IR, and would have to know the
  operand types of a call before it has recorded them.

### One crate for the IR and lowering

* Good, because there is one crate fewer.
* Bad, because a backend that depends on it depends on lowering, and so on the
  analyzer.

### A backend may depend on the analyzer, kept apart by review

* Good, because it needs no new crate.
* Bad, because nothing stops the next convenient call into the analyzer.

## More Information

The design is `specs/design/lowered-program.md`, section "Position in the
Pipeline".
