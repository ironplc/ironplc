# Gate the Enumeration Base-Type Suffix Behind `--allow-enum-base-type`

## Goal

Strict Edition 2 (`--dialect=iec61131-3-ed2`, the default) accepts the
base-type suffix on an enumeration declaration:

```iecst
TYPE E_Small : (A, B) WORD; END_TYPE
```

The suffix names the elementary type the members are stored in, overriding
IronPLC's automatic count/value-based sizing. It is a CODESYS/TwinCAT
extension beyond IEC 61131-3 — see
[beckhoff-twincat-dialect.md](../design/beckhoff-twincat-dialect.md) §3.10,
which lists "Enum with Underlying Type" among the TwinCAT constructs — so no
strict dialect should accept it.

Closes the base-type half of issue #1541. The explicit-values half shipped
separately as `--allow-enum-explicit-values` / `P4055`.

## Architecture

Same shape as the sibling `--allow-enum-explicit-values` gate, deliberately:
two flags over one language construct should be read and reviewed as a pair.

Per [ADR-0040](../adrs/0040-dialect-violations-diagnosed-in-policy-phase.md)
the grammar stays maximal and option-free and the violation is diagnosed in a
post-parse policy phase. The suffix is a *structural* extension — the
elementary type keyword that spells it (`BYTE`, `WORD`, `INT`, …) is ordinary
standard syntax everywhere else, so there is no token to demote, and a token
rejection rule would have to key on the `)`-then-type-keyword adjacency and
would fire on malformed input that is not an enum declaration at all. The AST
already records the suffix faithfully in
`EnumeratedSpecificationInit::underlying_type`, so a visitor over the parsed
tree is both exact and consistent with the sibling rule.

New flag `allow_enum_base_type` (`--allow-enum-base-type`), enabled by
`rusty`, `codesys` and `twincat` — **not** `iec61131-3-ed3`, which is where
this differs from the sibling flag. New problem code `P4056`.

### Diagnostic span

`ElementaryTypeName` is a fieldless enum and carries no `SourceSpan`, so the
label cannot point at the suffix keyword itself. The rule visits
`EnumerationDeclaration` and labels its `type_name`, which is on the same
line in every realistic declaration. Giving the suffix its own span means
threading one through `underlying_type` across the parser, renderer, both
analyzer transforms, the XML transform and codegen — far larger than this
change, so it is noted as a follow-up rather than done here.

## Prefactoring

None needed. The flag is one entry in the `define_compiler_options!` data
table (every consumer — `from_dialect`, the LSP, the playground, the MCP
option surface — is generated from or iterates over `FEATURE_DESCRIPTORS`),
and the rule is a new self-contained module.

The one candidate considered and rejected: giving `underlying_type` a
`SourceSpan` so the diagnostic can point at the suffix. That is a
behaviour-preserving reshape touching six files across five crates for a
caret position, which the "no unbounded rewrites" rule in
[development-standards.md](../steering/development-standards.md#when-not-to-prefactor)
says to write up separately rather than fold in. Recorded as a follow-up.

## Design doc reference

- [ADR-0040](../adrs/0040-dialect-violations-diagnosed-in-policy-phase.md) —
  dialect violations are diagnosed in a policy phase.
- [beckhoff-twincat-dialect.md](../design/beckhoff-twincat-dialect.md) §3.10 —
  the construct, and that it is a TwinCAT extension.

## File map

Created:
- `compiler/analyzer/src/rule_enum_base_type_allowed.rs`
- `docs/reference/compiler/problems/P4056.rst`

Modified:
- `compiler/problems/resources/problem-codes.csv` — add `P4056`
- `compiler/parser/src/options.rs` — flag entry + dialect-set tests
- `compiler/ironplc-cli/bin/main.rs` — CLI arg and `|=` overlay
- `compiler/analyzer/src/lib.rs`, `compiler/analyzer/src/stages.rs` — register
- `compiler/mcp/src/feature_flag_conformance.rs` — behavioral fixture
- `compiler/plc2plc/src/tests/enums.rs` — round trip under the flag
- `docs/explanation/enabling-dialects-and-features.rst` — flag entry and the
  three `**Enables:**` lists that gain it
- `docs/reference/compiler/ironplcc.rst` — flag entry
- `docs/reference/language/data-types/derived/enumerated-types.rst` — the
  Base Type section says which dialects accept it

## Tasks

- [ ] Add `P4056` to the problem code table
- [ ] Add the `allow_enum_base_type` flag and update the dialect tests
- [ ] Add the CLI argument and overlay
- [ ] Write `rule_enum_base_type_allowed` with accept/reject tests
- [ ] Register the rule in `stages.rs`
- [ ] Add the MCP behavioral fixture
- [ ] Update the plc2plc round-trip test to enable the flag
- [ ] Write the docs (flag, dialect lists, `P4056`, enumerated-types page)
- [ ] Open the follow-up issue for the suffix `SourceSpan`
- [ ] `cd compiler && just`
