# Plan: Preserve STRING/WSTRING spelling for character string literals

## Goal

`plc2plc` renders every character string literal with single quotes, so a
`WSTRING` literal written `"abc"` inside a statement body comes back as
`'abc'`. Fix the round trip by recording the literal's width on the AST node
and rendering the matching delimiter.

Scope is the cosmetic half of #1550 only. The runtime trap
(`V9014`, comparing a `WSTRING` variable to a literal) is explicitly out of
scope and stays open on that issue.

## Architecture

`CharacterStringLiteral` holds only `value: Vec<char>`. The parser already
knows which of the two rules matched — `single_byte_character_string` sees a
`SingleByteString` token, `double_byte_character_string` a
`DoubleByteString` — and throws the distinction away. Declarations do not have
this problem because `StringInitializer` and `StringDeclaration` carry a
`width: StringType` of their own, and the renderer picks the delimiter from it.

So: give `CharacterStringLiteral` the same `width: StringType` field the two
declaration nodes already carry, populate it in the parser, and let the
renderer choose the delimiter from it exactly as `visit_string_initializer`
already does.

Escaping stays parameterised by the delimiter: the delimiter in force is the
character that needs a `$` escape, so a `'` inside a `WSTRING` literal is
emitted bare rather than as `$'`.

## Prefactoring

None needed. The change adds a field to one leaf node and reads it in one
renderer method; it introduces no new branch in a second place. `StringType`
is the existing type for this distinction, so nothing new is abstracted.

## File map

- `compiler/dsl/src/common.rs` — add `width` to `CharacterStringLiteral`,
  keep `new` for the narrow case, add `new_wide`; make `Display` use the width
- `compiler/parser/src/parser.rs` — carry the width out of the two
  `character_string` rules
- `compiler/plc2plc/src/renderer.rs` — pick the delimiter from the width
- `compiler/resources/test/wstring_ops.st` and
  `compiler/plc2plc/resources/test/wstring_ops_rendered.st` — cover a literal
  in a statement body

## Tasks

- [ ] Add `width: StringType` to `CharacterStringLiteral`
- [ ] Populate it from the parser rules
- [ ] Render the matching delimiter and escape it
- [ ] Extend the corpus with statement-body literals of both widths
- [ ] Unit tests for the parser, the renderer and `Display`
- [ ] `cd compiler && just`
