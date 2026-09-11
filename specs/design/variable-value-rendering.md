# Design: Variable Value Rendering

## Overview

Four surfaces display the value of a VM variable:

| Surface | Where |
|---------|-------|
| `ironplcvm run --dump-vars` | `compiler/vm-cli/src/cli.rs` |
| DAP `variables` response | `compiler/vm-cli/src/dap/` |
| LSP run panel (VS Code) | `compiler/ironplc-cli/src/lsp_runner.rs` |
| Playground | `compiler/playground/src/lib.rs` |

They all render the same data — a raw 64-bit variable-table slot plus the
container's debug section — and a reader comparing two of them is entitled to
see the same text. This document specifies that text, and the crate that owns
it.

## Ownership

`ironplc_container::debug_format::VariableRenderer` is the **only** renderer.
The per-tag match, the data-region string reader and the enumeration lookup are
private to that module; a surface holds a `VariableRenderer` and calls
`name`, `render` or `line`.

This is a structural rule, not a stylistic one. The rules below are policy —
which literal syntax, which unit, what to show when a value cannot be read —
and a second copy of a policy is a copy that drifts. Two copies previously
existed and did drift: the same `STRING` printed as `0` in the CLI dump and as
its content in the playground, and the same `TIME` printed in two different
units.

- **REQ-VR-container-001** A renderer built from a container with no debug
  section renders every variable as name `var[<index>]` and value the slot as a
  signed 32-bit decimal.

## Naming

- **REQ-VR-container-002** When the debug section's VAR_NAME sub-table names a
  variable, its rendered name is that source name; otherwise it is
  `var[<index>]`.
- **REQ-VR-container-003** A variable's line rendering is `<name>: <value>`.

## Values held in the variable slot

- **REQ-VR-container-010** Values are rendered from the slot according to the
  variable's IEC type tag:

| Requirement | Tag | Format | Example |
|-------------|-----|--------|---------|
| **REQ-VR-container-011** | `BOOL` | `TRUE`/`FALSE` | `TRUE` |
| **REQ-VR-container-012** | `SINT`, `INT`, `DINT`, `LINT` | signed decimal | `-42` |
| **REQ-VR-container-013** | `USINT`, `UINT`, `UDINT`, `ULINT` | unsigned decimal | `42` |
| **REQ-VR-container-014** | `REAL`, `LREAL` | shortest round-tripping decimal | `3.14` |
| **REQ-VR-container-015** | `BYTE`, `WORD`, `DWORD`, `LWORD` | `16#` and two hex digits per byte | `16#DEADBEEF` |
| **REQ-VR-container-016** | `TIME`, `LTIME` | `T#<ms>ms`, `LTIME#<ms>ms` | `T#1500ms` |
| **REQ-VR-container-017** | `DATE`, `LDATE` | `D#YYYY-MM-DD`, `LDATE#YYYY-MM-DD` | `D#2024-01-15` |
| **REQ-VR-container-018** | `TIME_OF_DAY`, `LTOD` | `TOD#HH:MM:SS[.mmm]`, `LTOD#HH:MM:SS[.mmm]` | `TOD#23:59:59.999` |
| **REQ-VR-container-019** | `DATE_AND_TIME`, `LDT` | `DT#YYYY-MM-DD-HH:MM:SS`, `LDT#YYYY-MM-DD-HH:MM:SS` | `DT#2024-01-15-14:30:00` |
| **REQ-VR-container-020** | any other | signed 32-bit decimal | `0` |

- **REQ-VR-container-021** A negative duration places the sign after the `#`
  (`T#-250ms`), which is where IEC 61131-3 puts it.

Durations render in milliseconds rather than in the largest unit that fits
(`T#1500ms`, not `T#1.5s`) so the value is exact, unit-stable across the whole
range, and reparses as the same literal.

The calendar types take their units from
[ADR-0025](../adrs/0025-datetime-unsigned-representation.md) as amended:
`DATE`, `LDATE`, `DATE_AND_TIME` and `LDT` hold seconds since 1970-01-01;
`TIME_OF_DAY` and `LTOD` hold milliseconds since midnight.

## Values held in the data region

A `STRING` or `WSTRING` variable's slot is unused. Its bytes live in the data
region at the offset recorded by the debug section's STRING layout sub-table,
laid out per [ADR-0035](../adrs/0035-length-and-encoding-prefixed-string-layout.md) as
`[max_length: u16][cur_length: u16][char_width: u16][data…]`.

- **REQ-VR-container-030** `STRING` and `WSTRING` values are rendered from the
  data region at their STRING layout entry's offset, never from the slot.
- **REQ-VR-container-031** The value's byte span is `cur_length * char_width`
  bytes, and its encoding comes from the header's own `char_width` field rather
  than from the variable's type tag.
- **REQ-VR-container-032** A narrow (Latin-1) value renders as a single-quoted
  IEC literal and a wide (UTF-16LE) value as a double-quoted one, with `$`
  escapes: `$$`, `$'`/`$"`, `$T`, `$L`, `$P`, `$R`, and a hex escape (`$XX`
  narrow, `$XXXX` wide) for anything else outside printable ASCII.

## Values that cannot be read

A placeholder must never read as a value. `msg: 0` was indistinguishable from a
real zero, which is what made the original defect worse than an omission.

- **REQ-VR-container-040** A string variable with no STRING layout entry renders
  as `<unavailable>`.
- **REQ-VR-container-041** A string variable whose layout does not fit the data
  region, or whose header carries a `char_width` other than 1 or 2, renders as
  `<invalid>`.
- **REQ-VR-container-042** A rendered value is marked invalid when, and only
  when, its text is one of those placeholders, so a surface that styles values
  can show a placeholder as a placeholder.
- **REQ-VR-container-043** A `STRUCT`, `ARRAY` or `FB_INSTANCE` variable
  whose layout the container does not describe renders as `<TYPE_NAME>` from
  its declared type name, or `<aggregate>` when no type name is recorded, and
  is marked invalid. Its slot holds the byte offset of its contents in the data
  region rather than a value, so rendering the slot would publish an internal
  layout detail as program data — and a convincing one, since the offset moves
  when an unrelated declaration changes size.

These three tags are distinct from `OTHER` rather than folded into it because
`OTHER` also carries named subrange types, whose slot *does* hold their value.

## Aggregates with a recorded layout

The [Variable Inspection Model](variable-inspection-model.md) adds the layout
the rule above lacks. Once a container carries it, an aggregate renders as its
type name, is marked valid, and expands into named children — fields in
declaration order, elements by declared index — each rendered by the rules of
this document as if it were a variable of its own. The rules of this document
do not change; they gain callers. The placeholder of REQ-VR-container-043
remains the rendering for an aggregate whose layout is missing or unresolved.

## Enumerations

An enumeration variable's slot holds an ordinal. The debug section's ENUM_DEF
sub-table maps the declared type name to its value names in ordinal order.

- **REQ-VR-container-050** A variable whose declared type name matches an
  ENUM_DEF entry that has a value at the slot's ordinal renders as
  `<VALUE_NAME> (<ordinal>)`.
- **REQ-VR-container-051** An ordinal with no value name in that entry falls
  back to the type-tag rendering, so an out-of-range enumeration value still
  shows the number the VM holds.
