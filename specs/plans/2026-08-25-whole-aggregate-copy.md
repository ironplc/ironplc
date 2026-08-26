# Whole-aggregate assignment: copy the value, don't alias the region

Issue: [#1414](https://github.com/ironplc/ironplc/issues/1414)

## Problem

`x := y` between two array variables copies the *data-region offset* instead of
the elements, so `x` becomes an alias of `y`. Every later read **and write**
through `x` lands in `y`'s storage, and `x`'s own region is orphaned. No
diagnostic is produced.

```st
PROGRAM main
VAR
    x : ARRAY[1..2] OF DINT;
    y : ARRAY[1..2] OF DINT;
    rx : DINT;
    ry : DINT;
END_VAR
    y[1] := 5;
    x := y;
    x[1] := 99;
    rx := x[1];   (* 99 *)
    ry := y[1];   (* 99 -- should be 5 *)
END_PROGRAM
```

`compile_statement`'s assignment path special-cases a whole-**struct** target
(`compile_stmt.rs:222`) but has no equivalent for arrays, so an array target
falls through to the scalar arm (`compile_stmt.rs:278`) and emits:

```
x := y   =>   LOAD_VAR_I32  y
              STORE_VAR_I32 x
```

Since an array variable's slot holds its data-region byte offset, that copies
the offset. Structures are unaffected — they already have a copy protocol,
written out in full in [the appendix](#appendix-the-existing-struct-copy-protocol).

IEC 61131-3 §7.3.3.1 defines the assignment statement over a "single or
multi-element variable" — arrays and structures alike — as a value copy. There
is no reference semantics for a non-`REF_TO` variable.

The existing struct copy protocol (`compile_stmt.rs:230`) cannot be reused as
is: it pushes every slot of the value onto the operand stack, then pops them
back. Fine for a handful of struct fields, but a variable may occupy up to
`MAX_DATA_REGION_SLOTS` = 32768 slots (`compile.rs:107`), so applying it to
arrays would blow the operand stack. That ceiling is already a latent bug for a
sufficiently large struct.

## Approach

One block-copy opcode serves every whole-aggregate assignment — arrays of any
element type, structures, and the nested combinations — with the struct path
migrated onto it so the operand-stack ceiling goes away.

### Copy lengths are always compile-time constants

Worth establishing first, because it is what makes the opcode design below
possible.

IEC 61131-3 has no array slice or array-section notation. The grammar is
`subscript_list ::= '[' subscript {',' subscript} ']'` with
`subscript ::= expression` (`parser/src/parser.rs:936-937`) — commas separate
*dimensions*, and `..` is not an operator, so `y[1..2]` is a syntax error today.
Ranges appear only in declarations (`ARRAY[1..10] OF INT`), subrange types, and
CASE labels. Array sections are a Fortran/Ada feature.

The one other construct that could give an aggregate a runtime size is the
Ed. 3 variable-length array (`ARRAY[*]`), which ironplc does not implement —
there is no `ARRAY[*]` in the grammar and no `LOWER_BOUND`/`UPPER_BOUND` in the
stdlib.

So today no whole-aggregate copy has a runtime-variable length, and no runtime
length arithmetic is needed anywhere. Since that is load-bearing for the opcode
design, the next section works out what happens when it stops being true.

### If variable-length arrays arrive later

The design must not foreclose them, so here is how they would fit.

A variable-length array is only ever a **formal parameter** bound to a caller's
actual array (Ed. 3 restricts `ARRAY[*]` to `VAR_IN_OUT`), with bounds queried
at runtime via `LOWER_BOUND`/`UPPER_BOUND`. So the callee cannot have a
container-level array descriptor for it: its size is a property of the *call*,
not of the program text. Implementing them means the parameter carries a
**runtime descriptor** — bounds per dimension alongside the data offset —
which is also what `LOWER_BOUND`/`UPPER_BOUND` would read.

That gives a second region operation rather than a change to this one:

```
COPY_REGION     = encode_opcode(OP_CLASS_REGION_OP, 0)   // sizes from container descriptors
COPY_REGION_DYN = encode_opcode(OP_CLASS_REGION_OP, 1)   // sizes from runtime descriptors
```

Reserving the type tag for region operations — rather than spending the op class
on a single instruction — is precisely what buys this. `COPY_REGION_DYN` costs
zero op-class slots, and `0x3F` still stays free.

Two consequences worth deciding now, because they shape the current work:

1. **The size check stops being a backstop and becomes the real check.** For a
   VLA operand the analyzer can compare element types but not extents, so a
   mismatch is only detectable at runtime. `RegionSizeMismatch` (V9018) is then
   a legitimate runtime error a correct compiler can produce from a correct
   program — not, as it is under this change, a signal that codegen is broken.
   The V9018 documentation should be written so that later reframing does not
   contradict it: describe it as "source and destination regions differ in
   size", and put "indicates a compiler defect" in the *cause* section rather
   than in the definition.
2. **Nothing about `COPY_REGION` needs to change.** A fixed-size aggregate keeps
   a container descriptor whether or not VLAs exist, so the tag-0 form stays
   correct as-is.

None of this is implemented here. It is recorded so the opcode's shape is a
deliberate choice rather than an accident of what the language supports today.

### Why a new opcode rather than an emitted loop

The alternative is a bounded copy loop in bytecode (index variable,
`LOAD_ARRAY`/`STORE_ARRAY`, `CMP_BR`). No VM change, but O(n) bytecode per
assignment and a per-element scan cost. The opcode is one instruction and a
`copy_within`, and it fixes the struct case's stack ceiling at the same time.

### `COPY_REGION` does not replace `STORE_ARRAY`

They are not substitutes, and neither can be folded into the other:

| | `STORE_ARRAY` | `COPY_REGION` |
|---|---|---|
| Data comes from | operand stack (one slot) | the data region itself |
| Length | fixed, 8 bytes | the whole object |
| Position | **runtime** index | whole object, no index |
| Check that earns its keep | `0 <= i < total_elements` | src and dst are the same size |

Expressing `COPY_REGION` via `STORE_ARRAY` is the emitted loop above. Expressing
`STORE_ARRAY` via `COPY_REGION` needs a scratch region to stage the stack value
into, and drops the index bounds check — the only thing stopping `x[3] := 1` on
an `ARRAY[1..2]` from writing into the next variable. `DataRegionOutOfBounds`
would not catch that; it is well inside the region.

The same holds for structures: per ADR-0027 struct fields have no opcode of
their own — they resolve to compile-time offsets and reuse
`LOAD_ARRAY`/`STORE_ARRAY`. This change does not disturb that. It replaces only
the whole-struct copy *protocol*.

### Safety

The VM has two tiers of checking today, and they do different jobs:

1. **Semantic** — `LOAD_ARRAY`/`STORE_ARRAY` check the index against the *array
   descriptor's* `total_elements` and trap
   `ArrayIndexOutOfBounds{var_index, index, total_elements}`. This catches
   language-level errors *inside* the data region.
2. **Memory** — `byte_offset + 8 > data_region.len()` →
   `DataRegionOutOfBounds`. The backstop that holds even if the compiler emitted
   a bogus offset.

Tier 2 is easy to get right for any operand shape. The design question is how
much of tier 1 survives.

A shape carrying an `n_bytes` immediate keeps none of it: the length is
unvalidated, so a codegen bug that over-copies silently walks into the
neighbouring variable — the exact failure mode of this issue, relocated from
codegen into the opcode.

So **the opcode carries no length**. It names two array descriptors, and the VM
derives both sizes from them and traps if they disagree. That is the same
discipline `LOAD_ARRAY` already follows by taking `total_elements` from the
descriptor rather than trusting an immediate, and it makes the runtime check a
cross-check of exactly the invariant this bug violated.

Residual gap, stated plainly: descriptors cannot distinguish
`ARRAY[1..6] OF INT` from `ARRAY[1..2,1..3] OF INT` (identical descriptors), nor
`INT` from `DINT` elements (both 8-byte slots — and `add_array_descriptor`
dedupes by key, so they share an index). Declared-shape equality is compile
time's job:

| Layer | Owns |
|---|---|
| Analyzer (P2037) | declared types identical — element type, dimensions, `STRING` max_len/width, struct type |
| VM (V9018 + bounds) | descriptor byte sizes agree; both ranges inside the data region |

This is the same division of labour as everywhere else: the compiler is trusted
that the offsets denote the right objects, and the VM guarantees no access
escapes the data region.

## The opcode

Op class `0x3E` becomes `OP_CLASS_REGION_OP`, with the **type tag selecting the
region operation** — the precedent set by `OP_CLASS_BOOL_OP` and
`OP_CLASS_STACK_OP` (`opcode.rs:113,126`). Only `0x3E` and `0x3F` remain free,
so this leaves three region ops for the future *and* keeps `0x3F`, rather than
spending a whole class on a single instruction.

```
COPY_REGION = encode_opcode(OP_CLASS_REGION_OP, 0)   // 0xF8

  [op][dst_var:u16][dst_desc:u16][src_desc:u16]      // 7 bytes, a new size shape
  pops src_offset                                     // Effect::new(1, 0)
```

Destination by variable index so it goes through `scope.check_access` — it is
the side that writes. Source offset from the operand stack so that `s := t`
(emit `LOAD_VAR_I32 t`) and `s := f()` use one path: a struct-returning function
already allocates a data region for its return variable and leaves the offset on
the stack (`compile_fn.rs:308-322`). The source is only read, and tier 2 confines
that read to the data region.

Emission:

```
compile_expr(rhs)                          ; LOAD_VAR_I32 y  -> src_offset
COPY_REGION dst=x, dst_desc=.., src_desc=..
```

VM handler, next to `STORE_ARRAY` (`vm/src/vm.rs:2534`):

```
n_dst = descriptor byte size of dst_desc      -> Trap::InvalidVariableIndex if absent
n_src = descriptor byte size of src_desc
if n_dst != n_src                             -> Trap::RegionSizeMismatch
scope.check_access(dst_var)?
dst_offset = variables.load(dst_var)?.as_i32()
checked bounds: src_offset + n <= len, dst_offset + n <= len
                                              -> Trap::DataRegionOutOfBounds
data_region.copy_within(src..src + n, dst)    // handles x := x and any overlap
```

**No `FORMAT_VERSION` bump.** Adding an opcode in a free slot is additive;
precedent is the `CMP_BR` addition, which did not bump. (v1→v2 was the ADR-0033
renumbering, v2→v3 the ADR-0035 string encoding change.)

### Descriptor byte size

Add `ArrayDescriptor::byte_size() -> Option<u32>` in
`container/src/type_section.rs:92`, beside the existing `element_char_width()`.
Every aggregate already has a descriptor, and all three cases are derivable:

| Aggregate | Descriptor | Bytes |
|---|---|---|
| struct | `(Slot, total_slots, 0)` | `total_slots * 8` |
| array of scalars | `(elem_type, total_elements, 0)` | `total_elements * 8` |
| `ARRAY OF STRING`/`WSTRING` | `(String\|WString, total_elements, max_len)` | `total_elements * (STRING_HEADER_BYTES + max_len * char_width)` |

It goes in the container so codegen and the VM share one definition;
`codegen::compile::string_region_size` (`compile.rs:154`) should delegate to it
rather than restate the stride.

A byte copy is correct for `ARRAY OF STRING` precisely because the analyzer has
already required identical types: each element's `[max_length][cur_length][data]`
header is copied verbatim from a source whose `max_length` is the same value.
The nested cases need no extra work either — a `STRING` field inside a struct is
embedded slot-aligned and counted by `slot_count()`, so `total_slots * 8` spans
it.

## Changes

### `compiler/container/`

| File | Change |
|---|---|
| `src/opcode.rs` | `OP_CLASS_REGION_OP = 0x3E`; `COPY_REGION` with an operand-layout doc comment; **an arm in `instruction_size_opt`** (`:846`) for the new 7-byte shape; update the `0x3E..0x3F free` comment at `:181` |
| `src/type_section.rs` | `ArrayDescriptor::byte_size()` |
| `src/verify.rs` | arm in `effect_of` (`:534`) giving `Effect::new(1, 0)`. No `flow_of` arm — control flow is unchanged. The guard test at `:1081` fails until this is done |

### `compiler/vm/`

| File | Change |
|---|---|
| `src/vm.rs` | dispatch arm in the flat `match op`, before `_ => Trap::InvalidInstruction(op)` at `:2675` |
| `src/error.rs` | `Trap::RegionSizeMismatch { dst_bytes: u32, src_bytes: u32 }` and its `Display` arm |
| `resources/problem-codes.csv` | `V9018,RegionSizeMismatch,...,struct` |

### `compiler/codegen/`

- **`src/compile_aggregate.rs` (new)** — `try_compile_whole_assignment(...) ->
  Result<bool, Diagnostic>`. A new module because `compile_stmt.rs` is already
  1259 lines, over the 1000-line limit.
  1. Target must be a plain `Named` variable, not an element or field access.
  2. Look it up in `ctx.struct_vars` / `ctx.array_vars` for
     `(var_index, desc_index)`. Neither → `Ok(false)`, fall through untouched.
  3. Bail (`Ok(false)`) for a `REF_TO` array, so `PT := other_ref` stays a
     pointer copy on the scalar path.
  4. Resolve the source descriptor: a bare aggregate variable name, or a
     struct-returning user function call (from `ctx.user_functions`; extend
     `UserFunctionInfo` with the return struct's `desc_index` if it does not
     already carry it). Anything else → `Diagnostic::not_implemented`.
  5. `compile_expr(rhs)`, `emit_copy_region(...)`, `Ok(true)`.
- `src/emit.rs` — `emit_copy_region(dst_var, dst_desc, src_desc)` with
  `pop_stack(1)`. The emitter's stack accounting and `verify::effect_of` must
  agree or `stack_balance.rs` fails the build.
- `src/compile_stmt.rs` — call the new helper where the struct special-case
  sits, and **delete the push-all-slots protocol at `:222-253`**, including the
  "temporarily repoint `dst_var` at the source, then restore it" dance.
- `src/compile_array.rs` — add `is_ref: bool` to `ArrayVarInfo` (`:51`), `true`
  in `register_ref_to_array_metadata` (`:792`) and `false` in
  `register_array_metadata`. Those two constructors are the only sites.
- `src/lib.rs` — `mod compile_aggregate;`.

### `compiler/project/src/disassemble.rs`

Add a `match` arm in `decode_instructions` (`:238`). Without it the instruction
renders as `UNKNOWN(0xF8)` **and misaligns every instruction after it**, because
that function hardcodes its own `pc +=` steps rather than calling
`opcode::instruction_size`. `CMP_BR_*` at `:611-638` is the template.

### `compiler/analyzer/`

- **`src/rule_assignment_aggregate_type_compat.rs` (new)** — P2037. Follows
  `rule_ref_to.rs`, which already does assignment type checking for P2032: a
  `DiagnosticVisitor` tracking declared variable types per POU scope and
  inspecting `StmtKind::Assignment`.
  - Fires only when the target resolves to `IntermediateType::Array { .. }` or
    `Structure { .. }`.
  - Resolves the source type from a named variable's declared type or a
    function's return type. Anything else → no opinion, stay silent.
  - `IntermediateType` derives `PartialEq` (`intermediate_type.rs:64`), so the
    check is a direct `dst_type != src_type`. That compares element type,
    dimensions, `STRING` max_len/char_width, and struct fields in one shot,
    recursively.
- `src/lib.rs` and `src/stages.rs` — register in the rule batch at
  `stages.rs:337-354`.
- `compiler/problems/resources/problem-codes.csv` —
  `P2037,AggregateAssignmentTypeMismatch,Assignment between arrays or structures requires identical types`.
  P2036 is the current maximum, and P2xxx is where P2032 `ReferenceTypeMismatch`
  lives.

Scoping this rule to aggregates is deliberate. Extending it to scalars is the
correct end state but would start rejecting scalar assignments that compile
today, and interacts with implicit widening (ADR-0029/0031). That is its own
change.

### Documentation

- `docs/reference/compiler/problems/P2037.rst` and `docs/reference/runtime/problems/V9018.rst`,
  each with an index entry.
- `specs/design/bytecode-instruction-set.md` — add `COPY_REGION` to the op-class
  table (`:72-108`), a region-op instruction table, and the Opcode Summary
  (`:745-818`). **The summary is already stale** and should be corrected in the
  same pass: `:746` says `FORMAT_VERSION = 2` (it is 3) and `:816` still lists
  `0x3D–0x3F` as free though `CMP_BR` took `0x3D`.
- `specs/design/bytecode-verifier-rules.md` — the stack-effect rule.

No parser, plc2plc, or LSP work: `x := y` is existing syntax and the renderer is
unaffected.

## Tests

| Leg | Asserts |
|---|---|
| `codegen/tests/it/wire_format.rs` | `COPY_REGION == 0xF8` in a region family test; a golden encoding for the new 7-byte shape; the byte added to the `pinned` registry at `:723`. The completeness guard fails until this is done |
| `codegen/tests/it/common/mod.rs` | a `bc::copy_region(...)` builder in a new 7-byte shape group |
| `codegen/tests/it/compile_aggregate_copy.rs` (new) | the emitted sequence — `LOAD_VAR_I32 src` then `COPY_REGION` with the expected operands — for an array target and a struct target |
| `codegen/tests/it/end_to_end_aggregate_copy.rs` (new) | run results (below) |
| `vm/tests/it/execute_copy_region.rs` (new) | hand-assembled bytecode: nominal copy; mismatched descriptor sizes → `RegionSizeMismatch`; out-of-bounds src and dst → `DataRegionOutOfBounds`; zero length; identical src and dst offsets |
| analyzer rule tests | accepts `x := y` for identical types; rejects mismatched dimensions, mismatched element types, mismatched `STRING` max_len, and array-vs-struct, each with P2037 |

The end-to-end cases: the issue's exact repro first (`rx` = 99, `ry` = 5), then
each element width (`INT`/`DINT`/`LINT`/`REAL`), a 2-D array, `ARRAY OF STRING`,
an array of structs, a struct containing an array, a struct containing a
`STRING`, self-assignment `x := x`, and a struct large enough that the old
push-all-slots protocol would have overflowed the operand stack.

Register the new files with `mod` lines in `codegen/tests/it/main.rs` and
`vm/tests/it/main.rs`.

Existing struct-assignment tests that assert the old opcode sequence need
rebaselining — expected, and the point of the change.

## Verification

```bash
cd compiler && just        # compile + coverage (>=85%) + clippy + fmt
```

Beyond the test suite, disassemble a container containing a `COPY_REGION` and
confirm the instruction decodes by name and that instructions after it stay
aligned.

## Out of scope

- Assignment type checking for **scalars** — see the analyzer section above.
- Whole-array function arguments, array return values, and `x := PT^` (deep copy
  through a dereferenced `REF_TO ARRAY`). Each reports
  `Diagnostic::not_implemented`, as it does now.

## Appendix: the existing struct copy protocol

Recorded here because this change deletes it, and because two of its properties
are what motivate the replacement.

Emitted by `compile_stmt.rs:222-253` for `dst := src` where `dst` is a struct
variable with `n = dst_info.total_slots`:

```
    <compile RHS>                       ; leaves the SOURCE data_offset on the stack
    STORE_VAR_I32  dst_var              ; dst_var now points at the SOURCE region

    LOAD_CONST_I32 0                    ; -- read every slot through dst_var,
    LOAD_ARRAY     dst_var, dst_desc    ;    which is currently aliasing src
    LOAD_CONST_I32 1
    LOAD_ARRAY     dst_var, dst_desc
    ...                                 ; n times, values accumulate on the stack
    LOAD_CONST_I32 n-1
    LOAD_ARRAY     dst_var, dst_desc

    LOAD_CONST_I32 <dst_data_offset>    ; -- restore dst_var to its own region
    STORE_VAR_I32  dst_var

    LOAD_CONST_I32 n-1                  ; -- write them back, LIFO
    STORE_ARRAY    dst_var, dst_desc
    ...
    LOAD_CONST_I32 0
    STORE_ARRAY    dst_var, dst_desc
```

The store loop runs in reverse because `STORE_ARRAY` pops the index first and
the value second (`vm.rs:2537-2538`), so the index pushed last sits on top and
the value beneath it is the most recently loaded slot — `n-1`.

Cost: roughly `4n + 4` instructions and a **peak operand stack of `n + 1`**.
`COPY_REGION` is 2 instructions and a peak of 1.

Three properties motivate replacing it:

1. **The operand-stack ceiling.** A variable may occupy up to
   `MAX_DATA_REGION_SLOTS` = 32768 slots, so the peak of `n + 1` is unbounded
   in practice. This is the reason the protocol cannot simply be extended to
   arrays, and it is a latent bug for a large struct today.
2. **`dst_var` transiently aliases the source.** Between the first
   `STORE_VAR_I32` and the restore, the destination variable's slot holds the
   *source's* offset — the same aliasing this issue is about, as an intended
   intermediate state. Any trap or debugger stop in that window observes a
   variable pointing at another variable's storage, and the sequence is not
   re-entrant.
3. **It reads the source through the *destination's* descriptor and slot
   count.** `LOAD_ARRAY dst_var, dst_desc` is run `dst_info.total_slots` times
   regardless of how large the source actually is, so `a := b` between different
   struct types silently over-reads past `b`'s region when `a` is the larger.
   Nothing checks this today; the new analyzer rule (P2037) is what closes it.

Note that the protocol is *correct* for the case it was written for — two
variables of the same struct type — which is why whole-struct assignment behaves
correctly in the issue's test matrix. It is the ceiling and the missing
type check that make it the wrong foundation to build the array case on.
