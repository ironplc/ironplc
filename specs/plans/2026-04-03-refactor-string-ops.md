# Plan: Refactor VM String Operations & Add Duplicate Code Check

## Goal

Extract duplicated buffer management code from 7 string opcodes in `vm.rs` into
a shared `string_ops.rs` module, and add `cargo-dupes` to CI to prevent future
code duplication.

## Architecture

`compiler/vm/src/vm.rs` (2613 lines) contains REPLACE_STR, INSERT_STR,
DELETE_STR, LEFT_STR, RIGHT_STR, MID_STR, and CONCAT_STR opcodes that each
duplicate: string header reading, temp buffer allocation, and header writing.

New `string_ops.rs` provides three helpers (`read_string_header`,
`allocate_temp_buffer`, `write_string_header`) plus a `TempBufferSlot` struct.
Each opcode becomes a compact block using these helpers. Existing helpers
(`str_read_max_len`, `str_read_cur_len`, `str_write_header`) also move to the
new module.

`cargo-dupes` (AST-based Rust duplicate detector) is added to the CI lint
pipeline to catch future duplication.

## File Map

| File | Change |
|------|--------|
| `compiler/justfile` | Add `dupes` recipe, update `setup` and `default` |
| `compiler/vm/src/string_ops.rs` | New: shared helpers + unit tests |
| `compiler/vm/src/lib.rs` | Add `mod string_ops` |
| `compiler/vm/src/vm.rs` | Refactor 7 opcodes, remove moved helpers |

## Tasks

- [ ] Write plan
- [ ] Add `cargo-dupes` to CI justfile
- [ ] Create `string_ops.rs` with helpers, struct, unit tests
- [ ] Register module in `lib.rs`, move existing helpers
- [ ] Refactor LEFT_STR, RIGHT_STR, MID_STR
- [ ] Refactor DELETE_STR, CONCAT_STR
- [ ] Refactor INSERT_STR, REPLACE_STR
- [ ] Run full CI pipeline (`cd compiler && just`)
