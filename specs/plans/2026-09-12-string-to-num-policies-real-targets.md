# STRING_TO_* behavior policies: the real targets

Tracking issue: [#1683](https://github.com/ironplc/ironplc/issues/1683) (PR 4 of
the series, the last). PRs 2 (#1686) and 3 (#1696) put every integer target on
the block; this PR adds `REAL` and `LREAL` at the positions PR 2 reserved. The
policies, presets and encoding are fixed; what this PR decides is the real
literal grammar and two edge cases the integer targets do not have.

## Goal

`STRING_TO_REAL` and `STRING_TO_LREAL` honor the two string-to-number
behavior policies. `STRING_TO_REAL` exists today and returns 0.0 on any
failure whatever the policy says; it changes to the policy result.
`STRING_TO_LREAL` is neither compiled nor registered today; afterwards it is.

## Decisions

Recorded in [behavior-policies.md](../design/behavior-policies.md) as
`REQ-BP-vm-008` through `REQ-BP-vm-010`; listed here so the review can see
them before the code.

1. **Grammar.** A real literal is an optional sign, a mantissa, and an
   optional exponent. The mantissa is a run of decimal digits, a run with a
   decimal point and an optional fraction run (`9.876`, `1.`), or a decimal
   point with a fraction run (`.5`); single `_` separators may appear between
   the digits of a run. The exponent is `E` or `e`, an optional sign, and a
   run of decimal digits (`1.2E-34`). No based literal, no typed prefix, and
   no word: `inf`, `infinity` and `nan` are not literals. The integer form is
   accepted because every surveyed implementation converts `'5'` to 5.0;
   the point-first and point-last forms because the C `strtod` the
   surveyed runtimes build on accepts them, and rejecting `'.5'` while
   `ignore-surrounding` skips the point to convert `5` would be a worse
   result than 0.5. CODESYS documents the input as a floating-point number
   "also in exponential notation".
2. **Overflow to infinity is a failure.** A literal whose magnitude rounds to
   infinity at the target's width (`'1e39'` for `REAL`, `'1e309'` for
   `LREAL`) does not fit the target, exactly as `'300'` does not fit `SINT`:
   `REQ-BP-vm-005` applies unchanged, so `reject` + `trap` halts with V4006
   and `zero` yields 0.0. Underflow is not a failure: a literal too small for
   the width rounds to a subnormal or to zero, which is what the value is at
   that width.
3. **Nothing produces NaN.** The words that would spell one are not
   literals, so under every non-numeric alternative they are a failure:
   V4006 under `trap`, positive 0.0 under `zero`. `ignore-surrounding` skips
   the letters of `'nan5'` and converts 5.
4. **The conversion's value is the literal parsed at the target's width**
   (`f32` for `REAL`, `f64` for `LREAL`), not parsed wide and narrowed, so
   the result is the correctly rounded value of the text.

## Architecture

- **Encoding.** `Target::F32 = 8` (`STRING_TO_REAL`) and `Target::F64 = 9`
  (`STRING_TO_LREAL`): twelve rows at 0x04C0–0x04C5 and 0x04C8–0x04CD, pinned
  in the wire-format test.
- **VM.** A separate scanner module, `str_to_real`, applies the non-numeric
  policy with the shared trim and skip helpers, measures the literal by the
  grammar above, and parses it with `core`'s `FromStr` for `f32` / `f64`
  (already used by the VM, `no_std`). Underscores are removed into a bounded
  stack buffer before parsing; a literal longer than the buffer is a
  failure, which no real number needs. `str_to_num::convert` dispatches on
  the target: the integer targets to `scan_integer`, the real targets to
  `scan_real`, each producing the slot of its width.
- **Codegen.** `F32` selects the block func_id instead of `CONV_STR_TO_F32`;
  `F64` selects the block func_id instead of the todo diagnostic. The VM
  keeps the `CONV_STR_TO_F32` handler as a wire-format commitment, as it
  keeps `CONV_STR_TO_I32`.
- **Analyzer.** `STRING_TO_LREAL` is registered next to `STRING_TO_REAL`.
- **Trap.** V4006 names `REAL` or `LREAL`.

## Prefactoring

`str_to_num::convert` computes the integer scan and then the slot in one
expression, and the trim/skip helpers are private to the module. The
prefactor commit makes the helpers `pub(crate)` and splits `convert` into
"scan for this target" and "apply the failure policy", so the real scanner
drops into the first half and the failure policy is written once. No
behavior change; the existing tests pass on that commit.

## Design doc reference

- [behavior-policies.md](../design/behavior-policies.md): the real grammar
  and the two edge decisions as requirements; positions 8 and 9 assigned.
- [bytecode-instruction-set.md](../design/bytecode-instruction-set.md): the
  twelve rows and the target table; `CONV_STR_TO_F32` no longer emitted.
- [ADR-0049](../adrs/0049-behavior-policies-selected-at-compile-time.md): the
  postscript closes the series.

## File map

| File | Change |
|---|---|
| `compiler/vm/src/str_to_num.rs` | Prefactor: shared helpers `pub(crate)`, scan/failure split; then the real arms |
| `compiler/vm/src/str_to_real.rs` | New: the real scanner and its unit tests |
| `compiler/vm/src/lib.rs` | Module declaration |
| `compiler/container/src/builtin.rs` | `F32`, `F64` variants, twelve rows, tests |
| `compiler/container/src/spec_conformance_behavior_policies.rs` | Pin the new rows |
| `compiler/vm/src/error.rs` | `REAL` / `LREAL` names |
| `compiler/vm/tests/it/execute_builtin_str_to_num.rs` | `REQ-BP-vm-008/009/010` conformance tests through the builtins |
| `compiler/codegen/src/compile_call.rs` | The two real arms |
| `compiler/codegen/src/spec_conformance_behavior_policies.rs` | The two functions; width-aware load opcode |
| `compiler/codegen/tests/it/wire_format.rs` | Pin the twelve func_ids |
| `compiler/codegen/tests/it/end_to_end_string_to_real.rs` | New: both functions under all six combinations |
| `compiler/codegen/tests/it/end_to_end_conv_string.rs` | The `STRING_TO_REAL` cases that assumed 0.0-on-failure move to the new file |
| `compiler/analyzer/src/intermediates/stdlib_function.rs` | Register `STRING_TO_LREAL` |
| `compiler/benchmarks/benches/st_benchmark.rs` | `REAL` and `LREAL` cases |
| `docs/reference/standard-library/functions/type-conversions.rst` | `STRING_TO_LREAL` row, the real grammar, a results table, the overflow and NaN rules |
| `docs/reference/runtime/problems/V4006.rst` | The real examples |
| `specs/design/behavior-policies.md`, `specs/design/bytecode-instruction-set.md`, ADR-0049 | As above |

## Tasks

- [ ] Prefactor: shared helpers and the scan/failure split (own commit)
- [ ] Design doc: the three requirements and positions 8 and 9
- [ ] Container: variants, rows, tests, conformance pins
- [ ] VM: `str_to_real`, dispatch, type names, conformance tests
- [ ] Codegen: the real arms, conformance functions, wire-format pins
- [ ] Analyzer: `STRING_TO_LREAL`
- [ ] End-to-end tests under all six combinations
- [ ] Benchmark: `REAL` and `LREAL` cases, run and record numbers in the PR
- [ ] Docs and specs
- [ ] `cd compiler && just`, `cd specs && just`, docs build green
- [ ] `git rm` this plan
