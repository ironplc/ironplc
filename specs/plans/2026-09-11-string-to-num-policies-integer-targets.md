# STRING_TO_* behavior policies: the 8-, 16- and 32-bit integer targets

Tracking issue: [#1683](https://github.com/ironplc/ironplc/issues/1683) (PR 2 of
the series). PR 1 (#1656, #1657, #1669, #1677, #1678) built the mechanism for
`STRING_TO_UDINT`; this PR adds targets to it and redesigns nothing.

## Goal

`STRING_TO_SINT`, `STRING_TO_INT`, `STRING_TO_DINT`, `STRING_TO_USINT`,
`STRING_TO_UINT`, `STRING_TO_BYTE`, `STRING_TO_WORD` and `STRING_TO_DWORD`
honor the two string-to-number behavior policies exactly as `STRING_TO_UDINT`
does: the func_id names the target and both alternatives, the VM range-checks
inside the conversion, and nothing wraps. `STRING_TO_SINT('300')` fails; it
never becomes 44. The `TRUNC_*` codegen emitted after the conversion goes away.

## Architecture

Everything follows [behavior-policies.md](../design/behavior-policies.md):

- **Encoding.** Five new `Target`s in `builtin::str_to_num`, at the block
  positions this PR fixes for the whole series (bit-string types alias the
  unsigned target of the same width, so they cost no positions):

  | Position | Target | Functions | func_ids |
  |---|---|---|---|
  | 0 | U32 | `STRING_TO_UDINT`, `STRING_TO_DWORD` | 0x0480–0x0485 (landed) |
  | 1 | I32 | `STRING_TO_DINT` | 0x0488–0x048D |
  | 2 | U8 | `STRING_TO_USINT`, `STRING_TO_BYTE` | 0x0490–0x0495 |
  | 3 | I8 | `STRING_TO_SINT` | 0x0498–0x049D |
  | 4 | U16 | `STRING_TO_UINT`, `STRING_TO_WORD` | 0x04A0–0x04A5 |
  | 5 | I16 | `STRING_TO_INT` | 0x04A8–0x04AD |
  | 6, 7 | U64, I64 | PR 3 | 0x04B0–0x04BD (reserved) |
  | 8, 9 | F32, F64 | PR 4 | 0x04C0–0x04CD (reserved) |

  Even positions are unsigned and the odd position after each is its signed
  counterpart, in width order 32, 8, 16, 64 (32 first because U32 landed at 0).
  Thirty new `declare_builtins!` rows, pinned in the wire-format test.
- **VM.** One integer scanner, parameterised by the target's bounds. The
  scanner accumulates the magnitude in 64 bits (so PR 3 instantiates it too)
  and range-checks the signed value against the target's `min..=max` before
  producing the slot bits. `convert` pushes an `i32` bit pattern for every
  target in this PR.
- **Codegen.** The `StringToNum` arm maps every `W32` integer target to a
  `Target` by `(signedness, storage_bits)` and emits the block func_id; no
  `emit_truncation` follows. `CONV_STR_TO_I32` is no longer emitted (its VM
  handler stays, as a wire-format commitment; the spec says so).
- **Analyzer.** `STRING_TO_BYTE`, `STRING_TO_WORD` and `STRING_TO_DWORD` are
  registered next to the unsigned integer targets.
- **Trap.** `V4006` names the target type; the bit-string aliases report the
  unsigned integer type of the same width (documented).

## Prefactoring

The scanner in `vm/src/str_to_num.rs` is written for `u32`: `scan_u32`,
`digit_run` accumulating in `u32`, and a sign check that knows the target is
unsigned. Adding a second target means either a second scanner or
parameterising this one. The prefactor commit parameterises it with no
behavior change: `scan_integer(bytes, policy, bounds)` accumulates the
magnitude in `u64` and checks the signed value against `bounds`, and the
`U32` target instantiates it with `0..=u32::MAX`. The existing unit tests and
the `REQ-BP-vm-*` conformance tests pass unchanged on that commit.

## Design doc reference

- [behavior-policies.md](../design/behavior-policies.md): the block table
  and `REQ-BP-container-002`, `REQ-BP-codegen-001/002` generalise from "the
  U32 target" to "every integer target"; a target-position table is added.
- [bytecode-instruction-set.md](../design/bytecode-instruction-set.md): the
  thirty new rows and the target table.
- [ADR-0049](../adrs/0049-behavior-policies-selected-at-compile-time.md): a
  dated postscript under *What landed*.

## File map

| File | Change |
|---|---|
| `compiler/vm/src/str_to_num.rs` | Prefactor: bounds-parameterised scanner; then the five targets' bounds and slot bits |
| `compiler/container/src/builtin.rs` | Five `Target` variants, thirty rows, table test |
| `compiler/container/src/spec_conformance_behavior_policies.rs` | Pin every target's rows |
| `compiler/vm/src/error.rs` | Type names for the new targets |
| `compiler/vm/tests/it/execute_builtin_str_to_u32.rs` | Range tests per target at the instruction level |
| `compiler/codegen/src/compile_call.rs` | Map `(signedness, storage_bits)` to `Target`; drop `emit_truncation` |
| `compiler/codegen/src/spec_conformance_behavior_policies.rs` | Generalise to every target |
| `compiler/codegen/tests/it/wire_format.rs` | Pin the thirty func_ids |
| `compiler/codegen/tests/it/end_to_end_string_to_int.rs` | New: min, max, one past each bound, shared invalid inputs, under all six combinations, for each of the eight functions |
| `compiler/codegen/tests/it/end_to_end_conv_string.rs` | The `STRING_TO_<INTEGER>` cases that assumed 0-on-failure move to the new file |
| `compiler/analyzer/src/intermediates/stdlib_function.rs` | Register `STRING_TO_BYTE/WORD/DWORD` |
| `compiler/benchmarks/benches/st_benchmark.rs` | `st_string_to_num` group: one case per width (8, 16, 32 signed, 32 unsigned) |
| `docs/reference/standard-library/functions/type-conversions.rst` | Support column, per-type ranges, the alias note; drop "only UDINT" |
| `docs/reference/runtime/problems/V4006.rst` | Bit-string alias note |
| `specs/design/behavior-policies.md`, `specs/design/bytecode-instruction-set.md`, ADR-0049 | As above |

## Tasks

- [ ] Prefactor: bounds-parameterised integer scanner, U32 instantiates it (own commit)
- [ ] Container: `Target` variants, rows, `Target::ALL`, tests, conformance pins
- [ ] VM: bounds per target, slot bits, type names, instruction-level range tests
- [ ] Codegen: target selection, no truncation, conformance tests, wire-format pins
- [ ] Analyzer: bit-string `STRING_TO_*` registrations
- [ ] End-to-end tests for the eight functions under all six combinations
- [ ] Benchmarks: `st_string_to_num`, run and record numbers in the PR
- [ ] Docs: type-conversions page, V4006 page
- [ ] Specs: design docs, instruction-set table, ADR postscript
- [ ] `cd compiler && just`, `cd specs && just`, docs build green
- [ ] `git rm` this plan
