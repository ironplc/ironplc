# VAR_IN_OUT Parameters

## Overview

A `VAR_IN_OUT` parameter lets a POU read and write a variable that belongs to
its caller. IEC 61131-3 defines it as pass-by-reference: the argument must be
a variable, and an assignment to the parameter inside the POU is an assignment
to the caller's variable.

```
FUNCTION AddTo : DINT
VAR_INPUT  amount : DINT; END_VAR
VAR_IN_OUT total  : DINT; END_VAR
    total := total + amount;
    AddTo := total;
END_FUNCTION

PROGRAM main
VAR sum : DINT := 100; result : DINT; END_VAR
    result := AddTo(amount := 42, total := sum);   (* sum is now 142 *)
END_PROGRAM
```

IronPLC already has references: `REF_TO` stores a variable-table index and
dereferences it with `LOAD_INDIRECT`/`STORE_INDIRECT`
([REF_TO design](ref-to.md)). This design compiles a `VAR_IN_OUT` parameter as
an **implicit reference**: a `REF_TO` the caller binds when it calls, that is
never `NULL` and that the POU cannot reassign. Nothing new is added to the
container format or the instruction set.

Related documents:

- [REF_TO design](ref-to.md) — reference representation and runtime safety
- [User-defined function calls](user-defined-function-calls-design.md) — the
  `CALL` convention and the flat variable table
- [Bytecode instruction set](bytecode-instruction-set.md) — reference
  operations, including a `VAR_IN_OUT` bytecode example
- [ADR-0046](../adrs/0046-flat-variable-table-for-function-calls.md) — one
  statically allocated variable region per function

## Scope

**In scope:** `VAR_IN_OUT` parameters of user `FUNCTION`s whose type is
elementary (integers, reals, bit strings, `BOOL`, time and date types), with a
named variable as the argument.

**Designed here, built later** (see [Extensions](#extensions)):

- `VAR_IN_OUT` on function blocks and methods
- non-elementary `VAR_IN_OUT` types: `STRING`/`WSTRING`, arrays, structures,
  `REF_TO`, function block instances
- array elements and structure fields as arguments

## Semantics

| Property | Rule |
|---|---|
| Argument | Must be a variable. A literal or an expression has no storage to write to. |
| Argument type | Must be the parameter's type exactly (an alias of it counts as the same type). No implicit conversion applies. |
| Binding | Fixed for the duration of the call; the parameter cannot be made to refer to another variable. |
| Aliasing | Two `VAR_IN_OUT` parameters may refer to the same variable, and a parameter may alias a global the POU also names. A write through one is seen through the other immediately. |
| Visibility | A write is visible to the caller the moment it happens, not only when the call returns. |

The last two rows are why this is a reference and not copy-in/copy-out.
With copy-in/copy-out, `BOTH(x, x)` below would leave `x` at either 1 or 10,
depending on the order the copies are written back. By reference, `x` is 11.

```
FUNCTION BOTH : DINT
VAR_IN_OUT a : DINT; b : DINT; END_VAR
    a := a + 1;
    b := b + 10;
    BOTH := a;       (* 11: b's write is visible through a *)
END_FUNCTION
```

Exact type matching is a safety requirement, not a style choice. The POU
writes values of the parameter's type into the caller's variable. If an `INT`
variable could be bound to a `DINT` parameter, the POU could store 100000 into
it: a value the `INT` cannot hold, which later `INT` arithmetic would
misinterpret.

## Semantic Analysis

### Argument list

A call's argument list is the `VAR_INPUT` and `VAR_IN_OUT` declarations, in
declaration order. `FunctionSignature::input_parameters` and
`input_parameter_count` iterate over parameters that are
`is_input_compatible()`, the same list `xform_named_to_positional_args` orders
named arguments by. So the arity check (P4018), positional binding and the
named-to-positional rewrite all agree, whichever order the two kinds of
parameter are declared in.

### Argument rules

`rule_function_call_in_out_argument` checks each argument bound to a
`VAR_IN_OUT` parameter:

| Code | Condition |
|---|---|
| P4057 `FunctionCallInOutArgNotVariable` | The argument is not a variable: a literal, an arithmetic expression, a function call. |
| P4058 `FunctionCallInOutArgTypeMismatch` | The parameter's type resolves to an elementary type, and the argument's type does not resolve to that same elementary type. This includes arguments of non-elementary type, such as an enumeration, a structure or a reference. |

Both sides are resolved through aliases and subranges before they are
compared. P4026 (`FunctionCallArgTypeMismatch`), which allows implicit
widening, skips `VAR_IN_OUT` parameters so that a mismatch is reported only
once.

A `REF_TO` parameter type is left to the reference rules. A parameter of a
non-elementary type is not compared yet; codegen refuses it (see below).

## Code Generation

### Parameter slot

A `VAR_IN_OUT` parameter takes one slot in the function's region, in its
declaration position among the input-compatible parameters, like any other
parameter. The slot holds a reference (a 64-bit variable-table index), not the
value. The compiler still records the parameter's own `VarTypeInfo` (width,
signedness, storage bits), because reads and writes operate on the referenced
value.

`CompileContext::in_out_params` holds the names of the `VAR_IN_OUT`
parameters of the function being compiled. It is saved and restored around
each function body, like the other per-scope maps.

### Call site

`UserFunctionInfo::params` records, for each input-compatible parameter, how
the call passes its argument:

| `ParamPassing` | Call site emits |
|---|---|
| `Value(op_type)` | the argument's value at `op_type` |
| `String(info)` | a copy of the string into the parameter's data region, then a dummy slot |
| `Reference` | a reference to the argument variable |

For `Reference`, the argument must be a named variable that occupies exactly
one slot, meaning it has type info and is not a string, array, structure or
function block instance. The emitted code depends on the argument:

| Argument | Emitted | Why |
|---|---|---|
| A variable of the caller | `LOAD_CONST_I64 <index>` | the same value `REF(x)` pushes |
| A `VAR_IN_OUT` parameter of the calling function | `LOAD_VAR_I64 <its slot>` | forwards the reference it holds, so the write lands in the original variable and not in the caller's parameter slot |

`CALL` then pops the reference into the parameter's slot, like any argument.

### Inside the function

| Source | Bytecode |
|---|---|
| read `p` | `LOAD_VAR_I64 slot`, `LOAD_INDIRECT` |
| `p := e` | `e`, truncation to `p`'s type, `LOAD_VAR_I64 slot`, `STORE_INDIRECT` |
| `REF(p)` | `LOAD_VAR_I64 slot` (the caller's variable, not the slot) |
| `p` passed on to another `VAR_IN_OUT` | `LOAD_VAR_I64 slot` |
| `p` passed to a `VAR_INPUT` | a read, as above |

Reads and writes go through `ResolvedAccess::InOut { ref_slot }`, which
`resolve_access` returns for a named `VAR_IN_OUT` parameter. They are one
arm in each place that already dispatches on `ResolvedAccess`.

A write truncates exactly as a write to a local of the same type does. Slots
hold raw 64-bit values, so `LOAD_INDIRECT` returns what `STORE_VAR_I32` or
`STORE_VAR_F32` wrote, bit for bit.

### Refusing everything else

`CompileContext::var_index`, the lookup used by every site that loads or
stores a variable's slot directly, refuses a `VAR_IN_OUT` parameter with a
not-implemented diagnostic. A site that handles `VAR_IN_OUT` asks
`CompileContext::in_out_ref_slot` first. Any site that has not been taught
about references therefore fails to compile instead of silently treating the
reference as the value.

The sites refused this way today are:

- the control variable of a `FOR` loop
- the target of a bit or partial access write (`p.3 := TRUE`)
- the target of a dereference assignment (`p^ := e`)
- the target of a `=>` output assignment

The comparison peephole (`try_classify_cmp`) looks variables up with the same
function and falls back to the general path.

### Debug information

A `VAR_IN_OUT` parameter's `VarNameEntry` is tagged `iec_type_tag::OTHER`
with type name `REF_TO <type>`. A debugger then shows the slot's content, a
reference, as a reference and not as a value of the parameter's type.

## Runtime

A reference names a variable in some caller's frame: a program variable, a
function block's fields, or a calling function's locals. `CALL` narrows the
current frame's `VariableScope` to the callee's own region plus the shared
globals. That scope covers none of a caller's function block fields or
locals, so checking an indirect access against it would trap on valid
programs.

`LOAD_INDIRECT` and `STORE_INDIRECT` therefore check the target index against
the **program instance's scope**, the `entry_scope` the instance started
with, rather than the current frame's:

- Every frame of the instance lies inside the instance's partition, so a
  reference to any caller's variable is valid.
- Another program instance's partition is still out of reach, so instance
  isolation holds.
- The null check (`V4004 NullDereference`) and bounds check are unchanged.
  Codegen never produces a null `VAR_IN_OUT` reference, because the caller
  always binds a variable.

This is the rule [ref-to.md](ref-to.md) already states for `REF_TO`
("validates that the index is within the variable table bounds, not that it
belongs to the current scope"). `LOAD_ARRAY_DEREF` and `STORE_ARRAY_DEREF`
already skip the frame scope for the same reason. It changes `REF_TO` too: a
reference passed into a function can now reach a caller's function block
fields or locals where it used to trap.

No reference can dangle. Function and function block regions are allocated
statically for the life of the program (ADR-0046), so a region's slots exist
even after its call returns.

## Alternatives Considered

**Copy-in/copy-out (value-result).** Copy the argument into the parameter
slot before `CALL`, and copy the slot back into the argument after `RET`.
This needs no VM change and keeps every body access a plain `LOAD_VAR`.
Rejected: it breaks the aliasing and visibility rules above (`BOTH(x, x)`,
and a function that reads a global it also receives by `VAR_IN_OUT`), and a
function that returns early still needs every exit to write back. The
by-value implementation this design replaces was the first half of this
approach without the copy back, and lost every write.

**A dedicated reference opcode pair** (`LOAD_PARAM_REF`/`STORE_PARAM_REF`
taking the slot as an operand). It would save one instruction per access.
Rejected for now: `LOAD_VAR_I64` + `LOAD_INDIRECT` expresses the same thing
with existing, verified opcodes. The peephole optimizer is the place to fuse
the pair if profiling shows it matters.

**Checking indirect access against the current frame's scope, with the
caller widening the callee's scope.** Rejected: the scope is one contiguous
range plus globals, and the referenced variables can sit in several
unrelated caller regions.

## Extensions

These follow the same model: the slot holds a reference, and the call site
binds it. Each is refused by codegen until it is built.

**Function blocks and methods.** A function block's `VAR_IN_OUT` field holds a
reference. `FB_STORE_PARAM` stores the reference the call site pushes, and the
body reads and writes through it with `LOAD_INDIRECT`/`STORE_INDIRECT`. The
[bytecode instruction set](bytecode-instruction-set.md#reference-operations-ref_to-and-var_in_out)
shows this sequence. Because the field is copied between the instance's data
region and the body's scratch slots like any other field, the reference
survives the copy. Open question: IEC requires an FB's `VAR_IN_OUT` to be
bound, but a field keeps its value between calls. Should a call that omits the
argument be rejected by the analyzer, or should it reuse the previous binding?
Methods would reuse the function call-site path, since their arguments are
popped into parameter slots like `CALL`'s.

**Non-elementary types.** Strings, arrays and structures live in the data
region, and their slot holds a data-region offset. A reference to the
variable's slot would let the callee reach the offset, and through it the
data. It would need the element metadata (string capacity, array descriptor,
structure layout) that `REF_TO ARRAY` already registers
(`register_ref_to_array_metadata`). Every string, array and structure access
path would also need an indirect form. A string argument must also have the
same capacity as the parameter, or the parameter must accept any capacity
(`STRING` without a length), which needs a design decision.

**Array elements and structure fields as arguments.** A reference is a single
variable-table index and cannot encode an element offset. This is the same
limit as `REF(arr[i])`. It needs a wider reference representation, shared with
`REF_TO`.

## Testing

- **Analyzer.** Arity and binding with `VAR_IN_OUT` before and after
  `VAR_INPUT`, in positional and named calls. P4026 names the right parameter
  after a `VAR_IN_OUT`. P4057 fires for literals and expressions, and P4058 for
  a narrower integer, another type category, and a structure.
- **End to end** (`codegen/tests/it/end_to_end_user_function_in_out.rs`).
  Every test asserts on the caller's variable after the call, not only on the
  function's result. The cases are write-back, swap, two parameters aliasing
  one variable, forwarding through two functions, a function block field and a
  function local as arguments, `INT` wrap-around at the caller's width, `BOOL`
  and `REAL`, and `REF(param)`. Each program is first run through the full
  semantic analysis, because the end-to-end harness runs only type resolution,
  and a program the checker refuses must not pass.
- **Refusals.** A `STRING` parameter, an array element argument and a `FOR`
  control variable are each reported as not implemented.
