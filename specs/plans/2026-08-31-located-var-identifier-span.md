# Fix P4036 pointing at line 1 instead of the located variable

Issue: https://github.com/ironplc/ironplc/issues/1537

## Goal

`P4036` (AT-located variable in a plain `VAR` block) puts its caret on the
first character of the file instead of on the located variable, because
`DirectVariableIdentifier` is built with `span: SourceSpan::default()`
(`range(0, 0)`) at every construction site. Make the span of a located
variable identifier its real source position, and cover it with a test that
asserts the span rather than only `is_err()`.

## Architecture

`DirectVariableIdentifier` carries a `span` field that no construction site
ever fills in — the parser (`vars.rs`, twice) and
`VariableIdentifier::new_direct` all pass `SourceSpan::default()`. The field
is redundant: the node already holds the two things that have a source
position, the optional symbolic `name` and the `address_assignment`.

So the fix removes the field and implements `Located` by hand: the name's
span when there is one, otherwise the address's. That makes the defect
unrepresentable — there is no longer a span to forget to set — rather than
patching the three call sites and leaving the fourth one to be written
wrong.

The address half needs the parser to stop dropping the address span:
`AddressAssignment::try_from` (a `&str` conversion with no position to work
with) leaves `position` at the default, and neither `direct_variable()` nor
`incompl_location()` puts the token's span back. Both rules have the token
in hand, so both set it.

## Prefactoring

Removing the redundant `span` field is the prefactoring signal "a similar
bug could occur rather than being prevented at compile time", but here it is
not separable from the fix: deleting the field *is* what corrects the span,
so it cannot land as a behaviour-preserving commit of its own.

## File map

- `compiler/dsl/src/common.rs` — drop `DirectVariableIdentifier::span`, hand-write
  `Located`; drop the `span` argument handling in `new_direct`; add
  `AddressAssignment::with_position`
- `compiler/dsl/src/core.rs` — correct the `SourceSpan::end` doc comment (it is
  exclusive, as the lexer and the codespan rendering both treat it)
- `compiler/parser/src/vars.rs` — drop the two `span: SourceSpan::default()`
- `compiler/parser/src/parser.rs` — carry the token span into the two
  `AddressAssignment` constructions
- `compiler/analyzer/src/test_macros.rs` — `rule_err1_at!`, a rule test that
  asserts the primary label's span covers a given substring of the program
- `compiler/analyzer/src/rule_mixed_located_var_declarations.rs` — use it

## Tasks

- [ ] `AddressAssignment::with_position`, and set it in the two parser rules
- [ ] Remove `DirectVariableIdentifier::span`, hand-write `Located`
- [ ] `rule_err1_at!` macro asserting a diagnostic's primary span
- [ ] P4036 span tests (named and mixed with several blocks)
- [ ] `cd compiler && just`
- [ ] `git rm` this plan
