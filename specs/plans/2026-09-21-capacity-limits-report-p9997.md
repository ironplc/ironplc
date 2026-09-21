# Report the remaining fixed capacity limits as P9997

Implements [issue #1473](https://github.com/ironplc/ironplc/issues/1473).

## Problem

[#1415](https://github.com/ironplc/ironplc/pull/1415) introduced P9997
(`NotSupported`) for a capability the compiler deliberately does not offer, as
distinct from P9999 (`NotImplemented`), which promises "not yet". It applied
P9997 to the capacity checks in the new array-of-structure code, but the
identical pre-existing checks elsewhere still report P9999.

The result is a user-visible seam — the same limit, hit two ways, reports two
different codes:

```st
(* P9997 -- array of structures *)
readings : ARRAY[1..40000] OF Item;

(* P9999 -- array of a primitive, same limit, same reason *)
readings : ARRAY[1..40000] OF DINT;
```

## Prefactor

The seam exists because the *same* data region reservation is written out three
times — in `compile_array.rs`, `compile_struct.rs` and `compile_array_struct.rs`
— so a code change applied to one copy leaves the others behind. Before
changing any code, collapse the reservation into a single function:

```rust
// compiler/codegen/src/data_region.rs
#[track_caller]
pub(crate) fn reserve(
    ctx: &mut CompileContext,
    total_bytes: u32,
    span: &SourceSpan,
) -> Result<u32, Diagnostic>
```

It performs the two checks all three copies share — the `checked_add` on the
running offset, and the `i32::MAX` (2 GiB) ceiling — and returns the offset of
the start of the reserved run. `#[track_caller]` keeps the compiler
`file#Lline` that `Diagnostic::not_supported` records pointing at the calling
site rather than at the helper, so the P9xxx telemetry dashboards still rank by
the code that hit the limit.

The three copies do not agree today: `compile_array.rs` tests the *start* of the
run against `i32::MAX` while the other two test the *end*. The end is the
correct bound — the last byte of the run is what has to be addressable — so the
shared function takes it. With the 32768-slot cap in force a variable can
occupy at most 256 KiB, so no reachable program can tell the two apart.

## Changes

### 1. `compiler/codegen/src/data_region.rs` (new)

`reserve()` as above. Declared in `lib.rs`.

### 2. `compiler/codegen/src/compile_array.rs`

`compute_dimensions` and `register_array_variable` switch their *capacity*
checks to `Diagnostic::not_supported`:

- element count `checked_mul` overflow ("Array too large")
- the 32768-element cap
- the STRING element stride `checked_mul` overflow
- the data region `checked_add` overflow and the 2 GiB ceiling — both now
  reached through `data_region::reserve`

`"Unsupported array element type"` keeps P9999: an element type the compiler
does not handle *yet* is exactly what P9999 is for.

### 3. `compiler/codegen/src/compile_struct.rs`

`allocate_struct_variable` switches the same class of check:

- `SlotCountError::Overflow` ("Structure is too large")
- the 32768-slot cap
- the `slots * 8` byte `checked_mul` overflow
- the data region `checked_add` overflow and the 2 GiB ceiling — via
  `data_region::reserve`
- the `ARRAY OF STRING` field element count `checked_mul` overflow

These keep P9999:

- `"Unknown structure type"`
- `SlotCountError::UnsupportedFieldType` — STRING, WSTRING and function-block
  fields are unimplemented, not unsupported
- `SlotCountError::MaxDepthExceeded` — a defense-in-depth guard against a
  recursive type the analyzer should have rejected, so it reports a program the
  compiler does not handle rather than a capacity the format does not have

### 4. `compiler/codegen/src/compile_array_struct.rs`

Already P9997 throughout; it only loses its copy of the reservation to
`data_region::reserve`.

### 5. `docs/reference/compiler/problems/P9997.rst`

The worked example is an array of structures and stays correct, but it now
reads as though the limit were particular to structures. Add a sentence after
it noting that an array of a primitive type reaches the same limit the same
way, with the DINT declaration from the issue.

## Verification

New tests, each asserting the P9997 code rather than merely that compilation
failed:

| Test | Program | File |
|---|---|---|
| `compile_when_array_exceeds_element_limit_then_not_supported` | `ARRAY[1..40000] OF DINT` | `tests/it/compile_array.rs` |
| `compile_when_multidim_array_exceeds_element_limit_then_not_supported` | `ARRAY[1..200, 1..200] OF DINT` | `tests/it/compile_array.rs` |
| `compile_when_array_of_string_exceeds_element_limit_then_not_supported` | `ARRAY[1..40000] OF STRING[10]` | `tests/it/compile_array.rs` |
| `compile_when_array_element_type_unsupported_then_not_implemented` | `ARRAY[1..4] OF <unsupported>` | `tests/it/compile_array.rs` |
| `compile_when_struct_exceeds_slot_limit_then_not_supported` | struct holding `ARRAY[1..40000] OF DINT` | `tests/it/compile_struct.rs` |
| `compile_when_struct_field_type_unsupported_then_not_implemented` | struct holding a function block field | `tests/it/compile_struct.rs` |

The last test in each pair is the guard on the split: it fails if the sweep is
applied too widely and takes a genuine "not yet" diagnostic with it.

Then `cd compiler && just`.

## Release note

P9999 is public API. A program that declares an array or a structure over one
of the fixed capacity limits now reports P9997 instead of P9999. Nothing else
about the diagnostic changes — same message, same span. The repository keeps no
changelog file, so this is recorded in the pull request description.

## Left out

`compile_setup.rs` and `compile_fn.rs` reserve data region space for STRING
variables and function block instances with the same `checked_add` overflow
check, still reporting P9999. Issue #1473 does not list them, so they stay as
they are; they are worth a follow-up issue rather than a silent widening of
this change.
