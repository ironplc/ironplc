# PROPERTY syntax

Issue: [#1420](https://github.com/ironplc/ironplc/issues/1420) (syntax).
Multi-PR tracker: [#1692](https://github.com/ironplc/ironplc/issues/1692)
(PROPERTY end to end, ADR-0041 Phase 1).

## Goal

Parse `PROPERTY` declarations on function blocks, in ST text and in `.TcPOU`
files, and render them back. After this PR a function block that declares
properties compiles, as long as nothing uses them. Using a property is
rejected with a diagnostic that names it, never compiled into a wrong result.

This is PR 1 of 3 for #1692:

1. **This PR:** syntax (token, AST, parser, `.TcPOU`, plc2plc, flag, scope of
   accessor bodies).
2. [#1421](https://github.com/ironplc/ironplc/issues/1421): method calls in
   expression position. A property read is a `GET` call that returns a value,
   so PR 3 builds on it.
3. Semantics: resolve property reads and writes to `GET`/`SET` calls,
   execution tests, ADR-0041 to `accepted`.

## Design doc reference

- [beckhoff-twincat-dialect.md §1.2](../design/beckhoff-twincat-dialect.md)
  (updated in this PR, see below)
- [ADR-0041](../adrs/0041-staged-method-and-interface-dispatch.md) Phase 1
  (semantics, PR 3)

## Decisions

- **Text form:** as in #1420 and the design doc:

  ```
  PROPERTY Speed : REAL
      GET
      VAR
      END_VAR
          Speed := _speed;
      END_GET
      SET
          _speed := Speed;
      END_SET
  END_PROPERTY
  ```

  `GET` and `SET` are each optional, in that order.
- **`GET`/`SET` are contextual, not keyword tokens.** #1420 and the design doc
  propose keyword tokens that demote when the flag is off. That is not enough:
  under `--dialect twincat` the flag is on, and `SET` is an ordinary name in
  TwinCAT code (the `RS` block's input is `SET`). So the property rule matches
  an identifier whose text is `GET`/`SET`, case-insensitively.
  `PROPERTY`, `END_PROPERTY`, `END_GET` and `END_SET` are keyword tokens,
  demoted unless the flag is set, like `METHOD`/`END_METHOD`.
- **Flag:** `--allow-fb-inheritance`, which already gates `METHOD`, `THIS` and
  `SUPER`. Only its description changes.
- **Out of scope:**
  - Access modifiers (`PROPERTY PUBLIC`), which are
    [#1424](https://github.com/ironplc/ironplc/issues/1424), together with
    `METHOD PUBLIC`.
  - Properties in `INTERFACE` declarations. Interfaces have no member list
    yet (they don't hold method prototypes either).
  - Any use of a property (PR 3).

## Architecture

- **AST (`dsl`):** `PropertyDeclaration { name, property_type, get, set, span }`
  with `get`/`set: Option<PropertyAccessor>`, and
  `PropertyAccessor { variables, edge_variables, body, span }`, the same parts
  a `MethodDeclaration` has. `FunctionBlockDeclaration` gets
  `properties: Vec<PropertyDeclaration>` next to `methods`. Visitor and fold
  get the usual `dispatch!` entries.
- **Parser:** a `property_declaration` rule, and a function-block member rule
  that accepts methods and properties interleaved (TwinCAT stores them in
  file order) and splits them into `methods` and `properties`. A
  case-insensitive sibling of `tok_eq` matches `GET`/`SET`.
- **`.TcPOU`:** each `<Property>` becomes `PROPERTY` + its `<Declaration>`,
  then `GET … END_GET` from `<Get>` and `SET … END_SET` from `<Set>`, each
  from its own `<Declaration>`/`<Implementation>`, then `END_PROPERTY`. This
  mirrors what `append_methods` does for `<Method>`, so both go through one
  helper (prefactor 2).
- **Scope (analyzer):** an accessor body sees the function block's variables,
  its own `VAR` blocks, and the property name as a variable of the property's
  type (the return value in `GET`, the assigned value in `SET`). Without this,
  every accessor body would report its own property name as undefined. Same
  mechanism as `ScopeNode::Method`.
- **Use of a property** (`fb.Prop`, `fb.Prop := x`, bare `Prop` inside the FB)
  gives a not-implemented diagnostic naming the property, not the generic
  "not a variable" one.
- **Codegen:** property accessors are not compiled. Nothing can call them yet.
- **plc2plc:** renders the text form above.

## Prefactoring

1. **Move the OOP AST types out of `dsl/src/common.rs`** (4057 lines, far over
   the 1000-line limit) into a new `dsl/src/oop.rs`: `MethodDeclaration`,
   `FunctionBlockOop`, `InterfaceDeclaration` and their impls. Re-exported
   from `common` so no caller changes. `PropertyDeclaration` then goes into
   `oop.rs` rather than growing `common.rs`. Behaviour-preserving, own commit.
2. **Extract the `<Declaration>`/`<Implementation>` block reconstruction from
   `append_methods`** in `sources/src/parsers/twincat_parser.rs` into a helper
   that takes the element and its closing keyword. Methods keep using it; `GET`
   and `SET` use it in the feature commit. Behaviour-preserving, own commit.

Not prefactored, deliberately: `parser/src/parser.rs` (2125 lines) and
`plc2plc/src/renderer.rs` (1902 lines) are also over the limit, but the grammar
is one `peg` macro and the renderer one `Visitor` impl. Splitting them is a
rewrite much larger than this feature, which the prefactoring rules exclude.
This PR adds roughly 30 and 40 lines to them.

## File map

Created:

- `compiler/dsl/src/oop.rs`
- `compiler/parser/src/tests/property.rs`
- `compiler/plc2plc/src/tests/property.rs`
- `compiler/resources/test/property.st`
- An analyzer rule or `.rs` test file for the "property use not implemented" diagnostic (name decided when implementing)

Modified:

- `compiler/dsl/src/common.rs`, `lib.rs`, `visitor.rs`, `fold.rs`, `scope.rs`
- `compiler/parser/src/token.rs`, `xform_demote_keywords.rs`, `parser.rs`,
  `options.rs` (flag description), `tests/mod.rs`, `tests/whitespace.rs`
- `compiler/sources/src/parsers/twincat_parser.rs`, `twincat_parser/tests.rs`
- `compiler/analyzer/src/…` (scope of accessor bodies, use diagnostic)
- `compiler/plc2plc/src/renderer.rs`, `tests/mod.rs`
- `docs/explanation/enabling-dialects-and-features.rst`,
  `docs/reference/compiler/ironplcc.rst` (flag now covers `PROPERTY`)
- `specs/design/beckhoff-twincat-dialect.md` §1.2: contextual `GET`/`SET`,
  the AST as built

## Tasks

- [ ] Commit this plan
- [ ] Prefactor 1: move OOP AST types to `dsl/src/oop.rs`; all tests pass unchanged
- [ ] Prefactor 2: extract the block helper in `twincat_parser.rs`; all tests pass unchanged
- [ ] Tokens `Property`, `EndProperty`, `EndGet`, `EndSet`, and their demotion under `allow_fb_inheritance`, with demotion tests
- [ ] AST: `PropertyDeclaration`, `PropertyAccessor`, `FunctionBlockDeclaration::properties`, visitor/fold
- [ ] Parser: `property_declaration`, interleaved FB members, case-insensitive `GET`/`SET`; AST-shape tests (GET only, SET only, both, with VAR blocks, interleaved with methods, `SET` still usable as an identifier under `--dialect twincat`)
- [ ] Whitespace rows for the new rule's gaps
- [ ] `.TcPOU`: `<Property>` with `<Get>`/`<Set>`, empty implementation, missing `<Declaration>` → `TwinCatMalformed`; diagnostics point into the XML
- [ ] Analyzer: accessor scope (property name, FB variables, own `VAR`), and the not-implemented diagnostic on use
- [ ] plc2plc: render plus round-trip test that re-parses
- [ ] Flag description and docs
- [ ] Design doc §1.2
- [ ] Check against the brotlib TwinCAT code: files with properties get past parsing (the pass rate may still be limited by `METHOD PUBLIC`, #1424)
- [ ] Open issues for anything this plan names but doesn't deliver
- [ ] `git rm` this plan
- [ ] `cd compiler && just`
