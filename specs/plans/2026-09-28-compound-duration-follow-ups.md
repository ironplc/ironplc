# Compound duration literal follow-ups

Issue: https://github.com/ironplc/ironplc/issues/1880

## Goal

Close the three gaps left after compound duration literals landed.

## Changes

1. **`_` inside a number after the first part.** `split()` in
   `compiler/parser/src/xform_split_duration_units.rs` makes every `_` its
   own token, so `T#1m1_000ms` fails. Classify an `_` with a digit on both
   sides as part of the digit run; an `_` next to a letter stays a
   separator.
2. **Fixed-point earlier parts.** `combine_interval_parts` in
   `compiler/parser/src/parser.rs` rejects a non-last part by value, so
   `T#1.0h30m` gets through. `interval_part()` records whether the number
   came from a `FixedPoint` token and `combine_interval_parts` rejects any
   such part that is not the last.
3. **End-to-end test.** Add `compiler/codegen/tests/it/end_to_end_compound_duration.rs`
   running `T#1m30s`, `T#1m1.5s` and `T#1m1_000ms` and checking the stored
   milliseconds.

## Prefactoring

None needed: both fixes are local to one function each.

## Tests

- Transform: `T#1m1_000ms` splits to `m`, `1_000`, `ms`; `T#1h_30m` still
  keeps `_` separate.
- Parser: `T#1m1_000ms` equals `T#1m1s`; `T#1.0h30m` is rejected.
- Codegen end-to-end as above.
