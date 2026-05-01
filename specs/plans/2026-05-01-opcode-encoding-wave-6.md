# Plan: Opcode Encoding Wave 6 — BOOL, Bitwise, GE_S, U-comparisons

## Context

Continues the structured `[op_class:6][type:2]` migration (ADR-0033).
Waves 1–5 covered load/store, TRUNC + non-deref array, ADD/SUB/MUL,
DIV/MOD/NEG, and EQ/NE/GT_S. The remaining comparisons (`LT_S`,
`LE_S`, `GE_S`, `LT_U`, `LE_U`, `GT_U`, `GE_U`), the boolean ops
(`BOOL_AND/OR/XOR/NOT`), and the bitwise ops form a tightly-coupled
collision graph — moving any subset without the rest creates external
collisions.

This wave moves all of them in one cohesive cascade. After Wave 6,
the only remaining comparison family is `LT_S` and `LE_S`, which stay
blocked by the still-flat `SUB_F32 / SUB_F64`.

## Scope

Renumber **24 opcodes** to their final encoded positions:

### BOOL (op-class 0x1E — type tag selects op)

| Opcode    | Old byte | New byte (encoded)                        |
| --------- | -------- | ----------------------------------------- |
| BOOL_AND  | 0x54     | 0x78 (`encode_opcode(OP_CLASS_BOOL_OP, 0)`) |
| BOOL_OR   | 0x55     | 0x79 (`encode_opcode(OP_CLASS_BOOL_OP, 1)`) |
| BOOL_XOR  | 0x56     | 0x7A (`encode_opcode(OP_CLASS_BOOL_OP, 2)`) |
| BOOL_NOT  | 0x57     | 0x7B (`encode_opcode(OP_CLASS_BOOL_OP, 3)`) |

### Bitwise (type tag 0 = W32, 1 = W64)

| Opcode      | Old byte | New byte (encoded)                              |
| ----------- | -------- | ----------------------------------------------- |
| BIT_AND_32  | 0x58     | 0x68 (`encode_opcode(OP_CLASS_BIT_AND, 0)`)     |
| BIT_AND_64  | 0x60     | 0x69 (`encode_opcode(OP_CLASS_BIT_AND, 1)`)     |
| BIT_OR_32   | 0x59     | 0x6C (`encode_opcode(OP_CLASS_BIT_OR, 0)`)      |
| BIT_OR_64   | 0x61     | 0x6D (`encode_opcode(OP_CLASS_BIT_OR, 1)`)      |
| BIT_XOR_32  | 0x5A     | 0x70 (`encode_opcode(OP_CLASS_BIT_XOR, 0)`)     |
| BIT_XOR_64  | 0x62     | 0x71 (`encode_opcode(OP_CLASS_BIT_XOR, 1)`)     |
| BIT_NOT_32  | 0x5B     | 0x74 (`encode_opcode(OP_CLASS_BIT_NOT, 0)`)     |
| BIT_NOT_64  | 0x63     | 0x75 (`encode_opcode(OP_CLASS_BIT_NOT, 1)`)     |

### GE_S

| Opcode | Old byte | New byte (encoded)                           |
| ------ | -------- | -------------------------------------------- |
| GE_I32 | 0x6D     | 0x54 (`encode_opcode(OP_CLASS_GE_S, T_I32)`) |
| GE_I64 | 0x75     | 0x55 (`encode_opcode(OP_CLASS_GE_S, T_I64)`) |
| GE_F32 | 0x85     | 0x56 (`encode_opcode(OP_CLASS_GE_S, T_F32)`) |
| GE_F64 | 0x8D     | 0x57 (`encode_opcode(OP_CLASS_GE_S, T_F64)`) |

### Unsigned comparisons (type tag = width: 0 = U32, 1 = U64)

| Opcode  | Old byte | New byte (encoded)                            |
| ------- | -------- | --------------------------------------------- |
| LT_U32  | 0x78     | 0x58 (`encode_opcode(OP_CLASS_LT_U, T_I32)`)  |
| LT_U64  | 0x7C     | 0x59 (`encode_opcode(OP_CLASS_LT_U, T_I64)`)  |
| LE_U32  | 0x79     | 0x5C (`encode_opcode(OP_CLASS_LE_U, T_I32)`)  |
| LE_U64  | 0x7D     | 0x5D (`encode_opcode(OP_CLASS_LE_U, T_I64)`)  |
| GT_U32  | 0x7A     | 0x60 (`encode_opcode(OP_CLASS_GT_U, T_I32)`)  |
| GT_U64  | 0x7E     | 0x61 (`encode_opcode(OP_CLASS_GT_U, T_I64)`)  |
| GE_U32  | 0x7B     | 0x64 (`encode_opcode(OP_CLASS_GE_U, T_I32)`)  |
| GE_U64  | 0x7F     | 0x65 (`encode_opcode(OP_CLASS_GE_U, T_I64)`)  |

## Why one wave, not several

The collision graph is a cycle:

- `BOOL` targets `0x78-0x7B` collide with `LT_U32/LE_U32/GT_U32/GE_U32`.
- `BIT_OR_64`'s target `0x6D` collides with `GE_I32`.
- `BIT_NOT_64`'s target `0x75` collides with `GE_I64`.
- `GE_S` targets `0x54-0x57` collide with `BOOL_AND/OR/XOR/NOT`.
- `LT_U` and `GT_U` targets collide with `BIT_AND/OR_*`.

Splitting the wave any way leaves one side blocked. Moving all 24
together resolves all collisions atomically inside a single
`opcode.rs` change.

## What this wave does NOT do

- **`LT_S` / `LE_S`.** Their target ranges include `0x49` and `0x4F`,
  which still hold the deferred `SUB_F32 / SUB_F64`. Wait for the
  control-flow wave that unblocks SUB_F32/F64.
- **Control flow, stack, FB, BUILTIN, DEREF, string ops.** Independent
  op-classes; future waves.
- **`FORMAT_VERSION` bump.** Stays at 1 until the final wave.

## Steps

1. **`compiler/container/src/opcode.rs`** — Convert the 24 constants
   from flat hex to `encode_opcode(OP_CLASS_*, type_tag)`. Type tags:
   - `BOOL_OP`: 0=AND, 1=OR, 2=XOR, 3=NOT (per opcode.rs comment).
   - `BIT_*`: 0=W32, 1=W64.
   - `GE_S`: standard `T_I32`/`T_I64`/`T_F32`/`T_F64`.
   - `LT_U`/`LE_U`/`GT_U`/`GE_U`: 0=U32, 1=U64. The `T_I32`/`T_I64`
     constants happen to match these tag values, so reuse them for
     readability.
2. **Hex-byte test sweep.** Update every literal byte that asserts one
   of these 24 opcodes — comment-anchored, bare-byte, and helper-arg
   patterns. Bytes to find: 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5A,
   0x5B, 0x60, 0x61, 0x62, 0x63, 0x6D, 0x75, 0x78, 0x79, 0x7A, 0x7B,
   0x7C, 0x7D, 0x7E, 0x7F, 0x85, 0x8D.
3. **Spec doc sync.** Update `specs/design/bytecode-instruction-set.md`
   I32 entries for these opcodes (where they exist and are accurate).
4. **Regenerate `compiler/vm-cli/resources/test/steel_thread.iplc`**
   if needed. The `steel_thread.st` source uses no boolean/bitwise/U
   comparisons, so the golden file shouldn't change; verify.
5. **Run the full CI pipeline** (`cd compiler && just`).

## Lessons applied from prior waves

- Bare-byte sweep alongside comment-anchored — Wave 3 missed 3
  vm.rs unit tests; Wave 5 made sure to grep every byte.
- Helper-arg patterns: `assert_two_arg_bytecode`, etc.
- vm.rs unit tests embed inline bytecode — sweep `src/`, not just
  `tests/`.
- BUILTIN function-id operands: `0xC4, 0x68, …` — the second byte is
  part of the u16 fn-id, not an opcode (false positive).
- `false-positive bytes`: file-format magic, value-comparison operands,
  pool indexes — distinguish opcode-position bytes from data bytes.

## Verification

`cd compiler && just` — compile, coverage (≥85%), clippy, fmt, dupes
all green before opening the PR.
