# Method calls in expression position

Issue: [#1421](https://github.com/ironplc/ironplc/issues/1421).
Part of [#1692](https://github.com/ironplc/ironplc/issues/1692): a property
read is a `GET` call that returns a value, so the PROPERTY semantics build on
this.

## Goal

A method call with a return type can be used anywhere an expression is
legal: `v := m.GetSpeed();`, `IF m.IsReady() THEN`, `x := a + m.Value();`,
and as an argument to another call. The value the method returns reaches the
caller. Calling a method without a return type in an expression is rejected
with a new problem code.

## Design doc reference

- [ADR-0041](../adrs/0041-staged-method-and-interface-dispatch.md) Phase 1
  (static dispatch; unchanged)
- `docs/reference/language/object-orientation/method.rst`, "Current
  limitations" (updated in this PR)

## Decisions

- **One call rule, two positions.** The grammar gets one `method_call` rule
  producing a `MethodCall`. The statement form wraps it in
  `StmtKind::MethodCall` as today; the expression form wraps it in a new
  `ExprKind::MethodCall`. The receiver grammar stays in one place, so #1422
  (wider receivers) changes one rule for both positions.
- **Receivers unchanged:** a bare instance name, or `THIS^`/`SUPER^` (still
  rejected with P9999, #1406). `p^.M()` and `a[i].M()` are #1422.
- **Void method in an expression:** new problem code **P4057**
  `MethodCallWithoutReturnValue`, "Method has no return value and cannot be
  used in an expression", with its docs page. A call in statement position to
  a method *with* a return type stays legal and discards the value, as in
  CODESYS/TwinCAT.
- **Codegen:** `METHOD_CALL` leaves the instance reference with the return
  value on top. The statement form pops both (as today); the expression form
  emits `SWAP; POP` and keeps the value. The verifier checks `SWAP` only for
  depth (2 in, 2 out), so no verifier change.
- **Unchanged limitations:** a STRING/WSTRING return type still reports P9999
  (as for a statement call today), and a call that resolves to an inherited
  method through `EXTENDS` is still not compiled (#1438).
- **Side effect worth noting:** a library-qualified call such as
  `Tc2_Standard.LEN(s)` has the same shape as a method call. Today it is a
  P0002; after this PR it parses as a method call on an instance named
  `Tc2_Standard` and fails in analysis instead. Not a regression, and no
  support for qualified calls is added here.

## Architecture

- **AST (`dsl`):** `ExprKind::MethodCall(MethodCall)`, reusing the existing
  struct. Visitor/fold entries as needed.
- **Parser:** `method_call()` rule; `method_invocation()` (statement) and a
  new `primary_expression` alternative both use it. The expression alternative
  goes before `variable()`, since `m.GetSpeed` is a valid prefix of a
  structured-variable parse.
- **Analyzer:**
  - `xform_resolve_expr_types`: the type of an `ExprKind::MethodCall` is the
    resolved method's return type (via `callee_resolution`).
  - `rule_method_call_declared` already visits every `MethodCall`, so arity
    and name checks apply to expression calls unchanged. It gains the P4057
    check for an expression call to a method without a return type.
  - Any other pass that matches `ExprKind` exhaustively gets an arm; the
    compiler lists them.
- **Codegen:** the shared part of `compile_method_call` (resolve, push
  instance, arguments, `METHOD_CALL`) is reused by the statement form (pop
  both) and a new `ExprKind::MethodCall` arm in `compile_expr` (`SWAP; POP`).
- **plc2plc:** renders the call without the trailing `;` in expression
  position.

## Prefactoring

1. **Move `compile_method_call` out of `codegen/src/compile_stmt.rs`**
   (1341 lines, over the limit) into `codegen/src/compile_method.rs`, next to
   the method body compilation, and split it into the call itself and the
   statement-position stack cleanup. Behaviour-preserving, own commit.
2. **Renderer:** `visit_method_call` writes the trailing `;` and newline
   itself. Move those to where the statement is rendered, so the same visitor
   renders the call in an expression. Behaviour-preserving (golden files
   unchanged), own commit.

Not prefactored: `parser.rs` and `compile_expr.rs` are over the limit too,
for the same reason as in the PROPERTY PR (one `peg` macro; splitting
`compile_expr.rs` is far larger than this change). This PR adds a few lines to
each.

## File map

Created:

- `compiler/parser/src/tests/method_call_expression.rs`
- `compiler/plc2plc/src/tests/method_call_expression.rs`
- `compiler/codegen/tests/it/end_to_end_method_call_expression.rs`
- `docs/reference/compiler/problems/P4057.rst`

Modified:

- `compiler/dsl/src/textual.rs` (+ visitor/fold if needed)
- `compiler/parser/src/parser.rs`
- `compiler/analyzer/src/xform_resolve_expr_types.rs`,
  `rule_method_call_declared.rs`, and any exhaustive `ExprKind` match
- `compiler/codegen/src/compile_stmt.rs`, `compile_method.rs`,
  `compile_expr.rs`
- `compiler/plc2plc/src/renderer.rs`
- `compiler/problems/resources/problem-codes.csv`
- `docs/reference/language/object-orientation/method.rst`

## Tasks

- [ ] Commit this plan
- [ ] Prefactor 1: move and split `compile_method_call`; all tests pass unchanged
- [ ] Prefactor 2: trailing `;` out of the renderer's `visit_method_call`; all tests pass unchanged
- [ ] `ExprKind::MethodCall`, shared `method_call` rule, expression alternative
- [ ] Parser tests: AST shape in assignment, `IF` condition, binary operand, call argument; `m.x` field read still a variable
- [ ] Analyzer: expression type from the return type; P4057 with its docs page; tests
- [ ] Codegen: expression arm with `SWAP; POP`; end-to-end tests (assignment, `IF`, arithmetic, nested in a call argument, argument passing, a method that also mutates the instance, widening of the return value into a wider target)
- [ ] plc2plc round trips that re-parse
- [ ] `method.rst`: remove the "return value is discarded" limitation
- [ ] Check against the brotlib TwinCAT code
- [ ] Open issues for anything this plan names but doesn't deliver
- [ ] `git rm` this plan
- [ ] `cd compiler && just`
