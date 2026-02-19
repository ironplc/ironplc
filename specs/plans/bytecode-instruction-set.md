# Spec: Virtual PLC Bytecode Instruction Set

## Overview

This spec defines the bytecode instruction set for the IronPLC virtual PLC runtime. The instruction set is designed for a stack-based virtual machine that executes IEC 61131-3 programs compiled from Structured Text (and potentially other IEC 61131-3 languages).

The instruction set builds on five design decisions documented as ADRs:

0. **[ADR-0000](../adrs/0000-stack-based-bytecode-vm.md)**: Stack-based bytecode VM as the execution model — chosen over register-based VM, native compilation, tree-walking interpretation, and C transpilation
1. **[ADR-0001](../adrs/0001-bytecode-integer-arithmetic-type-strategy.md)**: Two-width integer arithmetic with explicit narrowing — sub-32-bit types are promoted to 32-bit on load; 64-bit types remain at 64-bit; explicit NARROW instructions handle truncation back to narrow types
2. **[ADR-0002](../adrs/0002-bytecode-overflow-behavior.md)**: Configurable overflow behavior at narrowing points — the VM supports wrap, saturate, and fault modes as a startup configuration
3. **[ADR-0003](../adrs/0003-plc-standard-function-blocks-as-intrinsics.md)**: Standard function blocks as VM intrinsics via FB_CALL — timers, counters, and other standard FBs use the same FB_CALL instruction as user-defined FBs, with the VM fast-pathing known types
4. **[ADR-0008](../adrs/0008-unified-builtin-opcode.md)**: Unified BUILTIN opcode for standard library functions — string functions, numeric functions, and other standard library functions share a single BUILTIN opcode with func_id dispatch, freeing opcode slots for future extensions

## Encoding

All opcodes are encoded as a single byte (0x00–0xFF). Operands follow the opcode byte and are encoded as fixed-width values whose size depends on the opcode. The encoding is little-endian.

| Operand type | Size | Description |
|-------------|------|-------------|
| u8 | 1 byte | Small index (field index, type tag) |
| u16 | 2 bytes | Variable/constant pool index |
| i16 | 2 bytes | Signed jump offset |
| u32 | 4 bytes | Extended index (for large constant pools) |

## Type System

The VM operates on six value types internally, following the two-width model from ADR-0001:

| VM type | Width | IEC 61131-3 source types | Notes |
|---------|-------|-------------------------|-------|
| I32 | 32-bit signed | SINT, INT, DINT | SINT/INT sign-extended on load |
| U32 | 32-bit unsigned | USINT, UINT, UDINT | USINT/UINT zero-extended on load |
| I64 | 64-bit signed | LINT | Native width |
| U64 | 64-bit unsigned | ULINT | Native width |
| F32 | 32-bit float | REAL | IEEE 754 single |
| F64 | 64-bit float | LREAL | IEEE 754 double |

Additional IEC 61131-3 types are handled as follows:

| IEC type | VM representation | Notes |
|----------|------------------|-------|
| BOOL | I32 (0 or 1) | Promoted to I32; boolean ops produce 0 or 1 |
| BYTE | U32 | Treated as unsigned 8-bit, zero-extended |
| WORD | U32 | Treated as unsigned 16-bit, zero-extended |
| DWORD | U32 | Native width |
| LWORD | U64 | Native width |
| TIME | I64 | Microseconds since epoch; sign allows negative durations |
| DATE, TOD, DT | I64 | Microseconds; specific interpretation is runtime-defined |
| STRING | buf_idx | Index to a fixed-size buffer (max length known at compile time); see String Operations |
| WSTRING | buf_idx | Index to a fixed-size wide-char buffer (max length known at compile time); see String Operations |

## Instruction Set

### Notation

Each instruction is shown as:

```
OPCODE operand1, operand2   — description
  Stack effect: [before] → [after]
```

Stack effects show what the instruction pops from and pushes to the operand stack. Values are consumed left-to-right and the rightmost value is the top of stack.

---

### Load and Store

These instructions move values between the operand stack and memory regions.

#### Constants

| # | Opcode | Operands | Stack effect | Description |
|---|--------|----------|-------------|-------------|
| 0x01 | LOAD_CONST_I32 | index: u16 | [] → [I32] | Push 32-bit signed integer from constant pool |
| 0x02 | LOAD_CONST_U32 | index: u16 | [] → [U32] | Push 32-bit unsigned integer from constant pool |
| 0x03 | LOAD_CONST_I64 | index: u16 | [] → [I64] | Push 64-bit signed integer from constant pool |
| 0x04 | LOAD_CONST_U64 | index: u16 | [] → [U64] | Push 64-bit unsigned integer from constant pool |
| 0x05 | LOAD_CONST_F32 | index: u16 | [] → [F32] | Push 32-bit float from constant pool |
| 0x06 | LOAD_CONST_F64 | index: u16 | [] → [F64] | Push 64-bit float from constant pool |
| 0x07 | LOAD_TRUE | — | [] → [I32] | Push I32 value 1 (boolean TRUE) |
| 0x08 | LOAD_FALSE | — | [] → [I32] | Push I32 value 0 (boolean FALSE) |
| 0x09 | LOAD_CONST_STR | index: u16 | [] → [buf_idx] | Copy STRING literal from constant pool into a temporary buffer; push buf_idx |
| 0x0A | LOAD_CONST_WSTR | index: u16 | [] → [buf_idx] | Copy WSTRING literal from constant pool into a temporary buffer; push buf_idx |

#### Variables

Variable instructions use a 16-bit index into the current scope's variable table. The compiler resolves variable names to indices at compile time.

| # | Opcode | Operands | Stack effect | Description |
|---|--------|----------|-------------|-------------|
| 0x10 | LOAD_VAR_I32 | index: u16 | [] → [I32] | Load 32-bit signed variable (includes promoted SINT, INT, DINT) |
| 0x11 | LOAD_VAR_U32 | index: u16 | [] → [U32] | Load 32-bit unsigned variable (includes promoted USINT, UINT, UDINT) |
| 0x12 | LOAD_VAR_I64 | index: u16 | [] → [I64] | Load 64-bit signed variable |
| 0x13 | LOAD_VAR_U64 | index: u16 | [] → [U64] | Load 64-bit unsigned variable |
| 0x14 | LOAD_VAR_F32 | index: u16 | [] → [F32] | Load 32-bit float variable |
| 0x15 | LOAD_VAR_F64 | index: u16 | [] → [F64] | Load 64-bit float variable |
| 0x18 | STORE_VAR_I32 | index: u16 | [I32] → [] | Store to 32-bit signed variable |
| 0x19 | STORE_VAR_U32 | index: u16 | [U32] → [] | Store to 32-bit unsigned variable |
| 0x1A | STORE_VAR_I64 | index: u16 | [I64] → [] | Store to 64-bit signed variable |
| 0x1B | STORE_VAR_U64 | index: u16 | [U64] → [] | Store to 64-bit unsigned variable |
| 0x1C | STORE_VAR_F32 | index: u16 | [F32] → [] | Store to 32-bit float variable |
| 0x1D | STORE_VAR_F64 | index: u16 | [F64] → [] | Store to 64-bit float variable |

#### Process Image (I/O)

Process image instructions access the PLC's input and output memory. Inputs are frozen at the start of each scan cycle; outputs are flushed at the end.

| # | Opcode | Operands | Stack effect | Description |
|---|--------|----------|-------------|-------------|
| 0x20 | LOAD_INPUT | region: u8, index: u16 | [] → [value] | Read from input process image (%I) |
| 0x21 | STORE_OUTPUT | region: u8, index: u16 | [value] → [] | Write to output process image (%Q) |
| 0x22 | LOAD_MEMORY | region: u8, index: u16 | [] → [value] | Read from memory region (%M) |
| 0x23 | STORE_MEMORY | region: u8, index: u16 | [value] → [] | Write to memory region (%M) |

The `region` byte encodes the access width and determines the stack value type:

| Region | Width | IEC notation | Stack type |
|--------|-------|-------------|------------|
| 0 | Bit | X | I32 (0 or 1) |
| 1 | Byte | B | U32 (zero-extended) |
| 2 | Word | W | U32 (zero-extended) |
| 3 | Doubleword | D | U32 |
| 4 | Longword | L | U64 |

The verifier uses this mapping to determine the type pushed by LOAD_INPUT / LOAD_MEMORY and the type expected by STORE_OUTPUT / STORE_MEMORY.

#### Array Access

Dedicated array opcodes enforce bounds checking on every access. The VM validates that the index is within the declared array bounds and traps on out-of-bounds access — eliminating buffer overflows by construction. The alternative (compiling array access to pointer arithmetic) would make bounds checking optional and fragile.

The `type` byte encodes the element type: 0=I32, 1=U32, 2=I64, 3=U64, 4=F32, 5=F64, 6=buf_idx (STRING element), 7=buf_idx (WSTRING element), 8=fb_ref (struct/FB element). The `array` operand is a u16 index into the variable table identifying the array base.

| # | Opcode | Operands | Stack effect | Description |
|---|--------|----------|-------------|-------------|
| 0x24 | LOAD_ARRAY | array: u16, type: u8 | [I32] → [value] | Load element from array; index on stack; traps on out-of-bounds |
| 0x25 | STORE_ARRAY | array: u16, type: u8 | [value, I32] → [] | Store element to array; index on stack; traps on out-of-bounds |

#### Struct and FB Fields

| # | Opcode | Operands | Stack effect | Description |
|---|--------|----------|-------------|-------------|
| 0x28 | LOAD_FIELD | field: u8 | [fb_ref] → [value] | Load field from struct/FB instance on stack |
| 0x29 | STORE_FIELD | field: u8 | [value, fb_ref] → [] | Store field to struct/FB instance on stack |

---

### Arithmetic

All arithmetic operates at the promoted width per ADR-0001. The compiler emits NARROW instructions (see Type Conversion) when the result must be stored to a sub-32-bit variable.

#### Integer Arithmetic (32-bit)

| # | Opcode | Operands | Stack effect | Description |
|---|--------|----------|-------------|-------------|
| 0x30 | ADD_I32 | — | [I32, I32] → [I32] | Signed 32-bit addition |
| 0x31 | SUB_I32 | — | [I32, I32] → [I32] | Signed 32-bit subtraction |
| 0x32 | MUL_I32 | — | [I32, I32] → [I32] | Signed 32-bit multiplication |
| 0x33 | DIV_I32 | — | [I32, I32] → [I32] | Signed 32-bit division (truncates toward zero) |
| 0x34 | MOD_I32 | — | [I32, I32] → [I32] | Signed 32-bit modulo |
| 0x35 | NEG_I32 | — | [I32] → [I32] | Signed 32-bit negation |
| 0x36 | ADD_U32 | — | [U32, U32] → [U32] | Unsigned 32-bit addition |
| 0x37 | SUB_U32 | — | [U32, U32] → [U32] | Unsigned 32-bit subtraction |
| 0x38 | MUL_U32 | — | [U32, U32] → [U32] | Unsigned 32-bit multiplication |
| 0x39 | DIV_U32 | — | [U32, U32] → [U32] | Unsigned 32-bit division |
| 0x3A | MOD_U32 | — | [U32, U32] → [U32] | Unsigned 32-bit modulo |

#### Integer Arithmetic (64-bit)

| # | Opcode | Operands | Stack effect | Description |
|---|--------|----------|-------------|-------------|
| 0x3C | ADD_I64 | — | [I64, I64] → [I64] | Signed 64-bit addition |
| 0x3D | SUB_I64 | — | [I64, I64] → [I64] | Signed 64-bit subtraction |
| 0x3E | MUL_I64 | — | [I64, I64] → [I64] | Signed 64-bit multiplication |
| 0x3F | DIV_I64 | — | [I64, I64] → [I64] | Signed 64-bit division |
| 0x40 | MOD_I64 | — | [I64, I64] → [I64] | Signed 64-bit modulo |
| 0x41 | NEG_I64 | — | [I64] → [I64] | Signed 64-bit negation |
| 0x42 | ADD_U64 | — | [U64, U64] → [U64] | Unsigned 64-bit addition |
| 0x43 | SUB_U64 | — | [U64, U64] → [U64] | Unsigned 64-bit subtraction |
| 0x44 | MUL_U64 | — | [U64, U64] → [U64] | Unsigned 64-bit multiplication |
| 0x45 | DIV_U64 | — | [U64, U64] → [U64] | Unsigned 64-bit division |
| 0x46 | MOD_U64 | — | [U64, U64] → [U64] | Unsigned 64-bit modulo |

#### Floating-Point Arithmetic

| # | Opcode | Operands | Stack effect | Description |
|---|--------|----------|-------------|-------------|
| 0x48 | ADD_F32 | — | [F32, F32] → [F32] | 32-bit float addition |
| 0x49 | SUB_F32 | — | [F32, F32] → [F32] | 32-bit float subtraction |
| 0x4A | MUL_F32 | — | [F32, F32] → [F32] | 32-bit float multiplication |
| 0x4B | DIV_F32 | — | [F32, F32] → [F32] | 32-bit float division |
| 0x4C | NEG_F32 | — | [F32] → [F32] | 32-bit float negation |
| 0x4D | ADD_F64 | — | [F64, F64] → [F64] | 64-bit float addition |
| 0x4E | SUB_F64 | — | [F64, F64] → [F64] | 64-bit float subtraction |
| 0x4F | MUL_F64 | — | [F64, F64] → [F64] | 64-bit float multiplication |
| 0x50 | DIV_F64 | — | [F64, F64] → [F64] | 64-bit float division |
| 0x51 | NEG_F64 | — | [F64] → [F64] | 64-bit float negation |

---

### Boolean and Bitwise

Boolean operations operate on I32 values where 0 = FALSE and 1 = TRUE. Bitwise operations operate on the full bit width of their operands.

#### Boolean (operate on BOOL, which is I32 0 or 1)

| # | Opcode | Operands | Stack effect | Description |
|---|--------|----------|-------------|-------------|
| 0x54 | BOOL_AND | — | [I32, I32] → [I32] | Logical AND (result is 0 or 1) |
| 0x55 | BOOL_OR | — | [I32, I32] → [I32] | Logical OR (result is 0 or 1) |
| 0x56 | BOOL_XOR | — | [I32, I32] → [I32] | Logical XOR (result is 0 or 1) |
| 0x57 | BOOL_NOT | — | [I32] → [I32] | Logical NOT (result is 0 or 1) |

#### Bitwise (operate on full-width integer values)

| # | Opcode | Operands | Stack effect | Description |
|---|--------|----------|-------------|-------------|
| 0x58 | BIT_AND_32 | — | [U32, U32] → [U32] | Bitwise AND, 32-bit |
| 0x59 | BIT_OR_32 | — | [U32, U32] → [U32] | Bitwise OR, 32-bit |
| 0x5A | BIT_XOR_32 | — | [U32, U32] → [U32] | Bitwise XOR, 32-bit |
| 0x5B | BIT_NOT_32 | — | [U32] → [U32] | Bitwise NOT, 32-bit |
| 0x5C | SHL_32 | — | [U32, U32] → [U32] | Shift left, 32-bit (shift amount on top) |
| 0x5D | SHR_32 | — | [U32, U32] → [U32] | Shift right (logical), 32-bit |
| 0x5E | ROL_32 | — | [U32, U32] → [U32] | Rotate left, 32-bit |
| 0x5F | ROR_32 | — | [U32, U32] → [U32] | Rotate right, 32-bit |
| 0x60 | BIT_AND_64 | — | [U64, U64] → [U64] | Bitwise AND, 64-bit |
| 0x61 | BIT_OR_64 | — | [U64, U64] → [U64] | Bitwise OR, 64-bit |
| 0x62 | BIT_XOR_64 | — | [U64, U64] → [U64] | Bitwise XOR, 64-bit |
| 0x63 | BIT_NOT_64 | — | [U64] → [U64] | Bitwise NOT, 64-bit |
| 0x64 | SHL_64 | — | [U64, U64] → [U64] | Shift left, 64-bit |
| 0x65 | SHR_64 | — | [U64, U64] → [U64] | Shift right (logical), 64-bit |
| 0x66 | ROL_64 | — | [U64, U64] → [U64] | Rotate left, 64-bit |
| 0x67 | ROR_64 | — | [U64, U64] → [U64] | Rotate right, 64-bit |

---

### Comparison

Comparison instructions pop two values and push an I32 (0 or 1) result. Separate opcodes for signed, unsigned, and float comparisons because the hardware operations differ (signed vs unsigned comparison, IEEE 754 float comparison with NaN handling).

#### Signed Integer Comparison (32-bit)

| # | Opcode | Operands | Stack effect | Description |
|---|--------|----------|-------------|-------------|
| 0x68 | EQ_I32 | — | [I32, I32] → [I32] | Equal |
| 0x69 | NE_I32 | — | [I32, I32] → [I32] | Not equal |
| 0x6A | LT_I32 | — | [I32, I32] → [I32] | Less than (signed) |
| 0x6B | LE_I32 | — | [I32, I32] → [I32] | Less than or equal (signed) |
| 0x6C | GT_I32 | — | [I32, I32] → [I32] | Greater than (signed) |
| 0x6D | GE_I32 | — | [I32, I32] → [I32] | Greater than or equal (signed) |

#### Unsigned Integer Comparison (32-bit)

| # | Opcode | Operands | Stack effect | Description |
|---|--------|----------|-------------|-------------|
| 0x6E | EQ_U32 | — | [U32, U32] → [I32] | Equal |
| 0x6F | NE_U32 | — | [U32, U32] → [I32] | Not equal |
| 0x70 | LT_U32 | — | [U32, U32] → [I32] | Less than (unsigned) |
| 0x71 | LE_U32 | — | [U32, U32] → [I32] | Less than or equal (unsigned) |
| 0x72 | GT_U32 | — | [U32, U32] → [I32] | Greater than (unsigned) |
| 0x73 | GE_U32 | — | [U32, U32] → [I32] | Greater than or equal (unsigned) |

#### 64-bit Comparison (signed and unsigned)

| # | Opcode | Operands | Stack effect | Description |
|---|--------|----------|-------------|-------------|
| 0x74 | EQ_I64 | — | [I64, I64] → [I32] | Equal (signed 64-bit) |
| 0x75 | NE_I64 | — | [I64, I64] → [I32] | Not equal (signed 64-bit) |
| 0x76 | LT_I64 | — | [I64, I64] → [I32] | Less than (signed 64-bit) |
| 0x77 | LE_I64 | — | [I64, I64] → [I32] | Less than or equal (signed 64-bit) |
| 0x78 | GT_I64 | — | [I64, I64] → [I32] | Greater than (signed 64-bit) |
| 0x79 | GE_I64 | — | [I64, I64] → [I32] | Greater than or equal (signed 64-bit) |
| 0x7A | EQ_U64 | — | [U64, U64] → [I32] | Equal (unsigned 64-bit) |
| 0x7B | NE_U64 | — | [U64, U64] → [I32] | Not equal (unsigned 64-bit) |
| 0x7C | LT_U64 | — | [U64, U64] → [I32] | Less than (unsigned 64-bit) |
| 0x7D | LE_U64 | — | [U64, U64] → [I32] | Less than or equal (unsigned 64-bit) |
| 0x7E | GT_U64 | — | [U64, U64] → [I32] | Greater than (unsigned 64-bit) |
| 0x7F | GE_U64 | — | [U64, U64] → [I32] | Greater than or equal (unsigned 64-bit) |

#### Float Comparison

| # | Opcode | Operands | Stack effect | Description |
|---|--------|----------|-------------|-------------|
| 0x80 | EQ_F32 | — | [F32, F32] → [I32] | Equal (NaN ≠ NaN → 0) |
| 0x81 | NE_F32 | — | [F32, F32] → [I32] | Not equal |
| 0x82 | LT_F32 | — | [F32, F32] → [I32] | Less than |
| 0x83 | LE_F32 | — | [F32, F32] → [I32] | Less than or equal |
| 0x84 | GT_F32 | — | [F32, F32] → [I32] | Greater than |
| 0x85 | GE_F32 | — | [F32, F32] → [I32] | Greater than or equal |
| 0x86 | EQ_F64 | — | [F64, F64] → [I32] | Equal (NaN ≠ NaN → 0) |
| 0x87 | NE_F64 | — | [F64, F64] → [I32] | Not equal |
| 0x88 | LT_F64 | — | [F64, F64] → [I32] | Less than |
| 0x89 | LE_F64 | — | [F64, F64] → [I32] | Less than or equal |
| 0x8A | GT_F64 | — | [F64, F64] → [I32] | Greater than |
| 0x8B | GE_F64 | — | [F64, F64] → [I32] | Greater than or equal |

---

### Type Conversion

#### Narrowing (with overflow policy per ADR-0002)

These instructions truncate a 32-bit value to a sub-32-bit width. The VM's configured overflow policy (wrap, saturate, fault) determines the behavior when the value exceeds the target range.

| # | Opcode | Operands | Stack effect | Description |
|---|--------|----------|-------------|-------------|
| 0x90 | NARROW_I8 | — | [I32] → [I32] | Narrow to SINT range (-128..127), result stays in I32 |
| 0x91 | NARROW_I16 | — | [I32] → [I32] | Narrow to INT range (-32768..32767), result stays in I32 |
| 0x92 | NARROW_U8 | — | [U32] → [U32] | Narrow to USINT range (0..255), result stays in U32 |
| 0x93 | NARROW_U16 | — | [U32] → [U32] | Narrow to UINT range (0..65535), result stays in U32 |

Note: the narrowed value remains at 32-bit width on the stack (since the VM always operates at 32-bit minimum). The NARROW instruction constrains the *value* to the target range, not the stack slot width.

#### Widening (lossless)

| # | Opcode | Operands | Stack effect | Description |
|---|--------|----------|-------------|-------------|
| 0x94 | WIDEN_I32_TO_I64 | — | [I32] → [I64] | Sign-extend I32 to I64 |
| 0x95 | WIDEN_U32_TO_U64 | — | [U32] → [U64] | Zero-extend U32 to U64 |
| 0x96 | WIDEN_F32_TO_F64 | — | [F32] → [F64] | Promote float to double |

#### Cross-Domain Conversion

| # | Opcode | Operands | Stack effect | Description |
|---|--------|----------|-------------|-------------|
| 0x98 | I32_TO_F32 | — | [I32] → [F32] | Signed integer to float |
| 0x99 | I32_TO_F64 | — | [I32] → [F64] | Signed integer to double |
| 0x9A | I64_TO_F64 | — | [I64] → [F64] | Signed long to double |
| 0x9B | U32_TO_F32 | — | [U32] → [F32] | Unsigned integer to float |
| 0x9C | U32_TO_F64 | — | [U32] → [F64] | Unsigned integer to double |
| 0x9D | U64_TO_F64 | — | [U64] → [F64] | Unsigned long to double |
| 0x9E | F32_TO_I32 | — | [F32] → [I32] | Float to signed integer (truncates toward zero) |
| 0x9F | F64_TO_I32 | — | [F64] → [I32] | Double to signed integer (truncates toward zero) |
| 0xA0 | F64_TO_I64 | — | [F64] → [I64] | Double to signed long (truncates toward zero) |
| 0xA1 | NARROW_I64_TO_I32 | — | [I64] → [I32] | Narrow 64-bit to 32-bit signed (with overflow policy) |
| 0xA2 | NARROW_U64_TO_U32 | — | [U64] → [U32] | Narrow 64-bit to 32-bit unsigned (with overflow policy) |
| 0xA3 | NARROW_F64_TO_F32 | — | [F64] → [F32] | Narrow double to float (IEEE 754 rounding) |

#### TIME Arithmetic

TIME values are I64 microseconds. Although raw I64 arithmetic produces correct results, dedicated TIME opcodes enforce type discipline: the VM can verify that only TIME-typed values are passed to these instructions, catching accidental mixing of TIME and unrelated integers. This prevents a class of unit-confusion bugs (e.g., accidentally adding a loop counter to a timestamp).

| # | Opcode | Operands | Stack effect | Description |
|---|--------|----------|-------------|-------------|
| 0xA4 | TIME_ADD | — | [I64, I64] → [I64] | Add two TIME/duration values (microseconds) |
| 0xA5 | TIME_SUB | — | [I64, I64] → [I64] | Subtract TIME/duration values (microseconds) |

---

### Control Flow

| # | Opcode | Operands | Stack effect | Description |
|---|--------|----------|-------------|-------------|
| 0xB0 | JMP | offset: i16 | [] → [] | Unconditional jump (relative to next instruction) |
| 0xB1 | JMP_IF | offset: i16 | [I32] → [] | Jump if top of stack is nonzero (TRUE) |
| 0xB2 | JMP_IF_NOT | offset: i16 | [I32] → [] | Jump if top of stack is zero (FALSE) |
| 0xB3 | CALL | index: u16 | [args...] → [result] | Call function by index; pushes return value |
| 0xB4 | RET | — | [result] → [] | Return from function; pops return value |
| 0xB5 | RET_VOID | — | [] → [] | Return from function with no return value |

---

### Function Block Operations

Function block invocation follows the pattern: load the FB instance reference, store input parameters, call the FB, load output parameters.

| # | Opcode | Operands | Stack effect | Description |
|---|--------|----------|-------------|-------------|
| 0xC0 | FB_LOAD_INSTANCE | index: u16 | [] → [fb_ref] | Push FB instance reference from variable table |
| 0xC1 | FB_STORE_PARAM | field: u8 | [value, fb_ref] → [fb_ref] | Store input parameter on FB instance; keeps fb_ref on stack |
| 0xC2 | FB_LOAD_PARAM | field: u8 | [fb_ref] → [value, fb_ref] | Load output parameter from FB instance; keeps fb_ref on stack |
| 0xC3 | FB_CALL | type_id: u16 | [fb_ref] → [] | Call function block (VM dispatches to intrinsic or bytecode body per ADR-0003) |

#### Calling Convention

A typical FB invocation compiles to:

```
(* Source: myTimer(IN := start, PT := T#5s); elapsed := myTimer.ET; *)

FB_LOAD_INSTANCE  0x0001      -- push myTimer instance ref
LOAD_VAR_I32      0x0002      -- push start variable
FB_STORE_PARAM    0            -- store to IN (field 0), ref stays on stack
LOAD_CONST_I64    0x0003      -- push T#5s as I64 microseconds
FB_STORE_PARAM    1            -- store to PT (field 1), ref stays on stack
FB_CALL           0x0010      -- call TON (type_id 0x0010); VM may use intrinsic
FB_LOAD_INSTANCE  0x0001      -- push myTimer instance ref again
FB_LOAD_PARAM     3            -- load ET (field 3)
STORE_VAR_I64     0x0004      -- store to elapsed variable
```

FB_STORE_PARAM and FB_LOAD_PARAM keep the instance reference on the stack to allow chaining multiple parameter operations without reloading the reference.

---

### Built-in Standard Library Functions

The BUILTIN opcode provides a single dispatch mechanism for all standard library functions (string operations, numeric functions, and future extensions). Rather than dedicating a separate opcode to each function, BUILTIN uses a u16 `func_id` operand to identify the target function. The VM dispatches to the appropriate native implementation based on the func_id. The verifier uses the func_id to determine the expected stack types (see Built-in Function Table below).

This approach parallels FB_CALL (ADR-0003): one opcode handles an extensible family of operations, with the operand identifying the specific operation. See ADR-0008 for the full rationale.

| # | Opcode | Operands | Stack effect | Description |
|---|--------|----------|-------------|-------------|
| 0xC4 | BUILTIN | func_id: u16 | [args...] → [result] | Call built-in function; stack effect depends on func_id |

#### Built-in Function Table

##### STRING Functions (func_id 0x0100–0x010A)

| func_id | Name | Stack effect | Description |
|---------|------|-------------|-------------|
| 0x0100 | STR_LEN | [buf_idx_str] → [I32] | String length (LEN) |
| 0x0101 | STR_CONCAT | [buf_idx_str, buf_idx_str] → [buf_idx_str] | Concatenate two strings (CONCAT) |
| 0x0102 | STR_LEFT | [buf_idx_str, I32] → [buf_idx_str] | Left substring (LEFT) |
| 0x0103 | STR_RIGHT | [buf_idx_str, I32] → [buf_idx_str] | Right substring (RIGHT) |
| 0x0104 | STR_MID | [buf_idx_str, I32, I32] → [buf_idx_str] | Mid substring (MID); position, length on stack |
| 0x0105 | STR_FIND | [buf_idx_str, buf_idx_str] → [I32] | Find substring position (FIND); 0 if not found |
| 0x0106 | STR_INSERT | [buf_idx_str, buf_idx_str, I32] → [buf_idx_str] | Insert string at position (INSERT) |
| 0x0107 | STR_DELETE | [buf_idx_str, I32, I32] → [buf_idx_str] | Delete characters (DELETE); position, length |
| 0x0108 | STR_REPLACE | [buf_idx_str, buf_idx_str, I32, I32] → [buf_idx_str] | Replace characters (REPLACE) |
| 0x0109 | STR_EQ | [buf_idx_str, buf_idx_str] → [I32] | String equality comparison |
| 0x010A | STR_LT | [buf_idx_str, buf_idx_str] → [I32] | String less-than (lexicographic) |

##### WSTRING Functions (func_id 0x0200–0x020A)

| func_id | Name | Stack effect | Description |
|---------|------|-------------|-------------|
| 0x0200 | WSTR_LEN | [buf_idx_wstr] → [I32] | Wide string length (LEN) |
| 0x0201 | WSTR_CONCAT | [buf_idx_wstr, buf_idx_wstr] → [buf_idx_wstr] | Concatenate two wide strings (CONCAT) |
| 0x0202 | WSTR_LEFT | [buf_idx_wstr, I32] → [buf_idx_wstr] | Left substring (LEFT) |
| 0x0203 | WSTR_RIGHT | [buf_idx_wstr, I32] → [buf_idx_wstr] | Right substring (RIGHT) |
| 0x0204 | WSTR_MID | [buf_idx_wstr, I32, I32] → [buf_idx_wstr] | Mid substring (MID); position, length on stack |
| 0x0205 | WSTR_FIND | [buf_idx_wstr, buf_idx_wstr] → [I32] | Find substring position (FIND); 0 if not found |
| 0x0206 | WSTR_INSERT | [buf_idx_wstr, buf_idx_wstr, I32] → [buf_idx_wstr] | Insert string at position (INSERT) |
| 0x0207 | WSTR_DELETE | [buf_idx_wstr, I32, I32] → [buf_idx_wstr] | Delete characters (DELETE); position, length |
| 0x0208 | WSTR_REPLACE | [buf_idx_wstr, buf_idx_wstr, I32, I32] → [buf_idx_wstr] | Replace characters (REPLACE) |
| 0x0209 | WSTR_EQ | [buf_idx_wstr, buf_idx_wstr] → [I32] | Wide string equality comparison |
| 0x020A | WSTR_LT | [buf_idx_wstr, buf_idx_wstr] → [I32] | Wide string less-than (lexicographic) |

##### Numeric Functions (func_id 0x0300–0x03FF)

Numeric functions are monomorphized by the compiler from generic IEC 61131-3 signatures (ANY_NUM, ANY_REAL) to type-specific func_ids. The compiler determines the concrete type from the arguments and emits the appropriate func_id.

| func_id | Name | Stack effect | Description |
|---------|------|-------------|-------------|
| 0x0300 | ABS_I32 | [I32] → [I32] | Absolute value (signed 32-bit) |
| 0x0301 | ABS_I64 | [I64] → [I64] | Absolute value (signed 64-bit) |
| 0x0302 | ABS_F32 | [F32] → [F32] | Absolute value (32-bit float) |
| 0x0303 | ABS_F64 | [F64] → [F64] | Absolute value (64-bit float) |
| 0x0310 | SQRT_F32 | [F32] → [F32] | Square root (32-bit float) |
| 0x0311 | SQRT_F64 | [F64] → [F64] | Square root (64-bit float) |
| 0x0320 | MIN_I32 | [I32, I32] → [I32] | Minimum (signed 32-bit) |
| 0x0321 | MIN_U32 | [U32, U32] → [U32] | Minimum (unsigned 32-bit) |
| 0x0322 | MIN_I64 | [I64, I64] → [I64] | Minimum (signed 64-bit) |
| 0x0323 | MIN_U64 | [U64, U64] → [U64] | Minimum (unsigned 64-bit) |
| 0x0324 | MIN_F32 | [F32, F32] → [F32] | Minimum (32-bit float) |
| 0x0325 | MIN_F64 | [F64, F64] → [F64] | Minimum (64-bit float) |
| 0x0330 | MAX_I32 | [I32, I32] → [I32] | Maximum (signed 32-bit) |
| 0x0331 | MAX_U32 | [U32, U32] → [U32] | Maximum (unsigned 32-bit) |
| 0x0332 | MAX_I64 | [I64, I64] → [I64] | Maximum (signed 64-bit) |
| 0x0333 | MAX_U64 | [U64, U64] → [U64] | Maximum (unsigned 64-bit) |
| 0x0334 | MAX_F32 | [F32, F32] → [F32] | Maximum (32-bit float) |
| 0x0335 | MAX_F64 | [F64, F64] → [F64] | Maximum (64-bit float) |
| 0x0340 | LIMIT_I32 | [I32, I32, I32] → [I32] | Clamp to range (signed 32-bit): MN, IN, MX |
| 0x0341 | LIMIT_U32 | [U32, U32, U32] → [U32] | Clamp to range (unsigned 32-bit): MN, IN, MX |
| 0x0342 | LIMIT_I64 | [I64, I64, I64] → [I64] | Clamp to range (signed 64-bit): MN, IN, MX |
| 0x0343 | LIMIT_U64 | [U64, U64, U64] → [U64] | Clamp to range (unsigned 64-bit): MN, IN, MX |
| 0x0344 | LIMIT_F32 | [F32, F32, F32] → [F32] | Clamp to range (32-bit float): MN, IN, MX |
| 0x0345 | LIMIT_F64 | [F64, F64, F64] → [F64] | Clamp to range (64-bit float): MN, IN, MX |

#### Built-in Function ID Ranges

| Range | Category | Description |
|-------|----------|-------------|
| 0x0000–0x00FF | Reserved | Future use |
| 0x0100–0x01FF | STRING functions | String operations on single-byte STRING buffers |
| 0x0200–0x02FF | WSTRING functions | String operations on wide-character WSTRING buffers |
| 0x0300–0x03FF | Numeric functions | ABS, SQRT, MIN, MAX, LIMIT (type-specific variants) |
| 0x0400–0xFFFF | Reserved | Future standard library extensions (trigonometric, logarithmic, date/time, etc.) |

STRING and WSTRING functions are in separate func_id ranges to preserve the type safety property from ADR-0004: the verifier can distinguish STRING operations from WSTRING operations by func_id alone, without runtime type tags.

---

### Stack Operations

| # | Opcode | Operands | Stack effect | Description |
|---|--------|----------|-------------|-------------|
| 0xD0 | POP | — | [value] → [] | Discard top of stack |
| 0xD1 | DUP | — | [value] → [value, value] | Duplicate top of stack |
| 0xD2 | SWAP | — | [a, b] → [b, a] | Swap top two stack values |

---

### String Operations

IEC 61131-3 strings have a declared maximum length known at compile time (e.g., `STRING(20)` holds at most 20 characters). Strings are stored as fixed-size buffers — not heap-allocated — matching the behavior of PLC runtimes like CODESYS and TwinCAT. This ensures deterministic memory usage with no dynamic allocation during scan cycles.

The VM manages two kinds of string buffers:

- **Variable buffers** — each STRING/WSTRING variable has a fixed-size buffer in the variable table, sized per its declaration (e.g., 82 bytes for `STRING(80)`: 80 chars + current length + null terminator)
- **Temporary buffers** — a pre-allocated pool of fixed-size buffers used for intermediate results from string operations (e.g., the result of CONCAT before it is stored). The compiler determines the required pool size by analyzing maximum expression depth.

The `buf_idx` values on the operand stack are small indices (not pointers) into the buffer table. Stack operations like DUP and SWAP copy only the index, not the buffer contents. Actual buffer-to-buffer copies happen only at STR_STORE_VAR / WSTR_STORE_VAR (string assignment) and within string operation handlers.

STRING and WSTRING are statically distinguished throughout the instruction set (ADR-0004). Variable access uses separate opcodes (STR_LOAD_VAR vs WSTR_LOAD_VAR), and string functions use separate BUILTIN func_id ranges (0x0100 for STRING, 0x0200 for WSTRING). The VM asserts that STRING operations always receive single-byte buffers and WSTRING operations always receive wide-character buffers, trapping immediately on a mismatch rather than silently misinterpreting character data.

String operations are dispatched through the BUILTIN opcode (0xC4) with function-specific func_id values (ADR-0008). This keeps the instruction set compact while supporting the full IEC 61131-3 string function library and providing an extensible mechanism for future functions. String operations that produce a string result (CONCAT, LEFT, etc.) write into a temporary buffer and push its index. If the result exceeds the temporary buffer's max length, it is truncated — matching standard PLC string truncation semantics.

#### STRING Variable Access

String variable access uses dedicated opcodes because string assignment has different semantics from integer assignment — STR_STORE_VAR performs a buffer-to-buffer content copy (value semantics), not an index copy.

| # | Opcode | Operands | Stack effect | Description |
|---|--------|----------|-------------|-------------|
| 0xE0 | STR_LOAD_VAR | index: u16 | [] → [buf_idx] | Push buffer index for a STRING variable |
| 0xE1 | STR_STORE_VAR | index: u16 | [buf_idx] → [] | Copy buffer contents into STRING variable's buffer (value-copy assignment) |

#### WSTRING Variable Access

| # | Opcode | Operands | Stack effect | Description |
|---|--------|----------|-------------|-------------|
| 0xE2 | WSTR_LOAD_VAR | index: u16 | [] → [buf_idx] | Push buffer index for a WSTRING variable |
| 0xE3 | WSTR_STORE_VAR | index: u16 | [buf_idx] → [] | Copy buffer contents into WSTRING variable's buffer (value-copy assignment) |

#### String Functions

String functions (LEN, CONCAT, LEFT, RIGHT, MID, FIND, INSERT, DELETE, REPLACE, EQ, LT) for both STRING and WSTRING are dispatched through the BUILTIN opcode (0xC4) using func_id operands. See the Built-in Function Table in the Built-in Standard Library Functions section for the complete function ID assignments.

---

### Debug

| # | Opcode | Operands | Stack effect | Description |
|---|--------|----------|-------------|-------------|
| 0xFC | NOP | — | [] → [] | No operation |
| 0xFD | BREAKPOINT | — | [] → [] | Debug breakpoint (NOP in release mode) |
| 0xFE | LINE | line: u16 | [] → [] | Source line number marker for debugging |

---

## Opcode Summary

| Category | Range | Count | Description |
|----------|-------|-------|-------------|
| Load/Store Constants | 0x01–0x0A | 10 | Constant pool loads, boolean literals, string constants |
| Load/Store Variables | 0x10–0x1D | 12 | Typed variable access (numeric types only) |
| Process Image | 0x20–0x23 | 4 | I/O and memory access (%I, %Q, %M) |
| Array Access | 0x24–0x25 | 2 | Bounds-checked array element load/store |
| Struct/FB Fields | 0x28–0x29 | 2 | Field access on FB references |
| Integer Arithmetic 32 | 0x30–0x3A | 11 | I32 and U32 arithmetic |
| Integer Arithmetic 64 | 0x3C–0x46 | 11 | I64 and U64 arithmetic |
| Float Arithmetic | 0x48–0x51 | 10 | F32 and F64 arithmetic |
| Boolean | 0x54–0x57 | 4 | Logical AND/OR/XOR/NOT |
| Bitwise | 0x58–0x67 | 16 | Bitwise ops and shifts, 32 and 64-bit |
| Comparison | 0x68–0x8B | 36 | Typed comparisons across all VM types |
| Type Conversion | 0x90–0xA3 | 16 | Narrowing, widening, cross-domain |
| TIME Arithmetic | 0xA4–0xA5 | 2 | Type-checked TIME addition and subtraction |
| Control Flow | 0xB0–0xB5 | 6 | Jumps, calls, returns |
| Function Block | 0xC0–0xC3 | 4 | FB instance management and invocation |
| Built-in Functions | 0xC4 | 1 | BUILTIN dispatch for standard library functions (ADR-0008) |
| Stack | 0xD0–0xD2 | 3 | Stack manipulation |
| STRING Variable Access | 0xE0–0xE1 | 2 | STRING variable load/store |
| WSTRING Variable Access | 0xE2–0xE3 | 2 | WSTRING variable load/store |
| Debug | 0xFC–0xFE | 3 | NOP, breakpoint, line info |
| **Total** | | **157** | |

The opcode budget uses 157 of 256 slots (61%), leaving 99 slots for future extensions (e.g., OOP method dispatch, pointer/reference operations).

## Compilation Examples

### Simple Arithmetic with Narrowing

```
(* Source *)
VAR x : SINT; y : SINT; z : SINT; END_VAR
z := x + y;

(* Bytecode *)
LOAD_VAR_I32   0x0000    -- load x (SINT, sign-extended to I32)
LOAD_VAR_I32   0x0001    -- load y (SINT, sign-extended to I32)
ADD_I32                   -- I32 addition
NARROW_I8                 -- apply overflow policy, constrain to SINT range
STORE_VAR_I32  0x0002    -- store z
```

### IF/ELSE

```
(* Source *)
IF condition THEN
  x := 1;
ELSE
  x := 2;
END_IF;

(* Bytecode *)
LOAD_VAR_I32   0x0000    -- load condition (BOOL as I32)
JMP_IF_NOT     +8        -- jump to ELSE if false
LOAD_CONST_I32 0x0001    -- push 1
STORE_VAR_I32  0x0001    -- store x
JMP            +5        -- jump past ELSE
LOAD_CONST_I32 0x0002    -- push 2 (ELSE target)
STORE_VAR_I32  0x0001    -- store x
                          -- (end of IF)
```

### FOR Loop

```
(* Source *)
FOR i := 0 TO 9 DO
  sum := sum + i;
END_FOR;

(* Bytecode *)
LOAD_CONST_I32 0x0000    -- push 0
STORE_VAR_I32  0x0000    -- i := 0
                          -- loop_start:
LOAD_VAR_I32   0x0000    -- load i
LOAD_CONST_I32 0x0001    -- push 9
GT_I32                    -- i > 9?
JMP_IF         +12       -- exit loop if true
LOAD_VAR_I32   0x0001    -- load sum
LOAD_VAR_I32   0x0000    -- load i
ADD_I32                   -- sum + i
STORE_VAR_I32  0x0001    -- store sum
LOAD_VAR_I32   0x0000    -- load i
LOAD_CONST_I32 0x0002    -- push 1
ADD_I32                   -- i + 1
STORE_VAR_I32  0x0000    -- store i
JMP            -28       -- jump to loop_start
                          -- loop_exit:
```

### Function Block Call (Timer)

```
(* Source *)
myTimer(IN := startButton, PT := T#5s);
IF myTimer.Q THEN
  output := TRUE;
END_IF;

(* Bytecode *)
FB_LOAD_INSTANCE 0x0000  -- push myTimer ref
LOAD_VAR_I32     0x0001  -- push startButton
FB_STORE_PARAM   0        -- store IN parameter
LOAD_CONST_I64   0x0000  -- push 5000000 (5s in microseconds)
FB_STORE_PARAM   1        -- store PT parameter
FB_CALL          0x0010  -- invoke TON (intrinsic per ADR-0003)
FB_LOAD_INSTANCE 0x0000  -- push myTimer ref
FB_LOAD_PARAM    2        -- load Q output
JMP_IF_NOT       +4       -- skip if Q is FALSE
LOAD_TRUE                 -- push TRUE
STORE_VAR_I32    0x0002  -- store output
```

## Design Decisions

The following questions were resolved with a "prioritize safety" principle: when in doubt, prefer opcodes that encode type information and invariants statically, so the VM can enforce them, over clever encodings that save opcode space but rely on the compiler always getting things right.

1. **WSTRING vs STRING opcodes → Separate opcodes.** Polymorphic dispatch would require a runtime type-tag check on every string operation, and a bug in the tag (or a stale `buf_idx`) would silently misinterpret character data — UTF-16 as single-byte or vice versa. Separate STR_* and WSTR_* opcode families make the encoding type statically checkable: the compiler emits the correct family, and the VM traps immediately on a type mismatch. The cost is 13 additional opcodes (one WSTR_* per STR_*), well within the 256-opcode budget.

2. **Array access → Dedicated LOAD_ARRAY / STORE_ARRAY with mandatory bounds checking.** Compiling array access to pointer arithmetic (`base + index * size`) makes bounds checking optional and fragile — a compiler bug silently produces buffer overflows. A dedicated opcode makes bounds checking mandatory and atomic: the VM validates the index against the declared array size on every access and traps on out-of-bounds. Buffer overflows are the #1 class of safety bugs in embedded systems; this eliminates them by construction.

3. **CASE statement → No TABLE_SWITCH; keep JMP_IF chains.** A TABLE_SWITCH opcode requires the VM to validate that the jump table is well-formed (no out-of-range targets, no missing entries). A chain of CMP + JMP_IF comparisons is trivially verifiable — each jump target is individually validated. The performance difference is negligible for typical PLC CASE statements (5–20 arms). TABLE_SWITCH is a premature optimization that adds an opcode with a complex encoding and a new class of potential bugs.

4. **Exponentiation → Standard library call, not an opcode.** Exponentiation involves floating-point edge cases (0^0, negative base with fractional exponent, overflow). A library function can return explicit error indicators and be tested/audited independently. Baking it into the VM as an opcode fixes the error-handling semantics and makes them harder to inspect. Since EXPT is rare in PLC code, there is no performance argument for a dedicated opcode.

5. **String and numeric functions → Single BUILTIN opcode with func_id dispatch (ADR-0008).** String operations (LEN, CONCAT, LEFT, etc.) and numeric functions (ABS, SQRT, MIN, MAX, LIMIT) are standard library functions, not fundamental type operations. A single BUILTIN opcode with a u16 func_id operand handles all of them, freeing 21 opcode slots. The type safety properties from ADR-0004 are preserved because STRING and WSTRING functions have distinct func_id ranges, and the verifier checks type correctness per func_id. The BUILTIN pattern mirrors FB_CALL: one opcode dispatches to an extensible set of native implementations.

6. **TIME arithmetic → Dedicated TIME_ADD / TIME_SUB opcodes.** Raw I64 arithmetic on TIME values is numerically correct but semantically invisible to the VM. If a programmer accidentally adds a TIME and a DINT, raw I64 arithmetic silently produces a nonsensical result. Dedicated TIME opcodes let the VM enforce type discipline: TIME_ADD only accepts two TIME-typed operands (or TIME + duration). This catches unit-confusion bugs at runtime and makes bytecode verification easier — an auditor can confirm that time values are never mixed with unrelated integers.

## Arithmetic Edge Cases

The following behaviors are normative. The VM must implement these exactly to ensure deterministic, portable execution across all targets.

### Division by Zero

All integer division and modulo instructions trap on division by zero. The VM halts the current scan cycle and reports a runtime fault. This applies to:

- DIV_I32, DIV_U32, DIV_I64, DIV_U64
- MOD_I32, MOD_U32, MOD_I64, MOD_U64

Floating-point division by zero follows IEEE 754: `x / 0.0` produces `+Inf` or `-Inf` (depending on the sign of x), and `0.0 / 0.0` produces `NaN`. This applies to DIV_F32 and DIV_F64. The VM does not trap on floating-point division by zero.

### Signed Integer Overflow on Negation

NEG_I32 on `i32::MIN` (-2147483648) and NEG_I64 on `i64::MIN` produce a result governed by the configured overflow policy (ADR-0002):

| Policy | NEG_I32 on -2147483648 | NEG_I64 on i64::MIN |
|--------|------------------------|----------------------|
| Wrap | -2147483648 (wraps to itself) | i64::MIN (wraps to itself) |
| Saturate | 2147483647 (i32::MAX) | i64::MAX |
| Fault | Runtime trap | Runtime trap |

### Shift Amounts

Shift and rotate instructions mask the shift amount to the bit width of the operand:

| Instructions | Mask | Effect |
|---|---|---|
| SHL_32, SHR_32, ROL_32, ROR_32 | `amount & 31` | Shift amount 0–31 |
| SHL_64, SHR_64, ROL_64, ROR_64 | `amount & 63` | Shift amount 0–63 |

A shift by 32 on a 32-bit value produces the same result as a shift by 0. This matches Rust's `wrapping_shl` / `wrapping_shr` and ensures deterministic behavior across hardware platforms (ARM and x86 differ in their native shift behavior for out-of-range amounts).

### Float-to-Integer Overflow

When a floating-point value exceeds the range of the target integer type, the conversion instructions follow the configured overflow policy (ADR-0002):

| Instructions | Target range |
|---|---|
| F32_TO_I32, F64_TO_I32 | -2147483648 to 2147483647 |
| F64_TO_I64 | -9223372036854775808 to 9223372036854775807 |

| Policy | Value > max | Value < min | Value is NaN |
|--------|-------------|-------------|--------------|
| Wrap | Truncate to target width | Truncate to target width | 0 |
| Saturate | Target max | Target min | 0 |
| Fault | Runtime trap | Runtime trap | Runtime trap |

### Float Comparison with NaN

All float comparison instructions (EQ_F32, LT_F32, etc.) follow IEEE 754 semantics:

- `NaN == NaN` → 0 (false)
- `NaN != NaN` → 1 (true)
- `NaN < x` → 0 (false) for any x
- `NaN > x` → 0 (false) for any x
- `NaN <= x` → 0 (false) for any x
- `NaN >= x` → 0 (false) for any x

### Integer Overflow on Addition, Subtraction, Multiplication

For full-width arithmetic (ADD_I32 on I32, ADD_I64 on I64, etc.), the overflow policy from ADR-0002 applies. The compiler inserts NARROW instructions at assignment points for sub-width types (SINT, INT), but full-width overflow can occur in intermediate computations:

| Policy | Behavior |
|--------|----------|
| Wrap | Two's complement wrapping (Rust's `wrapping_add`, etc.) |
| Saturate | Clamp to type min/max |
| Fault | Runtime trap |

The overflow policy is a VM startup configuration, not a per-instruction setting.

## Known Limitations

The following are known limitations of this version of the instruction set. They are intentional trade-offs for the initial implementation and may be addressed in future versions.

1. **Jump offset range** — Jump offsets are i16 (range -32768..+32767 bytes from the next instruction). Functions whose bytecode exceeds ~32 KB cannot use jumps that span the entire body. This is sufficient for typical PLC programs. A future JMP_FAR with i32 offset could be added if needed.

2. **Array bounds** — Array descriptor bounds are i16 (range -32768..32767). Arrays with more than ~32K elements or arbitrary LINT-typed bounds cannot be represented. This is sufficient for typical PLC array usage.

3. **Field index** — LOAD_FIELD/STORE_FIELD use a u8 field index, limiting FB types to 255 fields. This is sufficient for all standard function blocks and typical user-defined FBs.

4. **No runtime service opcodes** — There is no CLOCK or SYSCALL instruction for bytecode to access runtime services (wall clock, hardware timers). Standard FBs that need timer access (TON, TOF, TP) are implemented as intrinsics (ADR-0003). User-defined FBs that extend standard timer FBs via EXTENDS will fall through to bytecode interpretation and will not have direct timer access. Such FBs should use composition (wrapping a standard timer instance) rather than inheritance to access timer functionality.

## Out of Scope for Version 1

The following PLC runtime features are **not addressed** by this instruction set and will require separate specifications:

1. **Multi-tasking** — IEC 61131-3 supports TASK configurations where multiple programs run at different priorities and intervals. This spec assumes single-threaded execution within a single scan cycle. Multi-task support (scheduling, shared memory, priority) will be a separate spec.

2. **Online change** — Updating the running program without stopping the PLC (hot-swapping bytecode while preserving variable state). This requires careful handling of variable persistence, bytecode replacement at scan cycle boundaries, and FB instance state migration.

3. **RETAIN / PERSISTENT variables** — IEC 61131-3 variables with RETAIN or PERSISTENT qualifiers survive power cycles (stored in non-volatile memory). The variable table currently has no flag for this. A future container format revision should add retention flags to VarEntry.

4. **User-defined types** — Enumerations and subrange types are compiled to their underlying integer types. Enumeration symbolic names are preserved only in the debug section. Subrange constraints are not enforced at runtime (they could be added as specialized NARROW instructions in a future version).

5. **Pointer / reference types** — IEC 61131-3 edition 3 introduces REFERENCE TO and pointer types. These are not supported in version 1. The opcode budget has reserved slots for future pointer operations.
