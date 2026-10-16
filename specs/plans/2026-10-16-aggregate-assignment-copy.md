# Whole-Aggregate Assignment as a Lowered `Copy`

## Goal

Lower a whole array or structure assignment (`x := y`) to the lowered
program's `Copy` statement, and emit that `Copy` from the lowered route in the
bytecode backend, so these statements stop compiling from the AST. Behaviour
does not change: every shape the AST route compiles today emits the same
instructions from the lowered route, and every shape it fails on still fails
the same way.

On `main`, 16 top-level statements of the end-to-end suite fall back to the
AST because they assign a whole aggregate. After this change the 14 the AST
route compiles take the lowered route; the other two (`arr[1] := one` and
`r := f.p`) lower to a `Copy` the bridge declines, so they still fall back
and still fail as they do today.

## Design doc reference

- [`specs/design/lowered-program.md`](../design/lowered-program.md) §3.7
  specifies `StmtKind::Copy { dst: Place, src: Place }` for a whole aggregate,
  and **REQ-LOW-lowering-061**: the two places have the same type, or types of
  identical shape, as the analyzer's `value_type::check` accepts.
- §3.10 "Meaning of operations" gets a rule for what `Copy` means
  (see [Spec changes](#spec-changes)), as the
  [backend guide](../steering/backend-guide.md#when-the-ir-changes) requires
  for a new statement kind.
- "Delivery Constraints" describes the per-statement route this change
  extends.

## Architecture

### IR (`compiler/ir`)

- `StmtKind::Copy { dst: Place, src: Place }`, documented as replacing the
  whole value at `dst` with the whole value at `src`.
- `Stmt::copy(types, dst, src, span)`, exposed as `ProgramBuilder::copy`,
  checks:
  - both places belong to the same program, as the other constructors do;
  - `dst` is an aggregate: an array, a structure or a function block
    instance (the design's `Arg::Copy` covers instances, so the constructor
    admits them; lowering does not produce one yet);
  - REQ-LOW-lowering-061: `src.ty() == dst.ty()`, or both are arrays or both
    are structures of identical shape.

  A violation is a P9998 through `invariant::broken`.
- Identical shape is a new `TypeTable::same_shape(a, b)`, recursive over the
  table, mirroring the analyzer's `composite_accepted` (representation
  equality):
  - two arrays: equal dimensions (bounds, in order) and elements of the
    same shape;
  - two structures: the same number of fields, with equal names in order and
    fields of the same shape;
  - any other pair: equal ids, or equal `TypeKind`s for leaves that carry no
    ids (integers, reals, strings with their capacity, ...).

  The lowered table keeps no field defaults, which the analyzer's equality
  also compares, so `same_shape` accepts a superset of what analysis accepts.
  That is enough for an invariant check: analysis has already rejected the
  rest (P2037).
- `print.rs` prints it as `copy <dst> <src>`.
- Every match over `StmtKind` handles the new arm (no wildcard arms):
  `print.rs`, `loops.rs`, `codegen/src/lowered/{mod,stmt}.rs`.

### Lowering (`compiler/lowering/src/statement.rs`)

`store_at` today returns `None` when the place has no operation type. It
instead:

1. Asks whether the place's type is an array or a structure. A function
   block instance target (`a := b` between instances) still returns `None`:
   the AST route aliases instead of copying, and the routes would disagree.
2. Lowers the value as a place: only an `ExprKind::Variable` source, through
   the existing `variable_place`. Any other source returns `None`; in
   particular a function call with a structure result belongs to separate
   work.
3. Builds the statement with `ProgramBuilder::copy`. A source place whose type
   does not match is a defect the constructor reports.

Every shape in the goal lowers to a `Copy`, including the element target
(`arr[1] := one`) and the function block output source (`r := f.p`): the
lowered program is correct for them; only the bytecode bridge declines them.

### Bytecode bridge (`compiler/codegen/src/lowered/`)

- `can_emit` for `Copy` is true only when both places have a
  `storage_name` (a whole variable, or a field of the current instance,
  which codegen copies into a slot of that name) and that name resolves to an
  aggregate region in codegen's layout (`ctx.struct_vars`, or a
  non-`is_ref` entry of `ctx.array_vars`). This covers whole variables,
  globals through `VAR_EXTERNAL` and aggregate fields of a function block's
  own instance. It is false for every other `Copy`, which falls back to the
  AST and fails as today:
  - an element target;
  - a field of another instance as source;
  - an instance copy (never produced by lowering, but declined anyway).
- `emit` for `Copy` emits exactly the AST route's sequence
  (`compile_aggregate.rs`): load the source's slot at the default slot type
  an aggregate value has, then `COPY_REGION dst_var, dst_desc, src_desc`.
  The bridge tests compare the bytes with the AST route's.
- The new arms live in a small `lowered/aggregate.rs` module to keep
  `lowered/mod.rs` (445 lines) and `lowered/stmt.rs` (626 lines) focused.

## Prefactoring

None. The AST route (`compile_aggregate.rs`) is not changed: the goal is that
it becomes dead code and is deleted, so reshaping it to share code with the
bridge would be work thrown away.

The bridge therefore owns its own region lookup (`ctx.struct_vars`, or a
non-`is_ref` entry of `ctx.array_vars`) and its own two-instruction emission
in `lowered/aggregate.rs`. These repeat a few lines of
`compile_aggregate.rs` on purpose, for as long as both routes exist; when the
AST route is removed, the bridge's copy is the only one left. The bridge
tests pin the two to the same bytes until then.

No other prefactoring is needed:

- the IR change adds one variant and one constructor beside the existing
  ones;
- the lowering change is one branch in `store_at`;
- `storage_name` already gives the bridge the names it needs.

## Spec changes

In the core PR, in `specs/design/lowered-program.md`:

- §3.7: document `ProgramBuilder::copy` against REQ-LOW-lowering-061
  (aggregate destination, same type or identical shape), and add the
  constructor's name to the statement list's prose.
- §3.10: add the next `REQ-LOW-codegen-NNN` rule:

  > After a `Copy`, every element or field of `dst` holds the value the same
  > element or field of `src` held before it. A `Copy` of a place to itself
  > changes nothing.

  The end-to-end suite already has programs for it
  (`end_to_end_aggregate_copy.rs`, including `x := x`).

## File map

**Core PR**
- `compiler/ir/src/lowered/stmt.rs`: `StmtKind::Copy`, `Stmt::copy`.
- `compiler/ir/src/lowered/types.rs`: `TypeTable::same_shape`.
- `compiler/ir/src/lowered/program.rs`: `ProgramBuilder::copy`.
- `compiler/ir/src/lowered/print.rs`: `copy <dst> <src>`.
- `compiler/ir/src/lowered/loops.rs`: new `StmtKind` arm.
- `compiler/ir/src/lowered/tests.rs` (or a new `tests/copy.rs` if `tests.rs`,
  at 758 lines, would pass 1000): constructor and text-form tests.
- `compiler/lowering/src/statement.rs`: aggregate branch in `store_at`.
- `compiler/lowering/src/copy_tests.rs` (new, beside the other `*_tests.rs`):
  lowering tests in the `tests.rs` text style.
- `compiler/codegen/src/lowered/aggregate.rs` (new): `can_emit` and `emit`
  for `Copy`.
- `compiler/codegen/src/lowered/{mod,stmt}.rs`: new `StmtKind` arms
  delegating to `aggregate.rs`.
- `compiler/codegen/tests/it/compile_route_lowered_aggregate.rs` (new) and
  `compiler/codegen/tests/it/main.rs`: bridge tests.
- `specs/design/lowered-program.md`: the spec changes above.

## Tests

- **IR**
  - `copy_when_same_type_then_ok`;
  - `copy_when_arrays_of_identical_shape_then_ok` (two inline
    `ARRAY[1..2] OF DINT` types);
  - `copy_when_structures_of_identical_shape_then_ok`;
  - `copy_when_extents_differ_then_p9998`;
  - `copy_when_2d_and_1d_same_count_then_p9998`;
  - `copy_when_destination_scalar_then_p9998`;
  - `copy_when_field_names_differ_then_p9998`;
  - `print_when_copy_then_text`.
- **Lowering** (expected text form), one per shape:
  - a DINT array;
  - an `ARRAY OF STRING[8]`;
  - a 2-D array;
  - a structure with an array field;
  - `x := x`;
  - `g := a` and `a := g` through `VAR_EXTERNAL`;
  - an aggregate field of the block's own instance;
  - `arr[1] := one` (lowers to a `Copy`);
  - `r := f.p` (lowers to a `Copy`);
  - declined (`None`): a struct-returning function call source and a
    function block instance target.
- **Bridge** (`Route::RequireLowered`, with `assert_same_code` as in
  `compile_route_lowered_fb_field.rs`), the same instructions by both routes
  and the values after a run, for:
  - DINT and 2-D arrays;
  - a structure;
  - a global through `VAR_EXTERNAL`;
  - an aggregate field of a function block's own instance.
- **End to end:** unchanged. `compile_both_routes` already compares both
  routes for every program. `just lowered-count` should report about 14 more
  lowered statements. `lowered_floor` in `compiler/justfile` is not changed.

## Tasks

### Core PR: lower and emit `Copy`

- [ ] IR: `StmtKind::Copy`, `TypeTable::same_shape`, `Stmt::copy` /
      `ProgramBuilder::copy`, text form, `loops.rs` arm, IR tests.
- [ ] Lowering: aggregate branch in `store_at`, lowering tests.
- [ ] Bridge: `lowered/aggregate.rs`, `can_emit` / `emit` arms, bridge
      tests.
- [ ] Spec: §3.7 constructor note, §3.10 meaning rule.
- [ ] `cd compiler && just` and `cd specs && just` exit 0; record the new
      `lowered-count`.

## Out of scope

These are AST-route bugs for later; their `Copy` is lowered but not emitted:

- whole-element targets (`arr[1] := one`, P9999 today);
- a function block's structure output as a source (`r := f.p`, `p => q`,
  P9998 today);
- function block instance copies (the AST route aliases).

Function results that are structures are separate work.
