# Fix: Resolve Unqualified Enum Values in Expression Contexts

## Goal

Enable unqualified enum values (e.g., `RUNNING`) to be used in expression contexts
beyond direct assignment, such as comparisons (`State = RUNNING`), boolean
expressions (`(State = RUNNING) AND Seal`), and IF conditions.

## Architecture

The `xform_resolve_late_bound_expr_kind` transform resolves ambiguous `LateBound`
identifiers based on assignment target type. When the target is non-enum (e.g.,
BOOL), enum values are incorrectly resolved as variable references.

The fix pre-scans the library for all known enum value names, then checks this set
when resolving `LateBound` identifiers in non-enum contexts. Variables take priority
over enum values (checked via `names_to_types`).

## Design doc reference

- `specs/design/enumeration-codegen.md` (REQ-EN-031, REQ-EN-032, REQ-EN-033)

## File map

| File | Change |
|------|--------|
| `compiler/analyzer/src/xform_resolve_late_bound_expr_kind.rs` | Core fix + unit tests |
| `compiler/analyzer/src/rule_use_declared_symbolic_var.rs` | Integration test |
| `compiler/codegen/tests/end_to_end_enum.rs` | End-to-end test |
| `compiler/codegen/src/spec_conformance.rs` | Update comment + test |

## Tasks

- [x] Write implementation plan
- [ ] Pre-scan library for enum value names in `apply()`
- [ ] Add `enum_values` field and `resolve_late_bound` helper to `DeclarationResolver`
- [ ] Update `fold_expr_kind` LateBound match arms
- [ ] Add unit tests
- [ ] Add integration and end-to-end tests
- [ ] Update spec conformance comment and test
- [ ] Run `cd compiler && just` and verify CI passes
