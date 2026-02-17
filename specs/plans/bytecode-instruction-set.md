# Spec: Virtual PLC Bytecode Instruction Set

## Overview

This spec defines the bytecode instruction set for the IronPLC virtual PLC runtime. The instruction set is designed for a stack-based virtual machine that executes IEC 61131-3 programs compiled from Structured Text (and potentially other IEC 61131-3 languages).

The instruction set builds on four design decisions documented as ADRs:

0. **[ADR-0000](../adrs/0000-stack-based-bytecode-vm.md)**: Stack-based bytecode VM as the execution model — chosen over register-based VM, native compilation, tree-walking interpretation, and C transpilation
1. **[ADR-0001](../adrs/0001-bytecode-integer-arithmetic-type-strategy.md)**: Two-width integer arithmetic with explicit narrowing — sub-32-bit types are promoted to 32-bit on load; 64-bit types remain at 64-bit; explicit NARROW instructions handle truncation back to narrow types
2. **[ADR-0002](../adrs/0002-bytecode-overflow-behavior.md)**: Configurable overflow behavior at narrowing points — the VM supports wrap, saturate, and fault modes as a startup configuration
3. **[ADR-0003](../adrs/0003-plc-standard-function-blocks-as-intrinsics.md)**: Standard function blocks as VM intrinsics via FB_CALL — timers, counters, and other standard FBs use the same FB_CALL instruction as user-defined FBs, with the VM fast-pathing known types

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
| STRING | ref | Reference to heap-allocated string; see String Operations |
| WSTRING | ref | Reference to heap-allocated wide string; see String Operations |

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
| 0x16 | LOAD_VAR_REF | index: u16 | [] → [ref] | Load reference variable (STRING, WSTRING, FB instance) |
| 0x18 | STORE_VAR_I32 | index: u16 | [I32] → [] | Store to 32-bit signed variable |
| 0x19 | STORE_VAR_U32 | index: u16 | [U32] → [] | Store to 32-bit unsigned variable |
| 0x1A | STORE_VAR_I64 | index: u16 | [I64] → [] | Store to 64-bit signed variable |
| 0x1B | STORE_VAR_U64 | index: u16 | [U64] → [] | Store to 64-bit unsigned variable |
| 0x1C | STORE_VAR_F32 | index: u16 | [F32] → [] | Store to 32-bit float variable |
| 0x1D | STORE_VAR_F64 | index: u16 | [F64] → [] | Store to 64-bit float variable |
| 0x1E | STORE_VAR_REF | index: u16 | [ref] → [] | Store reference variable |

#### Process Image (I/O)

Process image instructions access the PLC's input and output memory. Inputs are frozen at the start of each scan cycle; outputs are flushed at the end.

| # | Opcode | Operands | Stack effect | Description |
|---|--------|----------|-------------|-------------|
| 0x20 | LOAD_INPUT | region: u8, index: u16 | [] → [value] | Read from input process image (%I) |
| 0x21 | STORE_OUTPUT | region: u8, index: u16 | [value] → [] | Write to output process image (%Q) |
| 0x22 | LOAD_MEMORY | region: u8, index: u16 | [] → [value] | Read from memory region (%M) |
| 0x23 | STORE_MEMORY | region: u8, index: u16 | [value] → [] | Write to memory region (%M) |

The `region` byte encodes the access width: 0=bit (X), 1=byte (B), 2=word (W), 3=doubleword (D), 4=longword (L).

#### Struct and FB Fields

| # | Opcode | Operands | Stack effect | Description |
|---|--------|----------|-------------|-------------|
| 0x28 | LOAD_FIELD | field: u8 | [ref] → [value] | Load field from struct/FB instance on stack |
| 0x29 | STORE_FIELD | field: u8 | [value, ref] → [] | Store field to struct/FB instance on stack |

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
| 0xC0 | FB_LOAD_INSTANCE | index: u16 | [] → [ref] | Push FB instance reference from variable table |
| 0xC1 | FB_STORE_PARAM | field: u8 | [value, ref] → [ref] | Store input parameter on FB instance; keeps ref on stack |
| 0xC2 | FB_LOAD_PARAM | field: u8 | [ref] → [value, ref] | Load output parameter from FB instance; keeps ref on stack |
| 0xC3 | FB_CALL | type_id: u16 | [ref] → [] | Call function block (VM dispatches to intrinsic or bytecode body per ADR-0003) |

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

### Stack Operations

| # | Opcode | Operands | Stack effect | Description |
|---|--------|----------|-------------|-------------|
| 0xD0 | POP | — | [value] → [] | Discard top of stack |
| 0xD1 | DUP | — | [value] → [value, value] | Duplicate top of stack |
| 0xD2 | SWAP | — | [a, b] → [b, a] | Swap top two stack values |

---

### String Operations

String operations use reference values and are implemented as VM built-in functions rather than inline bytecode. This keeps the instruction set compact while supporting the full IEC 61131-3 string function library.

| # | Opcode | Operands | Stack effect | Description |
|---|--------|----------|-------------|-------------|
| 0xE0 | STR_LEN | — | [ref] → [I32] | String length (LEN) |
| 0xE1 | STR_CONCAT | — | [ref, ref] → [ref] | Concatenate two strings (CONCAT) |
| 0xE2 | STR_LEFT | — | [ref, I32] → [ref] | Left substring (LEFT) |
| 0xE3 | STR_RIGHT | — | [ref, I32] → [ref] | Right substring (RIGHT) |
| 0xE4 | STR_MID | — | [ref, I32, I32] → [ref] | Mid substring (MID); position, length on stack |
| 0xE5 | STR_FIND | — | [ref, ref] → [I32] | Find substring position (FIND); 0 if not found |
| 0xE6 | STR_INSERT | — | [ref, ref, I32] → [ref] | Insert string at position (INSERT) |
| 0xE7 | STR_DELETE | — | [ref, I32, I32] → [ref] | Delete characters (DELETE); position, length |
| 0xE8 | STR_REPLACE | — | [ref, ref, I32, I32] → [ref] | Replace characters (REPLACE) |
| 0xE9 | STR_EQ | — | [ref, ref] → [I32] | String equality comparison |
| 0xEA | STR_LT | — | [ref, ref] → [I32] | String less-than (lexicographic) |

---

### Debug

| # | Opcode | Operands | Stack effect | Description |
|---|--------|----------|-------------|-------------|
| 0xF0 | NOP | — | [] → [] | No operation |
| 0xF1 | BREAKPOINT | — | [] → [] | Debug breakpoint (NOP in release mode) |
| 0xF2 | LINE | line: u16 | [] → [] | Source line number marker for debugging |

---

## Opcode Summary

| Category | Range | Count | Description |
|----------|-------|-------|-------------|
| Load/Store Constants | 0x01–0x08 | 8 | Constant pool loads, boolean literals |
| Load/Store Variables | 0x10–0x1E | 14 | Typed variable access |
| Process Image | 0x20–0x23 | 4 | I/O and memory access (%I, %Q, %M) |
| Struct/FB Fields | 0x28–0x29 | 2 | Field access on references |
| Integer Arithmetic 32 | 0x30–0x3A | 11 | I32 and U32 arithmetic |
| Integer Arithmetic 64 | 0x3C–0x46 | 11 | I64 and U64 arithmetic |
| Float Arithmetic | 0x48–0x51 | 10 | F32 and F64 arithmetic |
| Boolean | 0x54–0x57 | 4 | Logical AND/OR/XOR/NOT |
| Bitwise | 0x58–0x67 | 16 | Bitwise ops and shifts, 32 and 64-bit |
| Comparison | 0x68–0x8B | 36 | Typed comparisons across all VM types |
| Type Conversion | 0x90–0xA3 | 16 | Narrowing, widening, cross-domain |
| Control Flow | 0xB0–0xB5 | 6 | Jumps, calls, returns |
| Function Block | 0xC0–0xC3 | 4 | FB instance management and invocation |
| Stack | 0xD0–0xD2 | 3 | Stack manipulation |
| String | 0xE0–0xEA | 11 | String operations |
| Debug | 0xF0–0xF2 | 3 | NOP, breakpoint, line info |
| **Total** | | **159** | |

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

## Open Questions

1. **WSTRING vs STRING opcodes**: Should wide string operations be separate opcodes, or should the string opcodes be polymorphic over STRING and WSTRING (distinguished by the reference type at runtime)?

2. **Array access**: This spec does not include array indexing opcodes. Arrays could be handled as computed offsets using arithmetic (base + index * element_size) with LOAD_FIELD/STORE_FIELD, or dedicated LOAD_ARRAY/STORE_ARRAY opcodes with bounds checking could be added.

3. **CASE statement**: Currently compiled as a chain of JMP_IF comparisons. A TABLE_SWITCH opcode (like the JVM's `tableswitch`) could optimize dense CASE statements, but may not be worth the complexity for typical PLC programs.

4. **Exponentiation**: IEC 61131-3 defines the EXPT function. This could be a dedicated opcode or a standard library call. The performance requirement is unclear — exponentiation is rare in PLC programs.

5. **TIME arithmetic**: TIME values are represented as I64 microseconds and can use I64 arithmetic directly. Should there be dedicated TIME_ADD / TIME_SUB opcodes for clarity, or is I64 arithmetic sufficient?
