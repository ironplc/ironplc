# Parse interface member prototypes

## Goal

Parse the methods and properties an `INTERFACE` declares, in textual ST and in
TwinCAT `.TcIO` files, into the AST. Today an interface with members is a syntax
error in ST (#1419), and in `.TcIO` files the `<Method>` and `<Property>`
elements are silently dropped (the interface half of #1418). After this change,
interface members are no longer lost, which #1891 (analyzing interfaces) needs
first.

Out of scope: any semantic change. Interfaces keep their P9999 ("INTERFACE
declaration is recognized but not yet supported") until #1891.

Builds on #1871 (PROPERTY syntax), which this branch is based on.

## Architecture

Members of an interface are **prototypes**: a signature without a body.
IEC 61131-3 Ed. 3 calls the method form a method prototype (`METHOD name
[: type] <IO var blocks> END_METHOD`). Properties are a CODESYS/TwinCAT
extension; their prototype is `PROPERTY name : type` with `GET` and/or `SET`
accessors that have no bodies.

They get their own AST types rather than reusing `MethodDeclaration` and
`PropertyDeclaration`, so the types themselves rule out a body, locals and
`VAR_TEMP`, and no pass that walks method bodies has to learn to skip
interface members:

- `MethodPrototype { name, return_type, variables, span }`. `variables` holds
  only `VAR_INPUT`/`VAR_OUTPUT`/`VAR_IN_OUT` declarations. It is
  `#[recurse(scope)]`, since its parameters must not leak into the enclosing
  (library-level) scope. That adds `ScopeNode::MethodPrototype`, and each of
  the 9 analyzer passes that match on `ScopeNode` gets an arm for it. This is
  deliberate: `scope.rs` makes that match exhaustive so every pass answers the
  question.
- `PropertyPrototype { name, property_type, get: Option<SourceSpan>,
  set: Option<SourceSpan>, span }`. No variables, so no scope.
- `InterfaceDeclaration` gains `methods: Vec<MethodPrototype>` and
  `properties: Vec<PropertyPrototype>`, in the same shape as
  `FunctionBlockDeclaration`.

Grammar: `interface_declaration` accepts a sequence of `method_prototype` and
`property_prototype` between the header and `END_INTERFACE`, in any order.
Anything else inside (a body, a `VAR` block) stays a syntax error, since the
Ed. 3 grammar does not allow it.

TwinCAT: for an `<Itf>`, `parse_pou` appends each `<Method>` as its
`<Declaration>` followed by `END_METHOD`, and each `<Property>` as its header
followed by `GET END_GET` / `SET END_SET` for each accessor element present,
then `END_PROPERTY`. An `<Implementation>` under an interface member is
reported as malformed (P0009) rather than dropped.

`METHOD ABSTRACT` inside an interface (2 methods in the TwinCAT corpus) needs
the member qualifiers from #1885. When #1885 merges, the rebase adds
`member_qualifiers()` to `method_prototype`. Until then, those 2 methods are
syntax errors.

## Prefactoring

1. **Parser:** extract the shared `METHOD name [: type]` header from
   `method_declaration` and the `PROPERTY name : type` header from
   `property_declaration` into their own rules, so the prototype rules reuse
   them instead of copying them.
2. **plc2plc renderer:** extract the rendering of a method signature (keyword,
   name, return type, var blocks) and of a property header from the
   declaration renderers, so the prototype renderers reuse them.
3. **TwinCAT parser:** replace `if closing == "END_FUNCTION_BLOCK"` in
   `parse_pou` with a `match` on the closing keyword, so the interface case is a
   new arm instead of a second string comparison.

No prefactor for the `ScopeNode` arms. They are the exhaustiveness check
working as designed (see Architecture).

## Design doc reference

- `specs/design/beckhoff-twincat-dialect.md` §1.3 says interface members are not
  parsed. Update it in this PR.
- ADR-0041 covers dispatch, not parsing, so it doesn't change.

## File map

- `compiler/dsl/src/oop.rs`: `MethodPrototype`, `PropertyPrototype`, the new
  `InterfaceDeclaration` fields, updated doc comments.
- `compiler/dsl/src/scope.rs`: `ScopeNode::MethodPrototype`, `ScopeBearing`.
- `compiler/dsl/src/visitor.rs`, `compiler/dsl/src/fold.rs`: dispatch for the new
  types.
- `compiler/parser/src/parser.rs`: header rules (prefactor), `method_prototype`,
  `property_prototype`, `interface_declaration`.
- `compiler/parser/src/tests/interfaces.rs` (new) and `tests/mod.rs`; rows in
  `tests/whitespace.rs`.
- `compiler/analyzer/src/{rule_constant_range, rule_assignment_aggregate_type_compat,
  rule_bit_and_partial_access_range, rule_use_declared_symbolic_var,
  rule_function_call_type_check, xform_resolve_symbol_and_function_environment,
  xform_mark_unwritten_constants, xform_fold_initializer_expressions,
  xform_resolve_expr_types}.rs`: `ScopeNode::MethodPrototype` arms.
- `compiler/plc2plc/src/renderer.rs`: prefactor + prototype rendering;
  `plc2plc/src/tests/interfaces.rs` (new) with round-trip tests.
- `compiler/sources/src/parsers/twincat_parser.rs` and a new
  `twincat_parser/interface_tests.rs`: `<Itf>` members.
- `docs/reference/language/object-orientation/interface.rst`: syntax and the
  "not yet parsed" note.
- `specs/design/beckhoff-twincat-dialect.md` §1.3.

## Tasks

- [ ] Prefactor: parser header rules (1). Existing tests unchanged. Commit.
- [ ] Prefactor: renderer signature helpers (2). Existing tests unchanged. Commit.
- [ ] Prefactor: `match` on the closing keyword in `parse_pou` (3). Commit.
- [ ] AST types, `ScopeNode` variant, visitor/fold dispatch, analyzer arms.
- [ ] Grammar: `method_prototype`, `property_prototype`, members in
      `interface_declaration`. Parser tests: methods with/without return type
      and IO blocks, properties with GET/SET/both, mixed order, `EXTENDS` plus
      members, rejection of a body, a `VAR` block and an accessor body.
      Whitespace rows for every new `_`.
- [ ] Renderer and plc2plc round-trip tests (parse → render → re-parse).
- [ ] TwinCAT `<Itf>` members. Tests: methods and properties kept in document
      order, spans point into the member CDATA, an `<Implementation>` under a
      member gives P0009, a Get-only property.
- [ ] Analyzer test: a prototype's parameter and property types resolve (an
      undeclared type is still reported), and its parameters do not leak into
      another POU's scope.
- [ ] Docs and design doc update.
- [ ] Corpus check: all 17 `.TcIO` files parse, except the 2 with
      `METHOD ABSTRACT` (expected until #1885). They still report only the
      interface P9999.
- [ ] `git rm` this plan, `cd compiler && just`, PR referencing #1419 and #1418.

No end-to-end execution test: interfaces are not executed (P9999) until #1891
and dispatch (#1870).
