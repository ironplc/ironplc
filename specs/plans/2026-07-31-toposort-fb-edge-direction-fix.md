# Plan: Fix toposort dependency-graph edge direction for eager FunctionBlock initializers

## Context

Split out of PR #1222, which bundled this pre-existing correctness fix with
a new feature (the CODESYS/TwinCAT call-style FB instance initializer,
`name : FB_Type(args);`). The maintainer asked to separate the two. This PR
is only the toposort fix; the feature that first makes the bug reachable
through user-facing syntax rides along in a separate PR.

## The bug

`compiler/analyzer/src/xform_toposort_declarations.rs` builds a directed
dependency graph and topologically sorts declarations so that a referenced
type is always emitted before the POU that references it
(referenced-type-before-referencer). Toposort orders an edge's source
before its target, so the established convention across the arms is
`add_edge(referenced, referencer)`:

- `visit_late_bound_declaration`: `add_edge(base_type, alias)`
- `visit_function` (call): `add_edge(callee, caller)`
- `visit_initial_value_assignment_kind`, `Structure` arm:
  `add_edge(referenced_struct, container)`
- `visit_initial_value_assignment_kind`, `LateResolvedType` arm:
  `add_edge(referenced, referencer)`

Two arms for `InitialValueAssignmentKind::FunctionBlock` had the edge
reversed (`add_edge(referencer, referenced)`):

1. `visit_function_block_initial_value_assignment` (the dedicated visitor)
2. the `InitialValueAssignmentKind::FunctionBlock(fb)` arm of
   `visit_initial_value_assignment_kind`

Both fire for the same eager FunctionBlock initializer. The reversed edge
can order a referenced FB type *after* its referencer, which surfaces
downstream as a spurious `P2011` "Parent type is not declared".

## Reachability note

At parse time, no current user-facing syntax produces an eager
`InitialValueAssignmentKind::FunctionBlock` in this codebase:

- A bare `inst : FB_Type;` inside a POU parses to `LateResolvedType` (the
  correct arm).
- A top-level `VAR_GLOBAL inst : FB_Type;` parses to `Simple`
  (`located_var_spec_init()` matches first, so `global_var_decl()`'s
  FunctionBlock alternative is dead code).
- A `TYPE FB_Alias : FB_Type;` alias parses to `LateBound`.
- `fb_name_decl()`'s FunctionBlock construction is unreachable (its
  `commasep_oneplus()` combinator requires a spurious trailing comma).

The eager form is produced at parse time only by the call-style grammar
(separate PR), and by the late-bound resolver — which runs *after* toposort
in `resolve_types`, so it doesn't feed toposort. The fix is still correct
and worth landing first: it makes the transform obey its own convention for
every eager FunctionBlock initializer, so the call-style feature is safe the
moment it lands.

## Change

Flip both edges to `add_edge(to, from)` (referenced → referencer) and update
the two comments to state the convention.

## Test

Because no parser path produces the eager form without the call-style
grammar, the regression test constructs the eager
`InitialValueAssignmentKind::FunctionBlock` initializer directly on a parsed
AST (a `Caller` FB forward-referencing a later-declared `Callee` FB) and
asserts `apply` orders `Callee` before `Caller`. Verified fail-before /
pass-after: the test fails with the original reversed edges and passes with
the flip. It deliberately avoids the `FB_Type(args)` call-style grammar,
which is not part of this PR.

## Non-goals

No DSL, parser-grammar, or renderer changes — those all belong to the
call-style feature PR.
