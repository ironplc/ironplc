# Spec: Debugger Support

## Overview

This spec defines the debugger architecture for IronPLC — the full stack from debug info in compiled bytecode through the VM debug engine to VS Code integration via the Debug Adapter Protocol (DAP). The goal is interactive debugging of IEC 61131-3 programs: set breakpoints on source lines, step through code, inspect variables, and control scan cycle execution.

This spec builds on:

- **[Bytecode Container Format](bytecode-container-format.md)**: Debug section format (line maps, variable names)
- **[Bytecode Instruction Set](bytecode-instruction-set.md)**: The instructions the debug engine must understand for stepping
- **[Runtime Execution Model](runtime-execution-model.md)**: VM lifecycle, scan cycle phases, and the diagnostic interface this debug interface extends

## Design Goals

1. **Source-level debugging** — breakpoints, stepping, and variable inspection use source line numbers and variable names, not bytecode offsets and indices
2. **Scan-cycle-aware** — the debugger understands PLC scan cycle semantics; users can pause between cycles, step one cycle at a time, or break mid-cycle
3. **Zero overhead when disabled** — when no debugger is attached, the VM runs at full speed with no breakpoint checks or callback overhead
4. **Separable debug info** — the debug section can be stripped from production containers without affecting execution; the debugger loads it separately if needed
5. **Standard protocol** — use the Debug Adapter Protocol (DAP) so any DAP-compatible editor can debug IronPLC programs, not just VS Code

## Architecture

```
┌──────────────────────────────────────────────────────────────────┐
│                          VS Code                                 │
│  ┌────────────────┐     ┌──────────────────────────────┐         │
│  │  IronPLC Ext   │────►│  Debug Adapter (TypeScript)  │         │
│  │  (LSP client)  │     │  launches DAP server process │         │
│  └────────────────┘     └──────────┬───────────────────┘         │
└─────────────────────────────────────┼────────────────────────────┘
                                      │ DAP (JSON, stdin/stdout)
                                      ▼
                            ┌───────────────────┐
                            │   DAP Server       │
                            │  (ironplcvm debug) │   Rust binary
                            │                    │   (vm-cli crate)
                            └────────┬──────────┘
                                     │ Rust API calls
                                     ▼
                            ┌───────────────────┐
                            │   VM Debug Engine  │   ironplc-vm crate
                            │  BreakpointTable   │
                            │  StepController    │
                            │  DebugState        │
                            └────────┬──────────┘
                                     │
                                     ▼
                            ┌───────────────────┐
                            │   VM Execution     │
                            │  execute() loop    │   existing VM
                            │  VmRunning state   │
                            └───────────────────┘
```

The debugger is four layers:

| Layer | Location | Responsibility |
|-------|----------|----------------|
| **Debug info** | `codegen` crate + `container` crate | Emit and parse line maps, variable names in the debug section |
| **VM debug engine** | `vm` crate | Breakpoint matching, step tracking, pause/continue, variable inspection with names |
| **DAP server** | `vm-cli` crate | Translate DAP protocol messages to VM debug engine API calls |
| **VS Code integration** | `integrations/vscode` | Launch configuration, debug adapter descriptor, UI contributions |

### Why not a separate DAP binary?

The DAP server is a subcommand of `ironplcvm` (`ironplcvm debug`) rather than a separate binary. This avoids duplicating the VM embedding code and keeps the build matrix simple. The VS Code extension launches `ironplcvm debug --dap <file.iplc>`, which speaks DAP on stdin/stdout.

## Layer 1: Debug Info

### Gap Analysis

The container format spec defines a debug section with three sub-tables: source text, line maps, and variable names. That format is insufficient for Structured Text debugging. This section identifies each gap and specifies the additions required.

| # | Gap | Impact | Required for v1? |
|---|-----|--------|-------------------|
| 1 | No function name table | DAP `stackTrace` can only show "function 0" — useless for multi-POU programs | Yes |
| 2 | No variable scope info | DAP `scopes`/`variables` can't separate Locals from Globals or group by IEC section (VAR_INPUT, VAR_OUTPUT, etc.) | Yes |
| 3 | No variable type names | Variables pane shows raw slot values with no type context; can't render BOOL as TRUE/FALSE, REAL as float, or FB instances by type name | Yes |
| 4 | No source column | Breakpoint highlight covers entire line; can't pinpoint within a line containing multiple statements | No — line-level is sufficient for ST (one statement per line is idiomatic) |
| 5 | No FB type name table | Expanding an FB instance variable shows "fb_type_3" instead of "TON" | No — FB debugging deferred to Phase 5 |
| 6 | No FB field name table | FB instance fields show as "field[0]" instead of "IN", "PT", "Q", "ET" | No — FB debugging deferred to Phase 5 |
| 7 | No source file table | Line maps can't reference files in multi-file projects | No — v1 assumes single file per container |

Gaps 1–3 must be addressed in the debug section format before ST debugging can work. Gaps 4–7 are deferred.

### Revised Debug Section Format

The debug section format extends the container format spec's definition. The additions are backward-compatible: a reader that only understands the original format can skip the new sub-tables by reading past them using the counts. The sub-tables appear in a fixed order after the original three sub-tables.

```
┌──────────────────────────────────────────────────────────────┐
│ Source text (original)                                        │
│   source_text_length: u32                                    │
│   source_text: [u8; N]          UTF-8 source (optional)      │
├──────────────────────────────────────────────────────────────┤
│ Line maps (original)                                          │
│   line_map_count: u16                                        │
│   line_maps: [LineMapEntry; count]   8 bytes each            │
├──────────────────────────────────────────────────────────────┤
│ Variable names (revised — extended fields)                    │
│   var_name_count: u16                                        │
│   var_names: [VarNameEntry; count]   variable size           │
├──────────────────────────────────────────────────────────────┤
│ Function names (new)                                          │
│   func_name_count: u16                                       │
│   func_names: [FuncNameEntry; count]   variable size         │
├──────────────────────────────────────────────────────────────┤
│ FB type names (new, reserved for Phase 5)                    │
│   type_name_count: u16                                       │
│   type_names: [TypeNameEntry; count]   variable size         │
├──────────────────────────────────────────────────────────────┤
│ FB field names (new, reserved for Phase 5)                    │
│   field_name_count: u16                                      │
│   field_names: [FieldNameEntry; count]   variable size       │
└──────────────────────────────────────────────────────────────┘
```

#### LineMapEntry (8 bytes, changed from 6)

| Offset | Field | Type | Description |
|--------|-------|------|-------------|
| 0 | function_id | u16 | Function containing this mapping |
| 2 | bytecode_offset | u16 | Offset within the function's bytecode |
| 4 | source_line | u16 | Source line number (1-based) |
| 6 | source_column | u16 | Source column number (1-based, 0 = unknown) |

The column field is populated when available but may be zero. Stepping and breakpoint resolution use the line number; the column is for editor highlight precision.

#### VarNameEntry (variable size, extended)

| Offset | Field | Type | Description |
|--------|-------|------|-------------|
| 0 | var_index | u16 | Variable table index |
| 2 | function_id | u16 | Owning function ID (0xFFFF = global scope) |
| 4 | var_section | u8 | IEC 61131-3 variable section (see encoding below) |
| 5 | name_length | u8 | Length of variable name in bytes |
| 6 | name | [u8; name_length] | UTF-8 variable name |
| 6+N | type_name_length | u8 | Length of type name in bytes |
| 7+N | type_name | [u8; type_name_length] | UTF-8 type name (e.g., "DINT", "REAL", "TON") |

**var_section encoding:**

| Value | IEC 61131-3 Section | DAP Scope Mapping |
|-------|--------------------|--------------------|
| 0 | VAR | Locals |
| 1 | VAR_TEMP | Locals |
| 2 | VAR_INPUT | Inputs |
| 3 | VAR_OUTPUT | Outputs |
| 4 | VAR_IN_OUT | In/Out |
| 5 | VAR_EXTERNAL | Globals |
| 6 | VAR_GLOBAL | Globals |

**Why function_id on variables?** The variable table is flat (all functions share one table with compiler-assigned partitions). Without `function_id`, the debugger can't determine which variables are visible in the current stack frame. The `function_id` lets the DAP server filter variables to show only those belonging to the current function plus globals (function_id = 0xFFFF).

**Why type_name as a string?** The type section encodes types as u8 enum codes (0=I32, 1=U32, ..., 8=FB_INSTANCE). This is sufficient for the verifier but not for human display. For elementary types, the DAP server could maintain a hardcoded mapping (0 → "DINT"), but this breaks for user-defined types (FB instances, enumerations, structures) where the u8 just says "FB_INSTANCE" with a type_id. Embedding the source-level type name in the debug section keeps the mapping simple and handles all cases uniformly.

#### FuncNameEntry (variable size, new)

| Offset | Field | Type | Description |
|--------|-------|------|-------------|
| 0 | function_id | u16 | Function ID from the code section |
| 2 | name_length | u8 | Length of function name in bytes |
| 3 | name | [u8; name_length] | UTF-8 POU name (e.g., "MAIN", "MotorControl") |

The DAP `stackTrace` response requires a function name for each frame. Without this table, stack frames can only display numeric function IDs.

#### TypeNameEntry (variable size, reserved for Phase 5)

| Offset | Field | Type | Description |
|--------|-------|------|-------------|
| 0 | type_id | u16 | FB type ID from the type section |
| 2 | name_length | u8 | Length of type name in bytes |
| 3 | name | [u8; name_length] | UTF-8 type name (e.g., "TON", "CTU", "MotorController") |

For v1, `type_name_count` is 0. This table is populated when FB support is added to the codegen.

#### FieldNameEntry (variable size, reserved for Phase 5)

| Offset | Field | Type | Description |
|--------|-------|------|-------------|
| 0 | type_id | u16 | FB type ID |
| 2 | field_index | u8 | Field index within the FB type descriptor |
| 3 | name_length | u8 | Length of field name in bytes |
| 4 | name | [u8; name_length] | UTF-8 field name (e.g., "IN", "PT", "Q", "ET") |

For v1, `field_name_count` is 0. When populated, this table allows the Variables pane to show `myTimer.IN = TRUE` instead of `myTimer.field[0] = 1`.

### How DAP Uses the Debug Info

The following table traces each DAP request to the debug info sub-table it reads:

| DAP Request | Sub-table Used | What It Provides |
|-------------|---------------|-------------------|
| `setBreakpoints` | Line maps | Resolve source line → (function_id, bytecode_offset); snap to nearest valid line |
| `stackTrace` | Function names + line maps | Frame name (FuncNameEntry.name) + source location (LineMapEntry.source_line) |
| `scopes` | Variable names | Group variables by var_section: Locals (VAR, VAR_TEMP), Inputs (VAR_INPUT), Outputs (VAR_OUTPUT), In/Out (VAR_IN_OUT), Globals (VAR_EXTERNAL, VAR_GLOBAL) |
| `variables` | Variable names + type section | Name (VarNameEntry.name), type (VarNameEntry.type_name), value (read from VariableTable, formatted according to type) |
| `evaluate` | Variable names | Look up variable by name, return formatted value |

### Codegen Changes

The compiler must emit the full debug section. The existing `Emitter` in `codegen/src/emit.rs` tracks bytecode offsets; it must also record source positions alongside each statement's bytecode.

#### Source Position Tracking

The AST carries `SourceSpan` (byte offsets into the source text). The codegen must convert byte offsets to line/column numbers. This requires a **line offset table** — an array of byte offsets where each line starts. The `compile()` function must accept the source text (or a precomputed line offset table) alongside the `Library` AST.

```rust
/// Precomputed table for converting byte offsets to line:column pairs.
struct LineOffsetTable {
    /// Byte offset of the start of each line (0-indexed).
    /// line_starts[0] = 0, line_starts[1] = offset of first '\n' + 1, etc.
    line_starts: Vec<usize>,
}

impl LineOffsetTable {
    fn from_source(source: &str) -> Self { ... }

    /// Converts a byte offset to (line, column), both 1-based.
    fn line_column(&self, byte_offset: usize) -> (u16, u16) { ... }
}
```

#### Emitter API Additions

```rust
impl Emitter {
    /// Records that the next emitted instruction corresponds to the given source position.
    /// Call this before emitting the first instruction of each statement.
    pub fn mark_source_position(
        &mut self,
        function_id: u16,
        source_line: u16,
        source_column: u16,
    ) { ... }

    /// Returns the accumulated line map entries.
    pub fn line_map(&self) -> &[LineMapEntry] { ... }
}
```

**When to emit entries.** One entry per statement start (assignment, IF, WHILE, FOR, CASE, RETURN, EXIT, function/FB call). Not per expression — stepping operates at the statement level.

#### Variable Debug Info Collection

The `CompileContext` currently maps `Id → u16` (name → index) in a `HashMap`. It must also collect the reverse mapping plus scope metadata:

```rust
struct VarDebugInfo {
    var_index: u16,
    name: String,
    function_id: u16,       // 0xFFFF for globals
    var_section: u8,        // from VariableType enum
    type_name: String,      // from the type_name in the AST
}
```

The compiler already has access to `VarDecl.var_type` (the IEC section: Var, Input, Output, etc.) and `VarDecl.initializer` (which contains the type name). The `assign_variables()` function must record this metadata alongside the index assignment.

#### Function Name Collection

The compiler emits one `FuncNameEntry` per compiled function. For v1 (single PROGRAM), this is one entry mapping function_id 0 to the program's name (e.g., "MAIN"). When FUNCTION and FUNCTION_BLOCK compilation is added, each compiled body produces an entry.

### Container Changes

The `container` crate must serialize and deserialize the revised debug section.

#### New Types

```rust
/// Parsed debug information from the container's debug section.
pub struct DebugInfo {
    pub source_text: Option<String>,
    pub line_maps: Vec<LineMapEntry>,
    pub var_names: Vec<VarNameEntry>,
    pub func_names: Vec<FuncNameEntry>,
    pub type_names: Vec<TypeNameEntry>,
    pub field_names: Vec<FieldNameEntry>,
}

pub struct LineMapEntry {
    pub function_id: u16,
    pub bytecode_offset: u16,
    pub source_line: u16,
    pub source_column: u16,
}

pub struct VarNameEntry {
    pub var_index: u16,
    pub function_id: u16,
    pub var_section: u8,
    pub name: String,
    pub type_name: String,
}

pub struct FuncNameEntry {
    pub function_id: u16,
    pub name: String,
}

pub struct TypeNameEntry {
    pub type_id: u16,
    pub name: String,
}

pub struct FieldNameEntry {
    pub type_id: u16,
    pub field_index: u8,
    pub name: String,
}
```

#### Builder Changes

`ContainerBuilder` gains a `debug_info(DebugInfo)` method. When present, the container is serialized with the debug section and `flags` bit 1 is set.

#### Read Path

`Container::read_from()` checks `flags` bit 1. If set, it reads and parses the debug section. Debug info is stored in `Container.debug_info: Option<DebugInfo>`. If the debug section is malformed, it is silently discarded (non-fatal, per the container format spec).

A reader that encounters fewer sub-tables than expected (e.g., a container produced before the function name table was added) treats the missing tables as empty. This provides forward compatibility: older containers work with newer debuggers, just with less information.

### Source Text Embedding

The debug section optionally embeds the source text. This enables debugging even when the original `.st` file is not available (e.g., debugging on a remote target). The source text is included by default in debug builds and excluded in release builds. This is a compiler flag, not a container format decision.

## Layer 2: VM Debug Engine

### Overview

The VM debug engine adds breakpoint matching, single-stepping, and pause/continue control to the existing `execute()` function. It is designed to have zero overhead when no debugger is attached.

### Debug State

A new `DebugState` struct holds all debug-related state:

```rust
/// Debug state attached to a running VM.
/// Only allocated when a debugger is attached.
pub struct DebugState {
    /// Active breakpoints, keyed by (function_id, bytecode_offset).
    breakpoints: BreakpointTable,

    /// Current stepping mode (None when running freely).
    step_mode: Option<StepMode>,

    /// Call depth at the point where stepping began (for step-over/step-out).
    step_origin_depth: u16,

    /// Source line at the point where stepping began (for step-over).
    step_origin_line: u16,

    /// Debug info from the container (line maps, var names, func names, etc.).
    debug_info: DebugInfo,
}
```

The `DebugInfo` struct contains all six sub-tables parsed from the debug section. The `DebugState` uses them as follows:

- **line_maps** — breakpoint resolution, step tracking (current line lookup), source location for `stackTrace`
- **var_names** — variable inspection with scope filtering, type-aware formatting, name-based `evaluate`
- **func_names** — stack frame names for `stackTrace`
- **type_names / field_names** — reserved for Phase 5 FB instance expansion

### Breakpoint Table

```rust
/// Fast breakpoint lookup table.
///
/// Uses a sorted Vec of (function_id, bytecode_offset) pairs.
/// At typical breakpoint counts (< 100), linear scan of a sorted
/// Vec is faster than a HashMap due to cache locality.
pub struct BreakpointTable {
    entries: Vec<BreakpointEntry>,
}

struct BreakpointEntry {
    function_id: u16,
    bytecode_offset: u16,
    id: u32,         // DAP breakpoint ID for reporting back
    enabled: bool,
}
```

**Breakpoint resolution.** The DAP client sends breakpoints as source line numbers. The DAP server resolves each line to the nearest `LineMapEntry` with `source_line >= requested_line` for the appropriate function. This "snap to next valid line" behavior matches how most debuggers handle breakpoints on non-statement lines (blank lines, comments, closing END_IF, etc.).

### Step Modes

```rust
enum StepMode {
    /// Step to the next statement on a different source line (regardless of call depth).
    /// DAP "next" (step over).
    StepOver,

    /// Step to the next statement on a different source line (allowing deeper call depth).
    /// DAP "stepIn".
    StepIn,

    /// Run until the call depth is less than the origin depth.
    /// DAP "stepOut".
    StepOut,

    /// Run until the next scan cycle boundary (end of EXECUTE phase).
    /// Custom: step one scan cycle.
    StepScan,
}
```

### Execute Loop Modification

The `execute()` function gains an optional `&mut DebugState` parameter. When `None`, the loop runs unchanged (zero overhead). When `Some`, the loop checks for debug events at each instruction dispatch:

```rust
fn execute(
    bytecode: &[u8],
    container: &Container,
    stack: &mut OperandStack,
    variables: &mut VariableTable,
    scope: &VariableScope,
    debug: Option<&mut DebugState>,
) -> Result<ExecuteResult, Trap> {
    let mut pc: usize = 0;

    while pc < bytecode.len() {
        // --- Debug check (only when debugger attached) ---
        if let Some(ref mut dbg) = debug {
            if let Some(action) = dbg.check(function_id, pc as u16) {
                return Ok(ExecuteResult::Paused(PauseReason::from(action)));
            }
        }

        let op = bytecode[pc];
        pc += 1;
        // ... existing dispatch ...
    }
}
```

**Performance note.** The `if let Some(ref mut dbg)` branch is trivially predictable when `debug` is `None` — the branch predictor will learn the pattern after one iteration and never mispredict. The cost is one branch per instruction (~0.5ns on modern hardware), which is negligible compared to the instruction dispatch cost itself. For production builds without a debugger, passing `None` means zero overhead.

#### DebugState::check()

```rust
impl DebugState {
    /// Called at each instruction boundary when a debugger is attached.
    /// Returns Some(action) if execution should pause.
    fn check(&mut self, function_id: u16, bytecode_offset: u16) -> Option<PauseAction> {
        // 1. Check breakpoints
        if self.breakpoints.hit(function_id, bytecode_offset) {
            return Some(PauseAction::Breakpoint);
        }

        // 2. Check step mode
        if let Some(ref mode) = self.step_mode {
            let current_line = self.debug_info.lookup_line(function_id, bytecode_offset);
            match mode {
                StepMode::StepOver => {
                    if current_line != self.step_origin_line
                        && self.current_call_depth <= self.step_origin_depth
                    {
                        self.step_mode = None;
                        return Some(PauseAction::Step);
                    }
                }
                StepMode::StepIn => {
                    if current_line != self.step_origin_line {
                        self.step_mode = None;
                        return Some(PauseAction::Step);
                    }
                }
                StepMode::StepOut => {
                    if self.current_call_depth < self.step_origin_depth {
                        self.step_mode = None;
                        return Some(PauseAction::Step);
                    }
                }
                StepMode::StepScan => {
                    // Handled at the scan cycle boundary, not here
                }
            }
        }

        None
    }
}
```

### Execute Result

The `execute()` function returns a richer result type when debugging:

```rust
enum ExecuteResult {
    /// Normal completion (RET_VOID from entry function).
    Completed,
    /// Execution paused for a debug event.
    Paused(PauseReason),
}

enum PauseReason {
    /// Hit a breakpoint.
    Breakpoint(u32),  // breakpoint ID
    /// Step completed.
    Step,
    /// User requested pause.
    UserPause,
}
```

When the execute loop returns `Paused`, the VM stays in the RUNNING state but does not proceed to OUTPUT_FLUSH. The DAP server can then:
- Read variable values
- Set/clear breakpoints
- Issue a continue/step command to resume

### Scan Cycle Control

For PLC-specific debugging, users need scan-level control:

| Command | Behavior |
|---------|----------|
| **Step Scan** | Run one complete scan cycle (INPUT_FREEZE → EXECUTE → OUTPUT_FLUSH), then pause before the next |
| **Pause Between Scans** | Always pause after OUTPUT_FLUSH, before the next INPUT_FREEZE |
| **Run to Scan N** | Continue until `scan_count` reaches a target value |

These are implemented in `VmRunning::run_round()`:

```rust
impl VmRunning {
    pub fn run_round_debug(
        &mut self,
        current_time_us: u64,
        debug: &mut DebugState,
    ) -> Result<RoundResult, FaultContext> {
        // ... existing scheduling logic ...

        // Execute with debug hooks
        let result = execute(bytecode, ..., Some(debug))?;

        match result {
            ExecuteResult::Completed => {
                // Normal completion — check if scan-level pause requested
                if debug.step_mode == Some(StepMode::StepScan) {
                    debug.step_mode = None;
                    return Ok(RoundResult::PausedAfterScan);
                }
                // ... OUTPUT_FLUSH, continue to next round ...
            }
            ExecuteResult::Paused(reason) => {
                // Mid-scan pause — outputs are NOT flushed
                return Ok(RoundResult::PausedMidScan(reason));
            }
        }
    }
}
```

### Variable Inspection

When paused, the debugger can inspect variables using the debug info's scope and type metadata:

```rust
impl DebugState {
    /// Returns variables visible in the given scope, with names, types, and values.
    pub fn inspect_variables(
        &self,
        variables: &VariableTable,
        function_id: u16,
        scope_filter: ScopeFilter,
    ) -> Vec<NamedVariable> {
        self.debug_info
            .var_names
            .iter()
            .filter(|entry| {
                // Show variables owned by this function OR globals
                let owned = entry.function_id == function_id
                    || entry.function_id == 0xFFFF;
                owned && scope_filter.matches(entry.var_section)
            })
            .filter_map(|entry| {
                let slot = variables.load(entry.var_index).ok()?;
                Some(NamedVariable {
                    name: entry.name.clone(),
                    type_name: entry.type_name.clone(),
                    var_section: entry.var_section,
                    index: entry.var_index,
                    value: format_value(slot, &entry.type_name),
                })
            })
            .collect()
    }

    /// Resolves a function_id to its source name for stack frames.
    pub fn function_name(&self, function_id: u16) -> Option<&str> {
        self.debug_info
            .func_names
            .iter()
            .find(|f| f.function_id == function_id)
            .map(|f| f.name.as_str())
    }
}
```

**Type-aware formatting.** The `VarNameEntry.type_name` field provides the source-level type name (e.g., "DINT", "REAL", "BOOL"). The `format_value()` function uses this to render slot values correctly:

| type_name | Formatting |
|-----------|-----------|
| BOOL | `TRUE` / `FALSE` |
| SINT, INT, DINT | Signed decimal |
| USINT, UINT, UDINT | Unsigned decimal |
| LINT | Signed 64-bit decimal |
| ULINT | Unsigned 64-bit decimal |
| REAL | Float with ~7 significant digits |
| LREAL | Float with ~15 significant digits |
| BYTE, WORD, DWORD, LWORD | Hexadecimal with `16#` prefix |
| TIME, LTIME | `T#` duration format |
| STRING | Quoted string content |
| (FB type) | `{type_name}` (expand via field names in Phase 5) |

### Variable Forcing (Write)

Variable forcing allows the debugger to override a variable's value:

```rust
impl DebugState {
    /// Forces a variable to a specific value. The value persists until
    /// unforced or the program writes to it.
    pub fn force_variable(
        &self,
        variables: &mut VariableTable,
        var_index: u16,
        value: Slot,
    ) -> Result<(), Trap> {
        variables.store(var_index, value)
    }
}
```

**Safety considerations.** Variable forcing can cause unexpected program behavior. The DAP server should display forced variables differently in the UI and log all force operations. A future extension could add a "force table" that re-applies forced values at each scan cycle start (matching industrial PLC behavior), but the initial implementation uses simple write-through.

## Layer 3: DAP Server

### Protocol

The DAP server speaks the [Debug Adapter Protocol](https://microsoft.github.io/debug-adapter-protocol/) over stdin/stdout using JSON messages with Content-Length headers, identical to LSP transport.

### Launch Configuration

The DAP server is launched as a subprocess by the VS Code extension:

```
ironplcvm debug --dap <file.iplc>
```

The `--dap` flag switches to DAP mode (stdin/stdout JSON messages instead of the normal run-to-completion behavior).

### DAP Request Mapping

| DAP Request | VM Debug Engine Action |
|-------------|----------------------|
| `initialize` | Return capabilities (supportsConfigurationDoneRequest, supportsSingleThread) |
| `launch` | Load container, allocate VM, transition to READY |
| `setBreakpoints` | Resolve source lines to bytecode offsets via line map; update BreakpointTable |
| `configurationDone` | Start VM (READY → RUNNING), run until first breakpoint or pause |
| `threads` | Return single thread (PLC programs are single-threaded within the scan cycle) |
| `stackTrace` | Return current function name (FuncNameEntry) + source line/column (LineMapEntry) |
| `scopes` | Return IEC-specific scopes: Locals (VAR, VAR_TEMP), Inputs (VAR_INPUT), Outputs (VAR_OUTPUT), In/Out (VAR_IN_OUT), Globals (VAR_EXTERNAL, VAR_GLOBAL) — filtered by VarNameEntry.var_section |
| `variables` | Read variable values from VariableTable; display name (VarNameEntry.name), type (VarNameEntry.type_name), formatted value |
| `continue` | Resume execution (clear step mode, run until next breakpoint) |
| `next` | Set StepMode::StepOver, resume execution |
| `stepIn` | Set StepMode::StepIn, resume execution |
| `stepOut` | Set StepMode::StepOut, resume execution |
| `pause` | Set a flag that DebugState::check() tests on next instruction |
| `disconnect` | Stop VM, exit process |
| `evaluate` | Read a single variable by name (watches) |

### Custom DAP Requests

For PLC-specific debugging, the DAP server supports custom requests:

| Custom Request | Description |
|----------------|-------------|
| `ironplc/stepScan` | Run one complete scan cycle, then pause |
| `ironplc/scanCount` | Return current scan_count |
| `ironplc/forceVariable` | Force a variable to a value |
| `ironplc/unforceVariable` | Remove a variable force |

### Threading Model

The DAP server runs two threads:

1. **DAP I/O thread** — reads DAP requests from stdin, sends responses/events to stdout
2. **VM execution thread** — runs the VM; pauses when a debug event fires

Communication between threads uses a channel pair:
- DAP I/O → VM: `DebugCommand` (Continue, StepOver, SetBreakpoints, Pause, etc.)
- VM → DAP I/O: `DebugEvent` (Stopped, Continued, Exited, Output, etc.)

```
┌────────────────┐     commands     ┌─────────────────┐
│  DAP I/O       │────────────────►│  VM Execution    │
│  thread        │◄────────────────│  thread          │
│  (stdin/stdout)│     events      │  (run_round)     │
└────────────────┘                  └─────────────────┘
```

### Capabilities

The DAP server advertises these capabilities in the `initialize` response:

```json
{
    "supportsConfigurationDoneRequest": true,
    "supportsSingleThreadExecutionRequests": true,
    "supportsSetVariable": true,
    "supportsEvaluateForHovers": true,
    "supportsTerminateRequest": true,
    "supportsSteppingGranularity": false,
    "exceptionBreakpointFilters": [
        {
            "filter": "traps",
            "label": "VM Traps",
            "description": "Break on VM traps (divide by zero, overflow, etc.)",
            "default": true
        }
    ]
}
```

### Trap Breakpoints

When "VM Traps" is enabled (the default), the VM pauses on any trap instead of transitioning to FAULTED. The DAP server sends a `stopped` event with `reason: "exception"` and includes the trap details. The user can inspect variables at the point of failure, then disconnect.

## Layer 4: VS Code Integration

### Debug Adapter Configuration

The VS Code extension registers a debug adapter in `package.json`:

```json
{
    "contributes": {
        "debuggers": [
            {
                "type": "ironplc",
                "label": "IronPLC Debugger",
                "program": "${command:ironplc.getDebugAdapterPath}",
                "runtime": "executable",
                "configurationAttributes": {
                    "launch": {
                        "required": ["program"],
                        "properties": {
                            "program": {
                                "type": "string",
                                "description": "Path to the .iplc bytecode file or .st source file",
                                "default": "${workspaceFolder}/${command:ironplc.getActiveFile}"
                            },
                            "stopOnEntry": {
                                "type": "boolean",
                                "description": "Pause at the first statement",
                                "default": true
                            },
                            "compileFirst": {
                                "type": "boolean",
                                "description": "Compile .st source to .iplc before debugging",
                                "default": true
                            },
                            "scanLimit": {
                                "type": "number",
                                "description": "Maximum scan cycles before auto-stop (0 = unlimited)",
                                "default": 0
                            }
                        }
                    }
                },
                "configurationSnippets": [
                    {
                        "label": "IronPLC: Debug Current File",
                        "description": "Compile and debug the current Structured Text file",
                        "body": {
                            "type": "ironplc",
                            "request": "launch",
                            "name": "Debug ${1:Program}",
                            "program": "^\"${2:\\${workspaceFolder}/\\${file}}\"",
                            "compileFirst": true,
                            "stopOnEntry": true
                        }
                    }
                ],
                "initialConfigurations": [
                    {
                        "type": "ironplc",
                        "request": "launch",
                        "name": "Debug IronPLC Program",
                        "program": "${workspaceFolder}/${file}",
                        "compileFirst": true,
                        "stopOnEntry": true
                    }
                ]
            }
        ]
    }
}
```

### Debug Adapter Factory

The extension implements a `DebugAdapterDescriptorFactory` that:

1. If `compileFirst` is true and `program` ends with `.st`, compiles the source to a temp `.iplc` file using the IronPLC compiler (same binary used for LSP)
2. Launches `ironplcvm debug --dap <file.iplc>` as a child process
3. Connects stdin/stdout to VS Code's DAP client

### Scan Cycle Toolbar

For PLC-specific debugging, the extension adds a custom toolbar button:

- **Step Scan** button in the debug toolbar (sends the `ironplc/stepScan` custom request)
- **Scan Count** display in the debug status bar

These are optional enhancements that can be added after the core debugger works.

## Phased Implementation

### Phase 1: Debug Info Foundation

**Goal:** Compile programs with full debug info (line maps, variable names with scope/type, function names) and verify via the disassembler.

**Changes:**

| Crate | Files | Changes |
|-------|-------|---------|
| `container` | `container.rs`, new `debug_info.rs` | Add `DebugInfo`, `LineMapEntry`, `VarNameEntry`, `FuncNameEntry`, `TypeNameEntry`, `FieldNameEntry` types; serialize/deserialize the full debug section; add `debug_info` field to `Container` |
| `container` | `builder.rs` | Add `debug_info()` method to `ContainerBuilder` |
| `container` | `header.rs` | Write `debug_section_offset` and `debug_section_size` when debug section present; set flags bit 1 |
| `codegen` | `emit.rs` | Add `mark_source_position()`, `line_map()` methods to `Emitter`; track line map entries alongside bytecode |
| `codegen` | `compile.rs` | Accept source text for `LineOffsetTable` construction; call `mark_source_position()` before each statement; collect `VarDebugInfo` (name, function_id, var_section, type_name) during `assign_variables()`; emit `FuncNameEntry` for each compiled POU; build and pass `DebugInfo` to `ContainerBuilder` |
| `plc2x` | `disassemble.rs` | Display debug section in disassembly output: line maps with function names, variable names with scope/type annotations |

**Tests:**
- Codegen: compile a program, verify line map entries map to expected source lines and columns
- Codegen: verify `VarNameEntry` includes correct `function_id`, `var_section`, and `type_name` for each variable
- Codegen: verify `FuncNameEntry` maps function_id 0 to the program's name
- Container: roundtrip test — write full debug section (all 6 sub-tables), read it back, verify contents
- Container: read container without debug section — `debug_info` is `None`
- Container: read container with malformed debug section — silently discarded
- Container: read container with partial debug section (missing new sub-tables) — missing tables treated as empty

### Phase 2: VM Debug Engine

**Goal:** The VM can pause at breakpoints, single-step, and inspect variables by name.

**Changes:**

| Crate | Files | Changes |
|-------|-------|---------|
| `vm` | new `debug.rs` | `DebugState`, `BreakpointTable`, `StepMode`, `PauseReason`, `ExecuteResult` types |
| `vm` | `vm.rs` | Add `debug: Option<DebugState>` to `VmRunning`; add `run_round_debug()` method; wire debug state into execute |
| `vm` | `vm.rs` (execute fn) | Add `debug: Option<&mut DebugState>` parameter; add debug check at instruction dispatch |
| `vm` | `lib.rs` | Export debug types |

**Tests:**
- Breakpoint: set breakpoint on a line, run, verify VM pauses at the correct bytecode offset
- Step over: pause at line N, step over, verify pause at line N+1 (not inside called function)
- Step in: pause at a function call line, step in, verify pause inside the function body
- Step out: pause inside a function, step out, verify pause at the caller's next line
- Continue: pause at breakpoint, continue, verify VM runs until next breakpoint or completion
- No debug overhead: run without debug state, verify execute() signature accepts `None`
- Variable inspection: pause at breakpoint, inspect variables by name, verify correct values

### Phase 3: DAP Server

**Goal:** Launch `ironplcvm debug --dap <file.iplc>` and debug from VS Code using standard DAP.

**Changes:**

| Crate | Files | Changes |
|-------|-------|---------|
| `vm-cli` | `main.rs` | Add `Debug` subcommand with `--dap` flag |
| `vm-cli` | new `dap.rs` | DAP message parsing (Content-Length framing, JSON deserialization) |
| `vm-cli` | new `dap_server.rs` | DAP request handling, maps to VM debug engine API |
| `vm-cli` | new `dap_types.rs` | DAP protocol types (Request, Response, Event, Capabilities, etc.) |

**Dependency consideration:** The DAP protocol is simpler than LSP and does not require a large framework. The initial implementation can use manual JSON serialization with `serde_json`, similar to how the disassembler produces JSON. If a well-maintained DAP crate exists (e.g., `dap-rs`), prefer it over hand-rolling the protocol.

**Tests:**
- Unit tests for DAP message parsing (Content-Length framing)
- Integration test: send `initialize` + `launch` + `setBreakpoints` + `configurationDone` messages to a DAP server process, verify it sends `stopped` event at the breakpoint
- Integration test: send `continue` after stopped, verify execution completes
- Integration test: send `variables` request while paused, verify variable names and values

### Phase 4: VS Code Integration

**Goal:** Click "Debug" in VS Code and get a working debug session with breakpoints, stepping, and variable inspection.

**Changes:**

| Location | Files | Changes |
|----------|-------|---------|
| `integrations/vscode` | `package.json` | Add `debuggers` contribution (see VS Code Integration section above) |
| `integrations/vscode/src` | new `debugAdapter.ts` | `DebugAdapterDescriptorFactory` implementation; launches `ironplcvm debug --dap` |
| `integrations/vscode/src` | `extension.ts` | Register debug adapter factory on activation |

**Tests:**
- Extension test: verify debug adapter is registered
- Extension test: verify launch configuration resolves the ironplcvm path
- Manual test: set breakpoint in .st file, press F5, verify breakpoint hit and variable inspection works

### Phase 5: PLC-Specific Enhancements (Future)

These enhancements build on the core debugger but are not required for initial functionality:

1. **Scan cycle toolbar** — custom VS Code toolbar button for Step Scan
2. **Variable forcing** — write variables through the debug interface with force indicators in the UI
3. **Process image inspection** — view %I, %Q, %M regions with bit/byte/word addressing
4. **Conditional breakpoints** — DAP `condition` field on breakpoints, evaluated by the VM
5. **Logpoints** — DAP `logMessage` field, VM prints message without stopping
6. **Hot reload during debug** — recompile and online-change while paused, preserving breakpoints
7. **Multi-task debugging** — when multi-task scheduling is implemented, show tasks as DAP threads

## Container Format Compatibility

This spec **extends** the debug section format defined in the container format spec. The extensions are:

1. **LineMapEntry grows from 6 to 8 bytes** — adds `source_column: u16`
2. **VarNameEntry gains scope and type fields** — adds `function_id`, `var_section`, `type_name`
3. **Three new sub-tables** — function names, FB type names, FB field names

These changes affect only the debug section, which is independently hashed and signed (via `debug_hash` and the debug signature section). Adding or modifying debug info does not affect the content signature or the content hash, so existing containers remain valid.

The container format spec's debug section definition should be updated to match the format specified in this document. Since the debug section has not yet been implemented in the container crate, this is a spec revision, not a breaking change.

**Forward compatibility.** The new sub-tables are appended after the existing sub-tables. A reader that encounters a debug section shorter than expected (because it was produced by an older compiler) treats missing sub-tables as having count 0. A reader that encounters extra data after the last expected sub-table ignores it. This allows gradual rollout: an older debugger can read the line maps and variable names from a new-format container, just without the function name or FB type/field name tables.

The VM handles the debug section in its loading sequence (step 13 in the container format spec): if present and valid, load it; if invalid, discard it silently.

## Bytecode-Level Debugging

The debug section is designed for source-level debugging but also supports bytecode-level debugging with no additional format changes. The bytecode instruction set is self-describing — each opcode encodes its own operand sizes — so a disassembler can decode any bytecode stream without debug info.

For bytecode-level debugging in the DAP:

- The DAP `stackTrace` response includes `instructionPointerReference` (the bytecode offset as a hex string), which is always available regardless of debug info.
- The DAP `disassemble` request returns decoded instructions. The VM can disassemble any function's bytecode on-the-fly using the instruction set definition. No debug section data is needed.
- When debug info is present, the DAP server annotates disassembled instructions with source line numbers from the line map, enabling mixed source+bytecode views.
- Breakpoints can be set by bytecode offset (DAP `setInstructionBreakpoints`) independently of source breakpoints.

This means bytecode debugging works even when the debug section is stripped — the user just loses source correlation.

## Out of Scope

1. **Remote debugging** — DAP over TCP to debug programs on remote targets (embedded PLCs). The initial implementation uses stdin/stdout only.
2. **Multi-file debugging** — debugging programs that span multiple source files. The initial implementation assumes a single source file per container. The debug section format reserves space for a source file table (via a future sub-table) but does not define it in v1.
3. **Ladder Diagram / FBD debugging** — graphical IEC 61131-3 languages (LD, FBD) have fundamentally different debugging UIs (highlighting rungs, showing power flow, animating contacts/coils). The debug section format could support LD by adding a rung-map sub-table (mapping bytecode offsets to rung IDs and element positions within a rung), but the compilation pipeline, DAP server, and VS Code extension would all need LD-specific support. This is deferred until graphical language compilation is implemented.
4. **Memory breakpoints** — breaking when a specific variable's value changes (hardware watchpoints). These require polling or page-fault tricks that are not practical in the VM.
5. **Time-travel debugging** — recording and replaying execution. Would require snapshotting VM state at each scan cycle, which is a significant memory and performance cost.
