# Gate Explicit Enumeration Member Values Behind `--allow-enum-explicit-values`

## Goal

Strict Edition 2 (`--dialect=iec61131-3-ed2`, the default) currently accepts an
explicit per-member enumeration value:

```iecst
TYPE E_ModeLanguage : (Deutsch := 1, English := 2); END_TYPE
```

Explicit enumerator values are an IEC 61131-3:2013 (Edition 3) addition and a
CODESYS/TwinCAT/RuSTy extension — they are not Edition 2 syntax, so the strict
Edition 2 dialect must reject them with an actionable diagnostic. Every other
dialect keeps accepting them.

Closes the explicit-value half of issue #1541.

## Architecture

Per [ADR-0040](../adrs/0040-dialect-violations-diagnosed-in-policy-phase.md),
the grammar stays maximal and option-free; a disabled feature is diagnosed in a
policy phase with its own problem code.

An explicit value is distinguishable only *structurally* — the `:=` token is
used in a dozen unrelated positions, so there is no token to key on. ADR-0040
rule 3 sends that case to a post-parse analyzer rule over the AST.

The AST already carries the information faithfully:
`EnumeratedValue::explicit_value: Option<SignedInteger>` holds the value the
user wrote, and only the enum *declaration* grammar path sets it (references,
defaults, and case labels leave it `None`). So the rule is a visitor that
reports every `EnumeratedValue` whose `explicit_value` is `Some`.

New flag `allow_enum_explicit_values` (`--allow-enum-explicit-values`), enabled
by `iec61131-3-ed3`, `rusty`, `codesys`, and `twincat`. New problem code
`P4055`.

## Prefactoring

None needed. Adding the flag is one entry in the `define_compiler_options!`
data table (no new `match` arms anywhere — `from_dialect`, the CLI, the LSP,
the playground, and the MCP surface are all generated from or iterate over
`FEATURE_DESCRIPTORS`), and the rule is a new self-contained module rather
than a branch added to an existing one. No existing test setup is duplicated,
and no module approaches the size limit.

## Design doc reference

- [ADR-0040](../adrs/0040-dialect-violations-diagnosed-in-policy-phase.md) —
  dialect violations are diagnosed in a policy phase.
- [syntax-support-guide.md](../steering/syntax-support-guide.md) — the
  new-flag checklist this plan follows.

## Scope note

Issue #1541 also reports the Beckhoff base-type suffix (`(A, B) BYTE;`) as
ungated. That is a separate standards question (a vendor extension, *not*
Edition 3) and therefore a separate flag with a different dialect set; it is
out of scope here and stays on the issue.

## File map

Created:
- `compiler/analyzer/src/rule_enum_explicit_value_allowed.rs`
- `docs/reference/compiler/problems/P4055.rst`

Modified:
- `compiler/problems/resources/problem-codes.csv` — add `P4055`
- `compiler/parser/src/options.rs` — flag entry + dialect-set tests
- `compiler/ironplc-cli/bin/main.rs` — CLI arg and `|=` overlay
- `compiler/analyzer/src/lib.rs`, `compiler/analyzer/src/stages.rs` — register
- `compiler/mcp/src/feature_flag_conformance.rs` — behavioral fixture
- `compiler/plc2plc/src/tests/enums.rs` — round trip under the flag
- `docs/explanation/enabling-dialects-and-features.rst` — flag entry and the
  four `**Enables:**` lists
- `docs/reference/compiler/ironplcc.rst` — flag entry
- `docs/reference/language/data-types/derived/enumerated-types.rst` — document
  explicit values and that they need the flag

## Tasks

- [ ] Add `P4055` to the problem code table
- [ ] Add the `allow_enum_explicit_values` flag and update the dialect tests
- [ ] Add the CLI argument and overlay
- [ ] Write `rule_enum_explicit_value_allowed` with accept/reject tests
- [ ] Register the rule in `stages.rs`
- [ ] Add the MCP behavioral fixture
- [ ] Update the plc2plc round-trip test to enable the flag
- [ ] Write the docs (flag, dialect lists, `P4055`, enumerated-types page)
- [ ] `cd compiler && just`
