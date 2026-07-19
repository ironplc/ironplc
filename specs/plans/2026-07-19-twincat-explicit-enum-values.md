# Plan: Explicit Enum-Value Assignment (`off := 0, on`) + Enum Base-Type Suffix

## Goal

Survey item 1 from `twincat-status.md`'s "Next" list (11 files). Support
CODESYS/TwinCAT enum declarations where members carry explicit integer
values, and an optional underlying integer/bit-string base type after the
value list:

```
TYPE E_ModeLanguage : (Deutsch := 1, English := 2); END_TYPE

TYPE E_AssertionType :
(
    Type_UNDEFINED := 0,
    Type_ANY,
    Type_BOOL
    (* ... ~40 more, unlabeled *)
) BYTE;
END_TYPE
```

Currently **both fail to parse at all** — `enumerated_value()` only
accepts a bare identifier, with no `:=` position inside the value list at
all (confirmed directly: `TYPE E_ModeLanguage :(Deutsch:=1,English:=2);`
fails with `P0002` exactly at the first `:=`).

## Verification against real files

Checked `/home/husser/code/brotlib` and the 5 cross-repo clones from the
prior branch's validation (still present in the scratchpad) before
designing anything:

- `brotlib`: `E_ModeLanguage.TcDUT` — `(Deutsch:=1,English:=2)`, **every**
  member has an explicit value, no base-type suffix.
- Cross-repo: `tcunit/TcUnit`'s `E_AssertionType.TcDUT` (the file that
  originally motivated this survey item) and `E_XmlError.TcDUT`,
  `OpenCommissioning`'s `E_ControllerType.TcDUT`/`E_FileIoState.TcDUT`/
  `E_NetworkByteOrder.TcDUT` — all use the **"only the first member (or a
  few) has an explicit value, the rest are implicit"** pattern (matching
  ordinary C-style enum semantics: an unlabeled member continues from the
  previous resolved value + 1).
- **New finding, not in the original survey**: `E_AssertionType.TcDUT`
  and `E_XmlError.TcDUT` both end the value list with `) BYTE;` — an
  explicit underlying-type suffix. Only 2 files in the cross-repo sample,
  but one of them is the *exact* file that motivated this survey item in
  the first place — without also supporting this suffix, that file would
  still fail to parse (just further down), so it's included here rather
  than filed as a fully separate follow-up.
- No file combines an explicit default (`:= member`) with the base-type
  suffix, and none use a non-`BYTE` base type, but the grammar accepts
  any integer/bit-string elementary type name for robustness (same
  low-risk, permissive-superset reasoning as other additions this
  session).

## Standard-vs-extension status (per the open question in `twincat-status.md`)

Beckhoff's own docs describe per-member explicit values
(`Red := 2, Green, Blue := 10`) as ordinary syntax, and separately call
out *custom enum base types* (e.g. `BYTE` instead of the implicit
default) as "extension beyond IEC 61131-3." This matches IEC 61131-3:2013
(Edition 3), which added explicit enum values as standard syntax. In
practice this distinction doesn't change the implementation approach
here: **the parser has no mechanism to gate a non-keyword grammar change
by dialect or edition at all** — `parser::parse_library` never receives
`CompilerOptions` (confirmed by reading `lib.rs`: only the lexer stage
gets `options`, via token-demotion transforms for keyword-based features
like `EXTENDS`/`LTIME`; the `peg` grammar itself has zero access to
options). Since `:=` and integer literals are already-existing tokens (no
new keyword to demote), this is parsed unconditionally under every
dialect, exactly matching the precedent already established for
`STRING(n)` parentheses and qualified method calls in the prior two
branches.

## Design

### DSL: `EnumeratedValue.explicit_value`

```rust
// compiler/dsl/src/common.rs
pub struct EnumeratedValue {
    pub type_name: Option<TypeName>,
    pub value: Id,
    /// Present when this member's value was assigned explicitly
    /// (`member := 5`) in an enum *declaration's* value list. `None` in
    /// every other context `EnumeratedValue` is used in (references,
    /// default values, case labels) -- only the declaration-list grammar
    /// path (`enumerated_value_decl()` below) ever sets this.
    pub explicit_value: Option<SignedInteger>,
}
```

13 existing construction sites (across `dsl/src/common.rs`,
`parser/src/parser.rs`, `analyzer/src/xform_resolve_late_bound_expr_kind.rs`,
`sources/src/xml/transform.rs`) get `explicit_value: None` except the one
new grammar path below.

### DSL: `EnumeratedSpecificationInit.underlying_type`

```rust
// compiler/dsl/src/common.rs
pub struct EnumeratedSpecificationInit {
    pub spec: EnumeratedSpecificationKind,
    pub default: Option<EnumeratedValue>,
    /// Present for the CODESYS/TwinCAT base-type suffix
    /// (`(A, B) BYTE;`) -- overrides the automatic count/value-based
    /// sizing in `enumeration.rs`'s `try_from_values`. `None` uses the
    /// existing automatic sizing.
    pub underlying_type: Option<ElementaryTypeName>,
}
```

### Grammar

```
// Only used in the enum member *declaration* list -- NOT the same rule
// used for references (defaults, case labels, expressions), which stays
// exactly as-is.
rule enumerated_value_decl() -> EnumeratedValue =
  ev:enumerated_value() explicit:(_ tok(Assignment) _ si:signed_integer() { si })? {
    EnumeratedValue { explicit_value: explicit, ..ev }
  }

rule enumerated_specification__only_values() -> Vec<EnumeratedValue> =
  tok(LeftParen) _ v:enumerated_value_decl() ++ (_ tok(Comma) _) _ tok(RightParen) { v }

rule enumerated_specification() -> EnumeratedSpecificationKind =
  tok(LeftParen) _ v:enumerated_value_decl() ++ (_ tok(Comma) _) _ tok(RightParen) { EnumeratedSpecificationKind::values(v) }
  / name:enumerated_type_name() { SpecificationKind::Named(name) }

// New: optional base-type suffix, tried at both call sites that build
// EnumeratedSpecificationInit (enumerated_spec_init / enumerated_spec_init__with_values).
rule enum_underlying_type() -> ElementaryTypeName = signed_integer_type_name() / bit_string_type_name()
```

No ordering hazard: `enumerated_value()` (used for references elsewhere)
is unchanged, and the new `:=`/base-type suffixes are purely additive
optional tails on the *declaration*-only productions.

### Shared ordinal-resolution helper (avoids duplicating continuation logic)

Both `analyzer::intermediates::enumeration::try_from_values` (sizing) and
`codegen::compile_enum::build_enum_ordinal_map` (actual codegen ordinals)
need the same "resolve each member to its effective integer value"
algorithm: an explicit value is used as-is; an unlabeled member continues
from the previous resolved value + 1 (starting at 0 if the very first
member has no explicit value) -- ordinary C-style enum semantics, matching
Beckhoff's own documented example and every real file found.

Added once in `ironplc_analyzer::intermediates::enumeration` (re-exported
from the crate root, since `ironplc-codegen` already depends on
`ironplc-analyzer`):

```rust
pub fn resolve_ordinal_values(values: &[EnumeratedValue]) -> Vec<i64> { ... }
```

- `enumeration.rs`'s `try_from_values`: sizing uses
  `underlying_type` directly if present (mapping `BYTE`/`SINT`/`USINT`→B8,
  `WORD`/`INT`/`UINT`→B16, `DWORD`/`DINT`/`UDINT`→B32,
  `LWORD`/`LINT`/`ULINT`→B64); otherwise falls back to the max of
  `resolve_ordinal_values(...)` (not just `values.len()` as today), since
  an explicit value can exceed the member count.
- `compile_enum.rs`'s `build_enum_ordinal_map`: uses
  `resolve_ordinal_values(...)` instead of `values.iter().enumerate()` to
  populate `ordinals`/`value_lookup`/`definitions`. `resolve_enum_ordinal`/
  `resolve_enum_default_ordinal`/expression codegen (`compile_expr.rs`,
  which always loads the ordinal as a plain `i32` constant regardless of
  the enum's underlying-type sizing) need no changes.

## Non-goals

- Validating that an explicit value actually fits within a declared
  `underlying_type` (e.g. `(A := 999) BYTE;`) — no real file needs this
  check; out of scope.
- Negative explicit values sizing correctness beyond "parses and resolves
  to the right `i64`" — no real file uses them; the existing B8/B16/B32
  thresholds are unsigned-range based, which is an existing property of
  `enumeration.rs`, not something this change needs to fix.
- Rejecting duplicate or non-monotonic explicit values — ordinary enum
  semantics allow both; not validated.
- A `--allow-x` dialect flag — see "Standard-vs-extension status" above;
  no gating mechanism exists for this kind of change, and none is needed.

## File Map

| File | Change |
|------|--------|
| `compiler/dsl/src/common.rs` | `EnumeratedValue.explicit_value`; `EnumeratedSpecificationInit.underlying_type` |
| `compiler/parser/src/parser.rs` | `enumerated_value_decl()` grammar rule; base-type-suffix grammar; update the two `EnumeratedSpecificationInit`-constructing rules |
| `compiler/analyzer/src/intermediates/enumeration.rs` | `resolve_ordinal_values()`; sizing uses explicit `underlying_type` or resolved max value |
| `compiler/analyzer/src/lib.rs` | Re-export `resolve_ordinal_values` |
| `compiler/codegen/src/compile_enum.rs` | `build_enum_ordinal_map` uses `resolve_ordinal_values()` |
| Other `EnumeratedValue`/`EnumeratedSpecificationInit` construction sites | Add `explicit_value: None` / `underlying_type: None` |

## Testing Strategy

- Parser tests: all-explicit (`(Deutsch := 1, English := 2)`, matches
  `brotlib`); first-only-explicit (`(A := 0, B, C)`, matches
  `E_AssertionType`); base-type suffix (`(A, B) BYTE;`); regression —
  plain enum with no explicit values or base type still parses unchanged.
- Unit tests for `resolve_ordinal_values`: all-implicit (0,1,2,...);
  all-explicit; first-only-explicit continuation; a gap
  (`A := 5, B` → B resolves to 6).
- `enumeration.rs` sizing tests: explicit `underlying_type` overrides
  count-based sizing; a small member count with a large explicit value
  still sizes correctly from the resolved max, not the count.
- `compile_enum.rs` test: `build_enum_ordinal_map` assigns the resolved
  (not positional) ordinals when explicit values are present.
- plc2plc round-trip test for both new grammar shapes.

## Tasks

- [x] Write plan (this document)
- [x] Grammar: `enumerated_value_decl()` + base-type suffix
- [x] DSL: `EnumeratedValue.explicit_value`, `EnumeratedSpecificationInit.underlying_type`
- [x] `resolve_ordinal_values()` in `enumeration.rs`, re-exported from analyzer
- [x] Update `enumeration.rs` sizing and `compile_enum.rs` ordinal map to use it
- [x] Check plc2plc renderer; fix/extend if needed (found and fixed two
      pre-existing bugs — see Implementation Notes)
- [x] Tests from Testing Strategy
- [x] Run full CI pipeline (`cd compiler && just`)
- [ ] Push branch to fork (no PR against `ironplc/ironplc` without explicit
      go-ahead, per standing instruction)
- [ ] Merge into `twincat-dev`, update `twincat-status.md`, push

## Implementation Notes

- **`fb_name_decl()`-style dead code, again**: `enumerated_spec_init__with_values()`
  (a tuple-returning helper used only by `enumerated_type_declaration__with_value()`)
  became fully unused once that rule was rewritten to inline both branches
  directly (needed to thread the new `underlying_type` field through
  without polluting the shared tuple helpers, since one of them —
  `enumerated_spec_init__with_value()`, singular — is *also* used by
  `structure_element_declaration()`, which must not gain a base-type
  suffix). Deleted the now-dead plural helper; left the singular one and
  the already-separately-dead `enumerated_spec_init()` (confirmed
  unreferenced anywhere, predating this branch) alone beyond fixing their
  compile errors from the new `EnumeratedSpecificationInit` field.
- **Found and fixed two pre-existing plc2plc renderer bugs while adding
  `explicit_value` rendering**, neither related to this feature directly
  but both surfaced by the same investigation: (1) there was no dedicated
  `visit_enumerated_value` override at all — the default recursive
  visitor rendered a qualified value's `TypeName` via `visit_id`'s
  `write_ws`, producing `COLOR RED` instead of `COLOR#RED` (the `#` was
  silently dropped) for *any* qualified enum value reference, regardless
  of this feature; (2) `explicit_value` itself, being `#[recurse(ignore)]`,
  was never visited at all by the pre-existing default recursion, so
  before adding the override, `(Deutsch := 1, English := 2)` silently
  rendered as `(Deutsch, English)` — total data loss on round-trip. Both
  fixed by one new `visit_enumerated_value` override.
- **Sizing must be based on the resolved ordinal value, not the member
  count**: confirmed necessary (not just theoretical) with a real test —
  `(A := 300, B)` has only 2 members (would auto-size to 1 byte via the
  original count-only check) but needs 2 bytes to hold 300. Fixed by
  computing sizing from `resolve_ordinal_values(...).max()` instead of
  `values.len()`; the existing 256/65,536-member-count test thresholds
  are unaffected since, for the all-implicit case, `max value == count -
  1`, giving identical results to the previous behavior (verified: no
  regressions in the pre-existing 10/257/65,537-value sizing tests).
- **The enum base-type suffix (`) BYTE;`) was a new finding, not in the
  original survey** — found while checking real files for a
  disambiguating "does an unlabeled member continue from the previous
  explicit value, or from its own declaration position" example (needed
  to confirm continuation semantics before implementing). Included here
  rather than filed separately because one of the two files using it
  (`tcunit/TcUnit`'s `E_AssertionType.TcDUT`) is the *exact* file that
  originally motivated this survey item — without support for the
  suffix, that file would still fail to parse (just further down).
