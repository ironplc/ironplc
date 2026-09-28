# Plan: Member Qualifiers (`PUBLIC`/`PRIVATE`/`PROTECTED`/`INTERNAL`/`FINAL`/`OVERRIDE`/`ABSTRACT`)

Issue: #1424

## Goal

Parse the member qualifiers that CODESYS, TwinCAT and IEC 61131-3 Ed. 3 put
on methods and function blocks, and keep them as metadata on the AST:

```st
FUNCTION_BLOCK PUBLIC FINAL FB_Motor

METHOD PRIVATE Reset
END_METHOD

METHOD PUBLIC FINAL Start : BOOL
END_METHOD
END_FUNCTION_BLOCK
```

Today all of these are P0002 except `FUNCTION_BLOCK ABSTRACT`. In the
158-file TwinCAT corpus from #1199 there are about 310 method qualifiers
(`METHOD PRIVATE` 293, `METHOD PUBLIC` 16, `METHOD PROTECTED` 2,
`METHOD ABSTRACT` 1) and 2 `PROPERTY PUBLIC`, so this is the largest single
blocker left in that corpus.

Access is not enforced. ADR-0041 lists access-modifier enforcement as a
non-goal, so a call to a `PRIVATE` method from outside still compiles.

## What TwinCAT accepts

Checked in TwinCAT XAE 3.1.4024.66 on 2026-09-28, one isolated PLC project
per case. Only pass/fail was available: the Error List stayed empty in the
headless shell, so no error text was captured.

| Case | TwinCAT 4024 |
|------|--------------|
| `FUNCTION_BLOCK PUBLIC` / `INTERNAL` / `FINAL` / `PUBLIC FINAL` | builds |
| `FUNCTION_BLOCK PRIVATE` / `PROTECTED` / `OVERRIDE` | error |
| `FUNCTION_BLOCK ABSTRACT FINAL` (either order) | error |
| `FUNCTION_BLOCK FINAL PUBLIC` | error (order matters) |
| `METHOD INTERNAL`, `METHOD PUBLIC FINAL`, `METHOD PUBLIC ABSTRACT` (in `ABSTRACT` FB) | builds |
| `METHOD FINAL PUBLIC`, `METHOD ABSTRACT PUBLIC` | error (order matters) |
| `METHOD FINAL ABSTRACT`, `METHOD PUBLIC PRIVATE`, `METHOD PUBLIC PUBLIC` | error |
| `METHOD ABSTRACT` in a function block that is not `ABSTRACT` | error |
| `METHOD ABSTRACT` with a body | error |
| `METHOD OVERRIDE M` redeclaring a base method | error (cause unknown) |
| `PROPERTY PUBLIC`, `PROPERTY PRIVATE FINAL`, `PROPERTY ABSTRACT` | builds |
| `VAR PUBLIC` / `PROTECTED` / `PRIVATE`, `VAR_INPUT PUBLIC`, with or without `CONSTANT` | error |
| `Final`, `Private`, `Internal`, `Public`, `Protected` as variable names | error |
| `Override` as a variable name | builds |
| Extending a `FINAL` FB, redeclaring a `FINAL` method | error |

What this changes compared to the issue and the first draft of this plan:

- **Access comes first.** The issue says CODESYS is not strict about order.
  TwinCAT 4024 is: the access specifier must come before `FINAL`/`ABSTRACT`.
- **An `ABSTRACT` method does not make its FB abstract.** It is an error on
  an FB that is not itself `ABSTRACT`. The issue's suggested P4045 change is
  not needed: the existing P4045 already covers every valid case.
- **`VAR` access specifiers do not exist in TwinCAT.** Dropped from scope.
- **`OVERRIDE` is probably not TwinCAT syntax** (usable as a variable name,
  and an `OVERRIDE` redeclaration fails). It is still parsed, as Ed. 3
  syntax. `specs/design/beckhoff-twincat-dialect.md` §3.5 lists it as
  TwinCAT syntax and is corrected.
- **Five of the six words are reserved in TwinCAT.** See below.

## Architecture

### Contextual words, not keyword tokens

`PUBLIC`, `PRIVATE`, `PROTECTED`, `INTERNAL`, `FINAL` and `OVERRIDE` are
matched by text, and only in a qualifier slot. Everywhere else they stay
identifiers.

TwinCAT reserves all of them except `OVERRIDE`, so keyword tokens would not
break valid TwinCAT code. Contextual matching is still the better choice:

- `OVERRIDE` is a legal name in TwinCAT, so it has to be contextual anyway.
- `allow_fb_inheritance` is also enabled by the CODESYS and Ed. 3 dialects,
  where the reserved-word list has not been checked.
- Accepting `VAR Final : BOOL;` is a superset of TwinCAT, which is what
  ADR-0040 asks for. Rejecting reserved names is a separate dialect check
  if anyone ever needs it.

`ABSTRACT` is already a token (demoted with the OOP group) and stays one.
The qualifier slot accepts it next to the contextual words.

A word is only taken as a qualifier when another qualifier, or an
identifier that does not start a statement, follows it:

```
rule member_qualifiers() -> MemberQualifiers =
    qs:(q:member_qualifier() &(_ (member_qualifier() / identifier() !(_ statement_continuation()))) { q }) ** _
```

`statement_continuation` is the token after a statement's first
identifier: `:=`, `(`, `.`, `[`, `^`, `REF=`, `S=`, `R=`.

So `METHOD Override : BOOL` is a method named `Override`, and
`FUNCTION_BLOCK Internal VAR ...` is a function block named `Internal`.
The statement check matters because a method without a header is
followed directly by its body: in `METHOD Override x := 1;` the name is
`Override` and `x := 1;` the body. The simpler "followed by an identifier"
lookahead would have taken `Override` as a qualifier there, and that
shape is common in `.TcPOU` files, where the body comes from a separate
`<Implementation>` element.

### Any order in the grammar, order checked afterwards

The grammar accepts qualifiers in any order and keeps them in source order.
The semantic rule then reports a wrong order with a precise message, which
is more useful than a P0002 at the second qualifier. It also keeps the
grammar to one rule for both positions (and for `PROPERTY` later).

### Flag gating (ADR-0040 rule 3)

The parser has no access to `CompilerOptions`, so contextual words cannot
be gated at the token level. Methods are already gated (the `METHOD` token
is demoted without the flag). Function block qualifiers are gated by a
post-parse analyzer rule, following `rule_enum_base_type_allowed`.

### AST

New module `dsl/src/member_qualifier.rs` (`common.rs` is already over the
1000-line limit, and #1871 creates `oop.rs`, so a separate module avoids
both):

```rust
pub enum AccessSpecifier { Public, Private, Protected, Internal }

pub enum MemberQualifierKind { Access(AccessSpecifier), Abstract, Final, Override }

pub struct MemberQualifier { pub kind: MemberQualifierKind, pub span: SourceSpan }

/// Source order preserved.
pub struct MemberQualifiers(Vec<MemberQualifier>);
impl MemberQualifiers {
    fn access(&self) -> Option<AccessSpecifier>;
    fn is_abstract(&self) -> bool;
    fn is_final(&self) -> bool;
    fn is_override(&self) -> bool;
}
```

- `FunctionBlockOop::is_abstract: bool` becomes `qualifiers: MemberQualifiers`.
- `MethodDeclaration` gains `qualifiers: MemberQualifiers`.
- `FunctionBlockDeclaration::is_abstract()` replaces the three copies of
  `oop.as_ref().is_some_and(|oop| oop.is_abstract)`.

### Diagnostics

Two new problem codes (next free numbers at implementation time; P4057 is
taken by #1860):

- **MemberQualifierNotAllowed**: a qualifier on a function block without
  `--allow-fb-inheritance` (or a dialect that enables it).
- **MemberQualifierInvalid**: one code, the message says which case:
  - the same qualifier twice
  - more than one access specifier
  - `ABSTRACT` together with `FINAL`
  - an access specifier after `FINAL`/`ABSTRACT`/`OVERRIDE`
  - `OVERRIDE`, `PRIVATE` or `PROTECTED` on a function block
  - `ABSTRACT` method in a function block that is not `ABSTRACT`
  - `ABSTRACT` method with a non-empty body

P9004 is unchanged. An `ABSTRACT` method can only be valid inside an
`ABSTRACT` function block, which already raises it. Access specifiers,
`FINAL` and `OVERRIDE` do not raise P9004: they are metadata-only by design
(ADR-0041), and raising it would turn all 293 `METHOD PRIVATE` in the
corpus into errors.

## Open points

- **Order in CODESYS and Ed. 3.** Only TwinCAT 4024 was checked. TwinCAT's
  PLC compiler is CODESYS-based, so CODESYS very likely behaves the same.
  I think the Ed. 3 grammar also puts the access specifier first, but I
  have not checked the standard. If a dialect turns out to allow any order,
  the order check becomes dialect-dependent.
- **The XAE results are pass/fail only.** Where a case has more than one
  possible cause (T21 `OVERRIDE`), the cause is inferred. TwinCAT 4026 was
  not checked.
- **`PROPERTY` accessors (#1871).** XAE stores a `Get` accessor's
  `<Declaration>` as `PUBLIC\nVAR\nEND_VAR`, so an accessor carries its own
  access specifier. The `.TcPOU` reader in #1871 needs to accept that.
  To be checked on that branch, not here.

## Prefactoring

1. Replace `FunctionBlockOop::is_abstract: bool` with
   `qualifiers: MemberQualifiers` (only `Abstract` exists at this point),
   and add `FunctionBlockDeclaration::is_abstract()`. The three readers
   (P4045, P9004, renderer) call the helper instead of reaching into `oop`.
   No behaviour change. This is where the new qualifier kinds drop in.
2. Add the `contextual_keyword(text)` parser rule and use it for the
   existing inline `[t if t.token_type == Identifier && t.text.eq_ignore_ascii_case(..)]`
   matches (`REF`, `S`, `R`). #1871 adds an identical rule; whichever PR
   merges second drops its copy.

## Design doc reference

- `specs/design/beckhoff-twincat-dialect.md` §1.5, §3.5, token list at the end
- ADR-0040 (post-parse enforcement of dialect flags)
- ADR-0041 (access enforcement is a non-goal)

## File Map

| File | Change |
|------|--------|
| `compiler/dsl/src/member_qualifier.rs` | New: `AccessSpecifier`, `MemberQualifier(s)` |
| `compiler/dsl/src/lib.rs` | Register module |
| `compiler/dsl/src/common.rs` | `FunctionBlockOop.qualifiers`, `MethodDeclaration.qualifiers`, `FunctionBlockDeclaration::is_abstract()` |
| `compiler/dsl/src/visitor.rs`, `fold.rs` | Dispatch for new types if `Recurse` needs it |
| `compiler/parser/src/parser.rs` | `contextual_keyword`, `member_qualifiers`, use in FB and method rules |
| `compiler/parser/src/tests/` | New `member_qualifiers.rs` |
| `compiler/analyzer/src/rule_member_qualifier_allowed.rs` | New: flag gate |
| `compiler/analyzer/src/rule_member_qualifier_invalid.rs` | New: combination/order/position check |
| `compiler/analyzer/src/rule_abstract_not_instantiated.rs` | Use `is_abstract()` |
| `compiler/analyzer/src/rule_unsupported_extension.rs` | Use `is_abstract()` |
| `compiler/analyzer/src/stages.rs` | Register the two rules |
| `compiler/plc2plc/src/renderer.rs` | Render qualifiers on FB and method |
| `compiler/plc2plc/resources/test/` | Round-trip fixture |
| `compiler/sources/src/parsers/twincat_parser/tests.rs` | `.TcPOU` with `METHOD PRIVATE` |
| `compiler/sources/src/xml/transform.rs`, other `MethodDeclaration`/`FunctionBlockOop` construction sites | New fields, mechanical |
| `compiler/problems/resources/problem-codes.csv` | Two new codes |
| `docs/compiler/problems/P40xx.rst` | Two new pages |
| `specs/design/beckhoff-twincat-dialect.md` | §1.5: suffix form, access-first order, contextual matching, metadata-only, no `VAR` access specifiers. §3.5: `OVERRIDE` not TwinCAT 4024 syntax |

## Tasks

- [x] Commit this plan
- [x] Prefactor 1: `MemberQualifiers` on `FunctionBlockOop`, `is_abstract()` helper (own commit)
- [x] Prefactor 2: `contextual_keyword` rule (own commit)
- [x] Methods: grammar, `MethodDeclaration.qualifiers`, renderer, parser tests incl. identifier regressions (`METHOD Override`, `x := Private;`), round-trip, `.TcPOU` test
- [ ] Function blocks: `FINAL`/access in the FB slot, flag-gate rule + problem code
- [ ] Validation: `MemberQualifierInvalid` rule + problem code, one test per case in the XAE table
- [ ] Update design doc §1.5 and §3.5
- [ ] Measure corpus pass rate before/after (`--dialect twincat`, method in #1199)
- [ ] Open issues: `FINAL` enforcement (extending a `FINAL` FB, redeclaring a `FINAL` method); `PROPERTY` qualifiers once #1871 lands
- [ ] `git rm` this plan
- [ ] `cd compiler && just`
