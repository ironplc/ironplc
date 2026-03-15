# Array Bounds Safety

status: proposed
date: 2026-03-13

## Context and Problem Statement

The compiler is adding code generation support for arrays of primitive types. IEC 61131-3 arrays support arbitrary lower bounds (`ARRAY[-5..5] OF INT`), multiple dimensions (`ARRAY[1..3, 1..4] OF REAL`), and nested declarations (`ARRAY[1..3] OF ARRAY[1..4] OF INT`). Array access can use constant subscripts (known at compile time) or variable subscripts (known only at runtime).

A PLC controlling physical processes must never access memory outside an array's allocated region. How should the compiler and VM enforce array bounds safety?

## Decision Drivers

* **Physical safety** — out-of-bounds array access in a PLC program can corrupt variables that control actuators, with potential for physical damage (ADR-0005)
* **Defense-in-depth** — the compiler may have bugs; the VM must independently enforce bounds even for well-compiled programs (ADR-0005 confirmation checklist item 2)
* **Zero-cost when possible** — constant subscripts are common in PLC programs and should be validated at compile time with no runtime overhead
* **Simplicity** — the VM bounds check should be a single comparison, not a multi-dimensional traversal
* **Debuggability** — when a bounds violation occurs, the error should help the programmer locate the bug

## Considered Options

* **Option A: Per-dimension runtime checking** — the VM checks each subscript against its dimension's bounds independently, then computes the flat offset
* **Option B: Flat 0-based runtime checking with compile-time per-dimension checking** — the compiler normalizes all subscripts to a single 0-based flat index; the VM checks `0 ≤ flat_index < total_elements`
* **Option C: Compiler-only checking** — the compiler inserts conditional-branch guard code before each array access; no VM-level check

## Decision Outcome

Chosen option: **Option B** — flat 0-based runtime checking with compile-time per-dimension checking. This provides three layers of enforcement with minimal VM complexity.

### Three-Layer Enforcement Model

| Layer | What it checks | When | Cost |
|-------|---------------|------|------|
| **Compiler (static)** | Each constant subscript checked against its dimension's declared bounds | Compile time | Zero runtime cost |
| **VM (LOAD_ARRAY / STORE_ARRAY)** | `0 ≤ flat_index < total_elements` | Every array access | One comparison + one conditional branch |
| **Bytecode verifier** | Variable has `is_array` flag; `type` byte matches descriptor's `element_type` | Load time | Zero per-access cost |

### How It Works

**Constant subscripts** — When all subscripts in an array access are compile-time constants, the compiler:

1. Validates each subscript against its dimension's declared bounds (per-dimension check)
2. Computes the 0-based flat index at compile time
3. Emits a `LOAD_CONST flat_index` followed by `LOAD_ARRAY` / `STORE_ARRAY`

If any subscript is out of bounds, the compiler emits a diagnostic error. The programmer sees which dimension and which bound was violated.

**Variable subscripts** — When any subscript is a runtime expression, the compiler:

1. Emits code to compute the 0-based flat index: `flat_index = Σ (subscript_k - lower_bound_k) × stride_k`
2. Emits `LOAD_ARRAY` / `STORE_ARRAY` with the flat index on the stack

The VM checks the flat index against `[0, total_elements)`. If the check fails, execution traps with `ArrayIndexOutOfBounds`.

**Always 0-based descriptors** — The compiler normalizes all array indices to 0-based before emitting `LOAD_ARRAY` / `STORE_ARRAY`. Array descriptors in the container always store `lower_bound = 0` and `upper_bound = total_elements - 1`. Original IEC 61131-3 bounds are preserved in the debug section for error reporting. This simplifies the VM to a single unsigned comparison: `index < total_elements`.

### Why Flat Checking Is Sufficient for Memory Safety

The flat bounds check prevents any access outside the array's allocated data region. Consider the potential failure modes:

1. **Subscript too large in one dimension**: increases the flat index beyond `total_elements`, caught by `index < total_elements`
2. **Subscript too small (below lower bound)**: the expression `(subscript - lower_bound)` produces a negative value. Since the stack uses signed I32 arithmetic, the negative intermediate may wrap or remain negative. Either way:
   - If the final flat index is negative (as I32), it fails the unsigned comparison `index < total_elements`
   - If intermediate arithmetic wraps to a large positive value, it exceeds `total_elements`
3. **Invalid dimension combination that lands in-bounds**: For example, in `ARRAY[1..3, 1..4]`, accessing `[0, 5]` computes `(0-1)*4 + (5-1) = 0`, which is in-bounds but semantically wrong. This is a logical error, not a memory safety issue — it reads a valid array element (the first one). The flat check guarantees memory safety but not semantic correctness for per-dimension bounds.

Per-dimension runtime checks could catch case (3) and provide better error messages, but they are not required for memory safety. They can be added as a future enhancement by emitting compiler-generated comparison + trap instructions before the flat index computation.

### Consequences

* Good, because memory safety is guaranteed by the VM independently of the compiler — defense-in-depth
* Good, because the VM bounds check is a single comparison — minimal performance overhead per array access
* Good, because constant-subscript programs (the common case in PLC code) get zero-cost compile-time checking with per-dimension error messages
* Good, because the always-0-based descriptor simplifies the VM: no signed arithmetic in the hot path, one unsigned comparison
* Good, because the design extends to arrays of complex types (strings, structs, FBs) — the flat index logic is independent of element type
* Neutral, because variable-subscript programs get flat-only runtime checking — logically invalid multi-dimensional indices that happen to produce valid flat indices are not caught. This is acceptable because it only affects which valid element is accessed, not memory safety
* Bad, because variable-subscript runtime error messages report only the flat index, not the original dimension subscripts — the programmer must manually map back. Debug section metadata can help tools provide better messages in the future

## Pros and Cons of the Options

### Option A: Per-Dimension Runtime Checking

The VM checks each subscript independently against its dimension's bounds, then computes the flat offset.

* Good, because every out-of-bounds access is caught with precise per-dimension error messages
* Bad, because the VM must store and traverse per-dimension metadata at runtime — array descriptors become variable-length, adding complexity to the container format and VM
* Bad, because the bounds check cost scales with the number of dimensions — O(N) comparisons per access instead of O(1)
* Bad, because this requires passing multiple subscripts to the VM (either on the stack or as operands), changing the LOAD_ARRAY/STORE_ARRAY calling convention from the current spec

### Option B: Flat 0-Based Runtime Checking (chosen)

The compiler computes a single 0-based flat index. The VM checks it with one comparison.

* Good, because the VM is simple — one comparison, fixed-size descriptor, no dimension traversal
* Good, because the existing LOAD_ARRAY/STORE_ARRAY calling convention (single index on stack) is preserved
* Good, because constant subscripts still get per-dimension checking at compile time
* Bad, because variable-subscript per-dimension errors are reported as flat-index violations
* Bad, because logically invalid multi-dimensional combinations that produce valid flat indices are not caught at runtime

### Option C: Compiler-Only Checking

The compiler inserts conditional branches before each array access. The VM has no bounds check.

* Good, because no special VM support is needed — uses existing conditional-jump instructions
* Bad, because bounds safety depends entirely on the compiler being correct — violates defense-in-depth (ADR-0005 item 2)
* Bad, because a compiler bug or a hand-crafted bytecode container can bypass all bounds checks
* Bad, because the conditional branches add code size and branch-prediction overhead to every array access, even when the verifier could prove safety

## More Information

### Multi-Dimensional Index Computation

For an N-dimensional array `ARRAY[l_1..u_1, l_2..u_2, ..., l_N..u_N] OF T`, the 0-based flat index for subscripts `(s_1, s_2, ..., s_N)` is:

```
flat_index = (s_1 - l_1) * stride_1 + (s_2 - l_2) * stride_2 + ... + (s_N - l_N) * stride_N
```

where `stride_k = size_{k+1} * size_{k+2} * ... * size_N` and `size_k = u_k - l_k + 1`.

The last dimension has `stride_N = 1`. This is row-major order, consistent with the bytecode instruction set spec.

### Nested Arrays

`ARRAY[1..3] OF ARRAY[1..4] OF INT` is flattened identically to `ARRAY[1..3, 1..4] OF INT`. The compiler recursively resolves nested `ArrayVariable` AST nodes to collect all subscripts and dimension bounds, then applies the same flat-index formula. In the container, both forms produce a single 12-element array with one descriptor (`lower_bound=0, upper_bound=11`).

### Element Storage

Each array element occupies 8 bytes (one slot width) in the data region, regardless of the element's declared type. This is consistent with function block field storage and simplifies the VM (every element is at `data_offset + flat_index * 8`). Packed element storage is a future optimization tracked separately.

### Relationship to ADR-0005

ADR-0005 established the safety-first principle and specifically listed "Dedicated LOAD_ARRAY/STORE_ARRAY with mandatory bounds checking" as a safety choice (2 opcodes spent). This ADR defines the concrete bounds-checking strategy for those opcodes.

### Relationship to ADR-0017

ADR-0017 established the unified data region where array elements are stored. The `data_offset` in the variable's slot points to the start of the array's element storage within the data region.
