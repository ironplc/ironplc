# Explicit Element Stride in Array Descriptors

status: accepted
date: 2026-09-26

## Context and Problem Statement

An array descriptor in the container's type section tells the VM how many
elements an array has and what type they are. The three string-array opcodes
(`STR_INIT_ARRAY`, `STR_LOAD_ARRAY_ELEM`, `STR_STORE_ARRAY_ELEM`) also derive
from it the byte distance between consecutive elements:
`STRING_HEADER_BYTES + max_length × char_width`
([ADR-0015](0015-string-memory-layout.md),
[ADR-0035](0035-length-and-encoding-prefixed-string-layout.md)). That is
right when the strings are packed back to back, as in `ARRAY[1..6] OF
STRING[50]`.

It is wrong for a STRING field of each element of an array of structures,
such as `MeterQRScanner[i].LastCode` where `MeterQRScanner : ARRAY[1..6] OF
QRScanner`. Consecutive copies of `LastCode` are one `QRScanner` apart, a
distance unrelated to the string's own size. The 8-byte descriptor had no
field to say so, so codegen rejected the access with P9999
([#1382](https://github.com/ironplc/ironplc/issues/1382), from the program
reported in [#1376](https://github.com/ironplc/ironplc/issues/1376)).

The same limit blocked initialization. `STR_INIT_ARRAY` writes each
element's header at the derived stride, and a header left zeroed has
`char_width` 0, which traps on first use.

## Decision Drivers

- **A subscript out of range must trap.** Every other array access traps
  `ArrayIndexOutOfBounds`, which the VM checks against the descriptor's
  element count.
- **One rule in the VM.** The VM runs on embedded targets
  ([ADR-0010](0010-no-std-vm-for-embedded-targets.md)). Addressing should
  not branch on how an array came to be.
- **No bytecode growth proportional to array size.**

## Considered Options

1. **An explicit `element_stride` field in every array descriptor.**
2. **Fold the address into codegen**: compute
   `base + field_offset + i × struct_size` into a scratch variable at run
   time, and use a one-element descriptor with index 0.
3. **Encode the stride in the descriptor's reserved byte.**

## Decision Outcome

Chosen option: **an explicit `element_stride` field in every array
descriptor**, because it is the only option that keeps the subscript bounds
trap without growing bytecode with the array.

The descriptor grows from 8 to 12 bytes, and the container format version
goes from 3 to 4:

```
[element_type: u8] [reserved: u8] [total_elements: u32] [element_extra: u16] [element_stride: u32]
```

- The stride is always written, and is the byte distance between the starts
  of consecutive elements. Codegen writes the element's natural size except
  for a STRING field of an array-of-struct element, where it writes the size
  of one structure.
- The string-array opcodes take the stride from the descriptor. The bounds
  check is unchanged: the index is checked against `total_elements`, which
  for an array-of-struct STRING field is the number of structures.
- The reader validates the stride when it loads the type section, because
  the bytecode verifier does not inspect descriptors:
  - a STRING/WSTRING stride may not be smaller than one element, or elements
    would overlap;
  - every other element type must use exactly one slot, because
    `LOAD_ARRAY` and `STORE_ARRAY` step by one slot regardless of the
    descriptor.
- `u32`, because a data region can exceed 64 KiB.

### Consequences

- Good, because `MeterQRScanner[7].LastCode` traps with
  `ArrayIndexOutOfBounds` rather than writing into a neighbouring field.
- Good, because one `STR_INIT_ARRAY` initializes every element's copy of a
  field, however many elements there are.
- Good, because the wire is self-describing: a reader need not know how the
  compiler lays out structures to address an element.
- Bad, because every descriptor grows by 4 bytes, including the many whose
  stride could have been derived.
- Bad, because it is a format break. A version 3 container is rejected, as
  every earlier version bump was.
- Neutral: `LOAD_ARRAY`/`STORE_ARRAY` do not read the stride. The load-time
  check keeps a descriptor from claiming a stride they would ignore. Making
  them honour it is possible later without another format change.

### Confirmation

- `compiler/container/src/type_section.rs` tests round-trip a strided
  descriptor and reject an overlapping STRING stride and a non-slot
  primitive stride.
- `compiler/vm/tests/it/execute_string_ops.rs` tests initialize, load and
  store through a strided descriptor, and trap on an index past the end.
- `compiler/codegen/tests/it/end_to_end_array_of_struct_string.rs` runs
  STRING and WSTRING fields of array-of-struct elements end to end,
  including an out-of-range subscript.

## Pros and Cons of the Options

### An explicit `element_stride` field in every array descriptor

- Good, because the bounds trap and the addressing both stay in the VM.
- Good, because initialization is one instruction per field.
- Bad, because it changes the container format.

### Fold the address into codegen

- Good, because the container format does not change.
- Bad, because the run-time bounds trap is lost. The index is always 0, so an
  out-of-range subscript silently writes into a neighbouring field. The VM's
  data-region check still prevents writes outside the region, so memory
  safety holds, but program correctness does not.
- Bad, because initialization must be unrolled per element: one `STR_INIT`
  per element per field.

### Encode the stride in the descriptor's reserved byte

- Good, because the descriptor size does not change.
- Bad, because a byte cannot hold a structure size, which can exceed 255
  bytes. It could hold a multiplier or a slot count only up to a limit that
  real structures pass.

## More Information

- [#1382](https://github.com/ironplc/ironplc/issues/1382): the gap this
  closes.
- [#1791](https://github.com/ironplc/ironplc/issues/1791): a STRING nested
  deeper inside an element (`a[i].names[j]`) needs two strides and remains
  unsupported.
- `specs/design/bytecode-container-format.md`: REQ-CF-container-003 and
  REQ-CF-container-019.
