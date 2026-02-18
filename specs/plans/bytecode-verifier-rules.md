# Spec: Bytecode Verifier Rules

## Overview

This spec defines the rules that the IronPLC bytecode verifier must enforce. The verifier is a static analysis pass that runs at bytecode load time, before execution begins. It either accepts the bytecode (all rules pass) or rejects it with a specific error.

The verifier builds on:

- **[ADR-0006](../adrs/0006-bytecode-verification-requirement.md)**: Bytecode verification as a requirement
- **[Bytecode Instruction Set](bytecode-instruction-set.md)**: The instruction set being verified
- **[Bytecode Container Format](bytecode-container-format.md)**: The container providing type metadata

## Verification Model

The verifier performs **abstract interpretation** over the bytecode. For each instruction, it tracks:

- **Stack depth** — the number of values on the operand stack
- **Stack types** — the type of each value on the stack (I32, U32, I64, U64, F32, F64, buf_idx_str, buf_idx_wstr, fb_ref)
- **Local types** — the type of each local variable (set on first store, checked on subsequent loads)

The verifier processes instructions in order, following control flow. At branch merge points (jump targets, loop headers), it checks that all incoming paths agree on stack depth and types.

### Abstract Types

The verifier tracks nine abstract types on the stack:

| Abstract type | Produced by | Consumed by |
|---|---|---|
| I32 | LOAD_VAR_I32, LOAD_CONST_I32, LOAD_TRUE, LOAD_FALSE, comparisons, STR_LEN, WSTR_LEN, BOOL_*, NARROW_* | STORE_VAR_I32, JMP_IF, JMP_IF_NOT, BOOL_*, arithmetic I32, comparisons I32, NARROW_* |
| U32 | LOAD_VAR_U32, LOAD_CONST_U32, BIT_* 32 | STORE_VAR_U32, BIT_* 32, arithmetic U32, comparisons U32 |
| I64 | LOAD_VAR_I64, LOAD_CONST_I64, TIME_ADD, TIME_SUB | STORE_VAR_I64, arithmetic I64, comparisons I64, TIME_ADD, TIME_SUB |
| U64 | LOAD_VAR_U64, LOAD_CONST_U64, BIT_* 64 | STORE_VAR_U64, BIT_* 64, arithmetic U64, comparisons U64 |
| F32 | LOAD_VAR_F32, LOAD_CONST_F32 | STORE_VAR_F32, arithmetic F32, comparisons F32 |
| F64 | LOAD_VAR_F64, LOAD_CONST_F64 | STORE_VAR_F64, arithmetic F64, comparisons F64 |
| buf_idx_str | STR_LOAD_VAR, STR_CONCAT, STR_LEFT, STR_RIGHT, STR_MID, STR_INSERT, STR_DELETE, STR_REPLACE | STR_STORE_VAR, STR_LEN, STR_CONCAT, STR_LEFT, STR_RIGHT, STR_MID, STR_FIND, STR_INSERT, STR_DELETE, STR_REPLACE, STR_EQ, STR_LT |
| buf_idx_wstr | WSTR_LOAD_VAR, WSTR_CONCAT, WSTR_LEFT, WSTR_RIGHT, WSTR_MID, WSTR_INSERT, WSTR_DELETE, WSTR_REPLACE | WSTR_STORE_VAR, WSTR_LEN, WSTR_CONCAT, WSTR_LEFT, WSTR_RIGHT, WSTR_MID, WSTR_FIND, WSTR_INSERT, WSTR_DELETE, WSTR_REPLACE, WSTR_EQ, WSTR_LT |
| fb_ref | FB_LOAD_INSTANCE | FB_STORE_PARAM, FB_LOAD_PARAM, FB_CALL, LOAD_FIELD, STORE_FIELD |

Note: `buf_idx_str` and `buf_idx_wstr` are distinct verifier types even though both are represented as `buf_idx` on the operand stack at runtime. The verifier distinguishes them to prevent STRING/WSTRING cross-contamination.

## Rules

### Rule V-001: Valid Opcodes

Every byte at an instruction position must be a defined opcode. Undefined opcode bytes (0x00, 0x09, 0x0A–0x0F, 0x16, 0x17, 0x1E, 0x1F, 0x26, 0x27, 0x2A–0x2F, 0x3B, 0x47, 0x52, 0x53, 0xA6–0xAF, 0xB6–0xBF, 0xC4–0xCF, 0xD3–0xDF, 0xFA, 0xFB, 0xFF) must be rejected.

**Error**: `INVALID_OPCODE(offset, byte_value)`

### Rule V-002: Operand Bounds

Every operand that is an index into a table must be within bounds:

| Operand | Bound |
|---------|-------|
| LOAD_CONST_* index | < constant pool count |
| LOAD_VAR_* / STORE_VAR_* index | < variable table count |
| STR_LOAD_VAR / STR_STORE_VAR index | < variable table count AND variable type is STRING |
| WSTR_LOAD_VAR / WSTR_STORE_VAR index | < variable table count AND variable type is WSTRING |
| FB_LOAD_INSTANCE index | < variable table count AND variable type is FB_INSTANCE |
| LOAD_ARRAY / STORE_ARRAY array | < variable table count AND variable has array flag set |
| CALL function_id | < function count |
| FB_CALL type_id | < FB type count |
| FB_STORE_PARAM / FB_LOAD_PARAM field | < num_fields for the target FB type |
| LOAD_FIELD / STORE_FIELD field | < num_fields for the current fb_ref's type |

**Error**: `OPERAND_OUT_OF_BOUNDS(offset, operand_name, value, max)`

### Rule V-003: Constant Type Match

Every LOAD_CONST_* opcode must reference a constant pool entry whose type matches:

| Opcode | Required constant type |
|--------|----------------------|
| LOAD_CONST_I32 | I32 |
| LOAD_CONST_U32 | U32 |
| LOAD_CONST_I64 | I64 |
| LOAD_CONST_U64 | U64 |
| LOAD_CONST_F32 | F32 |
| LOAD_CONST_F64 | F64 |

**Error**: `CONSTANT_TYPE_MISMATCH(offset, expected_type, actual_type)`

### Rule V-004: Variable Type Match

Every LOAD_VAR_* / STORE_VAR_* opcode must reference a variable whose declared type matches:

| Opcode | Required variable type |
|--------|----------------------|
| LOAD_VAR_I32 / STORE_VAR_I32 | I32 (includes BOOL, SINT, INT, DINT) |
| LOAD_VAR_U32 / STORE_VAR_U32 | U32 (includes USINT, UINT, UDINT, BYTE, WORD, DWORD) |
| LOAD_VAR_I64 / STORE_VAR_I64 | I64 (includes LINT, TIME, DATE, TOD, DT) |
| LOAD_VAR_U64 / STORE_VAR_U64 | U64 (includes ULINT, LWORD) |
| LOAD_VAR_F32 / STORE_VAR_F32 | F32 |
| LOAD_VAR_F64 / STORE_VAR_F64 | F64 |

**Error**: `VARIABLE_TYPE_MISMATCH(offset, opcode, expected_type, actual_type)`

### Rule V-005: Stack Depth Consistency at Merge Points

At every instruction that is the target of a forward jump, backward jump, or fall-through from a conditional branch, the stack depth must be the same on all incoming paths.

The verifier identifies merge points by scanning for all jump targets before the main analysis pass.

**Error**: `STACK_DEPTH_MISMATCH(offset, depth_path_a, depth_path_b)`

### Rule V-006: Stack Type Consistency at Merge Points

At every merge point, the type of each stack slot must be the same on all incoming paths. Types are not compatible across categories — I32 and U32 are distinct, buf_idx_str and buf_idx_wstr are distinct.

**Error**: `STACK_TYPE_MISMATCH(offset, slot, type_path_a, type_path_b)`

### Rule V-007: No Stack Underflow

No instruction may pop from an empty stack. The verifier checks that the stack depth before each instruction is >= the number of values the instruction consumes.

**Error**: `STACK_UNDERFLOW(offset, opcode, required_depth, actual_depth)`

### Rule V-008: No Stack Overflow

The stack depth must never exceed the function's declared `max_stack_depth` (from the code section's function directory). The verifier tracks the maximum depth reached on all paths and checks it against the declared maximum.

**Error**: `STACK_OVERFLOW(offset, opcode, depth_after, declared_max)`

### Rule V-009: Jump Target Validity

Every JMP, JMP_IF, and JMP_IF_NOT offset must satisfy:

1. The computed target `(current_pc + operand_size + offset)` must be >= 0 and < the function's bytecode length
2. The target must land on a valid instruction boundary (the first byte of an instruction, not in the middle of an operand)

The verifier builds an instruction boundary map in the first pass and checks all jump targets against it.

**Error**: `INVALID_JUMP_TARGET(offset, target_offset, reason)` where reason is "out_of_bounds" or "mid_operand"

### Rule V-010: Stack Type Correctness

Every instruction must find the correct types on the stack. The verifier checks the top-of-stack type(s) against the instruction's expected input types as defined in the instruction set spec.

Examples:
- ADD_I32 requires [I32, I32] on top of stack
- STR_CONCAT requires [buf_idx_str, buf_idx_str]
- FB_STORE_PARAM requires [any_value, fb_ref] (with fb_ref on top, value below)
- BOOL_AND requires [I32, I32]

**Error**: `TYPE_ERROR(offset, opcode, expected_types, actual_types)`

### Rule V-011: Return Path Completeness

Every code path through a function must end in RET, RET_VOID, or an unconditional backward jump (loop). A path that falls off the end of the bytecode without a return instruction is rejected.

The verifier tracks reachability: after processing all instructions, every reachable instruction must either be a terminator (RET, RET_VOID) or have a successor instruction that is also reachable.

**Error**: `MISSING_RETURN(function_id, offset)` — the offset is the last reachable instruction that has no successor and is not a return.

### Rule V-012: Call Depth Bound

The verifier constructs a static call graph from CALL and FB_CALL instructions. The maximum depth of the call graph must not exceed the header's `max_call_depth`.

If the call graph contains cycles (recursion), the function is rejected. IEC 61131-3 does not permit recursion in standard PLC programs.

**Error**: `CALL_DEPTH_EXCEEDED(function_id, depth, max_depth)` or `RECURSIVE_CALL(function_id, cycle_path)`

### Rule V-013: Array Type Consistency

Every LOAD_ARRAY and STORE_ARRAY instruction's `type` byte must match the declared element type of the array variable referenced by the `array` operand.

| Opcode type byte | Required array element type |
|---|---|
| 0 (I32) | I32 |
| 1 (U32) | U32 |
| 2 (I64) | I64 |
| 3 (U64) | U64 |
| 4 (F32) | F32 |
| 5 (F64) | F64 |
| 6 (buf_idx STRING) | STRING |
| 7 (buf_idx WSTRING) | WSTRING |
| 8 (fb_ref) | FB_INSTANCE |

**Error**: `ARRAY_TYPE_MISMATCH(offset, declared_element_type, opcode_type_byte)`

### Rule V-014: Process Image Region Validity

Every LOAD_INPUT, STORE_OUTPUT, LOAD_MEMORY, and STORE_MEMORY instruction's `region` byte must be a valid access width (0–4):

| Value | Width | Meaning |
|-------|-------|---------|
| 0 | Bit | %IX, %QX, %MX |
| 1 | Byte | %IB, %QB, %MB |
| 2 | Word | %IW, %QW, %MW |
| 3 | Doubleword | %ID, %QD, %MD |
| 4 | Longword | %IL, %QL, %ML |

Values >= 5 are rejected.

**Error**: `INVALID_REGION(offset, region_value)`

### Rule V-015: FB_STORE_PARAM / FB_LOAD_PARAM Require Active FB Reference

FB_STORE_PARAM and FB_LOAD_PARAM must only appear after an FB_LOAD_INSTANCE (or after another FB_STORE_PARAM/FB_LOAD_PARAM that preserved the fb_ref on the stack). The verifier checks that the top of stack (for FB_LOAD_PARAM) or second-from-top (for FB_STORE_PARAM) is fb_ref.

**Error**: `FB_REF_REQUIRED(offset, opcode, actual_type)`

### Rule V-016: TIME Opcode Type Enforcement

TIME_ADD and TIME_SUB require two I64 values on the stack. The verifier further checks (via variable type metadata) that the source values are TIME-typed variables, not arbitrary I64 values.

To enforce this, the verifier tracks a sub-type for I64 stack slots: `I64_integer` (from LOAD_VAR_I64 of a LINT variable) vs `I64_time` (from LOAD_VAR_I64 of a TIME/DATE/TOD/DT variable, or from TIME_ADD/TIME_SUB output). TIME_ADD and TIME_SUB require `I64_time` inputs.

**Error**: `TIME_TYPE_MISMATCH(offset, opcode, expected_subtype, actual_subtype)`

## Verification Algorithm

The verifier uses a worklist-based approach:

```
function verify(bytecode, metadata):
    // Phase 1: Build instruction boundary map
    boundaries = scan_instruction_boundaries(bytecode)

    // Phase 2: Abstract interpretation
    worklist = {entry_point}
    state_at = {}  // maps instruction offset → abstract state (stack depth + types)
    state_at[entry_point] = initial_state(metadata)

    while worklist is not empty:
        offset = worklist.pop()
        state = state_at[offset]
        instruction = decode(bytecode, offset)

        // Check rules V-001 through V-016 for this instruction
        check_all_rules(instruction, state, metadata)

        // Compute successor state(s)
        for (successor_offset, successor_state) in successors(instruction, state):
            if successor_offset not in state_at:
                state_at[successor_offset] = successor_state
                worklist.add(successor_offset)
            else:
                // Merge point: check V-005 and V-006
                existing = state_at[successor_offset]
                if existing.depth != successor_state.depth:
                    error(V-005)
                if existing.types != successor_state.types:
                    error(V-006)
                // States match — no need to re-process

    // Phase 3: Check V-011 (return path completeness)
    check_return_paths(bytecode, state_at, boundaries)

    // Phase 4: Check V-012 (call depth)
    check_call_graph(metadata)

    return PASS
```

### Complexity

The verifier visits each instruction at most once per incoming control flow path. For acyclic functions, this is O(n) where n is the number of instructions. For functions with loops, merge-point checking ensures each instruction is processed at most once after the state stabilizes.

The memory cost is one abstract state per merge point. Each abstract state contains the stack depth (u16) and one type tag (u8) per stack slot. For a function with max stack depth 32 and 20 merge points, this is ~20 × 33 = 660 bytes.

## Error Reporting

The verifier produces structured error messages that include:

1. The rule ID (V-001 through V-016)
2. The bytecode offset of the offending instruction
3. The specific values that caused the failure (expected vs. actual types, bounds, depths)

If the debug section is available, the verifier maps the bytecode offset to a source line number using the line map.

Multiple errors may be reported in a single verification pass (the verifier does not stop at the first error). However, after the first error, subsequent errors may be spurious due to cascading effects. The first error is always accurate.
