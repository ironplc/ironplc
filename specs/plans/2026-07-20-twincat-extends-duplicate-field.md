# Plan: Reject a Derived Function Block Redeclaring a Base-Class Field

## Goal

`FUNCTION_BLOCK FB_Derived EXTENDS FB_Base` currently allows `FB_Derived`
to redeclare a field that `FB_Base` already declares (with any type),
with no diagnostic. Real TwinCAT rejects this outright as a duplicate
definition.

## Verification against real code

Confirmed directly against a real TcXaeShell instance:

```
FUNCTION_BLOCK FB_Base
VAR
    state : INT;
END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Derived EXTENDS FB_Base
VAR
    state : BOOL;
END_VAR
END_FUNCTION_BLOCK
```

produces `C0097: Duplicate definition of variable 'state' in function
block 'FB_Derived' and in base 'FB_ShadowBase'`. TwinCAT does not allow
this even when the redeclared field has a different type. IronPLC's
field-inheritance implementation (`collect_inherited_fields()`,
landed earlier) designed "derived field shadows base field" as the
default without a real file confirming the edge case — this test shows
that assumption was wrong: it isn't silently allowed shadowing, it's a
hard error.

## Design

New semantic rule, `rule_extends_field_duplicated.rs`, reusing the
existing `collect_inherited_fields()` helper (no changes needed there):
for every `FunctionBlockDeclaration` with `extends`, check whether any
of its own `variables` names collides (case-insensitively, via `Id`'s
existing `PartialEq`) with any inherited field name. If so, flag a new
problem.

New problem code `P4039` (`ExtendsFieldNameDuplicated`), following the
`P4011`/`P4014`/`P4019`-style "duplicated name" convention already used
for other declaration-collision checks.

## Non-goals

- Any change to `collect_inherited_fields()` itself — it already
  correctly returns the ancestor chain's fields; this is purely a new
  consumer of it.
- Diamond/multiple-inheritance shadowing rules — `FUNCTION_BLOCK EXTENDS`
  only supports a single base type (confirmed during the original
  field-inheritance work), so there's no multi-parent ambiguity to
  resolve.

## File Map

| File | Change |
|------|--------|
| `compiler/problems/resources/problem-codes.csv` | New `P4039` |
| `docs/reference/compiler/problems/P4039.rst` | New problem doc |
| `compiler/analyzer/src/rule_extends_field_duplicated.rs` | New semantic rule |
| `compiler/analyzer/src/stages.rs` | Register the new rule |

## Testing Strategy

- Semantic tests: a derived FB redeclaring a base field (same type, and
  a different type) both produce `P4039`; a derived FB with no field
  collisions is unaffected; a multi-level chain (`FB_C EXTENDS FB_B
  EXTENDS FB_A`) catches a collision against a grandparent's field, not
  just the immediate parent's.
- Regression: a plain (non-`EXTENDS`) function block redeclaring
  nothing is unaffected (already implicitly covered by `rule_decl_struct_element_unique_names`-style
  within-one-FB duplicate checks, which are unrelated to this new
  cross-FB check).

## Tasks

- [x] Write plan (this document)
- [ ] New `P4039` problem code + doc
- [ ] New semantic rule + registration
- [ ] Tests from Testing Strategy
- [ ] Run full CI pipeline (`cd compiler && just`)
- [ ] Push branch to fork
- [ ] Merge into `twincat-dev`, update `twincat-status.md`, push
