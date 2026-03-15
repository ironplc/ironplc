# Function Local Re-initialization via Init Template Section

status: accepted
date: 2026-03-15

## Context and Problem Statement

IEC 61131-3 functions are stateless — local variables (VAR, return variable) must be re-initialized to their declared initial values (or type defaults) on every call. The VM currently does not re-initialize function locals between calls. Because functions use a flat variable table (ADR-0021), local variable slots retain stale values from the previous invocation.

```
FUNCTION accumulate : DINT
  VAR_INPUT a : DINT; END_VAR
  VAR counter : DINT := 10; END_VAR
  counter := counter + a;
  accumulate := counter;
END_FUNCTION

PROGRAM main
  VAR r1 : DINT; r2 : DINT; END_VAR
  r1 := accumulate(5);   (* Expected: 15, Actual: 15 ✓ *)
  r2 := accumulate(3);   (* Expected: 13, Actual: 18 ✗ — counter still 15 from first call *)
END_PROGRAM
```

How should the VM ensure function locals are re-initialized on each call?

## Decision Drivers

* **IEC 61131-3 correctness** — the standard requires functions to be stateless; locals must start fresh on every invocation
* **Performance** — function calls occur frequently in scan cycle code and must be fast
* **VM simplicity** — the VM targets embedded (no_std) environments; avoid dynamic allocation and complex logic
* **Container format impact** — prefer minimal, backward-compatible changes to the binary format
* **Codegen/VM consistency** — the mechanism for computing initial values at compile time must produce identical results to the runtime initialization used for program variables (ADR-0014)

## Considered Options

* **Option A**: Init template section — pre-compute initial Slot values at compile time and store them in a new container section; on each CALL the VM memcpys the template into the variable table
* **Option B**: Bytecode prologue — emit initialization bytecode (LOAD_CONST + STORE_VAR sequences) at the start of each function body; the function re-initializes its own locals every time it runs
* **Option C**: VM zero-fill — the VM unconditionally zeroes non-parameter local slots on each CALL, without consulting any template data

## Decision Outcome

Chosen option: "Option B — Bytecode prologue", because it reuses the existing `emit_initial_values()` / `compile_constant()` codegen path with zero risk of divergence between two constant-evaluation code paths. The prologue adds ~7-10 bytes of bytecode per local variable, but for typical IEC 61131-3 functions (5-20 locals) this overhead is negligible compared to scan cycle execution time. No container format changes are required, keeping the implementation simple and backward-compatible by default.

### Consequences

* Good, because no container format changes — all initialization is in the function's bytecode
* Good, because reuses the existing `compile_constant()` + `emit_truncation()` codegen path identically, with zero risk of divergence
* Good, because no VM changes — the CALL handler is unchanged; the prologue is just bytecode the function executes
* Good, because simpler overall — no new section, no new parsing logic in Container or ContainerRef
* Bad, because every function call interprets multiple bytecode instructions per local (LOAD_CONST + optional TRUNC + STORE_VAR = 7-10 bytes per local), which is slower than a memcpy
* Bad, because function bytecode size increases by ~7-10 bytes per local variable, even when the initial value is zero
* Neutral, because for functions with no non-parameter locals, the prologue is empty (only the return variable zero-init), adding minimal overhead

## Pros and Cons of the Options

### Option A: Init Template Section

Store pre-computed initial `Slot` values (u64 LE) for each function's non-parameter locals in a new container section. The CALL opcode handler reads the template and copies it into the variable table before executing the function body.

* Good, because the CALL handler does one memcpy instead of interpreting bytecode for each local
* Good, because non-zero initial values have zero additional runtime cost beyond the copy
* Good, because the template data is compact: exactly 8 bytes per non-parameter local, no opcode overhead
* Good, because backward-compatible — the new header fields fall in the previously-zeroed reserved region
* Bad, because requires a new container section (init_template) with its own directory and data blob
* Bad, because codegen must evaluate constants to raw Slot values at compile time, duplicating some logic from `compile_constant()` and `emit_truncation()`

### Option B: Bytecode Prologue

Emit `LOAD_CONST` + `TRUNC` (if narrow) + `STORE_VAR` instructions at the start of each function's bytecode, before the function body. The existing `emit_initial_values()` codegen helper can be reused directly.

* Good, because no container format changes — all initialization is in the function's bytecode
* Good, because reuses the existing `emit_initial_values()` codegen path identically, with zero risk of divergence
* Good, because simpler overall — no new section, no new parsing logic in Container or ContainerRef
* Bad, because every function call must interpret multiple bytecode instructions per local (LOAD_CONST + optional TRUNC + STORE_VAR = 7-10 bytes per local), which is slower than a memcpy
* Bad, because function bytecode size increases by ~7-10 bytes per local variable, even when the initial value is zero
* Neutral, because for zero-initialized locals the prologue still emits LOAD_CONST(0) + STORE_VAR, which is unnecessary work compared to a memcpy of zeros

### Option C: VM Zero-Fill

The CALL handler unconditionally zeroes all non-parameter local slots (sets them to `Slot::default()` / 0u64) before executing the function body. Non-zero initial values (`VAR x : INT := 42;`) would require a bytecode prologue in addition to the zero-fill.

* Good, because zero-fill is trivial to implement — no container format changes, no codegen changes for zero-initialized locals
* Good, because handles the common case (no initializer → default to zero) with minimal complexity
* Bad, because non-zero initial values still require a bytecode prologue, making this a hybrid approach with two mechanisms
* Bad, because the hybrid approach is confusing: some initialization happens in the VM (zero-fill) and some in bytecode (non-zero values), splitting responsibility across layers
* Bad, because it is slower than Option A for functions with non-zero initial values (zero-fill + bytecode prologue vs. single memcpy)

## More Information

### Container Format Changes

The 256-byte file header has 38 reserved bytes at positions 218-255. This change carves 8 bytes:

```
bytes 218-221: init_template_offset  (u32 LE)
bytes 222-225: init_template_size    (u32 LE)
bytes 226-255: reserved             (30 bytes, shrunk from 38)
```

The init template section layout:

```
┌────────────────────────────────────────────────┐
│ Directory (num_functions × 8 bytes)            │
│   template_offset: u32, template_size: u32     │
├────────────────────────────────────────────────┤
│ Data blob (concatenated u64 LE Slot values)    │
└────────────────────────────────────────────────┘
```

### Mitigating the Divergence Risk

The main risk with Option A is that `compute_initial_slot_value()` must produce the same bit patterns as `compile_constant()` + `emit_truncation()`. To mitigate this:

1. Extract truncation mask logic into a shared helper used by both code paths
2. Add end-to-end tests that verify a function called twice with non-zero initial values produces identical results both times

### Relationship to ADR-0014

ADR-0014 established the separate init function for program-level variable initialization (run once at startup). This ADR addresses a different problem: per-call re-initialization of function locals. The two mechanisms coexist — the init function handles program variables, and the init template handles function locals.
