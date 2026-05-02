# Plan: Opcode Encoding Wave 8 — STACK_OP, BUILTIN, FB ops, string ops, FORMAT_VERSION bump

## Context

This is the **final wave** of the structured `[op_class:6][type:2]`
opcode encoding migration (ADR-0033). Waves 1–7 covered load/store,
TRUNC + non-deref array, ADD/MUL × 4 + SUB × {I32,I64}, DIV/MOD/NEG,
EQ/NE/GT_S, BOOL/Bitwise/GE_S/U-cmps, and control-flow + DEREF +
SUB_F32/F64 + LT_S + LE_S.

After this wave, every opcode in `opcode.rs` is derived via
`encode_opcode(OP_CLASS_*, type_tag)`, no flat hex constants remain,
and the wire format matches the published encoding scheme.
**`FORMAT_VERSION` bumps from 1 to 2.**

## Scope

Renumber **23 opcodes** plus the format-version bump.

### Stack family (consolidated under `OP_CLASS_STACK_OP`)

The `STACK_OP` op-class collapses `POP`, `DUP`, and `SWAP` into a
single op-class slot (`0x24`), with the type tag selecting which
operation. Per ADR-0033, this is one of the in-class consolidations
the encoding scheme bakes in (the tag here is the operation, not a
data shape).

| Opcode | Old byte | New byte (encoded)                          |
| ------ | -------- | ------------------------------------------- |
| POP    | 0xA0     | 0x90 (`encode_opcode(OP_CLASS_STACK_OP, 0)`)|
| DUP    | 0xA1     | 0x91 (`encode_opcode(OP_CLASS_STACK_OP, 1)`)|
| SWAP   | 0xA2     | 0x92 (`encode_opcode(OP_CLASS_STACK_OP, 2)`)|

### BUILTIN

| Opcode  | Old byte | New byte (encoded)                       |
| ------- | -------- | ---------------------------------------- |
| BUILTIN | 0xC4     | 0x94 (`encode_opcode(OP_CLASS_BUILTIN, 0)`) |

### Function block ops

| Opcode             | Old byte | New byte (encoded)                                  |
| ------------------ | -------- | --------------------------------------------------- |
| FB_LOAD_INSTANCE   | 0xC0     | 0x98 (`encode_opcode(OP_CLASS_FB_LOAD_INSTANCE, 0)`)|
| FB_STORE_PARAM     | 0xC1     | 0x9C (`encode_opcode(OP_CLASS_FB_STORE_PARAM, 0)`)  |
| FB_LOAD_PARAM      | 0xC2     | 0xA0 (`encode_opcode(OP_CLASS_FB_LOAD_PARAM, 0)`)   |
| FB_CALL            | 0xC3     | 0xA4 (`encode_opcode(OP_CLASS_FB_CALL, 0)`)         |

### String ops (15)

| Opcode               | Old byte | New byte (encoded)                                    |
| -------------------- | -------- | ----------------------------------------------------- |
| STR_INIT             | 0xE4     | 0xB8 (`encode_opcode(OP_CLASS_STR_INIT, 0)`)          |
| STR_LOAD_VAR         | 0xE0     | 0xBC (`encode_opcode(OP_CLASS_STR_LOAD_VAR, 0)`)      |
| STR_STORE_VAR        | 0xE1     | 0xC0 (`encode_opcode(OP_CLASS_STR_STORE_VAR, 0)`)     |
| LEN_STR              | 0xE2     | 0xC4 (`encode_opcode(OP_CLASS_LEN_STR, 0)`)           |
| FIND_STR             | 0xE3     | 0xC8 (`encode_opcode(OP_CLASS_FIND_STR, 0)`)          |
| REPLACE_STR          | 0xE5     | 0xCC (`encode_opcode(OP_CLASS_REPLACE_STR, 0)`)       |
| INSERT_STR           | 0xE6     | 0xD0 (`encode_opcode(OP_CLASS_INSERT_STR, 0)`)        |
| DELETE_STR           | 0xE7     | 0xD4 (`encode_opcode(OP_CLASS_DELETE_STR, 0)`)        |
| LEFT_STR             | 0xE8     | 0xD8 (`encode_opcode(OP_CLASS_LEFT_STR, 0)`)          |
| RIGHT_STR            | 0xE9     | 0xDC (`encode_opcode(OP_CLASS_RIGHT_STR, 0)`)         |
| MID_STR              | 0xEA     | 0xE0 (`encode_opcode(OP_CLASS_MID_STR, 0)`)           |
| CONCAT_STR           | 0xEB     | 0xE4 (`encode_opcode(OP_CLASS_CONCAT_STR, 0)`)        |
| STR_INIT_ARRAY       | 0xEC     | 0xE8 (`encode_opcode(OP_CLASS_STR_INIT_ARRAY, 0)`)    |
| STR_LOAD_ARRAY_ELEM  | 0xED     | 0xEC (`encode_opcode(OP_CLASS_STR_LOAD_ARRAY_ELEM, 0)`) |
| STR_STORE_ARRAY_ELEM | 0xEE     | 0xF0 (`encode_opcode(OP_CLASS_STR_STORE_ARRAY_ELEM, 0)`) |

### FORMAT_VERSION bump

`compiler/container/src/header.rs` — `pub const FORMAT_VERSION: u16 = 1;`
becomes `= 2;`. Header tests and any goldens that hardcode the version
byte need to be regenerated alongside.

## Within-wave collisions (all resolve atomically)

- `FB_LOAD_PARAM` → `0xA0` was `POP`'s byte (also moving).
- `STR_STORE_VAR` → `0xC0` was `FB_LOAD_INSTANCE`'s byte (also moving).
- `LEN_STR` → `0xC4` was `BUILTIN`'s byte (also moving).
- `MID_STR` → `0xE0` was `STR_LOAD_VAR`'s byte (also moving).
- `CONCAT_STR` → `0xE4` was `STR_INIT`'s byte (also moving).
- `STR_INIT_ARRAY` → `0xE8` was `LEFT_STR`'s byte (also moving).
- `STR_LOAD_ARRAY_ELEM` → `0xEC` was `STR_INIT_ARRAY`'s byte (also moving).

Outside the wave, all targets are currently free:
- `0x90/0x91/0x92` (STACK_OP) — never assigned.
- `0x94` (BUILTIN) — never assigned.
- `0x98/0x9C/0xA4` (FB ops other than FB_LOAD_PARAM) — never assigned.
- `0xB8/0xBC/0xC8/0xCC/0xD0/0xD4/0xD8/0xDC/0xF0` (string targets
  outside the in-wave collision set) — never assigned.

## Steps

1. **`compiler/container/src/opcode.rs`** — Convert the 23 constants
   from flat hex to `encode_opcode(OP_CLASS_*, tag)`. STACK_OP uses
   tag 0/1/2 to select POP/DUP/SWAP per its consolidated-family
   convention; everything else uses tag 0.
2. **`compiler/container/src/header.rs`** — Bump `FORMAT_VERSION` to
   2.
3. **Hex-byte test sweep.** Bytes to find: 0xA0, 0xA1, 0xA2, 0xC0,
   0xC1, 0xC2, 0xC3, 0xC4, 0xE0–0xEE. Use the same comment-anchored
   programmatic sweep that handled Wave 7's 179 sites; bare-byte
   pass for `&[0xA1]`, `b == 0xC4`, `assert_two_arg_bytecode(_, 0xC4)`,
   etc. `0xC4, <fn-id-LSB>, <fn-id-MSB>` BUILTIN call sequences mean
   only the *first* byte changes — not the function-id operand
   bytes.
4. **`FORMAT_VERSION` test sweep.** Search for any test that hardcodes
   the format version byte (likely in `header.rs` round-trip tests).
   Note `header.rs:342` does `FORMAT_VERSION + 1` to test the
   future-version-rejected path; that line stays as-is (still
   relative).
5. **Regenerate `compiler/vm-cli/resources/test/steel_thread.iplc`**.
   Every byte of bytecode that was `RET_VOID` is changing — but those
   already changed in Wave 7. Wave 8's only impact on this file is
   the FORMAT_VERSION header byte.
6. **Spec doc sync.** Update entries in
   `specs/design/bytecode-instruction-set.md` for the 23 opcodes
   where the doc is currently accurate.
7. **Run the full CI pipeline** (`cd compiler && just`).

## Test churn

- `0xC4` (BUILTIN) appears in ~21 files (every stdlib-function test
  has at least one `0xC4, <id_lsb>, <id_msb>` triple).
- `0xA1` (DUP) appears in ~30 files (every test that hits the
  store-load DUP optimization).
- `0xE0`–`0xEE` (string ops) cluster in the string test files.
- FB ops cluster in `execute_fb_*.rs`.

Estimated diff: ~40-60 files, ~150-200 byte-level changes.

## Lessons applied from prior waves

- Comment-anchored sweep first via the same Wave 7 script approach
  (handles ~80% of sites).
- Bare-byte sweep next for `&[…]` patterns and helper-args.
- BUILTIN's function-id operand bytes (the second/third bytes of
  `0xC4, lsb, msb`) are *not* opcodes — leave unchanged. Detection:
  whenever a 0xC4-prefixed line is matched, only update the first
  byte.
- DUP's old byte 0xA1 appears in store-load optimization comments;
  many tests mention DUP explicitly.
- String tests are dense — sweep `vm/tests/execute_str_*.rs`,
  `codegen/tests/compile_str*.rs`, and `end_to_end_str*.rs` carefully.

## Verification

`cd compiler && just` — compile, coverage (≥85%), clippy, fmt, dupes
all green before opening the PR. Note that the PR will need to be
rebased onto `main` after Wave 7 merges (this branch starts on
Wave 7's branch).

## Significance

This is the final wave. After it merges:

- All ~120 opcodes are on the structured `[op_class:6][type:2]` encoding.
- 41 of 64 op-class slots claimed; 23 free for future fused
  superinstructions.
- `FORMAT_VERSION` reflects the new wire format.
- ADR-0033 transitions from `proposed` to `accepted`.
