# TwinCAT `<Method>` Elements

Fixes the method half of [#1418](https://github.com/ironplc/ironplc/issues/1418):
TwinCAT `<Method>` and `<Property>` XML elements are silently discarded.

## Goal

Stop discarding TwinCAT `<Method>` elements. A `.TcPOU` function block with
methods must parse into a `FunctionBlockDeclaration` whose `methods` field
holds one `MethodDeclaration` per `<Method>` element, with source positions
that point back into the method's own CDATA section in the XML file.

Scope is deliberately narrow — **methods on function blocks only**:

- `<Property>`, `<Get>`, `<Set>` are out of scope. The DSL has no property
  representation and the grammar has no `PROPERTY` rule, so supporting them
  is a grammar change rather than a source-reconstruction change.
- `<Method>` children of an `Itf` (interface) object are out of scope. The
  `interface_declaration` grammar rule parses the header only and has no
  place to put a method signature; appending one would turn every real
  `.TcIO` file that parses today into a parse error.
- `<Method>` children of a `PROGRAM` or `FUNCTION` POU are out of scope for
  the same reason: `method_declaration` is reachable only from
  `function_block_declaration`.

## Design doc reference

`specs/design/beckhoff-twincat-dialect.md` §1.1 already specifies the
intended shape — "the `twincat_parser.rs` module needs to iterate over
`Method` child elements and parse each one, then attach the parsed methods to
the parent FB in the AST". §1.2 covers `<Property>`/`<Get>`/`<Set>`, which
this change does not implement.

## Architecture

`parse_pou` already reconstructs an ST source text by concatenating the
`<Declaration>` CDATA, the `<Implementation><ST>` CDATA, and a synthetic
closing keyword, then hands that to the ST parser. Methods need no grammar
work — `function_block_declaration` already accepts

```
FUNCTION_BLOCK ... <body> <method_declaration>* END_FUNCTION_BLOCK
```

so a `<Method>` element becomes inline ST by **appending** its reconstructed
`METHOD ... END_METHOD` text after the function block body and before
`END_FUNCTION_BLOCK`. A TwinCAT method `<Declaration>` already begins with
the `METHOD` keyword, so only the closing `END_METHOD` is synthesized —
exactly the shape `parse_pou` already uses for the POU itself.

The position mapping is the part that has to change. `CdataOffsets` models
exactly two CDATA regions (a declaration offset/length plus an optional
implementation offset). With per-method CDATA there are `2 + 2n` regions, so
it becomes a list of segments — each recording where a run of copied bytes
starts in the combined text and where the same bytes start in the XML — and
`adjust_byte_offset` looks the position up in that list. Positions that land
in synthetic text (the joining newlines, `END_METHOD`, `END_FUNCTION_BLOCK`)
map to the end of the preceding segment, which is what the two-region
version already did for the synthetic closing keyword.

Text assembly moves into a small builder so that copied text and synthetic
text cannot get out of step with the recorded offsets. For a POU with no
`<Method>` children the builder produces byte-for-byte the same combined text
as today, so existing position behaviour is unchanged.

## File map

Modified:

- `compiler/sources/src/parsers/twincat_parser.rs` — segment-based
  `CdataOffsets`, `CombinedText` builder, `<Method>` extraction and
  appending, unit tests.
- `compiler/parser/src/parser.rs` — correct the `method_declaration` doc
  comment, which claims the TwinCAT XML form is transformed directly to a
  `MethodDeclaration` "without going through this grammar". It never was,
  and after this change it explicitly does go through this grammar.

## Tasks

- [ ] Replace `CdataOffsets`'s two-region fields with a segment list and
      rewrite `adjust_byte_offset` to search it.
- [ ] Add a `CombinedText` builder that records a segment for copied CDATA
      and nothing for synthetic text.
- [ ] Generalize `extract_pou_implementation` to any element with an
      `<Implementation>` child so methods reuse it (including its P9003
      check for FBD/LD/IL/SFC method bodies).
- [ ] Append each `<Method>` child of a `FUNCTION_BLOCK` POU as
      `METHOD ... END_METHOD` between the body and the closing keyword.
- [ ] Correct the `method_declaration` doc comment in the grammar.
- [ ] Tests: methods parsed into `FunctionBlockDeclaration::methods`;
      multiple methods keep source order; a method with a return type and
      `VAR_INPUT`; a method with no implementation; positions inside a
      method body point into that method's CDATA; a syntax error inside a
      method body reports a position inside that method's CDATA; a method
      with an FBD body reports P9003; interfaces and non-function-block
      POUs are unaffected.
- [ ] End-to-end CLI test: a TwinCAT solution fixture whose function block
      declares methods and whose `MAIN` calls them, checked with
      `--dialect twincat`. This is the scenario from the issue that reported
      P4046 against a method declared in the file being checked.
- [ ] `cd compiler && just`
