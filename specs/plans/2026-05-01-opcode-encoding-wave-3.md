# Plan: Opcode Encoding Wave 3 — ADD, MUL, partial SUB

## Context

[ADR-0033](../adrs/0033-opcode-encoding-by-class-and-type.md) defines the
`[op_class:6][type:2]` opcode encoding. Wave 1 (#1002) renumbered the 17
load/store opcodes; Wave 2 (#1009) renumbered TRUNC + non-deref array ops,
freeing the flat range `0x20–0x25` for arithmetic.

This wave picks up the arithmetic family. The natural target is
`ADD/SUB/MUL × {I32, I64, F32, F64}` (12 opcodes), but two slots
(`SUB_F32`, `SUB_F64`) collide with the still-flat `LOAD_ARRAY_DEREF`
(0x26) and `STORE_ARRAY_DEREF` (0x27). Those array-deref ops can only
move once the control-flow ops (`JMP`/`RET`/etc.) clear out of `0xB0–0xB5`
— a separate wave. So Wave 3 covers the 10 collision-free targets:

## Scope

Renumber **10 opcodes** to their final encoded positions:

| Opcode  | Old byte | New byte (encoded)                         |
| ------- | -------- | ------------------------------------------ |
| ADD_I32 | 0x30     | 0x20 (`encode_opcode(OP_CLASS_ADD, 0)`)    |
| ADD_I64 | 0x38     | 0x21 (`encode_opcode(OP_CLASS_ADD, 1)`)    |
| ADD_F32 | 0x48     | 0x22 (`encode_opcode(OP_CLASS_ADD, 2)`)    |
| ADD_F64 | 0x4E     | 0x23 (`encode_opcode(OP_CLASS_ADD, 3)`)    |
| SUB_I32 | 0x31     | 0x24 (`encode_opcode(OP_CLASS_SUB, 0)`)    |
| SUB_I64 | 0x39     | 0x25 (`encode_opcode(OP_CLASS_SUB, 1)`)    |
| MUL_I32 | 0x32     | 0x28 (`encode_opcode(OP_CLASS_MUL, 0)`)    |
| MUL_I64 | 0x3A     | 0x29 (`encode_opcode(OP_CLASS_MUL, 1)`)    |
| MUL_F32 | 0x4A     | 0x2A (`encode_opcode(OP_CLASS_MUL, 2)`)    |
| MUL_F64 | 0x50     | 0x2B (`encode_opcode(OP_CLASS_MUL, 3)`)    |

Completes `OP_CLASS_ADD` (0x08) and `OP_CLASS_MUL` (0x0A) fully; partially
fills `OP_CLASS_SUB` (0x09).

## What this wave does NOT do

- **`SUB_F32` / `SUB_F64`.** Their target slots `0x26` / `0x27` collide with
  `LOAD_ARRAY_DEREF` / `STORE_ARRAY_DEREF`, which are themselves blocked by
  the control-flow ops at `0xB0`–`0xB5`. Deferred to a control-flow wave.
- **DIV / MOD / NEG.** These op-classes (`OP_CLASS_DIV_S`, `DIV_U`,
  `MOD_S`, `MOD_U`, `NEG`) are independent and target free ranges
  (`0x30–0x37`, `0x38–0x3B`, `0x2C–0x2F`); they fit a future wave.
- **`FORMAT_VERSION` bump.** Stays at 1 until the final wave.

## Steps

1. **`compiler/container/src/opcode.rs`** — Convert the 10 constants from
   flat hex to `encode_opcode(OP_CLASS_*, type_tag)` calls. Type tags
   follow the canonical mapping (`T_I32=0`, `T_I64=1`, `T_F32=2`,
   `T_F64=3`) defined in Wave 1.
2. **Hex-byte test updates.** Locate every literal hex byte that asserts
   one of the ten opcodes and replace with the new value, preserving the
   human-readable comment naming the opcode.
3. **Regenerate `compiler/vm-cli/resources/test/steel_thread.iplc`.** The
   golden file is compiled from `compiler/resources/test/steel_thread.st`
   (which performs `y := x + 32`, embedding `ADD_I32`). Recompile via
   `ironplcc compile` so the embedded `ADD_I32` byte updates from `0x30`
   to `0x20`.
4. **Verify VM dispatch and disassembler use named constants** (Wave 1
   already centralized this; spot-check no flat bytes remain).
5. **Run the full CI pipeline** (`cd compiler && just`).

## Tests still assert specific hex bytes

Per the convention Wave 1 established, test assertions continue to use
specific hex bytes (not `opcode::ADD_I32` etc.) so future renumbering
shows up as deliberate test failures rather than silent drift.

## Verification

`cd compiler && just` — compile, coverage (≥85%), clippy, fmt, dupes all
green before opening the PR.
