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
- `VAR_OUTPUT` parameters on user-defined functions (`VAR_IN_OUT` was added later; see [VAR_IN_OUT parameters](#var_in_out-parameters))

## Key Decisions

### Flat variable table (no stack frames)

IEC 61131-3 prohibits recursive function calls. This means each function has exactly one activation at a time, so function locals can be statically allocated in the shared variable table. No stack frame save/restore is needed. The VM's existing `VariableScope` mechanism provides scoped access to a region of the variable table.

See [ADR-0046](../../specs/adrs/0046-flat-variable-table-for-function-calls.md).

### Exact type matching for arguments

The type-checking rule requires exact type matches between arguments and parameters. `INT` argument for `INT` parameter passes; `INT` argument for `DINT` parameter fails. Users must use explicit conversion functions (e.g., `INT_TO_DINT`). This avoids implicit conversion complexity and matches the project's safety-first design principle.

See [ADR-0047](../../specs/adrs/0047-exact-type-matching-for-function-arguments.md).

### CALL opcode (not inlining)

User-defined function calls use the `CALL` opcode (0x84), already specified in the bytecode instruction set. Each function body is compiled once into the container. This avoids code bloat from inlining and cleanly models the calling semantics.

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
3. For each function, compile into its own region of the flat variable table (ADR-0046), based at the `var_offset` codegen assigns it. Indices in the emitted bytecode are absolute, not region-relative:
   - Parameters occupy `var_offset .. var_offset + num_params`, in declaration order
   - Local variables (VAR) occupy the slots after them
   - The return variable (same name as function, per IEC 61131-3) occupies the last slot of the region
   - Body is compiled, then `LOAD_VAR <return_slot>` + `RET` is emitted at the end
4. Add each function to the container via `ContainerBuilder::add_function`.

**Call site compilation:**

1. Look up function name in the name→ID mapping
2. Compile each positional argument using the parameter's resolved type for opcode selection
3. Emit `CALL func_id, var_offset` — `var_offset` is the base of the callee's region in the flat variable table (ADR-0046)
4. Return value is on the stack for the caller to use

### Container

**`FuncEntry` change:** Add `num_params: u16` field. The CALL opcode handler uses this to know how many values to pop from the operand stack into the function's parameter variable slots. For init/scan functions, `num_params` is 0. The entry's full wire layout is in [Bytecode Container Format](bytecode-container-format.md) (REQ-CF-container-022).

### VM

**`CALL` opcode (0x84)** — Operands: `u16` function ID, `u16` variable offset

1. Look up `FuncEntry` by function ID (code_offset, code_length, num_locals, num_params)
2. Build the callee's variable scope from the `var_offset` operand — the base of its region in the flat variable table (ADR-0046) — spanning num_locals slots. The scope bounds which indices the callee may touch (its own region plus the shared globals); it does not rebase them, so the body's operands are absolute indices
3. Pop num_params values from the operand stack into the function's parameter slots (`var_offset .. var_offset + num_params`), in reverse order, so the leftmost argument lands in the lowest slot
4. Push a call frame for the callee onto the frame stack and continue executing at the function's bytecode; exceeding the container's declared call depth traps `V9012 CallStackOverflow`
5. When the callee returns, the return value is on the operand stack

**`RET` opcode (0x88)** — No operands

1. Pop the callee's frame and resume the caller. The top of the operand stack holds the return value, which remains on the stack for the caller.

### VAR_IN_OUT parameters

A `VAR_IN_OUT` parameter is passed by reference, as an implicit `REF_TO` that
is never null and never reassigned (see [REF_TO design](ref-to.md)). It takes
an argument like a `VAR_INPUT` does: the argument list is the `VAR_INPUT` and
`VAR_IN_OUT` declarations in declaration order, and `CALL` pops one slot per
parameter into that order.

**Analysis.** `FunctionSignature::input_parameters` lists both kinds, so the
arity check (P4018) and argument binding agree with the named-to-positional
rewrite. `rule_function_call_in_out_argument` requires a `VAR_IN_OUT` argument
to be a variable (P4057) of exactly the parameter's elementary type (P4058):
the function writes values of the parameter's type into the caller's variable,
so no implicit conversion is sound. P4026 does not check `VAR_IN_OUT`
arguments.

**Call site.** The caller pushes the argument variable's table index (what
`REF(x)` pushes) instead of its value. An argument that is itself a
`VAR_IN_OUT` parameter of the calling function forwards the reference its
slot holds.

**Callee.** The parameter's slot holds the reference. A read compiles to
`LOAD_VAR_I64 slot; LOAD_INDIRECT`, a write to `<value>; <truncate>;
LOAD_VAR_I64 slot; STORE_INDIRECT`, and `REF(param)` to `LOAD_VAR_I64 slot`.
`CompileContext::in_out_params` names the parameters; `CompileContext::var_index`
refuses them, so a site that would load or store the slot directly is reported
as not implemented rather than compiled wrongly.

**VM.** The referenced variable lives in a caller's frame (a program variable,
a function block's fields, or a calling function's locals), outside the
callee's own scope. `LOAD_INDIRECT`/`STORE_INDIRECT` therefore check the
target against the program instance's scope, not the current frame's.

**Not yet supported** (reported as not implemented by codegen): parameters of
a non-elementary type (strings, arrays, structures, references, function block
instances), arguments that are array elements or structure fields, and a
`VAR_IN_OUT` parameter used as a `FOR` control variable, a bit or partial
access write target, or a dereference assignment target. `VAR_IN_OUT` on
function blocks and methods is still passed by value.

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
- Function 1 (scan): `LOAD_CONST 3`, `LOAD_CONST 7`, `CALL 2, 1`, `STORE_VAR result`, `RET_VOID`
- Function 2 (ADD_INTS): num_params=2, num_locals=3, var_offset=1 (slots 1-3: `A`, `B`, `ADD_INTS`). `LOAD_VAR 1`, `LOAD_VAR 2`, `ADD_I32`, `STORE_VAR 3`, `LOAD_VAR 3`, `RET`

**VM execution of `CALL 2, 1`:**
1. Look up function 2: num_params=2, num_locals=3
2. Build the callee's scope over slots 1-3 — the num_locals=3 region based at the `var_offset` operand (1)
3. Pop 7 → slot 2 (B), pop 3 → slot 1 (A)
4. Execute function 2: A + B = 10, store to slot 3, load slot 3, RET
5. Return value 10 on stack, pop the callee's frame
6. Caller stores 10 into `result`
