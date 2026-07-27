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
- [ ] DSL: `CaseSelectionKind::BitStringLiteral`
- [ ] Grammar: `case_bit_string_literal()`, `case_list_element()` update
- [ ] Codegen: `compile_case_selector` arm
- [ ] Check plc2plc renderer; add explicit case only if the generic visitor doesn't already cover it
- [ ] Tests from Testing Strategy
- [ ] Run full CI pipeline (`cd compiler && just`)
- [ ] Push branch to fork
