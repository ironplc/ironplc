# Plan: the `STRING_TO_<numeric>` scanner and the V4006 trap in the VM

Slice 4 of 5 of the `STRING_TO_UDINT` steel thread for
[ADR-0049](../adrs/0049-behavior-policies-selected-at-compile-time.md)
(fixes [#1592](https://github.com/ironplc/ironplc/issues/1592) when the
series lands). The slices, in order: the `CodegenOptions` / VM prefactor;
the policy enums and func_id block; the compiler options and front ends;
**this VM slice**; codegen selection and user documentation.

## Goal

Dispatch the six `CONV_STR_TO_U32_*` builtins in the VM. Each decodes its
target and policies from the func_id, scans the IEC unsigned-integer literal
grammar under the non-numeric policy, and on failure traps
`V4006 StringNotConvertible` with the offending value or yields zero. No
compiled program reaches these IDs until the codegen slice.

## Architecture

- `vm/src/str_to_num.rs` owns the scanner and the failure policy, so
  `vm.rs` (already far over the module size limit) gains one dispatch arm
  over the block's range that decodes the ID and delegates. It holds no
  policy state (ADR-0049 confirmation 3).
- `Trap::StringNotConvertible` carries the target and a bounded
  `StringPreview` of the string, because the VM is `no_std` and a trap
  cannot own the value; the message names both.
- Out of range is a failure under every non-numeric alternative, which is
  the #1592 fix once codegen emits the block: nothing wraps.

## Prefactoring

Done in slice 1: `string_ops::narrow_str_bytes` is the shared
"resolve a narrow string operand" preamble the new arm reuses.

## Design doc reference

`specs/design/behavior-policies.md` (the VM-owned sections: the literal
grammar, scan semantics, failure).

## File map

| File | Change |
|---|---|
| `compiler/vm/src/str_to_num.rs` (new) | scanner, failure policy, unit tests |
| `compiler/vm/src/error.rs`, `resources/problem-codes.csv` | `StringNotConvertible`, `StringPreview`, V4006 |
| `compiler/vm/src/vm.rs`, `lib.rs`, `build.rs` | dispatch arm, module, design-doc registration |
| `compiler/vm/tests/it/execute_builtin_str_to_u32.rs` (new), `main.rs` | REQ-BP-vm tests through `execute()` |
| `docs/reference/runtime/problems/V4006.rst` (new) | problem page |
| `specs/design/bytecode-instruction-set.md` | V4006 in the trap table |

## Tasks

- [ ] Scanner with unit tests per alternative
- [ ] Trap, CSV row, problem page
- [ ] Dispatch arm; instruction-level conformance tests
- [ ] `cd compiler && just`; docs build; delete this plan
