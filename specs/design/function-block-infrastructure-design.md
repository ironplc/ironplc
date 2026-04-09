# Function Block Infrastructure Design

Date: 2026-03-09

## Goal

Add full function block (FB) infrastructure to the compiler and VM, enabling standard library FBs to execute. TON (on-delay timer) is the first intrinsic. The infrastructure supports all future standard FBs (TOF, TP, CTU, etc.) without architectural changes.

## Scope

- Full FB calling convention: FB_LOAD_INSTANCE, FB_STORE_PARAM, FB_CALL, FB_LOAD_PARAM opcodes
- Type section in the container format (for verifier, not VM)
- Codegen for programs calling standard FBs (not user-defined FB body compilation)
- VM data region, FB opcode handlers, intrinsic dispatch table
- Runtime clock snapshot (system + simulated modes)
- TON as the first native intrinsic

### Out of scope

- User-defined FB body compilation (bytecode interpretation path in FB_CALL)
- Nested FB instances
- VAR_IN_OUT parameters on FBs
- Other intrinsics beyond TON (TOF, TP, counters, edge detectors)

## Architecture

The design follows the existing specs:
- [ADR-0003](../../specs/adrs/0003-plc-standard-function-blocks-as-intrinsics.md): Standard FBs as VM intrinsics via FB_CALL
- [Bytecode Instruction Set](../../specs/design/bytecode-instruction-set.md): FB opcode definitions (0xC0-0xC3)
- [Bytecode Container Format](../../specs/design/bytecode-container-format.md): Type section with FB type descriptors
- [Runtime Execution Model](../../specs/design/runtime-execution-model.md): FB instance memory, clock snapshot, intrinsic dispatch

### Key design principle: no_std VM

The VM reads only the fixed-size header, never the type section. The compiler pre-computes all memory layout (data region offsets, field indices) and encodes them directly in opcode operands and header fields. The type section exists only for the verifier (std-only).

## Layer-by-Layer Design

### Container

**Opcodes** (`opcode.rs`):
- `FB_LOAD_INSTANCE` (0xC0) - operand: u16 variable index
- `FB_STORE_PARAM` (0xC1) - operand: u8 field index
- `FB_LOAD_PARAM` (0xC2) - operand: u8 field index
- `FB_CALL` (0xC3) - operand: u16 type_id

Well-known intrinsic type IDs (e.g., `FB_TYPE_TON: u16 = 0x0010`).

**Type section** (std-only): Serialize/deserialize FB type descriptors, variable table entries per the container format spec. For the verifier, not the VM.

**Builder**: Extend `ContainerBuilder` to accept FB type descriptors and set `num_fb_types` in the header.

### Codegen

**Variable allocation**: FB instance variables get a variable table slot (holding the data region byte offset) and a contiguous region in the data region (each field = 8 bytes).

**FB call emission** for `myTimer(IN := start, PT := T#5s); elapsed := myTimer.ET;`:
```
FB_LOAD_INSTANCE  <var_index>     -- push fb_ref from variable table
LOAD_VAR_I32      <start_index>   -- push start value
FB_STORE_PARAM    0               -- store to IN (field 0)
LOAD_CONST_I64    <time_const>    -- push T#5s as I64 microseconds
FB_STORE_PARAM    1               -- store to PT (field 1)
FB_CALL           <TON_type_id>   -- call TON intrinsic
FB_LOAD_PARAM     3               -- load ET (field 3)
STORE_VAR_I64     <elapsed_index> -- store to elapsed
POP                               -- discard fb_ref
```

**Field index mapping**: Standard FB field layouts come from analyzer type definitions in `stdlib_function_block.rs`. TON: IN=0, PT=1, Q=2, ET=3.

### VM

**Data region**: Flat byte array of `data_region_bytes` size, zero-filled at init.

**FB opcode handlers**:
- `FB_LOAD_INSTANCE(var_index)`: Read data region offset from variable table, push as fb_ref
- `FB_STORE_PARAM(field)`: Pop value, peek fb_ref, write to `data_region[fb_ref + field * 8]`
- `FB_LOAD_PARAM(field)`: Peek fb_ref, read from `data_region[fb_ref + field * 8]`, push value
- `FB_CALL(type_id)`: Peek fb_ref, dispatch to intrinsic table or trap

**Intrinsic dispatch**: Match on type_id. Unknown type_ids trap (user-defined FB body interpretation is future work).

**Runtime clock**: Monotonic clock snapshot (`cycle_time: i64` microseconds) taken at scan cycle start. Simulated clock mode for deterministic testing.

### TON Intrinsic

Fields: IN (i32, field 0), PT (i64, field 1), Q (i32, field 2), ET (i64, field 3).
Hidden fields: start_time (i64, field 4), running (i32, field 5). Compiler allocates these but source program doesn't see them.

TON logic per scan:
1. IN rises (TRUE, was not running): `start_time = cycle_time`, `running = true`
2. IN stays TRUE: `ET = min(cycle_time - start_time, PT)`. If `ET >= PT`: `Q = true`
3. IN falls (FALSE): `Q = false`, `ET = 0`, `running = false`

## PR Structure

1. **Container**: FB opcodes, type IDs, type section, builder extensions
2. **Codegen**: FB instance allocation, FB call sequence emission, stack depth tracking
3. **VM**: Data region, FB opcode handlers, intrinsic dispatch (empty), runtime clock
4. **VM - TON**: TON native handler with hidden fields
5. **End-to-end**: Integration test, documentation update
