# The CONTINUE statement

Issue: #1819

## Goal

Accept the `CONTINUE` statement of IEC 61131-3 Edition 3 and run it: inside
a `FOR`, `WHILE` or `REPEAT` loop, `CONTINUE` skips the rest of the body of
the innermost loop and goes on with its next iteration.

```
FOR i := 1 TO 10 DO
  IF i MOD 2 = 0 THEN
    CONTINUE;
  END_IF;
  odd := odd + 1;
END_FOR;
```

Today `ironplcc check --dialect iec61131-3-ed3` reports P0002 (syntax error)
on it with `ironplcc` 0.246.0 and on `main` at `12162d75`. `EXIT` in the same
place is accepted.

## Non-goals

- Accepting `CONTINUE` under the default dialect (Edition 2), where it is not
  a keyword and stays a valid identifier.

## Gating

`CONTINUE` is new in Edition 3, so it is gated like the other Edition 3
syntax: a new flag `allow_continue` (`--allow-continue`), enabled by the
`iec61131-3-ed3`, `rusty`, `codesys` and `twincat` dialects (all three
vendors document the statement). The lexer always produces
`TokenType::Continue`; `xform_demote_keywords` demotes it to an identifier
when the flag is off, so an Edition 2 program may still name a variable
`continue`. Per ADR-0040 the grammar stays option-free.

## Architecture

- **Lexer** (`parser/src/token.rs`): `TokenType::Continue`, its display name
  and its keyword-table row, next to `Exit`.
- **Token transforms**: the demotion arm in `xform_demote_keywords.rs`, and
  `Continue` in the list of tokens that start a statement in a `CASE` branch
  (`xform_tokens.rs`, beside `Exit`).
- **Parser** (`parser/src/parser.rs`): `continue_statement()` beside
  `exit_statement()`, an alternative of `iteration_statement()`.
- **AST** (`dsl/src/textual.rs`): `StmtKind::Continue(SourceSpan)`, with its
  span in `StmtKind::span()`.
- **Analyzer**: `rule_exit_inside_loop` generalises to both statements
  (renamed `rule_loop_control_inside_loop`); `CONTINUE` outside a loop is a
  new problem, P4065 `ContinueOutsideLoop`. (P4062 to P4064 are taken by
  open PRs #1885 and #1860.)
- **Code generation**: the stack of loop exit labels becomes a stack of
  `LoopLabels { exit, next }`. `CONTINUE` jumps to `next`:
  - `FOR`: the increment of the control variable, before the jump back to
    the head test;
  - `WHILE`: the condition test (the back-edge `CMP_BR` on the fused path,
    the loop head on the fallback path);
  - `REPEAT`: the `UNTIL` test.
  Outside a loop, code generation reports P4065 as it does P4021 for `EXIT`.
- **plc2plc** (`plc2plc/src/renderer.rs`): renders `CONTINUE;`.
- **Editor**: `CONTINUE` is a keyword for the semantic tokens of the language
  server (`ironplc-cli/src/semantic_tokens.rs`).
- **Flag plumbing**: `define_compiler_options!`, the CLI `FileArgs` overlay,
  `mcp/src/feature_flag_conformance.rs`, and the docs of the flag.

## Prefactoring

`compiler/codegen/src/compile_stmt.rs` is 1247 lines on `main`, over the
1000-line limit, and the change adds to its loop functions. The prefactor,
in its own commit and without changing behaviour: move the loop
compilation (`compile_while`, `compile_repeat`, `compile_for` and the helpers
only they use: `StepSign`, `try_constant_sign`, `try_constant_i64`,
`narrow_type_range`, `for_loop_trunc_can_be_elided`, `try_classify_for_head`)
into a new module `compiler/codegen/src/compile_loop.rs`. That brings
`compile_stmt.rs` under the limit, and the `CONTINUE` change then touches the
new module only.

## Tests

- Parser (`parser/src/tests/continue_statement.rs`): the AST shape of
  `CONTINUE` in each loop kind; with the default dialect, `CONTINUE;` is a
  syntax error and `continue` is an ordinary variable name.
- plc2plc (`plc2plc/src/tests/continue_statement.rs`): the round trip.
- Analyzer: `CONTINUE` in a program body, in an `IF` outside a loop and in a
  function reports P4065; inside each loop kind and nested loops it passes.
- Code generation `end_to_end_continue.rs`: `CONTINUE` in `FOR` (constant
  and negative step, and the narrow-type path), in `WHILE` (fused and
  fallback conditions), in `REPEAT` (fused and fallback `UNTIL`), in nested
  loops (only the innermost continues), and `CONTINUE` and `EXIT` in the
  same loop.
- LSP semantic tokens: `CONTINUE` is a keyword.

## Documentation

- `docs/reference/language/structured-text/continue.rst`, in the format of
  `exit.rst`, with the Edition 3 note and a playground example under
  `:dialect: iec61131-3-ed3`; the index entry; "See Also" links from
  `exit.rst`, `for.rst`, `while.rst` and `repeat.rst`.
- `docs/reference/compiler/problems/P4065.rst`.
- The flag in `docs/reference/compiler/ironplcc.rst` and
  `docs/explanation/enabling-dialects-and-features.rst`.

## Tasks

- [ ] Prefactor: move loop compilation into `compile_loop.rs`
- [ ] Tests first (parser, plc2plc, analyzer, end to end)
- [ ] Token, demotion, grammar, AST
- [ ] Analyzer rule and P4065
- [ ] Code generation
- [ ] Renderer, semantic tokens, flag plumbing
- [ ] Documentation
- [ ] `cd compiler && just`; docs build without warnings
- [ ] Remove this plan
