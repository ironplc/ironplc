# Short-Circuit Operator Codegen (`AND_THEN` / `OR_ELSE`)

Implements [issue #1476](https://github.com/ironplc/ironplc/issues/1476).

## Goal

`ironplcc compile` emits correct short-circuit bytecode for `AND_THEN` and
`OR_ELSE`, so a program that `ironplcc check` accepts can also be compiled and
run. The right operand must not be evaluated when the left operand already
decides the result — that is the whole point of the operators, since their
motivating use is guarding a dereference:

```st
IF (ptr <> 0 AND_THEN ptr^ = 99) THEN
```

`OR_ELSE` is the dual of `AND_THEN`. It is described in the design document and
listed in its keyword table, but nothing in the compiler implements it today —
it is not a token, not a `CompareOp` variant, and not in the grammar. So this
change adds `OR_ELSE` through the whole pipeline and gives both operators
codegen.

## Design doc reference

`specs/design/beckhoff-twincat-dialect.md` §3.4 "Short-Circuit Boolean
Operators" and the keyword table in §"Keyword Tokens". §3.4 needs updating: it
says evaluation order "is a code generation concern" but does not record what
codegen does, and codegen currently refuses.

## Architecture

Both operators lower to the same shape — evaluate the left operand, branch on
it, and let one arm evaluate the right operand while the other materialises the
constant that the left operand already forced:

```text
    <left>                  ; push left
    JMP_IF_NOT alt          ; pops left
    <then-arm>              ; AND_THEN: <right>   OR_ELSE: LOAD_TRUE
    JMP end
alt:
    <else-arm>              ; AND_THEN: LOAD_FALSE  OR_ELSE: <right>
end:
```

One emitter routine covers both; the operator only selects which arm evaluates
the right operand. This is the conditional-branch-over-an-expression-region
pattern the loop and `IF` codegen already use (`create_label` / `emit_jmp_if_not`
/ `bind_label`).

### Stack-depth bookkeeping

`Emitter` tracks operand-stack depth in *emission* order, so it would count both
arms as if they ran back to back and over-report `max_stack_depth` by one per
short-circuit expression. `IF` and `CASE` do not hit this because statement
bodies are stack-neutral; an expression arm is not. The emitter gains a
`reset_stack_depth` so the second arm starts from the depth the first arm
started from. The shipped bytecode is verified independently by
`ironplc_container::verify_stack_balance`, which walks the real control-flow
graph.

### Non-`BOOL` operands

`AND_THEN` / `OR_ELSE` are `BOOL`-only operators. The AST allows any operand
type (the analyzer types them like `AND`/`OR`, which are also bitwise), and
"skip the right operand" has no meaning for a bit-string result: short-circuit
lowering of `2#1010 AND_THEN 2#0110` would yield `2#0110`, not `2#0010`. So
codegen short-circuits only when both operands resolve to `BOOL`, and otherwise
emits the same eager bitwise op `AND`/`OR` emits. No path silently changes a
`BOOL` program's meaning, which is what the current refusal exists to prevent.

## Prefactoring

Adding one operator variant currently means editing two independent tables that
already say the same thing, and dropping a second branchy block into the middle
of `compile_expr`'s 1900-line `match`. Two prefactorings land first, each in its
own commit and each behaviour-preserving:

1. **One spelling of `CompareOp`.** `plc2plc/src/renderer.rs`
   `visit_compare_expr` duplicates the operator→text table in `CompareOp`'s
   `Display`. Give `CompareOp` an `as_str` that both use, so a new operator is
   spelled once.
2. **Extract the comparison arm of `compile_expr`.** The `ExprKind::Compare`
   arm becomes `compile_compare`, giving the short-circuit path an obvious home
   and an early return instead of another nested block.

## File map

Prefactoring:

- `compiler/dsl/src/textual.rs` — `CompareOp::as_str`, used by `Display`
- `compiler/plc2plc/src/renderer.rs` — render via `as_str`
- `compiler/codegen/src/compile_expr.rs` — extract `compile_compare`

`OR_ELSE` through the pipeline:

- `compiler/parser/src/token.rs` — `TokenType::OrElse`
- `compiler/parser/src/xform_demote_keywords.rs` — demote with `AndThen`
- `compiler/parser/src/parser.rs` — grammar rule at `OR` precedence
- `compiler/parser/src/options.rs` — flag description covers both operators
- `compiler/dsl/src/textual.rs` — `CompareOp::OrElse`
- `compiler/analyzer/src/xform_resolve_expr_types.rs`,
  `compiler/analyzer/src/rule_ref_to.rs` — treat like `AndThen`
- `compiler/ironplc-cli/src/lsp_project.rs` — operator semantic token
- `compiler/ironplc-cli/bin/main.rs` — CLI flag doc comment

Codegen:

- `compiler/codegen/src/emit.rs` — `stack_depth` / `reset_stack_depth`
- `compiler/codegen/src/compile_short_circuit.rs` — new module
- `compiler/codegen/src/compile_expr.rs` — dispatch to it
- `compiler/codegen/src/lib.rs` — register the module

Tests:

- `compiler/parser/src/tests/short_circuit.rs`
- `compiler/plc2plc/src/tests/short_circuit.rs`
- `compiler/codegen/tests/it/compile_bool.rs` — replace the pin on the refusal
- `compiler/codegen/tests/it/end_to_end_short_circuit.rs` — new

Docs:

- `docs/explanation/enabling-dialects-and-features.rst`
- `docs/reference/compiler/ironplcc.rst`
- `specs/design/beckhoff-twincat-dialect.md` §3.4

## Tasks

- [ ] Prefactor 1: `CompareOp::as_str` shared by `Display` and the renderer
- [ ] Prefactor 2: extract `compile_compare` out of `compile_expr`
- [ ] Add `OR_ELSE`: token, demotion, grammar, `CompareOp::OrElse`, analyzer,
      renderer, LSP
- [ ] Emitter: `stack_depth` / `reset_stack_depth` for branch merges
- [ ] `compile_short_circuit`: emit the branch form for `BOOL` operands, eager
      bitwise otherwise
- [ ] Parser tests for `OR_ELSE` AST shape and demotion
- [ ] plc2plc round-trip tests for `OR_ELSE`
- [ ] Codegen `compile_*` tests: emitted shape skips the right operand
- [ ] End-to-end tests: both operators, both truth values, guarded dereference
- [ ] Docs and design doc updated
- [ ] `cd compiler && just` passes
- [ ] `git rm` this plan
