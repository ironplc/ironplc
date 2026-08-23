# plc2plc: re-parse by default, and fix the renderings that then fail

Issue: [#1407](https://github.com/ironplc/ironplc/issues/1407)

## Problem

`plc2plc` renders an array subscript as `refs [ 0 ]`. The parser's
`symbolic_variable` rule chains `.field`, `[i]` and `^` with no
optional-whitespace rule between them, so nothing may separate `refs` from its
`[`. Every rendered program containing a subscript therefore fails to re-parse,
in both expression and assignment-target position.

The deeper problem is why nobody noticed. The plc2plc tests compared rendered
text against committed `*_rendered.st` fixtures and never fed the rendering
back to the parser, so the broken spelling was recorded *as the expected
output*: `reference_to_rendered.st` pinned `value := refs [ 0 ]^ ;`, which does
not parse. A golden-file comparison cannot tell correct output from output the
parser rejects.

## Approach

Two parts, in this order.

1. **Make re-parsing the default** for every plc2plc test. A renderer test is
   now one of two shapes, both in `plc2plc/src/tests/common.rs`:
   - `assert_round_trips(source, &options)` — parse → render → re-parse,
     same AST required;
   - `assert_resource_renders_to(source, rendered, &options)` — the same round
     trip, plus the rendered text pinned against the golden file.

   Both return the rendered text so `contains` checks can sit on top. For
   renderings that deliberately normalize to a different AST spelling there is
   `assert_round_trips_idempotently`, which still re-parses but compares
   text-to-text.

2. **Fix every rendering the re-parse then rejects.** Turning the assertion on
   surfaced #1407 plus five further latent bugs; all are fixed here, since a
   test file that cannot be made to pass is not a working default.

## Changes

### `compiler/plc2plc/src/renderer.rs`

| Bug | Rendered | Fix |
|---|---|---|
| Array subscript (#1407) | `refs [ 0 ]` | `write("[")`, not `write_ws("[")` — only in `visit_array_variable`; the declaration-context `[` call sites are unaffected |
| Whole-valued real | `LREAL#11.0` → `11` | keep a decimal point unless the text already has `.`/`e`/`E` |
| Negative integer | `INT (- 10.. 10 )` | write the sign tight against the digits |
| STRING/WSTRING quotes | `STRING [5] := 'v'` → `:="v"` | the two quote characters were swapped in `visit_string_type_declaration` relative to `visit_string_initializer` |
| Array initializer | `:= [2,7,18]` → `:= 2 , 7 , 18` | restore the enclosing `[ ]` |
| Repeated element | `[3(2)]` → `3 2` | add a `visit_repeated` override rendering `3 ( 2 )` |
| `VAR_CONFIG` | one block per initializer | emit a single block in `visit_configuration_declaration`; the item visitors render just their line |

### `compiler/parser/src/parser.rs`

Two whitespace asymmetries that made valid renderings unparseable:

- `semisep_oneplus` had no `_` before its terminating semicolon (unlike
  `semisep` and `semisep_or_empty`), so a trailing ` ;` was eaten as a
  separator and the rule then demanded another item.
- `array_initial_elements` allowed no whitespace inside the repeat
  parentheses.

### `compiler/plc2plc/src/tests/`

Every file routed through the new helpers. `spec_conformance.rs` and
`spec_conformance_pointer_to.rs` re-parse inside their shared `render()`.

New regression coverage in `reference_to.rs` for subscripts in expression and
assignment-target position, `refs[0]^`, `PT^[0]`, `s.items[0]` and
`grid[1, 2]`; `this_super.rs` regains the `THIS^.values[2]` case that #1404
had to exclude.

`declarations.rs`'s hand-built `LateBoundDeclaration` used `INT` as the base
type, which the parser never produces (an elementary base resolves to a simple
type declaration) and cannot re-parse. Changed to a user-defined base name.

### Fixtures

Twelve `*_rendered.st` files regenerated to the corrected spellings.

## Out of scope

`TYPE MY_ALIAS : INT; END_TYPE` does not parse — an elementary-type alias is a
parser gap, not a renderer bug, and no rendering produces it.

Whether `symbolic_variable` should allow `_` between its elements at all is
the open question from #1404 and #1407; the renderer no longer depends on the
answer.

## Verification

- `cd compiler && just` — full CI: compile, coverage (≥85%), clippy, fmt.
- The re-parse itself is the verification for #1407: `reference_to.rs`'s
  `subscript_expression` / `subscript_assignment_target` cases fail on the old
  renderer and pass on the new one.
