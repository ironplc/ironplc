# Plan: Opcode Encoding Wave 7 — control flow, DEREF, SUB_F, LT_S, LE_S

## Context

Continues the structured `[op_class:6][type:2]` migration (ADR-0033).
Waves 1–6 covered load/store, TRUNC + non-deref array, ADD/MUL × 4 +
SUB × {I32,I64}, DIV/MOD/NEG, EQ/NE/GT_S, and BOOL/Bitwise/GE_S/U-cmps.

This wave resolves the **deferred chain** that has been blocking
several earlier waves:

```
SUB_F32 / SUB_F64  ←  LOAD/STORE_ARRAY_DEREF  ←  JMP / RET / RET_VOID
LT_S, LE_S float variants  ←  SUB_F32 / SUB_F64
```

Moving control-flow opcodes out of `0xB0-0xB5` frees the slots needed
by the array-deref ops; moving the deref ops out of `0x26/0x27` frees
the slots needed by `SUB_F32/F64`; moving those frees `0x49/0x4F`
needed by `LT_I64`/`LE_F64`. All five steps must happen atomically in
one `opcode.rs` change.

The only remaining wave after this is the non-arithmetic tail
(STACK_OP + BUILTIN + FB + string ops + FORMAT_VERSION bump).

## Scope

Renumber **17 opcodes** to their final encoded positions:

### Control flow (5)

| Opcode      | Old byte | New byte (encoded)                              |
| ----------- | -------- | ----------------------------------------------- |
| JMP         | 0xB0     | 0x7C (`encode_opcode(OP_CLASS_JMP, 0)`)         |
| JMP_IF_NOT  | 0xB2     | 0x80 (`encode_opcode(OP_CLASS_JMP_IF_NOT, 0)`)  |
| CALL        | 0xB3     | 0x84 (`encode_opcode(OP_CLASS_CALL, 0)`)        |
| RET         | 0xB4     | 0x88 (`encode_opcode(OP_CLASS_RET, 0)`)         |
| RET_VOID    | 0xB5     | 0x8C (`encode_opcode(OP_CLASS_RET_VOID, 0)`)    |

### Array DEREF (2)

| Opcode             | Old byte | New byte (encoded)                                       |
| ------------------ | -------- | -------------------------------------------------------- |
| LOAD_ARRAY_DEREF   | 0x26     | 0xB0 (`encode_opcode(OP_CLASS_LOAD_ARRAY_DEREF, 0)`)     |
| STORE_ARRAY_DEREF  | 0x27     | 0xB4 (`encode_opcode(OP_CLASS_STORE_ARRAY_DEREF, 0)`)    |

### Float SUB (2)

| Opcode  | Old byte | New byte (encoded)                          |
| ------- | -------- | ------------------------------------------- |
| SUB_F32 | 0x49     | 0x26 (`encode_opcode(OP_CLASS_SUB, T_F32)`) |
| SUB_F64 | 0x4F     | 0x27 (`encode_opcode(OP_CLASS_SUB, T_F64)`) |

### LT_S (4)

| Opcode | Old byte | New byte (encoded)                           |
| ------ | -------- | -------------------------------------------- |
| LT_I32 | 0x6A     | 0x48 (`encode_opcode(OP_CLASS_LT_S, T_I32)`) |
| LT_I64 | 0x72     | 0x49 (`encode_opcode(OP_CLASS_LT_S, T_I64)`) |
| LT_F32 | 0x82     | 0x4A (`encode_opcode(OP_CLASS_LT_S, T_F32)`) |
| LT_F64 | 0x8A     | 0x4B (`encode_opcode(OP_CLASS_LT_S, T_F64)`) |

### LE_S (4)

| Opcode | Old byte | New byte (encoded)                           |
| ------ | -------- | -------------------------------------------- |
| LE_I32 | 0x6B     | 0x4C (`encode_opcode(OP_CLASS_LE_S, T_I32)`) |
| LE_I64 | 0x73     | 0x4D (`encode_opcode(OP_CLASS_LE_S, T_I64)`) |
| LE_F32 | 0x83     | 0x4E (`encode_opcode(OP_CLASS_LE_S, T_F32)`) |
| LE_F64 | 0x8B     | 0x4F (`encode_opcode(OP_CLASS_LE_S, T_F64)`) |

## Within-wave collisions (all resolve atomically)

- `LOAD_ARRAY_DEREF` → `0xB0` was `JMP`'s byte (also moving in wave).
- `STORE_ARRAY_DEREF` → `0xB4` was `RET`'s byte (also moving).
- `SUB_F32` → `0x26` was `LOAD_ARRAY_DEREF`'s byte (also moving).
- `SUB_F64` → `0x27` was `STORE_ARRAY_DEREF`'s byte (also moving).
- `LT_I64` → `0x49` was `SUB_F32`'s byte (also moving).
- `LE_F64` → `0x4F` was `SUB_F64`'s byte (also moving).

All other targets (`0x7C/0x80/0x84/0x88/0x8C/0x48/0x4A-0x4E`) were
freed by Waves 3–6. No external collisions.

## What this wave does NOT do

- **STACK_OP, BUILTIN, FB, string ops.** Future Wave 8 (the final).
- **`FORMAT_VERSION` bump.** Wave 8 — by that point every opcode is
  on the structured encoding and the bytecode container's wire format
  is fully compatible with the published encoding scheme.

## Test churn

The control-flow opcodes are pervasive — `0xB5` (RET_VOID) appears in
~63 test files (every codegen / VM test ends with it). The byte-level
sweep is the bulk of this PR's diff. Lessons from prior waves apply:

- Comment-anchored sweep first (`0xB5, // RET_VOID`, etc.).
- Bare-byte sweep next: `&[0xB5]`, `b == 0xB5`, helper-arg patterns,
  inline bytecode in `vm.rs` unit tests.
- BUILTIN function-id operands like `0xC4, 0x68, 0x03` have second/
  third bytes that *look* like opcode bytes — false positives unless
  preceded by `0xC4`. (BUILTIN itself stays at `0xC4` for this wave.)

## Steps

1. **`compiler/container/src/opcode.rs`** — Convert the 17 constants
   from flat hex to `encode_opcode(OP_CLASS_*, type_tag)`. The control
   flow ops use `type_tag = 0` (single-variant op-classes per
   ADR-0033). DEREFs likewise. SUB/LT_S/LE_S use the standard
   `T_I32`/`T_I64`/`T_F32`/`T_F64` mapping.
2. **Hex-byte test sweep.** Bytes to find: 0x26, 0x27, 0x49, 0x4F,
   0x6A, 0x6B, 0x72, 0x73, 0x82, 0x83, 0x8A, 0x8B, 0xB0, 0xB2, 0xB3,
   0xB4, 0xB5. Update comment-anchored, bare-byte, and helper-arg
   patterns.
3. **Spec doc sync.** Update entries in
   `specs/design/bytecode-instruction-set.md` for the 17 opcodes
   where the doc is currently accurate (skip pre-existing off-by-N
   I64/F64 rows).
4. **Regenerate `compiler/vm-cli/resources/test/steel_thread.iplc`**.
   The `steel_thread.st` source uses `RET_VOID` (every program does),
   so the golden file's last byte will change from `0xB5` to `0xC8`.
   Wait — `0x8C`, not `0xC8`. Let me re-check: `RET_VOID` →
   `encode_opcode(0x23, 0)` = `0x8C`. The golden file MUST be
   regenerated.
5. **Run the full CI pipeline** (`cd compiler && just`).

## Verification

`cd compiler && just` — compile, coverage (≥85%), clippy, fmt, dupes
all green before opening the PR.
