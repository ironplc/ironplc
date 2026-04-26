# Share Debug-Format Helpers Across LSP Runner, VM CLI, and Playground

## Goal

Remove the duplicate `build_var_debug_map` / `format_variable_value` helpers
flagged by `cargo dupes` by moving them — together with the small
`VarDebugInfo` struct — into a shared module in the `ironplc-container` crate.

## Context

`cargo dupes` flagged two near-identical helpers used to render VM variable
state:

- `build_var_debug_map` is identical in
  `compiler/ironplc-cli/src/lsp_runner.rs` and
  `compiler/playground/src/lib.rs`.
- `format_variable_value` is near-identical in
  `compiler/ironplc-cli/src/lsp_runner.rs` and
  `compiler/vm-cli/src/cli.rs` (the only drift is that `vm-cli` adds the
  spec-mandated `TIME` / `LTIME` cases; `lsp_runner` falls through them).

All three call sites already depend on `ironplc-container`, and `Container`
itself lives there, so the natural home for these helpers is the container
crate.

## Architecture

Create `compiler/container/src/debug_format.rs` (an `std`-only module,
matching `debug_section`) exposing:

- `pub struct VarDebugInfo { pub name: String, pub type_name: String, pub iec_type_tag: u8 }`
- `pub fn build_var_debug_map(container: &Container) -> HashMap<u16, VarDebugInfo>`
- `pub fn format_variable_value(raw: u64, tag: u8) -> String`

The shared `format_variable_value` follows the spec laid out in
`specs/design/vm-cli.md` REQ-VC-009 (vm-cli's existing simple
`T#<ms>ms` / `LTIME#<ms>ms` rendering). This matches both vm-cli's existing
`#[spec_test(REQ_VC_009)]` tests and lsp_runner's behaviour (which simply
fell through to the `_` arm for TIME/LTIME and is now upgraded to the same
spec'd output). Existing tests in both crates continue to pass.

The playground keeps its own richer `format_variable_value` /
`format_variable_value_with_enum` plus the date/time helpers
(`format_time_value_ms`, `format_date_value`, `format_tod_value`,
`format_dt_value`) — those are not duplicates of anything else and produce
UI-friendly output (`T#1.5s`, `D#YYYY-MM-DD`, etc.) that is intentionally
different from the spec'd CLI dump format. The playground does, however,
adopt the shared `VarDebugInfo` and `build_var_debug_map`.

vm-cli previously kept its own `build_var_debug_map` returning
`HashMap<u16, (&str, u8)>`. Switching to the shared `VarDebugInfo`-valued
map costs a small amount of cloning at dump time (CLI output, not perf
critical) and removes another local helper.

## File Map

Created:

- `compiler/container/src/debug_format.rs` — new shared helpers and tests.

Modified:

- `compiler/container/src/lib.rs` — add `pub mod debug_format;` (std-only).
- `compiler/ironplc-cli/src/lsp_runner.rs` — drop local
  `VarDebugInfo`, `build_var_debug_map`, `format_variable_value`; import
  from `ironplc_container::debug_format`.
- `compiler/vm-cli/src/cli.rs` — drop local
  `format_variable_value` and the `(&str, u8)`-valued `build_var_debug_map`;
  use the shared helpers; thread `VarDebugInfo` through
  `write_variable_line` and `dump_variables_*`.
- `compiler/playground/src/lib.rs` — drop local `VarDebugInfo` and
  `build_var_debug_map`; keep playground-specific rich formatting and
  enum-aware formatter.

## Tasks

- [ ] Create `compiler/container/src/debug_format.rs` with the shared
      struct, helpers, and unit tests for `format_variable_value` covering
      every IEC type tag plus the unknown-tag fallback.
- [ ] Wire the new module into `compiler/container/src/lib.rs`.
- [ ] Refactor `lsp_runner.rs` to consume the shared helpers; keep its
      existing tests intact.
- [ ] Refactor `vm-cli/src/cli.rs`: replace local helpers, update
      `write_variable_line` to take `&HashMap<u16, VarDebugInfo>`, update
      the spec tests to construct `VarDebugInfo` instead of `(&str, u8)`.
- [ ] Refactor `playground/src/lib.rs`: drop the duplicate `VarDebugInfo`
      / `build_var_debug_map`; keep the rich formatter and enum map.
- [ ] Run `cd compiler && just` — compile, test, coverage, lint must all
      pass.
