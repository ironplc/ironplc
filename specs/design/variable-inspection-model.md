# Design: Variable Inspection Model

## Overview

This design specifies how a `STRUCT`, `ARRAY` or `FUNCTION_BLOCK` instance
variable is presented for inspection: what the compiler records about its
layout, how a reader turns that layout plus the data region into a tree of
named, typed values, and how the DAP server and the `--dump-vars` command
expose that tree.

Today every surface stops at the aggregate's type name. The
[Variable Value Rendering](variable-value-rendering.md) design made that
stop deliberate (REQ-VR-container-043) rather than print the data-region
offset the slot holds as if it were a value, and left the contents for "a
layout sub-table the container does not yet carry." This document specifies
that sub-table.

The decision behind the shape of the tables is
[ADR-0049](../adrs/0049-type-directed-debug-layout-for-aggregates.md).
This design builds on:

- **[Bytecode Container Format](bytecode-container-format.md)** — the debug
  section, its tagged sub-table directory, and the type section whose array
  descriptors the layout tables must agree with
- **[Debugger Support](debugger-support.md)** — the DAP `scopes` and
  `variables` requests this design extends
- **[Variable Value Rendering](variable-value-rendering.md)** — the leaf
  rendering rules, which this design reuses unchanged
- **[Structure Code Generation](structure-codegen-memory-layout.md)**,
  [ADR-0026](../adrs/0026-structure-memory-layout.md) and
  [ADR-0027](../adrs/0027-compile-time-field-offset-resolution.md) — the
  layouts being described
- **[Enumeration Code Generation](enumeration-codegen.md)** — the `ENUM_DEF`
  table that enumeration fields resolve against

It supersedes the draft of the same name in pull request #1115, applying that
draft's review: the type reference and the static offset live on
`VarNameEntry` rather than in a separate table, there is no per-field
`inline` flag, the composite type-id space is partitioned, enumerations are a
kind of their own, and stdlib FB fields come from the codegen field maps.

### Requirements and conformance

Testable claims carry `REQ-VI-<crate>-NNN` identifiers per
[Development Standards — Design Requirement](../steering/development-standards.md#design-requirement).
`VI` is for **V**ariable **I**nspection. The crate slugs are `container`
(wire format and rendering), `codegen` (emission) and `vm-cli` (DAP and
`--dump-vars`).

This file is not yet registered in any crate's `build.rs`. Each
implementation PR registers it with the crate whose requirements it
implements and lands a `#[spec_test]` with a real assertion for each; the
completeness meta-test then holds the rest of the file to account.

## Design Goals

1. **Bounded debug info.** Size grows with the number of aggregate *types*
   and *variables*, never with the number of elements.
2. **Lazy.** A reader opens one node — `items[7].inner` — from the type tree
   and a base offset without decoding anything else. This is the DAP
   expansion model.
3. **Static shape.** Names, types, nesting and offsets are readable from the
   container alone. Only the *values* need a VM.
4. **One place per fact.** Every offset, stride and size in the tables is the
   number codegen emitted into the bytecode, produced by the same function,
   and a conformance test says so.
5. **Independent of the slot model.** Offsets are in bytes so that the
   packed-layout migration in ADR-0026 does not touch this format.
6. **Nothing new in the VM.** The debugger reads the data region and the
   variable table it can already see. No opcode, verifier rule or VM API
   changes.

## Scope

**In scope:** structures (nested to any depth), arrays of scalars, strings,
and structures, multi-dimensional arrays, arrays embedded in structures,
stdlib and user-defined function block instances, `STRING`/`WSTRING`
variables and fields, enumeration variables and fields, and aggregates
declared in programs, globals, functions and function block bodies.

**Out of scope, unchanged by this design:**

- `REF_TO` variables — the slot holds a variable index, not an offset. They
  render as today (`REF_TO`, signed decimal) and are not expandable.
- Function block instances as fields of structures or of other function
  blocks — codegen does not lay these out yet. The format can express them
  (a field with a composite reference of kind `FUNCTION_BLOCK`) once it does.
- Writing or forcing values.
- Expression evaluation. Each child carries an `evaluateName` in IEC syntax
  so the v1 `evaluate` subset in [Debugger Support](debugger-support.md) can
  resolve it later.
- The playground and the LSP run panel. Both render through the same
  renderer and can adopt the tree later; this design changes neither.

## 1. What the container records

### 1.1 Type references

A **type reference** names what a variable or field *is*, in three bytes.

**REQ-VI-container-001** A type reference is encoded as `kind: u8` followed
by `id: u16`, with the meaning of `id` determined by `kind`:

| Requirement | kind | Name | `id` | Value location |
|-------------|------|------|------|----------------|
| **REQ-VI-container-002** | 0 | `SCALAR` | An `iec_type_tag` (ADR-0019; values 0–24 and 255) | 8 bytes at the location, interpreted per the tag |
| **REQ-VI-container-003** | 1 | `ENUM` | Index into the `ENUM_DEF` sub-table (tag 9) | 8 bytes at the location; the low 32 bits are the ordinal |
| **REQ-VI-container-004** | 2 | `STRING` | Declared maximum length in code units | An ADR-0035 string (`[max_length][cur_length][char_width][data…]`) at the location |
| **REQ-VI-container-005** | 3 | `COMPOSITE` | `type_id` of a `COMPOSITE_TYPE` entry | The entry's `byte_size` bytes at the location, fields inline |
| **REQ-VI-container-006** | 4 | `ARRAY` | Index into the `ARRAY_TYPE` sub-table | Elements inline at the location, `element_stride` bytes apart |

**REQ-VI-container-007** A reader that meets a `kind` it does not recognise,
a `COMPOSITE` id with no entry, or an `ENUM`/`ARRAY` index past the end of
its table treats the reference as unresolved: the variable or field renders
as its declared type name in angle brackets and is not expandable
(REQ-VR-container-043).

`STRING` needs no table of its own: the encoding is in the ADR-0035 header at
the location, and the maximum length is all a reader needs to bound the read.
`SCALAR` reuses the tag encoding that drives every leaf rendering today, so
the rules in [Variable Value Rendering](variable-value-rendering.md) apply to
a field exactly as they apply to a variable.

The `iec_type_tag` values 25 (`STRUCT`), 26 (`ARRAY`) and 27 (`FB_INSTANCE`)
are retired. They existed to say "this slot holds an offset, do not print
it"; the `kind` now says that.

### 1.2 `VarNameEntry` (tag 2, revised)

The variable name entry replaces its `iec_type_tag` byte with a type
reference and gains the static offset of the variable's contents.

**REQ-VI-container-010** A `VarNameEntry` is laid out as:

| Requirement | Offset | Field | Type | Description |
|-------------|--------|-------|------|-------------|
| | 0 | var_index | u16 | Variable table index |
| | 2 | function_id | u16 | Owning function ID (`GLOBAL_SCOPE` = 0xFFFF for program and global scope) |
| | 4 | var_section | u8 | IEC 61131-3 variable section (unchanged encoding) |
| **REQ-VI-container-011** | 5 | type_ref | 3 bytes | Type reference (§1.1) |
| **REQ-VI-container-012** | 8 | data_offset | u32 | For `STRING`, `COMPOSITE` and `ARRAY` kinds: byte offset of the contents in the data region. Zero for every other kind. |
| | 12 | name_length | u8 | Length of the variable name in bytes |
| | 13 | name | [u8; N] | UTF-8 variable name |
| | 13+N | type_name_length | u8 | Length of the type name in bytes |
| | 14+N | type_name | [u8; M] | UTF-8 declared type name, e.g. `DINT`, `Point`, `TON`, `ARRAY [1..3] OF Point` |

**REQ-VI-container-013** `data_offset` is the compile-time constant the init
function stores into the variable's slot. A reader resolves the tree from
`data_offset`, not from the live slot, so the shape of the tree is readable
from the container alone.

**REQ-VI-container-014** The `STRING_LAYOUT` sub-table (tag 4) is retired. A
string variable is a `VarNameEntry` of kind `STRING` whose `data_offset` and
`id` carry what that table carried. A reader ignores a tag 4 payload if it
meets one.

### 1.3 `COMPOSITE_TYPE` (tag 5)

One entry per structure type and per function block type that any variable
or field references. Tag 5 was the never-implemented `FB_FIELD_NAME` table;
this table subsumes it.

**REQ-VI-container-020** The payload is `count: u16` followed by `count`
entries, each laid out as:

| Requirement | Field | Type | Description |
|-------------|-------|------|-------------|
| **REQ-VI-container-021** | type_id | u16 | Unique across the table (see partition below) |
| **REQ-VI-container-022** | kind | u8 | 0 = `STRUCT`, 1 = `FUNCTION_BLOCK` |
| **REQ-VI-container-023** | byte_size | u32 | Bytes one value of this type occupies in the data region, including any fields not listed |
| | name_length | u8 | Length of the type name in bytes |
| | name | [u8; N] | UTF-8 type name, e.g. `Point`, `TON`, `MotorControl` |
| | field_count | u16 | Number of field entries |
| **REQ-VI-container-024** | fields | [Field; field_count] | Fields in declaration order |

Each field:

| Requirement | Field | Type | Description |
|-------------|-------|------|-------------|
| | name_length | u8 | Length of the field name in bytes |
| | name | [u8; N] | UTF-8 field name as declared, e.g. `x`, `IN` |
| **REQ-VI-container-025** | var_section | u8 | Section the field was declared in (`VAR` for structure fields; `VAR_INPUT`, `VAR_OUTPUT` or `VAR` for FB fields), same encoding as `VarNameEntry` |
| **REQ-VI-container-026** | byte_offset | u32 | Offset of the field from the start of the value |
| **REQ-VI-container-027** | type_ref | 3 bytes | The field's type (§1.1) |

**REQ-VI-container-028** Every field is inline: its contents begin at
`base + byte_offset`, where `base` is the offset of the containing value.
There is no per-field indirection flag. If a by-reference field is ever laid
out, it gets a binding axis on the field entry, not a boolean.

**REQ-VI-container-029** `type_id` is partitioned so that the three sources
of composite types cannot collide:

| Range | Source |
|-------|--------|
| `0x0000`–`0x0FFF` | Standard library function blocks; the id is the `FB_CALL` type id (`TON` = `0x0010`, `CTU` = `0x0020`, …) |
| `0x1000`–`0x7FFF` | User-defined function blocks; the id is the `FB_CALL` type id assigned by codegen |
| `0x8000`–`0xFFFF` | Structures, assigned by codegen in first-reference order |

`kind` and the range say the same thing. A reader dispatches on `kind`; the
range is what makes `type_id` a usable key for the file viewer to correlate
an FB entry with the type section's FB descriptors and with `FB_CALL`
operands.

A function block type lists the fields the program declared. Standard
library blocks keep runtime state in fields the program never sees (a
timer's start time and running flag); those are covered by `byte_size` and
not listed, so a debugger never shows them and a file viewer can still
account for the bytes.

### 1.4 `ARRAY_TYPE` (tag 10)

One entry per distinct array type. Position in the table is the entry's id.

**REQ-VI-container-030** The payload is `count: u16` followed by `count`
entries, each laid out as:

| Requirement | Field | Type | Description |
|-------------|-------|------|-------------|
| **REQ-VI-container-031** | element_ref | 3 bytes | Type of every element (§1.1); may itself be `ARRAY` or `COMPOSITE` |
| **REQ-VI-container-032** | element_stride | u32 | Bytes from the start of one element to the start of the next |
| | dimension_count | u8 | Number of dimensions, at least 1 |
| **REQ-VI-container-033** | dimensions | [Dimension; dimension_count] | Declared bounds, outermost dimension first |

Each dimension is `lower: i32` followed by `upper: i32`, both inclusive, as
declared in the source.

**REQ-VI-container-034** Elements are stored in row-major order: the flat
index of element `[i, j]` of an `ARRAY[l1..u1, l2..u2]` is
`(i - l1) * (u2 - l2 + 1) + (j - l2)`, and its contents begin at
`base + flat_index * element_stride`. This is the order codegen normalises
subscripts to (ADR-0023).

`element_stride` is recorded rather than derived because it is not always the
element's size: a string element's stride is the ADR-0035 region size
(`6 + max_length * char_width`), while a string *field* of a structure is
rounded up to whole slots. Recording what codegen used is the rule; deriving
it would be a second computation that could disagree.

The type section's `ArrayDescriptor` is not referenced. It is deduplicated by
`(element_type, total_elements)`, so two arrays with different bounds share
one, and it encodes element types as `FieldType` rather than as IEC tags.
The two are kept consistent by test (REQ-VI-codegen-047), not by reference.

### 1.5 Tag registry changes

| Tag | Before | After |
|-----|--------|-------|
| 2 | `VAR_NAME` with `iec_type_tag` | `VAR_NAME` with `type_ref` and `data_offset` (§1.2) |
| 4 | `STRING_LAYOUT` | Retired; not reused |
| 5 | `FB_FIELD_NAME`, in development | `COMPOSITE_TYPE` (§1.3) |
| 9 | `ENUM_DEF`, order unspecified | `ENUM_DEF`, entries sorted by type name (§2.5) |
| 10 | Reserved | `ARRAY_TYPE` (§1.4) |

Tags 7 (`LD_RUNG_MAP`) and 8 (`FBD_NETWORK_MAP`) stay reserved for the
graphical languages. Tag 4 is retired rather than reused so that "tag 4" means
one thing in the history of the format.

**REQ-VI-container-035** The container format version is 4. A reader rejects
any other version, as it does today; there is no dual-format reader.

## 2. What codegen emits

Every fact below already exists in codegen at the point the variable is
allocated. This section says which existing value each field is, so the
tables and the bytecode are produced from the same numbers.

### 2.1 Variables

**REQ-VI-codegen-040** Every variable's `VarNameEntry` carries a type
reference derived from its declaration: `SCALAR` with the elementary type's
tag; `ENUM` for a named enumeration type; `STRING` for `STRING`/`WSTRING`;
`COMPOSITE` for a structure or function block instance; `ARRAY` for an array.
A subrange variable carries its base type's tag, not `OTHER`.

**REQ-VI-codegen-041** For every `STRING`, `COMPOSITE` and `ARRAY` variable,
`data_offset` equals the data-region offset codegen allocated for it — the
same constant the init function stores into the slot.

**REQ-VI-codegen-042** Aggregates declared in functions and function block
bodies, and structure-typed function return variables, receive the same
entries with their owning `function_id`. Their offsets are static under the
flat variable table of ADR-0046.

**REQ-VI-codegen-043** A `REF_TO` variable carries `SCALAR` with tag `OTHER`
and type name `REF_TO`, with `data_offset` zero.

### 2.2 Structures

**REQ-VI-codegen-044** One `COMPOSITE_TYPE` entry of kind `STRUCT` is emitted
per structure type referenced by any variable or field, with `byte_size`
equal to the type's slot count times 8 and one field per declared element in
declaration order.

**REQ-VI-codegen-045** Each field's `byte_offset` equals the slot offset
codegen resolves field accesses to (`StructFieldInfo::slot_offset`) times 8,
and its `type_ref` is derived from the field's declared type by the rule in
REQ-VI-codegen-040. The declared type name is carried on
`IntermediateStructField` so that enumeration and nested-structure fields
name their types.

### 2.3 Arrays

**REQ-VI-codegen-046** One `ARRAY_TYPE` entry is emitted per distinct
`(element_ref, dimensions, element_stride)` referenced by any variable or
field; equal entries are shared.

**REQ-VI-codegen-047** For every array variable, the `ARRAY_TYPE` entry and
the `ArrayDescriptor` the variable's `LOAD_ARRAY`/`STORE_ARRAY` use agree:
the product of the dimension sizes equals the descriptor's `total_elements`
(times the element structure's slot count for arrays of structures),
`element_stride` equals the descriptor's `element_stride()`, and a `SCALAR`
or `STRING` `element_ref` maps to the descriptor's `element_type`. This is a
codegen test over a corpus of array declarations, bridging the `FieldType`
and `iec_type_tag` encodings.

**REQ-VI-codegen-048** An array of structures is an `ARRAY_TYPE` whose
`element_ref` is the structure's `COMPOSITE` reference and whose
`element_stride` is the structure's `byte_size`. An array embedded in a
structure is a field whose `type_ref` is `ARRAY`.

### 2.4 Function blocks

**REQ-VI-codegen-049** One `COMPOSITE_TYPE` entry of kind `FUNCTION_BLOCK` is
emitted per stdlib block type instantiated by the program, with `type_id`
equal to its `FB_CALL` type id, `byte_size` equal to its instance field count
(including hidden fields) times 8, and one field per program-visible field at
`field_index * 8`. Names and indices come from the codegen field maps that
drive `FB_STORE_PARAM`/`FB_LOAD_PARAM`; types and sections come from the
analyzer's stdlib definitions.

**REQ-VI-codegen-050** One `COMPOSITE_TYPE` entry of kind `FUNCTION_BLOCK` is
emitted per user-defined block type instantiated by the program, with fields
in the order the instance region uses (inputs, then outputs, then locals),
each at `field_index * 8`.

**REQ-VI-codegen-051** A user FB field whose type codegen does not lay out in
the instance region is recorded as `SCALAR` with tag `OTHER` and its declared
type name, so a reader shows the raw slot rather than claiming a layout the
bytecode does not implement.

### 2.5 Enumerations

**REQ-VI-codegen-052** An enumeration variable or field carries `ENUM` with
`id` equal to the index of its type's `ENUM_DEF` entry.

**REQ-VI-codegen-053** `ENUM_DEF` entries are emitted sorted by type name
(UTF-8 byte order), so the index is stable across compilations. Today the
table is written in hash-map order, which is not.

### 2.6 Determinism

**REQ-VI-codegen-054** Compiling the same source twice produces identical
`COMPOSITE_TYPE`, `ARRAY_TYPE` and `ENUM_DEF` payloads: structure ids in
first-reference order over a deterministic walk, array entries in
first-reference order, enumerations sorted.

## 3. Rendering the tree

`ironplc_container::debug_format::VariableRenderer` remains the only
renderer (see [Variable Value Rendering — Ownership](variable-value-rendering.md#ownership)).
It gains a tree API; every surface calls it and none re-implements the walk.

### 3.1 The node model

A node is either a **leaf** — a rendered value — or an **aggregate** — a type
name and a way to enumerate its children. Nothing is materialised until a
caller asks for it.

```rust
/// Where a node lives: a variable, then a path of steps into it.
pub struct NodePath { var_index: u16, steps: Vec<Step> }
pub enum Step { Field(u16), Element(u32) }   // field ordinal; flat element index

pub struct Node {
    pub name: String,          // "pt", "x", "[2]", "[1,0]"
    pub path: String,          // IEC access path: "pt.x", "items[2].inner.values[1]"
    pub type_name: String,     // "REAL", "Point", "ARRAY [1..3] OF Point"
    pub value: RenderedValue,  // leaf: the value; aggregate: the type name
    pub children: Children,    // None | Fields(count) | Elements(count)
}

impl VariableRenderer {
    pub fn root(&self, index: u16, raw: u64, data_region: &[u8]) -> Node;
    pub fn children(&self, path: &NodePath, data_region: &[u8],
                    range: Range<u32>) -> Vec<Node>;
}
```

`render`, `name` and `line` keep their signatures; `root` is what they are
built from.

### 3.2 Aggregate nodes

**REQ-VI-container-060** A variable or field whose type reference resolves to
a `COMPOSITE` or `ARRAY` renders as its type name (`Point`,
`ARRAY [1..3] OF Point`, `TON`), is marked valid, and reports its child count:
the number of fields, or the product of the dimension sizes.

**REQ-VI-container-061** A `COMPOSITE` node's children are its fields in
declaration order, named as declared, each with the field's type name and
the path `<parent path>.<field name>`.

**REQ-VI-container-062** An `ARRAY` node's children are its elements in
row-major order, named `[i]` for one dimension and `[i,j]` for several, using
the declared bounds (`ARRAY[1..3]` yields `[1]`, `[2]`, `[3]`), each with the
element type name and the path `<parent path>[i,j]`.

**REQ-VI-container-063** A child's contents begin at the parent's base plus
the field's `byte_offset`, or plus `flat_index * element_stride`; the root's
base is its `VarNameEntry.data_offset`. Nesting composes by addition to any
depth.

**REQ-VI-container-064** Each child's own `type_ref` decides whether it is a
leaf or an aggregate, by the same rules, so `items[2].inner.values[1]`
resolves array → structure → structure → array → leaf.

### 3.3 Leaf nodes

**REQ-VI-container-065** A `SCALAR` leaf reads 8 bytes little-endian at its
location and renders them by its tag under the rules of
[Variable Value Rendering](variable-value-rendering.md) — the same function
that renders a variable slot, so a field and a variable of the same type and
value produce the same text.

**REQ-VI-container-066** An `ENUM` leaf reads the ordinal at its location and
renders `<VALUE_NAME> (<ordinal>)` from the `ENUM_DEF` entry its reference
indexes, falling back to the signed decimal when the ordinal has no name
(REQ-VR-container-050, REQ-VR-container-051).

**REQ-VI-container-067** A `STRING` leaf renders the ADR-0035 string at its
location under REQ-VR-container-030 through REQ-VR-container-032; the
encoding comes from the header's `char_width`, and the type name shown is
`STRING` or `WSTRING` accordingly.

### 3.4 Values that cannot be read

**REQ-VI-container-068** A node whose location does not fit the data region
— its base plus its size, or its string header plus its payload, extends past
the end — renders `<invalid>`, is marked invalid, and has no children.

**REQ-VI-container-069** A variable or field whose type reference is
unresolved (REQ-VI-container-007) renders `<TYPE_NAME>` in angle brackets, is
marked invalid, and has no children. This is REQ-VR-container-043 restated
for the case that still applies: a layout the container does not describe.

### 3.5 Flat surfaces

**REQ-VI-container-070** `line` for an aggregate variable produces one line
per leaf reachable from it, depth-first in child order, each
`<path>: <value>` with the path in IEC syntax (`pt.x: 1.0`,
`items[2].inner.values[1]: 42`). An unresolved aggregate produces the single
line `<name>: <TYPE_NAME>` it produces today.

Paths rather than indentation: the path is the context, every line stands on
its own in a diff, and a test can assert one line without reproducing the
lines around it. Indentation would spend columns to encode what the path
already says.

## 4. DAP server

The `variables` request already dispatches on `variablesReference`. This
design makes references plural.

**REQ-VI-vm-cli-080** Each aggregate node in a `variables` response carries a
non-zero `variablesReference`, allocated when the node is returned and valid
until the next resume; a reference the server did not issue in the current
stop yields an empty list.

**REQ-VI-vm-cli-081** A `variables` request for an aggregate's reference
returns its children per §3.2, honouring the request's `start` and `count`
arguments so a client can page a large array.

**REQ-VI-vm-cli-082** When the client declared `supportsVariablePaging`, an
array node reports `indexedVariables` equal to its element count and a
composite node reports `namedVariables` equal to its field count.

**REQ-VI-vm-cli-083** Each child carries `evaluateName` equal to its IEC path,
and `type` equal to its type name.

**REQ-VI-vm-cli-084** A leaf node has `variablesReference` zero. An
unresolved or unreadable aggregate (§3.4) has `variablesReference` zero and
shows its placeholder as its value.

The `Program` and `Runtime` scopes, and their `variablesReference` handles,
are unchanged. Which scope lists a variable is the concern of
[Debugger Support — Scopes](debugger-support.md#scopes); what a variable
expands into is the concern of this design, and does not depend on the scope.

### `--dump-vars`

**REQ-VI-vm-cli-085** `ironplcvm run --dump-vars` writes the lines of
REQ-VI-container-070, so an end-to-end test can assert the contents of a
structure or array without a DAP client. REQ-VC-vm-cli-008 in
[VM CLI](vm-cli.md) is amended to read `<path>: <value>` when this lands.

## 5. Worked example

```iecst
TYPE Point : STRUCT
    x : REAL;
    y : REAL;
END_STRUCT; END_TYPE

TYPE Color : (RED, GREEN, BLUE); END_TYPE

PROGRAM Main
VAR
    pt    : Point := (x := 1.0, y := 2.0);
    pts   : ARRAY[1..2] OF Point;
    hue   : Color := GREEN;
    timer : TON;
    label : STRING[8] := 'ready';
END_VAR
END_PROGRAM
```

Debug section (names abbreviated):

```
ENUM_DEF        [0] Color: RED, GREEN, BLUE

COMPOSITE_TYPE  0x0010 FUNCTION_BLOCK "TON"   byte_size 48
                    IN  VAR_INPUT  @0   SCALAR BOOL
                    PT  VAR_INPUT  @8   SCALAR TIME
                    Q   VAR_OUTPUT @16  SCALAR BOOL
                    ET  VAR_OUTPUT @24  SCALAR TIME
                0x8000 STRUCT "Point"          byte_size 16
                    x   VAR        @0   SCALAR REAL
                    y   VAR        @8   SCALAR REAL

ARRAY_TYPE      [0] element COMPOSITE 0x8000, stride 16, dims [1..2]

VAR_NAME        0 pt     COMPOSITE 0x8000  data_offset 0    "Point"
                1 pts    ARRAY 0           data_offset 16   "ARRAY [1..2] OF Point"
                2 hue    ENUM 0            data_offset 0    "Color"
                3 timer  COMPOSITE 0x0010  data_offset 48   "TON"
                4 label  STRING 8          data_offset 96   "STRING"
```

The Variables pane after one scan:

```
pt : Point
  x : REAL = 1.0
  y : REAL = 2.0
pts : ARRAY [1..2] OF Point
  [1] : Point
    x : REAL = 0.0
    y : REAL = 0.0
  [2] : Point
    …
hue : Color = GREEN (1)
timer : TON
  IN : BOOL = FALSE
  PT : TIME = T#0ms
  Q : BOOL = FALSE
  ET : TIME = T#0ms
label : STRING = 'ready'
```

`--dump-vars`:

```
pt.x: 1.0
pt.y: 2.0
pts[1].x: 0.0
pts[1].y: 0.0
pts[2].x: 0.0
pts[2].y: 0.0
hue: GREEN (1)
timer.IN: FALSE
timer.PT: T#0ms
timer.Q: FALSE
timer.ET: T#0ms
label: 'ready'
```

Reading `pts[2].y`: base 16 (from `pts`), plus flat index 1 × stride 16,
plus field offset 8 = byte 40 of the data region, 8 bytes, tag `REAL`.

## 6. Size

For a program with *T* aggregate types, *F* fields across them, *A* array
types and *V* variables, the new bytes are roughly `T × (10 + name) + F ×
(9 + name) + A × (8 + 8 per dimension) + 6 × V`. No term depends on element
counts. A `ARRAY[1..10000] OF Point` costs one `ARRAY_TYPE` entry, one
`COMPOSITE_TYPE` entry and one `VarNameEntry`.

## 7. Implementation sequence

Each step is independently reviewable and leaves every existing rendering
unchanged until the last.

1. **Container.** Type references, `COMPOSITE_TYPE`, `ARRAY_TYPE`, the revised
   `VarNameEntry`, retirement of `STRING_LAYOUT`, format version 4. Codegen
   emits only what it emits today (strings, enumerations) through the new
   fields. The renderer reads strings from `data_offset` and enumerations
   through the `ENUM` kind. `ENUM_DEF` is sorted. Every existing rendering
   test passes unchanged.
2. **Codegen.** Emit the layout tables for structures, arrays, arrays of
   structures, stdlib and user FB instances, function locals. The consistency
   tests of §2.
3. **Renderer.** The tree API of §3 and the `--dump-vars` lines, with
   end-to-end tests that compile a program, run it, and assert leaf lines.
4. **DAP.** Expandable variables per §4. The debugging reference page in the
   documentation gains a structured-variable example.

## 8. Amendments to other documents

Landing this design changes claims that other documents currently make and
test. Each is amended by the PR that changes the behaviour, not before:

| Document | Claim | Change |
|----------|-------|--------|
| [Bytecode Container Format](bytecode-container-format.md) | REQ-CF-container-003: format version is 3 | Version 4 |
| [Bytecode Container Format](bytecode-container-format.md) | `VarNameEntry` layout, tag registry, `iec_type_tag` table | Per §1 |
| [Enumeration Code Generation](enumeration-codegen.md) | REQ-EN-codegen-012: enumeration variables carry tag `DINT` and the type name | Kind `ENUM` with the `ENUM_DEF` index; the type name is still carried |
| [Variable Value Rendering](variable-value-rendering.md) | REQ-VR-container-043: aggregates render as `<TYPE_NAME>` | Only when the type reference is unresolved (REQ-VI-container-069) |
| [VM CLI](vm-cli.md) | REQ-VC-vm-cli-008: `<name>: <value>` | `<path>: <value>` |
| [ADR-0019](../adrs/0019-type-encoding-in-debug-variable-names.md) | Tag table; "Future: ENUM Display" | Postscript: tags 25–27 retired, enumerations resolved by index |
