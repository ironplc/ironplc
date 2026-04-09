# User-Defined Function Calls Design

Date: 2026-03-12

## Goal

Enable compilation and execution of user-defined IEC 61131-3 functions. A user-defined function (declared with `FUNCTION ... END_FUNCTION`) can be called from programs, function blocks, or other functions. This adds a type-checking analysis rule, extends codegen to compile function bodies and call sites, and implements the `CALL`/`RET` opcodes in the VM.

## Scope

- Analysis rule to verify argument types match parameter types (exact match, no implicit widening)
- Analysis rule to verify return type matches assignment destination
- Pass `FunctionEnvironment` and `TypeEnvironment` from analyzer to codegen
- Compile user-defined function bodies as separate bytecode functions in the container
- Emit `CALL`/`RET` opcodes at call sites and function returns
- VM implementation of `CALL` (save scope, set up function variable region, execute) and `RET` (return value on stack)
- Add `num_params` to `FuncEntry` so the VM knows how many arguments to pop
- Debug section entries for function names and scoped variable names

### Out of scope

- Named arguments (a separate transformation normalizes these to positional before codegen)
- `ANY_*` type parameters (stdlib functions only)
- Implicit type widening/coercion
- Recursive call detection (IEC 61131-3 forbids recursion, but we don't enforce this yet)
- `VAR_OUTPUT` and `VAR_IN_OUT` parameters on user-defined functions

## Key Decisions

### Flat variable table (no stack frames)

IEC 61131-3 prohibits recursive function calls. This means each function has exactly one activation at a time, so function locals can be statically allocated in the shared variable table. No stack frame save/restore is needed. The VM's existing `VariableScope` mechanism provides scoped access to a region of the variable table.

See [ADR-0021](../../specs/adrs/0021-flat-variable-table-for-function-calls.md).

### Exact type matching for arguments

The type-checking rule requires exact type matches between arguments and parameters. `INT` argument for `INT` parameter passes; `INT` argument for `DINT` parameter fails. Users must use explicit conversion functions (e.g., `INT_TO_DINT`). This avoids implicit conversion complexity and matches the project's safety-first design principle.

See [ADR-0022](../../specs/adrs/0022-exact-type-matching-for-function-arguments.md).

### CALL opcode (not inlining)

User-defined function calls use the `CALL` opcode (0xB3), already specified in the bytecode instruction set. Each function body is compiled once into the container. This avoids code bloat from inlining and cleanly models the calling semantics.

## Architecture

### Analysis

**New rule: `rule_function_call_type_check`**

Validates function calls in two ways:

1. **Argument type matching** — For each positional argument, compares the expression's `resolved_type` (set by `xform_resolve_expr_types`) against the corresponding parameter's declared type from `FunctionSignature`. Exact match required. Skips stdlib functions (which use `ANY_*` types).

2. **Return type matching** — The function call expression's `resolved_type` is already set to the function's return type by `xform_resolve_expr_types`. Assignment compatibility is verified by comparing this against the destination variable's type.

New problem codes: `FunctionCallArgTypeMismatch`, `FunctionCallReturnTypeMismatch`.

### Codegen

**Signature change:** `compile(library: &Library)` becomes `compile(library: &Library, functions: &FunctionEnvironment, types: &TypeEnvironment)`.

- `FunctionEnvironment` — to distinguish user-defined from stdlib functions and look up parameter types for opcode selection
- `TypeEnvironment` — to resolve type aliases to elementary types for correct opcode selection

**Function compilation:**

1. Iterate `FunctionEnvironment` for non-stdlib functions. Find matching `FunctionDeclaration` in the library.
2. Assign function IDs starting at 2 (0 = init, 1 = scan). Store name→ID mapping in `CompileContext`.
3. For each function, compile independently with variable indices starting at 0:
   - Parameters occupy slots 0..num_params-1
   - Local variables occupy subsequent slots
   - The return variable (same name as function, per IEC 61131-3) occupies one of these slots
   - Body is compiled, then `LOAD_VAR <return_slot>` + `RET` is emitted at the end
4. Add each function to the container via `ContainerBuilder::add_function`.

**Call site compilation:**

1. Look up function name in the name→ID mapping
2. Compile each positional argument using the parameter's resolved type for opcode selection
3. Emit `CALL func_id`
4. Return value is on the stack for the caller to use

### Container

**`FuncEntry` change:** Add `num_params: u16` field. The CALL opcode handler uses this to know how many values to pop from the operand stack into the function's parameter variable slots. For init/scan functions, `num_params` is 0.

### VM

**`CALL` opcode (0xB3)** — Operand: `u16` function ID

1. Look up `FuncEntry` by function ID (bytecode, num_locals, num_params)
2. Allocate a variable scope region for the function (num_locals slots)
3. Pop num_params values from the operand stack into the function's parameter slots (slots 0..num_params-1), in reverse order (last arg popped first into highest param slot)
4. Recursively call `execute()` with the function's bytecode and new scope
5. After `execute()` returns, the return value is on the operand stack

**`RET` opcode (0xB4)** — No operands

1. Return from `execute()`. The top of the operand stack holds the return value, which remains on the stack for the caller.

### Debug Section

- Add `FuncNameEntry` for each user-defined function (function ID → function name)
- `VarNameEntry` already has a `function_id` field — use it to scope function parameter and local variable names to their owning function ID

## End-to-End Example

```iec
FUNCTION ADD_INTS : INT
VAR_INPUT
    A : INT;
    B : INT;
END_VAR
    ADD_INTS := A + B;
END_FUNCTION

PROGRAM main
VAR
    result : INT;
END_VAR
    result := ADD_INTS(3, 7);
END_PROGRAM
```

**Analysis:**
1. `FunctionEnvironment` registers `ADD_INTS`: return type `INT`, params `[A: INT, B: INT]`
2. `xform_resolve_expr_types` sets `resolved_type = INT` on `ADD_INTS(3, 7)`
3. `rule_function_call_declared` validates arg count (2 == 2)
4. `rule_function_call_type_check` validates: arg 0 `INT` == param A `INT`, arg 1 `INT` == param B `INT`, return `INT` == destination `result` `INT`

**Container layout:**
- Function 0 (init): program variable initializers, `RET_VOID`
- Function 1 (scan): `LOAD_CONST 3`, `LOAD_CONST 7`, `CALL 2`, `STORE_VAR result`, `RET_VOID`
- Function 2 (ADD_INTS): num_params=2, num_locals=3. `LOAD_VAR 0`, `LOAD_VAR 1`, `ADD_I32`, `STORE_VAR 2`, `LOAD_VAR 2`, `RET`

**VM execution of `CALL 2`:**
1. Look up function 2: num_params=2, num_locals=3
2. Allocate variable scope at next available region
3. Pop 7 → slot 1 (B), pop 3 → slot 0 (A)
4. Execute function 2: A + B = 10, store to slot 2, load slot 2, RET
5. Return value 10 on stack, restore caller scope
6. Caller stores 10 into `result`
