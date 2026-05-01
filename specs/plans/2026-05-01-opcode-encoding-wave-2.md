# Plan: Opcode Encoding Wave 2 — TRUNC + non-deref array ops

## Context

[ADR-0033](../adrs/0033-opcode-encoding-by-class-and-type.md) defines the
structured `[op_class:6][type:2]` opcode encoding. Wave 1 (#1002) established
the encoding scheme and renumbered the 17 load/store opcodes onto it; the
remaining ~80 opcodes still hold flat hex values that don't match the
encoding rules.

This wave moves a small set of opcodes whose final encoded positions are in
currently-free flat ranges. The choice is dictated by collisions: every
remaining opcode wave has to put new bytes into ranges that other opcodes
still occupy, so each wave has to either move colliders out first or be
sequenced behind another wave that does.

## Scope

Renumber **6 opcodes** to their final encoded positions:

| Opcode      | Old byte | New byte (encoded)                                | Op class       |
| ----------- | -------- | ------------------------------------------------- | -------------- |
| TRUNC_I8    | 0x20     | 0x1C (`encode_opcode(OP_CLASS_TRUNC, 0)`)         | 0x07           |
| TRUNC_U8    | 0x21     | 0x1D (`encode_opcode(OP_CLASS_TRUNC, 1)`)         | 0x07           |
| TRUNC_I16   | 0x22     | 0x1E (`encode_opcode(OP_CLASS_TRUNC, 2)`)         | 0x07           |
| TRUNC_U16   | 0x23     | 0x1F (`encode_opcode(OP_CLASS_TRUNC, 3)`)         | 0x07           |
| LOAD_ARRAY  | 0x24     | 0xA8 (`encode_opcode(OP_CLASS_LOAD_ARRAY, 0)`)    | 0x2A           |
| STORE_ARRAY | 0x25     | 0xAC (`encode_opcode(OP_CLASS_STORE_ARRAY, 0)`)   | 0x2B           |

This frees the flat range `0x20–0x25` for a future arithmetic wave (ADD/SUB
× I32/I64).

## What this wave does NOT do

- **`LOAD_ARRAY_DEREF` / `STORE_ARRAY_DEREF`** (currently 0x26 / 0x27). Their
  final encoded positions are 0xB0 and 0xB4, which currently hold `JMP` and
  `RET`. Renumbering them requires the control-flow ops to move first.
  Deferred to a wave that also moves control-flow.
- **Float arithmetic.** `ADD_F32 / ADD_F64` etc. would land at 0x22/0x23 and
  0x26/0x27 — the second pair collides with the deferred DEREF ops above.
  Integer ADD/SUB land cleanly in the freed `0x20–0x25` range and can ship
  in Wave 3.
- **`FORMAT_VERSION` bump.** Stays at 1 until the final wave once all
  opcodes are renumbered.

## Steps

1. **`compiler/container/src/opcode.rs`** — Convert the 6 constants from
   flat hex to `encode_opcode(OP_CLASS_*, type_tag)` calls. The TRUNC type
   tags follow the existing comment in `OP_CLASS_TRUNC` ("0=I8, 1=U8,
   2=I16, 3=U16"); the array ops use `0` since they don't have width
   variants.
2. **Hex-byte test updates.** All call-site updates land via test-byte
   assertions, not API changes. Locate every literal hex byte that asserts
   one of the six opcodes and replace with the new value, preserving the
   human-readable comment.
3. **`compiler/project/src/disassemble.rs`** — If the disassembler
   hardcodes flat bytes anywhere, update them. (Wave 1 already centralized
   most decoding through `opcode::*` constants, but verify.)
4. **`compiler/vm/src/vm.rs` and any per-family dispatcher modules** —
   Same: confirm dispatch uses the named constants and not flat bytes.
5. **Run the full CI pipeline** (`cd compiler && just`).

## Tests still assert specific hex bytes

Per the convention Wave 1 established, tests continue to assert specific
hex bytes (not `opcode::ADD_I32` etc.) so that future renumbering shows up
as a deliberate test failure rather than a silent behavior change.
Comments on each byte name the opcode for human readers.

## Verification

`cd compiler && just` — compile, coverage (≥85%), clippy, fmt, dupes all
green before opening the PR.
