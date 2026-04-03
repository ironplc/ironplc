# Fix: POU-local VAR CONSTANT used as array bound (P4030)

## Problem

When a `VAR CONSTANT` is declared inside a `FUNCTION_BLOCK`, `FUNCTION`, or
`PROGRAM` and used as an array dimension bound (or STRING length), the compiler
emits P4030 "Constant referenced in type parameter is not defined". The
`collect_constants()` function in `xform_resolve_constant_expressions.rs` only
collects constants from `GlobalVarDeclarations` and `ConfigurationDeclaration`,
ignoring POU-local `VAR CONSTANT` blocks.

This pattern is used in OSCAT for configurable-size FIFO and STACK function
blocks where the array size is a local constant.

## Solution: Scoped Resolution

Override `fold_function_block_declaration`, `fold_function_declaration`, and
`fold_program_declaration` on `ConstantResolver` to collect POU-local constants
before recursing into the POU's children, then restore the previous scope
afterward. This ensures local constants are only visible within their
declaring POU.

## Changes

**File:** `compiler/analyzer/src/xform_resolve_constant_expressions.rs`

1. Add three `fold_*` overrides using save/collect/recurse/restore pattern
2. Add `ProgramDeclaration` arm to `find_var_decl` test helper
3. Add 5 new tests covering FB, FUNCTION, PROGRAM local constants, scoping
   isolation, and non-constant rejection
