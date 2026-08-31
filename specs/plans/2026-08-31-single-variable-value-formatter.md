# Plan: One place that formats variable values

Fixes [#1558](https://github.com/ironplc/ironplc/issues/1558).

## Goal

`ironplcvm run --dump-vars` prints `msg: 0` for a `STRING` variable — an unused
variable-table slot rendered as a plausible-looking integer. The root cause is
not the missing `STRING` arm on its own: there are two near-identical copies of
the value formatter (`container/src/debug_format.rs` and
`playground/src/lib.rs`) that have drifted, plus per-surface `match tag` blocks
in the DAP server and the playground that bolt `STRING` handling on outside the
shared helper.

After this change there is exactly one place that turns a variable into display
text, and every surface — `--dump-vars`, the DAP variables pane, the LSP run
panel, the playground — routes through it.

## Architecture

`ironplc_container::debug_format` gains a `VariableRenderer`: built once from a
`Container` (or a bare `DebugSection`), it owns the three lookups a renderer
needs — VAR_NAME entries, STRING layouts, ENUM_DEF values — and exposes

```rust
fn var(&self, index: u16) -> Option<&VarDebugInfo>
fn name(&self, index: u16) -> String                       // `msg` or `var[3]`
fn render(&self, index: u16, raw: u64, data_region: &[u8]) -> RenderedValue
fn line(&self, index: u16, raw: u64, data_region: &[u8]) -> String
```

`RenderedValue` carries `{ text, valid }` so a surface can style a placeholder
differently from real content (the playground already does).

The per-tag `match` becomes private to the module, and `format_variable_value`,
`read_string_value` and `format_iec_string_literal` stop being public: the
renderer is the only entry point, so a surface cannot reintroduce a partial
copy by reaching past it.

### Divergences to resolve

The two copies disagree; one behaviour has to win.

| Case | shared today | playground today | chosen |
|------|--------------|------------------|--------|
| `TIME` | `T#1500ms` | `T#1.5s` | `T#1500ms` |
| negative `TIME` | `T#-2000ms` | `-T#2s` | `T#-2000ms` |
| `DATE`/`TOD`/`DT` | integer fallback | calendar rendering | calendar rendering |
| `LDATE`/`LTOD`/`LDT` | integer fallback | integer fallback | calendar rendering |
| enum value | ordinal | `RED (0)` | `RED (0)` |
| `STRING` | unused slot as integer | data-region content | data-region content |
| `WSTRING` | unused slot as integer | `<WSTRING>` | data-region content |
| unreadable `STRING` | — | `<invalid>` / `<unknown>` | `<invalid>` / `<unavailable>` |
| unreadable, DAP | `<not available>` | — | `<invalid>` / `<unavailable>` |

`T#<ms>ms` wins over `T#1.5s` because it is what `specs/design/vm-cli.md`
already documents, and because the playground's sign placement (`-T#2s`) is not
a valid IEC 61131-3 duration literal — the sign belongs after the `#`.

`WSTRING` renders rather than staying a placeholder: the data-region header
carries `char_width` (ADR-0035), so the reader can decode UTF-16LE and emit a
double-quoted literal. `read_string_value` currently ignores that field and
computes its byte span as `cur_len` rather than `cur_len * char_width`, which is
a latent bug the moment anything calls it for a wide string.

## Prefactoring

Two reshapings land first, both behaviour-preserving:

1. **`VmStopped` / `VmFaulted` gain `data_region()`.** `--dump-vars` runs after
   `stop()` or `fault()`, and neither state carries the data region, so vm-cli
   *cannot* read a STRING today no matter how the formatter is written.
   `VmRunning` already has the accessor; `stop()`/`fault()` consume `self`, so
   the slice just moves across.
2. **A `VariableView` trait in the vm crate** over `VmRunning`, `VmStopped` and
   `VmFaulted` (`num_variables`, `read_variable_raw`, `data_region`). Three
   surfaces currently carry a copy-per-VM-state pair of otherwise identical
   snapshot functions; the trait collapses each pair to one function and stops
   the new `data_region` argument from having to be threaded through six
   call sites instead of three.

## Design doc reference

New: `specs/design/variable-value-rendering.md` (`REQ-VR-container-*`), owned by
the container crate, holding the rendering table that both copies were
informally implementing. `specs/design/vm-cli.md` REQ-VC-vm-cli-009 keeps its ID
and defers to it instead of restating a table that drifted.

## File map

Created:
- `specs/design/variable-value-rendering.md`
- `compiler/container/src/debug_format/mod.rs` (from `debug_format.rs`)
- `compiler/container/src/debug_format/datetime.rs`
- `compiler/container/src/debug_format/string_value.rs`

Modified:
- `compiler/vm/src/vm.rs` — `data_region()` on stopped/faulted, `VariableView`
- `compiler/container/src/lib.rs` — re-exports
- `compiler/container/build.rs` — register the new design doc
- `compiler/vm-cli/src/cli.rs` — dump through the renderer
- `compiler/vm-cli/src/dap/debug_info.rs` — drop the local `match tag`
- `compiler/ironplc-cli/src/lsp_runner.rs` — drop the duplicate snapshot fns
- `compiler/playground/src/lib.rs` — delete the second formatter
- `specs/design/vm-cli.md` — REQ-VC-vm-cli-009 defers to the new doc
- `docs/reference/runtime/ironplcvm.rst` — document what a value looks like

## Tasks

- [ ] Prefactor: `data_region()` on `VmStopped`/`VmFaulted`; `VariableView`
- [ ] Prefactor: collapse the duplicated snapshot functions onto the trait
- [ ] Split `debug_format.rs` into a directory module
- [ ] `datetime.rs`: duration, date, time-of-day, date-and-time, long variants
- [ ] `string_value.rs`: honour `char_width`, decode UTF-16LE, IEC escapes
- [ ] `VariableRenderer` + `RenderedValue`; make the per-tag match private
- [ ] Rewire vm-cli, DAP, lsp_runner, playground; delete the second copy
- [ ] `specs/design/variable-value-rendering.md` + conformance tests
- [ ] Update `specs/design/vm-cli.md` and `docs/reference/runtime/ironplcvm.rst`
- [ ] End-to-end: a `--dump-vars` test over a real compiled program covering
      STRING, WSTRING, TIME, DATE and an enum — the gap that let this ship
- [ ] `cd compiler && just`
- [ ] `git rm` this plan
