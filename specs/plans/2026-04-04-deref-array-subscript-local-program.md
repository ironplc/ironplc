# Plan: Implement Deref Array Subscript for Local/Program Variables

## Context

Expressions like `pt^[i]` where `pt` is `REF_TO ARRAY` fail with P9999 "not implemented"
during codegen when the variable is a local or program variable. The error occurs at
`compile_array.rs:138` when `ctx.array_vars.get(&named.name)` returns `None`.

**Root cause:** `REF_TO ARRAY` variables declared as function parameters (VAR_INPUT)
correctly register array metadata in `ctx.array_vars` (compile.rs:590-656), but local
variables in functions (compile.rs:705-714) and program variables (compile.rs:1340-1350)
do NOT — they only register the type as W64/Unsigned.

Everything else already works: `resolve_access()` produces `DerefArrayElement`, bytecode
opcodes exist (`LOAD_ARRAY_DEREF`/`STORE_ARRAY_DEREF`), and dispatch sites handle the
variant. Only metadata registration is missing.

## Changes

### 1. Extract helper in `compile_array.rs`

Add `register_ref_to_array_metadata()` to encapsulate the REF_TO ARRAY metadata
registration logic currently inlined at compile.rs:604-656, eliminating triplication.

### 2. Refactor compile.rs

- Parameter path (~line 604): replace inline logic with helper call
- Function local variables (~line 705): bind `ref_init`, add helper call
- Program variables (~line 1340): bind `ref_init`, add helper call

### 3. Add end-to-end tests

Two tests in `compiler/codegen/tests/end_to_end_ref.rs`:
1. Program with local REF_TO ARRAY variable + deref subscript
2. Function with local REF_TO ARRAY variable + deref subscript
