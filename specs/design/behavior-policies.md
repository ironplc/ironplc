# Behavior Policies

## Overview

A *behavior policy* is a named choice among enumerated, documented,
deterministic alternatives for one semantic of a standard operation that
IEC 61131-3 leaves to the implementer. [ADR-0049](../adrs/0049-behavior-policies-selected-at-compile-time.md)
decides that a policy is selected at compile time and encoded in the
bytecode: the compiler picks an alternative from its options and emits the
builtin func_id that carries it, and the VM dispatches on what it decodes.
The VM holds no policy state and exposes no policy setting.

This document is the design for that machinery and for its first application,
the two policies of `STRING_TO_<numeric>`. It builds on:

- **[ADR-0049](../adrs/0049-behavior-policies-selected-at-compile-time.md)**: the decision and its rules
- **[Bytecode Instruction Set](bytecode-instruction-set.md)**: the `BUILTIN` opcode and func_id ranges
- **[ADR-0038](../adrs/0038-no-restrictions-on-flag-combinations.md)**: policies compose freely; a preset names a real combination


## The policy machinery

### Alternatives are encoding offsets

Every policy is an enum in `ironplc_container::policy` implementing
`BehaviorPolicy`. The enums live in the container crate rather than with the
compiler options because an alternative is an encoding commitment: its
discriminant is the offset of its func_id within the block for the operation
it governs. The parser (options), codegen (selection) and VM (dispatch) share
that one definition.

**REQ-BP-container-001** Each alternative of a behavior policy has an encoding offset equal to its position in the policy's `ALL` list, and the first alternative (offset 0) is the policy's default.

The first-is-default rule lets a preset table name the default without a
second source of truth, and matches ADR-0049 rule 4: the standard's result,
or a trap where the standard says "error", is always offset 0.

### Encoding: a func_id block per operation

An operation with policies owns a block of builtin func_ids. Within the
block, the ID is arithmetic over the target type and the policies, so the VM
decodes the alternatives from the ID rather than matching a table:

```text
func_id = BASE + target * TARGET_STRIDE + non_numeric * 2 + failure
```

Each assigned ID is also a named row in the instruction set's builtin table,
so the disassembler names the alternative and the wire-format tests pin it.
An ID inside the block that names no conversion (a spare slot, an unassigned
target) is an unknown builtin like any other and traps `V9007`. That is how
ADR-0049 rule 6 works: a VM build that omits an alternative omits its rows,
and nothing else changes.

## First application: `STRING_TO_<numeric>`

### The block

| Requirement | Value | Meaning |
|---|---|---|
| **REQ-BP-container-002** | `0x0480 + target * 8 + non_numeric * 2 + failure` | The func_id of a `STRING_TO_<numeric>` conversion; the six `CONV_STR_TO_U32_*` rows pin target 0 |
| **REQ-BP-container-003** | `builtin::str_to_num::decode` | Inverts the arithmetic for every encoded ID and returns `None` for every other ID in `0x0480..=0x04FF` |

The block reserves `0x0480..=0x04FF`: sixteen targets, each with a stride of
eight (three non-numeric alternatives times two failure alternatives, and two
spare for a further failure alternative). Only target 0, U32 (`UDINT`), is
assigned. The signed, sub-32-bit, 64-bit and real targets keep the
single-encoding `CONV_STR_TO_I32` / `CONV_STR_TO_F32` builtins until they are
moved onto the block.
