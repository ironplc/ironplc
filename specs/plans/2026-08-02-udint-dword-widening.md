# Plan: Allow equal-width `UDINT`↔`DWORD` implicit conversion

## Problem

`ironplcc check` rejects a bare (uncast) assignment or function-call
argument between `UDINT` and `DWORD` with `P4026`/`P4035` (type
mismatch), per ADR-0031's deliberately asymmetric cross-family
widening design: `can_widen_cross_family_to` only allows bit-string →
integer where the target is *strictly wider*, never equal width, and
never the reverse direction.

Beckhoff's own `tc3_plc_intro` documentation states explicitly that no
implicit conversion exists between bit-string and integer types, even
at equal width -- which is what motivated the current design. But a
direct verification against a real TcXaeShell build (2026-07-27,
recorded in `specs/plans/twincat-status.md` under "Resolved: UDINT →
DWORD implicit conversion") found the opposite: a test POU with four
cases -- (1) bare `dwFromUdint := udValue` (`UDINT` → `DWORD`, no
cast), (2) bare `udFromDword := dwValue` (`DWORD` → `UDINT`, no cast),
(3) the real motivating repro (`DWORD_TO_HEXSTR(ErrorID, 4, FALSE)`
with `ErrorID : UDINT`), and (4) the explicit-cast equivalents for
comparison -- **all four compiled with zero errors or warnings**,
including the three IronPLC currently rejects. So the real compiler is
more permissive than its own docs, specifically for this one pair at
equal width.

## Scope

Deliberately narrow, matching the verification: only `UDINT` ↔
`DWORD` (32-bit unsigned integer ↔ 32-bit bit-string), bidirectional.
**Not** generalized to other same-width bit-string/unsigned-integer
pairs (`BYTE`↔`USINT`, `WORD`↔`UINT`, `LWORD`↔`ULINT`) or to signed
integers (`DWORD`↔`DINT`) -- none of those were separately verified
against real hardware, and the twincat-status.md write-up explicitly
warns against assuming they behave the same.

Still gated behind `--allow-cross-family-widening` (not made
unconditional): this remains a non-standard, vendor-specific
permissiveness, consistent with how that flag already gates every
other cross-family (bit-string ↔ integer) case.

## Design

`can_widen_cross_family_to` in `compiler/dsl/src/common.rs` currently:

```rust
pub fn can_widen_cross_family_to(&self, target: &ElementaryTypeName) -> bool {
    use TypeFamily::*;
    let Some((src_family, src_bits)) = self.type_properties() else { return false; };
    let Some((tgt_family, tgt_bits)) = target.type_properties() else { return false; };
    match (&src_family, &tgt_family) {
        (BitString, SignedInteger | UnsignedInteger) => tgt_bits > src_bits,
        _ => false,
    }
}
```

Add one more arm, matching the exact verified pair by name (not by a
general family/bit-width rule, since only this one pair is confirmed):

```rust
        (BitString, SignedInteger | UnsignedInteger) => tgt_bits > src_bits,
        // UDINT <-> DWORD (32-bit), equal width, both directions: verified
        // permissive against real TcXaeShell despite Beckhoff's own docs
        // stating otherwise (see twincat-status.md, 2026-07-27). Scoped to
        // exactly this pair -- other same-width bit-string/integer pairs
        // are not verified and must not be assumed to behave the same.
        (BitString, UnsignedInteger) | (UnsignedInteger, BitString)
            if matches!(self, ElementaryTypeName::DWORD | ElementaryTypeName::UDINT)
                && matches!(target, ElementaryTypeName::DWORD | ElementaryTypeName::UDINT) =>
        {
            true
        }
        _ => false,
```

The `matches!` guards check `self`/`target` directly (not
family/bits), which is what makes this exact-pair-only rather than a
generalized equal-width rule -- `self`/`target` being one of
`{DWORD, UDINT}` each, combined with the outer match already requiring
one `BitString` and one `UnsignedInteger`, pins this to exactly the
`UDINT↔DWORD` pair.

## Files

- `compiler/dsl/src/common.rs` -- `can_widen_cross_family_to` and its
  unit tests.

## Tests

New tests alongside the existing `can_widen_to`/`can_widen_cross_family_to`
unit tests in `compiler/dsl/src/common.rs`:

- `can_widen_cross_family_to_when_udint_to_dword_then_true`
- `can_widen_cross_family_to_when_dword_to_udint_then_true`
- `can_widen_cross_family_to_when_dint_to_dword_then_false` (signed
  integer, not verified -- must still be rejected)
- `can_widen_cross_family_to_when_word_to_uint_then_false` (different
  pair at a different width -- must still be rejected)

Plus one end-to-end test in `rule_function_call_type_check.rs` (or
wherever the existing cross-family-widening assignment tests live)
covering `dwFromUdint := udValue;` and `udFromDword := dwValue;` with
`--allow-cross-family-widening`, and confirming it's still rejected
without the flag.

## Out of scope

- Any other same-width bit-string/unsigned-integer pair.
- Signed integer ↔ bit-string at any width.
- Revisiting ADR-0031's general asymmetric design beyond this one
  verified exception.
