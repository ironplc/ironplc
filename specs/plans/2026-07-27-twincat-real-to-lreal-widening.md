# Plan: REAL → LREAL Implicit Widening

## Goal

`ElementaryTypeName::can_widen_to()` governs which implicit type
conversions IEC 61131-3 allows without a flag (integer→integer chains,
integer→REAL/LREAL, bit-string→bit-string). It has no case at all for
`(Real, Real)` -- meaning a **typed** `REAL` expression (a variable,
field, or return value, not a bare literal) can never be passed where
`LREAL` is expected, even though REAL→LREAL is always lossless (32-bit
float safely widens to 64-bit float) and IEC 61131-3 treats it the same
as any other same-family widening.

This blocks real function calls: several Beckhoff `Tc2_Math`/`Tc3`
library functions (e.g. `FLOOR`, being registered alongside this fix)
declare their parameter as `LREAL`, so passing a `REAL`-typed
expression to them fails today with a type-mismatch diagnostic, even
though the equivalent call with an untyped literal argument already
works (untyped real literals infer as `ANY_REAL`, which is separately
handled in `is_compatible_with_generic_param`/`generic_actual_satisfies`
per ADR-0028 -- this plan is specifically about **typed** REAL
expressions, a different code path).

## Verification

Confirmed directly by reading `ElementaryTypeName::can_widen_to()`
(`compiler/dsl/src/common.rs`): its `match` covers
`(SignedInteger|UnsignedInteger, SignedInteger|UnsignedInteger)`,
`(SignedInteger|UnsignedInteger, Real)`, and `(BitString, BitString)`
-- there is no `(Real, Real)` arm at all, falling through to the
catch-all `_ => false`. `REAL` and `LREAL` both belong to
`TypeFamily::Real` with bit-widths 32 and 64 respectively
(`type_properties()`), so extending the existing bit-width-comparison
pattern already used for the integer chains is a direct, minimal fit.

Confirmed via `cargo run -p ironplc-cli -- check` that a `FUNCTION
TAKES_LREAL : LREAL VAR_INPUT IN : LREAL; END_VAR ...` called with a
`REAL`-typed local variable argument fails type checking today, and
that the identical call with a bare literal argument (e.g. `3.14`)
succeeds -- confirming the gap is specific to typed expressions, not
literals.

## Design

Add one match arm to `can_widen_to()`, following the exact same
bit-width-comparison shape already used for the integer chains:

```rust
// REAL -> LREAL widening (lossless: 32-bit float safely widens to
// 64-bit float). LREAL -> REAL is narrowing and NOT allowed here.
(Real, Real) => tgt_bits > src_bits,
```

No new dialect flag: this is standard, lossless, IEC 61131-3-legal
widening in the same category as the integer chains already
unconditionally allowed -- not a vendor extension, so it doesn't belong
behind `allow_cross_family_widening` (that flag is specifically for
*cross-family*, non-standard widening like bit-string↔integer).

## Non-goals

- LREAL → REAL narrowing (an actual precision-losing conversion; would
  need an explicit conversion function, not implicit widening).
- Cross-family widening (REAL/LREAL ↔ integer or bit-string) -- already
  governed separately by `allow_cross_family_widening` and unaffected
  by this plan.

## File Map

| File | Change |
|------|--------|
| `compiler/dsl/src/common.rs` | Add `(Real, Real)` arm to `can_widen_to()` |

## Testing Strategy

- Unit test on `can_widen_to()` directly: `REAL.can_widen_to(&LREAL)`
  is `true`; `LREAL.can_widen_to(&REAL)` is `false` (regression guard
  against accidentally allowing narrowing).
- Analyzer test: a `FUNCTION`/`FUNCTION_BLOCK` parameter typed `LREAL`
  called with a typed `REAL` local variable argument now passes type
  checking (previously failed).
- Regression: the existing untyped-literal-to-LREAL path (ADR-0028) is
  unaffected -- add a test confirming a bare real literal argument
  still works, to guard against conflating the two code paths.

## Tasks

- [ ] Write plan (this document)
- [ ] Add `(Real, Real)` arm to `can_widen_to()`
- [ ] Tests from Testing Strategy
- [ ] Run full CI pipeline (`cd compiler && just`)
- [ ] Push branch to fork
