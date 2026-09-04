# Plan: Fix 32- and 64-bit partial-access slice writes

## Goal

`x.%Dn := v` and `x.%Ln := v` crash the compiler with an arithmetic overflow
panic, and a 64-bit slice cannot receive a 64-bit value at all. Make every
slice width writable, and cover the widths with one parametrized test table
instead of one hand-written test per case.

Found while fixing #1595: reads of every width work, and 8- and 16-bit
writes work, so the docs already describe the feature as supported.

## Architecture

All four write paths (named scalar, array element, struct field, struct-field
array element) share `emit_partial_access_read_modify_write` in
`compiler/codegen/src/compile_expr.rs`. It has two defects:

1. The slice mask is built with `1i32 << access_bits` (and `1i64 <<` for the
   64-bit case), which overflows when the slice is exactly as wide as the
   integer. Build every mask in `u128` and narrow to the operand width.
2. The right-hand side is always compiled at `DEFAULT_OP_TYPE` (32-bit
   signed). A 64-bit slice needs a 64-bit right-hand side: an `LWORD`
   variable loaded at 32 bits keeps only its low half, and an `LWORD` literal
   is rejected with P2026 by the 32-bit constant pool. Compile the right-hand
   side at the slice's own width, unsigned, since the slice is a bit string.

A 64-bit slice of a 64-bit base is the whole value, so the clear mask is zero
and the slice mask is all ones; the generic sequence handles it without a
special case.

## Prefactoring

Convert `compiler/codegen/tests/it/end_to_end_partial_access.rs` from one
`e2e_i32_with!` invocation per case to `rstest` tables driven by a shared
program template. The new width cases then become table rows rather than
more copies of the same program.

## Design doc reference

`specs/design/partial-access-bit-syntax.md` — REQ-PAB-131 (slice writes)
currently names only the byte and word forms; extend it to all four and point
the requirements table at the new test names.

## File map

- `compiler/codegen/src/compile_expr.rs` — mask construction and RHS op type
  in `emit_partial_access_read_modify_write`
- `compiler/codegen/tests/it/end_to_end_partial_access.rs` — rstest tables
- `specs/design/partial-access-bit-syntax.md` — REQ-PAB-131 and test table

## Tasks

- [ ] Prefactor: convert the partial-access e2e tests to rstest tables
- [ ] Fix mask construction and RHS width in
      `emit_partial_access_read_modify_write`
- [ ] Add table rows for `.%D` and `.%L` writes (variable and literal RHS,
      `DWORD` and `LWORD` bases) and a masked over-wide RHS
- [ ] Update REQ-PAB-131 and the requirements table
- [ ] Run `cd compiler && just`
- [ ] Delete this plan
