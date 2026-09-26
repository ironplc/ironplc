# Report EXIT outside a loop from `check`

Issue: #1817

## Goal

`ironplcc check` reports P4021 (EXIT statement is not inside a loop) for the
programs `ironplcc compile` rejects with it, so the language server and the
VS Code extension show the error while editing.

## Architecture

P4021 is emitted only by code generation (`compile_stmt.rs`, when the loop
label stack is empty). `check` stops after semantic analysis, so it never sees
it. Add a semantic rule `rule_exit_inside_loop` that walks each statement list
tracking the loop depth and reports P4021 for an `EXIT` at depth 0.

The code generator keeps its own check: it is the invariant guard for the
label stack, and is no longer reachable from the CLI.

## Prefactoring

None needed. The rule is a new visitor that follows the existing rule shape
(`rule_support::run_rule`); no existing code has to change shape for it.

## Design doc reference

None; the rule is a standard IEC 61131-3 constraint (EXIT inside FOR, WHILE,
or REPEAT).

## File map

- `compiler/analyzer/src/rule_exit_inside_loop.rs` (new)
- `compiler/analyzer/src/lib.rs`, `compiler/analyzer/src/stages.rs` (register)
- `docs/compiler/problems/P4021.rst` (if it states where the check happens)

## Tasks

- [ ] Tests: EXIT at POU level fails with P4021; EXIT in FOR/WHILE/REPEAT
      passes; EXIT in IF inside a loop passes; EXIT after a loop fails
- [ ] Implement the rule and register it
- [ ] Run `cd compiler && just`
- [ ] Delete this plan
