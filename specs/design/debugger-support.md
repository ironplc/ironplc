# Spec: Debugger Support

## Overview

This spec defines the debugger architecture for IronPLC — the full stack from debug info in compiled bytecode through the VM debug engine to VS Code integration via the Debug Adapter Protocol (DAP). The goal is interactive debugging of IEC 61131-3 programs: set breakpoints on source lines, step through code, inspect variables, and control scan cycle execution.

This spec builds on:

- **[Bytecode Container Format](bytecode-container-format.md)**: Debug section format (line maps, variable names)
- **[Bytecode Instruction Set](bytecode-instruction-set.md)**: The instructions the debug engine must understand for stepping
- **[Runtime Execution Model](runtime-execution-model.md)**: VM lifecycle, scan cycle phases, and the diagnostic interface this debug interface extends

## Design Goals

1. **Source-level debugging** — breakpoints, stepping, and variable inspection use source line numbers and variable names, not bytecode offsets and indices
2. **Instruction-level pausing** — the VM can pause between any two bytecode instructions, including inside nested CALLs and inside multi-statement expressions, and resume exactly where it left off
3. **Scan-cycle-aware** — the debugger understands PLC scan cycle semantics; users can pause between cycles, step one cycle at a time, or break mid-cycle
4. **Zero overhead when disabled** — when no debugger is attached, the VM runs at full speed; the debug hook compiles to nothing via monomorphization
5. **Separable debug info** — the debug section can be stripped from production containers without affecting execution; the debugger loads it separately if needed
6. **Standard protocol** — use the Debug Adapter Protocol (DAP) so any DAP-compatible editor can debug IronPLC programs, not just VS Code

### Execution-model commitment

Goal #2 forces a single design decision that drives the rest of this spec. Today `execute_with_hook` (`compiler/vm/src/vm.rs`) implements PLC `CALL` by recursing into itself, so the Rust call stack *is* the PLC call stack. That architecture cannot pause mid-CALL: a `Paused` return value from the innermost frame leaves the operand-stack contents, locals, pc, and the chain of Rust frames stranded, with no way to resume.

This spec therefore commits to converting the VM to a **non-recursive (iterative) dispatch** with an **explicit, embedder-provided frame stack**. Every PLC `CALL` pushes a frame; every `RET` pops one; the dispatch loop runs until the frame stack is empty or a debug pause is requested. The frame stack is the single piece of state that has to be checkpointed at a pause and restored at a resume — no Rust frames are involved.

**No heap allocation.** The frame stack follows the same pattern the VM already uses for `OperandStack`, `VariableTable`, `data_region`, and `temp_buf` (`compiler/vm/src/stack.rs:5`, `variable_table.rs:42`): a borrowed `&mut [Frame]` slice owned by the embedder. The compiler already computes the worst-case call depth at compile time — IEC 61131-3 forbids recursion, so the call graph is a DAG and its longest path is the bound — and writes it to the container header as `max_call_depth: u16` (`compiler/container/src/header.rs:57`). The embedder reads that field, allocates a buffer of exactly that size, and hands it to the VM:

- **Hosted (LSP, vm-cli, playground):** `Vec<Frame>` of length `header.max_call_depth`.
- **`no_std` / Arduino-class:** `heapless::Vec<Frame, N>` or a fixed `[MaybeUninit<Frame>; N]` where `N` is a const upper bound chosen at firmware build time. A program whose `header.max_call_depth` exceeds the embedder's `N` is rejected at load with a clear error, the same way the operand stack and variable table already are.

This preserves the no-heap, no-`alloc` execution profile that the existing VM already supports (see `2026-04-XX-no-std-vm-impl.md`). The debugger adds no new allocation requirement.

This is the largest change in the plan. It must land before any breakpoint, step, or pause feature can work; Phase 2 cannot be skipped or reduced. The alternative — a "scan-boundary-only" debugger — is explicitly rejected because it cannot offer breakpoints in the middle of a function body, which is the headline debugger feature.

## v1 Scope Decisions

The v1 debugger deliberately drops three features from the architecture in order to ship a smaller, more useful first experience. Each cut is paired with a replacement or a documented restriction:

| Dropped from v1 | Reason | Replacement |
|-----------------|--------|-------------|
| **Variable forcing** (write to variable while paused) | The industrial-PLC mental model of "force" is "value held across scans." A simple paused-only write is silently overwritten on the next INPUT_FREEZE / program logic, which trains users that the debugger is broken. The full force-table semantics that match Codesys/Beckhoff/TwinCAT need real design and are out of v1 scope. | **Logpoints**: a breakpoint that, instead of pausing, formats a message against current variables and writes to the debug console, then continues. Reuses the line map, breakpoint table, and variable lookup. Single most useful obs feature for scan-cycle code. |
| **Multi-instance pause semantics** | "Global breakpoints fire on the first instance to arrive" + "instances `k+1..` haven't run this scan; their state is from the previous cycle" is going to confuse every user who tries it. The implementation cost is also non-trivial (mid-scan resume across `instances_for_task`, custom `ironplc/instances` request). | **Refuse the launch**: `launch` rejects programs with `program_instances.len() > 1` with a clear error pointing at this v1 limitation. Multi-instance debugging is a future phase with its own design. |
| **Pause-while-running (interactive `pause`)** | The `ArcSwap<BreakpointTable>` + `AtomicBool pause_requested` + two-thread DAP server is the heaviest architectural piece in the design and gates DAP server delivery on getting Send/Sync right. For a first debugger, "interrupt a running scan from a button" is a corner case; the common path is set-breakpoints-then-launch. | **Single-threaded DAP loop**: the VM runs to a natural stop point (breakpoint, step landing, scan boundary, completion) and *then* the DAP loop services queued requests synchronously. `pause` returns `requestNotApplicable`. `setBreakpoints` is processed at the next natural stop. Runaway prevention uses the existing `scanLimit` launch option. |

These three cuts are reflected throughout the rest of this document. Each section lists what is in v1 and what is deferred.

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
                            │   DebuggerHook     │   ironplc-vm crate
                            │  (impls DebugHook) │
                            │  BreakpointTable   │
                            │  StepController    │
                            │  LogpointTable     │
                            └────────┬──────────┘
                                     │ DebugHook trait
                                     ▼
                            ┌───────────────────┐
                            │   Iterative VM     │   restructured VM
                            │  dispatch loop     │   explicit FrameStack
                            │  yieldable/        │   pausable between
                            │  resumable         │   any two opcodes
                            └───────────────────┘
```

The debugger is four layers:

| Layer | Location | Responsibility |
|-------|----------|----------------|
| **Debug info** | `codegen` crate + `container` crate | Emit and parse line maps, variable names in the debug section |
| **VM execution model** | `vm` crate | Iterative dispatch with an explicit `FrameStack`; pausable/resumable. Existing recursive `execute_with_hook` is retired. |
| **VM debug engine** | `vm` crate | A `DebugHook` implementation (`DebuggerHook`) that holds the breakpoint table, step controller, and logpoint table; tracks call depth via `before_call`/`after_return` callbacks |
| **DAP server** | `vm-cli` crate (feature-gated) | Translate DAP protocol messages to VM debug engine API calls |
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

The debug section uses a **tagged sub-table** layout. A directory at the start lists every sub-table by type tag and byte size. A reader skips unknown tags by size, so future sub-tables (LD rung maps, FBD network maps, source file tables) can be added without breaking existing readers.

#### Section Layout

```
┌──────────────────────────────────────────────────────────────┐
│ Debug Section Header                                          │
│   sub_table_count: u16                                       │
│   directory: [SubTableEntry; sub_table_count]                │
├──────────────────────────────────────────────────────────────┤
│ Sub-table payloads (concatenated, in directory order)         │
│   payload[0]: [u8; directory[0].size]                        │
│   payload[1]: [u8; directory[1].size]                        │
│   ...                                                         │
│   payload[N-1]: [u8; directory[N-1].size]                    │
└──────────────────────────────────────────────────────────────┘
```

Each SubTableEntry (8 bytes):

| Offset | Field | Type | Description |
|--------|-------|------|-------------|
| 0 | tag | u16 | Sub-table type identifier (see tag registry below) |
| 2 | _reserved | u16 | Must be zero |
| 4 | size | u32 | Size of this sub-table's payload in bytes |

To find the data for sub-table at directory index `i`, compute:

```
payload_offset = 2 + (8 × sub_table_count) + sum(directory[0..i].size)
```

#### Tag Registry

| Tag | Name | Status | Description |
|-----|------|--------|-------------|
| 0 | SOURCE_TEXT | v1 | Embedded source text (UTF-8) |
| 1 | LINE_MAP | v1 | Bytecode offset → source line/column mappings |
| 2 | VAR_NAME | v1 | Variable names with scope and type metadata |
| 3 | FUNC_NAME | v1 | Function/POU name mappings |
| 4 | FB_TYPE_NAME | Phase 5 | FB type ID → type name mappings |
| 5 | FB_FIELD_NAME | Phase 5 | FB field index → field name mappings |
| 6 | SOURCE_FILE | reserved | Source file table for multi-file projects |
| 7 | LD_RUNG_MAP | reserved | Ladder Diagram rung ID → bytecode mappings |
| 8 | FBD_NETWORK_MAP | reserved | Function Block Diagram network/element mappings |
| 9 | ENUM_DEF | implemented | Enumeration type → ordinal-ordered value names (`compiler/container/src/debug_section.rs`) |
| 10–65535 | — | reserved | Future use |

**Rules:**
- Each tag may appear **at most once** in the directory. A reader that encounters a duplicate tag discards the debug section.
- Tags may appear in **any order**. Readers must not assume a specific ordering.
- A reader that encounters an unknown tag **skips it** using the `size` field. This is the core extensibility mechanism.
- A required tag that is missing is treated as an empty table (count = 0).

#### Why a directory instead of sequential parsing?

The original container format spec defined sub-tables as a flat sequence: source text, then line maps, then variable names. This requires parsing every byte of every preceding table to find the one you want. Worse, adding a new table type (like an LD rung map) forces every existing reader to either understand the new format or fail — there's no way to skip variable-size tables without knowing their internal structure.

The directory solves both problems. The `size` field on each entry lets a reader jump past any sub-table it doesn't understand. The `tag` field lets it find the sub-tables it cares about regardless of order. This means:

- An ST debugger ignores tag 7 (LD_RUNG_MAP) — it doesn't even need to know what's inside.
- An LD debugger ignores tag 1 (LINE_MAP) if it uses rung maps instead.
- A minimal reader that only needs variable names scans the directory for tag 2 and jumps directly to it.

#### Sub-table Payload Formats

Each sub-table payload starts with its own item count, followed by the items. This is self-contained — the payload is parseable without the directory (the directory provides the size for skip-ability, but the count provides the item count for parsing).

**Tag 0 — SOURCE_TEXT:**

| Offset | Field | Type | Description |
|--------|-------|------|-------------|
| 0 | text | [u8; size] | UTF-8 source text (the entire payload is the text; size comes from the directory) |

**Tag 1 — LINE_MAP:**

| Offset | Field | Type | Description |
|--------|-------|------|-------------|
| 0 | count | u16 | Number of entries |
| 2 | entries | [LineMapEntry; count] | 8 bytes each |

**Tag 2 — VAR_NAME:**

| Offset | Field | Type | Description |
|--------|-------|------|-------------|
| 0 | count | u16 | Number of entries |
| 2 | entries | [VarNameEntry; count] | Variable size each |

**Tag 3 — FUNC_NAME:**

| Offset | Field | Type | Description |
|--------|-------|------|-------------|
| 0 | count | u16 | Number of entries |
| 2 | entries | [FuncNameEntry; count] | Variable size each |

**Tags 4, 5 — FB_TYPE_NAME, FB_FIELD_NAME:**

Same pattern (count + entries). Payloads are 0 bytes (count = 0) in v1.

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
| 5 | iec_type_tag | u8 | IEC 61131-3 type tag for value interpretation (see [ADR-0019](../adrs/0019-type-encoding-in-debug-variable-names.md)) |
| 6 | name_length | u8 | Length of variable name in bytes |
| 7 | name | [u8; name_length] | UTF-8 variable name |
| 7+N | type_name_length | u8 | Length of type name in bytes |
| 8+N | type_name | [u8; type_name_length] | UTF-8 type name (e.g., "DINT", "REAL", "TON") |

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

The DAP server scans the debug section directory for the tags it needs and ignores the rest. The following table traces each DAP request to the sub-table tag it reads:

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

`Container::read_from()` checks `flags` bit 1. If set, it reads the debug section directory, then parses sub-tables for known tags and skips unknown tags by size. Debug info is stored in `Container.debug_info: Option<DebugInfo>`. If the directory is malformed (e.g., a sub-table's size extends past the section boundary, or a duplicate tag appears), the entire debug section is silently discarded (non-fatal, per the container format spec).

A reader that does not find a particular tag treats that sub-table as empty (count = 0). This provides forward compatibility: older containers (with fewer tags) work with newer debuggers, and newer containers (with extra tags) work with older debuggers.

### Source Text Embedding

The debug section optionally embeds the source text. This enables debugging even when the original `.st` file is not available (e.g., debugging on a remote target). The source text is included by default in debug builds and excluded in release builds. This is a compiler flag, not a container format decision.

## Layer 2: VM Debug Engine

### Overview

Layer 2 has two parts. **Part A** restructures the VM dispatch loop from recursion to an iterative loop over an explicit `FrameStack`. This is a prerequisite — without it, no instruction-level pause is possible. **Part B** introduces a `DebuggerHook` that implements the existing `DebugHook` trait (`compiler/vm/src/debug_hook.rs`) extended with the callbacks the debugger needs, and wires it into the iterative loop.

The existing `NoopDebugHook` plus monomorphization already provides "zero overhead when disabled"; we keep that mechanism rather than replacing it with a runtime `Option<&mut DebugState>` branch. The plan reuses, not retires, the trait.

### Part A — Iterative dispatch with an explicit frame stack

#### Why iterative

`execute_with_hook` recurses on every PLC `CALL` (`vm.rs:1044`, `vm.rs:1865`). To pause execution between any two opcodes — including opcodes inside a callee — the dispatch loop must be able to *return* control to the DAP server while leaving every PLC frame intact and resumable. With Rust-stack recursion this is impossible: returning unwinds frames, and resume cannot push them back.

The fix is the standard interpreter pattern: store frames on a `Vec<Frame>`, pop the topmost frame's `pc` into a local register, dispatch one opcode, write `pc` back, and loop. `CALL` pushes a frame; `RET` pops one; the loop terminates when the frame stack becomes empty (program done) or a pause is requested (suspended; can resume).

#### Frame layout

```rust
/// One PLC call frame on the explicit frame stack.
/// `Copy` so the frame stack can live in `[MaybeUninit<Frame>; N]` on no_std.
#[derive(Clone, Copy)]
pub struct Frame {
    /// The function this frame is executing.
    pub function_id: FunctionId,
    /// Byte offset within `function`'s bytecode of the *next* opcode to execute.
    pub pc: u32,
    /// Variable scope for this frame (locals + globals view).
    pub scope: VariableScope,
    /// Operand-stack height when this frame was pushed; used by RET to
    /// restore the caller's stack and detect stack imbalance bugs.
    pub stack_floor: u32,
    /// Temp-buffer allocator checkpoint at frame entry; restored on RET.
    pub temp_alloc_mark: TempAllocMark,
    /// The instance this frame belongs to. v1 only ever has one instance
    /// (see §Multi-instance: not supported in v1) so this is always the
    /// sole instance's id; the field is kept on `Frame` for forward
    /// compatibility with Phase 6 multi-instance debugging.
    pub instance_id: InstanceId,
}
```

`Frame` is plain-old-data so it costs nothing to copy and can be stored in a fixed-size array on `no_std` targets.

#### FrameStack: borrowed slice, no heap

The frame stack is a borrowed slice plus a length, mirroring `OperandStack` and `VariableTable`:

```rust
pub struct FrameStack<'a> {
    slots: &'a mut [Frame],
    len: u16,
}

impl<'a> FrameStack<'a> {
    /// `backing` must be at least `header.max_call_depth` Frames long.
    pub fn new(backing: &'a mut [Frame]) -> Self { ... }

    pub fn push(&mut self, frame: Frame) -> Result<(), Trap> {
        if (self.len as usize) >= self.slots.len() {
            return Err(Trap::CallStackOverflow);
        }
        self.slots[self.len as usize] = frame;
        self.len += 1;
        Ok(())
    }

    pub fn pop(&mut self) -> Option<Frame> { ... }
    pub fn top(&self) -> Option<&Frame> { ... }
    pub fn top_mut(&mut self) -> Option<&mut Frame> { ... }
    pub fn len(&self) -> u16 { self.len }
    pub fn is_empty(&self) -> bool { self.len == 0 }
}
```

The slice may be backed by `Vec<Frame>`, `[Frame; N]`, `heapless::Vec<Frame, N>`, or any other contiguous storage. `FrameStack::new` does no allocation. The bound is enforced at runtime via `Trap::CallStackOverflow` (the existing trap), which fires both when the firmware-side const `N` is too small *and* when a malicious container under-reports `max_call_depth`. The constant `MAX_CALL_DEPTH = 32` in `vm.rs:31` is removed in favor of the per-program bound.

#### Container-header bound is authoritative

The compiler computes the worst-case call depth via a topological walk of the call graph (already legal because IEC 61131-3 forbids recursion) and writes it to `FileHeader.max_call_depth`. The codegen change in Phase 2 is to ensure this field is populated for every program; today it defaults to 0. The verifier (`compiler/codegen/src/spec_conformance.rs` or equivalent) gains a check that:

- `header.max_call_depth ≥ longest_path(call_graph) + 1` (the +1 accounts for the entry frame).
- The call graph contains no cycles (already enforced upstream as the recursion ban; this is a defense-in-depth check).

Embedders read `header.max_call_depth` after loading the container and provision the frame slice. On `no_std` targets where the slice is a fixed `[Frame; N]`, the load fails with `LoadError::ProgramExceedsCallDepth { required, available }` if `header.max_call_depth > N`. This is the same policy the VM already uses for `max_stack_depth` against the operand-stack buffer.

#### Restructured execute

```rust
pub(crate) fn execute_with_hook<'a, H: DebugHook>(
    container: &Container,
    stack: &mut OperandStack<'a>,
    variables: &mut VariableTable<'a>,
    data_region: &mut [u8],
    temp_buf: &mut [u8],
    max_temp_buf_bytes: usize,
    frames: &mut FrameStack<'a>,   // borrowed; no heap involved
    current_time_us: u64,
    hook: &mut H,
) -> Result<ExecuteOutcome, Trap> {
    while let Some(top) = frames.top_mut() {
        let bytecode = container.code.get_function_bytecode(top.function_id)
            .ok_or(Trap::InvalidFunctionId(top.function_id))?;

        if top.pc >= bytecode.len() {
            return Err(Trap::PcOutOfBounds);
        }

        let op = bytecode[top.pc];
        // Hook fires *before* pc advances so the hook sees the opcode's offset.
        match hook.before_instruction(top.function_id, top.pc, op) {
            HookAction::Continue => {}
            HookAction::Pause(reason) => return Ok(ExecuteOutcome::Paused(reason)),
        }
        top.pc += 1;

        match op {
            opcode::CALL => {
                let func_id = read_u16_le(bytecode, &mut top.pc)?;
                let new_frame = build_callee_frame(container, stack, variables, func_id, top)?;
                hook.before_call(top.function_id, new_frame.function_id);
                frames.push(new_frame)?;   // Trap::CallStackOverflow if MAX exceeded
            }
            opcode::RET | opcode::RET_VOID => {
                let returning = frames.pop().expect("non-empty by while-let");
                temp_alloc.rewind_to(returning.temp_alloc_mark);
                hook.after_return(returning.function_id,
                                  frames.top().map(|f| f.function_id));
                // Caller's frame, if any, resumes on the next loop iteration.
            }
            // ... all other opcodes operate on the borrowed `top` ...
        }
    }
    Ok(ExecuteOutcome::Completed)
}
```

Two properties matter: every observable state transition (pc bump, frame push/pop) is in this loop, never on the Rust stack; and a `HookAction::Pause` return immediately exits the loop with all frames intact. A subsequent call to `execute_with_hook` with the same `frames` resumes exactly where it stopped.

#### `ExecuteOutcome`

```rust
pub enum ExecuteOutcome {
    /// Frame stack drained — program reached the entry function's RET.
    Completed,
    /// The hook requested a pause. Frames + operand stack + variables are
    /// preserved verbatim; another execute_with_hook call resumes.
    Paused(PauseReason),
}
```

A trap remains an `Err(Trap)` and is *not* a Paused result. The outer scan-cycle loop converts a trap into either a fault (no debugger) or a "trap pause" (debugger present and trap-breakpoints enabled — see §Trap Breakpoints).

#### Operand-stack invariants under pause

A pause can land mid-expression, with partial values on the operand stack. That is fine: the operand stack lives in `VmRunning` (not on the Rust stack) and is preserved across a Pause/Resume. The DAP server **must not** mutate the operand stack while paused — `variables` and the frame `pc` are the only legal write targets (and `variables` only via the controlled forcing API).

#### Migration path

The conversion is mechanical but large. It is done in three commits to keep each diff reviewable:

1. **Introduce `FrameStack` and route `CALL`/`RET` through it** without changing recursion. `execute_with_hook` still recurses, but the new frame stack is built and maintained in parallel for cross-checking. Add an internal assert that the explicit depth always matches Rust recursion depth.
2. **Flip the loop to iterative.** Remove the recursive `execute_with_hook(..., depth + 1, ...)` call inside the `CALL` arm; replace with `frames.push(...)`. Remove the `depth` parameter. Top-level callers gain a `frames: &mut FrameStack` argument.
3. **Delete the old depth bookkeeping** and the `depth` parameter chain.

After step 2 the VM is pausable; before step 2 it is not. Step 2 is the gate for Phase 2.

### Part B — Extending the DebugHook trait

The current trait (`compiler/vm/src/debug_hook.rs:35`):

```rust
pub trait DebugHook {
    fn before_instruction(&mut self, pc: usize, op: u8);
}
```

is insufficient for breakpoints (no `function_id`), pause-requests (no return value), or step-out (no CALL/RET notification). We extend it:

```rust
pub trait DebugHook {
    /// Called immediately before the opcode at `(function_id, pc)` executes.
    /// The hook may request a pause via the return value.
    fn before_instruction(
        &mut self,
        function_id: FunctionId,
        pc: usize,
        op: u8,
    ) -> HookAction;

    /// Called when a CALL is about to push a new frame for `callee`.
    /// `caller` is the function that issued the CALL.
    fn before_call(&mut self, caller: FunctionId, callee: FunctionId) {
        let _ = (caller, callee);
    }

    /// Called when a RET has popped `returning`'s frame; `caller` is the
    /// function the loop will resume into (None when returning from the
    /// entry function — i.e. ExecuteOutcome::Completed is next).
    fn after_return(&mut self, returning: FunctionId, caller: Option<FunctionId>) {
        let _ = (returning, caller);
    }
}

#[derive(Clone, Copy)]
pub enum HookAction {
    Continue,
    Pause(PauseReason),
}

#[derive(Clone, Copy, Debug)]
pub enum PauseReason {
    Breakpoint(BreakpointId),
    Step,
    // No `UserPause` variant in v1: the DAP `pause` request is unsupported
    // (see §Single-threaded DAP loop). All pause origins are VM-internal.
    Trap(Trap),
}
```

`before_call` and `after_return` have default empty bodies, so existing `NoopDebugHook` and any test hooks compile unchanged. The new return type on `before_instruction` is a breaking change to the trait — `NoopDebugHook::before_instruction` returns `HookAction::Continue` from a `#[inline(always)]` body, so monomorphization still folds the call away.

#### Why `before_call`/`after_return` instead of inferring from opcodes

The hook *could* infer call depth by counting `CALL` and `RET` opcodes, but that fails for traps (frame popped without RET) and for any future tail-call optimization. Explicit callbacks keep the depth model stable across these cases.

### DebuggerHook (the actual debugger implementation of DebugHook)

```rust
/// The DAP server's DebugHook. Holds the breakpoint table, step controller,
/// step controller, logpoint table, and a reference to the debug info needed
/// to resolve source locations.
pub struct DebuggerHook<'a> {
    breakpoints: &'a BreakpointTable,
    logpoints: &'a LogpointTable,
    step: StepController,
    debug_info: &'a DebugInfo,
    log_sink: &'a mut dyn LogSink,   // writes formatted logpoint output
    /// PLC call depth, maintained via before_call/after_return.
    depth: u16,
}

impl<'a> DebugHook for DebuggerHook<'a> {
    fn before_instruction(
        &mut self,
        function_id: FunctionId,
        pc: usize,
        _op: u8,
    ) -> HookAction {
        if let Some(bp_id) = self.breakpoints.hit(function_id, pc as u16) {
            return HookAction::Pause(PauseReason::Breakpoint(bp_id));
        }
        if let Some(lp) = self.logpoints.hit(function_id, pc as u16) {
            // Logpoints format and continue — they never pause.
            self.log_sink.emit(lp.format(self.debug_info /*, &VariableTable */));
        }
        if let Some(reason) = self.step.check(function_id, pc as u16, self.depth, self.debug_info) {
            return HookAction::Pause(reason);
        }
        HookAction::Continue
    }
    fn before_call(&mut self, _caller: FunctionId, _callee: FunctionId) {
        self.depth = self.depth.saturating_add(1);
    }
    fn after_return(&mut self, _returning: FunctionId, _caller: Option<FunctionId>) {
        self.depth = self.depth.saturating_sub(1);
    }
}
```

`StepController` keeps the origin line and origin depth and decides when stepping ends. `BreakpointTable.hit()` returns `Option<BreakpointId>` keyed by `(function_id, bytecode_offset)`. `LogpointTable.hit()` is the same shape and returns `Option<&Logpoint>` whose `format()` interpolates current variable values into a stored format string.

There is no `pause_requested` flag in v1: the DAP `pause` request is not supported (see §v1 Scope Decisions). All pause transitions originate inside the VM thread itself — breakpoints, step-mode landings, scan-step boundaries, or traps.

### Breakpoint Table

```rust
/// Fast breakpoint lookup, keyed by (FunctionId, bytecode_offset).
/// Backed by a sorted Vec; linear scan beats HashMap at typical breakpoint
/// counts (<100) due to cache locality.
///
/// v1: the table is owned by the DAP server and only mutated at "natural
/// stop points" (paused VM, scan boundary, completion, breakpoint hit).
/// While the VM is running, the DebuggerHook holds a `&BreakpointTable`
/// reference and reads it without locking — no cross-thread mutation.
/// See §Single-threaded DAP loop.
pub struct BreakpointTable {
    entries: Vec<BreakpointEntry>,
}

struct BreakpointEntry {
    function_id: FunctionId,
    bytecode_offset: u16,
    id: BreakpointId,
    enabled: bool,
    /// Optional logpoint — when set, the hook formats this against current
    /// variables and writes to the log sink instead of pausing.
    log_message: Option<String>,
}

pub type BreakpointId = u32;
```

**Breakpoint resolution.** The DAP client sends breakpoints as source line numbers. The DAP server resolves each line to the nearest `LineMapEntry` with `source_line >= requested_line` for the appropriate function, **after** optimization (see §Optimizer Contract). The resolved line is reported back in the DAP `Breakpoint` response so the client highlights the actual stop line, which may differ from the requested line.

**Logpoint vs. breakpoint.** A breakpoint with a `log_message` is a logpoint: it never pauses, it only formats and emits. Internally it lives in the same sorted Vec as breakpoints, but the `DebuggerHook` checks `log_message` first and continues without returning `HookAction::Pause`. This means setting a logpoint costs the same as setting a breakpoint and uses the same `setBreakpoints` DAP request.

### Step Modes

```rust
enum StepMode {
    /// Step to the next statement on a different source line at *equal or
    /// shallower* call depth (does not descend into callees). DAP "next".
    StepOver,
    /// Step to the next statement on a different source line, allowing
    /// deeper call depth. DAP "stepIn".
    StepIn,
    /// Run until call depth is strictly less than the origin depth.
    /// DAP "stepOut".
    StepOut,
    /// Run until the next scan cycle boundary. Custom DAP request.
    StepScan,
}
```

`StepController::check` consumes the depth maintained by `before_call`/`after_return` — not a depth inferred from the operand stack — so it stays correct across traps that abort frames.

### Pause/Resume protocol

A pause leaves the VM in a well-defined `PausedAt(PauseReason)` sub-state inside `VmRunning`. **No allocation happens at pause or resume** — every piece of preserved state already lives in embedder-provided buffers (operand stack, variable table, data region, temp-buf, frame stack). Pause is just a state-flag flip plus a return out of the dispatch loop; resume is just a re-entry into it. The complete checkpoint is:

| State | Owner | Why preserved |
|-------|-------|---------------|
| Frame stack | `VmRunning.frames` | Resume position |
| Operand stack | `VmRunning.stack` | Mid-expression values |
| Variable table | `VmRunning.variables` | All locals, globals, FB instances |
| Data region | `VmRunning.data_region` | String contents, large composites |
| Temp-buf allocator state | `VmRunning.temp_alloc` | Per-frame allocations |
| Scan cycle phase | `VmRunning.phase` | Whether INPUT_FREEZE happened, whether OUTPUT_FLUSH is pending |
| Step controller | `DebuggerHook.step` | What the user was asking for |

Resume calls `execute_with_hook` with the same arguments. No reconstruction; nothing is rebuilt. **Operand-stack and frame-stack invariants** are tested by a property test that pauses at every opcode boundary in a corpus of programs and asserts that resume produces the same final state as an unpaused run.

### Multi-instance: not supported in v1

v1 supports debugging **single-instance programs only**. The DAP `launch` request rejects programs with `program_instances.len() > 1` with the error message:

> `MultiInstanceUnsupported: this program declares N program instances; the v1 debugger supports single-instance programs only. Multi-instance debugging is planned for a future phase.`

This is a **deliberate v1 cut** (see §v1 Scope Decisions). The half-implemented multi-instance pause behavior in earlier drafts of this spec — "global breakpoints fire on the first instance to arrive" plus "instances `k+1..` show stale state from the previous cycle" — would confuse every user who tried it. Refusing the launch is one explicit error message instead of a permanently confusing experience.

A future phase will add proper multi-instance debugging: per-instance breakpoint filters (`instance_filter` field on `BreakpointEntry`), DAP threads exposing each instance, `current_instance_id` tracking on pause, and mid-round resume across `instances_for_task`. That design has its own spec; it is not implied by anything in this document.

The VM's `run_round_debug` is therefore simplified for v1: there is no `current_instance_id` field, no mid-round resume across instance boundaries, and no `ironplc/instances` custom request. When the lone instance's frame stack drains, the round is complete.

### Single-threaded DAP loop (v1)

v1 uses a **single-threaded** DAP server. There is no I/O thread separate from the VM thread; one event loop alternates between two modes:

1. **Running mode**: the VM is executing under `run_round_debug`. The DAP server is not reading stdin. JSON input accumulates in the OS pipe buffer.
2. **Stopped mode**: the VM has reached a natural stop point (breakpoint hit, step landing, scan boundary, completion, trap). The loop drains any queued DAP requests and services them synchronously, then returns to running mode (or exits).

The natural stop points are:

| Stop reason | When |
|-------------|------|
| Breakpoint | `DebuggerHook` returns `HookAction::Pause(Breakpoint)` |
| Step landing | `DebuggerHook` returns `HookAction::Pause(Step)` |
| Scan boundary | `run_round_debug` returns `RoundOutcome::Completed` (one complete scan finished) |
| Step Scan landing | `run_round_debug` returns `RoundOutcome::PausedAfterScan` |
| Trap | `Err(Trap)` from the dispatch loop, with trap-bp enabled |
| Disconnect timer | `scanLimit` reached (launch-config; runaway prevention) |

**No pause-while-running.** The DAP `pause` request is **not supported in v1** — it returns `requestNotApplicable`. Users who want to interrupt a running program either set a breakpoint in advance, use a `scanLimit` to bound execution, or use `disconnect` to terminate. This is a deliberate v1 cut (see §v1 Scope Decisions); it eliminates the entire two-thread / `ArcSwap` / `AtomicBool` machinery from the original design.

**Implications for `setBreakpoints` while running.** A `setBreakpoints` request that arrives while the VM is in running mode is *queued* and processed at the next natural stop point. The VS Code UI shows the breakpoint as "pending" until then. In practice this is invisible because the VM's between-stop interval is short (one scan cycle) and most users set breakpoints either before launch or while paused.

**State sharing.** Because the VM thread *is* the DAP thread, no synchronization primitives are needed. The DAP server holds `&mut VmRunning` directly when stopped; while running, it holds nothing. There is no `Send`, no `Sync`, no `Arc`, no `AtomicBool`. The `vm` crate's `DebuggerHook` reads a plain `&BreakpointTable` reference owned by the DAP server.

```text
       ┌──────────────────────────────────────┐
       │ DAP server event loop (single thread)│
       │                                      │
       │  loop {                              │
       │    if stopped {                      │
       │      drain stdin → service requests  │
       │      handle continue/step/disconnect │
       │    }                                 │
       │    run_round_debug(&mut hook)        │
       │     ───► VM runs to next stop point  │
       │  }                                   │
       └──────────────────────────────────────┘
```

This is the same shape as the LSP server already uses. It also keeps the core `vm` crate `no_std`-clean: there is no Arc/Atomic/thread code in `vm` at all.

**Per-instruction cost of a real debugger session:** a sorted-Vec scan of the breakpoint table (~5 ns at 10 breakpoints) — and zero when no debugger is attached because `NoopDebugHook` is monomorphized. Lower than the original two-thread design because the `ArcSwap` load and `AtomicBool::swap` are gone.

**What v1 trades away.** `pause` while running, `setBreakpoints` taking effect mid-instruction. **What v1 keeps**: every other DAP request, all stepping modes, breakpoints, logpoints, scan stepping, variable inspection, trap pause. The cost is a corner-case UX limit; the savings are most of the architectural risk in the design.

### Scan Cycle Control

For PLC-specific debugging, users need scan-level control:

| Command | Behavior |
|---------|----------|
| **Step Scan** | Run one complete scan cycle (INPUT_FREEZE → EXECUTE → OUTPUT_FLUSH), then pause before the next |
| **Pause Between Scans** | Always pause after OUTPUT_FLUSH, before the next INPUT_FREEZE |
| **Run to Scan N** | Continue until `scan_count` reaches a target value |

A new `VmRunning::run_round_debug` method drives a single round under a `DebuggerHook`. v1 supports a single program instance only (see §Multi-instance: not supported in v1), so the `instances_for_task` loop is collapsed to "the one instance":

```rust
impl<'a> VmRunning<'a> {
    pub fn run_round_debug<H: DebugHook>(
        &mut self,
        current_time_us: u64,
        hook: &mut H,
    ) -> Result<RoundOutcome, FaultContext> {
        // collect_ready_tasks, INPUT_FREEZE, system-uptime injection — unchanged

        // v1: exactly one program instance; the launch precondition rejects
        // multi-instance programs.
        let pi = self.sole_instance();
        if self.frames.is_empty() {
            self.frames.push(entry_frame_for(pi))?;
        }

        match execute_with_hook(self.container, &mut self.stack,
                                &mut self.variables, self.data_region,
                                self.temp_buf, self.max_temp_buf_bytes,
                                &mut self.frames, current_time_us, hook)? {
            ExecuteOutcome::Completed => { /* frames drained: instance done */ }
            ExecuteOutcome::Paused(reason) => {
                // Mid-instance pause: frames non-empty marks where we will
                // resume. Outputs are NOT flushed.
                return Ok(RoundOutcome::Paused(reason));
            }
        }

        // OUTPUT_FLUSH and scan_count++ run only when the instance completed.
        self.flush_outputs();
        self.scan_count += 1;
        if hook.took_step_scan_now() {
            return Ok(RoundOutcome::PausedAfterScan);
        }
        Ok(RoundOutcome::Completed)
    }
}
```

`run_round_debug` is the single re-entry point: a paused VM resumes by calling it again. The non-empty `self.frames` is how we know we're resuming.

### Variable Inspection

When paused, the debugger inspects variables using debug info plus the topmost frame's `function_id`:

```rust
impl DebuggerHook<'_> {
    pub fn inspect_variables(
        &self,
        variables: &VariableTable,
        function_id: FunctionId,
        scope_filter: ScopeFilter,
    ) -> Vec<NamedVariable> {
        self.debug_info.var_names.iter()
            .filter(|entry| {
                let owned = entry.function_id == function_id
                    || entry.function_id == FunctionId::GLOBAL_SCOPE;
                owned && scope_filter.matches(entry.var_section)
            })
            .filter_map(|entry| {
                let slot = variables.load(entry.var_index).ok()?;
                Some(NamedVariable {
                    name: entry.name.clone(),
                    type_name: entry.type_name.clone(),
                    var_section: entry.var_section,
                    index: entry.var_index,
                    value: format_value(slot, entry.iec_type_tag),
                })
            })
            .collect()
    }

    pub fn function_name(&self, function_id: FunctionId) -> Option<&str> {
        self.debug_info.func_names.iter()
            .find(|f| f.function_id == function_id)
            .map(|f| f.name.as_str())
    }
}
```

The "global" sentinel is `FunctionId::GLOBAL_SCOPE` (defined in `compiler/container/src/id_types.rs`), not a magic `0xFFFF`. Specs and tests must use the constant.

**Type-aware formatting.** The `VarNameEntry.iec_type_tag` field provides the numeric IEC type tag (see [ADR-0019](../adrs/0019-type-encoding-in-debug-variable-names.md)). The `format_value()` function matches on this tag to render slot values correctly — no string parsing needed:

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

### Variable forcing: not in v1

Variable forcing is **deliberately out of v1 scope** (see §v1 Scope Decisions). A simple paused-only write that gets overwritten on the next INPUT_FREEZE / program logic teaches users that the debugger is broken. A correct implementation needs a force-table (re-applied at each INPUT_FREEZE, persisted across scans, surfaced in the UI as "forced") plus the matching DAP UX, plus an unforce request — that is its own design and will be specified separately.

The DAP capabilities advertised by v1 reflect this: `supportsSetVariable: false`. The `ironplc/forceVariable` and `ironplc/unforceVariable` custom requests are removed from v1.

### Logpoints (replaces Variable Forcing in v1)

A logpoint is a breakpoint that, instead of pausing, formats a message against current variables and writes it to the DAP debug console — then continues. v1 logpoints are the v1 replacement for variable forcing as the headline observability feature: they are the way a user observes scan-cycle behavior without breaking timing.

#### DAP wiring

Logpoints arrive on the standard `setBreakpoints` request via the per-breakpoint `logMessage` field (DAP 1.51+):

```jsonc
{
    "command": "setBreakpoints",
    "arguments": {
        "source": { "path": "...MAIN.st" },
        "breakpoints": [
            { "line": 12, "logMessage": "speed = {motor.speed}, err = {err}" }
        ]
    }
}
```

A breakpoint with `logMessage` set becomes a `BreakpointEntry { log_message: Some(...), .. }` in the table. The DAP server advertises `supportsLogPoints: true` in the `initialize` response.

#### Format string syntax

| Token | Meaning |
|-------|---------|
| `{name}` | Look up the bare identifier in the current frame's locals; fall back to globals; format using the variable's `iec_type_tag` (same as the Variables pane) |
| `{name.field}` | Field access on a struct or FB instance |
| `{name[N]}` | Constant subscript on an array (N is a decimal literal) |
| `{{` / `}}` | Literal `{` / `}` |
| anything else | Literal text |

The supported expressions are exactly the v1 `evaluate` subset (§Evaluate scope) — by design, so the same lookup path serves both `evaluate` and logpoints. Unsupported tokens render as `<unsupported: motor.speed * 2>` rather than failing the logpoint, so a typo never silently disables observability.

#### Implementation

`Logpoint::format(debug_info, variables)` walks the format string once, emits each `{...}` against the lookup path, and returns a `String`. The `DebuggerHook` calls it inline (see the implementation in §DebuggerHook above), writes the result to the `LogSink`, and returns `HookAction::Continue`. No pause occurs.

**Cost.** A logpoint adds the cost of one variable lookup per `{...}` token plus formatting — measured on the VM thread, but only when the logpoint actually fires. Between hits it is cheaper than a breakpoint because the table check is shared. Users can drop a logpoint into a 1 ms scan loop without breaking timing in practice; the plan documents this in `--help`.

**Why this is the v1 obs feature.** Pausing a 50 Hz scan with a regular breakpoint desynchronizes timers and I/O timing — you can't actually step through real PLC code without breaking the very behavior you're debugging. Logpoints let you observe without stopping, and they reuse the line map, breakpoint table, and variable lookup that v1 already needs. They cost a few hundred lines on top of the rest of the debugger.

### State machine

The VM's `Phase` enum gains paused sub-states. All DAP requests are evaluated against this enum and rejected when illegal.

```text
                    configurationDone
        READY ───────────────────────────► RUNNING ─────► (round done) ─┐
          ▲                                  │                          │
          │ disconnect                       │ breakpoint / step /      │
          │                                  │ step-scan                │
          │                                  ▼                          │
        EXITED                            PausedAt(reason) ◄────────────┘
                                              │  ▲
                                              │  │  (variables, scopes,
                                              │  │   stackTrace, evaluate,
                                              │  │   setBreakpoints)
                                              │  │
                                continue/next/stepIn/stepOut/stepScan
                                              │
                                              ▼ (re-enter run_round_debug)
                                          RUNNING

   Trap path:
        RUNNING ── trap ──► (debugger attached + trap-bp) ──► PausedAt(Trap)
                          ── (otherwise) ─────────────────► FAULTED ──► EXITED

   PausedAt(Trap) is a *terminal pause*: only inspection requests
   (variables, stackTrace, evaluate) and `disconnect` are legal. Continue
   and stepping are rejected. Disconnect transitions to EXITED.

   v1 omits a RUNNING → PausedAt edge for an interactive `pause` request:
   the DAP `pause` request is not supported. All pause transitions
   originate inside the VM thread (breakpoint, step, step-scan, trap).
```

The state diagram is the contract for §DAP Request Mapping below — every request lists the states in which it is legal.

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

The "Legal in" column lists the VM `Phase` values in which each request is accepted; requests in any other state return DAP error `requestNotApplicable`.

| DAP Request | Legal in | VM Debug Engine Action |
|-------------|----------|------------------------|
| `initialize` | (any) | Return capabilities |
| `launch` | READY | Load container, validate single-instance precondition (else `MultiInstanceUnsupported`), allocate VM |
| `setBreakpoints` | READY, PausedAt | Resolve source lines via line map; replace the `BreakpointTable`. Logpoints (entries with `logMessage`) and breakpoints share this request. **Queued while RUNNING** and applied at the next natural stop point (see §Single-threaded DAP loop). |
| `setExceptionBreakpoints` | READY, PausedAt | Toggle `traps` filter for trap pauses |
| `configurationDone` | READY | Start VM: transition READY → RUNNING, call `run_round_debug` |
| `threads` | RUNNING, PausedAt | One DAP thread for the single program instance (v1 hard limit: one instance, enforced at launch) |
| `stackTrace` | PausedAt | Walk `frames` top-to-bottom; for each frame produce `name = func_names[function_id]`, `line/column = line_map.lookup(function_id, pc)` |
| `scopes` | PausedAt | IEC-specific scopes, filtered by `var_section` of the topmost frame's `function_id` |
| `variables` | PausedAt | Read from `VariableTable`; format per `iec_type_tag` |
| `continue` | PausedAt (non-terminal) | Clear step mode; re-enter `run_round_debug` |
| `next` | PausedAt (non-terminal) | Set `StepMode::StepOver` (origin = current line, depth = current depth); re-enter |
| `stepIn` | PausedAt (non-terminal) | Set `StepMode::StepIn`; re-enter |
| `stepOut` | PausedAt (non-terminal) | Set `StepMode::StepOut`; re-enter |
| `pause` | — | **Not supported in v1.** Always returns `requestNotApplicable`. See §Single-threaded DAP loop and §v1 Scope Decisions. |
| `setVariable` | — | **Not supported in v1.** Variable forcing is deferred; see §Variable forcing: not in v1. |
| `disconnect` | (any) | Drop VM, exit |
| `evaluate` | PausedAt | Bare-identifier lookup in v1 (see §Evaluate scope below) |

**Terminal vs non-terminal pause.** `PausedAt(Trap(_))` is terminal: continue/step are rejected. Every other `PausedAt` is non-terminal.

#### Evaluate scope (v1 limit)

DAP `evaluate` is used for hovers, watches, and the debug console. Full expression evaluation is out of scope for v1 because it requires the parser plus a constant-folding evaluator over live VM state. v1 supports:

- Bare identifiers: `myVar`, `motor`
- Field access on identifiers: `motor.speed`, `state.flags.error`
- Constant subscripts: `vec[3]`, `motor.history[0]`

It does **not** support arithmetic, function calls, or non-constant subscripts. The DAP server returns DAP error `evaluateUnsupported` for unsupported forms with a message pointing at the unsupported token. A follow-up phase (out of v1) layers in a sandboxed expression evaluator that reuses the AST evaluator from the constant-folder.

### Custom DAP Requests

| Custom Request | Description |
|----------------|-------------|
| `ironplc/stepScan` | Run one complete scan cycle, then pause |
| `ironplc/scanCount` | Return current scan_count |

Removed from v1 (deferred):

- `ironplc/forceVariable` / `ironplc/unforceVariable` — variable forcing is out of v1 scope (see §Variable forcing: not in v1).
- `ironplc/instances` — multi-instance debugging is rejected at launch in v1 (see §Multi-instance: not supported in v1), so there is nothing to enumerate.

VS Code does not surface custom requests automatically. The extension registers VS Code commands that wrap each custom request and contributes them to the debug toolbar/menus via `package.json` `menus.debug/toolBar`. Without those contributions the requests are unreachable.

### Threading

(Already specified in Layer 2 §Single-threaded DAP loop.) Briefly: the DAP server and VM run in a single thread; the loop alternates between draining DAP requests at natural stop points and running the VM under `run_round_debug`. There is no `ArcSwap`, no `AtomicBool`, no `Send`/`Sync` boundary.

### Capabilities

The DAP server advertises these capabilities in the `initialize` response:

```json
{
    "supportsConfigurationDoneRequest": true,
    "supportsSingleThreadExecutionRequests": true,
    "supportsSetVariable": false,
    "supportsLogPoints": true,
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

Note that `supportsSetVariable` is **false** for v1 (variable forcing deferred — see §Variable forcing: not in v1) and `supportsLogPoints` is **true** (§Logpoints replaces forcing as the v1 obs feature). The `pause` request is omitted entirely and returns `requestNotApplicable` (see §Single-threaded DAP loop).

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

The phasing is reorganized so that the iterative-dispatch rewrite (the prerequisite for instruction-level debugging) is its own phase before any debug feature work. The order is: debug info → iterative VM → debug hook → DAP → VS Code.

### Phase 1: Debug Info Foundation

**Goal:** Compile programs with full debug info (line maps, variable names with scope/type, function names) and verify via the disassembler.

**Status:** Partially in place — `compiler/container/src/debug_section.rs` already defines `LineMapEntry` (with `source_column`), `VarNameEntry` (with `function_id`, `var_section`, `iec_type_tag`, `type_name`), `FuncNameEntry`, and `EnumDefEntry`, plus the directory/tag mechanism. Outstanding work: codegen wiring, optimizer round-trip, disassembler output.

**Changes:**

| Crate | Files | Changes |
|-------|-------|---------|
| `container` | `debug_section.rs` | Reconcile tag registry with this spec (Tag 9 = ENUM_DEF must be added to the table; Tag 4 = FB_TYPE_NAME). No on-disk format change. |
| `container` | `builder.rs` | Existing builder API; verify it accepts a fully populated `DebugSection`. |
| `container` | `header.rs` | Write `debug_section_offset` and `debug_section_size` when debug section present; set flags bit 1 |
| `codegen` | `emit.rs` | `set_current_span` per-statement (per `2026-04-07-debug-source-map-and-hook.md`); deduplicate consecutive identical spans |
| `codegen` | `compile.rs` | Build `LineOffsetTable` from source; collect `VarNameEntry` + `FuncNameEntry` during compilation; pass `DebugSection` to the container builder |
| `codegen` | `optimize.rs` | `optimize_with_source_map` — remap line-map offsets through the optimizer's old→new offset table; "snap forward" entries for removed instructions; **invariant test** that every line-map offset lands on an instruction boundary in the optimized stream |
| `plc2x` | `disassemble.rs` | Render line maps and variable names alongside disassembly |

**Tests** (in addition to the optimizer property tests in `2026-04-07-debug-source-map-and-hook.md`):
- Codegen: compile, verify line-map entries map to expected lines and columns
- Codegen: verify `VarNameEntry` carries the right `function_id`, `var_section`, `iec_type_tag`, and `type_name`
- Codegen: verify `FuncNameEntry` for the program's entry function
- Container: roundtrip with all sub-tables (line map, var name, func name, enum def)
- Container: read container without debug section — `debug_info` is `None`
- Container: malformed debug section — silently discarded
- Container: partial debug section — missing tables treated as empty

### Phase 2: Iterative VM dispatch (prerequisite)

**Goal:** Convert `execute_with_hook` from recursive to iterative. No new debugger features in this phase — the *only* observable change is that `CALL`/`RET` go through an explicit `FrameStack`. Behavior is identical to today.

**Changes:**

| Crate | Files | Changes |
|-------|-------|---------|
| `vm` | new `frame_stack.rs` | `Frame`, `FrameStack` (bounded by `MAX_CALL_DEPTH`), `TempAllocMark` |
| `vm` | `vm.rs` | Replace recursive `CALL` arm (`vm.rs:1044`, `vm.rs:1865`) with `frames.push(...)`; lift the dispatch loop to iterate `frames`; `execute_with_hook` becomes the loop body's wrapper. Remove `depth` parameter — depth is now `frames.len()`. Add `ExecuteOutcome::{Completed, Paused}` (Paused only reachable after Phase 3 extends the hook signature; for now it's unreachable but the type exists). |
| `vm` | `vm.rs` | `VmRunning<'a>` gains `frames: FrameStack<'a>`, `phase: Phase`. (No `current_instance_id`: v1 is single-instance — see §Multi-instance: not supported in v1.) The frame backing slice is supplied to `Vm::new()` alongside the existing operand-stack and variable-table backing slices; no allocation is added. `Phase::Running` is the only legal phase reached in this phase; `PausedAt` is added but only used in Phase 3. |
| `vm` | `vm.rs` | `Vm::load()` validates `header.max_call_depth ≤ frames.capacity()` and returns `LoadError::ProgramExceedsCallDepth` otherwise (matches the existing operand-stack/variable-table sizing checks). |
| `codegen` | `compile_fn.rs` / `compile.rs` | Compute worst-case call depth from the call graph (longest path; the recursion ban guarantees acyclicity) and write it to `FileHeader.max_call_depth`. |
| `codegen` | `spec_conformance.rs` | Verify the call graph is acyclic and that the emitted `max_call_depth` is at least the longest path + 1. |
| `vm` | `lib.rs` | Re-export `Frame`, `FrameStack`, `Phase`, `ExecuteOutcome` |

**Migration order** (each commit must compile and pass `cd compiler && just`):
1. Introduce `FrameStack`; maintain it in parallel with recursion; assert `frames.len() == depth + 1`.
2. Flip the loop iterative; remove the recursive `execute_with_hook(...)` self-call inside `CALL`.
3. Delete `depth` parameter and recursion-based bookkeeping.

**Tests:**
- All existing VM tests must pass unchanged (this phase is a no-op behavioural change).
- Frame-stack overflow returns `Trap::CallStackOverflow` exactly as before.
- New: pause-corpus test scaffolding — for every opcode in `MAX_OPCODE`, a fixture program where the VM can be paused at that opcode boundary by a *test* hook and resumed; the final `variables` and `data_region` are bitwise identical to an unpaused run. (The pause path is exercised by a synthetic hook; it does not yet need the DebuggerHook.)

### Phase 3: VM Debug Engine

**Goal:** With iterative dispatch in place, add the debugger-grade `DebugHook` extension and the `DebuggerHook` impl. The VM can pause at breakpoints, single-step (over/in/out/scan), inspect variables, and emit logpoints.

**Changes:**

| Crate | Files | Changes |
|-------|-------|---------|
| `vm` | `debug_hook.rs` | Extend trait: `before_instruction(function_id, pc, op) -> HookAction`; add `before_call`/`after_return` with default empty bodies; add `HookAction`, `PauseReason`. `NoopDebugHook` returns `HookAction::Continue` from `#[inline(always)]`. |
| `vm` | `vm.rs` (dispatch loop) | Inspect `HookAction` from `before_instruction`; call `before_call` before pushing a frame; call `after_return` after popping. Surface `ExecuteOutcome::Paused` from the loop. |
| `vm` | new `debug.rs` | `BreakpointTable` (plain sorted Vec — no `ArcSwap`, no atomics; the single-threaded DAP loop owns it directly), `BreakpointId`, `StepMode`, `StepController`, `Logpoint`, `LogpointTable`, `LogSink` trait, `DebuggerHook` (impls `DebugHook`). |
| `vm` | `vm.rs` | `VmRunning::run_round_debug<H: DebugHook>` — re-entrant; runs the (single) instance to a stop point and returns `RoundOutcome::{Completed, PausedAfterScan, Paused(reason)}`. `Phase::PausedAt(reason)` set on pause. **No `force_variable` API** (variable forcing deferred — see §Variable forcing: not in v1). |
| `vm` | `lib.rs` | Export `DebuggerHook`, `BreakpointTable`, `BreakpointId`, `StepMode`, `PauseReason`, `RoundOutcome`, `Logpoint`, `LogpointTable`, `LogSink`. |

**Explicitly NOT in this phase (deferred / cut):**
- `pause_requested: AtomicBool` and any cross-thread pause mechanism (see §Single-threaded DAP loop).
- `arc_swap` / `ArcSwap<BreakpointTable>` (same reason).
- `current_instance_id` / multi-instance pause/resume (see §Multi-instance: not supported in v1).
- `force_variable` API (see §Variable forcing: not in v1).

**Tests:**
- Breakpoint: set bp on a line in the entry function — pauses at the correct `(function_id, bytecode_offset)`.
- Breakpoint: set bp on a line **inside a callee** — pauses with the callee's frame on top of the frame stack; `stackTrace` walks back to the caller.
- Logpoint: set a logpoint with `"x = {x}, y = {y}"` on a hot statement — VM does not pause; `LogSink` captures one formatted message per hit; final variable values match an unhooked run.
- Logpoint: format string with unsupported token (`"{x * 2}"`) — emits `<unsupported: x * 2>` rather than failing the logpoint.
- Step-over across a CALL: at line with a CALL, step-over lands on the next line *after* the CALL with depth equal to origin.
- Step-in into a CALL: lands on the first statement of the callee with depth = origin + 1.
- Step-out from inside a callee: lands on the line after the CALL in the caller, depth = origin − 1.
- Pause/Resume parity: from a corpus of programs, pause at every instruction boundary, resume, assert final `variables` and `data_region` are bitwise identical to an unpaused run.
- No overhead with `NoopDebugHook`: a `criterion` bench shows < 1% delta vs main on the existing benchmark suite.
- Trap during debug: with trap-bp enabled, transitions to `PausedAt(Trap)`; only inspection requests are accepted; `continue`/`step` return `requestNotApplicable`; `disconnect` cleanly tears down.

### Phase 4: DAP Server

**Goal:** Launch `ironplcvm debug --dap <file.iplc>` and debug from VS Code using standard DAP.

**Packaging decision.** DAP support is **feature-gated** on the `vm-cli` crate (`--features dap`) so that the production VM binary doesn't pull in `serde_json`, the DAP types, and the I/O loop unless it's a debug build. Distribution ships two binaries: `ironplcvm` (no DAP) and `ironplcvm-debug` (DAP enabled), or one binary with the `dap` feature on by default in the VS Code distribution.

**Changes:**

| Crate | Files | Changes |
|-------|-------|---------|
| `vm-cli` | `Cargo.toml` | New `dap` feature gating the new modules below |
| `vm-cli` | `main.rs` | Behind `#[cfg(feature = "dap")]`: add `Debug` subcommand with `--dap` flag |
| `vm-cli` | new `dap/framing.rs` | Content-Length framing reader/writer |
| `vm-cli` | new `dap/types.rs` | DAP protocol types (Request, Response, Event, Capabilities). Prefer the `dap-types` crate if it's a fit; otherwise hand-rolled with `serde`. |
| `vm-cli` | new `dap/server.rs` | **Single-threaded** event loop: alternate between draining queued DAP requests at natural stop points and running the VM under `run_round_debug` (see §Single-threaded DAP loop). No I/O thread, no `Send`/`Sync`, no `Arc`, no `AtomicBool`. |
| `vm-cli` | new `dap/state.rs` | `Phase` mirror plus per-state legality checks (returns `requestNotApplicable` for illegal requests, including `pause` and `setVariable` which are unsupported in v1) |
| `vm-cli` | new `dap/launch.rs` | `launch` precondition: reject containers with multiple program instances (`MultiInstanceUnsupported`); reject containers without a debug section (`NoDebugInfo`) — see §Launch errors. |

**Tests:**
- Unit: Content-Length framing roundtrip.
- Unit: state-machine legality table — every (state, request) pair returns the documented response. Includes `pause` → `requestNotApplicable` and `setVariable` → `requestNotApplicable`.
- Unit: launch precondition for multi-instance program returns `MultiInstanceUnsupported` cleanly.
- Integration: spawn `ironplcvm-debug --dap` as a subprocess, send `initialize` + `launch` + `setBreakpoints` + `configurationDone`, expect `stopped` at the breakpoint.
- Integration: from `stopped`, request `stackTrace`, `scopes`, `variables`; verify entries.
- Integration: send `setBreakpoints` while RUNNING, verify it is queued and applied at the next breakpoint hit / scan boundary (no immediate effect mid-instruction — that's expected per §Single-threaded DAP loop).
- Integration: send `pause` while RUNNING, verify it returns `requestNotApplicable` (v1 cut).
- Integration: set a logpoint via `setBreakpoints` with `logMessage`, verify the VM does not pause and the formatted message appears as an `output` event.
- Integration: trigger a trap with trap-bp enabled, verify `stopped` event with `reason: exception` and `disconnect` returns cleanly.

### Phase 5: VS Code Integration

| Location | Files | Changes |
|----------|-------|---------|
| `integrations/vscode` | `package.json` | Add `debuggers` contribution; **also add `commands` and `menus.debug/toolBar` entries for `ironplc.stepScan` and `ironplc.scanCount`** (otherwise custom DAP requests are unreachable) |
| `integrations/vscode/src` | new `debugAdapter.ts` | `DebugAdapterDescriptorFactory`; if `program` is `.st`, run the compiler with debug info enabled to emit a temp `.iplc`; launch `ironplcvm-debug --dap <file.iplc>` |
| `integrations/vscode/src` | new `customRequests.ts` | Wraps `ironplc/stepScan`, `ironplc/scanCount` as VS Code commands. (No force/unforce — those are out of v1 scope.) |
| `integrations/vscode/src` | `extension.ts` | Register debug adapter factory and custom-request commands |

**Tests:**
- Extension: debug adapter registered.
- Extension: launch configuration resolves the `ironplcvm-debug` path (env var, settings, then bundled).
- Manual: F5 with a breakpoint hits, variable inspection populates, Step Scan toolbar works.

### Phase 6: Beyond v1 (Future)

These enhancements build on the v1 debugger. Several were dropped from v1 (see §v1 Scope Decisions) and have explicit follow-up phases.

1. **Variable forcing with a force-table** — paused-write that *persists* across scans, re-applied at INPUT_FREEZE, surfaced in the UI as "forced". Replaces the placeholder "no forcing in v1." Adds `ironplc/forceVariable` and `ironplc/unforceVariable`, sets `supportsSetVariable: true`. (Cut from v1: see §Variable forcing: not in v1.)
2. **Multi-task and multi-instance debugging** — per-instance breakpoint filters (`instance_filter` field on `BreakpointEntry`), DAP threads per instance, `current_instance_id` tracking, mid-round resume across `instances_for_task`, `ironplc/instances` custom request. Removes the v1 launch precondition. (Cut from v1: see §Multi-instance: not supported in v1.)
3. **Pause-while-running** — `ArcSwap<BreakpointTable>` + `AtomicBool pause_requested` + two-thread DAP server. Adds the DAP `pause` request and `setBreakpoints`-takes-effect-mid-instruction. (Cut from v1: see §Single-threaded DAP loop.)
4. **Conditional breakpoints** — DAP `condition` field on breakpoints, evaluated by the VM. The same expression evaluator that powers conditional breakpoints also powers full `evaluate`. (Builds on the v1 evaluate subset and the v1 logpoint format strings.)
5. **Compound expression evaluation** — full `evaluate` (arithmetic, function calls), via a sandboxed evaluator that reuses the constant-folder. Logpoint format strings inherit it.
6. **Scan cycle status bar** — show `scan_count` and a "step scan" button (the toolbar button itself ships in Phase 5)
7. **Process image inspection** — view %I, %Q, %M regions with bit/byte/word addressing
8. **Hot reload during debug** — recompile and online-change while paused, preserving breakpoints
9. **Type-name string deduplication** — intern `type_name` strings in a separate sub-table to shrink debug sections
10. **Watchpoints (data breakpoints)** — instrument `STORE_VAR` opcodes to fire when a specific variable's value changes. Cheap in a VM (no page-fault tricks needed); listed here as a Phase 6 because v1's first reviewer feedback called it out as a notable miss.

## Optimizer Contract

The bytecode optimizer (`compiler/codegen/src/optimize.rs`) removes instructions and shifts jump targets. Source-level debugging requires a stable contract between the optimizer and debug info:

1. **Line map remapping is mandatory.** Any pass that changes the bytecode must rewrite the line map through its old→new offset table. `optimize_with_source_map` (per `2026-04-07-debug-source-map-and-hook.md`) is the only legal way to invoke optimization when debug info is enabled.
2. **Snap-forward for removed instructions.** When the offset that an entry references is removed, the entry's offset advances to the *next surviving instruction*; consecutive duplicate entries collapse.
3. **Instruction-boundary invariant.** Every line-map offset in the optimized stream must land on the first byte of an instruction. A property test in `compiler/codegen/tests/` enforces this on a corpus of programs.
4. **Breakpoint resolution is post-optimization.** The DAP server resolves source lines against the line map *as emitted* (already remapped). It then reports the resolved line back to the client in the `Breakpoint` response so the editor highlights the actual stop line. Breakpoints requested on lines whose statements were entirely optimized away resolve to the next surviving line in the same function; if no such line exists, the breakpoint is reported `verified: false` with `message: "line eliminated by optimizer"`.
5. **`--debug` build flag (codegen).** When set, the optimizer is configured to *preserve* user-visible source positions even when otherwise legal to remove them (e.g., it does not collapse a `LOAD_CONST_I32 0; STORE_VAR` if that sequence is the only line-map anchor for a statement). This trades some optimization quality for predictable stepping. Release builds keep the existing aggressive pipeline and accept that some statements may have no breakpoint location.

## Container Format Compatibility

This spec **replaces** the debug section format defined in the container format spec with a tagged sub-table layout. The changes are:

1. **New directory header** — `sub_table_count: u16` + array of `(tag: u16, _reserved: u16, size: u32)` entries
2. **Tagged sub-tables** — each sub-table has a type tag; readers skip unknown tags by size
3. **LineMapEntry grows from 6 to 8 bytes** — adds `source_column: u16`
4. **VarNameEntry gains scope and type fields** — adds `function_id`, `var_section`, `type_name`
5. **Three new sub-table types** — function names (tag 3), FB type names (tag 4), FB field names (tag 5)
6. **Reserved tags** — source file table (tag 6), LD rung map (tag 7), FBD network map (tag 8)

These changes affect only the debug section, which is independently hashed and signed (via `debug_hash` and the debug signature section). Adding or modifying debug info does not affect the content signature or the content hash, so existing containers remain valid.

The container format spec's debug section definition should be updated to match the format specified in this document. Since the debug section has not yet been implemented in the container crate, this is a spec revision, not a breaking change.

**Forward compatibility.** The tagged directory is the extensibility mechanism. A reader that encounters an unknown tag skips it using the `size` field — no knowledge of the sub-table's internal structure is required. This means:

- Future sub-tables (LD rung maps, source file tables, etc.) can be added by allocating a new tag. Existing readers continue to work — they simply skip the unknown tag.
- A compiler that emits a new sub-table type does not break older debuggers.
- The debug section does not need its own version number. The tag registry serves the same purpose: readers understand the tags they know and skip the rest.

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

The items below are out of v1 scope. The first three are the **deliberate v1 cuts** documented in §v1 Scope Decisions; the rest are out-of-scope for independent reasons.

1. **Variable forcing** (paused-write to variables) — deferred. A simple write-while-paused gets overwritten on the next scan and trains users that the debugger is broken; a correct force-table design is a separate effort. v1 replaces this with **logpoints**. See §Variable forcing: not in v1 and Phase 6 item 1.
2. **Multi-instance debugging** — deferred. v1 rejects multi-instance programs at launch with `MultiInstanceUnsupported`. See §Multi-instance: not supported in v1 and Phase 6 item 2.
3. **Pause-while-running (DAP `pause` request)** — deferred. v1 uses a single-threaded DAP loop; users set breakpoints in advance, use `scanLimit`, or `disconnect`. The `ArcSwap<BreakpointTable>` + `AtomicBool` two-thread design ships in Phase 6. See §Single-threaded DAP loop and Phase 6 item 3.
4. **Conditional breakpoints** — DAP `condition` field requires compound expression evaluation; deferred to Phase 6.
5. **Compound expression evaluation in `evaluate` and logpoints** — v1 supports bare identifiers, dotted field access on identifiers, and constant subscripts only. Arithmetic, function calls, and non-constant subscripts return `evaluateUnsupported` (or `<unsupported: ...>` in logpoints). Lifted in Phase 6.
6. **Watchpoints (data breakpoints)** — break when a specific variable's value changes. Cheap to add in a VM (instrument `STORE_VAR`); listed for Phase 6.
7. **Remote debugging** — DAP over TCP to debug programs on remote targets (embedded PLCs). The initial implementation uses stdin/stdout only.
8. **Multi-file debugging** — debugging programs that span multiple source files. The initial implementation assumes a single source file per container. The debug section format reserves space for a source file table (via a future sub-table) but does not define it in v1.
9. **Ladder Diagram / FBD debugging** — graphical IEC 61131-3 languages (LD, FBD) have fundamentally different debugging UIs (highlighting rungs, showing power flow, animating contacts/coils). The debug section format reserves tags 7 (LD_RUNG_MAP) and 8 (FBD_NETWORK_MAP) for this purpose — future compilers can emit these sub-tables and existing ST debuggers will skip them harmlessly. However, the compilation pipeline, DAP server, and VS Code extension would all need LD/FBD-specific support. This is deferred until graphical language compilation is implemented.
10. **Time-travel debugging** — recording and replaying execution. Would require snapshotting VM state at each scan cycle, which is a significant memory and performance cost.
11. **`attach` / online-change** — v1 supports `launch` only. Attaching to a running VM and online code change are deferred.

## Summary of architectural changes from the original plan

For reviewers familiar with the prior version of this document:

1. **Iterative dispatch with explicit `FrameStack`** replaces the recursive `execute_with_hook`. The frame stack is a **borrowed slice** sized at load time from `header.max_call_depth` (computed by the compiler from the call graph; recursion is forbidden by IEC 61131-3 so the longest path is well-defined). No heap allocation; matches the existing `OperandStack` / `VariableTable` model and works on Arduino-class targets. This is the prerequisite for instruction-level pausing.
2. **`DebugHook` trait extension** — `before_instruction` returns `HookAction`; new `before_call`/`after_return` callbacks; `function_id` parameter added. The existing `NoopDebugHook` + monomorphization model is retained, **not** replaced by `Option<&mut DebugState>`.
3. **`DebuggerHook` is a `DebugHook` impl**, not a fourth-thing-named-`DebugState`. It owns the breakpoint table reference, step controller, and logpoint table.
4. **Pause/Resume protocol is defined**: a complete checkpoint table lists every piece of state preserved across a pause, plus a property test asserting bitwise resume parity.
5. **Optimizer contract** spelled out: post-optimization line map, snap-forward, instruction-boundary invariant, breakpoint-resolution feedback.
6. **State machine documented** with a Phase enum and a per-DAP-request legality column.
7. **Tag registry reconciled** with the existing `Tag 9 = ENUM_DEF` implementation.
8. **DAP packaging** is `--features dap` on `vm-cli`, distributed as `ironplcvm-debug` for the VS Code extension.

### v1 scope cuts (changes vs. earlier drafts)

The current draft makes three deliberate v1 cuts (§v1 Scope Decisions). Each is paired with a replacement or a clean error path; each unlocks engineering capacity that earlier drafts spent on partial implementations.

| Earlier draft | Current v1 | Phase 6 plan |
|---------------|-----------|--------------|
| Variable forcing as paused write-through (broken across scans) | **Removed** — `supportsSetVariable: false`; `force_variable` API not present | Force-table with INPUT_FREEZE re-application |
| Multi-instance pause semantics with `current_instance_id` and global breakpoints | **Removed** — `launch` rejects multi-instance with `MultiInstanceUnsupported` | Per-instance breakpoint filters + DAP threads |
| Two-thread DAP server with `ArcSwap<BreakpointTable>` and `AtomicBool pause_requested` | **Removed** — single-threaded DAP loop, DAP `pause` returns `requestNotApplicable` | Add interactive pause + mid-instruction setBreakpoints |
| Logpoints listed as Phase 6 future work | **Promoted to v1** — `supportsLogPoints: true`, format strings reuse the v1 `evaluate` subset | Compound expression evaluator extends logpoint syntax |

These trade specific UX corner cases (interactive pause, multi-instance, write-to-variable) for a smaller, simpler v1 that ships logpoints — the headline observability feature for scan-cycle code — and removes the most architecturally heavy machinery from the critical path. The full features ship in Phase 6 with proper designs of their own.
