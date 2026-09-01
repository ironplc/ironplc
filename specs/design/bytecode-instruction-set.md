# Spec: Virtual PLC Bytecode Instruction Set

## Overview

This spec defines the bytecode instruction set for the IronPLC virtual PLC runtime. The instruction set is designed for a stack-based virtual machine that executes IEC 61131-3 programs compiled from Structured Text (and potentially other IEC 61131-3 languages).

This document describes the instruction set **as implemented**. The normative source of truth is `compiler/container/src/opcode.rs` (opcode bytes, operand widths, builtin func_ids) and `compiler/vm/src/vm.rs` (execution semantics). Byte values are pinned by `compiler/codegen/tests/it/wire_format.rs`, whose completeness guard fails if an opcode is added, removed, or renumbered without updating the pins.

The instruction set builds on these design decisions documented as ADRs:

- **[ADR-0000](../adrs/0000-stack-based-bytecode-vm.md)**: Stack-based bytecode VM as the execution model — chosen over register-based VM, native compilation, tree-walking interpretation, and C transpilation
- **[ADR-0001](../adrs/0001-bytecode-integer-arithmetic-type-strategy.md)**: Two-width integer arithmetic with explicit narrowing — sub-32-bit types are promoted to 32-bit on load; 64-bit types remain at 64-bit; explicit `TRUNC_*` instructions handle truncation back to narrow types
- **[ADR-0002](../adrs/0002-bytecode-overflow-behavior.md)**: Configurable overflow behavior (**proposed, not implemented** — see [Arithmetic Edge Cases](#arithmetic-edge-cases); the VM today is unconditionally wrapping)
- **[ADR-0003](../adrs/0003-plc-standard-function-blocks-as-intrinsics.md)**: Standard function blocks as VM intrinsics via `FB_CALL` — timers, counters, and other standard FBs use the same `FB_CALL` instruction as user-defined FBs, with the VM fast-pathing known type_ids
- **[ADR-0008](../adrs/0008-unified-builtin-opcode.md)**: Unified `BUILTIN` opcode for standard library functions — numeric functions, conversions, shifts, and selection functions share a single `BUILTIN` opcode with func_id dispatch
- **[ADR-0017](../adrs/0017-unified-data-region.md)**: Unified data region — strings, arrays, structures, and FB instances live in one byte-addressed data region; the variable table slot holds a byte offset into it
- **[ADR-0033](../adrs/0033-opcode-encoding-by-class-and-type.md)**: `[op_class:6][type:2]` opcode encoding
- **[ADR-0034](../adrs/0034-string-distinction-via-operand-typing.md)**: A single `STR_*` opcode family serves both STRING and WSTRING; encoding travels with the data, not the opcode (supersedes ADR-0004's separate `WSTR_*` family)
- **[ADR-0035](../adrs/0035-length-and-encoding-prefixed-string-layout.md)**: 6-byte string header `[max_length:u16][cur_length:u16][char_width:u16]`
- **[ADR-0042](../adrs/0042-library-functions-over-compiler-intrinsics.md)**: Library functions over compiler intrinsics — `__TRUNC` / `__MOD` lower to dedicated builtin func_ids

## Encoding

All opcodes are encoded as a single byte (0x00–0xFF). Operands follow the opcode byte and are encoded as fixed-width values whose size depends on the opcode. The encoding is little-endian.

| Operand type | Size | Description |
|-------------|------|-------------|
| u8 | 1 byte | Small index (FB field index, `cmp_op` code, `char_width`) |
| u16 | 2 bytes | Variable index, constant pool index, array descriptor index, function id, FB type id, builtin func_id, string max length |
| i16 | 2 bytes | Signed jump offset |
| u32 | 4 bytes | Data-region byte offset (string opcodes) |

There is no i32 operand: all jumps are i16-relative.

### Opcode Byte Layout

The opcode byte itself is structured as `[op_class:6][type:2]`:

```text
  bits:    7 6 5 4 3 2 1 0
           └──op_class──┘└type┘
```

- **op_class** (high 6 bits) selects the operation. 64 op-class slots in total.
- **type tag** (low 2 bits) selects the type variant (or, for some op classes, the operation within a small consolidated family).

Type tag values:

| Tag | Name | Meaning |
|-----|------|---------|
| 0 | `T_I32` | 32-bit integer (signed, or width-32 unsigned) |
| 1 | `T_I64` | 64-bit integer (signed, or width-64 unsigned) |
| 2 | `T_F32` | 32-bit IEEE-754 float |
| 3 | `T_F64` | 64-bit IEEE-754 float |

Op-classes that operate only on integer subsets use `T_I32`/`T_I64`; the float type tags in those classes are unassigned bytes and trap as `V9003 InvalidInstruction`. Untyped op-classes (jumps, calls, single-variant ops) use `type_tag = 0`.

### Three structural rules

The encoding bakes in three rules that determine where future capacity comes from:

1. **Op class encodes "what operation."** Scarce: 64 slots total. One per top-level operation.
2. **Type tag encodes "what kind of data."** Plentiful: 4 slots per op class, scoped locally. Used for genuine data-shape variation (width, int/float, signed/unsigned).
3. **Sub-opcode (in the operand stream) encodes "which family member."** When a family of related operations shares structure, they may share an op-class slot with an operand byte distinguishing the family member. `CMP_BR` uses this: its first operand byte is the comparison operator.

### In-class consolidations

Three small op-classes pack multiple opcodes into a single op-class slot using the type-tag bits as the operation discriminator (no sub-opcode byte needed):

- `LOAD_BOOL` — collapses `LOAD_FALSE` / `LOAD_TRUE`. Type tag is the boolean value (`0 = FALSE`, `1 = TRUE`).
- `BOOL_OP` — collapses `BOOL_AND` / `BOOL_OR` / `BOOL_XOR` / `BOOL_NOT`. Type tag selects the operator.
- `STACK_OP` — collapses `POP` / `DUP` / `SWAP`. Type tag selects the operator.

### Op-class assignments

The full op-class table (63 of 64 slots used; 0x3F free):

| Class | Op class | Type variants used | Notes |
|---|---|---|---|
| `LOAD_CONST` | 0x00 | I32, I64, F32, F64 | u16 constant pool index operand |
| `LOAD_BOOL` | 0x01 | tags 0=FALSE, 1=TRUE | type tag *is* the value |
| `LOAD_CONST_STR` | 0x02 | only tag 0 | u16 constant pool index; serves STRING and WSTRING |
| `LOAD_VAR` | 0x03 | I32, I64, F32, F64 | u16 var index operand |
| `STORE_VAR` | 0x04 | I32, I64, F32, F64 | u16 var index operand |
| `LOAD_INDIRECT` | 0x05 | only tag 0 | |
| `STORE_INDIRECT` | 0x06 | only tag 0 | |
| `TRUNC` | 0x07 | tags 0=I8, 1=U8, 2=I16, 3=U16 | |
| `ADD` | 0x08 | I32, I64, F32, F64 | wrapping for ints |
| `SUB` | 0x09 | I32, I64, F32, F64 | wrapping for ints |
| `MUL` | 0x0A | I32, I64, F32, F64 | wrapping for ints |
| `NEG` | 0x0B | I32, I64, F32, F64 | |
| `DIV_S` | 0x0C | I32, I64, F32, F64 | signed int + float |
| `DIV_U` | 0x0D | tags 0=U32, 1=U64 | unsigned int only |
| `MOD_S` | 0x0E | tags 0=I32, 1=I64 | no float MOD opcode (see `MOD_F32`/`MOD_F64` builtins) |
| `MOD_U` | 0x0F | tags 0=U32, 1=U64 | unsigned int only |
| `EQ` | 0x10 | I32, I64, F32, F64 | sign-blind |
| `NE` | 0x11 | I32, I64, F32, F64 | sign-blind |
| `LT_S`, `LE_S`, `GT_S`, `GE_S` | 0x12–0x15 | I32, I64, F32, F64 | signed int + float |
| `LT_U`, `LE_U`, `GT_U`, `GE_U` | 0x16–0x19 | tags 0=U32, 1=U64 | unsigned int only |
| `BIT_AND`, `BIT_OR`, `BIT_XOR`, `BIT_NOT` | 0x1A–0x1D | tags 0=W32, 1=W64 | |
| `BOOL_OP` | 0x1E | tags 0=AND, 1=OR, 2=XOR, 3=NOT | type tag selects op |
| `JMP`, `JMP_IF_NOT` | 0x1F–0x20 | only tag 0 | i16 offset operand |
| `CALL`, `RET`, `RET_VOID` | 0x21–0x23 | only tag 0 | |
| `STACK_OP` | 0x24 | tags 0=POP, 1=DUP, 2=SWAP | type tag selects op |
| `BUILTIN` | 0x25 | only tag 0 | u16 builtin func_id operand |
| `FB_LOAD_INSTANCE`, `FB_STORE_PARAM`, `FB_LOAD_PARAM`, `FB_CALL` | 0x26–0x29 | only tag 0 | |
| `LOAD_ARRAY`, `LOAD_ARRAY_DEREF`, `STORE_ARRAY_DEREF` | 0x2A, 0x2C–0x2D | only tag 0 | |
| `STORE_ARRAY` | 0x2B | tags 0=element, 1=`COPY_REGION` | type tag selects the granularity of the store |
| `STR_INIT`, `STR_LOAD_VAR`, `STR_STORE_VAR`, `LEN_STR`, `FIND_STR`, `REPLACE_STR`, `INSERT_STR`, `DELETE_STR`, `LEFT_STR`, `RIGHT_STR`, `MID_STR`, `CONCAT_STR`, `STR_INIT_ARRAY`, `STR_LOAD_ARRAY_ELEM`, `STR_STORE_ARRAY_ELEM` | 0x2E–0x3C | only tag 0 | one slot each |
| `CMP_BR` | 0x3D | tags 0=I32, 1=I64 | fused compare-and-branch; F32/F64 tags reserved |
| `METHOD_CALL` | 0x3E | only tag 0 | OOP extension (ADR-0041 Phase 1 static dispatch) |
| _free_ | 0x3F | — | 1 slot reserved for future use |

### Migration status

The encoding migration is complete: every opcode in `opcode.rs` is derived via `encode_opcode(OP_CLASS_*, type_tag)` and matches the byte values in this document. The container `FORMAT_VERSION` is **3**. Tests in `wire_format.rs` assert specific hex bytes to guard against accidental renumbering — any change to an opcode byte requires updating the corresponding test bytes and bumping `FORMAT_VERSION`.

## Type System

The VM's operand stack is untyped: every slot is a fixed 8-byte `Slot`. The **opcode** carries the type, so `ADD_I32` and `ADD_F32` reinterpret the same slot bits differently. Four machine types are addressable through the type tag:

| VM type | Width | IEC 61131-3 source types | Notes |
|---------|-------|-------------------------|-------|
| I32 | 32-bit | SINT, INT, DINT, BOOL, BYTE, WORD, DWORD, TIME, DATE, TOD | Also carries USINT/UINT/UDINT; the *opcode* (`DIV_U32` vs `DIV_I32`) selects the signed interpretation |
| I64 | 64-bit | LINT, ULINT, LWORD, LTIME, LDT, DT | Also carries reference values (variable indices) |
| F32 | 32-bit float | REAL | IEEE 754 single |
| F64 | 64-bit float | LREAL | IEEE 754 double |

There are no separate `*_U32` / `*_U64` load/store/add/sub/mul opcodes. Unsignedness appears only where the machine operation genuinely differs: division (`DIV_U32`, `DIV_U64`), modulo (`MOD_U32`, `MOD_U64`), and ordered comparison (`LT_U32` … `GE_U64`). Equality is sign-blind and has no unsigned variant.

Additional IEC 61131-3 types are handled as follows:

| IEC type | VM representation | Notes |
|----------|------------------|-------|
| BOOL | I32 (0 or 1) | Boolean ops normalize to 0 or 1 |
| BYTE, WORD, DWORD | I32 | Bit-string ops use `BIT_*_32`; `TRUNC_U8`/`TRUNC_U16` constrain the value range |
| LWORD | I64 | `BIT_*_64` |
| TIME | I32 milliseconds | 32 bits per [ADR-0021](../adrs/0021-time-32bit-ltime-64bit.md); loaded with `LOAD_CONST_I32` |
| LTIME | I64 milliseconds | Loaded with `LOAD_CONST_I64` |
| DATE, TOD, DT, LDT | I32 / I64 | Unsigned representation per [ADR-0025](../adrs/0025-datetime-unsigned-representation.md) |
| STRING, WSTRING | data-region `data_offset` (compile-time constant operand) or temp `buf_idx` (on the stack) | See [String Operations](#string-operations) |
| ARRAY, STRUCT, FB instance | I32 `data_offset` in the variable slot | See [Array Access](#array-access) and [Function Block Operations](#function-block-operations) |
| REF_TO / pointer | I64 holding the target's variable-table index; `u64::MAX` is NULL | See [Reference Operations](#reference-operations-ref_to-and-var_in_out) |

There is no distinct `FieldType::Time`-carrying stack type; `FieldType` (0=I32, 1=U32, 2=I64, 3=U64, 4=F32, 5=F64, 6=String, 7=WString, 8=FbInstance, 9=Time, 10=Slot) is metadata in the container's type section, used for array element descriptors and FB field descriptors — not an operand of any instruction.

### Stack Slot Layout

All stack slots are 8 bytes wide (uniform width). Smaller values (I32, F32, buf_idx, data_offset) occupy the low bytes of the slot; the upper bytes are zero-filled. This uniform layout simplifies the interpreter — `POP`, `DUP`, and `SWAP` operate on fixed-size slots without consulting a type tag. The 8-byte width accommodates the widest native types (I64, F64) without padding or spilling.

## Instruction Set

### Notation

Stack effects show what the instruction pops from and pushes to the operand stack, written `[before] → [after]`. **The rightmost value is the top of stack.**

---

### Load and Store

#### Constants

| Byte | Opcode | Operands | Stack effect | Description |
|---|--------|----------|-------------|-------------|
| 0x00 | LOAD_CONST_I32 | index: u16 | [] → [I32] | Push 32-bit integer from constant pool |
| 0x01 | LOAD_CONST_I64 | index: u16 | [] → [I64] | Push 64-bit integer from constant pool |
| 0x02 | LOAD_CONST_F32 | index: u16 | [] → [F32] | Push 32-bit float from constant pool |
| 0x03 | LOAD_CONST_F64 | index: u16 | [] → [F64] | Push 64-bit float from constant pool |
| 0x04 | LOAD_FALSE | — | [] → [I32] | Push I32 value 0 (boolean FALSE) |
| 0x05 | LOAD_TRUE | — | [] → [I32] | Push I32 value 1 (boolean TRUE) |
| 0x08 | LOAD_CONST_STR | index: u16 | [] → [buf_idx] | Copy a string literal from the constant pool into a freshly allocated temp buffer; push its buf_idx |

The constant pool tags each entry with a `ConstType` (0=I32, 1=U32, 2=I64, 3=U64, 4=F32, 5=F64, 6=Str, 7=WStr). `LOAD_CONST_STR` reads the entry's tag to decide the temp buffer's `char_width`, which is how one opcode serves both STRING and WSTRING (ADR-0034). An index outside the pool traps `V9004 InvalidConstantIndex`.

TIME and LTIME literals are ordinary integer constants: `LOAD_CONST_I32` for TIME (i32 milliseconds), `LOAD_CONST_I64` for LTIME. There is no `LOAD_CONST_TIME` opcode.

#### Variables

Variable instructions use a 16-bit index into the flat variable table ([ADR-0021 — flat variable table](../adrs/0021-flat-variable-table-for-function-calls.md)). The compiler resolves variable names to indices at compile time. Each access is bounds- and scope-checked; a violation traps `V9005 InvalidVariableIndex`.

| Byte | Opcode | Operands | Stack effect | Description |
|---|--------|----------|-------------|-------------|
| 0x0C | LOAD_VAR_I32 | index: u16 | [] → [I32] | Load 32-bit variable (includes promoted SINT/INT/DINT and all unsigned 32-bit types) |
| 0x0D | LOAD_VAR_I64 | index: u16 | [] → [I64] | Load 64-bit variable |
| 0x0E | LOAD_VAR_F32 | index: u16 | [] → [F32] | Load 32-bit float variable |
| 0x0F | LOAD_VAR_F64 | index: u16 | [] → [F64] | Load 64-bit float variable |
| 0x10 | STORE_VAR_I32 | index: u16 | [I32] → [] | Store to 32-bit variable |
| 0x11 | STORE_VAR_I64 | index: u16 | [I64] → [] | Store to 64-bit variable |
| 0x12 | STORE_VAR_F32 | index: u16 | [F32] → [] | Store to 32-bit float variable |
| 0x13 | STORE_VAR_F64 | index: u16 | [F64] → [] | Store to 64-bit float variable |

Because a slot is 8 bytes and untyped, the I32/I64/F32/F64 variants differ only in how the value is widened or reinterpreted; the width tag lets the verifier and the disassembler recover the intended type.

#### Indirect access

| Byte | Opcode | Operands | Stack effect | Description |
|---|--------|----------|-------------|-------------|
| 0x14 | LOAD_INDIRECT | — | [ref] → [value] | Pop a reference (variable index), load that variable, push its value |
| 0x18 | STORE_INDIRECT | — | [value, ref] → [] | Pop a reference and a value, store the value into the referenced variable |

Both trap `V4004 NullDereference` when the reference is the NULL sentinel (`u64::MAX`), and `V9005 InvalidVariableIndex` when the target index is out of scope.

#### Process image (I/O)

There are **no** `LOAD_INPUT` / `STORE_OUTPUT` / `LOAD_MEMORY` / `STORE_MEMORY` opcodes. Located variables (`%I`, `%Q`, `%M`) are allocated ordinary variable-table slots by the compiler and accessed with `LOAD_VAR_*` / `STORE_VAR_*`. A process-image mapping layer is future work.

#### Array Access

Dedicated array opcodes enforce bounds checking on every access. The VM validates that the flat index is within the descriptor's `total_elements` and traps `V4005 ArrayIndexOutOfBounds` otherwise — eliminating buffer overflows by construction.

The array's variable-table slot holds the base `data_offset` into the unified data region (ADR-0017). Elements occupy 8-byte slots, so the element address is `data_offset + index * 8`. The element *type* comes from the array descriptor (`element_type: FieldType`), not from an operand byte.

| Byte | Opcode | Operands | Stack effect | Description |
|---|--------|----------|-------------|-------------|
| 0xA8 | LOAD_ARRAY | var: u16, desc: u16 | [I32] → [value] | Load element; flat index on stack |
| 0xAC | STORE_ARRAY | var: u16, desc: u16 | [value, I32] → [] | Store element; flat index on top |
| 0xB0 | LOAD_ARRAY_DEREF | ref_var: u16, desc: u16 | [I32] → [value] | As `LOAD_ARRAY`, but `ref_var` holds a reference to the target array's variable index (double indirection) |
| 0xB4 | STORE_ARRAY_DEREF | ref_var: u16, desc: u16 | [value, I32] → [] | As `STORE_ARRAY`, through a reference |

The array descriptor is `[element_type: u8][reserved: u8][total_elements: u32 LE][element_extra: u16 LE]`. `element_extra` carries the declared max string length for arrays of STRING/WSTRING.

Multi-dimensional arrays are flattened by the compiler to one dimension in row-major order; the bytecode sees only a flat index.

#### Struct and FB fields

There are no `LOAD_FIELD` / `STORE_FIELD` opcodes. Structure fields resolve to compile-time byte offsets ([ADR-0027](../adrs/0027-compile-time-field-offset-resolution.md)) and are accessed as flat 8-byte slots through `LOAD_ARRAY` / `STORE_ARRAY` with a descriptor whose `element_type` is `FieldType::Slot`. Function-block parameters use `FB_STORE_PARAM` / `FB_LOAD_PARAM` (see [Function Block Operations](#function-block-operations)).

---

### Arithmetic

All arithmetic operates at the promoted width per ADR-0001. The compiler emits `TRUNC_*` (see [Type Conversion](#type-conversion)) when the result must be constrained to a sub-32-bit range.

#### Integer arithmetic

| Byte | Opcode | Stack effect | Description |
|---|--------|-------------|-------------|
| 0x20 | ADD_I32 | [I32, I32] → [I32] | 32-bit addition (`wrapping_add`) |
| 0x21 | ADD_I64 | [I64, I64] → [I64] | 64-bit addition (`wrapping_add`) |
| 0x24 | SUB_I32 | [I32, I32] → [I32] | 32-bit subtraction (`wrapping_sub`) |
| 0x25 | SUB_I64 | [I64, I64] → [I64] | 64-bit subtraction (`wrapping_sub`) |
| 0x28 | MUL_I32 | [I32, I32] → [I32] | 32-bit multiplication (`wrapping_mul`) |
| 0x29 | MUL_I64 | [I64, I64] → [I64] | 64-bit multiplication (`wrapping_mul`) |
| 0x2C | NEG_I32 | [I32] → [I32] | 32-bit negation (`wrapping_neg`) |
| 0x2D | NEG_I64 | [I64] → [I64] | 64-bit negation (`wrapping_neg`) |
| 0x30 | DIV_I32 | [I32, I32] → [I32] | Signed 32-bit division, truncating toward zero |
| 0x31 | DIV_I64 | [I64, I64] → [I64] | Signed 64-bit division |
| 0x34 | DIV_U32 | [I32, I32] → [I32] | Unsigned 32-bit division (operands reinterpreted as u32) |
| 0x35 | DIV_U64 | [I64, I64] → [I64] | Unsigned 64-bit division |
| 0x38 | MOD_I32 | [I32, I32] → [I32] | Signed 32-bit remainder |
| 0x39 | MOD_I64 | [I64, I64] → [I64] | Signed 64-bit remainder |
| 0x3C | MOD_U32 | [I32, I32] → [I32] | Unsigned 32-bit remainder |
| 0x3D | MOD_U64 | [I64, I64] → [I64] | Unsigned 64-bit remainder |

Addition, subtraction, multiplication, and negation are sign-agnostic in two's complement, so there is a single opcode per width.

#### Floating-point arithmetic

| Byte | Opcode | Stack effect | Description |
|---|--------|-------------|-------------|
| 0x22 | ADD_F32 | [F32, F32] → [F32] | 32-bit float addition |
| 0x23 | ADD_F64 | [F64, F64] → [F64] | 64-bit float addition |
| 0x26 | SUB_F32 | [F32, F32] → [F32] | 32-bit float subtraction |
| 0x27 | SUB_F64 | [F64, F64] → [F64] | 64-bit float subtraction |
| 0x2A | MUL_F32 | [F32, F32] → [F32] | 32-bit float multiplication |
| 0x2B | MUL_F64 | [F64, F64] → [F64] | 64-bit float multiplication |
| 0x2E | NEG_F32 | [F32] → [F32] | 32-bit float negation |
| 0x2F | NEG_F64 | [F64] → [F64] | 64-bit float negation |
| 0x32 | DIV_F32 | [F32, F32] → [F32] | 32-bit float division (IEEE 754) |
| 0x33 | DIV_F64 | [F64, F64] → [F64] | 64-bit float division (IEEE 754) |

Float modulo has no opcode; it is the `MOD_F32` / `MOD_F64` builtins (ADR-0042).

---

### Boolean and Bitwise

#### Boolean (operate on BOOL, which is I32 0 or 1)

| Byte | Opcode | Stack effect | Description |
|---|--------|-------------|-------------|
| 0x78 | BOOL_AND | [I32, I32] → [I32] | Logical AND (result is 0 or 1) |
| 0x79 | BOOL_OR | [I32, I32] → [I32] | Logical OR |
| 0x7A | BOOL_XOR | [I32, I32] → [I32] | Logical XOR |
| 0x7B | BOOL_NOT | [I32] → [I32] | Logical NOT |

Boolean operations coerce inputs: any non-zero value is treated as TRUE and normalized to 1 before the operation, and the result is always 0 or 1. `BOOL_AND` on (5, 3) therefore produces 1, not a bitwise AND. The compiler keeps BOOL variables canonical; the opcodes are defensive anyway.

#### Bitwise

| Byte | Opcode | Stack effect | Description |
|---|--------|-------------|-------------|
| 0x68 | BIT_AND_32 | [I32, I32] → [I32] | Bitwise AND, 32-bit |
| 0x69 | BIT_AND_64 | [I64, I64] → [I64] | Bitwise AND, 64-bit |
| 0x6C | BIT_OR_32 | [I32, I32] → [I32] | Bitwise OR, 32-bit |
| 0x6D | BIT_OR_64 | [I64, I64] → [I64] | Bitwise OR, 64-bit |
| 0x70 | BIT_XOR_32 | [I32, I32] → [I32] | Bitwise XOR, 32-bit |
| 0x71 | BIT_XOR_64 | [I64, I64] → [I64] | Bitwise XOR, 64-bit |
| 0x74 | BIT_NOT_32 | [I32] → [I32] | Bitwise NOT, 32-bit |
| 0x75 | BIT_NOT_64 | [I64] → [I64] | Bitwise NOT, 64-bit |

Shifts and rotates (`SHL`, `SHR`, `ROL`, `ROR`) are **not** opcodes. They dispatch through `BUILTIN` — see [Bitwise shift and rotate](#bitwise-shift-and-rotate-builtins). BYTE and WORD rotates have their own narrow-width builtins so that the rotate wraps within 8 or 16 bits rather than 32.

---

### Comparison

Comparison instructions pop two values and push an I32 (0 or 1) result. Equality is sign-blind and needs no unsigned variant; ordering does, because signed and unsigned comparison are different machine operations.

#### Equality (sign-blind)

| Byte | Opcode | Stack effect | Byte | Opcode | Stack effect |
|---|---|---|---|---|---|
| 0x40 | EQ_I32 | [I32, I32] → [I32] | 0x44 | NE_I32 | [I32, I32] → [I32] |
| 0x41 | EQ_I64 | [I64, I64] → [I32] | 0x45 | NE_I64 | [I64, I64] → [I32] |
| 0x42 | EQ_F32 | [F32, F32] → [I32] | 0x46 | NE_F32 | [F32, F32] → [I32] |
| 0x43 | EQ_F64 | [F64, F64] → [I32] | 0x47 | NE_F64 | [F64, F64] → [I32] |

#### Signed and float ordering

| Byte | Opcode | Byte | Opcode | Byte | Opcode | Byte | Opcode |
|---|---|---|---|---|---|---|---|
| 0x48 | LT_I32 | 0x4C | LE_I32 | 0x50 | GT_I32 | 0x54 | GE_I32 |
| 0x49 | LT_I64 | 0x4D | LE_I64 | 0x51 | GT_I64 | 0x55 | GE_I64 |
| 0x4A | LT_F32 | 0x4E | LE_F32 | 0x52 | GT_F32 | 0x56 | GE_F32 |
| 0x4B | LT_F64 | 0x4F | LE_F64 | 0x53 | GT_F64 | 0x57 | GE_F64 |

All pop two operands and push I32 0 or 1.

#### Unsigned ordering

| Byte | Opcode | Byte | Opcode | Byte | Opcode | Byte | Opcode |
|---|---|---|---|---|---|---|---|
| 0x58 | LT_U32 | 0x5C | LE_U32 | 0x60 | GT_U32 | 0x64 | GE_U32 |
| 0x59 | LT_U64 | 0x5D | LE_U64 | 0x61 | GT_U64 | 0x65 | GE_U64 |

Operands are reinterpreted as `u32` / `u64`; the result is I32 0 or 1.

---

### Type Conversion

#### Truncation to narrow integer ranges

| Byte | Opcode | Stack effect | Description |
|---|--------|-------------|-------------|
| 0x1C | TRUNC_I8 | [I32] → [I32] | `(v as i8) as i32` — wraps to −128..127 (SINT) |
| 0x1D | TRUNC_U8 | [I32] → [I32] | `(v as u8) as i32` — wraps to 0..255 (USINT, BYTE) |
| 0x1E | TRUNC_I16 | [I32] → [I32] | `(v as i16) as i32` — wraps to −32768..32767 (INT) |
| 0x1F | TRUNC_U16 | [I32] → [I32] | `(v as u16) as i32` — wraps to 0..65535 (UINT, WORD) |

The truncated value stays 32 bits wide on the stack; the instruction constrains the *value*, not the slot. Truncation is unconditionally wrapping — the configurable overflow policy of ADR-0002 is not implemented (see [Arithmetic Edge Cases](#arithmetic-edge-cases)).

#### Widening, cross-domain, and float conversion

There are no `WIDEN_*`, `NARROW_*`, `REINTERPRET_*`, or `*_TO_*` opcodes. All width and domain conversions go through `BUILTIN` with a `CONV_*` func_id — see [Conversion builtins](#conversion-builtins). Signedness reinterpretation is free: the stack slot is untyped, so the compiler simply picks the signed or unsigned opcode for the next operation and emits nothing.

#### TIME arithmetic

There are no `TIME_ADD` / `TIME_SUB` opcodes. TIME is I32 milliseconds and LTIME is I64 milliseconds (ADR-0021), so duration arithmetic uses the ordinary `ADD_I32` / `SUB_I32` / `ADD_I64` / `SUB_I64` opcodes. Type discipline between TIME and unrelated integers is enforced by the analyzer at compile time, not by dedicated opcodes.

---

### Control Flow

| Byte | Opcode | Operands | Stack effect | Description |
|---|--------|----------|-------------|-------------|
| 0x7C | JMP | offset: i16 | [] → [] | Unconditional jump, relative to the next instruction |
| 0x80 | JMP_IF_NOT | offset: i16 | [I32] → [] | Pop condition; jump if it is zero (FALSE) |
| 0x84 | CALL | function_id: u16, var_offset: u16 | [args…] → [result?] | Call a function; see calling convention below |
| 0x88 | RET | — | [result] → [result] | Return from a function; the value stays on the operand stack for the caller |
| 0x8C | RET_VOID | — | [] → [] | Return from a function with no return value |

There is no `JMP_IF` (branch-if-true) opcode: the compiler inverts the predicate and emits `JMP_IF_NOT`. There are no far-jump variants; all offsets are i16.

#### Calling convention

`CALL` carries both the callee's `function_id` and its `var_offset` — the base of the callee's window in the flat variable table (ADR-0021 — flat variable table). The VM pops `num_params` arguments (declared in the callee's function descriptor) into `var_offset .. var_offset + num_params` in reverse order, so the leftmost argument lands in the lowest slot, then pushes a call frame. The frame stack is bounded by the container's declared worst-case call depth, computed by codegen from the static call graph; exceeding it traps `V9012 CallStackOverflow`, and a container declaring zero depth is rejected at start with `ZeroCallDepth`.

Falling off the end of a function body behaves as `RET_VOID` ([ADR-0044](../adrs/0044-implicit-ret-void-at-end-of-function-body.md), superseding ADR-0011). `verify_stack_balance` checks that point as a return site, and codegen runs it over every container it emits, so an emitted body cannot fall off the end with a non-empty operand stack.

### Fused compare-and-branch (CMP_BR)

| Byte | Opcode | Operands | Stack effect | Description |
|---|--------|----------|-------------|-------------|
| 0xF4 | CMP_BR_I32 | cmp_op: u8, var_idx: u16, const_idx: u16, target: i16 | [] → [] | Compare `vars[var_idx]` (i32) to `const_pool[const_idx]` (i32) with `cmp_op`; branch by `target` if true |
| 0xF5 | CMP_BR_I64 | cmp_op: u8, var_idx: u16, const_idx: u16, target: i16 | [] → [] | As `CMP_BR_I32` but on 64-bit signed integers |

`cmp_op` byte values (`opcode::cmp_op`):

| Code | Mnemonic | Predicate |
|------|----------|-----------|
| 0    | EQ       | `var == const` |
| 1    | NE       | `var != const` |
| 2    | LT_S     | `var <  const` (signed) |
| 3    | LE_S     | `var <= const` (signed) |
| 4    | GT_S     | `var >  const` (signed) |
| 5    | GE_S     | `var >= const` (signed) |

Out-of-range `cmp_op` values trap `V9013 InvalidCmpOp`. The F32/F64 type tags of op-class 0x3D (0xF6, 0xF7) are unassigned and reserved for a future NaN-aware extension.

`CMP_BR_*` is a "branch if true" opcode. Codegen flips the comparison code to get "branch if false" semantics (`EQ ↔ NE`, `LT_S ↔ GE_S`, `LE_S ↔ GT_S`), and commutes the operator (`LT_S ↔ GT_S`, `LE_S ↔ GE_S`) when rewriting `const <cmp> var` into the canonical `var <cmp> const` operand layout.

The compiler emits `CMP_BR_*` for FOR head tests, WHILE conditions (after a do-while restructure), REPEAT `UNTIL` tails, and IF/ELSIF predicates whenever the comparison fits the shape `var <cmp> const` with a 32- or 64-bit signed integer variable and a constant integer literal that fits the variable's width. Other shapes (var-var comparisons, complex conditions, unsigned types, floats) fall back to the unfused `LOAD` + compare + `JMP_IF_NOT` sequence. See `vm-performance.md` §11.

---

### Function Block Operations

Function block invocation follows the pattern: load the FB instance reference, store input parameters, call the FB, load output parameters.

| Byte | Opcode | Operands | Stack effect | Description |
|---|--------|----------|-------------|-------------|
| 0x98 | FB_LOAD_INSTANCE | index: u16 | [] → [fb_ref] | Push the FB instance's `data_offset` from the variable table |
| 0x9C | FB_STORE_PARAM | field: u8 | [fb_ref, value] → [fb_ref] | Pop a value into field `field` of the instance; keeps fb_ref on the stack |
| 0xA0 | FB_LOAD_PARAM | field: u8 | [fb_ref] → [fb_ref, value] | Push field `field` of the instance; keeps fb_ref on the stack |
| 0xA4 | FB_CALL | type_id: u16 | [fb_ref] → [fb_ref] | Invoke the function block; preserves fb_ref for output-parameter access |

`fb_ref` is a byte offset into the unified data region (ADR-0017), not a handle. Fields are 8-byte slots, so field `n` lives at `fb_ref + n * 8`. An offset past the end of the data region traps `V9008 DataRegionOutOfBounds`.

`FB_CALL` dispatches on `type_id` (ADR-0003): the well-known ids below run a native intrinsic in place; anything else is looked up in the container's user-FB table, which supplies the FB's bytecode `function_id`, variable-table `var_offset`, and field count. For a user FB the VM copies the instance's fields from the data region into the variable window, runs the body, then copies them back. An unknown `type_id` traps `V9010 InvalidFbTypeId`.

| type_id | FB | type_id | FB |
|---|---|---|---|
| 0x0010 | TON | 0x0022 | CTUD |
| 0x0011 | TOF | 0x0030 | SR |
| 0x0012 | TP | 0x0031 | RS |
| 0x0020 | CTU | 0x0040 | R_TRIG |
| 0x0021 | CTD | 0x0041 | F_TRIG |

Timer instances (TON, TOF, TP) use a shared 6-field layout: `IN`, `PT`, `Q`, `ET`, plus two hidden fields for the start timestamp and the running flag.

#### Calling convention

A typical FB invocation compiles to:

```text
(* Source: myTimer(IN := start, PT := T#5s); elapsed := myTimer.ET; *)

FB_LOAD_INSTANCE  0x0001      -- push myTimer's data_offset
LOAD_VAR_I32      0x0002      -- push start
FB_STORE_PARAM    0           -- store to IN (field 0); fb_ref stays on stack
LOAD_CONST_I32    0x0003      -- push 5000 (T#5s as i32 milliseconds)
FB_STORE_PARAM    1           -- store to PT (field 1); fb_ref stays on stack
FB_CALL           0x0010      -- run TON; fb_ref stays on stack
FB_LOAD_PARAM     3           -- push ET (field 3); fb_ref still below it
STORE_VAR_I32     0x0004      -- store to elapsed
POP                           -- discard fb_ref
```

`FB_STORE_PARAM`, `FB_LOAD_PARAM`, and `FB_CALL` all keep the instance reference on the stack, so parameter stores, the call, and output loads chain without reloading it. The caller must `POP` the fb_ref when done.

#### Method calls

`METHOD_CALL` (OOP extension, ADR-0041 Phase 1 static dispatch) invokes a method declared on a function block type. It reuses `FB_CALL`'s copy-in/copy-out machinery, but the call site is fully resolved at compile time, so all addressing is carried in the operands rather than looked up by `type_id`.

| Byte | Opcode | Operands | Stack effect | Description |
|---|--------|----------|-------------|-------------|
| 0xF8 | METHOD_CALL | function_id: u16, field_var_off: u16, num_fields: u8, param_var_off: u16 | [fb_ref, arg…] → [fb_ref, result?] | Invoke a method on the instance; preserves fb_ref |

Arguments are popped into the method's own parameter slots at `param_var_off` before `fb_ref` is read, matching `CALL`'s convention. The instance's fields are then copied from the data region into the owning type's field scratch region at `field_var_off`, the body runs as its own frame, and the fields are copied back on return. A method with a return type leaves its value above `fb_ref`; a void method leaves nothing. Either way the caller `POP`s what the method left, then the fb_ref.

Methods reached only through a type's `EXTENDS` chain are not yet compiled: a derived type reserves no storage for its base type's fields, so there is nothing correct to copy in or out.

---

### Reference Operations (REF_TO and VAR_IN_OUT)

A reference is a variable-table index carried in an I64 slot, with `u64::MAX` reserved as the NULL sentinel. There are no dedicated `LOAD_VAR_REF` / `DEREF_LOAD` / `DEREF_STORE` opcodes:

- `REF(v)` compiles to `LOAD_CONST_I64` of `v`'s variable index.
- `NULL` compiles to `LOAD_CONST_I64` of `u64::MAX`.
- `r^` (read) compiles to the reference expression followed by `LOAD_INDIRECT`.
- `r^ := e` compiles to the value, the reference, then `STORE_INDIRECT`.
- Element access through a reference to an array uses `LOAD_ARRAY_DEREF` / `STORE_ARRAY_DEREF`, which resolve the reference and bounds-check in one instruction.

Every indirect access null-checks first (`V4004 NullDereference`) and scope-checks the resolved index (`V9005 InvalidVariableIndex`). Codegen initializes every reference variable to the NULL sentinel during program setup, because a zeroed slot would otherwise be a *valid* index to variable 0. See `specs/design/ref-to.md` for the full safety rules (no `REF` of array elements, no `REF` of function-local temporaries, `=`/`<>` comparison only).

#### Example

```text
(* Source *)
FUNCTION_BLOCK Accumulator
  VAR_IN_OUT counter : DINT; END_VAR
  VAR_INPUT  increment : DINT; END_VAR
  counter := counter + increment;
END_FUNCTION_BLOCK

(* FB body bytecode for counter := counter + increment *)
LOAD_VAR_I64   <counter_ref>   -- load the reference held in the VAR_IN_OUT slot
LOAD_INDIRECT                  -- dereference: push the caller's value
LOAD_VAR_I32   <increment>     -- push increment
ADD_I32                        -- counter + increment
LOAD_VAR_I64   <counter_ref>   -- reload the reference
STORE_INDIRECT                 -- write back through the reference
```

---

### Built-in Standard Library Functions

The `BUILTIN` opcode provides a single dispatch mechanism for the standard library (ADR-0008). Rather than dedicating an opcode to each function, `BUILTIN` takes a u16 `func_id` operand identifying the target. This parallels `FB_CALL`: one opcode handles an extensible family, with the operand naming the specific operation.

| Byte | Opcode | Operands | Stack effect | Description |
|---|--------|----------|-------------|-------------|
| 0x94 | BUILTIN | func_id: u16 | [args…] → [result] | Call a built-in; stack effect depends on func_id |

`opcode::builtin::arg_count(func_id)` is the single source of truth for how many operands a builtin pops; codegen uses it for stack-depth tracking and the stack verifier uses the non-panicking `arg_count_opt`. An unknown func_id traps `V9007 InvalidBuiltinFunction`.

Builtins are monomorphized by the compiler: a generic IEC 61131-3 signature (`ANY_NUM`, `ANY_REAL`) is resolved to a concrete type at the call site and the type-specific func_id is emitted.

#### Built-in func_id ranges

| Range | Category |
|-------|----------|
| 0x0000–0x033F | Unassigned |
| 0x0340–0x03A6 | Numeric, bitwise shift/rotate, selection, conversion, BCD, and string-conversion builtins |
| 0x03A7–0x03FF | Unassigned |
| 0x0402–0x0410 | `MUX` on I32 (base 0x0400 + arity) |
| 0x0422–0x0430 | `MUX` on I64 (base 0x0420 + arity) |
| 0x0442–0x0450 | `MUX` on F32 (base 0x0440 + arity) |
| 0x0462–0x0470 | `MUX` on F64 (base 0x0460 + arity) |
| 0x0471–0xFFFF | Unassigned |

#### Numeric and selection builtins

| func_id | Name | Args | Description |
|---------|------|------|-------------|
| 0x0340 | EXPT_I32 | 2 | `a ** b`; traps `V4002 NegativeExponent` on a negative exponent |
| 0x0341 | EXPT_F32 | 2 | `a.powf(b)` |
| 0x0342 | EXPT_F64 | 2 | `a.powf(b)` |
| 0x0343 | ABS_I32 | 1 | Absolute value (wrapping) |
| 0x0344 | MIN_I32 | 2 | Signed minimum |
| 0x0345 | MAX_I32 | 2 | Signed maximum |
| 0x0346 | LIMIT_I32 | 3 | `clamp(IN, MN, MX)`; pops MX, IN, MN |
| 0x0347 | SEL_I32 | 3 | `G = 0 ? IN0 : IN1`; pops IN1, IN0, G |
| 0x0354 | ABS_F32 | 1 | Absolute value |
| 0x0355 | ABS_F64 | 1 | Absolute value |
| 0x0356 | MIN_F32 | 2 | Minimum |
| 0x0357 | MIN_F64 | 2 | Minimum |
| 0x0358 | MAX_F32 | 2 | Maximum |
| 0x0359 | MAX_F64 | 2 | Maximum |
| 0x035A | LIMIT_F32 | 3 | Clamp |
| 0x035B | LIMIT_F64 | 3 | Clamp |
| 0x035C | SEL_F32 | 3 | Select (F32 values, I32 selector) |
| 0x035D | SEL_F64 | 3 | Select (F64 values, I32 selector) |
| 0x035E | SQRT_F32 | 1 | Square root |
| 0x035F | SQRT_F64 | 1 | Square root |
| 0x0360 | EXPT_I64 | 2 | `a ** b`; traps on negative exponent |
| 0x0361 | ABS_I64 | 1 | Absolute value (wrapping) |
| 0x0362 | MIN_I64 | 2 | Signed minimum |
| 0x0363 | MAX_I64 | 2 | Signed maximum |
| 0x0364 | LIMIT_I64 | 3 | Clamp |
| 0x0365 | SEL_I64 | 3 | Select (I64 values, I32 selector) |
| 0x0366 | MIN_U32 | 2 | Unsigned minimum |
| 0x0367 | MAX_U32 | 2 | Unsigned maximum |
| 0x0368 | LIMIT_U32 | 3 | Unsigned clamp |
| 0x0369 | MIN_U64 | 2 | Unsigned minimum |
| 0x036A | MAX_U64 | 2 | Unsigned maximum |
| 0x036B | LIMIT_U64 | 3 | Unsigned clamp |
| 0x03A3 | TRUNC_F64 | 1 | `f64::trunc` — result stays F64 (ADR-0042; `__TRUNC` lowering target) |
| 0x03A4 | MOD_F64 | 2 | Floating modulo, sign of dividend; `x % 0.0` is NaN, not a trap |
| 0x03A5 | TRUNC_F32 | 1 | `f32::trunc` |
| 0x03A6 | MOD_F32 | 2 | Floating modulo on F32 |

#### Math builtins

| func_id | Name | func_id | Name | func_id | Name |
|---------|------|---------|------|---------|------|
| 0x036C | LN_F32 | 0x0372 | SIN_F32 | 0x0378 | ASIN_F32 |
| 0x036D | LN_F64 | 0x0373 | SIN_F64 | 0x0379 | ASIN_F64 |
| 0x036E | LOG_F32 | 0x0374 | COS_F32 | 0x037A | ACOS_F32 |
| 0x036F | LOG_F64 | 0x0375 | COS_F64 | 0x037B | ACOS_F64 |
| 0x0370 | EXP_F32 | 0x0376 | TAN_F32 | 0x037C | ATAN_F32 |
| 0x0371 | EXP_F64 | 0x0377 | TAN_F64 | 0x037D | ATAN_F64 |

All take one argument. `LOG_*` is base-10; `LN_*` is natural. Two-argument variants:

| func_id | Name | Args | Description |
|---------|------|------|-------------|
| 0x039B | ATAN2_F32 | 2 | `atan2(IN1, IN2)`; pops IN2 (X) then IN1 (Y) |
| 0x039C | ATAN2_F64 | 2 | As above, on F64 |

#### Bitwise shift and rotate builtins

All take 2 arguments: the shift/rotate count `n` on top of the value `a`. Counts are applied with Rust's `wrapping_shl` / `wrapping_shr` / `rotate_*`, which mask the count to the operand's bit width — so a 32-bit shift by 32 behaves as a shift by 0, deterministically on every target.

| func_id | Name | Description |
|---------|------|-------------|
| 0x0348 | SHL_I32 | Shift left, 32-bit |
| 0x0349 | SHL_I64 | Shift left, 64-bit |
| 0x034A | SHR_I32 | Shift right (logical, zero-fill), 32-bit |
| 0x034B | SHR_I64 | Shift right (logical), 64-bit |
| 0x034C | ROL_I32 | Rotate left, 32-bit |
| 0x034D | ROL_I64 | Rotate left, 64-bit |
| 0x034E | ROR_I32 | Rotate right, 32-bit |
| 0x034F | ROR_I64 | Rotate right, 64-bit |
| 0x0350 | ROL_U8 | Rotate left within 8 bits (BYTE) |
| 0x0351 | ROL_U16 | Rotate left within 16 bits (WORD) |
| 0x0352 | ROR_U8 | Rotate right within 8 bits (BYTE) |
| 0x0353 | ROR_U16 | Rotate right within 16 bits (WORD) |

#### Conversion builtins

All take one argument.

| func_id | Name | func_id | Name |
|---------|------|---------|------|
| 0x037E | CONV_I32_TO_F32 | 0x0387 | CONV_F32_TO_I64 |
| 0x037F | CONV_I32_TO_F64 | 0x0388 | CONV_F64_TO_I32 |
| 0x0380 | CONV_I64_TO_F32 | 0x0389 | CONV_F64_TO_I64 |
| 0x0381 | CONV_I64_TO_F64 | 0x038A | CONV_F32_TO_U32 |
| 0x0382 | CONV_U32_TO_F32 | 0x038B | CONV_F32_TO_U64 |
| 0x0383 | CONV_U32_TO_F64 | 0x038C | CONV_F64_TO_U32 |
| 0x0384 | CONV_U64_TO_F32 | 0x038D | CONV_F64_TO_U64 |
| 0x0385 | CONV_U64_TO_F64 | 0x038E | CONV_F32_TO_F64 |
| 0x0386 | CONV_F32_TO_I32 | 0x038F | CONV_F64_TO_F32 |
| 0x0390 | CONV_U32_TO_I64 | 0x0399 | CONV_I32_TO_BOOL |
| | | 0x039A | CONV_I64_TO_BOOL |

Float-to-integer conversions truncate toward zero and follow Rust's `as` saturating-cast semantics (see [Float-to-integer overflow](#float-to-integer-overflow)). `CONV_*_TO_BOOL` maps zero to 0 and any non-zero value to 1.

BCD conversion:

| func_id | Name | func_id | Name |
|---------|------|---------|------|
| 0x0391 | BCD_TO_INT_8 (BYTE → USINT) | 0x0395 | INT_TO_BCD_8 (USINT → BYTE) |
| 0x0392 | BCD_TO_INT_16 (WORD → UINT) | 0x0396 | INT_TO_BCD_16 (UINT → WORD) |
| 0x0393 | BCD_TO_INT_32 (DWORD → UDINT) | 0x0397 | INT_TO_BCD_32 (UDINT → DWORD) |
| 0x0394 | BCD_TO_INT_64 (LWORD → ULINT) | 0x0398 | INT_TO_BCD_64 (ULINT → LWORD) |

#### String conversion and comparison builtins

These are dispatched inline in the VM main loop rather than through the shared builtin dispatcher, because they need access to the temp buffer pool and the data region.

| func_id | Name | Args | Stack effect | Description |
|---------|------|------|--------------|-------------|
| 0x039D | CONV_I32_TO_STR | 1 | [I32] → [buf_idx] | Signed decimal to string |
| 0x039E | CONV_U32_TO_STR | 1 | [I32] → [buf_idx] | Unsigned decimal to string |
| 0x039F | CONV_STR_TO_I32 | 1 | [data_offset] → [I32] | Parse decimal; 0 on failure |
| 0x03A0 | CONV_F32_TO_STR | 1 | [F32] → [buf_idx] | Float to decimal string |
| 0x03A1 | CONV_STR_TO_F32 | 1 | [data_offset] → [F32] | Parse decimal; 0.0 on failure |
| 0x03A2 | CMP_STR | 2 | [data_offset, data_offset] → [I32] | Three-way lexicographic compare; −1 / 0 / +1 |

`CMP_STR` is how the compiler lowers `=`, `<>`, `<`, `<=`, `>`, `>=` on strings: emit `CMP_STR`, then compare its result to 0 with the ordinary integer comparison opcodes.

#### MUX

`MUX` is extensible — the number of `IN` arguments varies per call site — so its arity is encoded in the func_id: `BASE + n`, where `n` is the number of `IN` arguments (2..16). The call pops `n + 1` values: the `n` inputs plus the `K` selector.

| Base | Type |
|------|------|
| 0x0400 | I32 |
| 0x0420 | I64 |
| 0x0440 | F32 |
| 0x0460 | F64 |

`MUX_MAX_INPUTS` is 16. `opcode::builtin::is_mux` and `mux_info` decode a func_id back to its arity.

---

### Stack Operations

| Byte | Opcode | Stack effect | Description |
|---|--------|-------------|-------------|
| 0x90 | POP | [value] → [] | Discard top of stack |
| 0x91 | DUP | [value] → [value, value] | Duplicate top of stack |
| 0x92 | SWAP | [a, b] → [b, a] | Swap top two stack values |

---

### Whole-Region Copy

Whole-aggregate assignment (`x := y` where both sides are arrays or
structures) is a value copy under IEC 61131-3 §7.3.3.1. An aggregate
variable's slot holds its data-region byte offset, so a load/store pair would
copy the offset and leave the destination aliasing the source; `COPY_REGION`
moves the bytes instead.

| # | Opcode | Operands | Stack effect | Description |
|---|--------|----------|-------------|-------------|
| 0xAD | COPY_REGION | dst_var: u16, dst_desc: u16, src_desc: u16 | [src_offset] → [] | Copy a whole aggregate within the data region |

**The instruction carries no length.** The VM derives the byte size of each
end from the array descriptor named in the operand and traps
`RegionSizeMismatch` (V9018) if the two disagree. A length immediate would let
a code-generation defect over-copy into a neighbouring variable — the class of
bug this instruction exists to prevent — whereas a descriptor is container
metadata the verifier can also inspect. It is the same discipline `LOAD_ARRAY`
follows by taking `total_elements` from the descriptor rather than an operand.

The destination is named by variable index so the access is scope-checked; it
is the side that writes. The source arrives as a data-region offset on the
stack so that both `s := t` (preceded by `LOAD_VAR_I32 t`) and `s := f()`,
where a struct-returning call leaves its offset on the stack and has no
variable index in the caller's scope, use one instruction.

Overlapping ranges are well defined — the VM uses `copy_within` — so `x := x`
is a no-op rather than corruption.

Descriptors cannot distinguish `ARRAY[1..6] OF INT` from
`ARRAY[1..2,1..3] OF INT`, nor `INT` elements from `DINT` elements (both are
8-byte slots). Declared-type equality is checked statically instead, by the
analyzer, which reports P2037 on a mismatch.

**Encoding.** `COPY_REGION` is `STORE_ARRAY` at type tag 1 — the same op
class, one granularity coarser — rather than an op class of its own. Op
classes exist to keep the dispatch table small, not to name operations, so a
single instruction does not earn one; measurement has not shown dispatch-table
size to be a bottleneck. Tags 2–3 of the class remain free. The obvious
candidate for one is a `COPY_REGION_DYN` taking its sizes from a runtime
descriptor rather than a container descriptor, which is what an Ed. 3
variable-length array (`ARRAY[*]`, a `VAR_IN_OUT` parameter whose extents are
a property of the call rather than of the program text) would need.

---

### String Operations

IEC 61131-3 strings have a declared maximum length known at compile time (e.g. `STRING(20)` holds at most 20 characters). Strings are stored as fixed-size regions — never heap-allocated — matching PLC runtimes like CODESYS and TwinCAT and giving deterministic memory usage with no dynamic allocation during a scan.

#### Storage model

Per [ADR-0035](../adrs/0035-length-and-encoding-prefixed-string-layout.md), every string value carries a 6-byte header:

```text
offset  size     field
0       2 bytes  max_length (u16, declared capacity in code units)
2       2 bytes  cur_length (u16, current length in code units)
4       2 bytes  char_width (u16, bytes per code unit: 1 = STRING, 2 = WSTRING)
6       n bytes  data (not null-terminated)
```

Total size is `max_length * char_width + 6` bytes. Lengths are code units, not bytes. `STRING` data is Latin-1 and `WSTRING` data is UTF-16LE ([ADR-0016](../adrs/0016-string-encoding.md)).

Two storage areas hold strings:

- **Data region** — string *variables* (and string array elements) live in the unified data region (ADR-0017), addressed by a compile-time-constant `data_offset`. `char_width` is written once at initialization and never changes.
- **Temp buffer pool** — a pre-allocated pool of fixed-size buffers holding intermediate results. A buffer is addressed by a small `buf_idx`, which is what string-producing operations push onto the stack. The container header declares `num_temp_bufs` and `max_temp_buf_bytes`; codegen sizes them from the program's string expressions. Exhausting the pool traps `V9009 TempBufferExhausted`.

Codegen sizes the pool by counting string-operation *call sites* statically,
and the VM rewinds the allocator only on function return. A string operation
inside a loop therefore allocates a fresh buffer on every iteration while
having been counted once, and a loop that runs more than a couple of times
traps `V9009`. This is why the bundled `Tc2_Utilities` `LREAL_TO_FMTSTR`
renders digits as unrolled per-weight blocks rather than a loop: rewriting it
as a loop would trap.

Because `buf_idx` is a small integer, `DUP` and `SWAP` copy only the index — never buffer contents. Real copies happen at `STR_STORE_VAR` and inside the string operation handlers.

#### STRING/WSTRING distinction

Per ADR-0034, a *single* `STR_*` opcode family serves both STRING and WSTRING; the encoding travels with the data. Every operand resolves to a location whose header names its `char_width`, and every operation that touches two strings verifies their widths agree, trapping `V9014 EncodingMismatch` on a mismatch. A `char_width` byte that is neither 1 nor 2 traps `V9015 InvalidCharWidth`. This preserves ADR-0004's safety property — STRING and WSTRING can never be silently confused — without a parallel `WSTR_*` opcode family.

#### Instructions

Operands marked `data_offset` are u32 byte offsets into the data region, fixed at compile time.

| Byte | Opcode | Operands | Stack effect | Description |
|---|--------|----------|-------------|-------------|
| 0xB8 | STR_INIT | data_offset: u32, max_length: u16, char_width: u8 | [] → [] | Write a string header (`max_length`, `cur_length = 0`, `char_width`) at `data_offset` |
| 0xBC | STR_LOAD_VAR | data_offset: u32 | [] → [buf_idx] | Copy the string at `data_offset` into a fresh temp buffer; push its index |
| 0xC0 | STR_STORE_VAR | data_offset: u32 | [buf_idx] → [] | Copy a temp buffer's contents into the string at `data_offset` (value-copy assignment, truncating to the destination's `max_length`) |
| 0xC4 | LEN_STR | data_offset: u32 | [] → [I32] | Push `cur_length` (LEN) |
| 0xC8 | FIND_STR | in1: u32, in2: u32 | [] → [I32] | 1-based position of IN2 within IN1, or 0 if absent (FIND) |
| 0xCC | REPLACE_STR | in1: u32, in2: u32 | [L, P] → [buf_idx] | Replace L characters of IN1 at position P with IN2 (REPLACE); P is popped first |
| 0xD0 | INSERT_STR | in1: u32, in2: u32 | [P] → [buf_idx] | Insert IN2 into IN1 after position P (INSERT) |
| 0xD4 | DELETE_STR | in1: u32 | [L, P] → [buf_idx] | Delete L characters of IN1 starting at position P (DELETE); P is popped first |
| 0xD8 | LEFT_STR | in: u32 | [L] → [buf_idx] | Leftmost L characters (LEFT) |
| 0xDC | RIGHT_STR | in: u32 | [L] → [buf_idx] | Rightmost L characters (RIGHT) |
| 0xE0 | MID_STR | in: u32 | [L, P] → [buf_idx] | L characters of IN starting at position P (MID); P is popped first |
| 0xE4 | CONCAT_STR | in1: u32, in2: u32 | [] → [buf_idx] | Concatenate IN1 and IN2 |

String *arrays* have their own opcodes, because the element's `data_offset` is computed at runtime from the flat index. All three take the array's variable index and an array descriptor index, and bounds-check like `LOAD_ARRAY`.

| Byte | Opcode | Operands | Stack effect | Description |
|---|--------|----------|-------------|-------------|
| 0xE8 | STR_INIT_ARRAY | var: u16, desc: u16 | [] → [] | Write a string header into every element, using `element_extra` from the descriptor as `max_length` |
| 0xEC | STR_LOAD_ARRAY_ELEM | var: u16, desc: u16 | [I32] → [buf_idx] | Copy element `index` into a temp buffer |
| 0xF0 | STR_STORE_ARRAY_ELEM | var: u16, desc: u16 | [buf_idx, I32] → [] | Copy a temp buffer into element `index` |

#### Operand model and nested expressions

The string function opcodes take their string inputs as *compile-time data offsets*, not stack values. A nested string expression therefore cannot be passed directly: the compiler allocates a scratch slot in the data region, emits `STR_INIT` for it, compiles the inner expression (which leaves a `buf_idx` on the stack), and emits `STR_STORE_VAR` to spill the result into the scratch slot — whose offset then becomes the outer operation's operand. A string *literal* argument is handled the same way: allocate a scratch slot, `STR_INIT`, `LOAD_CONST_STR`, `STR_STORE_VAR`.

Results always go to a temp buffer. If a result exceeds the temp buffer's capacity it is truncated, matching standard PLC string truncation semantics.

**Buffer lifecycle.** Temp buffers are allocated from the pool as string operations run, and the allocation mark is restored per call frame. A temp `buf_idx` is valid until the next operation that allocates from the pool; the compiler ensures no `buf_idx` stays live across such an operation, in practice by emitting `STR_STORE_VAR` as soon as a string expression completes. The pool size in the header guarantees the pool is never exhausted if that analysis is correct; if it is wrong the VM traps `V9009` rather than corrupting data.

---

### Debug

There are no `NOP`, `BREAKPOINT`, or `LINE` opcodes. Debug information is carried out-of-band in the container's debug section — a bytecode-offset-to-source-line map and variable name/type metadata ([ADR-0019](../adrs/0019-type-encoding-in-debug-variable-names.md)) — so debug builds and release builds execute byte-identical bytecode. Breakpoints are implemented by the VM's debug hook, which the `vm-cli` DAP adapter drives against the line map. See `specs/design/debug-info-in-iplc-container.md` and `specs/design/debugger-support.md`.

---

## Opcode Summary

The encoding allocates 63 of 64 op-class slots and 126 opcode bytes. Within each op-class, the type tag (low 2 bits) selects either the data-type variant or a family-member operation (for the consolidated `LOAD_BOOL`, `BOOL_OP`, and `STACK_OP` classes).

| Op-class | Bytes | Count | Description |
|----------|-------|-------|-------------|
| `LOAD_CONST` (0x00) | 0x00–0x03 | 4 | Constant pool loads — type tag selects width (I32/I64/F32/F64) |
| `LOAD_BOOL` (0x01) | 0x04–0x05 | 2 | Boolean literal — type tag *is* the value (0=FALSE, 1=TRUE) |
| `LOAD_CONST_STR` (0x02) | 0x08 | 1 | Load a STRING or WSTRING literal from the constant pool |
| `LOAD_VAR` (0x03) | 0x0C–0x0F | 4 | Variable load — type tag selects slot width |
| `STORE_VAR` (0x04) | 0x10–0x13 | 4 | Variable store — type tag selects slot width |
| `LOAD_INDIRECT` (0x05) | 0x14 | 1 | Indirect load (dereference a reference on the stack) |
| `STORE_INDIRECT` (0x06) | 0x18 | 1 | Indirect store |
| `TRUNC` (0x07) | 0x1C–0x1F | 4 | Truncate to a narrow integer range (tag: 0=I8, 1=U8, 2=I16, 3=U16) |
| `ADD` (0x08) | 0x20–0x23 | 4 | Addition |
| `SUB` (0x09) | 0x24–0x27 | 4 | Subtraction |
| `MUL` (0x0A) | 0x28–0x2B | 4 | Multiplication |
| `NEG` (0x0B) | 0x2C–0x2F | 4 | Negation |
| `DIV_S` (0x0C) | 0x30–0x33 | 4 | Signed integer / float division |
| `DIV_U` (0x0D) | 0x34–0x35 | 2 | Unsigned integer division |
| `MOD_S` (0x0E) | 0x38–0x39 | 2 | Signed integer modulo |
| `MOD_U` (0x0F) | 0x3C–0x3D | 2 | Unsigned integer modulo |
| `EQ` (0x10) | 0x40–0x43 | 4 | Equality (sign-blind) |
| `NE` (0x11) | 0x44–0x47 | 4 | Inequality (sign-blind) |
| `LT_S` (0x12) | 0x48–0x4B | 4 | Signed / float less-than |
| `LE_S` (0x13) | 0x4C–0x4F | 4 | Signed / float less-than-or-equal |
| `GT_S` (0x14) | 0x50–0x53 | 4 | Signed / float greater-than |
| `GE_S` (0x15) | 0x54–0x57 | 4 | Signed / float greater-than-or-equal |
| `LT_U` (0x16) | 0x58–0x59 | 2 | Unsigned less-than |
| `LE_U` (0x17) | 0x5C–0x5D | 2 | Unsigned less-than-or-equal |
| `GT_U` (0x18) | 0x60–0x61 | 2 | Unsigned greater-than |
| `GE_U` (0x19) | 0x64–0x65 | 2 | Unsigned greater-than-or-equal |
| `BIT_AND` (0x1A) | 0x68–0x69 | 2 | Bitwise AND (tag: 0=W32, 1=W64) |
| `BIT_OR` (0x1B) | 0x6C–0x6D | 2 | Bitwise OR |
| `BIT_XOR` (0x1C) | 0x70–0x71 | 2 | Bitwise XOR |
| `BIT_NOT` (0x1D) | 0x74–0x75 | 2 | Bitwise NOT |
| `BOOL_OP` (0x1E) | 0x78–0x7B | 4 | Boolean ops (tag: 0=AND, 1=OR, 2=XOR, 3=NOT) |
| `JMP` (0x1F) | 0x7C | 1 | Unconditional jump (i16 offset) |
| `JMP_IF_NOT` (0x20) | 0x80 | 1 | Jump if top-of-stack is zero |
| `CALL` (0x21) | 0x84 | 1 | Function call (u16 function_id, u16 var_offset) |
| `RET` (0x22) | 0x88 | 1 | Return with value |
| `RET_VOID` (0x23) | 0x8C | 1 | Return without value |
| `STACK_OP` (0x24) | 0x90–0x92 | 3 | Stack manipulation (tag: 0=POP, 1=DUP, 2=SWAP) |
| `BUILTIN` (0x25) | 0x94 | 1 | Built-in standard-library call (u16 func_id) |
| `FB_LOAD_INSTANCE` (0x26) | 0x98 | 1 | Push FB instance data_offset |
| `FB_STORE_PARAM` (0x27) | 0x9C | 1 | Store an FB parameter field |
| `FB_LOAD_PARAM` (0x28) | 0xA0 | 1 | Load an FB parameter field |
| `FB_CALL` (0x29) | 0xA4 | 1 | Invoke an FB (intrinsic or bytecode body) |
| `LOAD_ARRAY` (0x2A) | 0xA8 | 1 | Load array element |
| `STORE_ARRAY` (0x2B) | 0xAC–0xAD | 2 | Store into array storage (tag: 0=one element, 1=`COPY_REGION`) |
| `LOAD_ARRAY_DEREF` (0x2C) | 0xB0 | 1 | Load array element via reference |
| `STORE_ARRAY_DEREF` (0x2D) | 0xB4 | 1 | Store array element via reference |
| `STR_INIT` (0x2E) | 0xB8 | 1 | Initialize a string header in the data region |
| `STR_LOAD_VAR` (0x2F) | 0xBC | 1 | Copy a string variable into a temp buffer |
| `STR_STORE_VAR` (0x30) | 0xC0 | 1 | Copy a temp buffer into a string variable |
| `LEN_STR` (0x31) | 0xC4 | 1 | Read string length |
| `FIND_STR` (0x32) | 0xC8 | 1 | Find substring position |
| `REPLACE_STR` (0x33) | 0xCC | 1 | Replace substring |
| `INSERT_STR` (0x34) | 0xD0 | 1 | Insert substring |
| `DELETE_STR` (0x35) | 0xD4 | 1 | Delete substring |
| `LEFT_STR` (0x36) | 0xD8 | 1 | Leftmost N characters |
| `RIGHT_STR` (0x37) | 0xDC | 1 | Rightmost N characters |
| `MID_STR` (0x38) | 0xE0 | 1 | Middle substring |
| `CONCAT_STR` (0x39) | 0xE4 | 1 | Concatenate strings |
| `STR_INIT_ARRAY` (0x3A) | 0xE8 | 1 | Initialize every string header in an array |
| `STR_LOAD_ARRAY_ELEM` (0x3B) | 0xEC | 1 | Load a string from an array element |
| `STR_STORE_ARRAY_ELEM` (0x3C) | 0xF0 | 1 | Store a temp buffer into a string array element |
| `CMP_BR` (0x3D) | 0xF4–0xF5 | 2 | Fused compare-and-branch (tag: 0=I32, 1=I64) |
| `METHOD_CALL` (0x3E) | 0xF8 | 1 | Static-dispatch method call on a function block instance |
| _free_ (0x3F) | — | 0 | Reserved for future use |
| **Total** | | **126** | 63 of 64 op-class slots in use |

Every byte not listed above is unassigned; executing one traps `V9003 InvalidInstruction`. `opcode::instruction_size` is the single source of truth for instruction lengths (shared by the emitter, the optimizer, and the disassembler), and `opcode::is_assigned` is derived from it.

### Instruction sizes

| Size | Instructions |
|------|-------------|
| 1 byte | All arithmetic, comparison, boolean, bitwise, `TRUNC_*`, `LOAD_TRUE`/`LOAD_FALSE`, `LOAD_INDIRECT`/`STORE_INDIRECT`, `POP`/`DUP`/`SWAP`, `RET`/`RET_VOID` |
| 2 bytes | `FB_STORE_PARAM`, `FB_LOAD_PARAM` (u8 field index) |
| 3 bytes | `LOAD_CONST_*`, `LOAD_CONST_STR`, `LOAD_VAR_*`, `STORE_VAR_*`, `FB_LOAD_INSTANCE`, `FB_CALL`, `JMP`, `JMP_IF_NOT`, `BUILTIN` (u16) |
| 5 bytes | `CALL`, `LOAD_ARRAY`, `STORE_ARRAY`, `LOAD_ARRAY_DEREF`, `STORE_ARRAY_DEREF`, `STR_INIT_ARRAY`, `STR_LOAD_ARRAY_ELEM`, `STR_STORE_ARRAY_ELEM` (u16 + u16); `STR_LOAD_VAR`, `STR_STORE_VAR`, `LEN_STR`, `DELETE_STR`, `LEFT_STR`, `RIGHT_STR`, `MID_STR` (u32) |
| 7 bytes | `COPY_REGION` (u16 + u16 + u16) |
| 8 bytes | `STR_INIT` (u32 + u16 + u8); `CMP_BR_I32`, `CMP_BR_I64` (u8 + u16 + u16 + i16); `METHOD_CALL` (u16 + u16 + u8 + u16) |
| 9 bytes | `FIND_STR`, `REPLACE_STR`, `INSERT_STR`, `CONCAT_STR` (u32 + u32) |

## Compilation Examples

### Simple arithmetic with truncation

```text
(* Source *)
VAR x : SINT; y : SINT; z : SINT; END_VAR
z := x + y;

(* Bytecode *)
LOAD_VAR_I32   0x0000    -- load x (SINT, sign-extended to I32)
LOAD_VAR_I32   0x0001    -- load y
ADD_I32                  -- I32 addition
TRUNC_I8                 -- constrain to SINT range
STORE_VAR_I32  0x0002    -- store z
```

### IF/ELSE

```text
(* Source *)
IF condition THEN x := 1; ELSE x := 2; END_IF;

(* Bytecode *)
LOAD_VAR_I32   0x0000    -- load condition (BOOL as I32)
JMP_IF_NOT     +9        -- jump to ELSE if false
LOAD_CONST_I32 0x0001    -- push 1
STORE_VAR_I32  0x0001    -- store x
JMP            +6        -- jump past ELSE
LOAD_CONST_I32 0x0002    -- push 2 (ELSE target)
STORE_VAR_I32  0x0001    -- store x
```

### FOR loop with a fused head test

```text
(* Source *)
FOR i := 0 TO 9 DO sum := sum + i; END_FOR;

(* Bytecode *)
LOAD_CONST_I32 0x0000        -- push 0
STORE_VAR_I32  0x0000        -- i := 0
                             -- loop_start:
CMP_BR_I32 GT_S, 0x0000, 0x0001, +23
                             -- if i > 9 branch to loop_exit; no stack traffic
LOAD_VAR_I32   0x0001        -- load sum
LOAD_VAR_I32   0x0000        -- load i
ADD_I32                      -- sum + i
STORE_VAR_I32  0x0001        -- store sum
LOAD_VAR_I32   0x0000        -- load i
LOAD_CONST_I32 0x0002        -- push 1
ADD_I32                      -- i + 1
STORE_VAR_I32  0x0000        -- store i
JMP            -31           -- back to loop_start
                             -- loop_exit:
```

Without the fused form (for example when the bound is a variable), the head test compiles to `LOAD_VAR_I32` / `LOAD_CONST_I32` / `GT_I32` / `JMP_IF_NOT` with the predicate inverted.

### Function block call (timer)

```text
(* Source *)
myTimer(IN := startButton, PT := T#5s);
IF myTimer.Q THEN output := TRUE; END_IF;

(* Bytecode *)
FB_LOAD_INSTANCE 0x0000  -- push myTimer's data_offset
LOAD_VAR_I32     0x0001  -- push startButton
FB_STORE_PARAM   0       -- store IN
LOAD_CONST_I32   0x0000  -- push 5000 (T#5s in milliseconds)
FB_STORE_PARAM   1       -- store PT
FB_CALL          0x0010  -- run TON; fb_ref stays on stack
FB_LOAD_PARAM    2       -- push Q; fb_ref below it
SWAP                     -- [fb_ref, Q] → [Q, fb_ref]
POP                      -- discard fb_ref
JMP_IF_NOT       +4      -- skip if Q is FALSE
LOAD_TRUE                -- push TRUE
STORE_VAR_I32    0x0002  -- store output
```

### String assignment

```text
(* Source *)
VAR msg : STRING(20); END_VAR
msg := CONCAT('Hello, ', name);

(* Bytecode, during setup *)
STR_INIT   <msg_off>,     20, 1   -- header for msg
STR_INIT   <lit_off>,    254, 1   -- scratch slot for the literal
LOAD_CONST_STR 0x0000             -- 'Hello, ' into a temp buffer
STR_STORE_VAR  <lit_off>          -- spill it to the scratch slot

(* Bytecode, in the body *)
CONCAT_STR <lit_off>, <name_off>  -- result into a temp buffer; push buf_idx
STR_STORE_VAR  <msg_off>          -- value-copy into msg, truncating to 20
```

## Design Decisions

The following questions were resolved with a "prioritize safety" principle ([ADR-0005](../adrs/0005-safety-first-design-principle.md)): when in doubt, prefer encodings that make type information and invariants statically checkable over clever encodings that save opcode space but rely on the compiler always getting things right.

1. **WSTRING vs STRING → one opcode family, encoding on the data (ADR-0034, superseding ADR-0004).** ADR-0004 originally chose a parallel `WSTR_*` opcode family. Two changes since made that the wrong trade: ADR-0017 dissolved the buffer table where ADR-0004's per-entry encoding tag was to live, and the opcode budget turned out tighter than estimated. The safety property is preserved by moving the encoding into the data instead: each string's header carries `char_width`, every operand resolves to a location with a known width, and every operation that touches two strings verifies they agree (`V9014`). One family, no silent confusion.

2. **Array access → dedicated `LOAD_ARRAY` / `STORE_ARRAY` with mandatory bounds checking.** Compiling array access to pointer arithmetic (`base + index * size`) makes bounds checking optional and fragile — a compiler bug silently produces buffer overflows. A dedicated opcode makes the check mandatory and atomic: the VM validates the flat index against the descriptor's `total_elements` on every access. Buffer overflows are the top class of safety bugs in embedded systems; this eliminates them by construction ([ADR-0023](../adrs/0023-array-bounds-safety.md)).

3. **CASE statement → no `TABLE_SWITCH`; keep comparison chains.** A `TABLE_SWITCH` opcode requires the VM to validate that the jump table is well-formed. A chain of compare-and-branch instructions is trivially verifiable — each target is individually validated — and `CMP_BR_*` makes the chain cheap. The performance difference is negligible for typical PLC CASE statements (5–20 arms).

4. **Exponentiation → a builtin, not an opcode.** Exponentiation involves floating-point edge cases (0^0, negative base with fractional exponent, overflow). A library function can return explicit error indicators and be tested independently. Since `EXPT` is rare in PLC code, there is no performance argument for a dedicated opcode.

5. **Standard library → a single `BUILTIN` opcode with func_id dispatch (ADR-0008).** Numeric functions, conversions, shifts, selection functions, and string conversions are library functions, not fundamental type operations. One opcode with a u16 func_id handles all of them and leaves op-class slots for operations that genuinely need their own decode path. The pattern mirrors `FB_CALL`.

6. **Shifts and rotates → builtins, not opcodes.** `SHL`, `SHR`, `ROL`, `ROR` are IEC standard *functions*, not operators, and BYTE/WORD rotates need width-specific behavior that would cost four more type tags. Routing them through `BUILTIN` keeps four op-class slots free at no measurable cost.

7. **TIME → no dedicated arithmetic opcodes.** ADR-0021 made TIME a 32-bit millisecond integer and LTIME a 64-bit one, so TIME arithmetic *is* integer arithmetic. Dedicated `TIME_ADD`/`TIME_SUB` opcodes would spend op-class slots to re-check a property the analyzer already enforces at compile time, where the diagnostic is far more useful than a runtime trap.

8. **Fused compare-and-branch → one op-class, operator in the operand stream.** `CMP_BR_*` collapses the four-instruction `LOAD_VAR` / `LOAD_CONST` / compare / `JMP_IF_NOT` sequence that dominates loop heads and IF predicates. Encoding the six comparison operators as a `cmp_op` operand byte rather than six op-classes keeps the whole family in one slot (rule 3 of the encoding).

## Arithmetic Edge Cases

The following behaviors are normative. The VM implements these exactly to ensure deterministic, portable execution across all targets.

### Overflow behavior

**ADR-0002's configurable overflow policy is not implemented.** The VM is unconditionally *wrapping*: `ADD_*`, `SUB_*`, `MUL_*`, and `NEG_*` use two's complement wrapping arithmetic, and `TRUNC_*` truncates by discarding high bits. There is no VM startup setting for saturating or faulting arithmetic, and no bytecode encodes one. Adopting ADR-0002 would change VM configuration and possibly `TRUNC_*` semantics, but would not add opcodes.

| Operation | Behavior today |
|---|---|
| `ADD_I32` / `ADD_I64` / `SUB_*` / `MUL_*` | `wrapping_add` / `wrapping_sub` / `wrapping_mul` |
| `NEG_I32` on `i32::MIN`, `NEG_I64` on `i64::MIN` | `wrapping_neg` — the value wraps to itself |
| `TRUNC_I8` / `TRUNC_U8` / `TRUNC_I16` / `TRUNC_U16` | Discard high bits, then sign- or zero-extend back to 32 bits |

### Division by zero

All integer division and modulo instructions trap `V4001 DivideByZero`: `DIV_I32`, `DIV_U32`, `DIV_I64`, `DIV_U64`, `MOD_I32`, `MOD_U32`, `MOD_I64`, `MOD_U64`.

Floating-point division by zero follows IEEE 754: `x / 0.0` produces `±Inf` and `0.0 / 0.0` produces `NaN`. `DIV_F32` and `DIV_F64` do not trap. The `MOD_F32` / `MOD_F64` builtins likewise return NaN for `x % 0.0` rather than trapping.

### Shift amounts

The shift and rotate builtins mask the count to the operand's bit width:

| Builtins | Mask | Effect |
|---|---|---|
| `SHL_I32`, `SHR_I32`, `ROL_I32`, `ROR_I32` | `count & 31` | Count 0–31 |
| `SHL_I64`, `SHR_I64`, `ROL_I64`, `ROR_I64` | `count & 63` | Count 0–63 |
| `ROL_U8`, `ROR_U8` | `count & 7` | Rotate within 8 bits |
| `ROL_U16`, `ROR_U16` | `count & 15` | Rotate within 16 bits |

A 32-bit shift by 32 therefore behaves as a shift by 0. This matches Rust's `wrapping_shl` / `wrapping_shr` and makes behavior deterministic across platforms (ARM and x86 differ in their native out-of-range shift behavior).

`SHR_*` is a *logical* shift: the value is reinterpreted as unsigned before shifting, so the vacated high bits are zero-filled regardless of sign.

### Float-to-integer overflow

The `CONV_F*_TO_I*` and `CONV_F*_TO_U*` builtins truncate toward zero and use Rust's saturating `as` cast semantics:

| Input | Result |
|---|---|
| Value above the target maximum | Target maximum |
| Value below the target minimum | Target minimum |
| NaN | 0 |

They do not trap. The target ranges are the usual ones for i32, u32, i64, and u64.

### Float comparison with NaN

All float comparison instructions follow IEEE 754:

- `NaN == NaN` → 0 (false)
- `NaN != NaN` → 1 (true)
- `NaN < x`, `NaN > x`, `NaN <= x`, `NaN >= x` → 0 (false) for any x

## Verification

The container carries a stack-discipline verifier (`compiler/container/src/verify.rs`) that abstractly interprets each function's control-flow graph and checks the rules from `specs/design/bytecode-verifier-rules.md`:

| Rule | Meaning |
|------|---------|
| R0200 | Stack depth agrees at every control-flow merge point |
| R0202 | No instruction pops from an empty stack |
| R0203 | Depth never exceeds the container's declared `max_stack_depth` |
| — | Every path leaving a function leaves exactly the depth the calling convention promises: 1 slot for `RET`, 0 for `RET_VOID` and for falling off the end |

Verification runs on the bytecode that ships, independently of the emitter's own depth bookkeeping. Type-level verification (checking that a `ADD_F64` receives F64 operands) is not yet implemented; the VM's runtime traps — invalid instruction, invalid index, encoding mismatch, bounds violations — are the enforcement layer today.

### Runtime traps

| Code | Trap | Cause |
|---|---|---|
| V4001 | DivideByZero | Integer division or modulo by zero |
| V4002 | NegativeExponent | `EXPT_I32` / `EXPT_I64` with a negative exponent |
| V4003 | WatchdogTimeout | A task exceeded its watchdog interval |
| V4004 | NullDereference | Indirect access through the NULL sentinel |
| V4005 | ArrayIndexOutOfBounds | Flat index outside the descriptor's `total_elements` |
| V9001 / V9002 | StackOverflow / StackUnderflow | Operand stack limits |
| V9003 | InvalidInstruction | Unassigned opcode byte |
| V9004 | InvalidConstantIndex | Constant pool index out of range or wrong type |
| V9005 | InvalidVariableIndex | Variable index outside the current scope |
| V9006 | InvalidFunctionId | `CALL` to an unknown function |
| V9007 | InvalidBuiltinFunction | `BUILTIN` with an unknown func_id |
| V9008 | DataRegionOutOfBounds | Data-region access past the end |
| V9009 | TempBufferExhausted | String temp buffer pool exhausted |
| V9010 | InvalidFbTypeId | `FB_CALL` to an unknown type_id |
| V9011 | UnexpectedEndOfBytecode | Bytecode ended mid-instruction |
| V9012 | CallStackOverflow | Call frame stack exceeded its declared depth |
| V9013 | InvalidCmpOp | `CMP_BR_*` with a `cmp_op` byte above 5 |
| V9014 | EncodingMismatch | STRING/WSTRING width mismatch between two operands |
| V9015 | InvalidCharWidth | A string header's `char_width` is neither 1 nor 2 |
| V9016 | ProgramExceedsCallDepth | Declared call depth exceeds the VM's frame capacity |
| V9017 | ZeroCallDepth | Container declares a call depth of zero |

V4xxx codes are user errors (exit code 1); V9xxx codes are internal errors (exit code 3).

## Known Limitations

The following are known limitations of the current instruction set. They are intentional trade-offs and may be addressed in future versions.

1. **Jump offset range** — All jumps use i16 offsets (−32768..+32767 bytes from the next instruction). A single function whose bytecode exceeds that range cannot be encoded; there are no far-jump variants.

2. **Field index width** — `FB_STORE_PARAM` and `FB_LOAD_PARAM` use a u8 field index, limiting a function block to 255 parameter fields. This is sufficient for all standard function blocks and typical user-defined ones.

3. **Array element stride** — Array elements are fixed 8-byte slots, so an array of a small type spends 8 bytes per element. Structures are laid out as flat arrays of 8-byte slots for the same reason ([ADR-0026](../adrs/0026-structure-memory-layout.md)).

4. **Multi-dimensional arrays** — IEC 61131-3 supports multi-dimensional arrays (e.g. `ARRAY[1..3, 1..4] OF INT`). The bytecode supports only a flat index; the compiler flattens to row-major order and computes the linear index from the subscripts, so `arr[i, j]` in a `[1..3, 1..4]` array becomes `arr[(i-1)*4 + (j-1)]`.

5. **String operands are compile-time offsets** — The string function opcodes take data-region offsets, not stack values, so nested string expressions require the compiler to allocate scratch slots and spill intermediates (see [Operand model and nested expressions](#operand-model-and-nested-expressions)). Scratch slots are allocated per call site, so string-heavy code costs data region.

6. **No runtime service opcodes** — There is no `CLOCK` or `SYSCALL` instruction for bytecode to access runtime services. Standard FBs that need timer access (TON, TOF, TP) are intrinsics (ADR-0003), and system uptime is exposed as two reserved variable slots ([ADR-0030](../adrs/0030-dual-uptime-system-variables.md)). A user-defined FB that extends a standard timer via `EXTENDS` falls through to bytecode interpretation and has no direct timer access; such FBs should use composition instead.

7. **`REF` restrictions** — `REF()` of an array element is rejected, because a reference encodes only a variable-table index and cannot carry an element offset. `REF()` of a `FUNCTION`'s local or parameter is rejected to prevent dangling references.

8. **No type-level verification** — The shipped verifier checks stack discipline, not operand types. A hand-crafted container could feed F64 operands to `ADD_I32`; the result would be garbage rather than a trap.

## Out of Scope

The following PLC runtime features are **not addressed** by this instruction set and are covered elsewhere or not yet specified:

1. **Task scheduling** — Multi-task execution *is* implemented, but outside the instruction set: the container's task table declares tasks (interval, priority, watchdog) and program instances, and the VM's cooperative scheduler drives them. No opcode participates. See `specs/design/vm-task-scheduler.md` and `specs/design/61131-task-support.md`.

2. **Online change** — Updating the running program without stopping the PLC (hot-swapping bytecode while preserving variable state). This requires handling variable persistence, bytecode replacement at scan boundaries, and FB instance state migration.

3. **RETAIN / PERSISTENT variables** — IEC 61131-3 variables with `RETAIN` or `PERSISTENT` qualifiers survive power cycles. The container's variable metadata has no retention flag; a future format revision would add one.

4. **Subrange enforcement** — Enumerations and subrange types compile to their underlying integer types. Enumeration symbolic names survive only in the debug section, and subrange constraints are not enforced at runtime.

5. **General pointer types** — `REF_TO` and `REF()`/`^`/`NULL` are supported as variable-table references (see [Reference Operations](#reference-operations-ref_to-and-var_in_out)), but there is no pointer arithmetic and no reference to arbitrary memory.

6. **Process image mapping** — Located variables (`%I`, `%Q`, `%M`) are parsed and allocated ordinary variable slots, but there is no process-image freeze/flush cycle or I/O driver binding in the instruction set.
