# ADR-0019: Type Encoding in Debug Variable Names

status: proposed
date: 2026-03-08

## Context and Problem Statement

The debug section's VAR_NAME sub-table (Tag 2) associates each variable with metadata including its type. The debugger and playground need this type information to correctly interpret raw slot values (a `u64` in the VM) as the appropriate IEC 61131-3 type.

The existing Type Section defines `var_type` as a **storage representation** encoding (`0=I32, 1=U32, 2=I64, 3=U64, 4=F32, 5=F64, ...`). This is insufficient for display because multiple IEC types share the same storage representation:

| Storage Type | IEC 61131-3 Types | Display Difference |
|-------------|-------------------|-------------------|
| I32 | SINT, INT, DINT | SINT: -128..127, INT: -32768..32767, DINT: full i32 |
| I32 | BOOL | TRUE / FALSE |
| U32 | BYTE, WORD, DWORD | BYTE: `16#FF`, WORD: `16#FFFF`, DWORD: `16#FFFFFFFF` |
| U32 | USINT, UINT, UDINT | USINT: 0..255, UINT: 0..65535, UDINT: full u32 |

How should the debug section encode the IEC 61131-3 type for each variable?

## Decision Drivers

* **Display correctness** — BOOL must show TRUE/FALSE, BYTE must show hex, SINT must show -128..127 range
* **Space efficiency** — the debug section should not bloat the container size; variables are numerous
* **Matching performance** — the runtime (playground, debugger) must quickly determine the display format
* **User-defined types** — ENUM, subrange, and struct types have user-assigned names ("TrafficLight", "SmallInt") that should be visible
* **DAP compatibility** — the Debug Adapter Protocol's `variables` response includes a `type` string field
* **Extensibility** — new types should be addable without breaking existing readers

## Considered Options

### Option A: String type name only

Each VarNameEntry includes a `type_name` string (e.g., `"DINT"`, `"BOOL"`, `"TrafficLight"`).

* Good, because it's human-readable and self-describing
* Good, because user-defined type names work naturally
* Good, because it maps directly to DAP's `type` field
* Bad, because string matching is needed for display format selection (17+ comparisons)
* Bad, because strings consume 2-6 bytes per variable for common primitive types
* Bad, because case sensitivity and encoding must be handled carefully

### Option B: Numeric type tag only

Each VarNameEntry includes a `iec_type_tag` byte using a fixed encoding.

* Good, because a single byte encodes the type (1 byte vs 2-6 bytes for strings)
* Good, because integer match is fast and unambiguous
* Good, because it's compact for the common case (most variables are primitives)
* Bad, because user-defined types (ENUM, subrange) lose their names
* Bad, because the DAP `type` field must be reverse-mapped from tag → string
* Bad, because adding new primitive types requires updating the tag registry

### Option C: Numeric type tag + conditional string name

Each VarNameEntry includes a `iec_type_tag` byte for bit interpretation AND a `type_name` string for display. The string is the IEC type name for primitives (e.g., `"DINT"`) and the user-defined type name for derived types (e.g., `"TrafficLight"`).

* Good, because interpretation uses a fast integer match
* Good, because user-defined type names are preserved
* Good, because the string maps directly to DAP's `type` field
* Neutral, because there is mild redundancy for primitive types (tag and string both encode the same info)
* Bad, because it's the most complex format

## Decision Outcome

**Option C: Numeric type tag + string name.**

The `iec_type_tag` drives bit interpretation (how to read the raw `u64` slot value). The `type_name` string provides the display name for the type, which is essential for user-defined types (ENUMs, subranges) and maps directly to DAP's `type` field.

### IEC Type Tag Encoding

| Tag | IEC Type | Slot Interpretation | Display Format |
|-----|----------|-------------------|---------------|
| 0 | BOOL | `(raw as i32) != 0` | `TRUE` / `FALSE` |
| 1 | SINT | `raw as i32 as i8` | signed decimal |
| 2 | INT | `raw as i32 as i16` | signed decimal |
| 3 | DINT | `raw as i32` | signed decimal |
| 4 | LINT | `raw as i64` | signed decimal |
| 5 | USINT | `raw as u8` | unsigned decimal |
| 6 | UINT | `raw as u16` | unsigned decimal |
| 7 | UDINT | `raw as u32` | unsigned decimal |
| 8 | ULINT | `raw as u64` | unsigned decimal |
| 9 | REAL | `f32::from_bits(raw as u32)` | floating point |
| 10 | LREAL | `f64::from_bits(raw)` | floating point |
| 11 | BYTE | `raw as u8` | `16#XX` hex |
| 12 | WORD | `raw as u16` | `16#XXXX` hex |
| 13 | DWORD | `raw as u32` | `16#XXXXXXXX` hex |
| 14 | LWORD | `raw as u64` | `16#XXXXXXXXXXXXXXXX` hex |
| 15 | STRING | data region | deferred |
| 16 | WSTRING | data region | deferred |
| 17 | TIME | `raw as i64` (microseconds) | `T#...` |
| 18 | DATE | reserved | deferred |
| 19 | TIME_OF_DAY | reserved | deferred |
| 20 | DATE_AND_TIME | reserved | deferred |
| 21–254 | — | reserved | — |
| 255 | OTHER | fallback | display type_name + raw decimal |

For user-defined types (ENUM, subrange), the `iec_type_tag` is set to the **underlying primitive type** (e.g., tag 2 for `INT`-based enum), and `type_name` carries the user-defined name (e.g., `"TrafficLight"`). This means the value can always be displayed correctly using just the tag, even without understanding the derived type's definition.

### Consequences

* Good, because the playground can format any variable value with a simple `match` on a `u8` — no string parsing needed
* Good, because ENUM values display as their underlying integer type until an enum definition table is added (graceful degradation)
* Good, because the string type_name maps directly to DAP's `type` field
* Neutral, because primitive types have mild redundancy (1 byte tag + 2-6 byte string name)
* Bad, because the VarNameEntry format grows by 1 byte compared to Option A

## More Information

### Relationship to Type Section

The Type Section's `var_type` encoding (I32, U32, I64, etc.) describes **storage representation** for the verifier. The debug section's `iec_type_tag` describes **IEC semantics** for the debugger. These are related but distinct:

| var_type (storage) | iec_type_tag values (semantics) |
|-------------------|---------------------------------|
| I32 (0) | BOOL (0), SINT (1), INT (2), DINT (3) |
| U32 (1) | USINT (5), UINT (6), UDINT (7), BYTE (11), WORD (12), DWORD (13) |
| I64 (2) | LINT (4) |
| U64 (3) | ULINT (8), LWORD (14) |
| F32 (4) | REAL (9) |
| F64 (5) | LREAL (10) |

### Future: ENUM Display

When an enum definition table (Tag 4/5) is added, the debugger will look up the raw value in the enum definition to display the member name (e.g., `"Green"` instead of `2`). The `iec_type_tag` provides the fallback display format until then.
