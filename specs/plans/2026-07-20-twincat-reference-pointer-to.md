# Plan: `REFERENCE TO`/`POINTER TO` as Alternate Spellings of `REF_TO`

## Goal

`REFERENCE TO <type>` and `POINTER TO <type>` don't parse at all today —
confirmed directly: both fail immediately after the first word, with the
parser reading `REFERENCE`/`POINTER` as a plain identifier and then
choking on the following `TO` token (a different keyword pair from the
`REF_TO` that `--allow-ref-to`/`--dialect=codesys`/`--dialect=rusty`
already support). Accept both as additional keyword spellings producing
the exact same `ReferenceTarget`/reference-initializer shape `REF_TO`
already does, gated behind the same `--allow-ref-to` flag.

```
FUNCTION_BLOCK FB_Example
VAR
    fbComm : REFERENCE TO FB_Comm;  // currently a parse error
    pComm  : POINTER TO FB_Comm;    // currently a parse error
END_VAR
END_FUNCTION_BLOCK
```

## Verification against real files

Checked a private local checkout of a real TwinCAT codebase:

- Every real usage targets a function-block type name (`FB_Comm`,
  `FB_Sensor`, `FB_Motor`, etc.), never an elementary
  type — 10 files total.
- `REFERENCE TO`-typed variables are always accessed **without** `^`
  (`fbComm.Opened`, `fbComm.Close()`) — real IEC 61131-3:2013
  `REFERENCE TO` semantics (auto-dereferencing, unlike `REF_TO`).
- `POINTER TO`-typed variables are always accessed **with** `^`
  (`pComm^.SetState(...)`) — matching `REF_TO`'s existing pointer-style
  deref convention exactly. `POINTER TO` predates `REF_TO` in TwinCAT (it
  was the pre-Edition-3 pointer spelling) and behaves identically to it.
- Confirmed IronPLC's own semantic rules **don't actually enforce**
  `^`-vs-bare access on a `REF_TO`-typed variable today (tested directly:
  `x.val := 1;` on a `x : REF_TO FB_Comm;` produces zero diagnostics,
  same as `x^.val := 1;` would) — so reusing the exact same
  `ReferenceTarget` DSL shape for all three keyword spellings doesn't
  risk a false negative/positive from the semantic layer; the calling
  convention distinction is purely a source-level spelling habit that
  IronPLC doesn't currently check either way.

## Design

### New tokens: `Reference`, `Pointer`

Two new logos tokens (`REFERENCE`, `POINTER`), following the exact
pattern of the existing `RefTo`/`Ref`/`Null` tokens in `token.rs`.

### Demotion: gated by the same flag as `REF_TO`

Extended in `xform_demote_edition3_keywords.rs`: `Reference` and
`Pointer` demote to `Identifier` under the exact same condition as
`RefTo`/`Ref`/`Null` (`!allow_iec_61131_3_2013 && !allow_ref_to`) — no
new flag. These are alternate spellings of the same reference-type
concept `--allow-ref-to` already gates, not a separate feature.

### Grammar: one new `ref_to_keyword()` rule, three call sites updated

```
rule ref_to_keyword() -> () =
  tok(TokenType::RefTo) {}
  / tok(TokenType::Reference) _ tok(TokenType::To) {}
  / tok(TokenType::Pointer) _ tok(TokenType::To) {}
```

Replaces the three existing `tok(TokenType::RefTo)` call sites
(`ref_to_var_init_decl()`, the `TYPE` reference-type-alias rule, and
`array_specification()`'s `ARRAY [...] OF REF_TO type`) — all three gain
`REFERENCE TO`/`POINTER TO` support for free, matching the existing
precedent of extending every call site of a shared token consistently
(same approach as the `STRING(n)`/`WSTRING(n)` parenthesis work).

No grammar ordering hazard: `Reference`/`Pointer` are dedicated keyword
tokens (via the demotion pattern, matching every other new-keyword
feature this session), not identifiers that could collide with existing
grammar paths.

## Non-goals

- Enforcing `^`-vs-bare access as a real semantic distinction between
  `REFERENCE TO` and `POINTER TO`/`REF_TO` — IronPLC doesn't check this
  today for any of the three spellings, and there's no evidence a real
  file needs it checked.
- A new dialect flag — reuses `--allow-ref-to` entirely.
- `VAR_INST`, `UNION`, or any other construct from
  `specs/adrs/0012-accept-vendor-dialect-files-as-is.md`'s TwinCAT list
  not related to reference types.

## File Map

| File | Change |
|------|--------|
| `compiler/parser/src/token.rs` | New `Reference`/`Pointer` tokens |
| `compiler/parser/src/xform_demote_edition3_keywords.rs` | Demote both under the existing `allow_ref_to` condition |
| `compiler/parser/src/parser.rs` | New `ref_to_keyword()` rule; update 3 call sites |
| `docs/explanation/enabling-dialects-and-features.rst` | Mention the new spellings under the existing `--allow-ref-to` entry |

## Testing Strategy

- Parser tests: `REFERENCE TO <FB-type>` and `POINTER TO <FB-type>` VAR
  declarations parse to the same `ReferenceTarget::Named` shape as the
  equivalent `REF_TO` declaration; `TYPE`-alias and `ARRAY OF` forms too.
- Demotion tests: `Reference`/`Pointer` demote to identifier when
  `allow_ref_to` is disabled (matching `RefTo`'s existing regression
  tests), and stay keywords when enabled.
- Regression: existing `REF_TO` tests unaffected.
- plc2plc round-trip test for both new spellings.

## Tasks

- [x] Write plan (this document)
- [x] `Reference`/`Pointer` tokens
- [x] Demotion wiring
- [x] `ref_to_keyword()` grammar rule + 3 call sites
- [x] Check plc2plc renderer; fix/extend if needed (no change needed —
      already normalizes to `REF_TO` unconditionally, matching the
      `STRING(n)` precedent)
- [x] Tests from Testing Strategy
- [x] Update docs
- [x] Run full CI pipeline (`cd compiler && just`)
- [ ] Push branch to fork
- [ ] Merge into `twincat-dev`, update `twincat-status.md`, push

## Implementation Notes

- **Verified against real files directly, both before and after**: the
  10 real files in this bucket use `REFERENCE TO`/`POINTER TO FB-type`
  exclusively (never elementary types). Confirmed the exact deref-syntax
  split predicted by IEC 61131-3:2013 (`REFERENCE TO` accessed bare,
  `POINTER TO` accessed with `^`) actually holds in the corpus, and
  confirmed empirically that IronPLC doesn't enforce either convention
  today (`x.field` and `x^.field` both parse and analyze identically
  regardless of declared reference kind) — so unifying all three
  spellings onto the same `ReferenceTarget` DSL shape carries no risk of
  a false pass/fail either way.
- **Also found (not part of the original 10-file bucket, but exercised
  by the same real files once the primary blocker was fixed)**: a
  qualified method call on a `REFERENCE TO`-typed variable
  (`fbComm.someMethod();`) still correctly produces the pre-existing,
  unrelated `P9004` (qualified method calls are recognized-but-unsupported,
  landed in an earlier branch) — confirms this fix is properly isolated
  to the reference-type grammar gap and doesn't paper over or interact
  with the separate dispatch-semantics gap.
