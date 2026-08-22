# Plan: `THIS^` and `SUPER^` parsing (ADR-0041 Phase 1, front-end only)

## Goal

Parse `THIS^` and `SUPER^` — the self-reference and base-reference forms
used by CODESYS/TwinCAT OOP — behind the existing
`--allow-fb-inheritance` flag, round-trip them through plc2plc, and
report them as not-yet-implemented (P9999) instead of a syntax error.

This is the next slice named as out-of-scope in
`specs/plans/2026-08-12-oop-method-declarations-static-dispatch.md`, and
it is item 2 of Phase 2 in `specs/design/beckhoff-twincat-dialect.md`.
It is deliberately front-end only: **no code generation**. ADR-0041's
Phase 1 semantics for `THIS^`/`SUPER^` (a receiver pointer plumbed
through method codegen; `SUPER^` calling the base's own body) stay
deferred to the codegen slice that also owns `MethodCall` codegen, which
is likewise unstarted.

## Scope

In scope:

- `THIS` / `SUPER` keyword tokens, demoted to identifiers unless
  `allow_fb_inheritance` (same gate and same mechanism as `METHOD`,
  `EXTENDS`, `IMPLEMENTS`, `ABSTRACT`).
- `THIS^` / `SUPER^` as the head of a variable reference, in every
  position an ordinary variable reference is legal:
  - read: `v := THIS^.count;`, `v := SUPER^.arr[2];`
  - write: `THIS^.count := 1;`
  - statement-position method call: `THIS^.Start();`, `SUPER^.Start();`
  - bare: `v := THIS^;` (parses; nothing can be done with it yet)
- plc2plc rendering and round-trip.
- `P9999` per occurrence from `rule_unsupported_extension`, via a
  `LanguageExtension` impl — the pattern already used for
  `IMPLEMENTS`, `ABSTRACT`, and `INTERFACE`. Earlier passes will also
  report the construct as unimplemented, so the diagnostic output is
  expected to be redundant and blunt at first; see *Analyzer* for why
  that is preferred to silencing them.
- Codegen: an explicit not-implemented arm (`Diagnostic::todo`) as a
  backstop, mirroring how `StmtKind::MethodCall` is handled today.

Out of scope:

- Any execution semantics or bytecode. `SUPER^`'s base-method dispatch
  and `THIS^`'s receiver pointer belong to the codegen slice.
- Type-checking / member resolution through `THIS^`/`SUPER^` (no
  "member does not exist on the enclosing FB" diagnostic yet).
- Unqualified self-calls (`Start();` inside a method body).
- `THIS^` / `SUPER^` in expression-position method calls
  (`IF THIS^.IsMoving() THEN`) — expression-position method calls do
  not exist for any receiver yet.
- `p^.Method()` — a method call on a dereferenced pointer. It is a
  syntax error today and stays one; only `THIS^`/`SUPER^` get a
  receiver form here.
- Editor syntax highlighting. `integrations/vscode/syntaxes/` does not
  list `EXTENDS`/`METHOD` either; adding OOP keywords there is its own
  change.

## The whitespace question: is `SUPER ^` allowed?

**Short answer: nothing makes it illegal, real code never writes it, and
IronPLC should accept it.**

Neither IEC 61131-3 nor the CODESYS/TwinCAT dialects define `THIS^` as a
single lexical token. `THIS` and `SUPER` are keywords whose value is a
pointer to an instance, and `^` is the ordinary dereference operator
applied to them — the same `^` as in `pMotor^`. Token-based grammars
separate those two tokens on whitespace like any other pair, so on the
letter of the grammar `SUPER ^` is a dereference of `SUPER` and is legal.
This has not been verified against the CODESYS or TwinCAT compilers
themselves, and their documentation does not state a rule either way.

IronPLC's own parser is currently inconsistent about whitespace before
`^`, which is worth knowing before picking a rule. Measured against the
`codesys` dialect on the current `main`:

| Source | Result today |
|---|---|
| `v := p^;` | parses |
| `v := p ^;` | **P0002 syntax error** |
| `p^ := 1;` | parses |
| `p ^ := 1;` | parses |
| `p^.x := 1;` | parses |
| `p ^.x := 1;` | **P0002 syntax error** |
| `v := a [1];` | **P0002 syntax error** |

The cause is that `symbolic_variable` (`parser.rs:852`) chains its
elements — `.field`, `[i]`, `^` — with no `_` (optional
whitespace/comment) rule between them, while the deref-assignment rule
at `parser.rs:1817` does have `_` around its caret. So the strictness is
an accident of two rules, not a decision.

**Recommendation:** put `_` between the keyword and its caret in the new
rules, i.e. accept `THIS^`, `THIS ^`, and `THIS /* comment */ ^`.
Reasons:

- It costs one token in one rule.
- It cannot mis-accept a standard program: with the flag off, `THIS` and
  `SUPER` are demoted to plain identifiers and none of these rules
  apply. With the flag on they are keywords, so no program can be using
  `SUPER` as a variable that `SUPER ^` might have dereferenced.
- Accepting a shape a vendor might reject is harmless; rejecting one a
  vendor accepts is a parse failure on real code.

Two related decisions that follow from the same reasoning:

- **The `^` stays mandatory.** `THIS.count` and a bare `THIS` are not
  accepted. That matches CODESYS/TwinCAT, where these are pointers.
- **Whitespace *after* the caret keeps today's behavior** — `THIS^.x`
  parses, `THIS^ . x` does not, exactly as `p^.x` / `p^ . x` behave now.
  Relaxing member access generally is a separate change; this slice
  should not make `THIS^` more permissive than `p^` past the caret.

(The same inconsistency also produces a pre-existing plc2plc rendering
bug, tracked separately as issue #1404. It is not part of this feature
and this slice does not touch it.)

## Design

### Tokens (`compiler/parser/src/token.rs`)

Add next to the other OOP keywords (`token.rs:206`):

```rust
#[token("THIS", ignore(case))]
This,
#[token("SUPER", ignore(case))]
Super,
```

Plus their `describe`/display strings (`token.rs:669`) and an entry in
the keyword round-trip table (`token.rs:884`).

### Gating (`compiler/parser/src/xform_demote_keywords.rs`)

One arm added to the existing OOP group — no new flag, no new gate:

```rust
TokenType::Extends
| TokenType::Implements
| TokenType::Interface
| TokenType::EndInterface
| TokenType::Abstract
| TokenType::Method
| TokenType::EndMethod
| TokenType::This
| TokenType::Super => demote_oop,
```

With the flag off, `THIS : INT;` and `THIS := 1;` keep working, and
`THIS^.v := 1;` keeps its current meaning (dereference of a variable
named `THIS`). That existing behavior must not change — it is a
regression test, not just a nicety.

### AST (`compiler/dsl/src/textual.rs`)

`THIS^` is not only an expression: `THIS^.count := 1;` puts it in
assignment-target position, where the AST is `Variable` /
`SymbolicVariableKind`, not `Expr`. So the new node belongs at the
**head of a symbolic variable**, not as an `ExprKind` variant. (The
sketch in `specs/design/beckhoff-twincat-dialect.md:673` proposed
`ExprKind::ThisRef`/`SuperRef`; that shape cannot express the assignment
target, so this plan supersedes it.) Putting it at the head means every
existing element chain — `.field`, `[i]`, `.%X0`, `^` — composes with it
for free, in both read and write position.

```rust
pub enum SelfRefKind { This, Super }

pub struct SelfRefVariable {
    pub kind: SelfRefKind,
    pub span: SourceSpan,   // spans keyword through caret
}

pub enum SymbolicVariableKind {
    // ...existing...
    SelfRef(SelfRefVariable),
}
```

`Display` renders `THIS^` / `SUPER^` (the caret is part of the node —
see below).

**The caret is folded into the node** rather than modeled as
`DerefVariable { variable: This }`. `THIS`/`SUPER` are not pointers in
IronPLC's type system and there is no pointer type to give them, so an
un-dereferenced form would be a node nothing can type. Folding the caret
in also makes "the caret is mandatory" a structural property instead of
a semantic check. The trade-off is that `SelfRefVariable`'s `Display`
carries a `^` its name does not mention; the alternative (two variants,
`ThisRef`/`SuperRef`, no `kind` enum) is a reasonable second choice and
costs a few more match arms.

Adding a `SymbolicVariableKind` variant makes the compiler enumerate the
work: ~50 match sites across analyzer and codegen, most with existing
wildcard arms. That is the intended forcing function.

### Method-call receiver (`compiler/dsl/src/textual.rs`)

`MethodCall.instance` is an `Id` today (`textual.rs:308`), which cannot
hold `THIS^`. Narrowest change that works:

```rust
pub enum MethodReceiver {
    Instance(Id),           // instance.M()
    SelfRef(SelfRefVariable), // THIS^.M() / SUPER^.M()
}

pub struct MethodCall {
    pub receiver: MethodReceiver,
    pub method: Id,
    pub params: Vec<ParamAssignmentKind>,
    pub position: SourceSpan,
}
```

Not widened to a full `Variable` receiver: that would silently start
accepting `p^.M()` and `a.b.M()`, which is a separate feature with its
own resolution rules.

### Parser (`compiler/parser/src/parser.rs`)

One new rule, and one new alternative in each of two existing rules:

```
rule self_ref() -> SelfRefVariable =
    t:tok(TokenType::This)  _ c:tok(TokenType::Caret) { /* This  */ }
  / t:tok(TokenType::Super) _ c:tok(TokenType::Caret) { /* Super */ }
```

`symbolic_variable()` (`parser.rs:852`) gains a head alternative — the
element chain after it is untouched:

```
rule symbolic_variable() -> SymbolicVariableKind =
    head:( s:self_ref() { SymbolicVariableKind::SelfRef(s) }
         / name:variable_identifier() { SymbolicVariableKind::Named(..) } )
    elements:( ...unchanged... )*
```

`method_invocation()` (`parser.rs:1852`) gains a receiver alternative:

```
rule method_invocation() -> StmtKind =
    recv:( s:self_ref() { MethodReceiver::SelfRef(s) }
         / id:identifier() { MethodReceiver::Instance(id) } )
    _ period() _ method:identifier() _ tok(LeftParen) ... 
```

No dialect gating is needed in the grammar itself: without the flag the
`This`/`Super` tokens never reach the parser, so both alternatives are
unreachable — the same argument that already justifies ungated
`method_invocation`.

Ordering note: `statement()` tries `assignment_statement()` first, so
`THIS^.M();` is attempted as an assignment, fails at the missing `:=`,
and backtracks into `subprogram_control_statement()`. PEG backtracking
handles this; no reordering needed.

### Visitor / fold (`compiler/dsl/src/visitor.rs`, `fold.rs`)

`dispatch!(SelfRefVariable);` in both. The `MethodCall` receiver change
also touches whatever currently walks `instance`.

### Analyzer

**No quiet arms.** The compiler will force a new match arm at every site
that matches `SymbolicVariableKind` exhaustively. Every one of those arms
must be *loud* — return an explicit not-implemented diagnostic
(`Diagnostic::not_implemented` / `Diagnostic::todo`, naming the pass) —
never a silent `None`, `Ok(())`, or "skip this node". A quiet arm is
indistinguishable from a handled case, so when `THIS^`/`SUPER^` later
grows real semantics, a pass that was silently ignoring it keeps
ignoring it and miscompiles instead of failing. Being noisy and slightly
confusing at first is the accepted cost; the diagnostics get refined
once the construct has semantics worth resolving.

- `impl LanguageExtension for SelfRefVariable` with name
  `"THIS^"` / `"SUPER^"` (from `kind`), plus a
  `visit_self_ref_variable` override in
  `rule_unsupported_extension.rs` → one `P9999` per occurrence.
- The semantic rules run *after* the resolution transforms
  (`stages.rs:337` vs `stages.rs:135-300`), so the transforms meet the
  new variant first and will report it before `rule_unsupported_extension`
  ever runs. That is fine and expected: a program using `THIS^` is
  rejected either way. The sites the compiler will point at are
  `xform_resolve_expr_types.rs` (14, incl. `find_base_variable_name`),
  `xform_resolve_late_bound_expr_kind.rs` (7),
  `rule_bit_access_range.rs` (6), `rule_ref_to.rs` (8),
  `xform_insert_implicit_deref.rs`, `xform_fold_initializer_expressions.rs`,
  `rule_function_call_type_check.rs`, `rule_string_encoding_compat.rs`.
- Consequence to accept up front: a single `THIS^.count := 1;` may
  produce several diagnostics — one per pass that meets it — and their
  wording will be blunt ("`THIS^` is not supported by
  <pass>"). Do not tune this by suppressing passes; the redundancy is
  the signal that each pass has an unimplemented case.
- Existing `_ =>` wildcard arms are not to be *relied on* for the new
  variant. Where the compiler does not force a change, check whether a
  wildcard is swallowing `SelfRef` into a wrong-but-plausible path
  (rather than into a diagnostic); if it is, split the arm out
  explicitly. Where a wildcard already lands in an error path, leave it.
- `rule_use_declared_symbolic_var` needs no change *by construction*:
  a `SelfRef` head is not a `NamedVariable`, so `THIS` is never looked
  up as an undeclared symbol. Cover it with a test anyway.
- `rule_method_call_declared.rs` reports `MethodReceiver::SelfRef`
  receivers as not-implemented rather than skipping them — same rule:
  a silent skip would keep quietly passing once `THIS^.M()` is
  resolvable. It already tracks the enclosing function block, so
  wiring real resolution up later is cheap (#1406).

### Codegen

Add the not-implemented arms and stop:
`compile_expr.rs` (variable lowering), `compile_stmt.rs` (assignment
target and the `MethodCall` receiver), returning `Diagnostic::todo` the
way `StmtKind::MethodCall` does today (`compile_stmt.rs:438`). Analysis
already rejects the program with `P9999`, so these arms are a backstop
against a future analyzer change silently miscompiling.

### plc2plc (`compiler/plc2plc/src/renderer.rs`)

`visit_self_ref_variable` writes `THIS^` / `SUPER^` — with `write("^")`,
not `write_ws("^")`, so no space is introduced. Receiver rendering for
`MethodCall` follows the same split.

## Files

| File | Change |
|---|---|
| `compiler/parser/src/token.rs` | `This` / `Super` token types, display strings, keyword table entry |
| `compiler/parser/src/xform_demote_keywords.rs` | Two token types added to the existing `demote_oop` arm + doc comment |
| `compiler/parser/src/parser.rs` | `self_ref()` rule; head alternative in `symbolic_variable()`; receiver alternative in `method_invocation()` |
| `compiler/dsl/src/textual.rs` | `SelfRefKind`, `SelfRefVariable`, `SymbolicVariableKind::SelfRef`, `MethodReceiver`, `MethodCall.receiver` |
| `compiler/dsl/src/visitor.rs`, `fold.rs` | `dispatch!` entries; receiver walk |
| `compiler/dsl/src/extension.rs` consumers | `LanguageExtension` impl for `SelfRefVariable` |
| `compiler/analyzer/src/rule_unsupported_extension.rs` | `visit_self_ref_variable` → P9999 |
| `compiler/analyzer/src/xform_resolve_expr_types.rs` and the 7 other listed sites | Explicit not-implemented arms (no silent skips) |
| `compiler/analyzer/src/rule_method_call_declared.rs` | Skip `SelfRef` receivers; adapt to `receiver` field |
| `compiler/codegen/src/compile_expr.rs`, `compile_stmt.rs` | `Diagnostic::todo` arms |
| `compiler/plc2plc/src/renderer.rs` | Render `THIS^` / `SUPER^`; receiver rendering |
| `compiler/parser/src/tests/this_super.rs` (new) + `tests/mod.rs` | Parser tests |
| `compiler/plc2plc/src/tests/this_super.rs` (new) + `tests/mod.rs` | Round-trip tests |
| `compiler/codegen/tests/it/` | P9999-on-compile test |
| `docs/reference/language/object-orientation/this-and-super.rst` (new), `index.rst` | Reference page |
| `docs/explanation/object-orientation.rst` | Short section on self/base reference |

No new `--allow-*` flag, so no `options.rs`, no LSP `extract_compiler_options`,
and no `enabling-dialects-and-features.rst` / `ironplcc.rst` changes.

## Testing

Per `specs/steering/syntax-support-guide.md`, each leg asserts something
the others do not.

- **Parser** (`parser/src/tests/this_super.rs`) — AST shape:
  `THIS^.count := 1;` → `Structured{ record: SelfRef(This), field }`;
  `v := SUPER^.arr[2];`; `THIS^.M();` and `SUPER^.M(a := 1);` →
  `MethodCall` with a `SelfRef` receiver; `v := THIS^;` bare;
  `THIS ^.x := 1;` (the whitespace decision, asserted explicitly);
  `THIS.x := 1;` and bare `THIS := 1;` rejected with the flag on.
- **Parser, flag off** — `THIS`/`SUPER` usable as variable names
  (extend the existing keyword-safety test rather than writing a new
  one), and `THIS^.v := 1;` still parses as a deref of a variable named
  `THIS` — the no-regression case.
- **plc2plc** (`plc2plc/src/tests/this_super.rs`) — text → AST → text,
  including re-parsing the rendered output (as
  `plc2plc/src/tests/case.rs:33` does) so a stray space before `^`
  fails the test.
- **Analyzer** — a program using `THIS^`/`SUPER^` is rejected, and
  `P9999` is among the diagnostics. Assert *presence*, not an exact
  diagnostic set: the set is expected to be noisy and to change as
  passes are taught the construct, and pinning it would make every
  later improvement a test edit. Do assert that a program *without*
  `THIS^`/`SUPER^` is unaffected.
- **Codegen** — `compile` fails with `P9999`, like
  `codegen/tests/it/end_to_end_struct.rs:620`.
- **No end-to-end execution test** — nothing executes yet; the
  syntax-support-guide's execution-test requirement applies to syntax
  that produces executable code.
- `cd compiler && just` clean before the PR.

## Follow-ups (tracked separately, not part of this PR)

- **#1405** — diagnose `THIS` / `SUPER` written without the `^`, instead
  of the generic `P0002` syntax error.
- **#1406** — reject `SUPER^` where there is no base type, and
  `THIS^`/`SUPER^` outside a function block body.
- **#1404** — pre-existing plc2plc `ExprKind::Deref` rendering bug,
  unrelated to this feature.

## Tasks

- [x] `This` / `Super` tokens + demotion arm + token tests
- [x] `SelfRefVariable` / `SelfRefKind` / `MethodReceiver` AST + `dispatch!` entries
- [x] `self_ref()` rule; `symbolic_variable()` head; `method_invocation()` receiver
- [x] Parser tests (flag on and flag off, including the whitespace case) — `parser/src/tests/this_super.rs`, 17 tests
- [x] plc2plc rendering + re-parsing round-trip tests — `plc2plc/src/tests/this_super.rs`, 6 tests
- [x] `LanguageExtension` impl + `rule_unsupported_extension` override
- [x] Explicit not-implemented arms in the resolution transforms; analyzer test asserting rejection + P9999 present
- [x] Codegen `Diagnostic::todo` arms + compile-fails-with-P9999 test
- [x] Docs: reference page, `index.rst`, explanation page
- [x] `cd compiler && just` clean

### Notes from implementation (2026-08-22)

- The forced-match-arm count came in lower than the plan estimated: four
  sites in the analyzer (`rule_method_call_declared`, `rule_bit_access_range`,
  `xform_resolve_expr_types`, `xform_resolve_late_bound_expr_kind`), two in
  codegen (`compile_expr`), one in the LSP semantic-token map, and one in
  the renderer. The rest of the ~50 `SymbolicVariableKind` match sites
  already had wildcard arms landing in an error or recursion path.
- Two of the analyzer sites are `Option`-returning type resolvers, where a
  diagnostic cannot be returned from the helper itself. Those got the arm
  *plus* a loud guard at the pass's own entry point
  (`fold_self_ref_variable` / `visit_self_ref_variable`), so the pass
  reports and stops rather than proceeding on an unresolved type.
- Method bodies are not compiled at all yet, so a `THIS^` inside a
  `METHOD` never reaches codegen. The codegen test therefore puts the
  statement in the function block body; without that it passes
  vacuously.
- No array-subscript round-trip case: `THIS^.values[2]` renders as
  `THIS^.values [ 2 ]`, which does not re-parse. It is a pre-existing
  renderer bug affecting every subscript (`values [ 2 ]` alone fails the
  same way, with no dialect flags involved) — reported on issue #1404.
