# Plan: Reject Explicit `^` Dereference on a `REFERENCE TO` Variable

## Goal

`REFERENCE TO` auto-dereferences (`x := r;` reads through the reference
with no `^` needed); `POINTER TO`/`REF_TO` need an explicit `^`
(`y := p^;`). IronPLC currently unifies all three spellings into the
same AST shape with no enforced distinction, so it accepts `r^` on a
`REFERENCE TO` variable today. Real TwinCAT rejects it.

## Verification against real code

Confirmed directly against a real TcXaeShell instance:

```
FUNCTION_BLOCK FB_RefTestExplicitDeref
VAR
    r : REFERENCE TO INT;
    src : INT := 42;
    z : INT;
END_VAR
r REF= src;
z := r^;
END_FUNCTION_BLOCK
```

produces `C0032: Cannot convert type 'Unknown type: 'r^'' to type
'INT'` and `C0064: Dereference requires a pointer`. A companion test
confirmed the opposite direction: `z := r;` (no `^`, auto-deref) compiles
and runs clean. `POINTER TO`/`REF_TO` were not re-verified in this pass
(already assumed to need `^`, per the existing `P2031` check and the
IEC 61131-3:2013 spec text read during the original `REFERENCE TO`/
`POINTER TO` work), only that `REFERENCE TO` specifically rejects it.

This directly contradicts the original design ("IronPLC does not
currently enforce a semantic distinction between the three at the
access site -- all three produce the same reference-type
representation"), documented at the time as a deliberate, low-risk
choice since no known real file needed the distinction. It's now
verified needed.

## Design

### AST: track which keyword spelling was used

```rust
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ReferenceKeyword {
    RefTo,
    Reference,
    Pointer,
}
```

New `pub keyword: ReferenceKeyword` field on `ReferenceInitializer`
(`dsl/src/common.rs`). Only this struct gets the new field -- the two
other `ref_to_keyword()` call sites (a `TYPE X : REF_TO Y;` alias
declaration, and an inline `REF_TO`-array-element type) already discard
the rule's return value entirely (`ref_to_keyword()` with no binding, or
`.is_some()` on the `Option`), so widening the rule's return type from
`()` to `ReferenceKeyword` doesn't require touching either of them.
Scoped to just the verified case: a bare `VAR ... : REFERENCE TO ...;`
declaration.

### Grammar: `ref_to_keyword()` returns which alternative matched

```rust
rule ref_to_keyword() -> ReferenceKeyword =
  tok(TokenType::RefTo) { ReferenceKeyword::RefTo }
  / tok(TokenType::Reference) _ tok(TokenType::To) { ReferenceKeyword::Reference }
  / tok(TokenType::Pointer) _ tok(TokenType::To) { ReferenceKeyword::Pointer }
```

`ref_to_var_init_decl()` captures it and threads it into the
`ReferenceInitializer` it constructs.

### Semantic: extend `rule_ref_to.rs`'s existing `check_deref`

`check_deref` already looks up whether the dereferenced variable is
*some* reference type (`P2031` if not). Add a second check: if it *is* a
reference type and its `ReferenceInitializer.keyword ==
ReferenceKeyword::Reference`, flag a new problem (explicit `^` not
allowed -- auto-dereferences). New code `P2037`
(`ExplicitDerefOnAutoDerefReference`), fitting the existing dedicated
`P2028`-`P2036` block `rule_ref_to.rs` already owns for reference-type
semantics (rather than the unrelated `P40xx` "declaration constraint"
block used for the other two checks in this batch).

## Non-goals

- `TYPE X : REFERENCE TO Y;` alias declarations and inline
  `REFERENCE TO`-array-element types -- the verified real gap is a
  direct `VAR` declaration; these two call sites already discard the
  keyword and are left untouched.
- Any change to `POINTER TO`/`REF_TO` behavior -- both continue to
  require explicit `^` exactly as before; only `REFERENCE TO` gains the
  new restriction.
- Reconciling this with plc2plc's existing renderer, which always
  normalizes `REFERENCE TO`/`POINTER TO`/`REF_TO` back to `REF_TO` on
  render regardless of original spelling (a deliberate prior choice).
  This is now a known, pre-existing tension: a `REFERENCE TO` variable
  with disallowed `^` would round-trip through plc2plc into a `REF_TO`
  variable with *allowed* `^`, silently changing which check applies.
  Not fixed here -- flagged as a follow-up if it matters in practice
  (no evidence yet that anything round-trips `REFERENCE TO` code through
  plc2plc's renderer today).

## File Map

| File | Change |
|------|--------|
| `compiler/problems/resources/problem-codes.csv` | New `P2037` |
| `docs/reference/compiler/problems/P2037.rst` | New problem doc |
| `compiler/dsl/src/common.rs` | New `ReferenceKeyword` enum; `ReferenceInitializer.keyword` field |
| `compiler/parser/src/parser.rs` | `ref_to_keyword()` returns `ReferenceKeyword`; `ref_to_var_init_decl()` threads it through |
| `compiler/analyzer/src/rule_ref_to.rs` | Extend `check_deref` |

## Testing Strategy

- Parser test: `ReferenceInitializer.keyword` is `Reference` for
  `REFERENCE TO`, `RefTo` for `REF_TO`, `Pointer` for `POINTER TO`.
- Semantic tests: explicit `^` on a `REFERENCE TO` variable produces
  `P2037`; explicit `^` on `POINTER TO`/`REF_TO` variables is still
  fine (regression); bare (no `^`) access on a `REFERENCE TO` variable
  is still fine (regression, matches the auto-deref real-world usage
  already covered by existing tests).
- End-to-end: verify via the CLI that the exact TcXaeShell repro now
  produces `P2037`.

## Tasks

- [x] Write plan (this document)
- [x] `ReferenceKeyword` enum + `ReferenceInitializer.keyword` field
- [x] Grammar: `ref_to_keyword()` return type + `ref_to_var_init_decl()`
- [x] New `P2037` problem code + doc
- [x] `rule_ref_to.rs`: extend `check_deref`
- [x] Tests from Testing Strategy
- [x] Run full CI pipeline (`cd compiler && just`)
- [x] Push branch to fork
- [ ] Merge into `twincat-dev`, update `twincat-status.md`, push

## Implementation Notes

- `cargo build --workspace --tests` after adding the `keyword` field
  surfaced exactly one construction site needing a fix:
  `xform_resolve_late_bound_type_initializer.rs`'s TYPE-alias-resolved
  reference path, defaulted to `ReferenceKeyword::RefTo` (preserves
  prior, unrestricted behavior for that untouched path, per the
  TYPE-alias non-goal above).
- The two other `ref_to_keyword()` call sites (array-element type,
  TYPE-alias rule) needed zero grammar changes, confirmed by direct
  reading before implementing.
- The pre-existing plc2plc round-trip test
  `write_to_string_when_reference_to_fb_type_then_round_trips` asserted
  full `Library` equality between the original and re-parsed-after-render
  library. The new `keyword` field broke that: render normalizes
  `REFERENCE TO` to `REF_TO` text (pre-existing, documented behavior),
  so the re-parsed library now has `keyword: RefTo` where the original
  had `keyword: Reference` -- exactly the plc2plc-normalization tension
  flagged as a known non-goal above, now surfaced as a real test failure
  rather than just a theoretical risk. Fixed by asserting idempotency
  (render → parse → render again yields identical output) instead of
  literal AST equality with the pre-render original, matching the
  `assert!(rendered.contains(...))` -only style already used by the two
  neighboring "normalizes to REF_TO" tests.
- Also anonymized two parser test doc-comments
  (`parse_when_reference_to_var_decl_then_same_shape_as_ref_to`,
  `parse_when_pointer_to_var_decl_then_same_shape_as_ref_to`) that
  referenced a private test corpus by a domain-specific type name;
  replaced with a generic comment, per the standing anonymization rule.
- End-to-end CLI verification: the exact TcXaeShell repro (`r := REF(src);
  result := r^;` on a `REFERENCE TO INT` variable, compiled with
  `--dialect iec61131-3-ed3`) now produces `P2037`; the fixed version
  (`result := r;`, no `^`) compiles clean.
- Full CI (`cd compiler && just`) passed: build, coverage ≥85%, clippy
  (pre-existing unrelated `large_enum_variant` warning only), fmt, dupes
  check.
