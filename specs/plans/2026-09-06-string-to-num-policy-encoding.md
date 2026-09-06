# Plan: behavior policy enums and the `STRING_TO_<numeric>` func_id block

Slice 2 of 5 of the `STRING_TO_UDINT` steel thread for
[ADR-0049](../adrs/0049-behavior-policies-selected-at-compile-time.md)
(fixes [#1592](https://github.com/ironplc/ironplc/issues/1592) when the
series lands). The slices, in order: the `CodegenOptions` / VM prefactor;
**this encoding slice**; the compiler options and front ends; the VM scanner
and trap; codegen selection and user documentation.

## Goal

Land the encoding half of the policy machinery with no behavior change: the
policy enums, the func_id block that carries a `STRING_TO_<numeric>`
conversion's target and policies, and the six pinned `CONV_STR_TO_U32_*`
rows. Nothing emits or dispatches these IDs yet.

## Architecture

- `ironplc_container::policy` defines `BehaviorPolicy` and the two
  `STRING_TO_<numeric>` policies. They live in the container crate because an
  alternative's discriminant is its func_id offset: an encoding commitment
  the parser, codegen and VM must share.
- `builtin::str_to_num` reserves `0x0480..=0x04FF` and gives the block
  arithmetic (`func_id`) and its inverse (`decode`); the six U32 rows are
  ordinary `declare_builtins!` rows so the disassembler names them and the
  wire-format tests pin them.

## Prefactoring

None needed: the change is purely additive to the builtin table and adds one
module. The `declare_builtins!` macro already gives a new row its name and
argument count.

## Design doc reference

`specs/design/behavior-policies.md` (new, the container-owned sections) and
the func_id range and block tables in
`specs/design/bytecode-instruction-set.md`.

## File map

| File | Change |
|---|---|
| `compiler/container/src/policy.rs` (new) | policy trait and enums |
| `compiler/container/src/builtin.rs` | six rows, `str_to_num` block |
| `compiler/container/src/lib.rs`, `build.rs` | module and design-doc registration |
| `compiler/container/src/spec_conformance_behavior_policies.rs` (new) | REQ-BP-container tests |
| `compiler/codegen/tests/it/wire_format.rs` | pins |

## Tasks

- [ ] Policy enums with unit tests
- [ ] Block, rows, decode with unit and conformance tests
- [ ] Wire-format pins; design docs
- [ ] `cd compiler && just`; delete this plan
