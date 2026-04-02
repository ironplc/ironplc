# VM Review: Rust Interpreter Best Practices Applied to IronPLC

This document reviews IronPLC's VM implementation against best practices from
the *Rust Hosted Languages* book (rust-hosted-langs.github.io/book/) and
general VM/interpreter engineering knowledge. It identifies strengths, areas
for improvement, and concrete recommendations.

---

## Summary of Book's Key Practices

The book covers building a dynamic-language interpreter in Rust with:

1. **Tagged pointer value representation** — pack type tags into pointer bits to
   avoid per-value heap allocation; fall back to heap for large values.
2. **Arena/region-based memory management** — use typed arenas so objects of the
   same type are contiguous in memory, improving cache locality and simplifying
   lifetime management.
3. **Fixed-width 32-bit instruction encoding** — encode opcode + register
   operands + immediate into a single `u32`, enabling O(1) decode and better
   cache behavior vs variable-length encoding.
4. **Register-based VM** over stack-based — reduces instruction count,
   allows multi-operand instructions, and avoids constant push/pop overhead.
5. **Dual stack design** — separate the operand/register stack from the call
   frame stack so frames don't share space with values.
6. **Iterative dispatch loop** with `match` — avoid Rust-recursive function
   calls for call/return; manage a return-address stack explicitly.
7. **Careful use of `unsafe`** — the book accepts targeted unsafe for tagged
   pointers and arena indexing with thorough safe wrappers around it.
8. **Upvalue/closure mechanism** — for languages with closures, capture by
   reference with explicit close-over semantics.

---

## What IronPLC Already Does Well

### 1. Zero Unsafe Code
The VM, codegen, and container crates contain **no `unsafe` blocks**. All
bounds checking uses safe Rust idioms (`.get()`, `ok_or()`, `Result`). This is
an excellent choice for a PLC runtime where correctness and safety are
paramount.

### 2. Clean Type-State Machine for VM Lifecycle
The `Vm → VmReady → VmRunning → VmStopped | VmFaulted` progression uses Rust's
type system to enforce valid transitions at compile time. This is a textbook
Rust pattern that the book doesn't cover but which is superior to runtime state
checks.

### 3. Deterministic, No-GC Memory Model
IronPLC's target (IEC 61131-3 PLC programs) doesn't need garbage collection.
The fixed-size variable table, pre-allocated data region, and bump-allocated
temp buffers are appropriate for a deterministic real-time system. The book's
GC/arena patterns solve a different problem (dynamic languages with
heap-allocated objects).

### 4. Comprehensive Trap/Error System
The `Trap` enum with structured error codes (V-codes), distinct exit codes for
user errors vs internal errors, and `FaultContext` providing task/instance
context is well-designed. The book recommends similar structured error handling.

### 5. Caller-Provided Buffers (No-Alloc Execution)
The VM borrows all buffers from the caller (`stack_buf`, `var_buf`,
`data_region_buf`, `temp_buf`). This makes the VM `no_std`-compatible and
gives the embedder full control over memory — excellent for embedded/PLC
targets.

### 6. Good Use of Macros for Opcode Dispatch
The `binop!`, `cmpop!`, `unaryop!`, `checked_divop!`, and `load_const!` macros
reduce boilerplate in the execute loop while keeping the generated code
inlined. The book uses a similar approach with match arms.

### 7. Compile-Time Stack Depth Tracking
The `Emitter` tracks `current_stack_depth` and `max_stack_depth` during code
generation, so the container header can pre-allocate the exact stack size
needed. This is a best practice from production VMs.

### 8. Separation of Concerns Across Crates
The `container` (binary format), `codegen` (AST → bytecode), and `vm`
(execution) crates have clean boundaries. The container format is independently
serializable/deserializable, enabling offline tooling.

---

## Areas for Improvement

### CRITICAL: Recursive `execute()` for CALL Opcode

**Current:** `vm.rs:789` — the `CALL` opcode recursively calls `execute()`,
using Rust's native call stack for function return.

**Problem:** This is explicitly flagged with a TODO in the code. Each nested
call consumes Rust stack frames (~hundreds of bytes each). Deep IEC 61131-3
call chains (e.g., function blocks calling functions calling other FBs) can
overflow Rust's thread stack, and the depth limit is platform-dependent and
invisible to the user.

**Book's approach:** The book uses an explicit call frame stack and iterative
dispatch loop. On CALL, push a `CallFrame` (return PC + stack base); on RET,
pop the frame and restore PC.

**Recommendation:** Refactor `execute()` to maintain a `Vec<CallFrame>` (or
caller-provided `&mut [CallFrame]`). The main loop becomes:

```rust
struct CallFrame {
    bytecode_offset: usize,  // into the code section
    pc: usize,
    scope: VariableScope,
}

loop {
    let op = bytecode[frame.pc];
    frame.pc += 1;
    match op {
        CALL => {
            frames.push(current_frame);
            current_frame = new_frame_for_callee;
        }
        RET | RET_VOID => {
            current_frame = frames.pop().ok_or(Trap::StackUnderflow)?;
        }
        ...
    }
}
```

**Priority:** High — this is the single most impactful architectural change.

---

### Consider Fixed-Width Instruction Encoding

**Current:** Variable-length encoding: 1-byte opcode + 0/2/4 byte operands
depending on opcode.

**Book's approach:** 32-bit fixed-width instructions packing opcode + operands
into a single `u32`. Benefits:
- Every instruction is exactly 4 bytes → `pc` is always a simple index into a
  `[u32]` slice, not a byte stream
- Decode is a single load + bit-shift, no conditional byte reads
- Better instruction cache behavior (aligned, uniform access)
- Jump offsets become instruction-count-based, not byte-offset-based

**Trade-off:** Fixed-width wastes space on simple opcodes (e.g., `LOAD_TRUE`
only needs 1 byte today, would use 4). For PLC programs this is negligible.

**Recommendation:** Evaluate migration to 32-bit fixed-width encoding. This
would simplify `read_u16_le`/`read_u32_le`/`read_i16_le` helpers into simple
field extraction from `u32`. The opcode could use 8 bits, leaving 24 bits for
up to 3 register/operand fields.

**Priority:** Medium — improves performance and simplifies code, but the
current encoding works correctly.

---

### The `execute()` Function is Too Large

**Current:** `vm.rs` `execute()` is a single function spanning ~800+ lines
with a massive `match` expression.

**Problem:** Large functions hurt compiler optimization (LLVM may not inline or
optimize a function this large) and are harder to maintain. The string opcodes
alone account for ~500 lines of nearly identical buffer-management code.

**Recommendation:**
1. **Extract string operations** into a dedicated `string_ops.rs` module with
   helper functions like `str_replace()`, `str_insert()`, `str_delete()`,
   `str_left()`, `str_right()`, `str_mid()`, `str_concat()`. Each string
   opcode in the match arm would become a one-line call.
2. **Extract FB dispatch** (`FB_LOAD_INSTANCE`, `FB_STORE_PARAM`, etc.) into
   a `fb_ops.rs` module.
3. Keep arithmetic, comparison, and control flow in the main match since
   they're concise (one line each via macros).

**Priority:** Medium — maintainability improvement.

---

### String Operation Code Duplication

**Current:** Every string opcode (LEFT_STR, RIGHT_STR, MID_STR, DELETE_STR,
INSERT_STR, REPLACE_STR, CONCAT_STR) contains nearly identical code for:
- Reading string headers from the data region
- Allocating temp buffers via the bump allocator
- Writing headers to temp buffers
- Bounds checking

**Recommendation:** Extract shared helpers:

```rust
fn str_read(data_region: &[u8], offset: usize) -> Result<(u16, u16, &[u8]), Trap> {
    // Returns (max_len, cur_len, data_bytes)
}

fn str_alloc_and_write(
    temp_buf: &mut [u8],
    next_temp_buf: &mut u16,
    max_temp_buf_bytes: usize,
    data: &[u8],
) -> Result<usize, Trap> {
    // Returns buf_idx
}
```

The `str_alloc_temp` helper already exists but only handles allocation. A
higher-level helper that also writes the result would cut each string opcode
to ~10 lines.

**Priority:** Medium — reduces ~500 lines of near-duplicate code.

---

### Temp Buffer Bump Allocator Can Silently Wrap

**Current:** `next_temp_buf` is a `u16` that wraps via `wrapping_add(1)`.
If more temp buffers are allocated than exist, the index wraps silently and
overwrites a previously allocated buffer.

**Problem:** For complex string expressions with many intermediate values
(e.g., `CONCAT(CONCAT(CONCAT(a, b), c), d)`), the bump allocator could wrap
and corrupt an earlier temp buffer that's still logically live.

**Book's approach:** Arenas with explicit capacity tracking; allocation failure
triggers GC or a clear error.

**Recommendation:** Add a check when allocating:

```rust
fn str_alloc_temp(...) -> Result<(usize, usize), Trap> {
    let buf_idx = next_temp_buf;
    let buf_start = buf_idx as usize * max_temp_buf_bytes;
    if buf_start + max_temp_buf_bytes > temp_buf_total_len {
        return Err(Trap::TempBufferExhausted);
    }
    *next_temp_buf += 1;  // don't wrap — let the bounds check catch it
    ...
}
```

The existing `str_alloc_temp` function does check bounds, but the many
manually-inlined allocations in the string opcodes (REPLACE_STR, INSERT_STR,
DELETE_STR, etc.) duplicate the logic and use `wrapping_add` directly. If all
string opcodes used the shared `str_alloc_temp` helper, this would be solved.

**Priority:** Medium — correctness issue for complex string expressions.

---

### `Vm::load()` Takes Too Many Arguments

**Current:** `Vm::load()` accepts 8 separate mutable slice parameters
(`stack_buf`, `var_buf`, `data_region_buf`, `temp_buf`, `task_states`,
`program_instances`, `ready_buf`). The function has `#[allow(clippy::too_many_arguments)]`.

**Recommendation:** Consolidate into the existing `VmBuffers` struct:

```rust
impl Vm {
    pub fn load<'a>(self, container: &'a Container, bufs: &'a mut VmBuffers) -> VmReady<'a> {
        // Destructure bufs into individual slices
    }
}
```

`VmBuffers` already exists in `buffers.rs` and has all the needed fields.
Making it the primary API would eliminate the clippy suppression and simplify
the call sites.

**Priority:** Low — API ergonomics improvement.

---

### `Slot` Type Safety Could Be Stronger

**Current:** `Slot(u64)` can be interpreted as any type via `as_i32()`,
`as_f32()`, etc. with no runtime type checking. The book's tagged-pointer
approach uses tag bits so the runtime can verify types.

**IronPLC's situation:** This is actually appropriate for a PLC VM. IEC 61131-3
is statically typed, so the codegen guarantees correct types at each stack
position. Adding runtime type tags would add overhead with no benefit since
the compiler already ensures type correctness.

However, for **debug builds**, consider adding debug assertions:

```rust
#[cfg(debug_assertions)]
pub struct Slot { value: u64, debug_type: SlotType }
```

This would catch codegen bugs during testing without affecting release
performance.

**Priority:** Low — defense-in-depth for development.

---

### No Bytecode Verification Pass

**Current:** Bytecode is trusted at load time. Invalid bytecode (wrong operand
counts, stack imbalance, out-of-range jumps) is caught at runtime via traps.

**Book's approach:** The book doesn't cover this explicitly, but production VMs
(JVM, CLR, WASM) include a verification pass at load time that statically
checks:
- Stack balance (every path through a basic block has the same stack depth)
- Jump targets land on valid instruction boundaries
- Variable indices are in-range
- Constant pool indices are valid
- Type consistency (if tracked)

**Recommendation:** Add an optional `Container::verify()` method that walks the
bytecode and checks structural validity. This would catch codegen bugs early
(especially valuable during development) and produce clearer error messages
than runtime traps.

**Priority:** Low-Medium — valuable for development and diagnostics.

---

### Watchdog Timer is a Stub

**Current:** `vm.rs:327-329` comments indicate the watchdog check uses 0
elapsed time, so it never fires. The TODO references "Phase 4."

**Recommendation:** For production use, implement real elapsed-time tracking.
The caller already provides `current_time_us`, so recording wall-clock time
before and after each task's execution would enable watchdog enforcement:

```rust
let start = current_time_us;
// ... execute task ...
let elapsed = current_time_us_after - start;
if elapsed > task.watchdog_us && task.watchdog_us > 0 {
    return Err(FaultContext { trap: Trap::WatchdogTimeout(task_id), ... });
}
```

**Priority:** Medium — required for production PLC safety.

---

### Variable Scope Check Not Applied in All Paths

**Current:** `scope.check_access()` is called for `LOAD_VAR_*` and
`STORE_VAR_*` opcodes, and for indirect loads/stores. However, the CALL
opcode writes parameters directly via `variables.store(VarIndex::new(var_offset + i), val)`
at line 782 without a scope check.

**Recommendation:** Either:
- Add scope validation for the callee's variable range during CALL, or
- Document that CALL parameter writes are intentionally unchecked because the
  codegen guarantees valid offsets.

**Priority:** Low — the codegen controls the offsets, but defense-in-depth is
valuable.

---

### MUX Built-in Allocates on the Heap

**Current:** `dispatch_mux_i32` and friends use `vec![0i32; n]` to hold
intermediate values popped from the stack before indexing.

**Problem:** This heap-allocates per MUX call. For a no-alloc VM, this is a
policy violation. In a hot loop calling MUX repeatedly, this creates allocation
pressure.

**Recommendation:** Use a fixed-size array on the stack (MUX arity is bounded
by what fits in the stack):

```rust
fn dispatch_mux_i32(n: usize, stack: &mut OperandStack) -> Result<(), Trap> {
    const MAX_MUX: usize = 256;
    let mut inputs = [0i32; MAX_MUX];
    assert!(n <= MAX_MUX);
    for i in (0..n).rev() {
        inputs[i] = stack.pop()?.as_i32();
    }
    ...
}
```

Or pass in a caller-provided scratch buffer.

**Priority:** Low — MUX is rarely used in hot loops, but it violates the
no-alloc design goal.

---

### Byte-Level String Copy Loops vs `copy_from_slice`

**Current:** Several string opcodes use byte-by-byte copy loops:
```rust
for i in 0..prefix_copy {
    temp_buf[data_start + write_pos] = data_region[in1_start + i];
    write_pos += 1;
}
```

**Recommendation:** Replace with `copy_from_slice()` which the compiler can
optimize to `memcpy`:
```rust
temp_buf[data_start + write_pos..data_start + write_pos + prefix_copy]
    .copy_from_slice(&data_region[in1_start..in1_start + prefix_copy]);
write_pos += prefix_copy;
```

Some string opcodes already use `copy_from_slice` (e.g., LEFT_STR, RIGHT_STR)
while others use manual loops (REPLACE_STR, INSERT_STR, DELETE_STR). This
should be made consistent.

**Priority:** Low — performance improvement, consistency fix.

---

## Improvement Priority Summary

| Priority | Item | Impact |
|----------|------|--------|
| **High** | Iterative call frame stack (replace recursive execute) | Eliminates unbounded Rust stack usage |
| **Medium** | Extract string operations to helpers/module | ~500 lines of duplication removed |
| **Medium** | Fix temp buffer wrapping in manually-inlined allocations | Correctness for complex string expressions |
| **Medium** | Implement watchdog timer | Required for production PLC safety |
| **Medium** | Consider fixed-width 32-bit instruction encoding | Performance + simplicity |
| **Medium** | Bytecode verification pass | Better error messages, catches codegen bugs |
| **Low** | Consolidate `Vm::load()` parameters into `VmBuffers` | API ergonomics |
| **Low** | Debug-mode type tags in `Slot` | Catches codegen bugs during development |
| **Low** | Remove heap allocation from MUX dispatch | No-alloc consistency |
| **Low** | Use `copy_from_slice` in all string ops | Performance consistency |
| **Low** | Scope check on CALL parameter writes | Defense-in-depth |

---

## Items Not Recommended to Change

1. **Stack-based VM → Register-based:** The book advocates register-based VMs,
   but IronPLC's stack machine is appropriate. IEC 61131-3 expression semantics
   map naturally to a stack machine. The simpler codegen and smaller instruction
   encoding are worth the trade-off for a PLC runtime.

2. **Tagged pointers / NaN boxing:** The book uses tagged pointers for dynamic
   typing. IronPLC's untagged `Slot(u64)` is correct for a statically-typed
   language — adding tags would waste cycles on a system with compile-time type
   guarantees.

3. **Garbage collection:** Not needed. IEC 61131-3 has no dynamic allocation
   semantics. The current pre-allocated model is ideal.

4. **Unsafe code for performance:** The book accepts unsafe for tagged pointers
   and arena access. IronPLC's zero-unsafe approach is a strength for a
   safety-critical target. Don't introduce unsafe without clear benchmarked
   justification.
