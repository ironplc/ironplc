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

The [glossary](../steering/glossary.md#behavior-policy) defines the term and
how it relates to a dialect, an extension and a flag.

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

### Selection: `CompilerOptions`

A policy is a field on `CompilerOptions`, declared in the `policies` section
of `define_compiler_options!` next to the `--allow-*` flags. The declaration
names the policy's enum, the alternative each non-default dialect selects, and
the option key. From that one row the macro generates the field, the preset
mapping in `from_dialect`, and the `POLICY_DESCRIPTORS` table the CLI, LSP,
MCP server and `ironplcc dialects` derive their surface from.

**REQ-BP-parser-004** `CompilerOptions::POLICY_DESCRIPTORS` names every behavior policy with its `--policy-*` CLI flag, its `policy_*` option key, its alternatives in encoding order, and its default, which is the first alternative.

**REQ-BP-parser-003** `CompilerOptions::set_policy_by_key` selects an alternative by option key and CLI name, and rejects an unknown key or an alternative the policy does not have without changing the options.

Unlike a flag, which can only enable, a policy selection *replaces* the
preset's: `--dialect codesys --policy-string-to-num-failure trap` compiles
CODESYS syntax with the strict failure behavior. The three front ends apply
the same rule: the CLI flag takes the alternative's CLI name, the LSP
`initializationOptions` take it as a string under the lowerCamelCase key
(`policyStringToNumFailure`), and the MCP `options` object takes it as a
string under the option key.

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

### The policies

| Policy | Option key | Alternatives (encoding order) | Default |
|---|---|---|---|
| Non-numeric: what counts as convertible when the string has characters that are not part of a literal | `policy_string_to_num_non_numeric` | `reject`, `ignore-trailing`, `ignore-surrounding` | `reject` |
| Failure: what happens when the string is not convertible | `policy_string_to_num_failure` | `trap`, `zero` | `trap` |

**REQ-BP-parser-001** The default `CompilerOptions`, and the `iec61131-3-ed2` and `iec61131-3-ed3` dialects, select `reject` and `trap`.

**REQ-BP-parser-002** The `rusty` dialect selects `reject` and `zero`; the `codesys` and `twincat` dialects select `ignore-trailing` and `zero`.

The presets are the surveyed behaviors in ADR-0049: RuSTy rejects trailing
characters and never faults; CODESYS and TwinCAT (observed) stop at the first
invalid character and document 0 for a string that is not valid in the target
type. No preset selects `ignore-surrounding`; it exists because Rockwell
documents it for `STOD`, and a program ported from Logix can ask for it.

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
