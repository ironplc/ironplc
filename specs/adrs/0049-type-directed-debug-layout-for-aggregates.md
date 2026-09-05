# Type-Directed Layout Tables in the Debug Section for Aggregate Inspection

status: proposed
date: 2026-09-05

## Context and Problem Statement

A `STRUCT`, `ARRAY` or `FUNCTION_BLOCK` instance variable owns one slot in
the variable table, and that slot holds the byte offset of its contents in
the data region (ADR-0017, ADR-0026). Its fields and elements are laid out
at compile-time-constant offsets from there (ADR-0027). The debug section
records none of this. A `VarNameEntry` says the variable is a `STRUCT` named
`Point`; it does not say where `x` and `y` are, or what type they are.

So every surface that shows variables — the DAP debugger, `--dump-vars`, the
playground, the LSP run panel — stops at the aggregate's type name. The
[Variable Value Rendering](../design/variable-value-rendering.md) design made
that stop deliberate (REQ-VR-container-043) rather than print the offset as
if it were a value, and left the contents for "a layout sub-table the
container does not yet carry."

This ADR decides the shape of that sub-table. The concrete wire format is in
the [Variable Inspection Model](../design/variable-inspection-model.md).

## Decision Drivers

* **Bounded size.** A `ARRAY[1..10000] OF DINT` must not cost ten thousand
  debug entries. Debug info scales with the number of *types* and
  *variables*, not with the number of elements.
* **Lazy expansion.** The DAP `variables` request expands one node at a
  time; the format should let a reader open `items[7].inner` without
  decoding anything else.
* **Independent of the slot model.** ADR-0026 chose 8-byte slots per field
  with a stated migration path to packed byte layout. The debug tables must
  survive that migration unchanged.
* **One place per fact.** Every number in the table must be the number
  codegen emitted into the bytecode, produced by the same function, so the
  debugger and the VM cannot disagree about where a field is.
* **Structure without a VM.** The shape of the variable tree — names,
  types, nesting, offsets — should be readable from the container alone,
  so the `.iplc` file viewer can show it and so tests need no running VM.
* **Off the execution path.** The type section is read by the verifier and
  the VM; display-only information belongs in the strippable debug section
  (ADR-0019).

## Considered Options

* **Option A: flatten every leaf.** One debug entry per leaf value
  (`pt.x`, `pt.y`, `arr[1]`, …) with its offset, tag and name.
* **Option B: type-directed layout tables.** Describe each aggregate *type*
  once (fields with offsets, or element type with bounds and stride) and
  give each aggregate variable a reference to its type plus the static
  offset of its contents. Readers walk the type tree.
* **Option C: extend the type section.** Add names and bounds to the FB
  type descriptors and array descriptors the verifier already reads.

Within Option B, three further choices:

* **B1 where the per-variable link lives:** a separate sub-table keyed by
  `var_index` (as `STRING_LAYOUT` is today), or fields on `VarNameEntry`.
* **B2 what a composite field records about indirection:** an explicit
  `inline` flag per field, or nothing, with indirection determined by the
  field's type kind.
* **B3 offsets in slots or in bytes.**

## Decision Outcome

Chosen option: **Option B, type-directed layout tables**, with the link on
`VarNameEntry` (B1), no `inline` flag (B2), and byte offsets (B3).

### The tables

Two sub-tables describe types:

- `COMPOSITE_TYPE` — one entry per structure type and per function block
  type: name, kind, total byte size, and fields in declaration order, each
  with a name, an IEC section, a byte offset, and a type reference.
- `ARRAY_TYPE` — one entry per array type: element type reference, element
  byte stride, and the declared bounds of every dimension.

A **type reference** is three bytes: a `kind` (scalar, enumeration, string,
composite, array) and a `u16` id whose meaning depends on the kind. For a
scalar the id is the IEC type tag from ADR-0019, so the tag-driven rendering
that exists today is unchanged. For an enumeration the id indexes `ENUM_DEF`.

`VarNameEntry` replaces its `iec_type_tag` byte with a type reference and
gains the static `data_offset` of the variable's contents. The
`STRING_LAYOUT` sub-table is retired: a string variable is a `VarNameEntry`
whose reference has the string kind and whose `data_offset` is the one that
table used to carry.

### Why B1: the link belongs on `VarNameEntry`

A separate table keyed by `var_index` is what `STRING_LAYOUT` does, and it
has the cost such a join always has: every render correlates two tables, and
the format admits states it never defines — an aggregate with no layout
entry, or a layout entry with no name. Putting the reference and the offset
on the one entry every variable already has removes the join and the
undefined states. It also retires `STRING_LAYOUT` rather than leaving two
mechanisms for one fact.

The price is a wire-format change to `VarNameEntry` and a format version
bump. There is no deployed bytecode to migrate: the compiler and VM ship
together, the reader rejects any other version outright, and ADR-0033 and
ADR-0035 set the precedent of bumping rather than carrying two readers.

### Why B2: no `inline` flag

Every composite field the compiler lays out today is inline: a nested
structure's fields, an embedded array's elements and an FB instance's fields
all sit contiguously inside the parent's region (ADR-0026). A per-field
`inline` byte would be a constant. If a by-reference field ever appears
(`REF_TO`, `VAR_IN_OUT`), it is a pointer because of how it is *bound*, not
because of what it points at, and it will need a binding axis on the field
rather than a resurrected boolean. Until then, a codegen test asserts the
invariant instead of the format carrying it.

### Why B3: bytes, not slots

ADR-0026's migration to packed layout changes what a slot is. A table that
records byte offsets, byte strides and byte sizes describes either layout
without change; a table in slot units would have to be re-specified. The
cost is two bytes per offset, which is nothing against the name strings the
entries already carry.

### Consequences

* Good, because debug info grows with the number of types and variables. A
  `ARRAY[1..10000] OF Point` costs one `ARRAY_TYPE` entry, one
  `COMPOSITE_TYPE` entry and one `VarNameEntry`.
* Good, because a reader opens any node from the type tree and one base
  offset, without decoding its siblings — exactly the DAP expansion model.
* Good, because the tree's shape is static: the file viewer can show it and
  the conformance tests can assert it without running the program.
* Good, because the VM, the verifier and the instruction set are untouched;
  the debugger reads the data region it can already see.
* Good, because the format makes the existing `STRUCT`/`ARRAY`/`FB_INSTANCE`
  type tags (25–27) and `STRING_LAYOUT` redundant, and removes them rather
  than keeping two ways to say one thing.
* Bad, because `VarNameEntry` grows by six bytes per variable. Scalars pay
  for a `data_offset` they do not use.
* Bad, because the format version bumps to 4 and every `.iplc` on disk must
  be recompiled. There are no deployed containers; the cost is fixtures and
  the version test.
* Neutral, because enumeration variables move from "DINT tag plus name
  lookup" to an explicit enumeration kind. REQ-EN-codegen-012 in
  [Enumeration Code Generation](../design/enumeration-codegen.md) is amended
  when the change lands; the rendering the user sees does not change.

## Pros and Cons of the Options

### Option A: flatten every leaf

* Good, because the reader is trivial: look up a path, get an offset and a tag.
* Bad, because size is proportional to element count. Arrays are the common
  aggregate in PLC programs and they are large.
* Bad, because a name string per leaf repeats the field names of a
  structure once per element of every array of that structure.
* Bad, because it cannot express "this is an array of 3" for a client that
  wants to page — the reader would have to parse names back into indices.

### Option C: extend the type section

* Good, because FB type descriptors and array descriptors already exist and
  already carry field counts and element counts.
* Bad, because the type section is on the execution path; ADR-0019 rejected
  putting display-only information there for exactly this reason.
* Bad, because array descriptors are deduplicated by `(element_type,
  total_elements)`: two arrays with different bounds but the same size share
  one descriptor, so the descriptor cannot carry bounds. The debug
  `ARRAY_TYPE` is keyed by the source type instead, and a consistency test
  bridges the two encodings.
* Bad, because structures have no type descriptor at all; they are flat
  `SLOT` arrays to the verifier, and inventing one only the debugger reads
  is the debug section by another name.

## More Information

This ADR resolves the "structure type descriptors in the container — for
debugger/verifier support" item deferred by
[Structure Code Generation](../design/structure-codegen-memory-layout.md),
and the FB type/field name tables that
[Debugger Support](../design/debugger-support.md) carried as "in development"
under tag 5. Tag 5 is reclaimed for `COMPOSITE_TYPE`, which subsumes them.

The draft in pull request #1115 proposed the same tables with a separate
`VAR_TYPE_REF` sub-table and an `inline` flag; the review of that draft is
where B1 and B2 were decided.
