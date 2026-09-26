# Plan: explicit element stride in array descriptors

## Goal

Add an explicit `element_stride` to the container's `ArrayDescriptor`
(format version 3 → 4), and have the VM's string-array opcodes use it. No
compiler behaviour changes: every descriptor codegen emits keeps its current,
natural stride.

Part 4 of the split of #1792 (#1382). A STRING field of each element of an
array of structures repeats one structure apart, which the 8-byte descriptor
cannot express. Part 5 has codegen emit strided descriptors for those
fields. The decision to use an explicit stride, rather than computing the
address in codegen (which loses the run-time subscript bounds trap), is
recorded as ADR-0054 in this PR.

## Architecture

- **Descriptor.** `ArrayDescriptor` grows from 8 to 12 bytes, adding
  `element_stride: u32`, which is always written.
  - `ArrayDescriptor::new` and `ContainerBuilder::add_array_descriptor`
    compute the natural stride.
  - `add_strided_array_descriptor` takes an explicit one.
  - The dedup key includes the stride.
- **Validation.** The verifier does not inspect descriptors, so
  `TypeSection::read_from` checks the stride at load time:
  - STRING/WSTRING: no smaller than one element.
  - Every other element type: exactly one slot, because
    `LOAD_ARRAY`/`STORE_ARRAY` step by one slot regardless of the
    descriptor.
- **VM.** `STR_INIT_ARRAY`, `STR_LOAD_ARRAY_ELEM` and `STR_STORE_ARRAY_ELEM`
  read the stride from the descriptor.
- **Golden files.** The golden `.iplc` files have no type section, so only
  their version field changes.

## Prefactoring

None needed. Every consumer already reads descriptors through
`ArrayDescriptor`, so the new field has one definition.

## Design doc reference

`specs/design/bytecode-container-format.md` (REQ-CF-container-003,
REQ-CF-container-019), and ADR-0054 (new).

## File map

| File | Change |
| ---- | ------ |
| `compiler/container/src/type_section.rs` | Field, constructor, wire format, validation, tests |
| `compiler/container/src/builder.rs` | `add_strided_array_descriptor`, dedup key |
| `compiler/container/src/error.rs` | `InvalidArrayStride` |
| `compiler/container/src/header.rs` | `FORMAT_VERSION` 4 |
| `compiler/container/src/spec_conformance.rs` | REQ-CF-003 / REQ-CF-019 |
| `compiler/vm/src/vm.rs` | String-array opcodes use the stride |
| `compiler/vm/tests/it/execute_string_ops.rs` | Strided-descriptor tests |
| `compiler/project/src/disassemble.rs`, `compiler/vm-cli/` | Version 4 |
| `specs/adrs/0054-explicit-element-stride-in-array-descriptors.md` | New |
| `specs/design/bytecode-container-format.md` | Version, descriptor layout, layout hash |

## Tasks

- [ ] Container and VM changes with tests
- [ ] ADR-0054 and the design doc
- [ ] `git rm` this plan; `cd compiler && just` and `cd specs && just` pass
