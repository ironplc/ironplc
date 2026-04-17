# Plan: Bytecode Verifier Implementation

## Context

IronPLC's VM currently performs bounds checks on every memory access at runtime
(stack push/pop, variable load/store, scope check, array bounds, constant pool
lookup). ADR-0006 requires bytecode verification before execution to catch
codegen bugs, corruption, and tampered bytecode. The verifier spec (R0001-R0602)
is fully designed in `specs/design/bytecode-verifier-rules.md` but not yet
implemented.

Once the verifier runs at load time and proves structural validity, the VM can
switch to unchecked accessors in the hot loop — eliminating 2-4 branches per
instruction. The verifier is both a security requirement and the prerequisite
for this performance optimization.

### Design choice: verify-then-trust vs per-access checks

The analysis considered whether design changes (e.g., Wasm-style structured
control flow, typed bytecode builders) could eliminate the need for verification
entirely. Conclusion: **they cannot**, because bytecode travels over a network
to the PLC and must be validated against tampering and corruption. The correct
approach is the JVM/Wasm model: verify once at load time, then execute without
per-instruction bounds checks.

Checks the verifier can prove statically (removable from the hot loop after
verification):

- Stack overflow/underflow (R0202, R0203)
- Variable index bounds (R0002)
- Variable scope validity (implicit in R0002)
- Constant pool bounds (R0002, R0100)
- Jump target validity (R0400)
- Valid opcodes (R0001)
- Function/FB ID validity (R0002)
- Process image bounds (R0602)

Checks that remain runtime-only:

- Array index bounds (index is a runtime value)
- Divide by zero (divisor is a runtime value)
- Null dereference (reference validity is runtime state)
- String buffer exhaustion (depends on runtime string lengths)
- Watchdog timeout (real-time duration)

## Approach

Create a new `verifier` crate in `compiler/verifier/` that depends only on
`ironplc-container`. Implement rules incrementally in four phases.

The verifier takes `&Container` and returns `Result<(), Vec<VerificationError>>`.

## Phase 1: Structural Validity (Rules R0001, R0002)

These rules validate that bytecode is well-formed without needing abstract
interpretation.

### Step 1: Create the verifier crate

- Create `compiler/verifier/Cargo.toml` (name `ironplc-verifier`, version
  `0.188.0`, depends on `ironplc-container`)
- Add `"verifier"` to workspace members in `compiler/Cargo.toml`
- Create `compiler/verifier/src/lib.rs` with public
  `verify(&Container) -> Result<(), Vec<VerificationError>>` entry point
- Create `compiler/verifier/src/error.rs` with `VerificationError` enum

### Step 2: Implement instruction decoding

- Create `compiler/verifier/src/decode.rs`
- Implement `instruction_size(opcode: u8) -> Option<usize>` — returns `None`
  for undefined opcodes, total instruction size (including opcode byte)
  otherwise:
  - 1-byte: arithmetic, comparison, logical, stack ops (POP, DUP, SWAP, RET,
    RET_VOID), type conversions, NARROW/WIDEN
  - 2-byte: FB_STORE_PARAM (u8 field), FB_LOAD_PARAM (u8 field)
  - 3-byte: all LOAD_CONST, LOAD_VAR, STORE_VAR, JMP, JMP_IF_NOT, CALL,
    FB_LOAD_INSTANCE, FB_CALL, BUILTIN, STR_LOAD_VAR, STR_STORE_VAR,
    WSTR_LOAD_VAR, WSTR_STORE_VAR
  - 5-byte: LOAD_ARRAY, STORE_ARRAY, LOAD_ARRAY_DEREF, STORE_ARRAY_DEREF,
    STR_INIT, WSTR_INIT, and similar compound-operand instructions
- Source of truth: `compiler/container/src/opcode.rs`
- Implement `scan_instruction_boundaries(bytecode: &[u8]) -> Result<Vec<bool>,
  VerificationError>` — boolean map where `true` = valid instruction start

### Step 3: Implement R0001 (Valid Opcodes)

- During boundary scan, reject undefined opcode bytes
- Error: `R0001 { offset: u32, byte_value: u8 }`

### Step 4: Implement R0002 (Operand Bounds)

- For each instruction with an index operand, check against container metadata:
  - `LOAD_CONST_*` index < `container.constant_pool.len()`
  - `LOAD_VAR_*/STORE_VAR_*` index < `container.header.num_variables`
  - `CALL` function_id < `container.code.functions.len()`
  - `FB_CALL` type_id < fb_types count in type_section
  - `BUILTIN` func_id is a defined built-in function ID
  - `FB_STORE_PARAM/FB_LOAD_PARAM` field < num_fields for target FB type
  - `STR_LOAD_VAR/STR_STORE_VAR` index references STRING-typed variable
    (requires type_section)
- Error: `R0002 { offset: u32, operand: &'static str, value: u16, max: u16 }`

### Step 5: Tests for Phase 1

- Create `compiler/verifier/tests/` with per-rule test files
- Use `ContainerBuilder` to construct containers with malformed bytecode
- BDD-style test names per project convention
- Tests: valid bytecode passes; undefined opcode -> R0001; out-of-bounds
  constant/variable/function -> R0002

## Phase 2: Stack Discipline (Rules R0200-R0203, R0300)

### Step 6: Abstract interpretation framework

- Create `compiler/verifier/src/abstract_state.rs`
- `AbstractState { depth: u16, types: Vec<AbstractType> }`
- `AbstractType` enum: I32, U32, I64, U64, F32, F64, BufIdxStr, BufIdxWstr,
  FbRef, VarRef
- `stack_effect(opcode: u8) -> (pops: u8, pushes: u8)` per opcode
- `operand_types(opcode: u8)` and `result_type(opcode: u8)` for type checking

### Step 7: Worklist-based verifier

- Create `compiler/verifier/src/verify.rs`
- Algorithm (from `specs/design/bytecode-verifier-rules.md`):
  1. Build instruction boundary map (reuse Phase 1)
  2. Initialize worklist with function entry point
  3. Process each instruction: check depth, compute successor states
  4. At merge points: check depth/type consistency (R0200, R0201)
  5. After processing: check underflow (R0202) and max depth (R0203)

### Step 8: R0300 (Stack Type Correctness)

- Verify input types match expected types per opcode

### Step 9: R0100-R0102 (Type Metadata Consistency)

- R0100: LOAD_CONST type matches constant pool entry type
- R0101: LOAD_VAR/STORE_VAR type matches declared variable type
- R0102: LOAD_ARRAY/STORE_ARRAY type byte matches array element type
- Requires TypeSection

### Step 10: Tests for Phase 2

## Phase 3: Control Flow & Remaining Rules (R0400-R0602)

### Step 11: R0400 (Jump Target Validity)

- Jump offset must land on instruction boundary within function

### Step 12: R0401 (Return Path Completeness)

- Every reachable path ends in RET/RET_VOID or unconditional backward jump

### Step 13: R0402, R0403 (Call Depth / Recursion)

- Build static call graph; check for cycles; check max depth

### Step 14: R0404 (No Unreachable Code)

- All bytecode offsets within function body must be reachable

### Step 15: R0500, R0510 (FB Protocol)

- FB param ops require active fb_ref on stack
- BUILTIN func_id validity and argument types

### Step 16: R0600-R0602 (Domain-Specific)

- R0600: Process image region validity
- R0601: TIME opcode type enforcement (I64_time subtype)
- R0602: Process image bounds

### Step 17: Tests for Phase 3

## Phase 4: VM Integration

### Step 18: Wire verifier into VM loading

- Add `ironplc-verifier` dependency to `ironplc-vm`
- Call `verify()` in `Vm::load()` before constructing `VmReady`
- Gate on feature flag `verified-execution` initially

### Step 19: Unchecked execution paths (future, after fuzzing)

- Add `push_unchecked()/pop_unchecked()` to `OperandStack`
- Add `load_unchecked()/store_unchecked()` to `VariableTable`
- Keep checked paths as `debug_assert!()` in test builds

## Key Files

| File | Action |
|------|--------|
| `compiler/verifier/Cargo.toml` | NEW |
| `compiler/verifier/src/lib.rs` | NEW — public verify() entry point |
| `compiler/verifier/src/decode.rs` | NEW — instruction decoding, boundary map |
| `compiler/verifier/src/abstract_state.rs` | NEW — abstract type/state tracking |
| `compiler/verifier/src/verify.rs` | NEW — worklist algorithm, rule checks |
| `compiler/verifier/src/error.rs` | NEW — VerificationError enum |
| `compiler/verifier/tests/` | NEW — per-rule integration tests |
| `compiler/Cargo.toml` | MODIFY — add verifier to workspace |
| `compiler/vm/Cargo.toml` | MODIFY — add verifier dep (Phase 4) |
| `compiler/vm/src/vm.rs` | MODIFY — call verifier in load() (Phase 4) |

## Reusable Code

- `ContainerBuilder` from `ironplc-container` — test container construction
- `ironplc_container::opcode` constants — no duplication needed
- ID types (FunctionId, VarIndex, ConstantIndex, FbTypeId) from container crate
- `FieldType` enum from `type_section` — type matching in R0100-R0102
- Test helper pattern from `compiler/vm/tests/common/mod.rs`

## Reference Specs

- `specs/design/bytecode-verifier-rules.md` — normative rule definitions
- `specs/adrs/0006-bytecode-verification-requirement.md` — architectural decision
- `specs/design/bytecode-instruction-set.md` — instruction encoding
- `specs/design/bytecode-container-format.md` — container metadata

## Verification

1. `cd compiler && just` — full CI pipeline
2. Valid containers from codegen tests must pass verification
3. Hand-crafted invalid containers must be rejected per-rule
4. Fuzz verifier with random bytecode — must never panic (future)
