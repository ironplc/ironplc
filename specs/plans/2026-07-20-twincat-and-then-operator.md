# Plan: `AND_THEN` Short-Circuit Boolean Operator

## Goal

`AND_THEN` is an unrecognized token today (parse error) — 3 files in the
latest re-scan. It's a genuine Beckhoff/CODESYS extension: a
short-circuit variant of `AND` that only evaluates its right operand
when the left operand is `TRUE`, commonly used to guard a dereference
(`ptr <> 0 AND_THEN ptr^ = 99`).

## Verification against real documentation

Checked Beckhoff's own docs before implementing:

- Beckhoff describes `AND_THEN` as "an extension of the IEC 61131-3
  standard" for Structured Text: an `AND` operation on `BOOL`/`BIT`
  operands with short-circuit evaluation. "TwinCAT executes the
  expressions for other operands only if the first operand of the
  `AND_THEN` operator is `TRUE`" — unlike plain `AND`, where "TwinCAT
  always evaluates all operands." Supported by both TwinCAT and CODESYS.
- Only `AND_THEN` appears in the current survey (not `OR_ELSE`, its
  usual documented pair) — scoping to just `AND_THEN` per "don't add
  capability beyond what's verified needed."

## Design

### Parsing: a distinct token and AST node, not folded into `AND`

Unlike the `REFERENCE TO`/`POINTER TO` -> `REF_TO` unification (safe
because all three spellings are behaviorally identical even in real
TwinCAT/CODESYS), `AND_THEN` has a real, externally-visible behavioral
difference from `AND` recognized by TwinCAT/CODESYS itself
(short-circuit vs. eager evaluation) — the null-pointer-guard example is
exactly a case where eagerly evaluating the right operand would crash.
Folding `AND_THEN` into `CompareOp::And` would make plc2plc silently
rewrite it back to `AND` on render, which is not behavior-preserving for
a real downstream TwinCAT/CODESYS toolchain even though IronPLC's own
semantic analysis doesn't yet model the difference. So this needs its
own `CompareOp::AndThen` variant, not a shared representation.

- New `AndThen` token in `token.rs` (`#[token("AND_THEN", ignore(case))]`),
  right after `And`.
- New `allow_short_circuit_operators` flag (`define_compiler_options!`),
  `[Rusty, Codesys]` -- named for the category (in case `OR_ELSE` is
  ever needed later) rather than the single operator.
- New `xform_demote_short_circuit_operators.rs`, demoting `AndThen` to
  `Identifier` when the flag is off -- parallel structure to
  `xform_demote_oop_keywords.rs`. Registered in `lib.rs`'s
  `tokenize_program` pipeline alongside the other demotion passes.
- Grammar: add `AND_THEN` as an alternative at the same precedence tier
  as `AND` in `expression()`'s `precedence!` block, producing
  `ExprKind::compare(CompareOp::AndThen, x, y)`.
- New `CompareOp::AndThen` variant in `dsl/src/textual.rs`, `Display`
  renders `"AND_THEN"` (round-trip fidelity, not normalized to `AND`).

### Semantic analysis: type-checks identically to `AND`

`rule_ref_to.rs` and `xform_resolve_expr_types.rs`'s existing
`CompareOp::And | CompareOp::Or | CompareOp::Xor` groupings both need
`AndThen` added alongside them -- same operand-type/reference-safety
treatment as `AND`. No new semantic rule; `AND_THEN` isn't flagged as an
unsupported extension (unlike `EXTENDS`/`IMPLEMENTS`) since its meaning
is fully understood and checkable -- only the runtime evaluation-order
guarantee isn't modeled, which doesn't affect `ironplcc check`.

### Codegen: explicit "not implemented" rather than silently-wrong bytecode

`compile_expr.rs`'s `ExprKind::Compare` codegen unconditionally compiles
*both* operands before dispatching on the operator -- there's no
existing short-circuit (conditional-branch) codegen path for any
boolean operator today (`AND`/`OR` are eager too, which is *correct* for
them per Beckhoff's own description). Implementing genuine short-circuit
codegen would need restructuring this into conditional jumps -- out of
scope here (the motivating use case is `ironplcc check`, a diagnostics
backend, not executing compiled TwinCAT code via IronPLC's own VM).

Rather than silently emit eager (behaviorally wrong, and potentially
unsafe -- exactly the null-deref crash `AND_THEN` exists to prevent)
bytecode for `CompareOp::AndThen`, `compile_expr` returns
`Diagnostic::not_implemented(...)` for it -- `ironplcc check` fully
supports `AND_THEN` (the actual need), `ironplcc compile` fails clearly
instead of miscompiling. Two other exhaustive-match sites in
`compile_expr.rs` (`condition_op_type`, `compare_op_to_cmp_op`) also need
`AndThen` added to their existing `And | Or | Xor` groupings.

## Non-goals

- `OR_ELSE` -- not in the current survey; same pattern would apply if
  ever needed.
- Short-circuit codegen/VM execution semantics -- explicitly refused
  with a clear diagnostic rather than silently miscompiled; a much
  larger effort (conditional-branch codegen for all boolean operators)
  if ever pursued, unrelated to the `ironplcc check` motivating use
  case.
- Any change to how plain `AND` is parsed, type-checked, or compiled.

## File Map

| File | Change |
|------|--------|
| `compiler/parser/src/token.rs` | New `AndThen` token |
| `compiler/parser/src/options.rs` | New `allow_short_circuit_operators` flag |
| `compiler/parser/src/xform_demote_short_circuit_operators.rs` | New: demote `AndThen` when the flag is off |
| `compiler/parser/src/lib.rs` | Register the new demotion pass |
| `compiler/dsl/src/textual.rs` | New `CompareOp::AndThen` variant + `Display` |
| `compiler/parser/src/parser.rs` | Grammar: `AND_THEN` alternative in `expression()` |
| `compiler/plc2plc/src/renderer.rs` | Render `AndThen` as `"AND_THEN"` |
| `compiler/analyzer/src/xform_resolve_expr_types.rs` | Add `AndThen` to the type-preserving group |
| `compiler/analyzer/src/rule_ref_to.rs` | Add `AndThen` to the no-op group |
| `compiler/codegen/src/compile_expr.rs` | `AndThen` -> `not_implemented`; add to 2 other groupings |
| `compiler/ironplc-cli/src/lsp_project.rs` | `TokenType::AndThen` semantic-highlighting entry |
| `docs/explanation/enabling-dialects-and-features.rst` | New flag entry |

## Testing Strategy

- Demotion tests (parallel to `xform_demote_oop_keywords.rs`'s existing
  ones): `AndThen` demotes to identifier when the flag is off, stays a
  keyword when on.
- Parser tests: `x AND_THEN y` parses to `CompareOp::AndThen`; regression
  -- `AND_THEN` used as an ordinary identifier when the flag is off still
  parses; the real motivating shape (`ptr <> 0 AND_THEN ptr^ = 99`)
  parses under the flag.
- `xform_resolve_expr_types.rs` test: an `AND_THEN` expression's resolved
  type behaves like `AND`'s (preserves/widens operand type, not forced
  to `BOOL`).
- plc2plc round-trip test: renders as `"AND_THEN"`, not normalized to
  `"AND"`.
- Codegen test: compiling an `AND_THEN` expression produces a
  `Diagnostic::not_implemented` (`P9999`) rather than silently succeeding
  with eager bytecode.
- End-to-end: verify via the CLI that `ironplcc check` accepts the real
  motivating shape under `--dialect=codesys`.

## Tasks

- [x] Write plan (this document)
- [ ] `AndThen` token + demotion module + flag
- [ ] Grammar + `CompareOp::AndThen` + `Display`
- [ ] plc2plc renderer
- [ ] Semantic analysis groupings (`rule_ref_to.rs`, `xform_resolve_expr_types.rs`)
- [ ] Codegen: `not_implemented` + the two other groupings
- [ ] LSP semantic-highlighting entry
- [ ] Tests from Testing Strategy
- [ ] Docs
- [ ] Verify end-to-end via CLI
- [ ] Run full CI pipeline (`cd compiler && just`)
- [ ] Push branch to fork
- [ ] Merge into `twincat-dev`, update `twincat-status.md`, push
