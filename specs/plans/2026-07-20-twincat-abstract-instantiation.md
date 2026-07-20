# Plan: Reject Direct Instantiation of an `ABSTRACT` Function Block

## Goal

`FUNCTION_BLOCK ABSTRACT FB_Base` currently parses and is recognized,
but nothing stops declaring a variable of that abstract type directly
(`inst : FB_Base;`). Real TwinCAT rejects this.

## Verification against real code

Confirmed directly against a real TcXaeShell instance:

```
FUNCTION_BLOCK ABSTRACT FB_Base
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_User
VAR
    inst : FB_Base;
END_VAR
END_FUNCTION_BLOCK
```

produces `C0434: Function block FB_Base is ABSTRACT and cannot be
instantiated`. This was already known to be unenforced (documented as a
deliberate non-goal when `ABSTRACT` parsing landed), but not previously
confirmed as a *real* compile error in TwinCAT rather than just an
unenforced convention.

## Design

New semantic rule, `rule_abstract_not_instantiated.rs`. Deliberately
does **not** thread `is_abstract` through `IntermediateType::FunctionBlock`
(which would ripple through ~10+ construction sites in
`type_environment.rs`/`intermediate_type.rs`/`xform_resolve_type_decl_environment.rs`).
Instead, works directly off the AST, matching the scope of what's
actually needed:

1. Build a `HashSet<TypeName>` of every `FunctionBlockDeclaration` with
   `is_abstract == true`, directly from `lib.elements` -- no
   `TypeEnvironment` involvement.
2. Walk every `VarDecl` in the library; when its initializer is
   `InitialValueAssignmentKind::FunctionBlock(fb_init)` and
   `fb_init.type_name` is in the abstract set, flag it.

By the time semantic rules run (after `resolve_types`'s xforms,
including `xform_resolve_late_bound_type_initializer`), a bare
`inst : FB_Base;` declaration has already been resolved from
`LateResolvedType` into the concrete `FunctionBlock` variant, so this
check doesn't need to duplicate that resolution logic.

New problem code `P4045` (`AbstractFunctionBlockInstantiated`).

## Non-goals

- Threading `is_abstract` into `IntermediateType`/`TypeEnvironment` --
  not needed for this specific check, and would be a much larger,
  unrelated-to-the-goal change (every construction site of
  `IntermediateType::FunctionBlock` would need updating).
- Allowing an abstract function block to be extended but still checking
  *that* usage is fine -- `EXTENDS FB_AbstractBase` is unaffected by this
  rule, since extending isn't instantiating; only a direct `VAR` of the
  abstract type is flagged.
- Detecting indirect instantiation (e.g. an abstract type used as an
  array element type, or inside a STRUCT) -- not confirmed as a real
  gap; the verified case is a direct `VAR` declaration.

## File Map

| File | Change |
|------|--------|
| `compiler/problems/resources/problem-codes.csv` | New `P4045` |
| `docs/reference/compiler/problems/P4045.rst` | New problem doc |
| `compiler/analyzer/src/rule_abstract_not_instantiated.rs` | New semantic rule |
| `compiler/analyzer/src/stages.rs` | Register the new rule |

## Testing Strategy

- Semantic tests: declaring a variable of an `ABSTRACT` FB type produces
  `P4045`; declaring a variable of a non-abstract FB type is unaffected;
  a *derived* (non-abstract) FB that `EXTENDS` an abstract base can
  itself still be instantiated (only the abstract type itself is
  flagged, not its concrete subclasses).
- Regression: `EXTENDS`-ing an abstract base (not instantiating it) is
  unaffected.

## Tasks

- [x] Write plan (this document)
- [x] New `P4045` problem code + doc
- [x] New semantic rule + registration
- [x] Tests from Testing Strategy
- [x] Run full CI pipeline (`cd compiler && just`)
- [ ] Push branch to fork
- [ ] Merge into `twincat-dev`, update `twincat-status.md`, push

## Implementation Notes

- Verified end-to-end via the CLI: the exact TcXaeShell repro now
  produces `P4045` under `--dialect=codesys` (alongside the pre-existing
  `P9004` for the `ABSTRACT` clause itself -- both fire independently,
  which is correct: one flags the vendor extension as recognized-but-
  unsupported in general, the other specifically flags the instantiation
  attempt).
- Confirmed a concrete subclass of an abstract base can still be
  instantiated normally -- the check only looks at the *declared*
  type's own `is_abstract`, not anything about its ancestry.
