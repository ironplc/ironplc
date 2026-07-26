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
| `compiler/parser/src/parser.rs` | `case_bit_string_literal()` rule; `case_list_element()` gains the new alternative |
| `compiler/codegen/src/compile_stmt.rs` | New `CaseSelectionKind::BitStringLiteral` arm in `compile_case_selector`, mirroring `SignedInteger` |
| `compiler/plc2plc/src/renderer.rs` | Render the new variant (verify generic recursion covers it; add explicit case only if needed, per the recurring lesson in this series) |

## Testing Strategy

- Parser tests: the real motivating shape (`16#D012:`, `2#1010:` as
  `CASE` labels) parses to `CaseSelectionKind::BitStringLiteral`;
  regression -- plain decimal integer labels (`5:`) still resolve to
  `SignedInteger`, not accidentally shadowed by the new alternative.
- Codegen test: a `CASE` statement selecting on a hex-literal label
  produces correct bytecode (matches the selector value), not
  `not_implemented` -- this one should fully work, unlike the stdlib
  functions plan.
- plc2plc round-trip test.
- End-to-end: verify via the CLI that `ironplcc check`/`ironplcc compile`
  both accept the real motivating shape.

## Tasks

- [x] Write plan (this document)
- [x] Verify the exact failure point and confirm it's grammar-only (lexer already tokenizes correctly)
- [x] DSL: `CaseSelectionKind::BitStringLiteral`
- [x] Grammar: `case_bit_string_literal()`, `case_list_element()` update
- [x] Codegen: `compile_case_selector` arm
- [x] Check plc2plc renderer (generic visitor already covers it -- no explicit override needed, see notes)
- [x] Tests from Testing Strategy
- [x] Run full CI pipeline (`cd compiler && just`)
- [ ] Push branch to fork

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
