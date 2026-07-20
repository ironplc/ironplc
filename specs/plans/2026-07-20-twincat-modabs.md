# Plan: `MODABS` Stdlib Function

## Goal

`MODABS(x, y)` is an undeclared function call (`P4017`) — 6 files in the
latest re-scan, the single biggest remaining one-fix win. Same shape as
`LTRUNC`/`LMOD` (already landed): a Beckhoff `Tc2_Math` library
function, not core IEC 61131-3.

## Verification against real documentation

Checked Beckhoff's own `Tc2_Math` docs before implementing (same habit
as `LTRUNC`/`LMOD`):

- `MODABS` — `FUNCTION MODABS : LREAL`, parameters `lr_val : LREAL`,
  `lr_mod : LREAL`. Performs a modulo division and returns the
  **unsigned** modulo value within the modulo range — unlike `LMOD`,
  which can return a signed result. Beckhoff's own examples:
  `MODABS(400.56, 360) = 40.56`, `MODABS(-400.56, 360) = 319.44` (an
  `LMOD` call with the same arguments would return `-40.56` for the
  second case). Used in NC-axis contexts where modulo values are
  conventionally unsigned.

Same `LREAL`-only, `Tc2_Math`-specific shape as `LTRUNC`/`LMOD` — belongs
in the same `get_extended_math_functions()` table, gated by the same
`allow_extended_math_functions` flag already added for those two. Not a
new flag.

## Design

```rust
FunctionSignature::stdlib(
    "MODABS",
    TypeName::from("LREAL"),
    vec![input_param("IN1", "LREAL"), input_param("IN2", "LREAL")],
),
```

Added to the existing `get_extended_math_functions()` in
`stdlib_function.rs`, registered by the same existing conditional block
in `stages.rs` (`if options.allow_extended_math_functions { ... }`) —
no new registration code needed, just one more signature in the
already-iterated `Vec`.

## Non-goals

- Any other `Tc2_Math` function beyond `MODABS` — no other function from
  this family appears in the current re-scan.

## File Map

| File | Change |
|------|--------|
| `compiler/analyzer/src/intermediates/stdlib_function.rs` | Add `MODABS` to `get_extended_math_functions()` |
| `docs/explanation/enabling-dialects-and-features.rst` | Update the `--allow-extended-math-functions` description |

## Testing Strategy

- Unit test: `get_extended_math_functions()` now returns 3 functions,
  including `MODABS` with the correct signature.
- Semantic test: `MODABS(x, y)` resolves when the flag is on, still
  `P4017` when off (mirrors the `LTRUNC`/`LMOD` tests).
- End-to-end: verify via the CLI that `MODABS` resolves under
  `--dialect=codesys`.

## Tasks

- [x] Write plan (this document)
- [x] Add `MODABS` to `get_extended_math_functions()` + update its test
- [x] Semantic test
- [x] Docs update
- [x] Verify end-to-end via CLI
- [x] Run full CI pipeline (`cd compiler && just`)
- [ ] Push branch to fork
- [ ] Merge into `twincat-dev`, update `twincat-status.md`, push
