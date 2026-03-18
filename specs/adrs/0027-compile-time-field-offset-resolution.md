# Compile-Time Field Offset Resolution for Structures

status: proposed
date: 2026-03-18

## Context and Problem Statement

IEC 61131-3 structure field access uses dot notation: `myStruct.field`, `rect.topLeft.x`, `points[3].y`. The compiler must translate these accesses into bytecode that reads from or writes to the correct location in the data region.

In IEC 61131-3, there is no dynamic field selection — no reflection, computed member names, or runtime type dispatch for field access. The structure type and field name are always statically known at the point of access. This means the data region byte offset for any field access expression can be fully resolved at compile time.

How should structure field access be compiled to bytecode?

## Decision Drivers

* **Zero runtime overhead** — field access should compile to the same instructions as any other data region load/store, with no per-access indirection or lookup
* **No new opcodes** — the opcode budget (85 free of 256) should be preserved for future features (OOP method dispatch, packed element types); structure support should not consume opcodes if avoidable
* **Composability** — nested access (`a.b.c`), array-of-struct access (`arr[i].field`), and struct-with-array access (`s.arr[i]`) must all work
* **Debuggability** — while compile-time resolution eliminates field access from the bytecode, debug metadata should preserve the access chain for tools
* **Verifier compatibility** — the bytecode verifier must be able to validate that data region accesses are in-bounds without per-field metadata

## Considered Options

* **Option A: Compile-time offset resolution** — the compiler resolves every field access chain to a single data region byte offset. No struct-specific opcodes. The bytecode contains only generic data region load/store operations.
* **Option B: Struct reference + field index opcodes** — new opcodes (`STRUCT_LOAD_FIELD`, `STRUCT_STORE_FIELD`) that take a base reference (data_offset on the stack) and a field index operand. The VM computes the final offset at runtime. Similar to the FB_LOAD_PARAM/FB_STORE_PARAM pattern.
* **Option C: Struct reference + byte offset opcodes** — new opcodes that take a base reference and a byte offset operand. More flexible than field-index opcodes but still requires new opcodes.

## Decision Outcome

Chosen option: **Option A — Compile-time offset resolution**, because IEC 61131-3's static type system guarantees that all field offsets are compile-time constants, making runtime resolution unnecessary overhead.

### How It Works

The compiler maintains a mapping from structure variable names to their `StructVarInfo`:

```
StructVarInfo {
    var_index: u16,        // slot table index (holds data_offset)
    data_offset: u32,      // byte offset in data region
    struct_type: ...,      // resolved IntermediateType::Structure
}
```

For a field access `myStruct.x`:

1. Look up `myStruct` → `StructVarInfo { data_offset: 100, ... }`
2. Look up field `x` in the structure type → `field_slot_index: 2`
3. Compute absolute offset: `100 + 2 * 8 = 116`
4. Emit: `LOAD_CONST_I32 116` + generic data region load (reusing the array element load path with a constant index)

For nested access `outer.inner.x`:

1. Look up `outer` → `StructVarInfo { data_offset: 200, ... }`
2. Look up field `inner` in Outer type → starts at slot index 1
3. Look up field `x` in Inner type → slot index 0 within Inner
4. Compute absolute offset: `200 + (1 + 0) * 8 = 208`
5. Emit a single load at offset 208

For array-of-struct access `points[i].y`:

1. Look up `points` → array of structs, `data_offset: 300`, struct has 2 fields (x, y)
2. Field `y` is at slot index 1 within each struct element
3. Struct stride: 2 slots = 16 bytes
4. Emit: compute `300 + (flat_index * 2 + 1) * 8` where `flat_index` comes from the subscript expression
5. The `+ 1` (field offset within struct) is folded into the index computation at compile time

For struct-with-array access `s.values[i]`:

1. Look up `s` → `StructVarInfo { data_offset: 400, ... }`
2. Field `values` starts at slot index 1
3. Emit: compute `400 + (1 + flat_index) * 8` where `flat_index` comes from the subscript expression
4. The `+ 1` (slot offset of the array field within the struct) is folded into the base offset

### Consequences

* Good, because no new opcodes are needed — structure field access compiles to the same LOAD/STORE instructions used for arrays and scalar data region variables
* Good, because runtime performance is identical to scalar variable access — no per-access indirection, type dispatch, or field lookup
* Good, because the compiler is the only component that needs to understand structure types — the VM and verifier see only data region byte offsets
* Good, because composability is achieved through arithmetic — nested structures, arrays of structures, and structures of arrays all reduce to offset computation
* Good, because the opcode budget is preserved for future features
* Bad, because structure field access is invisible in the bytecode — a debugger sees a data region load at offset 208, not `outer.inner.x`. Debug metadata must bridge this gap
* Bad, because the bytecode verifier cannot distinguish "load field x of struct S" from "load arbitrary data region byte 208" — verification is limited to checking that the offset is within the data region bounds
* Neutral, because the compile-time resolution adds complexity to the compiler's field access resolution logic, but this is a one-time cost in a single code path

### Confirmation

Verify by compiling test programs and checking that:
1. `myStruct.field` compiles to a single constant-offset data region access
2. `a.b.c` (3-level nesting) compiles to a single constant-offset access
3. `arr[i].field` compiles to a computed offset with the field offset folded in
4. `s.arr[i]` compiles to a computed offset with the struct field base folded in
5. No STRUCT-specific opcodes appear in the bytecode

## Pros and Cons of the Options

### Option A: Compile-Time Offset Resolution (chosen)

The compiler resolves all field accesses to data region byte offsets. No struct-specific opcodes.

* Good, because zero opcodes consumed — 85 free slots preserved
* Good, because zero runtime overhead — no per-access indirection
* Good, because VM complexity does not increase — no new handlers
* Good, because the verifier is unchanged — only checks data region bounds
* Bad, because field access is invisible in the bytecode — harder to debug at the bytecode level
* Bad, because the verifier cannot enforce field-level type safety — it sees only byte offsets

### Option B: Struct Reference + Field Index Opcodes

New opcodes like FB_LOAD_PARAM/FB_STORE_PARAM but for structures. Push a struct reference (data_offset), then use `STRUCT_LOAD_FIELD(field_idx)` to access a field.

* Good, because field access is visible in the bytecode — debuggers and verifiers see explicit field operations
* Good, because the pattern matches the existing FB instance access model
* Bad, because it consumes at least 2 opcodes (load + store) and potentially more for typed variants
* Bad, because runtime overhead per field access — the VM must compute `base + field_index * 8` at runtime for information the compiler already knew
* Bad, because composability is harder — nested access `a.b.c` requires multiple reference + field operations instead of one offset computation
* Bad, because array-of-struct access `arr[i].field` requires an awkward combination of array indexing and struct field access on the stack

### Option C: Struct Reference + Byte Offset Opcodes

New opcodes that take a base reference and a byte offset operand: `STRUCT_LOAD(byte_offset)`, `STRUCT_STORE(byte_offset)`.

* Good, because more flexible than field-index opcodes — works with any layout
* Good, because field access is visible in the bytecode
* Bad, because it still consumes opcodes (at least 2, potentially typed variants)
* Bad, because `byte_offset` is a compile-time constant, so this is equivalent to Option A but with dedicated opcodes that add no semantic value beyond bytecode readability
* Bad, because the verifier still cannot do more than bounds-check the offset

## More Information

### Debug Metadata for Field Access

To support debuggers and diagnostic tools, the compiler should emit debug metadata that maps data region byte offsets back to structure field paths. This is an extension to the existing debug section (ADR-0019) and does not affect the bytecode or runtime. The format of this metadata is outside the scope of this ADR.

### Relationship to ADR-0026

ADR-0026 defines the memory layout of structures in the data region (8-byte slots per field, inline nesting). This ADR defines how the compiler translates field access expressions into byte offsets within that layout. The two are complementary: ADR-0026 defines "where" fields live, this ADR defines "how" the compiler finds them.

### Relationship to Future Packed Layout

When the memory layout transitions from 8-byte slots to packed byte-level layout (ADR-0026 migration path), the compile-time offset resolution strategy remains unchanged. Only the offset arithmetic changes: `field_slot_index * 8` becomes `field.byte_offset`. The bytecode continues to contain only data region byte offsets, and no opcodes change.

### Why This Differs from Function Block Access

Function block instances use `FB_LOAD_INSTANCE`/`FB_STORE_PARAM`/`FB_LOAD_PARAM` opcodes because FB access involves more than field read/write — it includes instance lifecycle management (pushing the FB reference for `FB_CALL`, parameter copy-in/copy-out semantics). Structures have no lifecycle — they are pure data containers. Using dedicated opcodes for structures would add complexity without adding capability.
