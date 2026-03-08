# Design: Debug Information in IPLC Container

## Overview

This design describes how to add debug information — specifically variable names with types and function names — to the IPLC bytecode container. This enables:

1. **Playground**: Display variable names and correctly formatted values (e.g., `counter : DINT = 42` instead of `var[0] = 42`)
2. **Future DAP debugger**: Implement `variables`, `scopes`, and `stackTrace` responses with human-readable names and types

This design implements Tags 2 (VAR_NAME) and 3 (FUNC_NAME) from the debug section format defined in [Bytecode Container Format](bytecode-container-format.md), following the type encoding approach in [ADR-0019](../adrs/0019-type-encoding-in-debug-variable-names.md).

## Background

### What Exists

The container format spec defines a debug section with tagged sub-tables, and the header already has `debug_section_offset`, `debug_section_size`, `debug_hash`, and flags bit 1. All are currently zero — no debug section is generated or consumed.

The codegen (`compile.rs`) has full access to variable names, IEC types, and section kinds during `assign_variables()`, but discards all of this after assigning numeric indices.

The VM stores all values as `Slot(u64)`. The current `read_variable()` method returns `i32`, which truncates 64-bit values and misinterprets float bit patterns. The playground's `VariableInfo` only has `index: u16` and `value: i32`.

### What This Design Adds

```
Source Code                    IPLC Container                 Playground Display
─────────────                  ──────────────                 ──────────────────
PROGRAM MAIN                   ┌─────────────┐
  VAR                          │ Header      │
    counter : DINT := 0;  ──►  │ ...         │
    temp    : REAL := 3.14;    │ flags: 0x02 │  (debug present)
    active  : BOOL := TRUE;    ├─────────────┤
  END_VAR                      │ Task Table  │
  ...                          ├─────────────┤
END_PROGRAM                    │ Const Pool  │
                               ├─────────────┤
                               │ Code Section│
                               ├─────────────┤
                               │ Debug Sect  │  ──►  counter : DINT = 0
                               │  VAR_NAME   │       temp : REAL = 3.14
                               │  FUNC_NAME  │       active : BOOL = TRUE
                               └─────────────┘
```

## Detailed Design

### 1. VarNameEntry Binary Format

Follows the spec with one addition — `iec_type_tag` (see [ADR-0019](../adrs/0019-type-encoding-in-debug-variable-names.md)):

| Offset | Field | Type | Description |
|--------|-------|------|-------------|
| 0 | var_index | u16 | Variable table index |
| 2 | function_id | u16 | Owning function ID (0xFFFF = global/program scope) |
| 4 | var_section | u8 | IEC 61131-3 variable section (0=VAR, 1=VAR_TEMP, ..., 6=VAR_GLOBAL) |
| 5 | iec_type_tag | u8 | IEC type for display interpretation (see ADR-0019 tag table) |
| 6 | name_length | u8 | Length of variable name in bytes |
| 7 | name | [u8; name_length] | UTF-8 variable name |
| 7+N | type_name_length | u8 | Length of type name in bytes |
| 8+N | type_name | [u8; type_name_length] | UTF-8 type name (e.g., "DINT", "TrafficLight") |

**Change from existing spec:** The `iec_type_tag` byte is inserted at offset 5, shifting `name_length` from offset 5 to offset 6. The existing spec's VarNameEntry format must be updated to match.

### 2. FuncNameEntry Binary Format

Unchanged from the existing spec:

| Offset | Field | Type | Description |
|--------|-------|------|-------------|
| 0 | function_id | u16 | Function ID from the code section |
| 2 | name_length | u8 | Length of function name in bytes |
| 3 | name | [u8; name_length] | UTF-8 POU name (e.g., "MAIN", "MAIN_init") |

### 3. VM: Raw Variable Access

Add `read_variable_raw() -> Result<u64, Trap>` to `VmRunning`, `VmStopped`, and `VmFaulted`. This returns the raw 64-bit slot value without interpretation, allowing the caller to apply the correct type-specific formatting.

The existing `read_variable() -> Result<i32, Trap>` remains for backward compatibility.

### 4. Container Crate Changes

**New module: `debug_section.rs`**

Data structures:
- `VarNameEntry` — fields as above, with `write_to` / `read_from`
- `FuncNameEntry` — fields as above, with `write_to` / `read_from`
- `VarSection` enum — all 7 IEC 61131-3 variable sections
- `IecTypeTag` — constants for all type tags from ADR-0019
- `DebugSection` — contains `Vec<VarNameEntry>` and `Vec<FuncNameEntry>`, with section-level `write_to` / `read_from` using the tagged sub-table directory format

**Container struct extension:**
```rust
pub struct Container {
    pub header: FileHeader,
    pub task_table: TaskTable,
    pub constant_pool: ConstantPool,
    pub code: CodeSection,
    pub debug_section: Option<DebugSection>,  // NEW
}
```

Serialization: debug section is written after code section. Header fields `debug_section_offset`, `debug_section_size`, and `flags` bit 1 are set when present.

Deserialization: if `debug_section_size > 0`, attempt to parse. On failure, set `None` (non-fatal — per spec, debug info is optional/strippable).

**ContainerBuilder extension:**
```rust
pub fn add_var_name(mut self, entry: VarNameEntry) -> Self
pub fn add_func_name(mut self, entry: FuncNameEntry) -> Self
```

### 5. Codegen Changes

In `assign_variables()`, for each `VarDecl` that gets assigned an index, also create a `VarNameEntry`:

| Source | VarNameEntry field |
|--------|-------------------|
| assigned index | `var_index` |
| current function being compiled | `function_id` |
| `decl.section` (VAR, VAR_INPUT, ...) | `var_section` |
| resolved IEC type name | `iec_type_tag` (map DINT→3, REAL→9, BOOL→0, etc.) |
| `decl.identifier.symbolic_id()` | `name` |
| IEC type name or user-defined type name | `type_name` |

In `compile_program()`, pass collected entries through the builder:
- One `FuncNameEntry` per emitted function (init function, scan function)
- All collected `VarNameEntry` values

### 6. Playground Changes

**VariableInfo** gains `name`, `type_name`, and changes `value` from `i32` to `String`:

```rust
struct VariableInfo {
    index: u16,
    value: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    type_name: String,
}
```

A `format_variable_value(raw: u64, iec_type_tag: u8) -> String` function handles type-correct display using a `match` on the tag byte. No string comparison needed.

After loading the container, build a lookup map from `var_index → (name, type_name, iec_type_tag)` from the debug section. Use `read_variable_raw()` + the tag to format each value.

**Breaking change:** `value` changes from `i32` to `String`. The playground frontend must display this string directly.

### 7. Data Flow

```
Source AST (VarDecl)
    │
    ▼
assign_variables()
    │  creates VarNameEntry per variable
    │  maps section → var_section, type → iec_type_tag
    ▼
CompileContext.debug_var_names: Vec<VarNameEntry>
    │
    ▼
compile_program()
    │  passes entries to ContainerBuilder
    │  also creates FuncNameEntry per function
    ▼
ContainerBuilder
    │  .add_var_name(entry)
    │  .add_func_name(entry)
    │  .build() → Container with Some(DebugSection)
    ▼
Container.write_to()
    │  serializes debug section after code section
    │  sets header.debug_section_offset/size/flags
    ▼
IPLC binary on disk / in memory
    │
    ▼
Container.read_from()
    │  parses debug section if present
    ▼
Playground / DAP debugger
    │  reads debug_section.var_names
    │  maps var_index → (name, type_name, iec_type_tag)
    │  uses read_variable_raw() + iec_type_tag → formatted string
    ▼
JSON output: { "index": 0, "name": "counter", "type_name": "DINT", "value": "42" }
```

## Scope and Deferrals

**In scope:**
- Tag 2 (VAR_NAME) with `iec_type_tag` — all IEC primitive types
- Tag 3 (FUNC_NAME) — function/POU names
- VM `read_variable_raw()` — raw slot access
- Playground type-aware display

**Deferred:**
- Tag 0 (SOURCE_TEXT) — embedded source for debugger "open source" feature
- Tag 1 (LINE_MAP) — needed for breakpoints/stepping
- Tags 4-5 (FB_TYPE_NAME, FB_FIELD_NAME) — function block field names
- STRING/WSTRING value display — requires reading from data region, not slot
- ENUM member name display — requires enum definition table
- DAP server implementation — separate effort after debug info exists

## Testing Strategy

- **Container roundtrip**: write container with debug section → read back → verify entries match
- **Backward compat**: read container without debug section → `debug_section` is `None`
- **Unknown tag skip**: debug section with an unknown tag → parsed entries still correct
- **Codegen integration**: compile a program → verify debug section has correct names, types, sections
- **Playground display**: compile + run → verify JSON includes named, typed, correctly formatted variables
- **VM raw read**: store f32/i64 in slot → `read_variable_raw()` returns correct bit pattern
