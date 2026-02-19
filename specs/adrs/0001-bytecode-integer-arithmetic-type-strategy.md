# Two-Width Integer Arithmetic With Explicit Narrowing

status: proposed
date: 2026-02-17

## Context and Problem Statement

The IronPLC virtual PLC runtime needs a bytecode instruction set. IEC 61131-3 defines many integer types: SINT (8-bit signed), INT (16-bit signed), DINT (32-bit signed), LINT (64-bit signed), and unsigned variants USINT, UINT, UDINT, ULINT. The instruction set must support arithmetic on all of them.

How should the bytecode represent arithmetic operations across these types? A naive approach (one opcode per type per operation) produces a combinatorial explosion. A fully generic approach (one untyped ADD opcode with runtime tag checking) sacrifices performance and prevents static verification.

## Decision Drivers

* **Opcode count** — the instruction set should fit in a single-byte encoding (max 256 opcodes), keeping the VM dispatch table compact
* **No runtime type checks in the hot path** — a PLC scan cycle has hard timing constraints; every runtime branch in the arithmetic path costs determinism
* **Static bytecode verification** — the bytecode should be verifiable for type safety without executing it, which requires type information to be encoded in opcodes or operands
* **Correct overflow semantics** — IEC 61131-3 integer types have different value ranges, and arithmetic that overflows at a narrow width produces different results than arithmetic at a wider width (see ADR-0002)
* **Signedness correctness** — unsigned types must be zero-extended on promotion, signed types sign-extended; getting this wrong silently corrupts comparisons and arithmetic

## Considered Options

* Per-width opcodes (WebAssembly style)
* Promote-all-to-maximum-width (single-width arithmetic)
* Two-width arithmetic with explicit narrowing (JVM-inspired, adapted)

## Decision Outcome

Chosen option: "Two-width arithmetic with explicit narrowing", because it keeps the opcode count manageable (~60-80 total), eliminates runtime type checks in arithmetic, enables static verification, and isolates overflow semantics to explicit narrowing instructions where they can be handled correctly per ADR-0002.

### Consequences

* Good, because the arithmetic opcode count is 8 (4 operations x 2 widths for integer, same for float) instead of ~40+ for per-width opcodes
* Good, because all sub-32-bit types are promoted on load, so arithmetic handlers don't need to handle 8-bit and 16-bit edge cases
* Good, because signed/unsigned distinction is preserved (separate I32/U32 and I64/U64 opcodes) preventing sign-extension bugs
* Good, because static verification is possible — the bytecode encodes type information in opcodes
* Bad, because the compiler must emit explicit NARROW instructions after every operation whose result is stored to a sub-32-bit variable, adding bytecode size
* Bad, because the promotion-on-load model means the VM never operates on native 8-bit or 16-bit values, which is slightly less efficient on 8-bit microcontrollers where 8-bit operations are cheaper than 32-bit ones

### Confirmation

Verify by inspection that the bytecode instruction set spec uses the two-width model consistently. When the VM is implemented, verify with tests that:
1. SINT/INT arithmetic produces identical results to native-width arithmetic for all wrapping cases
2. USINT/UINT values are zero-extended, SINT/INT values are sign-extended on load
3. Comparison operations on promoted unsigned values produce correct results (e.g., USINT 200 > USINT 128 is true)

## Pros and Cons of the Options

### Per-Width Opcodes (WebAssembly Style)

One opcode per type per operation: ADD_SINT, ADD_INT, ADD_DINT, ADD_LINT, ADD_USINT, ADD_UINT, ADD_UDINT, ADD_ULINT, and the same for SUB, MUL, DIV, MOD, NEG.

* Good, because overflow semantics are exact — each opcode operates at the native width, so overflow happens naturally at the correct boundary
* Good, because no promotion or narrowing instructions needed
* Good, because microcontrollers with native 8-bit or 16-bit ALUs can execute narrow operations efficiently
* Bad, because opcode count explodes: 8 integer types x 6 operations = 48 arithmetic opcodes for integers alone, plus float variants, plus comparisons — easily 80+ opcodes just for arithmetic
* Bad, because the VM dispatch table becomes large, and most handlers contain nearly identical code with only the width differing
* Bad, because adding a new operation requires adding 8+ opcodes

### Promote-All-to-Maximum-Width (Single-Width Arithmetic)

Promote all integer types to LINT (64-bit signed) on load. One set of arithmetic opcodes operating on 64-bit values. Narrow on store.

* Good, because minimal opcode count — one ADD, one SUB, etc.
* Good, because simple VM implementation — all values are the same width
* Bad, because unsigned semantics are lost — ULINT max value (2^64 - 1) cannot be represented as a signed LINT; the promotion is lossy for large unsigned values
* Bad, because 64-bit arithmetic on 32-bit microcontrollers requires software emulation (two 32-bit operations per 64-bit add), doubling the cost of every arithmetic operation even when the original types were 8 or 16 bits
* Bad, because all narrowing happens at store time, so intermediate overflow is never detected at the natural width (see ADR-0002)

### Two-Width Arithmetic With Explicit Narrowing (JVM-Inspired, Adapted)

Promote sub-32-bit types to 32-bit on load. Keep 64-bit types at 64-bit. Separate signed and unsigned opcodes. Emit explicit NARROW instructions when storing back to sub-32-bit types.

Arithmetic opcodes: ADD_I32, SUB_I32, MUL_I32, DIV_I32, ADD_U32, SUB_U32, MUL_U32, DIV_U32 (and the same pattern for 64-bit and float).

Load promotions: SINT/INT promote to I32 (sign-extend). USINT/UINT promote to U32 (zero-extend). DINT stays I32. UDINT stays U32. LINT stays I64. ULINT stays U64.

Narrowing: NARROW_I8, NARROW_I16, NARROW_U8, NARROW_U16 are emitted by the compiler when storing a 32-bit result to a sub-32-bit variable. These opcodes apply the configured overflow policy (see ADR-0002).

* Good, because 8 integer arithmetic opcodes (4 ops x signed/unsigned) at 32-bit, 8 at 64-bit = 16 total integer arithmetic opcodes — compact
* Good, because signed/unsigned distinction prevents sign-extension bugs during promotion
* Good, because 32-bit arithmetic is native on most ARM and x86 targets, with 64-bit for LINT/ULINT only when needed
* Good, because narrowing points are explicit in the bytecode, making overflow behavior auditable and configurable
* Neutral, because the compiler must track original types and emit NARROW instructions — moderate compiler complexity
* Bad, because sub-32-bit operations require an extra NARROW instruction, increasing bytecode size by roughly 10-15% for programs heavy in SINT/USINT usage

## More Information

The JVM uses a similar strategy (byte/short promoted to int, long separate, float/double separate) but does not distinguish signed from unsigned since Java lacks unsigned integer types. IEC 61131-3 has unsigned types, so we add signed/unsigned opcode variants to prevent the sign-extension bug where USINT 200 is incorrectly sign-extended to -56 when promoted.

WebAssembly uses four widths (i32, i64, f32, f64) without signed/unsigned opcode variants — instead, it has separate signed and unsigned comparison/conversion instructions. That approach also works but places the signed/unsigned distinction in different parts of the instruction set. Our approach places it uniformly in the arithmetic opcodes for consistency.

The NARROW instructions are closely related to ADR-0002 (overflow behavior), which defines what happens when a narrowing conversion produces a value outside the target type's range.
