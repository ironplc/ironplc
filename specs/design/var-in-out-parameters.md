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

IronPLC already has references: `REF_TO` stores a reference in a 64-bit slot
and dereferences it with `LOAD_INDIRECT`/`STORE_INDIRECT`
([REF_TO design](ref-to.md)). This design compiles a `VAR_IN_OUT` parameter as
an **implicit reference**: a `REF_TO` that the caller binds when it calls, that
is never `NULL`, and that the POU cannot reassign.

Today a reference can only name a whole variable that occupies one slot of the
variable table. A `VAR_IN_OUT` argument can also be a string, an array, a
structure, a function block instance, an array element or a structure field,
and all of those live in the data region. This design therefore adds a second
form of reference that names a location in the data region (see
[References](#references)). `REF_TO` gains the same reach as a side effect.

Related documents:

- [REF_TO design](ref-to.md) — reference representation and runtime safety
- [User-defined function calls](user-defined-function-calls-design.md) — the
  `CALL` convention and the flat variable table
- [Function block infrastructure](function-block-infrastructure-design.md) —
  instance data blocks, `FB_CALL`, and per-type field scratch regions
- [Bytecode instruction set](bytecode-instruction-set.md) — reference
  operations, including a `VAR_IN_OUT` bytecode example
- [ADR-0046](../adrs/0046-flat-variable-table-for-function-calls.md) — one
  statically allocated variable region per function

## Scope

- `VAR_IN_OUT` parameters of functions, function blocks and methods
- parameter types:
  - elementary types (integers, reals, bit strings, `BOOL`, time and date types)
  - `STRING[n]` and `WSTRING[n]`
  - arrays and structures, including nested combinations of the two
  - `REF_TO` types
  - function block instances
- arguments:
  - named variables
  - array elements (`arr[i]`)
  - structure fields (`s.x`)
  - function block input fields (`fb.IN`)
  - any combination of these (`s.arr[i].x`)

Not covered:

- **Variable-length arrays (`ARRAY[*]`).** An Edition 3 language feature of
  its own, not yet parsed. A `VAR_IN_OUT` of that type would have to carry the
  bounds with the reference. The data-region reference below leaves room for
  this, but the representation of the bounds belongs to that feature's design.
- **`VAR_IN_OUT CONSTANT`** (Edition 3). A read-only reference that accepts
  constants. Not parsed today. It is the answer to "how do I pass a constant by
  reference", and it composes with this design: see
  [Constant arguments](#constant-arguments).

## Semantics

| Property | Rule |
|---|---|
| Argument | Must be a **writable variable**: a variable, element or field that the calling POU can prove is not constant (see [Writable arguments](#writable-arguments)). |
| Argument type | Must be the parameter's type exactly (an alias of it counts as the same type). No implicit conversion applies, including to string capacity and array bounds. |
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
writes values of the parameter's type into the caller's storage. If an `INT`
variable could be bound to a `DINT` parameter, the POU could store 100000 into
it: a value the `INT` cannot hold, which later `INT` arithmetic would
misinterpret. The same applies to a `STRING[80]` parameter bound to a
`STRING[10]` variable, or to an `ARRAY[0..9]` parameter bound to an
`ARRAY[0..4]`: the POU would write past the caller's storage.

### Writable arguments

A `VAR_IN_OUT` argument must not be constant, and the analyzer must be able to
**prove** it is not. Where it cannot prove it, the argument is rejected, even
if the program would in fact never write a constant.

The proof is local to each call site. Every binding site proves its own
argument writable, so a `VAR_IN_OUT` parameter is itself writable: every
variable it can refer to was proved writable where it was bound. That
induction is what makes a chain of calls safe without whole-program analysis.

The argument's root variable, the named variable an element or field
expression starts from, decides the argument:

| Root variable declared in | Writable |
|---|---|
| `VAR`, `VAR_TEMP`, `VAR_OUTPUT` of the calling POU, without `CONSTANT` | yes |
| `VAR` of a program, without `CONSTANT` | yes |
| `VAR_GLOBAL` / `VAR_EXTERNAL`, without `CONSTANT` | yes (P4010 already makes `VAR_EXTERNAL` repeat the global's `CONSTANT`) |
| `VAR_IN_OUT` of the calling POU | yes, by induction |
| any section with `CONSTANT` | **no** |
| `VAR_INPUT` of the calling POU | **no** |
| a direct input address (`%I…`) | **no** |
| a dereference (`p^`, `p^[i]`) | **no** |
| a name the analyzer cannot resolve to a declaration | **no** |

`VAR_INPUT` is rejected because its value came from the POU's caller, which
may have passed a constant. Consider a function block that receives a
`CONSTANT` through `VAR_INPUT` and then binds that input to another POU's
`VAR_IN_OUT`. Nothing in the function block's body can tell that the input
originated as a constant, so the argument cannot be proved writable.

A dereference is rejected for the same reason. `REF()` accepts function block
inputs and other variables whose origin may be constant, so the target of a
reference is not provably writable. This can be relaxed if the `REF()` rules
are later tightened to accept only provably writable operands.

Selecting into the root keeps or loses writability:

| Argument | Writable when |
|---|---|
| `arr[i]`, `s.x` | the root is writable |
| `fb.IN` (an input of an instance the caller owns) | the instance is writable |
| `fb.OUT`, `fb.local` | **never**: a function block's outputs and locals are not assignable from outside the instance |
| `fb.IO` (an instance's own `VAR_IN_OUT`) | **never**: see [Function blocks](#function-blocks) |

### Constant arguments

A program that needs to pass a constant by reference declares the parameter
`VAR_IN_OUT CONSTANT` (Edition 3). Such a parameter:

- accepts any argument of the exact type, including a constant, a literal or
  an expression. The caller materializes a literal or an expression into a
  temporary of the parameter's type and passes a reference to it.
- is not writable inside the POU: it cannot be assigned, passed to a
  non-constant `VAR_IN_OUT`, or given to `REF()`.

It uses the same reference representation as the rest of this design.
Parsing it, which today is not supported, is the only part that is its own
feature.

## Semantic Analysis

### Argument list

A call's argument list is the `VAR_INPUT` and `VAR_IN_OUT` declarations, in
declaration order. `FunctionSignature::input_parameters` and
`input_parameter_count` iterate over parameters that are
`is_input_compatible()`, the same list `xform_named_to_positional_args` orders
named arguments by. So the arity check (P4018), positional binding and the
named-to-positional rewrite all agree, whichever order the two kinds of
parameter are declared in. Function block and method calls bind by the same
list through `call_assignment_check::bind_inputs`, which also has to include
`VAR_IN_OUT` in the positional order.

### Rules

A `VAR_IN_OUT` argument of a function, function block or method call is
checked by `rule_function_call_in_out_argument` and its function block/method
counterparts in `call_assignment_check`, with these codes:

| Code | Condition |
|---|---|
| P4057 `FunctionCallInOutArgNotVariable` | The argument is not a variable, element or field: a literal, an arithmetic expression, a function call. |
| P4058 `FunctionCallInOutArgTypeMismatch` | The argument's type is not the parameter's type. Both are resolved through aliases and subranges. Otherwise types match only by identity: the same string kind and capacity, the same array bounds and element type, the same structure or function block type. |
| P4059 `InOutArgNotWritable` | The argument cannot be proved writable (see [Writable arguments](#writable-arguments)). |
| P4060 `InOutNotBound` | A function block invocation does not bind one of the instance's `VAR_IN_OUT` parameters. |
| P4061 `InOutAccessedOutside` | `fb.IO` names an instance's `VAR_IN_OUT` from outside the instance. |

P4026 (`FunctionCallArgTypeMismatch`), which allows implicit widening, skips
`VAR_IN_OUT` parameters so a mismatch is reported once.

Assigning to a `VAR_IN_OUT` parameter inside the POU needs no new rule: it is
an ordinary assignment to a variable of the parameter's type.

## References

### Two forms

A reference is a 64-bit slot value in one of three forms:

| Form | Encoding | Names |
|---|---|---|
| null | `u64::MAX` | nothing; dereferencing traps `V4004 NullDereference` |
| variable reference | `0 ≤ r < 2^16` | slot `r` of the variable table |
| data-region reference | `2^32 + offset`, `offset < 2^32` | byte `offset` of the data region |

Any other value traps as an invalid reference. The forms cannot collide:
variable indices are 16-bit, and a data-region reference always has bit 32
set and bits 33–63 clear, so it can never equal `u64::MAX`.

A **variable reference** is today's `REF_TO` reference. It names an
elementary variable (or a `REF_TO` variable) that lives in one slot.

A **data-region reference** names a location in the data region:

- an elementary array element or structure field: its 8-byte slot
- a string: its header
- an array or structure: its first byte
- a function block instance: its data block (the value `FB_LOAD_INSTANCE`
  pushes)

**Every storage location has exactly one reference.** Slot-resident variables
are named only by variable references, and data-region locations only by
data-region references. `REF(arr)` of an array or structure therefore produces
a data-region reference too, instead of today's variable reference to the
slot that holds the array's offset. That keeps reference equality (`=`/`<>`)
meaningful: `REF(arr) = REF(p)` is `TRUE` when `p` is a `VAR_IN_OUT` bound to
`arr`.

### Instructions

Existing instructions change as follows:

| Instruction | Change |
|---|---|
| `LOAD_INDIRECT` / `STORE_INDIRECT` | Also accept a data-region reference: read or write the 8-byte slot at `offset`, after checking `offset + 8 ≤ data region size`. |
| `LOAD_ARRAY_DEREF` / `STORE_ARRAY_DEREF` | Also accept a data-region reference as the array's base, instead of loading the base from the referenced variable. |

Two instructions are new:

| Instruction | Stack effect | Description |
|---|---|---|
| `REF_ELEM var_index, desc_index` | `[index] → [ref]` | Bounds-checks `index` against the descriptor, as `LOAD_ARRAY` does. Then takes the base from `var_index`'s slot and pushes a data-region reference to element `index`. The stride comes from the descriptor: 8 for slot arrays and structures, header plus capacity times character width for string arrays. |
| `REF_BASE` | `[ref] → [offset]` | Pops a data-region reference and pushes its offset as an I32, the form that slot-based data-region instructions expect in a variable. Traps on null or on a variable reference. |

Structures are flat slot arrays with a descriptor, so `REF_ELEM` also creates
a reference to a structure field (index = the field's slot offset) and to an
element of an array inside a structure (index = the flat index plus the
field's slot offset). These are the indices that `LOAD_ARRAY` and
`STORE_ARRAY` already use for those accesses. A function block's input field
is addressed the same way, through a descriptor of the instance's data block
as a flat slot array; each function block type gets one, as each structure
type does.

`REF_ELEM` is also what `REF(arr[i])` and `REF(s.x)` need.
[ref-to.md](ref-to.md) defers both today because a reference could not encode
an element.

### Runtime checks

- **Variable references** are checked against the **program instance's
  scope**, not the current frame's. A reference names a variable in some
  caller's frame, which `CALL`'s narrowed scope does not cover. Another program
  instance's partition stays out of reach. This is the rule
  [ref-to.md](ref-to.md) already states ("validates that the index is within
  the variable table bounds, not that it belongs to the current scope"). It
  changes `REF_TO` too: a reference passed into a function can reach a
  caller's locals and function block fields.
- **Data-region references** are bounds-checked against the data region, the
  same check `FB_STORE_PARAM` applies to an instance's block. They are no
  easier to misuse than an FB reference: no instruction turns an integer into
  a reference or does arithmetic on one. Only `REF_ELEM` (bounds-checked) and
  compile-time constants create them.
- No reference can dangle. Function and function block regions and the data
  region are allocated statically for the life of the program (ADR-0046).

## Code Generation

### Call site

`UserFunctionInfo::params` records, for each input-compatible parameter, how
the call passes its argument:

| `ParamPassing` | Call site emits |
|---|---|
| `Value(op_type)` | the argument's value at `op_type` |
| `String(info)` | a copy of the string into the parameter's data region, then a dummy slot (`VAR_INPUT` strings) |
| `Reference` | a reference to the argument |

The reference pushed for each kind of argument:

| Argument | Emitted |
|---|---|
| slot-resident variable of the caller | `LOAD_CONST_I64 <var index>` |
| data-region variable with a fixed offset (string, array, structure or instance, anywhere but behind a reference) | `LOAD_CONST_I64 <2^32 + offset>` |
| a `VAR_IN_OUT` parameter of the calling POU | `LOAD_VAR_I64 <its slot>`, forwarding the reference unchanged |
| array element, structure field, instance input field | index computation, then `REF_ELEM <base var>, <descriptor>` |
| element or field of the caller's own `VAR_IN_OUT` array or structure | the same, with the parameter's base slot (below) as `<base var>` |

A function or method call then pops the references into parameter slots with
`CALL`/`METHOD_CALL`. A function block invocation stores each one into its
field with `FB_STORE_PARAM`, like an input.

### Parameter slots

Every `VAR_IN_OUT` parameter has one slot in its declaration position among
the input-compatible parameters. The slot holds the reference.
`CompileContext::in_out_params` names the parameters of the POU being
compiled, and is saved and restored around each body. Inside a function block
the parameter is a field, so its "slot" is the field's slot in the type's
scratch region. `FB_CALL` copies it there from the instance block on entry and
back on exit, like any field. The reference is unchanged by the round trip.

A parameter of a data-region type (string, array, structure, instance) also
gets a hidden **base slot**, a local allocated after the POU's other locals.
The POU's prologue fills it:

```
LOAD_VAR_I64   <param slot>
REF_BASE
STORE_VAR_I32  <base slot>
```

From then on the body compiles the parameter as an ordinary local of its type
whose slot is the base slot. The base slot holds the data offset, which is
exactly what an array, structure or instance local's slot holds. So every
existing access path works unchanged: `LOAD_ARRAY`/`STORE_ARRAY`, the
structure field and nested-array paths, `COPY_REGION`, `FB_LOAD_INSTANCE`,
`FB_STORE_PARAM`, `FB_LOAD_PARAM` and `FB_CALL`. The parameter slot keeps the
reference for forwarding and `REF()`.

### Accesses by parameter type

**Elementary.** Accesses go through `ResolvedAccess::InOut { ref_slot }`:

| Source | Bytecode |
|---|---|
| read `p` | `LOAD_VAR_I64 slot`, `LOAD_INDIRECT` |
| `p := e` | `e`, truncation to `p`'s type, `LOAD_VAR_I64 slot`, `STORE_INDIRECT` |
| `REF(p)` | `LOAD_VAR_I64 slot` |
| `p` passed to a `VAR_IN_OUT` | `LOAD_VAR_I64 slot` |
| `p` passed to a `VAR_INPUT` | a read, as above |

Slots, and 8-byte data-region slots, hold raw 64-bit values, so
`LOAD_INDIRECT` returns bit for bit what the variable's own store wrote,
whichever form the reference takes. A write truncates exactly as a write to a
local of the same type does.

**`REF_TO T`.** The slot holds a reference to the caller's reference variable,
which is elementary-sized. So `p` and `p := REF(x)` compile as in the
elementary case, and a dereference adds one indirection:

| Source | Bytecode |
|---|---|
| read `p^` | `LOAD_VAR_I64 slot`, `LOAD_INDIRECT`, `LOAD_INDIRECT` |
| `p^ := e` | `e`, `LOAD_VAR_I64 slot`, `LOAD_INDIRECT`, `STORE_INDIRECT` |
| `p^[i]` (`REF_TO ARRAY`) | `LOAD_VAR_I64 slot`, `LOAD_INDIRECT`, `STORE_VAR_I64 <scratch>`, then `LOAD_ARRAY_DEREF <scratch>` |

The scratch copy is taken at each access, so an assignment to `p` in the
caller's variable during the call is always seen.

**`STRING[n]` / `WSTRING[n]`.** The base slot holds the offset of the
caller's string header. A string local is addressed by a compile-time offset,
but the base is only known at run time, so the parameter is compiled as a
one-element string array whose descriptor has the parameter's capacity:

| Source | Bytecode |
|---|---|
| read `s` | `LOAD_CONST 0`, `STR_LOAD_ARRAY_ELEM <base slot>, <desc>` |
| `s := e` | `e`, `LOAD_CONST 0`, `STR_STORE_ARRAY_ELEM <base slot>, <desc>` |

String functions receive `s` through the read path, like any string value.
Because the capacity must match exactly (P4058), the descriptor describes the
caller's string correctly.

**Arrays and structures.** Compiled as a local of that type whose slot is the
base slot. Element and field reads and writes, whole-value assignment
(`COPY_REGION`, whose sizes come from the descriptors) and nested access are
all unchanged.

**Function block instances.** Compiled as an instance local whose slot is the
base slot. `p(...)`, `p.IN := e` and reads of `p.OUT` use the existing
function block instructions with the base slot as the instance's slot. The
call is dispatched on the parameter's declared type, which P4058 makes equal
to the argument's.

A function may take a function block instance as `VAR_IN_OUT` and invoke it:
the instance belongs to the caller, so the function stays stateless, and
P4054 already exempts it. Invoking an instance of type `T` runs `T`'s body in
`T`'s field scratch region, which is shared by every instance of `T`. If `T`'s
own body could reach, through a function, an invocation of another `T`
instance, that inner call would overwrite the outer call's fields
mid-execution. That path is a declaration cycle (`T` calls the function, and
the function's parameter has type `T`), which declaration ordering reports as
P4005 `RecursiveCycle`. The dependency on a parameter's type must stay an edge
in that ordering.

### Function blocks

A function block's `VAR_IN_OUT` field holds a reference, initialized to null.
Its accesses in the body are those of a function parameter, through the field
scratch slot and, for data-region types, a base slot filled by the body's
prologue. Two rules keep the stored reference from outliving the call that
bound it:

- **Every invocation binds every `VAR_IN_OUT`** (P4060). An instance
  never runs with a reference left over from an earlier call, and a body never
  sees the null initial value.
- **The field is not accessible from outside** (P4061).
  `fb.IO` would read or write through a reference whose target the reader
  cannot see.

### Methods

A method's arguments are popped into its own parameter slots, as `CALL` does
for a function, so a method's `VAR_IN_OUT` is compiled exactly like a
function's. A method may pass a field of its own instance: during the method,
the field lives in the type's field scratch region, which `METHOD_CALL`
copies back to the instance block on return.

### Refusing unhandled sites

`CompileContext::var_index`, the lookup used by every site that loads or
stores a variable's slot directly, refuses a `VAR_IN_OUT` parameter. A site
that handles `VAR_IN_OUT` asks `CompileContext::in_out_ref_slot` first. Any
site that has not been taught about references therefore fails to compile
instead of silently treating the reference as the value. The remaining
elementary sites handle it like this:

- **`FOR` control variable:** the loop reads and writes the variable through
  the reference, like any other read and write.
- **Bit and partial access writes** (`p.3 := TRUE`): read-modify-write through
  the reference.
- **`=>` output targets:** a store through the reference.

### Debug information

A `VAR_IN_OUT` parameter's `VarNameEntry` is tagged `iec_type_tag::OTHER`
with type name `REF_TO <type>`, so a debugger shows the slot's content as a
reference. Base slots are compiler-internal and get no entry.

## Alternatives Considered

**Copy-in/copy-out (value-result).** Copy the argument into the parameter
before the call and back after it. This needs no new reference form: an
element, a string or a whole array would just be copied. Rejected: it breaks
the aliasing and visibility rules above (`BOTH(x, x)`, or a function that
reads a global it also receives by `VAR_IN_OUT`), and copying a large array or
structure twice per call costs more than a reference. The by-value
implementation this design replaces was the first half of this approach, and
lost every write.

**One reference form: a data-region offset for everything.** Move every
variable a reference can name into the data region, so that a reference is
always an offset. Rejected: elementary variables live in the variable table,
and moving them would change every load and store in the compiler to reach a
feature that one tag bit provides.

**A (variable index, element offset) pair.** Keep references rooted at a
variable and add an offset for elements. Rejected: the variable's slot only
holds the data offset, so the pair resolves to the same address a
data-region reference names directly. It also gives one location two
encodings, which breaks reference equality.

**Dedicated `VAR_IN_OUT` opcodes** (`LOAD_PARAM_REF slot` and similar). They
would save one instruction per elementary access. Rejected for now:
`LOAD_VAR_I64` + `LOAD_INDIRECT` expresses the same thing with existing,
verified opcodes, and the peephole optimizer is the place to fuse them if
profiling shows the need.

**Checking variable references against the current frame's scope, with the
caller widening the callee's scope.** Rejected: the scope is one contiguous
range plus globals, and the referenced variables can sit in several
unrelated caller regions.

## Testing

- **Analyzer:**
  - arity and binding with `VAR_IN_OUT` before and after `VAR_INPUT`, in
    positional and named calls, for functions, function blocks and methods
  - each rule, with P4058 covering string capacity, array bounds, structure
    and instance types
  - every row of both writability tables, including a constant reaching a
    function block input that is then bound to a `VAR_IN_OUT`
  - an unbound function block `VAR_IN_OUT`, and `fb.IO` accessed from outside
- **VM:** both reference forms through `LOAD_INDIRECT`/`STORE_INDIRECT` and the
  `*_ARRAY_DEREF` pair; `REF_ELEM` bounds traps; `REF_BASE` on null and on a
  variable reference; and a variable reference into a caller's frame from a
  nested call
- **End to end:** every test asserts on the caller's storage after the call,
  not only on the result. Each program is first run through the full semantic
  analysis, because the end-to-end harness runs only type resolution, and a
  program the checker refuses must not pass. Cases:
  - per parameter type (elementary, `REF_TO`, string, array, structure,
    instance), per argument kind (variable, element, field, forwarded
    parameter), for each of function, function block and method
  - aliasing: two parameters bound to one variable, and an element bound to
    one parameter while its array is bound to another
  - a function block parameter rebound to a different variable on each call
