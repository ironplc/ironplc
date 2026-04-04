# Fix: Global Struct Field Access (P9999)

## Problem

Accessing fields of global struct variables (e.g., `phys.T0`) fails with
P9999 "Not implemented at codegen". This affects 36+ OSCAT functions that
reference fields of global struct variables (C_TO_K, DEG_TO_DIR, FACT, BETA,
etc.).

## Root Cause

The `compile()` function in `compiler/codegen/src/compile.rs` only collects
global variables from `ConfigurationDeclaration.global_var`. Top-level
`VAR_GLOBAL` blocks, parsed as `LibraryElementKind::GlobalVarDeclarations`,
are completely ignored by codegen. This means top-level global struct variables
never get registered in `ctx.struct_vars`, causing any field access to fail.

## Changes

### 1. Collect top-level `GlobalVarDeclarations` in codegen

**File**: `compiler/codegen/src/compile.rs`

In the `compile()` function, add a loop to collect
`LibraryElementKind::GlobalVarDeclarations` from the library elements, feeding
them into the same `synthetic_globals` vector that already collects
configuration globals and system uptime globals.

### 2. Add end-to-end tests

**File**: `compiler/codegen/tests/end_to_end_global.rs`

- Top-level global struct field read from PROGRAM body
- Top-level global struct field read from FUNCTION body
- Top-level global scalar variable read from PROGRAM body
