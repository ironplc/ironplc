# Plan: Diagnose a prefixed literal outside its own type's range (issue #1545)

## Goal

A typed integer literal states its own type, and the value can contradict it:
`INT#40000` is not an `INT` because `INT` holds -32768..32767. Today only the
*destination* type is checked (issue #1527), so `x : DINT := INT#40000` is
accepted because 40000 fits a `DINT`. After this change the literal is also
checked against the type it claims to be and reported as P2026 when it does
not fit.

## Architecture

The check lives in `rule_constant_range`, next to the destination check,
because it answers the same question ("does this value fit this type?") from
the same range table (`value_range::of`). The two checks stay separate:

- the destination check pushes the *stored-into* type down to the literals
  beneath it (unchanged);
- the new check visits every `IntegerLiteral` in the library through the
  visitor's `visit_integer_literal` hook and, when the literal carries a
  `data_type` prefix, looks that type up in the type environment and checks
  the value against its range.

Visiting the literal directly (rather than only where the destination check
already looks) means a prefixed literal is checked wherever it appears: an
initializer, an assignment, an operand, a comparison, a function argument.

### Radix-prefixed forms

`INT#16#FFFF` is checked by value, so it is reported (65535 is not an `INT`).
The destination check already decided that the spelling of a literal makes no
difference -- `16#1FF` is 511 whichever radix it was written in, and the radix
does not survive parsing in any case. The same reasoning applies here: the
type prefix says the value is an integer of that type, and a bit pattern that
wants to wrap is spelled with a bit-string prefix (`WORD#16#FFFF`), which this
rule deliberately does not check.

### Interaction with the destination check

A literal that fits neither its own type nor the destination
(`x : SINT := INT#40000`) is reported once for each, at the same span, with
the range of each type. Both statements are true and each fix is independent,
so neither report suppresses the other.

## Prefactoring

`check_constant` and `check_case` each build the P2026 diagnostic by hand:
the same label text, the same three context entries. The new check is a
third site that would need the same code. Extract `report_out_of_range`
(span, reported value, range) and make both existing sites call it, in its
own commit, before adding the third caller.

## Design doc reference

None. The rationale is local to the rule and lives in its module doc.

## File map

- `compiler/analyzer/src/rule_constant_range.rs` -- prefactor the diagnostic
  builder; add the prefixed-literal check and its tests; update the module
  doc.
- `docs/reference/compiler/problems/P2026.rst` -- document that a prefixed
  literal is checked against its own type, including the radix form.

## Tasks

- [ ] Commit this plan
- [ ] Prefactor: extract `report_out_of_range` (behaviour-preserving; existing tests pass untouched)
- [ ] Add `visit_integer_literal` check against the literal's own type
- [ ] Tests: boundaries per type, in-range accepted, negative into unsigned, radix form, assignment and comparison contexts
- [ ] Update the module doc and P2026 docs
- [ ] Delete this plan
- [ ] `cd compiler && just`
