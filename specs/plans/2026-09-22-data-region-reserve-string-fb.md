# Route STRING and function-block reservations through `data_region::reserve`

Issue: #1762

## Goal

Every data region allocation reports the same problem code (P9997) and enforces
the same 2 GiB ceiling, whatever kind of variable reaches the limit.

## Prefactor

None needed: `data_region::reserve` already exists (#1473) and is the
simplification. This change only removes the remaining hand-rolled copies.

## Steps

1. Replace the `checked_add` blocks in `compile_setup.rs` (STRING, standard
   library FB instance, user FB instance) with `data_region::reserve`.
2. Replace the four `checked_add` blocks in `compile_fn.rs` (STRING parameters,
   locals, return value, function-block body STRINGs) likewise.
3. Add unit tests in `data_region.rs` that drive `reserve` directly: success
   advances the offset, `u32` overflow reports P9997, and exceeding
   `i32::MAX` reports P9997. `CompileContext::new` becomes `pub(crate)` so the
   tests can build a context.
