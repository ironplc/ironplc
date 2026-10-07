# One Intrinsic Variant per Operation and Operand Type

status: proposed
date: 2026-10-07

## Context and Problem Statement

The IR names each operation the compiler implements itself, the standard
functions of IEC 61131-3 and the extensions
[ADR-0042](0042-library-functions-over-compiler-intrinsics.md) admits, with an
enum, `Intrinsic`. It names the standard function blocks with another,
`StandardBlock`.

An enum with one variant per operation, such as `Sqrt`, leaves the operand
type to the call. Then `Sqrt` at an integer type can be written, a constructor
has to refuse it, and a backend's exhaustive match over the enum does not show
that the backend handles every type of every operation.

Today codegen sends `CTU_INT`, `CTU_DINT`, `CTU_UDINT`, `CTU_LINT` and
`CTU_ULINT` to one VM intrinsic, which reads and writes `PV` and `CV` as signed
32-bit values. Nothing showed that `CTU_LINT` inherited that width.

How fine are the variants?

## Decision Drivers

* **A combination the language does not allow cannot be written.**
* **A backend's exhaustive match proves** it handles every combination.
* **Consistent with the VM**, whose opcodes and `func_id`s are already typed
  ([ADR-0034](0034-string-distinction-via-operand-typing.md),
  [ADR-0008](0008-unified-builtin-opcode.md)).

## Considered Options

* One variant per operation and operand type
* One variant per operation, with the type taken from the call

## Decision Outcome

Chosen option: "One variant per operation and operand type", because it makes
a combination the language does not allow impossible to write, and lets the
compiler check that a backend handles every combination.

* **Each `Intrinsic` variant** names one operation at one set of operand types:
  `SqrtF32` and `SqrtF64`, `ConcatString` and `ConcatWString`. A call's
  arguments have exactly the types its variant names.
* **Each `StandardBlock` variant** names one function block at one set of field
  types: `CtuInt` and `CtuLint` are different variants.
* **The analyzer resolves a generic function** such as `ADD` on `ANY_NUM` to
  one type before lowering sees it, and lowering maps the analyzer's
  `BuiltinFunction` and the recorded operand types to one variant
  ([ADR-0059](0059-backends-depend-only-on-the-ir-crate.md)).
* **A behaviour policy**
  ([ADR-0049](0049-behavior-policies-selected-at-compile-time.md)) selects
  among variants during lowering, so a backend receives no policy options.
* **The bytecode backend maps `Intrinsic` to a `func_id`** with a match that
  has no wildcard arm.

### Consequences

* Good, because the square root of an integer cannot be written.
* Good, because adding a variant fails to compile in every backend until the
  backend handles it.
* Good, because each counter width has to be implemented on purpose.
* Bad, because the enums are larger, and lowering's mapping grows with each
  operand type an operation gains.

### Confirmation

The requirements of `specs/design/lowered-program.md` confirm this decision:

* Each `Intrinsic` and `StandardBlock` variant names one operation at one set
  of operand types (REQ-LOW-ir-145, REQ-LOW-ir-146).
* The bytecode backend's mapping to a `func_id` has no wildcard arm
  (REQ-LOW-codegen-074), and neither has lowering's mapping to `Intrinsic`
  (REQ-LOW-lowering-147).

## Pros and Cons of the Options

### One variant per operation and operand type (chosen)

* Good, because the type system rules out invalid combinations.
* Bad, because the enum has many variants.

### One variant per operation, with the type taken from the call

* Good, because the enum is small, roughly the standard's list of functions.
* Bad, because invalid combinations can be written and must be refused at run
  time by a constructor.
* Bad, because an exhaustive match no longer shows which types a backend
  handles.

## More Information

The design is `specs/design/lowered-program.md`, section "Callees, arguments
and intrinsics".
