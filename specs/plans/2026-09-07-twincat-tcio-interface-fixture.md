# Real .TcIO interface fixture for the TwinCAT e2e corpus

## Goal

Add a real, original `.TcIO` fixture file (Beckhoff's file type for a
standalone `INTERFACE` declaration, the `<Itf>` root-child element) to
the e2e CLI test corpus, exercised end-to-end via `ironplcc check
--dialect twincat`.

Closes the still-open half of issue #1428 item 2 ("Add at least one real
`.TcPOU` and one `.TcIO` fixture file ... so XML ingestion is exercised
against a file shaped like the ones users actually have"). The `.TcPOU`
half is already done, via garretfick's PR #1452
(`ironplc-cli/resources/test/twincat_method_solution/`, a `.TcPOU` with
sibling `<Method>` elements). The `.TcIO`/`<Itf>` half has never had a
real file: `Itf` parsing is only exercised today by inline XML string
literals in `compiler/sources/src/parsers/twincat_parser/tests.rs`, and
the `.TcIO` extension itself is only unit-tested inline in
`compiler/sources/src/file_type.rs`. No open PR touches this.

## Architecture

Add a new e2e solution fixture,
`ironplc-cli/resources/test/twincat_interface_solution/`, following the
exact convention of `twincat_method_solution/` (the closest existing
precedent, same dialect and same "real files exercised through the CLI"
shape):

- `TwincatInterfaceSolution.sln`
- `TwincatInterfaceSolution/TwincatInterfaceSolution.tsproj`
- `TwincatInterfaceSolution/PlcAxis/PlcAxis.plcproj`
- `TwincatInterfaceSolution/PlcAxis/POUs/I_Drivable.TcIO` — a real
  `<Itf>` file declaring `INTERFACE I_Drivable`
- `TwincatInterfaceSolution/PlcAxis/POUs/FB_Axis.TcPOU` — a
  `FUNCTION_BLOCK FB_Axis IMPLEMENTS I_Drivable` with a `Start` method
  satisfying the interface
- `TwincatInterfaceSolution/PlcAxis/POUs/MAIN.TcPOU` — declares an
  `FB_Axis` instance and calls `Start()` on it

Register one new e2e test in `ironplc-cli/tests/cli.rs`, mirroring
`check_when_twincat_solution_declares_pou_methods_then_ok`: run
`ironplcc check --dialect twincat` against the new solution directory
and assert success with empty stdout. `--dialect twincat` already turns
on `allow_fb_inheritance` (confirmed in `parser/src/options.rs`), so no
extra flag is needed, same as the existing method-solution test.

Not touched: the inline `Itf` string-literal tests in
`twincat_parser/tests.rs` stay as-is — they test different things (XML
malformation errors, byte-offset/span correctness) that a single e2e
fixture can't replace.

## Prefactoring

None needed. This adds one new fixture directory and one new `#[test]`
function following an existing, already-established pattern
(`twincat_method_solution` / its own e2e test). No existing function
grows a new branch, no module crosses the line-count limit.

## Design doc reference

- `specs/design/beckhoff-twincat-dialect.md` §1.3 (`INTERFACE`/`IMPLEMENTS`,
  and the `.TcIO`/`<Itf>` file mapping)

Already exists; this plan adds test coverage for behavior it describes,
not new behavior.

## File map

- `ironplc-cli/resources/test/twincat_interface_solution/**` — new,
  the e2e solution fixture (`.sln`, `.tsproj`, `.plcproj`, `.TcIO`,
  2x `.TcPOU`)
- `ironplc-cli/tests/cli.rs` — modified, adds one new `#[test]`
  function

## Tasks

- [ ] Write the `.sln`/`.tsproj`/`.plcproj` scaffolding, adapted from
      `twincat_method_solution` with fresh GUIDs
- [ ] Write `I_Drivable.TcIO` (a real `<Itf>` element wrapping
      `INTERFACE I_Drivable ... END_INTERFACE`)
- [ ] Write `FB_Axis.TcPOU` (`FUNCTION_BLOCK FB_Axis IMPLEMENTS
      I_Drivable`, one `Start` method)
- [ ] Write `MAIN.TcPOU` (declares an `FB_Axis` instance, calls
      `Start()`)
- [ ] Add `check_when_twincat_solution_declares_interface_then_ok` to
      `ironplc-cli/tests/cli.rs`
- [ ] Run `cd compiler && just` (compile, coverage, clippy, fmt, dupes)
- [ ] `git rm` this plan file before opening the PR
- [ ] Push the branch and open a PR against `ironplc/ironplc` `main`
