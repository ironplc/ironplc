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

## Layer 1: Debug Info Emission

### Codegen Changes

The compiler must emit a debug section containing line maps and variable names. The existing `Emitter` in `codegen/src/emit.rs` tracks bytecode offsets; it must also record which source line each statement begins at.

#### Line Map Generation

For each statement in the AST, the compiler records a `LineMapEntry` mapping the bytecode offset of the statement's first instruction to the source line number:

```rust
struct LineMapEntry {
    function_id: u16,
    bytecode_offset: u16,
    source_line: u16,
}
```

**When to emit entries.** One entry per statement start (assignment, IF, WHILE, FOR, CASE, RETURN, EXIT, function/FB call). Not per expression — stepping operates at the statement level.

**Line number computation.** The AST carries `SourceSpan` (byte offsets). The codegen must convert byte offsets to line numbers using the source text. The `compile()` function already receives the `Library` AST; it must also receive the source text (or a precomputed line-offset table) so it can map `SourceSpan.start` to a 1-based line number.

**Emitter API additions:**

```rust
impl Emitter {
    /// Records that the next emitted instruction corresponds to the given source line.
    /// Call this before emitting the first instruction of each statement.
    pub fn mark_source_line(&mut self, function_id: u16, source_line: u16) { ... }

    /// Returns the accumulated line map entries.
    pub fn line_map(&self) -> &[LineMapEntry] { ... }
}
```

#### Variable Name Emission

For each variable in the variable table, the compiler emits a `VarNameEntry`:

```rust
struct VarNameEntry {
    var_index: u16,
    name: String,  // serialized as length-prefixed UTF-8
}
```

The compiler already assigns variable indices in `compile.rs`; it must also record the mapping from index to source name.

### Container Changes

The `container` crate must serialize and deserialize the debug section.

#### Debug Section Structure

The debug section follows the format defined in the container format spec:

```
source_text_length: u32
source_text: [u8; N]         (optional UTF-8 source)
line_map_count: u16
line_maps: [LineMapEntry]     (6 bytes each: function_id u16, bytecode_offset u16, source_line u16)
var_name_count: u16
var_names: [VarNameEntry]     (var_index u16, name_length u8, name bytes)
```

#### New Types

```rust
/// Parsed debug information from the container's debug section.
pub struct DebugInfo {
    pub source_text: Option<String>,
    pub line_maps: Vec<LineMapEntry>,
    pub var_names: Vec<VarNameEntry>,
}

pub struct LineMapEntry {
    pub function_id: u16,
    pub bytecode_offset: u16,
    pub source_line: u16,
}

pub struct VarNameEntry {
    pub var_index: u16,
    pub name: String,
}
```

#### Builder Changes

`ContainerBuilder` gains a `debug_info(DebugInfo)` method. When present, the container is serialized with the debug section and `flags` bit 1 is set.

#### Read Path

`Container::read_from()` checks `flags` bit 1. If set, it reads and parses the debug section. Debug info is stored in `Container.debug_info: Option<DebugInfo>`. If the debug section is malformed, it is silently discarded (non-fatal, per the container format spec).

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

    /// Debug info from the container (line maps, var names).
    debug_info: DebugInfo,
}
```

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

When paused, the debugger can inspect variables by name using the debug info:

```rust
impl DebugState {
    /// Returns all variables with their names and current values.
    pub fn inspect_variables(&self, variables: &VariableTable) -> Vec<NamedVariable> {
        self.debug_info
            .var_names
            .iter()
            .filter_map(|entry| {
                let value = variables.load(entry.var_index).ok()?;
                Some(NamedVariable {
                    name: entry.name.clone(),
                    index: entry.var_index,
                    value: value.as_i32(), // TODO: type-aware formatting
                })
            })
            .collect()
    }
}
```

**Type-aware formatting.** The debug section currently stores only variable names, not types. The type section in the container has `VarEntry.var_type` (u8 type code) and `VarEntry.flags`. The variable inspector should use this to display values correctly (e.g., `REAL` as float, `BOOL` as TRUE/FALSE, signed vs unsigned integers). This requires passing the type section to the inspector.

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
| `stackTrace` | Return current function + source line from debug info; future: full call stack |
| `scopes` | Return "Locals" and "Globals" scopes |
| `variables` | Read variable values from VariableTable, format with type info |
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

**Goal:** Compile programs with debug info (line maps + variable names) and verify via the disassembler.

**Changes:**

| Crate | Files | Changes |
|-------|-------|---------|
| `container` | `container.rs`, new `debug_info.rs` | Add `DebugInfo`, `LineMapEntry`, `VarNameEntry` types; serialize/deserialize debug section; add `debug_info` field to `Container` |
| `container` | `builder.rs` | Add `debug_info()` method to `ContainerBuilder` |
| `container` | `header.rs` | Write `debug_section_offset` and `debug_section_size` when debug section present; set flags bit 1 |
| `codegen` | `emit.rs` | Add `mark_source_line()`, `line_map()` methods to `Emitter`; track line map entries alongside bytecode |
| `codegen` | `compile.rs` | Call `mark_source_line()` before each statement's bytecode; collect var name mappings; pass `DebugInfo` to `ContainerBuilder` |
| `codegen` | `compile.rs` | Accept source text for line number computation (byte offset → line) |
| `plc2x` | `disassemble.rs` | Display debug section in disassembly output (line maps, variable names) |

**Tests:**
- Codegen: compile a program, verify line map entries map to expected source lines
- Container: roundtrip test — write debug section, read it back, verify contents
- Container: read container without debug section — `debug_info` is `None`
- Container: read container with malformed debug section — silently discarded

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

This spec uses the debug section format already defined in the container format spec. No changes to the container format are required. The debug section is independently hashed and signed (via `debug_hash` and the debug signature section), so adding or removing debug info does not affect the content signature.

The VM already handles the debug section in its loading sequence (step 13 in the container format spec): if present and valid, load it; if invalid, discard it silently.

## Out of Scope

1. **Remote debugging** — DAP over TCP to debug programs on remote targets (embedded PLCs). The initial implementation uses stdin/stdout only.
2. **Multi-file debugging** — debugging programs that span multiple source files. The initial implementation assumes a single source file per container.
3. **Disassembly view** — showing bytecode in the debug UI when source is not available. The existing `ironplc/disassemble` LSP command is separate.
4. **Memory breakpoints** — breaking when a specific variable's value changes (hardware watchpoints). These require polling or page-fault tricks that are not practical in the VM.
5. **Time-travel debugging** — recording and replaying execution. Would require snapshotting VM state at each scan cycle, which is a significant memory and performance cost.
