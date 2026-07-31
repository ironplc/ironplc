# Plan: Hex/Binary/Octal Bit-String Literals as `CASE` Labels

## Goal

`CASE` labels reject hex/binary/octal bit-string literals
(`16#D012:`, `2#1010:`) -- a real grammar gap, not a stdlib gap. In the
private test corpus, one file fails outright on its very first `CASE`
label, which cascades into 5 separate "function not declared" hits
elsewhere in the same project (once the parser gives up on that file,
everything downstream that would have referenced its declarations is
also unresolvable).

## Verification against the current parser

Reproduced directly (not assumed from the corpus report):

```
CASE x OF
    16#D012:
        y := 1;
    2#1010:
        y := 2;
    ...
```

fails with `P0002` exactly at `16#D012`, with the parser's own error
message confirming the token *is* correctly lexed (`"matched token
16#[0-9A-F][0-9A-F_]* (hexadecimal bit string)"`) -- this is purely a
grammar-acceptance gap in the `CASE` label position, not a lexer gap.

Traced the exact rule: `case_list_element()` (`compiler/parser/src/parser.rs`)
only accepts `subrange()`, `signed_integer()`, or `enumerated_value()` --
no bit-string-literal alternative at all. Plain decimal integer labels
(`5:`) already work fine (via `signed_integer()`); only the radix-prefixed
forms are missing.

## Design

### Grammar: a new `CaseSelectionKind` alternative, without disturbing plain integers

```rust
// compiler/dsl/src/textual.rs
pub enum CaseSelectionKind {
    Subrange(Subrange),
    SignedInteger(SignedInteger),
    EnumeratedValue(EnumeratedValue),
    BitStringLiteral(BitStringLiteral),  // NEW
}
```

```rust
rule case_list_element() -> CaseSelectionKind =
  sr:subrange() { CaseSelectionKind::Subrange(sr) }
  / si:signed_integer() { CaseSelectionKind::SignedInteger(si) }
  / ev:enumerated_value() { CaseSelectionKind::EnumeratedValue(ev) }
  / bsl:case_bit_string_literal() { CaseSelectionKind::BitStringLiteral(bsl) }

// Deliberately narrower than the general bit_string_literal() rule,
// which also falls back to a bare decimal integer() -- including that
// fallback here would create a PEG ordering hazard with signed_integer()
// (a plain "5:" label could start matching as a BitStringLiteral instead
// of a SignedInteger, depending on alternative order). Radix-prefixed
// literals (16#.../2#.../8#...) are already lexically distinct tokens
// from plain decimal digits, so this alternative can only ever fire for
// the genuinely new shape.
rule case_bit_string_literal() -> BitStringLiteral =
  value:(bi:binary_integer() { bi } / oi:octal_integer() { oi } / hi:hex_integer() { hi }) {
    BitStringLiteral { value, data_type: None }
  }
```

No lexer changes -- `binary_integer()`/`octal_integer()`/`hex_integer()`
already exist and are already used by `bit_string_literal()` for the
same underlying tokens (e.g. inside typed initializers).

### Gating: `--allow-bit-string-case-labels` enforced in the analyzer

A radix-prefixed literal as a `CASE` label is **not** IEC 61131-3. The
standard grammar (Annex B, unchanged Ed 2 -> Ed 3) is
`case_list_element ::= subrange | signed_integer | enumerated_value`, and
`signed_integer` is a *decimal* digit sequence -- bit-string literals
(`hex_integer`/`binary_integer`/`octal_integer`) are deliberately not
case labels. So per `specs/steering/syntax-support-guide.md`, this must
be gated behind an `--allow-x` flag; a conformant default/strict parser
must reject it.

The flag is `allow_bit_string_case_labels` (new; grouped with the
Rusty/CODESYS/TwinCAT dialect presets, since these bit-string labels are
a TwinCAT/CODESYS shape). The **parser stays dialect-agnostic** -- it
always produces `CaseSelectionKind::BitStringLiteral`, exactly like the
existing `allow_mixed_located_var_declarations` /
`allow_constant_initializer_expressions` extensions, because the grammar
here (a peg `parser!`) has no access to `CompilerOptions` and the token
`16#D012` is not lexically distinct from a bit-string literal used
anywhere else (an initializer, an expression), so a token-level rule
cannot tell "used as a CASE label" from "used elsewhere".

Enforcement is therefore a semantic rule,
`analyzer/src/rule_case_bit_string_label.rs`, that walks each `Case` and,
when the flag is off, emits `P4041` for every
`CaseSelectionKind::BitStringLiteral` selector. With the flag on the rule
is a no-op and codegen (below) runs.

Wiring (per the syntax-support-guide checklist):

- `parser/src/options.rs` -- new flag in `define_compiler_options!`,
  tagged `[Rusty, Codesys, TwinCat]`; the three dialect enable-set tests
  updated.
- `ironplc-cli/bin/main.rs` -- `--allow-bit-string-case-labels` arg + `|=`
  overlay (the CLI arg list is hand-maintained and guarded by
  `file_args_when_each_vendor_flag_cli_form_passed_then_option_enabled`).
- LSP (`ironplc-cli/src/lsp.rs`) -- **automatic**: `extract_compiler_options`
  derives keys from `FEATURE_DESCRIPTORS`, so `allowBitStringCaseLabels`
  is wired with no code change.
- `mcp/src/feature_flag_conformance.rs` -- new `FlagFixture` (required;
  `every_feature_flag_has_a_fixture` fails otherwise).
- Problem `P4041` (`BitStringCaseLabelNotAllowed`) in the problem-codes
  CSV + `docs/reference/compiler/problems/P4041.rst` + index.
- Docs: `enabling-dialects-and-features.rst`, `ironplcc.rst`, and the flag
  table in `syntax-support-guide.md`.

### Selector type in tests

The standard requires a `CASE` selector to be `ANY_INT` or an enumerated
type. `DWORD` is a bit-string (`ANY_BIT`) type and is itself outside the
standard as a selector, so the tests use an integer selector (`DINT`) and
exercise *only* the label form as the extension under test.

### Codegen: a real fix, not a stub

Unlike the stdlib functions plan, this is a plain literal value with no
runtime-computation ambiguity -- full codegen support is achievable
directly, reusing the exact conversion already used for
`ConstantKind::BitStringLiteral` in `compile_expr.rs` (`lit.value.value`
-> `i32`/`i64` via `u32::try_from`/similar, with `Problem::ConstantOverflow`
on failure). `compile_case_selector`'s existing `SignedInteger` arm is
the direct template.

## Non-goals

- A base-type-prefixed radix literal in `CASE` position (e.g.
  `WORD#16#D012:`) -- not the reported shape (which is bare `16#D012`);
  not investigated.
- Any change to `bit_string_literal()`'s existing behavior elsewhere
  (typed initializers, expressions) -- untouched.

## File Map

| File | Change |
|------|--------|
| `compiler/dsl/src/textual.rs` | New `CaseSelectionKind::BitStringLiteral` variant |
| `compiler/parser/src/parser.rs` | `case_bit_string_literal()` rule; `case_list_element()` gains the new alternative (dialect-agnostic) |
| `compiler/parser/src/options.rs` | New `allow_bit_string_case_labels` flag `[Rusty, Codesys, TwinCat]`; dialect enable-set tests |
| `compiler/analyzer/src/rule_case_bit_string_label.rs` | New semantic rule: emit `P4041` per bit-string CASE label when the flag is off |
| `compiler/analyzer/src/{lib,stages}.rs` | Register the new rule |
| `compiler/ironplc-cli/bin/main.rs` | `--allow-bit-string-case-labels` CLI arg + overlay |
| `compiler/mcp/src/feature_flag_conformance.rs` | New `FlagFixture` for the flag |
| `compiler/problems/resources/problem-codes.csv` | `P4041,BitStringCaseLabelNotAllowed` |
| `compiler/codegen/src/compile_stmt.rs` | New `CaseSelectionKind::BitStringLiteral` arm in `compile_case_selector`, mirroring `SignedInteger` |
| docs | `P4041.rst` + index; `enabling-dialects-and-features.rst`; `ironplcc.rst`; flag table in `syntax-support-guide.md` |

## Testing Strategy

- Parser tests: the real motivating shape (`16#D012:`, `2#1010:` as
  `CASE` labels) parses to `CaseSelectionKind::BitStringLiteral`
  (dialect-agnostic, so with default options); regression -- plain
  decimal integer labels (`5:`) still resolve to `SignedInteger`, not
  accidentally shadowed by the new alternative.
- Analyzer rule tests: flag off -> `P4041` per bit-string label; flag on
  -> accepted; a plain decimal label is never flagged.
- MCP feature-flag conformance fixture: same source rejected off,
  accepted on.
- Codegen test: a `CASE` statement (with the flag on) selecting on a
  hex-literal label produces correct bytecode (matches the selector
  value), not `not_implemented`.
- plc2plc round-trip test.

## Tasks

- [x] Write plan (this document)
- [x] Verify the exact failure point and confirm it's grammar-only (lexer already tokenizes correctly)
- [x] DSL: `CaseSelectionKind::BitStringLiteral`
- [x] Grammar: `case_bit_string_literal()`, `case_list_element()` update
- [x] Gate behind `--allow-bit-string-case-labels`: flag, analyzer rule, CLI, MCP fixture, P4041, docs
- [x] Codegen: `compile_case_selector` arm
- [x] Check plc2plc renderer (generic visitor already covers it -- no explicit override needed, see notes)
- [x] Tests from Testing Strategy
- [x] Use an integer (`DINT`) CASE selector in tests, not `DWORD` (`ANY_BIT` is a non-standard selector type)
- [x] Rebase onto current `main` (resolve the test-file-split conflict from #1261)
- [x] Run full CI pipeline (`cd compiler && just`)
- [ ] Push branch

## Implementation Notes

- **No new visitor/fold dispatch registration needed**: unlike
  `SymbolicVariableKind::SelfRef`/`StmtKind::SelfInvocation`/
  `MethodDeclaration` in earlier plans, `BitStringLiteral` is an
  *already-dispatched* type (used elsewhere via
  `ConstantKind::BitStringLiteral`) -- adding it as a new
  `CaseSelectionKind` variant needed zero `dispatch!` changes in
  `visitor.rs`/`fold.rs`. Confirmed by the build, not assumed.
- **Renderer needed no explicit override either**, unlike every prior
  plan in this series (`THIS`/`SUPER`, `METHOD`) -- `BitStringLiteral`
  already has real content in every field (no `#[recurse(ignore)]`
  leaf needing hand-written text), so the generic recursive visitor
  correctly writes it via the same path already used for
  `ConstantKind::BitStringLiteral`.
- **Found and worked around a real, pre-existing round-trip gap while
  writing the plc2plc test, not introduced by this change**:
  `BitStringLiteral`'s `Display` already renders decimalized everywhere
  in this codebase (confirmed: even an ordinary `x : DWORD := 16#D012;`
  VAR initializer renders as `53266`, not the original hex spelling) --
  and re-parsing that decimal text resolves to `SignedInteger`, not
  `BitStringLiteral` again (same value, different variant), so a plain
  `assert_eq!(original, reparsed)` fails. Not a regression from this
  plan and explicitly out of scope to fix (would mean changing bit-string
  literal rendering everywhere, not just `CASE` labels) -- worked around
  with a render-*idempotency* assertion instead (parse -> render ->
  reparse -> render again, same text), matching the pattern already
  established for the analogous `REFERENCE TO`/`POINTER TO`
  "normalizes to a different spelling" case in
  specs/plans/2026-07-20-twincat-reference-to-no-explicit-deref.md.
- **End-to-end codegen tests verify real execution correctness**, not
  just successful compilation: matching on the correct hex/binary arm,
  the correct binary arm, and confirming no arm executes on a
  non-matching selector -- since this plan (unlike prior ones) claimed
  *full*, not stubbed, codegen support, actual runtime behavior needed
  checking, not just "doesn't error."
