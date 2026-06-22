# Spec: Variable Inspection Model

## Overview

This spec defines how IronPLC presents PLC variables for inspection — in
the `ironplcvm` CLI dump today and in the DAP debugger's `variables` view
later. It exists because the first cut (`ironplcvm run --dump-vars
--group-by-scope`) grouped variables by **FUNCTION_BLOCK type**, which is
the wrong unit: a FB type has no state; its **instances** do, and a program
may have many of them.

The model here is a single source of truth that both consumers render:

- The **CLI dump** renders it as an indented text tree (with a flat list
  as a fallback).
- The **DAP server** renders it as the `variables` / `variablesReference`
  lazy-expansion tree (`specs/design/debugger-support.md`).

Keeping one model prevents the CLI and the debugger from drifting into two
incompatible notions of "what a variable is."

### Requirements and conformance

Testable claims carry **`REQ-VI-NNN`** requirement IDs (ID-first, one per
line; table claims use a Requirement column), per
[Development Standards — Design Requirement](steering/development-standards.md)
and [Spec Conformance Testing](spec-conformance-testing.md). The `VI`
prefix is for **V**ariable **I**nspection.

These requirements are **not yet wired into conformance enforcement**: no
crate `build.rs` registers this file, so the IDs are linkable documentation
today (the same status as `subrange-codegen.md` and `time-literals.md`
before their features shipped). Each implementation phase
([Phasing](#phasing)) adds this file to the relevant crate's `build.rs` and
lands a `#[spec_test(REQ_VI_NNN)]` test — `#[ignore]`d until the code
exists — for the requirements it implements.

This spec builds on and amends:

- **[Debugger Support](debugger-support.md)** — Layer 1 debug info and the
  DAP `variables` request. This spec supersedes that document's Gaps #5/#6
  (FB type/field tables) and corrects its debug-section tag registry (see
  [Debug-section tag reconciliation](#debug-section-tag-reconciliation)).
- **[Bytecode Container Format](bytecode-container-format.md)** — the debug
  section and the type section (array descriptors).
- **[Runtime Execution Model](runtime-execution-model.md)** — the data
  region and FB copy-in/copy-out.

## Problem statement

### Two things called "scope" were conflated

`--group-by-scope` produced output like:

```
[ACCUMULATOR]
  step : DINT = 3  (VAR_INPUT)
  total : DINT = 6  (VAR_OUTPUT)
```

Two defects:

1. **Type, not instance.** `[ACCUMULATOR]` is a FB *type*. With
   `m1, m2 : MotorController` there is no single thing to label, and both
   instances' state would collapse under one header.
2. **Scratch, not state.** A user FB's persistent fields live **per
   instance in the data region**. On every `FB_CALL` the VM copies that
   instance's fields into a **shared** set of variable-table slots (the FB
   body's locals), runs the body, and copies them back
   (`compiler/vm/src/vm.rs:2084` copy-in,
   `compiler/vm/src/frame_stack.rs:43` copy-out). Those body slots are
   reused across every instance, so after a scan they hold whatever the
   last call left behind. The dump was showing leftover scratch, not any
   instance's real state.

### Ground truth of the data model

(From a survey of `compiler/codegen/src/` and `compiler/vm/src/`.)

- A composite variable — FB instance, STRUCT, or ARRAY — gets **one
  variable-table slot that holds the byte offset of its region in the data
  region** (`compile_setup.rs:474`, `compile_struct.rs:567`,
  `compile_array.rs`). The real bytes live in the data region at that
  offset.
- FB instance fields are 8-byte slots laid out contiguously at the
  instance's `data_offset`; each instance gets its own region; all
  instances of a type share the body's var-table partition
  (`compile_setup.rs:162`, `vm.rs:2085`).
- The debug section records **none** of this composite layout today. An FB
  instance variable emits a `VarNameEntry` with `iec_type_tag = OTHER` and
  a `type_name` string (e.g. `"ACCUMULATOR"`) and nothing else
  (`compile_setup.rs:283`). Structs are the same. Arrays additionally have
  a runtime `ArrayDescriptor` in the **type section**
  (`type_section.rs:80`) but no debug entry.
- `StringLayoutEntry` (debug tag 4) already maps `var_index →
  (data_offset, max_length)` (`debug_section.rs:137`). It is the precedent
  for tying a variable to a data-region layout; the composite tables below
  generalize it.

So a faithful tree must (a) emit field-layout debug info we do not yet
produce, and (b) walk the **data region** at each instance's offset. That
is why this is a design doc, not a patch.

## The two hierarchies

A debugger surfaces two *orthogonal* trees. Conflating them is the original
sin of `--group-by-scope`.

### 1. The instance / data tree (static, always inspectable)

Rooted at the program. Always meaningful — even on an idle VM between
scans — because it reflects persistent state in the data region and globals
in the variable table.

```
program (plc_main)
├─ counter : DINT = 3                 (global / program VAR)
├─ acc : accumulator                  (FB instance — expandable)
│  ├─ step  : DINT = 3
│  └─ total : DINT = 6
├─ setpoints : ARRAY[0..2] OF INT     (array — expandable)
│  ├─ [0] : INT = 10
│  ├─ [1] : INT = 20
│  └─ [2] : INT = 30
└─ cfg : Settings                     (struct — expandable)
   ├─ gain : REAL = 1.5
   └─ timer : TON                     (nested FB instance — expandable)
      ├─ IN : BOOL = TRUE
      └─ ...
```

Composite nodes (FB instance, struct, array) expand into children;
children may themselves be composite (nested FBs, struct-of-array, etc.).
This is the tree the CLI dump renders and the DAP `variables` request
serves for the program/global scope.

### 2. The call-stack view (runtime, only while paused)

Meaningful **only** when execution is paused inside a call. A FUNCTION's
parameters and locals are transient per-call values with no instances;
they exist on a frame of the call stack, not in the data tree. This view is
`stackTrace` → `scopes` → `variables(frame)` in DAP, and has no CLI-dump
representation (the dump runs on a *stopped* VM with an empty frame stack).

This spec specifies tree #1 in full and defines where tree #2 plugs in; the
mechanics of pausing and frame inspection belong to
`debugger-support.md` Layer 2.

## The node model

One recursive abstraction both consumers build, independent of rendering:

```
VarNode {
    name: String,            // "counter", "acc", "[0]", "gain"
    type_name: String,       // "DINT", "accumulator", "ARRAY[0..2] OF INT"
    value: Option<Value>,    // Some for scalars; None for composites (a header)
    section: Option<VarSection>, // IEC section, for top-level frame/program vars
    children: Children,      // Leaf | Fields(Vec<VarNode>) | Elements(Vec<VarNode>)
}
```

**REQ-VI-001** A scalar variable is a **leaf** node: its `value` is `Some`,
formatted by `format_variable_value(raw, iec_type_tag)`
(`compiler/container/src/debug_format.rs`).

**REQ-VI-002** An FB-instance or STRUCT variable is a **composite** node:
its `value` is `None` (it is a header) and its children are its fields, in
declaration order.

**REQ-VI-003** An ARRAY variable is a composite node whose children are its
elements, each named `[i]` for index `i` over the array's IEC bounds.

**REQ-VI-004** Building a node is lazy: a composite node can be produced
without expanding its children, so the DAP server hands back a
`variablesReference` and expands on demand, while the CLI expands eagerly
to a configurable depth. Expanding the same node twice yields identical
children.

### Resolving a composite node's bytes

To expand a composite node the inspector needs a **base offset** into the
data region and a **field layout**:

**REQ-VI-005** The base offset of a top-level composite variable is the
`data_offset` stored in its variable-table slot (`compile_setup.rs:474`);
the inspector reads it from the live VM rather than from debug info.

**REQ-VI-006** Field values are read from the data region at `base_offset +
field.byte_offset`, dispatched on the field's type reference: a scalar
field is formatted as a leaf; a composite field is recursed into with its
own resolved base offset.

The field layout comes from the new composite-type debug tables below.

*Open item:* confirm during implementation whether a nested composite field
stores an inline region (structs lay fields out contiguously) or an offset
pointer (as top-level FB instances do); `FieldEntry.inline` (below) encodes
which, and REQ-VI-006's base resolution honors that flag.

## Required debug info

The tree needs layout facts the compiler has at codegen time but currently
discards. This section defines the additions. All are **new debug-section
sub-tables** (the directory format already lets readers skip unknown tags,
so old readers are unaffected).

### Debug-section tag reconciliation

`debugger-support.md`'s tag registry is **stale**: it lists tag 4 as
`FB_TYPE_NAME` and tag 5 as `FB_FIELD_NAME`, but the implemented code uses
tag 4 for `STRING_LAYOUT` (`debug_section.rs:13`). Implemented tags today:

| Tag | Name | Status |
|-----|------|--------|
| 1 | LINE_MAP | implemented |
| 2 | VAR_NAME | implemented |
| 3 | FUNC_NAME | implemented |
| 4 | STRING_LAYOUT | implemented |
| 6 | SOURCE_FILE | implemented |
| 9 | ENUM_DEF | implemented |

**REQ-VI-010** Debug tag 4 remains `STRING_LAYOUT` (as implemented); this
spec does not redefine it. `debugger-support.md`'s registry must be updated
to match (action item, not a format change).

This spec assigns fresh tags for composite layout, avoiding the collision:

| Requirement | Tag | Name | Purpose |
|-------------|-----|------|---------|
| **REQ-VI-011** | 10 | COMPOSITE_TYPE | FB + struct type descriptors (name + fields) |
| **REQ-VI-012** | 11 | VAR_TYPE_REF | var_index → type reference (for composite/array vars) |
| **REQ-VI-013** | 12 | ARRAY_TYPE | array layout for debug (element type + dims) |

Tags 5, 7, 8 (the stale FB_*/LD/FBD reservations) are left unused and
re-marked reserved.

### TypeRef encoding

A 3-byte reference naming what a variable or field *is*, so the inspector
knows how to expand it:

| Requirement | Offset | Field | Type | Meaning |
|-------------|--------|-------|------|---------|
| **REQ-VI-020** | 0 | kind | u8 | 0 = scalar, 1 = composite, 2 = array |
| **REQ-VI-021** | 1 | id | u16 | scalar: `iec_type_tag`; composite: COMPOSITE_TYPE id; array: ARRAY_TYPE id |

### Tag 10 — COMPOSITE_TYPE

**REQ-VI-030** A single COMPOSITE_TYPE table unifies FB types and structs:
both are "a named record of fields at offsets," with one descriptor per
user composite type.

`CompositeTypeEntry`:

| Requirement | Field | Type | Description |
|-------------|-------|------|-------------|
| **REQ-VI-031** | type_id | u16 | Matches `FbInstanceInfo.type_id` for FBs |
| **REQ-VI-032** | kind | u8 | 0 = struct, 1 = function_block |
| **REQ-VI-033** | name | String | Source type name, e.g. `"ACCUMULATOR"`, `"Settings"` |
| **REQ-VI-034** | fields | FieldEntry[] | One per field, in declaration order |

`FieldEntry`:

| Requirement | Field | Type | Description |
|-------------|-------|------|-------------|
| **REQ-VI-035** | name | String | Field name, e.g. `"step"`, `"gain"` |
| **REQ-VI-036** | byte_offset | u16 | Offset within the instance's data region |
| **REQ-VI-037** | inline | u8 | 1 = field bytes are inline; 0 = field slot holds a data_offset pointer |
| **REQ-VI-038** | type_ref | TypeRef | scalar / nested composite / array |

`byte_offset` + `inline` + `type_ref` are exactly what
`build_struct_fields` (`compile_struct.rs:629`) and the FB field map
(`FbInstanceInfo.field_indices`, `compile.rs:786`) already compute
internally — this table just persists them.

### Tag 11 — VAR_TYPE_REF

**REQ-VI-040** A VAR_TYPE_REF entry links a named variable to its TypeRef
without touching the existing `VarNameEntry` wire format (preserving the
#1106 work and its conformance tests).

**REQ-VI-041** VAR_TYPE_REF entries are emitted **only** for composite and
array variables; scalars are fully described by `VarNameEntry.iec_type_tag`
already.

`VarTypeRefEntry`:

| Requirement | Field | Type | Description |
|-------------|-------|------|-------------|
| **REQ-VI-042** | function_id | FunctionId | Owner (`GLOBAL_SCOPE` for program/globals) |
| **REQ-VI-043** | var_index | VarIndex | Variable-table index within the owner |
| **REQ-VI-044** | type_ref | TypeRef | Composite or array reference |

### Tag 12 — ARRAY_TYPE

The type section already has a runtime `ArrayDescriptor` (element type,
count, element_extra — `type_section.rs:80`). For source-level display the
debug side adds dimension bounds and a nested `type_ref` for composite
element types.

`ArrayTypeEntry`:

| Requirement | Field | Type | Description |
|-------------|-------|------|-------------|
| **REQ-VI-050** | array_id | u16 | Referenced by a TypeRef with kind = array |
| **REQ-VI-051** | element_ref | TypeRef | Element scalar tag, or composite/array id for nested |
| **REQ-VI-052** | dims | Dim[] | One per dimension, each `lower:i32, upper:i32` (IEC bounds) |

### Codegen emission

All facts already exist at codegen time:

**REQ-VI-060** Codegen emits one COMPOSITE_TYPE entry per registered user FB
type (`ctx.user_fb_types`) and per struct type, from the field maps already
built.

**REQ-VI-061** At each composite/array variable assignment
(`compile_setup.rs`, `compile_fn.rs`), codegen emits the variable's
VAR_TYPE_REF alongside the existing `VarNameEntry`.

**REQ-VI-062** Codegen emits an ARRAY_TYPE entry alongside the existing
`add_array_descriptor` (`compile_array.rs:617`), carrying source bounds.

**REQ-VI-063** Built-in (stdlib) FB types (TON, CTU, …) get COMPOSITE_TYPE
entries with their standard field names, so `acc : TON` expands to
`IN/PT/Q/ET`. This is the work `debugger-support.md` filed as Gaps #5/#6.

## Consumer 1: CLI dump

### Surface

`--group-by-scope` is **removed** (it was never released — branch only).
The dump grows one rendering mode:

| Requirement | Option | Behavior |
|-------------|--------|----------|
| **REQ-VI-070** | `--dump-vars [PATH]` | Default flat, one-line-per-slot format. Unchanged; still spec-locked by REQ-VC-005/008/009. |
| **REQ-VI-071** | `--dump-vars [PATH] --tree` | Renders the instance/data tree (this spec). |
| **REQ-VI-072** | `--dump-vars [PATH] --tree` (no composite debug info) | Falls back to the flat format. |

The **flat list is the fallback**, per the project decision to demote the
old grouping rather than make a misleading grouped view the default.

### Tree rendering

**REQ-VI-073** `--tree` walks tree #1 from the program/global roots,
depth-first, indenting two spaces per level.

**REQ-VI-074** A scalar prints `name : type = value`; a composite prints a
header line `name : type` followed by its expanded children; an array
element prints `[i] : type = value`.

**REQ-VI-075** `--tree-depth N` bounds recursion depth (default unbounded);
cycles are impossible because IEC composites form a DAG.

### Worked example

```
plc_main
  counter : DINT = 3
  acc : ACCUMULATOR
    step : DINT = 3
    total : DINT = 6
  setpoints : ARRAY[0..2] OF INT
    [0] : INT = 10
    [1] : INT = 20
    [2] : INT = 30
```

## Consumer 2: DAP `variables`

The DAP server builds the same `VarNode` tree and maps it to the protocol:

**REQ-VI-080** `scopes` returns at least a **Program / Globals** scope
(tree #1 root) and, when paused, per-frame **Locals / Inputs / Outputs**
scopes (tree #2).

**REQ-VI-081** Each composite `VarNode` is returned as a DAP `Variable`
with a non-zero `variablesReference`; expanding it issues
`variables(reference)`, which lazily builds that node's children.

**REQ-VI-082** Array nodes report their length via `indexedVariables` so
clients can page large arrays.

**REQ-VI-083** Leaf value formatting reuses `format_variable_value`, so a
scalar renders identically in the CLI dump and the debugger.

Because both consumers build `VarNode` from the same debug tables and the
same data-region walk, the CLI dump and the debugger cannot disagree about
structure.

## Phasing

This is multi-PR. Each phase is independently useful and testable. Each
phase registers this file in the relevant crate's `build.rs` (if not
already) and lands `#[spec_test(REQ_VI_NNN)]` tests for the requirements it
implements.

1. **Debug info (codegen + container).** Tags 10/11/12 and the TypeRef
   encoding; emit for structs, user FBs, stdlib FBs, and arrays.
   *Requirements:* REQ-VI-010..013, 020..021, 030..052, 060..063. Register
   in `compiler/codegen/build.rs` (or `container`). Round-trip tests; no
   consumer change yet.
2. **`VarNode` builder (shared crate).** A pure function from
   `(container debug tables, data-region bytes, variable-table values)` to
   a `VarNode` tree. *Requirements:* REQ-VI-001..006. Unit-tested with
   hand-built containers; no I/O.
3. **CLI `--tree`.** Render the builder's output; remove
   `--group-by-scope`; keep flat as fallback. *Requirements:*
   REQ-VI-070..075. Register in `compiler/vm-cli/build.rs`; update
   `vm-cli.md`.
4. **DAP wiring.** Map `VarNode` to `variables`/`variablesReference` when
   the debugger lands (depends on `debugger-support.md` Layer 2).
   *Requirements:* REQ-VI-080..083.

## Out of scope

- Pausing, stepping, breakpoints, the call-stack frame mechanics — those
  are `debugger-support.md` Layer 2/3. This spec only defines what a frame
  scope *contains* once it exists.
- Writing/forcing variable values. Read-only inspection here.
- Multi-instance *debugging semantics* (which instance a breakpoint fires
  on) — the data tree shows all instances' state; firing rules are a later
  debugger concern.
- Pointer/REF_TO graph following beyond one hop; shown as the raw target
  index for now.

## Open questions

1. **Nested composite base offset.** Confirm whether struct/FB fields that
   are themselves composite store inline bytes or an offset pointer; the
   `FieldEntry.inline` flag is provisioned for both, but emission must set
   it correctly per codegen reality (`compile_struct.rs`).
2. **Stdlib FB field tables.** Source the standard FBs' field names/offsets
   from the existing intrinsic layout (`compiler/vm/src/builtin.rs`) vs. a
   hand-authored table — decide during Phase 1.
3. **`type_id` namespace.** Confirm FB `type_id`s and struct ids don't
   collide in one COMPOSITE_TYPE table; if they can, add `kind` to the key
   or partition the id space.
