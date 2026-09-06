# Reject mixed string encodings at compile time

Issue: [#1550](https://github.com/ironplc/ironplc/issues/1550)

## What is left of #1550

The literal case the issue reported now works: `w = "abc"` on a `WSTRING`
compiles and compares wide, because #1610 taught `string_expr_char_width` to
read the literal's own spelling.

What still fails is every operation whose operands disagree about their
encoding. Measured on `3198fa0`:

| Program | Today |
|---|---|
| `eq := w = 'abc';` | `V9014` trap, scan 1 |
| `out := CONCAT(s, w);` | `V9014` trap |
| `pos := FIND(w, 'cd');` | `V9014` trap |
| `eq := s = "abc";` | `V9014` trap |

Each is a type error the compiler should have caught: `'abc'` is a `STRING`
literal and `"abc"` a `WSTRING` one (IEC 61131-3 Table 5), and `P4034` exists
to reject exactly this mixing. Instead each one compiles, and the VM discovers
it one scan in.

The reason is that each operand is resolved on its own. `resolve_string_arg`
asks `string_expr_char_width` what *this* operand yields and allocates its
temporary at that width, so two operands of one `CMP_STR` can be given
different widths with nothing comparing them.

## Change

An operation resolves **one** encoding for all of its operands, and that is
what every operand is produced at.

New in `string_width.rs`:

- `resolve_operand_char_width(ctx, operands, span)` — each operand answers with
  `string_expr_char_width`; all must agree. Two that disagree have no encoding
  they can share, which is `P4034` with the span of the operand that differs.
- `compile_string_value(emitter, ctx, expr, char_width)` — produces an
  expression as a string value at a named encoding. A literal is *encoded for*
  a declared destination rather than checked against it, which is what an
  assignment target, an array element, a structure field and a function
  parameter each need. Anything else must already match.

`resolve_string_arg` takes the resolved `char_width` from its caller instead of
asking for each operand separately.

Call sites that resolve a group: `compile_len`, `compile_string_compare`,
`compile_find`, `compile_concat`, `compile_replace`, `compile_insert`, and the
one-string helpers behind `LEFT`/`RIGHT`/`MID`/`DELETE`. The string parameter
copy-in in `compile_call.rs` passes the parameter's declared width, and
`STRING_TO_*` passes narrow — so a `WSTRING` argument to it becomes `P4034`
rather than a trap.

Destination sites in `compile_stmt.rs` (scalar string target, string array
element, structure string field) go through `compile_string_value`.

## Not in scope

`w := "abc"` as a statement is rejected by the analyzer with `P4035`, because
it types every character-string literal as `STRING` whatever its quotes. That
predates this work and is not codegen's to fix; it does mean the
destination-encodes-a-literal path is only reachable for `STRING` targets
today, which `compile_string_value` documents.

## Prefactor

Done and landed: #1650 moved the width inference into `string_width.rs`, so
this change is the rule and nothing else.

## Tests

Each of the four measured programs becomes a `P4034` assertion, alongside the
positives that must keep working: `w = "abc"`, `FIND(w, "cd")`, `CONCAT` of two
`WSTRING`s, a `WSTRING` structure field, a `WSTRING` array element, and the
narrow equivalents of all of them.

## Documentation

`V9014.rst` claims the analyzer rejects cross-encoding operations statically,
which is what made the reported trap look impossible. With this change the
claim becomes true for operands, so the page can say what actually reaches the
trap. `P4034.rst` and `wstring.rst` gain the rule that a literal's quotes are
its type.
