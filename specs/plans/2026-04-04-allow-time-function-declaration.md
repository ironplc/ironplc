# Allow TIME as a Function Declaration Name

## Goal

Allow `FUNCTION TIME : TIME ... END_FUNCTION` when `--allow-time-as-function-name` is enabled. Currently the flag only demotes `TIME` to an identifier when followed by `(` (function call context), but not when preceded by `FUNCTION` (declaration context). This prevents users from declaring a compatibility wrapper function named `TIME` for OSCAT.

## Architecture

Extend the existing `xform_demote_time_keyword` token transform to also demote `TIME` when it appears immediately after the `FUNCTION` keyword token. This is the minimal change — the same flag gates both call-site and declaration-site usage.

## File Map

| File | Change |
|------|--------|
| `compiler/parser/src/xform_demote_time_keyword.rs` | Add check: demote TIME when preceded by FUNCTION |
| `compiler/parser/src/tests.rs` | Add parser integration tests for `FUNCTION TIME` |
| `compiler/resources/test/time_function_decl.st` | Shared input file for plc2plc round-trip test |
| `compiler/plc2plc/resources/test/time_function_decl_rendered.st` | Expected rendered output |
| `compiler/plc2plc/src/tests.rs` | Add round-trip test |
| `compiler/codegen/tests/end_to_end_time_function.rs` | Add end-to-end execution test |
| `benchmarks/minimal_repros/35_declare_function_time.st` | Benchmark repro file |

## Tasks

- [x] Create plan
- [ ] Modify `xform_demote_time_keyword::apply` to demote TIME after FUNCTION
- [ ] Add unit tests in `xform_demote_time_keyword.rs`
- [ ] Add parser integration tests in `parser/src/tests.rs`
- [ ] Add benchmark repro file
- [ ] Add plc2plc round-trip test
- [ ] Add end-to-end execution test
- [ ] Run CI and verify all checks pass
