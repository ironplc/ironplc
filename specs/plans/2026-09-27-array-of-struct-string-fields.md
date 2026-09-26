# Plan: STRING fields of array-of-struct elements

Closes #1382. Part 5 of the split of #1792. Builds on the prefactors
(initialize_struct_variable, locate_array_of_struct), the WSTRING store fix,
and the explicit descriptor stride (ADR-0054).

## Goal

Compile and run programs that read and write a STRING field of an
array-of-struct element, such as `MyBay.Devices.MeterQRScanner[i].LastCode`
from the program reported in #1376. An out-of-range subscript traps.

## Architecture

- **Registration.** When a structure or a top-level `ARRAY OF <struct>` is
  declared, every direct STRING field of an array-of-struct element gets a
  strided STRING descriptor. This includes arrays reached through nested
  structure fields. The descriptor's element count is the number of
  structures, and its stride is the size of one structure. The entries are
  kept in declaration order, together with a scratch variable.
- **Access.** `a[i].s` resolves to the existing `StructStringElement`, built
  with that descriptor and the unscaled element dimensions. The VM applies
  the stride and bounds-checks the element index.
- **Initialization.** One `STR_INIT_ARRAY` per field, through the same
  descriptor, called from `initialize_struct_variable` and from the
  top-level array site.
- **Encoding.** `string_width` resolves the encoding of `a[i].s` using
  `locate_array_of_struct`.
- **Still rejected.** `a[i].names[j]` needs two strides and stays rejected,
  with a narrower message (#1791).

## Prefactoring

Done in the earlier parts of the split.

## Design doc reference

`specs/design/structure-codegen-memory-layout.md`; ADR-0054.

## File map

| File | Change |
| ---- | ------ |
| `compiler/codegen/src/compile_array_struct.rs` | Registration, STRING access, field-type lookup |
| `compiler/codegen/src/compile_struct.rs` | Register for structure variables |
| `compiler/codegen/src/compile_struct_init.rs` | `initialize_element_strings` |
| `compiler/codegen/src/compile_setup.rs` | Top-level array site initializes |
| `compiler/codegen/src/string_width.rs` | Encoding of `a[i].s` |
| `compiler/codegen/tests/it/end_to_end_array_of_struct_string.rs` | End-to-end tests |
| `specs/design/structure-codegen-memory-layout.md` | STRING fields of array elements; initialization |
| `specs/adrs/0054-...` | Confirmation cites the end-to-end tests |

## Tasks

- [ ] Registration, access, initialization, encoding
- [ ] End-to-end tests
- [ ] Design doc; ADR confirmation
- [ ] `git rm` this plan; `cd compiler && just` passes
