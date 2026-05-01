# Plan: Opcode Encoding Wave 4 — NEG, DIV, MOD

## Context

Continues the structured `[op_class:6][type:2]` migration started in
Wave 1 (#1002, ADR-0033) and continued in Waves 2 (#1009, TRUNC + array
load/store) and 3 (#1012, ADD/MUL × 4 types + SUB × {I32, I64}). After
Wave 3, the remaining arithmetic-family opcodes still on flat hex
values are: `DIV_S/DIV_U/MOD_S/MOD_U` and the four `NEG` variants. This
wave moves all 14 of them onto the structured encoding and completes
the integer/float arithmetic family — except `SUB_F32/F64`, which stay
deferred until a control-flow wave clears `LOAD_ARRAY_DEREF` /
`STORE_ARRAY_DEREF` out of `0x26/0x27`.

## Scope

Renumber **14 opcodes** to their final encoded positions:

| Opcode  | Old byte | New byte (encoded)                            |
| ------- | -------- | --------------------------------------------- |
| NEG_I32 | 0x35     | 0x2C (`encode_opcode(OP_CLASS_NEG, T_I32)`)   |
| NEG_I64 | 0x3D     | 0x2D (`encode_opcode(OP_CLASS_NEG, T_I64)`)   |
| NEG_F32 | 0x4C     | 0x2E (`encode_opcode(OP_CLASS_NEG, T_F32)`)   |
| NEG_F64 | 0x52     | 0x2F (`encode_opcode(OP_CLASS_NEG, T_F64)`)   |
| DIV_I32 | 0x33     | 0x30 (`encode_opcode(OP_CLASS_DIV_S, T_I32)`) |
| DIV_I64 | 0x3B     | 0x31 (`encode_opcode(OP_CLASS_DIV_S, T_I64)`) |
| DIV_F32 | 0x4B     | 0x32 (`encode_opcode(OP_CLASS_DIV_S, T_F32)`) |
| DIV_F64 | 0x51     | 0x33 (`encode_opcode(OP_CLASS_DIV_S, T_F64)`) |
| DIV_U32 | 0x40     | 0x34 (`encode_opcode(OP_CLASS_DIV_U, T_I32)`) |
| DIV_U64 | 0x42     | 0x35 (`encode_opcode(OP_CLASS_DIV_U, T_I64)`) |
| MOD_I32 | 0x34     | 0x38 (`encode_opcode(OP_CLASS_MOD_S, T_I32)`) |
| MOD_I64 | 0x3C     | 0x39 (`encode_opcode(OP_CLASS_MOD_S, T_I64)`) |
| MOD_U32 | 0x41     | 0x3C (`encode_opcode(OP_CLASS_MOD_U, T_I32)`) |
| MOD_U64 | 0x43     | 0x3D (`encode_opcode(OP_CLASS_MOD_U, T_I64)`) |

## Collisions and within-wave self-replacement

Several new bytes overlap with old bytes of opcodes that are also moving
in this wave. The four edges:

- `DIV_F64`'s new `0x33` was `DIV_I32`'s old byte (also moving).
- `DIV_U32`'s new `0x34` was `MOD_I32`'s old byte (also moving).
- `DIV_U64`'s new `0x35` was `NEG_I32`'s old byte (also moving).
- `MOD_U32`'s new `0x3C` was `MOD_I64`'s old byte (also moving).
- `MOD_U64`'s new `0x3D` was `NEG_I64`'s old byte (also moving).

These are simultaneous compile-time changes: every `pub const X: Opcode`
in `opcode.rs` updates in one commit, so no inter-wave ordering is
required. The collisions are *within* the wave but resolve atomically.

## What this wave does NOT do

- **`SUB_F32` / `SUB_F64`.** Their target slots `0x26 / 0x27` collide
  with `LOAD_ARRAY_DEREF` / `STORE_ARRAY_DEREF`, which are themselves
  blocked by the control-flow ops. Same blocker as Wave 3.
- **Comparisons, bitwise, BOOL_OP, control-flow, FB ops, array-deref,
  string ops, BUILTIN, stack ops.** Independent op-classes; future
  waves.
- **`FORMAT_VERSION` bump.** Stays at 1 until the final wave.

## Steps

1. **`compiler/container/src/opcode.rs`** — Convert the 14 constants
   from flat hex to `encode_opcode(OP_CLASS_*, type_tag)`. Type tags
   follow the canonical mapping (`T_I32=0`, `T_I64=1`, `T_F32=2`,
   `T_F64=3`); for the unsigned ops (`DIV_U` / `MOD_U`) the type tag is
   the width — `T_I32` for U32, `T_I64` for U64.
2. **Hex-byte test updates.** Sweep every literal hex byte that asserts
   one of these 14 opcodes — both commented (`0x35, // NEG_I32`) and
   bare (`&[0x33]`, `b == 0x40`, helper-arg patterns like Wave 3's
   `assert_two_arg_bytecode(_, 0x33)`). Preserve the comment naming the
   opcode so tests still serve as renumbering guards.
3. **Spec doc sync.** Update `specs/design/bytecode-instruction-set.md`
   for the I32 entries (the I64/F64 rows there are pre-existing
   off-by-N inconsistencies — leave them for a future doc cleanup).
4. **Regenerate `compiler/vm-cli/resources/test/steel_thread.iplc`** if
   any of the 14 renumbered opcodes appear in its bytecode. (The
   `steel_thread.st` source is `x := 10; y := x + 32;` over `INT` —
   uses `LOAD_CONST_I32`, `STORE_VAR_I32`, `ADD_I32`, `TRUNC_I16`,
   `RET_VOID`. None of Wave 4's opcodes appear, so the golden file
   should not need regeneration; verify regardless.)
5. **Run the full CI pipeline** (`cd compiler && just`).

## Lessons applied from Wave 3

- Bare-byte sweep: in addition to grepping `0x33, // DIV_I32`, grep for
  `0x33\b` in test files and check each match for opcode-position
  context (vm.rs unit tests had bare bytes in
  `single_function_container(&[0x30], …)`-style helpers).
- Helper args: tests like `assert_two_arg_bytecode(_, 0x33)` pass the
  opcode byte through a helper, so the literal isn't comment-anchored.
  Search `compile_func_forms.rs` and similar for any helper that takes
  an opcode byte arg.

## Verification

`cd compiler && just` — compile, coverage (≥85%), clippy, fmt, dupes
all green before opening the PR.
