# Add TwinCAT Dialect

## Goal

Add a `twincat` dialect preset that selects the vendor-extension flags needed to
parse code written for the Beckhoff TwinCAT IDE. IronPLC has been growing TwinCAT
support (pragma skipping, `AND_THEN`, empty `CASE` branches, recursive project
discovery), but there is no named `twincat` dialect that turns the relevant
extensions on together. Users currently have to reach for `rusty` or `codesys`
as a stand-in. A dedicated preset documents intent and matches the vendor name.

## Decision: which flags to enable

TwinCAT 3 is built on the CODESYS V3 runtime, so it accepts nearly the same
syntactic vendor extensions as the existing `codesys` preset. The `twincat`
dialect starts from the CODESYS flag set and diverges only where TwinCAT is
stricter:

- All flags that `codesys` enables, **except**:
  - `allow_ref_arithmetic` and `allow_ref_type_punning` are **not** enabled.
    TwinCAT's `REFERENCE TO` is a *managed* reference — it does not support
    pointer arithmetic, and reference type punning is not a TwinCAT idiom.
    (These stay CODESYS-only. Per maintainer review on PR #1250.)
- `allow_system_uptime_global` is **not** enabled, for the same reason it is
  omitted from `codesys`: the implicit `__SYSTEM_UP_TIME` / `__SYSTEM_UP_LTIME`
  globals are an IronPLC/RuSTy runtime convention, not a TwinCAT feature.

Edition 2 stays as the base so identifiers like `LDT` remain usable, matching the
`codesys` and `rusty` approach. Note that TwinCAT's own reference/pointer syntax
(`REFERENCE TO`, `POINTER TO`) is not yet parsed; the `twincat` docs and tests
therefore do not headline `REF_TO` (a CODESYS/IEC spelling TwinCAT does not use).

## File Map

| File | Change |
|------|--------|
| `compiler/parser/src/options.rs` | Add `Dialect::TwinCat` variant; update `ALL`, `display_name`, `description`, `cli_name`; add `TwinCat` to the 16 flag dialect lists (same as `Codesys`); add unit tests |
| `compiler/ironplc-cli/bin/main.rs` | Add `ClapDialect(Dialect::TwinCat)` to `value_variants` |
| `compiler/ironplc-cli/src/lsp.rs` | Add LSP test for the `twincat` dialect |
| `compiler/codegen/tests/it/end_to_end_dialect.rs` | Add end-to-end tests for the TwinCAT dialect |
| `specs/steering/syntax-support-guide.md` | Add TwinCAT row to dialect table |
| `docs/explanation/enabling-dialects-and-features.rst` | Document the `twincat` dialect |
| `docs/reference/compiler/ironplcc.rst` | Add `twincat` to the `--dialect` value list |
| `integrations/vscode/package.json` | Add `twincat` to the dialect setting enum, labels, and descriptions |

The MCP `list_options` tool and the LSP `extract_compiler_options` function are
already data-driven off `Dialect::ALL` / `FromStr`, so they pick up the new
dialect automatically with no code change (only tests are added).

## Tasks

- [x] Create plan
- [ ] Add `Dialect::TwinCat` and wire it into the macro flag lists (omit `allow_system_uptime_global`)
- [ ] Add unit tests covering the TwinCAT flag set
- [ ] Add `ClapDialect(Dialect::TwinCat)` to the CLI
- [ ] Add LSP `twincat` dialect test
- [ ] Add end-to-end dialect tests
- [ ] Update steering guide, docs, and VS Code extension
- [ ] Run `cd compiler && just` and verify all checks pass
