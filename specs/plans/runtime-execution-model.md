# Spec: Runtime Execution Model

## Overview

This spec defines how the IronPLC virtual machine executes a loaded bytecode program over time. It covers the VM lifecycle, the scan cycle state machine, process image management, function block instance memory, string buffer lifecycle, trap handling, and the runtime clock.

The other specs define *what* individual instructions do and *how* bytecode is packaged. This spec defines *when* and *in what order* things happen at runtime — the bridge between "the VM can execute ADD_I32" and "the VM runs a PLC program."

This spec builds on:

- **[Bytecode Instruction Set](bytecode-instruction-set.md)**: The instructions this runtime executes
- **[Bytecode Container Format](bytecode-container-format.md)**: The container this runtime loads
- **[ADR-0002](../adrs/0002-bytecode-overflow-behavior.md)**: Configurable overflow policy (a runtime configuration parameter)
- **[ADR-0003](../adrs/0003-plc-standard-function-blocks-as-intrinsics.md)**: Standard FB intrinsic dispatch (runtime behavior of FB_CALL)

## Design Goals

1. **Deterministic execution** — given the same inputs and clock, two VMs produce identical outputs, scan by scan
2. **Bounded resource usage** — all memory is allocated at initialization; no dynamic allocation during scan cycles
3. **Fail-safe defaults** — on any error, outputs hold their last-known-good values rather than going to an unknown state
4. **Observable** — the runtime exposes enough state for an engineering tool to diagnose problems without modifying the program

## VM Lifecycle

The VM progresses through five states. Transitions are triggered by external commands (load, start, stop) or internal events (initialization complete, trap).

```
                    load
  ┌──────────┐  bytecode   ┌──────────────┐  init     ┌─────────┐
  │  EMPTY   │────────────►│   LOADING    │─────────►│  READY  │
  └──────────┘             └──────────────┘           └────┬────┘
                                  │                        │
                                  │ load fails             │ start
                                  ▼                        ▼
                           ┌──────────────┐          ┌─────────┐
                           │   STOPPED    │◄─────────│ RUNNING │◄─┐
                           │  (error set) │  stop    └────┬────┘  │
                           └──────────────┘               │       │
                                  ▲                  trap │       │
                                  │                       ▼       │
                                  │ stop            ┌─────────┐  │
                                  └─────────────────│ FAULTED │──┘
                                                    └─────────┘
                                                      restart
```

| State | Description |
|-------|-------------|
| EMPTY | No bytecode loaded. The VM is idle and waiting for a load command. |
| LOADING | Bytecode is being loaded and verified (container format loading sequence steps 1–12). No runtime resources are allocated yet. |
| READY | Bytecode is loaded, verified, and runtime resources are allocated and initialized. The VM is waiting for a start command. |
| RUNNING | The VM is executing scan cycles. This is the normal operating state. |
| FAULTED | A trap occurred during execution. Outputs hold their last-known-good values. The VM is waiting for a restart or stop command. |
| STOPPED | The VM has stopped. If an error is set, the stop was due to a load failure or explicit stop command after a fault. The VM can accept a new load command to return to LOADING. |

### Transition Rules

| From | To | Trigger | Action |
|------|----|---------|--------|
| EMPTY | LOADING | Load command with bytecode | Begin container loading sequence |
| LOADING | READY | Loading sequence completes (step 12: bytecode marked read-only) | Allocate and initialize runtime resources (see Initialization Sequence, replacing container format steps 13–15) |
| LOADING | STOPPED | Any loading step fails | Set error with failure reason; release any partially allocated resources |
| READY | RUNNING | Start command | Begin first scan cycle |
| RUNNING | FAULTED | Trap during EXECUTE phase | Abort current scan; hold outputs at last-good values; record diagnostic |
| RUNNING | STOPPED | Stop command | Complete current scan cycle (if in progress), then stop |
| FAULTED | RUNNING | Restart command | Re-initialize all runtime state (see Initialization Sequence), then begin scanning |
| FAULTED | STOPPED | Stop command | Release runtime resources |
| STOPPED | LOADING | Load command with new bytecode | Begin container loading sequence |

A restart from FAULTED re-initializes all variables, FB instances, and string buffers to their initial values. It does not reload or re-verify the bytecode — the loaded bytecode remains in memory and is reused. This allows recovery from transient errors (e.g., a division-by-zero caused by a specific input combination) without the overhead of a full reload.

## Scan Cycle

When the VM is in the RUNNING state, it executes a repeating scan cycle. Each cycle has four phases executed in strict order:

```
┌──────────────────────────────────────────────────────────────┐
│                        Scan Cycle                            │
│                                                              │
│  ┌─────────────┐  ┌─────────┐  ┌──────────────┐  ┌──────┐  │
│  │ INPUT_FREEZE │─►│ EXECUTE │─►│ OUTPUT_FLUSH │─►│ IDLE │  │
│  └─────────────┘  └─────────┘  └──────────────┘  └──────┘  │
│        ▲                                              │      │
│        └──────────────────────────────────────────────┘      │
└──────────────────────────────────────────────────────────────┘
```

### Phase 1: INPUT_FREEZE

The VM copies the current physical input state into the input process image (%I region). After this copy, the input image is a frozen snapshot — it does not change during the EXECUTE phase, even if physical inputs change.

**Atomicity.** The copy must be atomic with respect to the I/O driver: no partial update where some input bytes reflect the new state and others reflect the old state. On platforms with memory-mapped I/O, this is a memcpy. On platforms with bus-based I/O (e.g., EtherCAT, Modbus), the I/O driver must have completed its cycle before INPUT_FREEZE begins.

**Duration.** INPUT_FREEZE is bounded by the input image size (header field `input_image_bytes`). For a 256-byte input image on a 1 GHz processor, the copy takes < 1 microsecond. This phase is not subject to the scan cycle watchdog.

### Phase 2: EXECUTE

The VM calls the entry function (identified by `entry_function_id` in the container header) and executes it to completion. The entry function may call other functions and FBs via CALL and FB_CALL instructions.

**Process image access during EXECUTE:**
- `LOAD_INPUT` reads from the frozen input image (read-only)
- `STORE_OUTPUT` writes to the output staging buffer (write-only)
- `LOAD_MEMORY` and `STORE_MEMORY` access the memory region (%M) directly (read-write)

Writes during EXECUTE are accumulated in the staging buffer and are not visible to physical outputs until OUTPUT_FLUSH.

**Scan cycle watchdog.** The VM monitors the wall-clock duration of the EXECUTE phase. If execution exceeds the configured `max_scan_time`, the VM treats this as a trap (see Trap Handling). The watchdog prevents infinite loops or unexpectedly long computations from blocking the scan cycle indefinitely.

**Stack and call state.** At the start of EXECUTE, the operand stack is empty and the call stack contains a single frame for the entry function. At normal completion (RET_VOID from the entry function), the operand stack must be empty and the call stack must contain only the entry frame.

### Phase 3: OUTPUT_FLUSH

The VM hands the output staging buffer contents to the I/O driver. The I/O driver copies the data into its own physical output buffer and makes it visible to external equipment.

**Atomicity.** The I/O driver must apply the output data atomically — external equipment sees either the complete old state or the complete new state, never a mix. This is the I/O driver's responsibility.

**Duration.** Bounded by `output_image_bytes`. For a 256-byte output image, the handoff takes < 1 microsecond. This phase is not subject to the scan cycle watchdog.

### Phase 4: IDLE

The VM waits for the next scan cycle trigger. The trigger source depends on the scan mode:

| Scan mode | Trigger | Behavior |
|-----------|---------|----------|
| Periodic | Timer expiration | The VM waits until `scan_interval` has elapsed since the start of the current cycle. If EXECUTE + FREEZE + FLUSH already exceeded `scan_interval`, the next cycle starts immediately (no wait). |
| Free-running | Immediate | The next cycle starts as soon as OUTPUT_FLUSH completes. The scan rate is determined by execution speed. |

The scan mode and `scan_interval` are VM configuration parameters, not encoded in bytecode. This allows the same compiled program to run at different scan rates on different hardware.

### Scan Cycle Counter

The VM maintains a `scan_count` (u64) that increments by 1 at the start of each cycle (before INPUT_FREEZE). The counter starts at 0 for the first scan cycle after entering RUNNING state (or after a restart from FAULTED). The counter is available to the diagnostic interface but is not directly accessible from bytecode.

### Scan Cycle Timing

The VM records the following timing measurements for each scan cycle:

| Measurement | Description |
|-------------|-------------|
| `cycle_start` | Monotonic timestamp at the start of INPUT_FREEZE |
| `execute_duration` | Wall-clock duration of the EXECUTE phase |
| `cycle_duration` | Wall-clock duration of the entire scan cycle (FREEZE through end of IDLE) |
| `max_execute_duration` | Maximum `execute_duration` observed since entering RUNNING |

These measurements are available to the diagnostic interface. They are not accessible from bytecode — the program observes time only through the runtime clock (see Runtime Clock).

## Process Image

The VM maintains three memory regions for I/O:

| Region | IEC notation | Direction | Size | Header field |
|--------|-------------|-----------|------|-------------|
| Input | %I | Read-only during EXECUTE | Fixed at load time | `input_image_bytes` |
| Output | %Q | Write-only during EXECUTE | Fixed at load time | `output_image_bytes` |
| Memory | %M | Read-write during EXECUTE | Fixed at load time | `memory_image_bytes` |

### Memory Layout

Each region is a flat byte array. Addresses within the region are computed from the instruction operands:

| Region value | Access width | Byte offset formula |
|-------------|-------------|---------------------|
| 0 (Bit) | 1 bit | `index / 8` (byte), `index % 8` (bit within byte, LSB-first) |
| 1 (Byte) | 1 byte | `index` |
| 2 (Word) | 2 bytes | `index * 2` |
| 3 (Doubleword) | 4 bytes | `index * 4` |
| 4 (Longword) | 8 bytes | `index * 8` |

Bit addressing uses LSB-first bit ordering within each byte: bit index 0 is the least significant bit of byte 0, bit index 7 is the most significant bit of byte 0, bit index 8 is the least significant bit of byte 1, and so on.

### Output Staging

The VM owns a single output staging buffer. The I/O driver owns its own physical output buffer (outside the VM's memory budget).

```
                         STORE_OUTPUT                        OUTPUT_FLUSH
                        (during EXECUTE)                    (VM → I/O driver)
                              │                                   │
                              ▼                                   ▼
                    ┌──────────────────┐                ┌──────────────────┐
                    │  Staging Buffer  │───────────────►│  I/O Driver      │
                    │   (%Q pending)   │                │  (physical out)  │
                    └──────────────────┘                └──────────────────┘
```

During EXECUTE, `STORE_OUTPUT` writes to the staging buffer. During OUTPUT_FLUSH, the VM hands the staging buffer contents to the I/O driver, which copies them to its own physical output in one atomic operation. The VM does not own or manage the physical output memory — that is the I/O driver's responsibility.

This separation means a trap during EXECUTE leaves the staging buffer in a partial state, but the I/O driver's physical output retains the values from the last successful OUTPUT_FLUSH. The "last-known-good" guarantee is provided by the I/O driver never seeing incomplete data.

### Input Snapshot

The input region is a snapshot taken during INPUT_FREEZE. The physical input source and the snapshot are separate memory regions:

```
┌──────────────────┐     INPUT_FREEZE     ┌──────────────────┐
│  Physical Input  │────────────────────►│    Snapshot       │
│    (live I/O)    │                      │  (%I frozen)     │
└──────────────────┘                      └──────────────────┘
```

During EXECUTE, `LOAD_INPUT` reads from the snapshot. The physical input source may continue changing, but the snapshot remains constant throughout the scan cycle.

### Memory Region (%M)

The memory region is directly accessed — no double buffering. `LOAD_MEMORY` and `STORE_MEMORY` read and write the same memory. The memory region is not connected to physical I/O; it serves as scratch space for inter-scan persistence and communication between program sections.

### Initialization

All regions are zero-filled during initialization. The output staging buffer is zero-filled, so the first OUTPUT_FLUSH hands all-zeros to the I/O driver.

## Variable Table

The variable table is a flat array of 8-byte slots, indexed by `u16` variable index. The total number of slots is `num_variables` from the container header.

### Slot Layout

Each slot is 8 bytes wide (matching the operand stack slot width). Values smaller than 8 bytes occupy the low bytes; upper bytes are zero-filled.

| Variable type | Bytes used | Layout |
|---------------|-----------|--------|
| I32 | 4 | Bytes 0–3 = value (little-endian), bytes 4–7 = sign-extended |
| U32 | 4 | Bytes 0–3 = value (little-endian), bytes 4–7 = 0x00 |
| I64 | 8 | Bytes 0–7 = value (little-endian) |
| U64 | 8 | Bytes 0–7 = value (little-endian) |
| F32 | 4 | Bytes 0–3 = IEEE 754 single (little-endian), bytes 4–7 = 0x00 |
| F64 | 8 | Bytes 0–7 = IEEE 754 double (little-endian) |

STRING and WSTRING variables are not stored in the variable table slots. Their slot holds a `buf_idx` (a u16 index into the string buffer table, stored in bytes 0–1, bytes 2–7 = 0x00). FB instance variables similarly hold an `fb_ref` (u16 index into the FB instance table).

### Scope

The variable table is global to the program. All functions and FB bodies access the same variable table using `LOAD_VAR_*` / `STORE_VAR_*` instructions. The compiler maps each source-level variable (local, global, FB field) to a unique variable table index.

Function-local variables occupy dedicated variable table slots that are logically scoped to the function. The VM does not enforce this scoping at runtime — it is a compiler invariant. The per-function `num_locals` field in the code section's function directory tells the verifier how many local slots the function uses.

## Function Block Instance Memory

Each FB instance is a contiguous region in the FB instance table, addressed by a `u16` instance index (the `fb_ref`). The total number of instances is `num_fb_instances` from the container header.

### Instance Layout

Each instance is laid out as a sequence of fields in the order declared by the FB type descriptor in the type section. Field sizes are determined by their types:

| Field type | Size per field |
|-----------|---------------|
| I32, U32, F32 | 8 bytes (padded to slot width) |
| I64, U64, F64 | 8 bytes |
| STRING | `buf_idx` (8-byte slot holding a u16 index into the string buffer table) |
| WSTRING | `buf_idx` (8-byte slot holding a u16 index into the string buffer table) |
| FB_INSTANCE (nested) | `fb_ref` (8-byte slot holding a u16 index into the FB instance table) |

All fields are 8-byte aligned, matching the operand stack slot width. This uniform alignment means field access is `base_offset + field_index * 8`, where `base_offset` is the start of the instance in the FB instance table.

### FB_CALL Execution

When the VM executes `FB_CALL type_id`:

1. Pop `fb_ref` from the operand stack.
2. Look up `type_id` in the intrinsic table.
3. **If intrinsic match:** Call the native intrinsic handler, passing the instance memory region. The intrinsic reads input fields and writes output fields directly. Push `fb_ref` back onto the stack.
4. **If no intrinsic match:** Look up the function body for this FB type in the code section. Push a new call frame with the FB instance as the active context. The FB body accesses its own fields via `LOAD_FIELD` / `STORE_FIELD` instructions, which resolve against the active instance. On `RET_VOID`, pop the call frame and push `fb_ref` back onto the stack.

### Call Frame Context

Each call frame on the call stack records:

| Field | Size | Description |
|-------|------|-------------|
| `return_pc` | u32 | Bytecode offset to resume after return |
| `return_function_id` | u16 | Function ID of the caller (for looking up bytecode region) |
| `stack_base` | u16 | Operand stack depth at call site (for stack cleanup on trap) |
| `temp_str_base` | u16 | Temp string buffer pool watermark at call site (for cleanup on return) |

`LOAD_FIELD` and `STORE_FIELD` always consume an `fb_ref` from the operand stack, as defined in the instruction set spec. Within an FB body, the compiler emits `FB_LOAD_INSTANCE` to push the instance's own `fb_ref` before each field access sequence. This keeps field access uniform — the same instruction semantics apply whether accessing fields of the current instance or a nested instance.

### Nested FB Instances

An FB type may contain fields of other FB types (composition). The compiler allocates separate instance table entries for the outer FB and each nested FB. The outer FB's field of type FB_INSTANCE holds the `fb_ref` of the nested instance.

For example, a `Controller` FB that contains a `TON` timer:

```
FB instance table:
  Index 0: Controller instance
    field 0 (I32): setpoint
    field 1 (fb_ref): timer_ref → index 1
    field 2 (I32): output
  Index 1: TON instance (nested inside Controller)
    field 0 (I32): IN
    field 1 (I64): PT
    field 2 (I32): Q
    field 3 (I64): ET
```

When the `Controller` body calls its nested `TON`, it loads the nested `fb_ref` via `LOAD_FIELD`, then proceeds with the normal `FB_STORE_PARAM` / `FB_CALL` / `FB_LOAD_PARAM` sequence.

### Initialization

All FB instance fields are initialized during the Initialization Sequence:

1. All fields are zero-filled.
2. For each FB type descriptor, fields with declared initial values (from the constant pool) are set to their initial values.
3. Initialization order: instances are initialized in instance table order (index 0 first). Nested instances are initialized before the instances that reference them, because the compiler allocates nested instances at lower indices.

## String Buffer Management

The VM manages two categories of string buffers: variable buffers and temporary buffers.

### Variable Buffers

Each STRING or WSTRING variable has a dedicated buffer in the string buffer table. The buffer is allocated during initialization and persists for the lifetime of the program.

| Buffer type | Allocation size | Description |
|-------------|----------------|-------------|
| STRING variable | `declared_length + 1` bytes | 1 byte current length + `declared_length` bytes character data |
| WSTRING variable | `declared_length * 2 + 2` bytes | 2 bytes current length (in UCS-2 code units) + `declared_length * 2` bytes character data |

No null terminator is stored. The length prefix is the sole indicator of string extent. The VM never passes string buffers to external C code; all string operations use the length prefix to determine valid data. This avoids the maintenance burden of keeping a null terminator in sync on every mutation.

Each variable buffer is sized to its own declared length (from `VarEntry.extra`). The compiler pre-computes the total bytes for all STRING and WSTRING variable buffers and stores the sums in the container header as `total_str_var_bytes` and `total_wstr_var_bytes`. This avoids wasting memory when variables have different declared lengths (e.g., a `STRING(10)` gets 11 bytes, not the program-wide maximum).

### Temporary Buffers

The temporary buffer pool provides scratch space for intermediate string results. The pool contains `num_temp_str_bufs` STRING buffers and `num_temp_wstr_bufs` WSTRING buffers. Unlike variable buffers (which are sized per-variable), all temp buffers must be sized to the worst case: `max_str_length + 1` for STRING temps and `max_wstr_length × 2 + 2` for WSTRING temps. This is because any string expression could produce a result up to the program-wide maximum length.

### Buffer Index Space

Variable buffers and temporary buffers share a single `buf_idx` index space:

```
buf_idx layout:
  0 .. num_str_vars-1                                STRING variable buffers (each sized per declaration)
  num_str_vars .. num_str_vars+num_temp-1             STRING temporary buffers (each sized to max)
  (WSTRING indices follow the same pattern in a separate table)
```

The verifier and runtime distinguish STRING and WSTRING indices through the opcode used (STR_* vs WSTR_*), not through the index value. STRING and WSTRING buffers are in separate tables with separate index spaces.

### Lifecycle Protocol

Temporary buffers follow an acquire-use-release lifecycle within each function call:

**Acquire.** A temporary buffer is acquired when an instruction produces a string result that does not go directly into a variable buffer:
- `LOAD_CONST_STR` / `LOAD_CONST_WSTR` — copies a string literal from the constant pool into a temp buffer
- BUILTIN string functions that produce a string result (STR_CONCAT, STR_LEFT, STR_RIGHT, STR_MID, STR_INSERT, STR_DELETE, STR_REPLACE, and WSTRING equivalents) — write their result into a temp buffer

Acquisition uses a stack-like allocator (bump pointer): each acquire increments the temp pool watermark by 1. The `buf_idx` pushed onto the operand stack is the index of the acquired temp buffer.

**Use.** The temp buffer is read by subsequent instructions that consume a `buf_idx` input:
- BUILTIN string functions that take string inputs (STR_LEN, STR_FIND, STR_CONCAT, etc.)
- STR_STORE_VAR / WSTR_STORE_VAR (copies temp buffer contents into a variable buffer)

**Release.** Temp buffers are released in two ways:
1. **At function return.** When `RET` or `RET_VOID` executes, the temp pool watermark is reset to the value recorded in the call frame's `temp_str_base`. This releases all temp buffers acquired during the function call, preventing leaks from any control flow path (including early returns).
2. **At scan cycle end.** After EXECUTE completes, all temp buffers are released (watermark reset to 0). This is a safety net; well-compiled bytecode releases all temps via function returns.

**Pool exhaustion.** If an acquire would exceed the pool size (`num_temp_str_bufs` or `num_temp_wstr_bufs`), the VM traps with a pool-exhaustion fault. The compiler must size the temp pools to cover the deepest string expression nesting in the program. The formula is: for each function, count the maximum number of temp buffers simultaneously live at any point in the function body. The header fields are set to the maximum across all functions.

### Compiler Invariant

The compiler must ensure that no `buf_idx` for a temporary buffer is used after a subsequent string operation that could reuse the same buffer slot. In practice, this means the compiler emits `STR_STORE_VAR` / `WSTR_STORE_VAR` immediately after a string expression completes, before starting the next string expression. This invariant is not verified by the bytecode verifier — it is a compiler correctness requirement. Violation results in silently reading stale data, not a memory safety issue (the buffer memory is always valid, just potentially overwritten).

### Example: String Expression Lifecycle

```
(* Source *)
result := CONCAT(greeting, name);

(* Bytecode *)
STR_LOAD_VAR    0x0000    -- push buf_idx for greeting (variable buffer, no acquire)
STR_LOAD_VAR    0x0001    -- push buf_idx for name (variable buffer, no acquire)
BUILTIN         0x0101    -- STR_CONCAT: pops 2 buf_idx, acquires temp, pushes temp buf_idx
STR_STORE_VAR   0x0002    -- copies temp buffer contents into result's variable buffer
```

The temp buffer is live from BUILTIN (acquire) until the function returns (watermark reset). In simple cases like this, only one temp buffer is ever live at a time.

### Example: Nested String Expression

```
(* Source *)
result := CONCAT(CONCAT(a, b), c);

(* Bytecode *)
STR_LOAD_VAR    0x0000    -- push a (variable buffer)
STR_LOAD_VAR    0x0001    -- push b (variable buffer)
BUILTIN         0x0101    -- STR_CONCAT(a, b): acquires temp[0], pushes temp[0] buf_idx
STR_LOAD_VAR    0x0002    -- push c (variable buffer)
BUILTIN         0x0101    -- STR_CONCAT(temp[0], c): acquires temp[1], pushes temp[1] buf_idx
STR_STORE_VAR   0x0003    -- copies temp[1] into result; watermark resets at function return
```

At peak, 2 temp buffers are live simultaneously. The compiler must set `num_temp_str_bufs >= 2` for this function.

## Trap Handling

A trap is an unrecoverable error detected during the EXECUTE phase. The VM cannot continue executing the current scan cycle after a trap.

### Trap Sources

| Trap code | Source | Description |
|-----------|--------|-------------|
| DIVIDE_BY_ZERO | DIV_I32, DIV_U32, DIV_I64, DIV_U64, MOD_I32, MOD_U32, MOD_I64, MOD_U64 | Integer division or modulo with zero divisor |
| OVERFLOW | ADD_*, SUB_*, MUL_*, NEG_*, NARROW_*, F*_TO_I*, F*_TO_U* | Overflow under the `fault` overflow policy (ADR-0002) |
| ARRAY_OUT_OF_BOUNDS | LOAD_ARRAY, STORE_ARRAY | Array index outside declared bounds |
| STACK_OVERFLOW | any instruction | Operand stack depth exceeds `max_stack_depth` |
| CALL_DEPTH_EXCEEDED | CALL, FB_CALL | Call stack depth exceeds `max_call_depth` |
| STRING_POOL_EXHAUSTED | BUILTIN (string), LOAD_CONST_STR, LOAD_CONST_WSTR | Temporary string buffer pool has no free slots |
| WATCHDOG_EXPIRED | (external) | EXECUTE phase exceeded `max_scan_time` |
| INVALID_INSTRUCTION | any | Undefined opcode encountered at runtime (should never happen if verifier ran, but defense-in-depth) |

### Trap Sequence

When a trap occurs:

1. **Abort EXECUTE.** Instruction execution stops immediately. The operand stack and call stack are unwound (no further instructions execute).
2. **Skip OUTPUT_FLUSH.** The staging buffer contains partial output writes from the incomplete scan cycle. The VM does not hand the staging buffer to the I/O driver, so the I/O driver's physical output retains the values from the last successfully completed OUTPUT_FLUSH. This is the "last-known-good" guarantee.
3. **Record diagnostic.** The VM records a trap diagnostic containing:

| Field | Type | Description |
|-------|------|-------------|
| trap_code | u8 | Trap type (see Trap Sources table) |
| scan_count | u64 | Scan cycle number when the trap occurred |
| function_id | u16 | Function that was executing when the trap occurred |
| bytecode_offset | u32 | Bytecode offset within the function |
| operand_a | u64 | First operand value (instruction-specific; e.g., divisor for DIVIDE_BY_ZERO, index for ARRAY_OUT_OF_BOUNDS) |
| operand_b | u64 | Second operand value (instruction-specific; e.g., array upper bound for ARRAY_OUT_OF_BOUNDS) |

If the debug section is loaded, the VM also maps `bytecode_offset` to a source line number via the line map.

4. **Transition to FAULTED.** The VM enters the FAULTED state. No further scan cycles execute until a restart or stop command.

### Output Hold Behavior

When the VM transitions to FAULTED, physical outputs hold their values from the last successful OUTPUT_FLUSH. This matches the behavior of Siemens S7 PLCs (OB85/OB121 "last value" mode) and is the safest default for most industrial applications — equipment continues in its last commanded state rather than going to an unknown state.

The alternative — zeroing all outputs on fault — is available as a VM configuration parameter `fault_output_mode`:

| Mode | Behavior | Use case |
|------|----------|----------|
| `hold` (default) | Outputs retain last-good values | Process control where sudden stop is dangerous (conveyor belts, heaters with thermal mass) |
| `zero` | Outputs are set to all-zeros | Applications where energized-on-fault is dangerous (solenoid valves, motor drives with external braking) |

The `fault_output_mode` is a VM configuration parameter. When set to `zero`, the VM zero-fills the staging buffer and performs OUTPUT_FLUSH as step 2 of the trap sequence, causing the I/O driver to set all physical outputs to zero.

### Watchdog Implementation

The scan cycle watchdog runs during the EXECUTE phase only (not during INPUT_FREEZE, OUTPUT_FLUSH, or IDLE). The VM checks the elapsed time at instruction dispatch boundaries — specifically, at backward jump targets (loop headers) and at CALL/FB_CALL entry. This ensures that infinite loops and deep recursion are caught without checking the clock on every instruction.

The check frequency is a trade-off between detection latency and overhead:
- Checking at every backward jump catches infinite loops within ~100 instructions of the actual timeout
- Checking at CALL/FB_CALL catches runaway recursion
- Not checking on every instruction avoids ~5% overhead from clock reads

The `max_scan_time` is a VM configuration parameter (microseconds, u64). A value of 0 disables the watchdog.

## Runtime Clock

The VM provides a monotonic time source for timer intrinsics and scan cycle timing.

### Clock Snapshot

At the start of each scan cycle (before INPUT_FREEZE), the VM reads the platform's monotonic clock and stores the result as the cycle's **clock snapshot** (`cycle_time`, I64 microseconds). All time-dependent operations within the scan cycle use this snapshot, not real-time reads:

- Timer intrinsics (TON, TOF, TP) compare the clock snapshot against their stored start time to compute elapsed time.
- `LOAD_CONST_TIME` loads compile-time TIME constants. These are compared against timer outputs that were computed from the clock snapshot.

Using a snapshot rather than real-time reads ensures that all timer evaluations within a single scan cycle see the same "now." This prevents subtle ordering bugs where a timer expires for one part of the program but not another within the same cycle.

### Clock Source

The clock source is a VM configuration parameter:

| Source | Description | Use case |
|--------|-------------|----------|
| `system` (default) | Platform monotonic clock (e.g., `clock_gettime(CLOCK_MONOTONIC)` on Linux, `QueryPerformanceCounter` on Windows) | Normal PLC operation |
| `simulated` | Software clock advanced by the VM or test harness | Unit testing, simulation, deterministic replay |

When using the `simulated` clock, the test harness provides the time value for each scan cycle. This enables deterministic testing: the same sequence of time values produces the same timer behavior regardless of actual wall-clock speed.

### Timer Intrinsic Interaction

Timer FBs (TON, TOF, TP) need to compute elapsed time since an event (e.g., rising edge of IN). The intrinsic implementation:

1. On the scan cycle where the triggering event occurs, the intrinsic stores `cycle_time` as the start time in the FB instance's internal state.
2. On subsequent scan cycles, the intrinsic computes `elapsed = cycle_time - start_time`.
3. The intrinsic compares `elapsed` against `PT` (preset time) to determine if the timer has expired.

This means timer resolution is limited to the scan cycle interval — a timer with `PT := T#1ms` on a 10ms scan cycle will expire after 1-2 scan cycles (10-20ms real time). This matches the behavior of all hardware PLC runtimes, where timer resolution is bounded by scan time.

## Initialization Sequence

When the VM transitions from LOADING to READY, it allocates and initializes all runtime resources. This sequence runs exactly once per load (and again on restart from FAULTED).

```
 Step  Action
 ────  ──────────────────────────────────────────────────────────
  1    Allocate operand stack (max_stack_depth × 8 bytes)
  2    Allocate call stack (max_call_depth × frame_size bytes)
  3    Allocate variable table (num_variables × 8 bytes)
  4    Zero-fill variable table
  5    Apply initial values from constant pool to variables with declared initializers
  6    Allocate FB instance table (total size computed from FB type descriptors)
  7    Zero-fill FB instance table
  8    Apply initial values to FB instance fields from FB type descriptors
  9    Allocate STRING variable buffers (total_str_var_bytes, per-variable sizing)
 10    Allocate WSTRING variable buffers (total_wstr_var_bytes, per-variable sizing)
 11    Zero-fill all string variable buffers (current_length = 0, empty string)
 12    Apply STRING/WSTRING initial values from constant pool
 13    Allocate STRING temp buffer pool (num_temp_str_bufs × buffer_size)
 14    Allocate WSTRING temp buffer pool (num_temp_wstr_bufs × buffer_size)
 15    Allocate input process image (input_image_bytes)
 16    Allocate output staging buffer (output_image_bytes)
 17    Allocate memory region (memory_image_bytes)
 18    Zero-fill all process image regions (input, output staging, memory)
 19    Initialize scan_count to 0
 20    Initialize temp buffer watermarks to 0
 21    Initialize runtime clock
```

After step 21, the VM is in the READY state and all resources are allocated and initialized. No further allocation occurs during scan cycles.

### Memory Budget

Total memory allocated:

```
total_ram =
    (max_stack_depth × 8)                                    // operand stack
  + (max_call_depth × call_frame_size)                       // call stack
  + (num_variables × 8)                                      // variable table
  + (fb_instance_total_size)                                 // FB instances
  + total_str_var_bytes                                      // STRING var buffers (compiler-summed)
  + total_wstr_var_bytes                                     // WSTRING var buffers (compiler-summed)
  + (num_temp_str_bufs × (max_str_length + 1))               // STRING temp buffers
  + (num_temp_wstr_bufs × (max_wstr_length × 2 + 2))        // WSTRING temp buffers
  + input_image_bytes                                        // input snapshot
  + output_image_bytes                                       // output staging buffer
  + memory_image_bytes                                       // memory region
```

The container header provides all the values needed to compute this total before allocation. If the total exceeds available RAM, the VM rejects the program at load time (container loading sequence step 6).

Note: This formula supersedes the `ram_required` formula in the container format spec, which omits process image allocations. The container format spec should be updated to match. The output image appears once (not doubled) because the VM owns only the staging buffer; the I/O driver owns its own physical output buffer outside the VM's memory budget.

## VM Configuration Parameters

The following parameters are set at VM startup, not encoded in bytecode. They allow the same compiled program to run under different runtime policies.

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `overflow_policy` | enum | `wrap` | Overflow behavior: `wrap`, `saturate`, or `fault` (ADR-0002) |
| `scan_mode` | enum | `periodic` | Scan cycle trigger: `periodic` or `free_running` |
| `scan_interval` | u64 (microseconds) | 10000 (10ms) | Scan cycle interval (ignored if `scan_mode` is `free_running`) |
| `max_scan_time` | u64 (microseconds) | 100000 (100ms) | EXECUTE phase watchdog timeout (0 = disabled) |
| `fault_output_mode` | enum | `hold` | Output behavior on fault: `hold` or `zero` |
| `clock_source` | enum | `system` | Time source: `system` or `simulated` |
| `verification_mode` | enum | `on_device` | Bytecode verification: `on_device` (full verification) or `signature_only` (ADR-0006) |

These parameters are provided by the runtime host (the application embedding the VM), not by the bytecode program. This separation ensures that safety-critical parameters (overflow policy, watchdog timeout, fault output mode) cannot be overridden by the program being executed.

## Diagnostic Interface

The VM exposes runtime state to external tools (engineering workstations, HMI systems, logging infrastructure) through a diagnostic interface. This spec defines *what* is exposed, not the transport protocol (which is deployment-specific).

### Readable State

| Category | Fields | Update frequency |
|----------|--------|-----------------|
| VM status | lifecycle state, error (if STOPPED with error) | On state change |
| Scan cycle | scan_count, cycle_start, execute_duration, cycle_duration, max_execute_duration | Every scan cycle |
| Trap info | trap_code, scan_count, function_id, bytecode_offset, operand_a, operand_b, source_line (if debug loaded) | On trap |
| Variable values | Variable table contents (indexed by variable index) | On request |
| Process image | Input, output, and memory region contents | On request |
| Configuration | All VM configuration parameters | On request |

### Variable Observation

When a diagnostic tool reads variable values, the VM provides a consistent snapshot from the most recent completed scan cycle. The tool never sees a partial update (e.g., a variable written halfway through EXECUTE). The VM achieves this by serving variable reads from a snapshot taken at the end of EXECUTE (before OUTPUT_FLUSH), or by serializing reads to occur during IDLE.

### Read-Only

The diagnostic interface is read-only. External tools cannot modify variables, process images, or VM state through this interface. Modification capabilities (forcing variables, overriding outputs) are out of scope for this spec and would require a separate debug/commissioning interface with appropriate authentication and safety controls.

## Out of Scope

The following are explicitly out of scope for this version of the runtime execution model:

1. **Multi-task scheduling** — Multiple programs running at different priorities and intervals within the same VM instance. This spec covers single-program, single-scan-cycle execution only.

2. **Online change** — Hot-swapping bytecode while the VM is in RUNNING state, preserving variable values across the change. This requires a separate specification covering variable matching, state migration, and safe transition points.

3. **RETAIN / PERSISTENT variables** — Saving variable values to non-volatile storage across power cycles. The initialization sequence always starts from zero/default values.

4. **Debug interface** — Breakpoints, single-stepping, variable forcing, and other interactive debugging capabilities. The diagnostic interface is observation-only.

5. **I/O driver model** — How the input process image is populated from physical hardware and how the output process image drives physical hardware. This spec assumes the I/O driver is a platform-specific component that the VM interacts with only during INPUT_FREEZE and OUTPUT_FLUSH.

6. **Inter-VM communication** — Multiple VM instances sharing data (e.g., for distributed control). Each VM instance is self-contained.

7. **Network protocols** — OPC UA, Modbus TCP, EtherNet/IP, or other industrial communication protocols. These are application-layer concerns above the VM.
