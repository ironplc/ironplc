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
| **REQ-BP-container-002** | `0x0480 + target * 8 + non_numeric * 2 + failure` | The func_id of a `STRING_TO_<numeric>` conversion; each assigned target's six `CONV_STR_TO_<target>_*` rows pin its position |
| **REQ-BP-container-003** | `builtin::str_to_num::decode` | Inverts the arithmetic for every encoded ID and returns `None` for every other ID in `0x0480..=0x04FF` |

The block reserves `0x0480..=0x04FF`: sixteen targets, each with a stride of
eight (three non-numeric alternatives times two failure alternatives, and two
spare for a further failure alternative). The targets sit at fixed positions:
even positions are unsigned and the odd position after each is the signed
type of the same width, in width order 32, 8, 16, 64 (32 first because U32
landed at 0), and the real targets follow. A bit-string type converts as the
unsigned integer of its width and has no target of its own, so
`STRING_TO_BYTE` and `STRING_TO_USINT` compile to the same func_id.

| Position | Target | Functions | func_ids |
|---|---|---|---|
| 0 | U32 | `STRING_TO_UDINT`, `STRING_TO_DWORD` | 0x0480–0x0485 |
| 1 | I32 | `STRING_TO_DINT` | 0x0488–0x048D |
| 2 | U8 | `STRING_TO_USINT`, `STRING_TO_BYTE` | 0x0490–0x0495 |
| 3 | I8 | `STRING_TO_SINT` | 0x0498–0x049D |
| 4 | U16 | `STRING_TO_UINT`, `STRING_TO_WORD` | 0x04A0–0x04A5 |
| 5 | I16 | `STRING_TO_INT` | 0x04A8–0x04AD |
| 6 | U64 | `STRING_TO_ULINT`, `STRING_TO_LWORD` | 0x04B0–0x04B5 |
| 7 | I64 | `STRING_TO_LINT` | 0x04B8–0x04BD |
| 8 | F32 | `STRING_TO_REAL` | 0x04C0–0x04C5 |
| 9 | F64 | `STRING_TO_LREAL` | 0x04C8–0x04CD |

Every `STRING_TO_<numeric>` function is on the block; positions 10 through
15 are unassigned. Codegen no longer emits `CONV_STR_TO_I32` or
`CONV_STR_TO_F32`; the VM keeps their handlers because a func_id is a
permanent wire-format commitment.

### Codegen

**REQ-BP-codegen-001** Every `STRING_TO_<numeric>` function compiles to `BUILTIN` with the func_id that `builtin::str_to_num::func_id(target, non_numeric, failure)` gives for its target and the selected policies, so the same source under two selections produces bytecode that differs only in that operand.

**REQ-BP-codegen-002** The conversion emits no `TRUNC_*` after its builtin: the VM range-checks against the target's own bounds and pushes the value as-is, so a sub-32-bit target never wraps.

### The literal grammar

Under every non-numeric alternative, what is converted is an IEC 61131-3
literal of the target type: CODESYS documents the input as "a valid literal
of the target type", and every surveyed implementation accepts `16#FF`, so
the grammar is the meaning of "convertible" rather than a policy.

**REQ-BP-vm-001** A literal is an optional sign, then either a run of decimal digits or a based literal (`2#`, `8#` or `16#` followed by digits of that base), where single `_` separators may appear between digits. A typed prefix (`UDINT#`) is not part of the grammar.

For an unsigned target a `-` sign is accepted by the grammar and is a range
failure for any value but zero, so `-5` fails on range rather than on
syntax. This keeps `ignore-surrounding` from skipping a sign that Rockwell's
`STOD` documents as part of the number.

**REQ-BP-vm-008** A real literal is an optional sign, then a mantissa that is a run of decimal digits, a run with a decimal point and an optional fraction run, or a decimal point with a fraction run, then an optional exponent of `E` or `e`, an optional sign and a run of decimal digits, where single `_` separators may appear between the digits of a run; a based literal, a typed prefix, and the words `inf`, `infinity` and `nan` are not part of the grammar.

The real grammar is CODESYS's "floating-point number, also in exponential
notation". The integer form (`'5'`) is a real literal because every
surveyed implementation converts it; the point-first and point-last forms
(`'.5'`, `'1.'`) because the C `strtod` the surveyed runtimes build on
accepts them, and because rejecting `'.5'` while `ignore-surrounding` skips
the point to convert 5 would be a worse result than 0.5. A decimal point is
not part of an integer literal: CODESYS documents that for an integer target
"floating-point numbers are not convertible; the decimal separator is
treated and truncated as a following character", which is what `reject`
(failure) and `ignore-trailing` (the integer part) already do.

### Scan semantics

**REQ-BP-vm-002** Under `reject`, the string less surrounding ASCII whitespace must be exactly one literal; an empty string, or anything before or after the literal, is a failure.

**REQ-BP-vm-003** Under `ignore-trailing`, the longest literal at the start of the string, after leading ASCII whitespace, is converted and everything after it is ignored; a string with no leading literal is a failure.

**REQ-BP-vm-004** Under `ignore-surrounding`, characters that cannot start a literal (anything but a digit, or a sign immediately followed by a digit) are skipped, then the string is converted as under `ignore-trailing`; a string with no literal anywhere is a failure.

**REQ-BP-vm-005** A literal whose value does not fit the target type is a failure under every non-numeric alternative; no alternative wraps or saturates.

**REQ-BP-vm-009** A real literal whose magnitude rounds to infinity at the target's width is a failure under every non-numeric alternative, and a literal too small for the width rounds to a subnormal or to zero and is not a failure.

**REQ-BP-vm-010** No conversion produces a NaN or an infinity: a word that would spell one is not a literal and so is a failure under every non-numeric alternative, `ignore-surrounding` skips it to a literal behind it, and under `zero` a failure produces positive 0.0.

Range is a failure, not a scan question (ADR-0049): `'4294967296'` is a
well-formed literal that is not convertible to `UDINT`, so under
`ignore-trailing` it fails rather than converting a shorter run of its digits.
The same holds at every width: `'300'` is not convertible to `SINT`, and the
conversion fails rather than truncating to 44.

One scanner serves every integer target. It accumulates the literal's
magnitude in 64 bits and checks it against the target's largest magnitude on
each side of zero, which holds `ULINT`'s `2^64 - 1` and `LINT`'s `2^63` below
zero without wider arithmetic; the target contributes nothing but those
bounds and its slot width, which is what keeps the eleven functions from
having eleven scanners.

The real targets have a second scanner behind the same policies and the
same failure handling. It measures the literal by the real grammar and
parses it with `core`'s `FromStr` at the target's width (`f32` for `REAL`,
`f64` for `LREAL`), so the value is the correctly rounded value of the text
at that width rather than a wide parse narrowed; a magnitude that rounds to
infinity there is the real targets' out-of-range. Overflow is a failure
because an infinity is a value the text did not name, exactly as `'300'`
names a value `SINT` does not hold; underflow is not, because zero or a
subnormal is the value the text names at that width.

### Failure

**REQ-BP-vm-006** Under `trap`, a failure halts execution with `V4006 StringNotConvertible`, whose message names the target type and the offending string; under `zero`, the conversion produces 0 and execution continues.

**REQ-BP-vm-007** A `WSTRING` operand traps `V9014 EncodingMismatch` regardless of policy, and a func_id in the block that names no conversion traps `V9007 InvalidBuiltinFunction`.

The trap carries a bounded preview of the offending string (its first
sixteen bytes, with an ellipsis if more followed), because the VM is `no_std`
and a trap cannot own the value; sixteen bytes is enough to show any
`UDINT` literal, the first sixteen characters of a 64-bit one, and to
recognise a mistyped one.

The trap names the target the func_id encodes. A bit-string function shares
the unsigned target of its width, so `STRING_TO_BYTE('300')` reports that
`'300'` is not convertible to `USINT`: the encoding does not say which of the
two functions was called, and the range it names is the same.

## Adding a policy

1. Add the enum to `ironplc_container::policy` implementing `BehaviorPolicy`,
   default first.
2. Reserve the operation's func_id block in `ironplc_container::builtin` and
   declare the assigned rows in `declare_builtins!`; pin them in the
   wire-format tests and record them in
   [bytecode-instruction-set.md](bytecode-instruction-set.md).
3. Add the row to the `policies` section of `define_compiler_options!` with
   the alternative each vendor preset selects. The CLI, LSP, MCP and
   `dialects` surfaces follow from `POLICY_DESCRIPTORS`; the CLI needs one
   `clap_policy!` newtype and one `Option` field, which its drift test
   enforces.
4. Thread the field from `CodegenOptions` to the emitter and select the
   func_id from it.
5. Dispatch in the VM on the decoded ID, in a module of its own.
6. Document the policy on the page for the operation it governs, with the
   result for every alternative and any divergence from a vendor whose
   behavior is undefined; add the trap's `V4xxx` page.

## Adding a target

1. Add the variant to `builtin::str_to_num::Target` at the position the
   table above reserves, and to `Target::ALL`; declare its six rows in
   `declare_builtins!`, pin them in the wire-format test, and record them in
   [bytecode-instruction-set.md](bytecode-instruction-set.md).
2. Give the VM its scan and slot width (`str_to_num::scan`) and its type
   name in `error.rs`.
3. Map the type to the target in codegen's `str_to_num_target`, and register
   the function in the analyzer if it is not.
4. Test end to end at min, max, one past each bound and the shared invalid
   inputs under every policy combination; extend the benchmark group.
5. Document the function's range on the type-conversions page.
