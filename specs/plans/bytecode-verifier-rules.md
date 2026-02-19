# Spec: Bytecode Verifier Rules

## Overview

This spec defines the rules that the IronPLC bytecode verifier must enforce. The verifier is a static analysis pass that runs at bytecode load time, before execution begins. It either accepts the bytecode (all rules pass) or rejects it with a specific error.

The verifier builds on:

- **[ADR-0006](../adrs/0006-bytecode-verification-requirement.md)**: Bytecode verification as a requirement
- **[Bytecode Instruction Set](bytecode-instruction-set.md)**: The instruction set being verified
- **[Bytecode Container Format](bytecode-container-format.md)**: The container providing type metadata

## Error Code Scheme

All verifier errors use codes in the format `R####` (R0001 through R9999). Codes are grouped by category:

| Range | Category |
|-------|----------|
| R0001–R0099 | Structural validity (opcodes, operands, instruction boundaries) |
| R0100–R0199 | Type metadata consistency (constants, variables, arrays) |
| R0200–R0299 | Stack discipline (depth, underflow, overflow) |
| R0300–R0399 | Stack type correctness |
| R0400–R0499 | Control flow (jumps, returns, call depth) |
| R0500–R0599 | Function block protocol |
| R0600–R0699 | Domain-specific type enforcement (TIME, process image) |
| R0700–R9999 | Reserved for future rules |

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
| buf_idx_str | STR_LOAD_VAR, LOAD_CONST_STR, STR_CONCAT, STR_LEFT, STR_RIGHT, STR_MID, STR_INSERT, STR_DELETE, STR_REPLACE | STR_STORE_VAR, STR_LEN, STR_CONCAT, STR_LEFT, STR_RIGHT, STR_MID, STR_FIND, STR_INSERT, STR_DELETE, STR_REPLACE, STR_EQ, STR_LT |
| buf_idx_wstr | WSTR_LOAD_VAR, LOAD_CONST_WSTR, WSTR_CONCAT, WSTR_LEFT, WSTR_RIGHT, WSTR_MID, WSTR_INSERT, WSTR_DELETE, WSTR_REPLACE | WSTR_STORE_VAR, WSTR_LEN, WSTR_CONCAT, WSTR_LEFT, WSTR_RIGHT, WSTR_MID, WSTR_FIND, WSTR_INSERT, WSTR_DELETE, WSTR_REPLACE, WSTR_EQ, WSTR_LT |
| fb_ref | FB_LOAD_INSTANCE | FB_STORE_PARAM, FB_LOAD_PARAM, FB_CALL, LOAD_FIELD, STORE_FIELD |

Note: `buf_idx_str` and `buf_idx_wstr` are distinct verifier types even though both are represented as `buf_idx` on the operand stack at runtime. The verifier distinguishes them to prevent STRING/WSTRING cross-contamination.

## Rules

### Rule R0001: Valid Opcodes

Every byte at an instruction position must be a defined opcode. Undefined opcode bytes (0x00, 0x0B–0x0F, 0x16, 0x17, 0x1E, 0x1F, 0x26, 0x27, 0x2A–0x2F, 0x3B, 0x47, 0x52, 0x53, 0xA6–0xAF, 0xB6–0xBF, 0xC4–0xCF, 0xD3–0xDF, 0xFA, 0xFB, 0xFF) must be rejected.

**Error**: `R0001(offset, byte_value)`

### Rule R0002: Operand Bounds

Every operand that is an index into a table must be within bounds:

| Operand | Bound |
|---------|-------|
| LOAD_CONST_* / LOAD_CONST_STR / LOAD_CONST_WSTR index | < constant pool count |
| LOAD_VAR_* / STORE_VAR_* index | < variable table count |
| STR_LOAD_VAR / STR_STORE_VAR index | < variable table count AND variable type is STRING |
| WSTR_LOAD_VAR / WSTR_STORE_VAR index | < variable table count AND variable type is WSTRING |
| FB_LOAD_INSTANCE index | < variable table count AND variable type is FB_INSTANCE |
| LOAD_ARRAY / STORE_ARRAY array | < variable table count AND variable has array flag set |
| CALL function_id | < function count |
| FB_CALL type_id | < FB type count |
| FB_STORE_PARAM / FB_LOAD_PARAM field | < num_fields for the target FB type |
| LOAD_FIELD / STORE_FIELD field | < num_fields for the current fb_ref's type |

**Error**: `R0002(offset, operand_name, value, max)`

### Rule R0100: Constant Type Match

Every LOAD_CONST_* opcode must reference a constant pool entry whose type matches:

| Opcode | Required constant type |
|--------|----------------------|
| LOAD_CONST_I32 | I32 |
| LOAD_CONST_U32 | U32 |
| LOAD_CONST_I64 | I64 |
| LOAD_CONST_U64 | U64 |
| LOAD_CONST_F32 | F32 |
| LOAD_CONST_F64 | F64 |
| LOAD_CONST_STR | STRING_LITERAL |
| LOAD_CONST_WSTR | WSTRING_LITERAL |

**Error**: `R0100(offset, expected_type, actual_type)`

### Rule R0101: Variable Type Match

Every LOAD_VAR_* / STORE_VAR_* opcode must reference a variable whose declared type matches:

| Opcode | Required variable type |
|--------|----------------------|
| LOAD_VAR_I32 / STORE_VAR_I32 | I32 (includes BOOL, SINT, INT, DINT) |
| LOAD_VAR_U32 / STORE_VAR_U32 | U32 (includes USINT, UINT, UDINT, BYTE, WORD, DWORD) |
| LOAD_VAR_I64 / STORE_VAR_I64 | I64 (includes LINT, TIME, DATE, TOD, DT) |
| LOAD_VAR_U64 / STORE_VAR_U64 | U64 (includes ULINT, LWORD) |
| LOAD_VAR_F32 / STORE_VAR_F32 | F32 |
| LOAD_VAR_F64 / STORE_VAR_F64 | F64 |

**Error**: `R0101(offset, opcode, expected_type, actual_type)`

### Rule R0102: Array Type Consistency

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

**Error**: `R0102(offset, declared_element_type, opcode_type_byte)`

### Rule R0200: Stack Depth Consistency at Merge Points

At every instruction that is the target of a forward jump, backward jump, or fall-through from a conditional branch, the stack depth must be the same on all incoming paths.

The verifier identifies merge points by scanning for all jump targets before the main analysis pass.

**Error**: `R0200(offset, depth_path_a, depth_path_b)`

### Rule R0201: Stack Type Consistency at Merge Points

At every merge point, the type of each stack slot must be the same on all incoming paths. Types are not compatible across categories — I32 and U32 are distinct, buf_idx_str and buf_idx_wstr are distinct.

**Error**: `R0201(offset, slot, type_path_a, type_path_b)`

### Rule R0202: No Stack Underflow

No instruction may pop from an empty stack. The verifier checks that the stack depth before each instruction is >= the number of values the instruction consumes.

**Error**: `R0202(offset, opcode, required_depth, actual_depth)`

### Rule R0203: No Stack Overflow

The stack depth must never exceed the function's declared `max_stack_depth` (from the code section's function directory). The verifier tracks the maximum depth reached on all paths and checks it against the declared maximum.

**Error**: `R0203(offset, opcode, depth_after, declared_max)`

### Rule R0300: Stack Type Correctness

Every instruction must find the correct types on the stack. The verifier checks the top-of-stack type(s) against the instruction's expected input types as defined in the instruction set spec.

Examples:
- ADD_I32 requires [I32, I32] on top of stack
- STR_CONCAT requires [buf_idx_str, buf_idx_str]
- FB_STORE_PARAM requires [any_value, fb_ref] (with fb_ref on top, value below)
- BOOL_AND requires [I32, I32]

**Error**: `R0300(offset, opcode, expected_types, actual_types)`

### Rule R0301: Function Call Parameter Type Correctness

Every CALL instruction must have the correct number and types of arguments on the stack, matching the target function's signature from the type section. The verifier checks:

1. The stack depth is >= the function's `num_params`
2. Each argument type matches the declared `param_types` in the function signature (bottom-to-top on the stack matches left-to-right in the signature)
3. The instruction's successor state has the function's `return_type` pushed (or nothing for void)

**Error**: `R0301(offset, function_id, param_index, expected_type, actual_type)`

### Rule R0302: Field Access Type Correctness

Every LOAD_FIELD and STORE_FIELD instruction must access a field whose type matches the value on the stack:

- For STORE_FIELD: the value being stored must match the field's declared `field_type` from the FB type descriptor
- For LOAD_FIELD: the pushed value type is determined by the field's declared `field_type`

Similarly, FB_STORE_PARAM and FB_LOAD_PARAM must match the field type of the target FB type.

**Error**: `R0302(offset, opcode, field_index, expected_type, actual_type)`

### Rule R0400: Jump Target Validity

Every JMP, JMP_IF, and JMP_IF_NOT offset must satisfy:

1. The computed target `(current_pc + operand_size + offset)` must be >= 0 and < the function's bytecode length
2. The target must land on a valid instruction boundary (the first byte of an instruction, not in the middle of an operand)

The verifier builds an instruction boundary map in the first pass and checks all jump targets against it.

**Error**: `R0400(offset, target_offset, reason)` where reason is "out_of_bounds" or "mid_operand"

### Rule R0401: Return Path Completeness

Every code path through a function must end in RET, RET_VOID, or an unconditional backward jump (loop). A path that falls off the end of the bytecode without a return instruction is rejected.

The verifier tracks reachability: after processing all instructions, every reachable instruction must either be a terminator (RET, RET_VOID) or have a successor instruction that is also reachable.

**Error**: `R0401(function_id, offset)` — the offset is the last reachable instruction that has no successor and is not a return.

### Rule R0402: Call Depth Bound

The verifier constructs a static call graph from CALL and FB_CALL instructions. The maximum depth of the call graph must not exceed the header's `max_call_depth`.

If the call graph contains cycles (recursion), the function is rejected. IEC 61131-3 does not permit recursion in standard PLC programs.

**Error**: `R0402(function_id, depth, max_depth)` or `R0403(function_id, cycle_path)`

### Rule R0500: FB_STORE_PARAM / FB_LOAD_PARAM Require Active FB Reference

FB_STORE_PARAM and FB_LOAD_PARAM must only appear after an FB_LOAD_INSTANCE (or after another FB_STORE_PARAM/FB_LOAD_PARAM that preserved the fb_ref on the stack). The verifier checks that the top of stack (for FB_LOAD_PARAM) or second-from-top (for FB_STORE_PARAM) is fb_ref.

**Error**: `R0500(offset, opcode, actual_type)`

### Rule R0600: Process Image Region Validity

Every LOAD_INPUT, STORE_OUTPUT, LOAD_MEMORY, and STORE_MEMORY instruction's `region` byte must be a valid access width (0–4):

| Value | Width | Meaning |
|-------|-------|---------|
| 0 | Bit | %IX, %QX, %MX |
| 1 | Byte | %IB, %QB, %MB |
| 2 | Word | %IW, %QW, %MW |
| 3 | Doubleword | %ID, %QD, %MD |
| 4 | Longword | %IL, %QL, %ML |

Values >= 5 are rejected.

**Error**: `R0600(offset, region_value)`

### Rule R0601: TIME Opcode Type Enforcement

TIME_ADD and TIME_SUB require two I64 values on the stack. The verifier further checks (via variable type metadata) that the source values are TIME-typed variables, not arbitrary I64 values.

To enforce this, the verifier tracks a sub-type for I64 stack slots: `I64_integer` (from LOAD_VAR_I64 of a LINT variable) vs `I64_time` (from LOAD_VAR_I64 of a TIME/DATE/TOD/DT variable, or from TIME_ADD/TIME_SUB output). TIME_ADD and TIME_SUB require `I64_time` inputs.

**Error**: `R0601(offset, opcode, expected_subtype, actual_subtype)`

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

        // Check all rules for this instruction
        check_all_rules(instruction, state, metadata)

        // Compute successor state(s)
        for (successor_offset, successor_state) in successors(instruction, state):
            if successor_offset not in state_at:
                state_at[successor_offset] = successor_state
                worklist.add(successor_offset)
            else:
                // Merge point: check R0200 and R0201
                existing = state_at[successor_offset]
                if existing.depth != successor_state.depth:
                    error(R0200)
                if existing.types != successor_state.types:
                    error(R0201)
                // States match — no need to re-process

    // Phase 3: Check R0401 (return path completeness)
    check_return_paths(bytecode, state_at, boundaries)

    // Phase 4: Check R0402/R0403 (call depth)
    check_call_graph(metadata)

    return PASS
```

### Complexity

The verifier visits each instruction at most once per incoming control flow path. For acyclic functions, this is O(n) where n is the number of instructions. For functions with loops, merge-point checking ensures each instruction is processed at most once after the state stabilizes.

The memory cost is one abstract state per merge point. Each abstract state contains the stack depth (u16) and one type tag (u8) per stack slot. For a function with max stack depth 32 and 20 merge points, this is ~20 × 33 = 660 bytes.

## Error Reporting

The verifier produces structured error messages that include:

1. The rule code (R0001 through R0601)
2. The bytecode offset of the offending instruction
3. The specific values that caused the failure (expected vs. actual types, bounds, depths)

If the debug section is available, the verifier maps the bytecode offset to a source line number using the line map.

Multiple errors may be reported in a single verification pass (the verifier does not stop at the first error). However, after the first error, subsequent errors may be spurious due to cascading effects. The first error is always accurate.

## Rule Index

| Code | Rule | Category |
|------|------|----------|
| R0001 | Valid Opcodes | Structural validity |
| R0002 | Operand Bounds | Structural validity |
| R0100 | Constant Type Match | Type metadata consistency |
| R0101 | Variable Type Match | Type metadata consistency |
| R0102 | Array Type Consistency | Type metadata consistency |
| R0200 | Stack Depth Consistency at Merge Points | Stack discipline |
| R0201 | Stack Type Consistency at Merge Points | Stack discipline |
| R0202 | No Stack Underflow | Stack discipline |
| R0203 | No Stack Overflow | Stack discipline |
| R0300 | Stack Type Correctness | Stack type correctness |
| R0301 | Function Call Parameter Type Correctness | Stack type correctness |
| R0302 | Field Access Type Correctness | Stack type correctness |
| R0400 | Jump Target Validity | Control flow |
| R0401 | Return Path Completeness | Control flow |
| R0402 | Call Depth Exceeded | Control flow |
| R0403 | Recursive Call Detected | Control flow |
| R0500 | FB Reference Required | Function block protocol |
| R0600 | Process Image Region Validity | Domain-specific type enforcement |
| R0601 | TIME Opcode Type Enforcement | Domain-specific type enforcement |
