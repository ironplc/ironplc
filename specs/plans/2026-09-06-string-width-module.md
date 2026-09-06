# Move the string-width inference into its own module

## Goal

`compile_string.rs` answers two different questions. One is "emit the bytecode
for `CONCAT`" — the job its module documentation describes. The other is "what
encoding does this string expression have?", which arrived with #1610 and now
runs to 175 lines: `string_expr_char_width` and the five helpers it delegates
to, plus the unit tests that pin them.

Those 175 lines are the thing the WSTRING operand fix (#1550) has to change.
Moving them out first means that change reads as the rule it is, rather than
as a rule plus a file relocation.

## Change

New `codegen/src/string_width.rs`, holding, verbatim:

| Moved | Visibility after |
|---|---|
| `string_expr_char_width` | `pub(crate)` — `resolve_string_arg` calls it |
| `variable_char_width` | private |
| `access_root` | private |
| `string_char_width_of` | private |
| `function_char_width` | private |
| `resolved_string_char_width` | private |
| `unknown_string_encoding` | private |
| `mod tests` (4 tests) | moves with what it tests |

`compile_string.rs` keeps everything that emits bytecode, and imports the one
entry point. `collect_positional_args` stays where it is — it is already
`pub(crate)` and the emitting code uses it too.

## Prefactor

This *is* the prefactor for the #1550 fix, taken on its own so it can be
reviewed as a move: no behaviour change, and no test edited beyond following
its subject into the new file.

## Confirmation

- No `#[test]` added, removed or edited; the four moved tests keep their names
  and bodies.
- `compile_string.rs` drops from 622 lines to roughly 440, further under the
  1000-line guideline, and its module documentation again describes all of what
  it holds.
- `cd compiler && just` passes.
