# STRING_TO_* behavior policies: the 64-bit integer targets

Tracking issue: [#1683](https://github.com/ironplc/ironplc/issues/1683) (PR 3 of
the series). PR 1 built the mechanism for `STRING_TO_UDINT`; PR 2 (#1686) added
the 8-, 16- and 32-bit integer targets and fixed the block positions for the
rest of the series. This PR adds the 64-bit targets at those positions and
redesigns nothing.

## Goal

`STRING_TO_LINT`, `STRING_TO_ULINT` and `STRING_TO_LWORD` compile and honor
the two string-to-number behavior policies exactly as the narrower integer
targets do. The codegen arm returns a "todo" diagnostic for 64-bit widths
today; afterwards it emits the block func_id and the VM pushes a 64-bit slot.

## Architecture

Everything follows [behavior-policies.md](../design/behavior-policies.md),
whose *Adding a target* checklist this PR walks:

- **Encoding.** `Target::U64 = 6` (`STRING_TO_ULINT`, `STRING_TO_LWORD`) and
  `Target::I64 = 7` (`STRING_TO_LINT`), the positions PR 2 reserved. Twelve
  `declare_builtins!` rows at 0x04B0–0x04B5 and 0x04B8–0x04BD, pinned in the
  wire-format test.
- **VM.** The scanner already accumulates in 64 bits and its magnitude-based
  bounds hold `u64::MAX` and `2^63` below zero, so the two targets are two
  `Bounds::of` arms. `str_to_num::convert` returns a `Slot` of the target's
  width instead of an `i32` (prefactor), and the dispatch pushes it as-is.
- **Codegen.** The `(OpWidth::W64, _, _)` arm splits into the two 64-bit
  integer shapes selecting the block func_id; `F64` keeps the todo.
- **Analyzer.** `STRING_TO_LINT`, `STRING_TO_ULINT` and `STRING_TO_LWORD` are
  registered next to the other integer targets.
- **Trap.** V4006 names `LINT` or `ULINT`; `LWORD` reports `ULINT` as the
  narrower bit-string types report their unsigned integer.

## Prefactoring

`str_to_num::convert` returns `Result<i32, Trap>` and the VM wraps it in
`Slot::from_i32`, which fixes every target to a 32-bit slot. The prefactor
commit moves the slot construction into `convert`, returning `Result<Slot,
Trap>`, with every existing target still producing a 32-bit slot: no behavior
change, and the 64-bit targets then differ only in which `Slot` constructor
their arm picks. The codegen conformance test for `REQ-BP-codegen-002`
asserts the instruction after the builtin is `LOAD_VAR_I32`; it takes the
load opcode of the target's width so a 64-bit target fits the same test.

## Design doc reference

- [behavior-policies.md](../design/behavior-policies.md): positions 6 and 7
  move from reserved to assigned; the preview note mentions 64-bit literals.
- [bytecode-instruction-set.md](../design/bytecode-instruction-set.md): the
  twelve new rows and the target table.
- [ADR-0049](../adrs/0049-behavior-policies-selected-at-compile-time.md): the
  postscript's "remaining items" line.

## File map

| File | Change |
|---|---|
| `compiler/vm/src/str_to_num.rs` | Prefactor: `convert` returns a `Slot`; then the two 64-bit bounds and slot arms |
| `compiler/vm/src/vm.rs` | Push the slot `convert` returns |
| `compiler/container/src/builtin.rs` | `U64`, `I64` variants, twelve rows, tests |
| `compiler/container/src/spec_conformance_behavior_policies.rs` | Pin the new rows |
| `compiler/vm/src/error.rs` | `LINT` / `ULINT` names |
| `compiler/vm/tests/it/execute_builtin_str_to_num.rs` | 64-bit instruction-level tests (a 64-bit store and read) |
| `compiler/codegen/src/compile_call.rs` | The 64-bit integer arms |
| `compiler/codegen/src/spec_conformance_behavior_policies.rs` | The three functions; width-aware load opcode |
| `compiler/codegen/tests/it/wire_format.rs` | Pin the twelve func_ids |
| `compiler/codegen/tests/it/end_to_end_string_to_int.rs` | The three functions at min, max, one past each bound, shared invalid inputs, all six combinations |
| `compiler/analyzer/src/intermediates/stdlib_function.rs` | Register the three functions |
| `compiler/benchmarks/benches/st_benchmark.rs` | `LINT` case in `st_string_to_num` |
| `docs/reference/standard-library/functions/type-conversions.rst` | Three table rows, three range rows |
| `docs/reference/runtime/problems/V4006.rst` | `LWORD` alias mention |
| `specs/design/behavior-policies.md`, `specs/design/bytecode-instruction-set.md`, ADR-0049 | As above |

## Tasks

- [ ] Prefactor: `convert` returns a `Slot`; conformance test takes the width's load opcode (own commit)
- [ ] Container: variants, rows, tests, conformance pins
- [ ] VM: bounds, slot arms, type names, instruction-level tests
- [ ] Codegen: the 64-bit arms, conformance functions, wire-format pins
- [ ] Analyzer: registrations
- [ ] End-to-end tests under all six combinations
- [ ] Benchmark: `LINT` case, run and record numbers in the PR
- [ ] Docs and specs
- [ ] `cd compiler && just`, `cd specs && just`, docs build green
- [ ] `git rm` this plan
