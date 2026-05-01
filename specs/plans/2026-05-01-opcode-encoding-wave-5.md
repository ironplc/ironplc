# Plan: Opcode Encoding Wave 5 — EQ, NE, GT_S

## Context

Continues the structured `[op_class:6][type:2]` migration (ADR-0033).
Waves 1–4 covered load/store, TRUNC + non-deref array, ADD/SUB/MUL ×
4 types (minus SUB_F32/F64), and DIV/MOD/NEG. The arithmetic family
is now complete except `SUB_F32/F64`, which stay blocked by
`LOAD_ARRAY_DEREF / STORE_ARRAY_DEREF`.

This wave moves into the comparison family. The comparison family has
32 opcodes total but most are entangled with future-wave moves:
`LT_S` and `LE_S` collide with the deferred `SUB_F32/F64`;
`GE_S` collides with `BOOL_AND/OR/XOR/NOT`; `LT_U` and `GT_U` collide
with the bitwise ops. The three classes with no current collisions
are `EQ`, `NE`, and `GT_S` — this wave moves all 12 of those.

## Scope

Renumber **12 opcodes** to their final encoded positions:

| Opcode  | Old byte | New byte (encoded)                          |
| ------- | -------- | ------------------------------------------- |
| EQ_I32  | 0x68     | 0x40 (`encode_opcode(OP_CLASS_EQ, T_I32)`)  |
| EQ_I64  | 0x70     | 0x41 (`encode_opcode(OP_CLASS_EQ, T_I64)`)  |
| EQ_F32  | 0x80     | 0x42 (`encode_opcode(OP_CLASS_EQ, T_F32)`)  |
| EQ_F64  | 0x88     | 0x43 (`encode_opcode(OP_CLASS_EQ, T_F64)`)  |
| NE_I32  | 0x69     | 0x44 (`encode_opcode(OP_CLASS_NE, T_I32)`)  |
| NE_I64  | 0x71     | 0x45 (`encode_opcode(OP_CLASS_NE, T_I64)`)  |
| NE_F32  | 0x81     | 0x46 (`encode_opcode(OP_CLASS_NE, T_F32)`)  |
| NE_F64  | 0x89     | 0x47 (`encode_opcode(OP_CLASS_NE, T_F64)`)  |
| GT_I32  | 0x6C     | 0x50 (`encode_opcode(OP_CLASS_GT_S, T_I32)`)|
| GT_I64  | 0x74     | 0x51 (`encode_opcode(OP_CLASS_GT_S, T_I64)`)|
| GT_F32  | 0x84     | 0x52 (`encode_opcode(OP_CLASS_GT_S, T_F32)`)|
| GT_F64  | 0x8C     | 0x53 (`encode_opcode(OP_CLASS_GT_S, T_F64)`)|

All 12 target slots are currently free (vacated by Waves 3/4 or never
assigned). No within-wave collisions.

## What this wave does NOT do

- **`LT_S` / `LE_S`.** Their target ranges include `0x49` and `0x4F`,
  which still hold the deferred `SUB_F32 / SUB_F64`. Wait for the
  control-flow wave that unblocks SUB_F32/F64.
- **`GE_S`.** Targets `0x54-0x57`, currently held by
  `BOOL_AND/OR/XOR/NOT` (still flat). Wait for a BOOL_OP wave.
- **`LT_U` / `GT_U`.** Targets collide with bitwise ops at
  `0x58-0x5B` and `0x60-0x63`. Wait for a bitwise wave.
- **`LE_U` / `GE_U`.** Their target ranges (`0x5C-0x5D` and
  `0x64-0x65`) are free, but skipping `LT_U`/`GT_U` would leave the
  unsigned comparison family disjoint and confusing. Defer until
  `LT_U`/`GT_U` can move together.
- **`FORMAT_VERSION` bump.** Stays at 1 until the final wave.

## Steps

1. **`compiler/container/src/opcode.rs`** — Convert the 12 constants
   from flat hex to `encode_opcode(OP_CLASS_*, T_*)`. Type tags follow
   the canonical mapping (`T_I32=0`, `T_I64=1`, `T_F32=2`, `T_F64=3`).
2. **Hex-byte test sweep.** Update every literal byte that asserts one
   of these 12 opcodes — both comment-anchored (`0x68, // EQ_I32`) and
   bare-byte / helper-arg patterns
   (`assert_two_arg_bytecode(_, 0x68)`, `&[0x6C]`, `b == 0x6C`).
3. **Spec doc sync.** Update `specs/design/bytecode-instruction-set.md`
   I32 entries for these opcodes (the I64/F32/F64 rows there are
   pre-existing inconsistencies — leave them for a doc cleanup).
4. **Regenerate `compiler/vm-cli/resources/test/steel_thread.iplc`**
   if needed. The `steel_thread.st` source uses no comparisons, so the
   golden file shouldn't change; verify.
5. **Run the full CI pipeline** (`cd compiler && just`).

## Lessons applied from prior waves

- Bare-byte sweep in addition to comment-anchored.
- Helper-arg patterns: check `assert_two_arg_bytecode` and similar.
- vm.rs unit tests embed inline bytecode — sweep src/, not just
  tests/.
- `0xC4, 0x68, …` BUILTIN function-id operands are false positives
  (the second byte is part of the u16 fn-id, not an opcode).

## Verification

`cd compiler && just` — compile, coverage (≥85%), clippy, fmt, dupes
all green before opening the PR.
