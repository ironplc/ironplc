# Implementation Plan: Partial Access Bit Syntax (`.%Xn`)

**Design:** [`specs/design/partial-access-bit-syntax.md`](../design/partial-access-bit-syntax.md)

## Goal

Support the IEC 61131-3:2013 partial-access notation `.%Xn` on symbolic
variables, including array elements and struct fields. Fixes programs like
`myByteArray[0].%X0 := TRUE;` which previously failed with P0003 "Unmatched
character sequence" on the `%`.

## Architecture

Treat `.%Xn` as an alternate spelling of the existing `.n` short form. Lowering
reuses the existing `BitAccessVariable` AST node — no new AST, analyzer, or
codegen paths. Only the lexer, parser, options, and a new gating rule change.

Because `BitAccessVariable.variable` is `Box<SymbolicVariableKind>`, chained
access like `arr[0].%X0` already parses and compiles correctly once `.%Xn` is
a recognized token.

Feature is gated behind a new flag `allow_partial_access_syntax`, enabled in
the `rusty` and `iec61131-3-ed3` dialect presets.

## File Map

| Action | File |
|--------|------|
| Add token `PartialAccessBit` | `compiler/parser/src/token.rs` |
| Add flag `allow_partial_access_syntax` | `compiler/parser/src/options.rs` |
| Extend `symbolic_variable` grammar rule | `compiler/parser/src/parser.rs` |
| Add validation rule for disabled flag | `compiler/parser/src/rule_token_no_partial_access_syntax.rs` (new) |
| Register rule in `check_tokens` | `compiler/parser/src/lib.rs` |
| Add problem code `P4033 PartialAccessSyntaxDisabled` | `compiler/problems/resources/problem-codes.csv` |
| Problem doc page | `docs/compiler/problems/P4033.rst` (new) |
| CLI flag | `compiler/ironplc-cli/src/main.rs` |
| LSP extraction | `compiler/plc2x/src/lsp.rs` |
| Playground defaults | `compiler/playground/src/lib.rs` |
| Round-trip fixtures | `compiler/resources/test/partial_access_bit.st` (new), `compiler/plc2plc/resources/test/partial_access_bit_rendered.st` (new) |
| Round-trip test | `compiler/plc2plc/src/tests.rs` |
| End-to-end tests (REQ-PAB-040..042) | `compiler/codegen/tests/end_to_end_bit_access.rs` |
| Parser/lexer tests (REQ-PAB-001..020, 050) | `compiler/parser/src/tests.rs` |
| Options dialect tests (REQ-PAB-051..052) | `compiler/parser/src/options.rs` (tests mod) |
| Analyzer test (REQ-PAB-030) | `compiler/analyzer/src/rule_bit_access_range.rs` (tests mod) |
| Design doc | `specs/design/partial-access-bit-syntax.md` (new) |
| Syntax guide flag table | `specs/steering/syntax-support-guide.md` |
| User docs | `docs/explanation/enabling-dialects-and-features.rst`, `docs/reference/compiler/ironplcc.rst` |

## Tasks

- [x] Commit this plan (phase 1 deliverable per the Planning Requirement).
- [ ] Commit the design doc with REQ-PAB-001..060 markers.
- [ ] Add the `PartialAccessBit` token (`%X\d+`, case-insensitive).
- [ ] Add `allow_partial_access_syntax` flag in `CompilerOptions`, turn on for
  `Rusty` via the macro and for `Iec61131_3Ed3` via explicit override.
- [ ] Extend `symbolic_variable` parser rule with a new `Element::Bit` alternative.
- [ ] Add `rule_token_no_partial_access_syntax` and register in `check_tokens`.
- [ ] Add `P4033 PartialAccessSyntaxDisabled` to `problem-codes.csv` and
  `docs/compiler/problems/P4033.rst`.
- [ ] Wire the CLI `--allow-partial-access-syntax` flag, LSP
  `allowPartialAccessSyntax`, and playground default.
- [ ] Add round-trip fixtures and `write_to_string_partial_access_bit` test.
- [ ] Add e2e tests exercising `arr[0].%X0` read and write (REQ-PAB-040..042).
- [ ] Add parser/lexer/analyzer/options tests for each remaining REQ-PAB.
- [ ] Update syntax-support-guide flag table and user-facing docs.
- [ ] Run `cd compiler && just` and fix any lint/coverage/format issues.
- [ ] Push branch `claude/array-element-bit-access-bSQ1Q`.

## Requirements Traceability

See design doc. Each REQ-PAB-NNN is tied to a specific test function using
the naming convention `{area}_spec_req_pab_nnn_{description}`. The design
doc contains the full requirements-to-tests table.

**Note on enforcement:** The existing `#[spec_test]` proc-macro enforcement in
`compiler/container/build.rs` is scoped to the container crate (bytecode
format and instruction set). Extending it to cover cross-crate language
features is out of scope for this change; REQ-PAB requirements use the
naming-convention + design-doc-table traceability approach instead.

## Verification

1. The user's exact program compiles under `--dialect=rusty` or
   `--allow-partial-access-syntax`:
   ```
   PROGRAM main
     VAR myByteArray : ARRAY[0..1] OF BYTE := [2#00000101, 2#00000000]; r : BOOL; END_VAR
     r := myByteArray[0].%X0;   (* TRUE *)
   END_PROGRAM
   ```
2. Without the flag, the program fails with `P4033 PartialAccessSyntaxDisabled`
   pointing at the `%X0` token (not the legacy P0003 lexer error).
3. `myByteArray[0].0` continues to work unchanged.
4. `cd compiler && just` passes end-to-end.
