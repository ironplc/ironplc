# Plan: `pou_lineage` reports standard library POUs

Fixes [#1164](https://github.com/ironplc/ironplc/issues/1164).

## Problem

`pou_lineage` returns an empty `upstream` list for a POU whose only
dependencies are standard library function blocks:

```st
PROGRAM MotorStartStop
VAR
    Star_Timer : TON;
END_VAR
Star_Timer(IN := ..., PT := T#5s);
END_PROGRAM
```

`TON` never appears. An agent using the tool to decide which POUs to pull
into context — or to understand the dependency graph before a refactor —
cannot tell a POU with ten function block instances from one with none.

The edge is built correctly and then discarded. `record_variables`
(`compiler/mcp/src/tools/pou_lineage.rs`) sees the variable as
`InitialValueAssignmentKind::FunctionBlock { type_name: TON }` — the
late-bound resolver resolves stdlib function block variables to exactly
that kind
(`compiler/analyzer/src/xform_resolve_late_bound_type_initializer.rs`) —
and calls `add_edge("MotorStartStop", "TON")`. `PouGraph::add_edge` then
drops it:

```rust
if !self.display.contains_key(&e_lower) {
    return; // ignore references to non-POUs (e.g. stdlib functions)
}
```

`display` is populated only from `Library::elements`. Standard library
function blocks are never library elements; they are built in
`analyzer/src/intermediates/stdlib_function_block.rs` and registered
directly into the type environment by `stages::resolve_types`. The same
guard drops calls to standard library *functions* (`MAX`, `SEL`,
`INT_TO_REAL`, …) recorded by `ReferenceCollector::visit_function`, which
is the identical defect one node kind over.

## Design

Standard library POUs are POUs. Register them as graph nodes so they are
findable through lineage in both directions, and label every entry with
where it came from so a caller can tell which dependencies correspond to
a file it must supply and which are built into the compiler.

### Classifying a POU

A POU is standard library iff its declaration span is builtin.
`SourceSpan::builtin()` already marks both halves of the standard
library:

- function blocks — `stdlib_function_block.rs` builds every one with
  `SourceSpan::builtin()`, asserted by its own
  `stdlib_function_blocks_have_builtin_span` test;
- functions — `FunctionSignature::is_stdlib()` is defined as
  `self.span.is_builtin()`.

Nothing else is needed. Compatibility libraries are files on disk that
`run_semantic_analysis` merges into the analyzed library like any other
source, so their POUs carry real spans and classify as `user` with no
special case — which is what they are.

Deriving the node set from the semantic environments rather than from the
`phf_set` in `analyzer/src/stdlib.rs` keeps it accurate for free: the
environments already reflect dialect and feature-flag gating (`SIZEOF` is
registered only under `allow_sizeof`), and a future standard library
addition needs no change here.

### Response shape

`upstream` and `downstream` become arrays of objects:

```json
{
  "ok": true,
  "found": true,
  "pou": "MotorStartStop",
  "upstream": [
    { "name": "Scale", "source": "user" },
    { "name": "TON",   "source": "stdlib" }
  ],
  "downstream": [],
  "diagnostics": []
}
```

This is a breaking change to the tool's JSON contract. The alternative —
keeping name arrays and adding a sibling `stdlib` list — makes every
caller cross-reference two fields to classify one entry, which is the
wrong shape for the decision the field exists to support. The MCP server
is experimental and its consumers are in-tree.

Ordering is unchanged: case-insensitive by name, both origins in one
list.

### Lineage queries for standard library POUs

Registering standard library POUs as nodes makes them addressable:
`pou_lineage(pou: "TON")` returns `found: true` with every POU that
instantiates `TON` as `downstream`. Standard library POUs have no
outgoing edges, so they are always leaves of an `upstream` walk. Names
that are neither user nor standard library POUs are still not found, so
REQ-TOL-mcp-231 is unaffected.

## Implementation

### `compiler/mcp/src/tools/pou_lineage.rs`

- Add `PouSource { User, Stdlib }`, serialized lowercase, and
  `LineageEntry { name, source }`.
- `PouLineageResponse::upstream` / `downstream` become
  `Vec<LineageEntry>`.
- `PouGraph` gains `source: BTreeMap<String, PouSource>`; `add_pou` takes
  the origin and, like `display`, keeps the first registration.
  `transitive` returns entries carrying their origin.
- `build_graph` takes the `SemanticContext` alongside the `Library`.
  After registering user POUs from `library.elements` it registers, as
  `Stdlib`, every type in `context.types()` whose span is builtin and
  whose representation is a function block, and every signature in
  `context.functions()` where `is_stdlib()`.
- `build_response` reads `project.semantic_context()` next to
  `project.analyzed_library()`. When the context is absent the graph is
  built from the library alone, so the existing "analysis produced no
  context" paths are unchanged.
- Delete the guard comment's premise: unresolved references still get no
  edge, but standard library references are now resolvable.

### Spec

- `specs/design/mcp-server.md` — REQ-TOL-mcp-230 restated for the object
  entries; example output updated.
- New **REQ-TOL-mcp-232**: standard library POUs appear in lineage with
  `source: "stdlib"` and are addressable as `pou`.

### Tests

- `compiler/mcp/src/spec_conformance.rs` — update the REQ-TOL-mcp-230 and
  -231 tests for the new entry shape; add the REQ-TOL-mcp-232 test
  (a program instantiating `TON` has it upstream tagged `stdlib`, and
  `pou_lineage(pou: "TON")` finds the program downstream).
- `pou_lineage.rs` unit tests — stdlib function block upstream, stdlib
  function call upstream, user function block still `user`, compatibility
  and user POUs unaffected, ordering across mixed origins.
- `compiler/mcp/tests/cli.rs` — wire-level case asserting
  `"source":"stdlib"` for a program using `TON`.
- `compiler/mcp/src/server.rs` — tool description states that builtins
  are included.

## Verification

`cd compiler && just`
