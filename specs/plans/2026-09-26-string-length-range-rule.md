# Reject a declared string length that does not fit its slot

## Goal

A `STRING[70000]` compiles today: codegen does `i.value as u16`, so the
capacity wraps to 4464 with no diagnostic. Report it in the analyzer instead,
as a problem code, at every site a length can be declared. Alongside it,
finish the follow-ups from #1732: reconcile the stale string buffer section of
the runtime design document, and widen the `CONCAT` property test to the
operand bounds #1771 made safe.

## Architecture

A new semantic rule, `rule_string_length_range`, visits the three DSL nodes
that carry a declared length -- `StringInitializer` (variables, structure
fields), `StringSpecification` (array elements, function returns) and
`StringDeclaration` (`TYPE` declarations) -- and reports P2041 for a literal
length above 65,535, the `u16` ceiling of the string header (ADR-0035).
Constants have already been folded to literals by the time rules run, so only
the literal form needs checking. Codegen's two `Result`-returning length
helpers convert with `try_from` and an internal error, so the wrap is
unreachable rather than merely unlikely.

## Prefactoring

None. The rule follows the shape of `rule_real_literal_range` exactly and
touches no existing code path beyond registration.

## Design doc reference

- `specs/design/runtime-execution-model.md` -- String Buffer Management
- ADR-0017, ADR-0034, ADR-0035, ADR-0052

## File map

| File | Change |
|---|---|
| `compiler/problems/resources/problem-codes.csv` | add P2041 |
| `compiler/analyzer/src/rule_string_length_range.rs` | new rule and tests |
| `compiler/analyzer/src/lib.rs`, `stages.rs` | register |
| `compiler/codegen/src/compile_stmt.rs` | `try_from` in the two length helpers |
| `docs/reference/compiler/problems/P2041.rst` | new page |
| `specs/design/runtime-execution-model.md` | rewrite the string buffer section and the stale fields it left elsewhere in the doc |
| `compiler/codegen/tests/it/end_to_end_concat.rs` | property test to 254 per side |

## Tasks

- [ ] Widen the `CONCAT` property test
- [ ] P2041 rule, registration, codegen helpers, docs page
- [ ] Reconcile the design document
- [ ] Remove this plan
