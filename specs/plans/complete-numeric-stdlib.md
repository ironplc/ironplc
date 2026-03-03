# Complete Numeric Standard Library Functions — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make ABS, MIN, MAX, LIMIT, SEL, and EXPT fully supported across all IEC 61131-3 integer types (SINT, INT, DINT, LINT, USINT, UINT, UDINT, ULINT) plus existing REAL/LREAL support. Update documentation to say "Supported" for every variant.

**Architecture:** Add 12 new builtin opcode constants (6 I64 signed, 3 U32 unsigned, 3 U64 unsigned), implement their VM dispatch handlers, update codegen's `lookup_builtin` to accept `Signedness` and route to the correct variant, and fix `emit_pow` for W64. SQRT is already complete (REAL/LREAL only).

**Tech Stack:** Rust (compiler crates: container, codegen, vm), reStructuredText (docs)

---

## Key Context

### File Locations
- Opcode constants: `compiler/container/src/opcode.rs` (builtin module, lines 399-515)
- VM dispatch: `compiler/vm/src/builtin.rs` (dispatch function, lines 17-218)
- Codegen routing: `compiler/codegen/src/compile.rs` (`lookup_builtin` lines 958-997, `emit_pow` lines 1598-1605)
- VM tests: `compiler/vm/tests/execute_builtin_*.rs`
- Codegen tests: `compiler/codegen/tests/end_to_end_*.rs` and `compile_*.rs`
- Docs: `docs/reference/standard-library/functions/*.rst`

### How the Stack-Based Builtins Work
1. Codegen compiles each argument expression, pushing values onto the operand stack
2. Codegen emits `BUILTIN <func_id>` (opcode 0xC4 + 2-byte LE function ID)
3. VM `builtin::dispatch(func_id, stack)` pops args, computes, pushes result
4. `Slot` is a 64-bit union: `from_i32`/`as_i32` sign-extends, `from_i64`/`as_i64` stores directly

### Type Width Mapping
- SINT, INT, DINT, USINT, UINT, UDINT → `OpWidth::W32` (stored as i32 in Slot)
- LINT, ULINT → `OpWidth::W64` (stored as i64 in Slot)
- REAL → `OpWidth::F32`, LREAL → `OpWidth::F64`

### Why Unsigned Variants Are Needed
- USINT (0-255), UINT (0-65535): values always fit in i32's positive range, so signed MIN/MAX/LIMIT give correct results — no new opcodes needed
- UDINT (0-4294967295): values ≥ 2^31 appear negative when stored as i32, so signed comparison gives wrong results — needs U32 variants
- ULINT: same issue at 64-bit — needs U64 variants

### New Opcodes (12 total)

| ID       | Name       | Args | Purpose                    |
|----------|------------|------|----------------------------|
| 0x0360   | EXPT_I64   | 2    | LINT exponentiation        |
| 0x0361   | ABS_I64    | 1    | LINT absolute value        |
| 0x0362   | MIN_I64    | 2    | LINT minimum (signed)      |
| 0x0363   | MAX_I64    | 2    | LINT maximum (signed)      |
| 0x0364   | LIMIT_I64  | 3    | LINT clamp (signed)        |
| 0x0365   | SEL_I64    | 3    | LINT/ULINT binary select   |
| 0x0366   | MIN_U32    | 2    | UDINT minimum (unsigned)   |
| 0x0367   | MAX_U32    | 2    | UDINT maximum (unsigned)   |
| 0x0368   | LIMIT_U32  | 3    | UDINT clamp (unsigned)     |
| 0x0369   | MIN_U64    | 2    | ULINT minimum (unsigned)   |
| 0x036A   | MAX_U64    | 2    | ULINT maximum (unsigned)   |
| 0x036B   | LIMIT_U64  | 3    | ULINT clamp (unsigned)     |

SEL_I64 handles both LINT and ULINT — the select operation is the same regardless of signedness.

ABS and EXPT don't need unsigned variants: ABS is identity for unsigned types (not in the spec), and EXPT isn't defined for unsigned types in IEC 61131-3.

---

### Task 1: Add Opcode Constants

**Files:**
- Modify: `compiler/container/src/opcode.rs:496-515`

**Step 1: Add the 12 new opcode constants**

After `SQRT_F64` (line 496), add:

```rust
/// EXPT for 64-bit integers: pops exponent (b) and base (a), pushes a ** b.
/// Traps on negative exponent.
pub const EXPT_I64: u16 = 0x0360;

/// ABS for 64-bit integers: pops one value, pushes its absolute value (wrapping).
pub const ABS_I64: u16 = 0x0361;

/// MIN for 64-bit signed integers: pops two values (b then a), pushes min(a, b).
pub const MIN_I64: u16 = 0x0362;

/// MAX for 64-bit signed integers: pops two values (b then a), pushes max(a, b).
pub const MAX_I64: u16 = 0x0363;

/// LIMIT for 64-bit signed integers: pops mx, in, mn, pushes clamp(in, mn, mx).
pub const LIMIT_I64: u16 = 0x0364;

/// SEL for 64-bit values: pops in1, in0 (i64), g (i32), pushes in0 if g==0 else in1.
pub const SEL_I64: u16 = 0x0365;

/// MIN for 32-bit unsigned integers: pops two values (b then a), pushes unsigned min.
pub const MIN_U32: u16 = 0x0366;

/// MAX for 32-bit unsigned integers: pops two values (b then a), pushes unsigned max.
pub const MAX_U32: u16 = 0x0367;

/// LIMIT for 32-bit unsigned integers: pops mx, in, mn, pushes unsigned clamp.
pub const LIMIT_U32: u16 = 0x0368;

/// MIN for 64-bit unsigned integers: pops two values (b then a), pushes unsigned min.
pub const MIN_U64: u16 = 0x0369;

/// MAX for 64-bit unsigned integers: pops two values (b then a), pushes unsigned max.
pub const MAX_U64: u16 = 0x036A;

/// LIMIT for 64-bit unsigned integers: pops mx, in, mn, pushes unsigned clamp.
pub const LIMIT_U64: u16 = 0x036B;
```

**Step 2: Update `arg_count` match arms**

In the `arg_count` function, add the new constants to existing arms:

```rust
pub fn arg_count(func_id: u16) -> u16 {
    match func_id {
        ABS_I32 | ABS_F32 | ABS_F64 | ABS_I64 | SQRT_F32 | SQRT_F64 => 1,
        EXPT_I32 | EXPT_F32 | EXPT_F64 | EXPT_I64
        | MIN_I32 | MIN_F32 | MIN_F64 | MIN_I64 | MIN_U32 | MIN_U64
        | MAX_I32 | MAX_F32 | MAX_F64 | MAX_I64 | MAX_U32 | MAX_U64
        | SHL_I32 | SHL_I64 | SHR_I32 | SHR_I64
        | ROL_I32 | ROL_I64 | ROR_I32 | ROR_I64
        | ROL_U8 | ROL_U16 | ROR_U8 | ROR_U16 => 2,
        LIMIT_I32 | LIMIT_F32 | LIMIT_F64 | LIMIT_I64 | LIMIT_U32 | LIMIT_U64
        | SEL_I32 | SEL_F32 | SEL_F64 | SEL_I64 => 3,
        _ => panic!("unknown builtin function ID: 0x{:04X}", func_id),
    }
}
```

**Step 3: Verify it compiles**

Run: `cd /workspaces/ironplc/compiler && cargo build -p ironplc-container`

**Step 4: Commit**

```bash
git add compiler/container/src/opcode.rs
git commit -m "feat: add opcode constants for I64/U32/U64 builtin variants"
```

---

### Task 2: Add I64 VM Dispatch Handlers + Tests

**Files:**
- Modify: `compiler/vm/src/builtin.rs:215-217` (before the `_ =>` catch-all)
- Create: `compiler/vm/tests/execute_builtin_abs_i64.rs`
- Create: `compiler/vm/tests/execute_builtin_min_i64.rs`
- Create: `compiler/vm/tests/execute_builtin_max_i64.rs`
- Create: `compiler/vm/tests/execute_builtin_limit_i64.rs`
- Create: `compiler/vm/tests/execute_builtin_sel_i64.rs`
- Create: `compiler/vm/tests/execute_builtin_expt_i64.rs`

**Step 1: Write VM test for ABS_I64**

Create `compiler/vm/tests/execute_builtin_abs_i64.rs`:

```rust
//! Integration tests for the BUILTIN ABS_I64 opcode.

mod common;

use common::VmBuffers;
use ironplc_container::ContainerBuilder;
use ironplc_vm::Vm;

#[test]
fn execute_when_abs_i64_positive_then_unchanged() {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x02, 0x00, 0x00,  // LOAD_CONST_I64 pool[0]
        0xC4, 0x61, 0x03,  // BUILTIN ABS_I64
        0x19, 0x00, 0x00,  // STORE_VAR_I64 var[0]
        0xB5,              // RET_VOID
    ];
    let c = ContainerBuilder::new()
        .num_variables(1)
        .add_i64_constant(42)
        .add_function(0, &bytecode, 16, 1)
        .build();
    let mut b = VmBuffers::from_container(&c);
    let mut vm = Vm::new()
        .load(&c, &mut b.stack, &mut b.vars, &mut b.tasks, &mut b.programs, &mut b.ready)
        .start();
    vm.run_round(0).unwrap();
    vm.stop();
    assert_eq!(b.vars[0].as_i64(), 42);
}

#[test]
fn execute_when_abs_i64_negative_then_positive() {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x02, 0x00, 0x00,  // LOAD_CONST_I64 pool[0]
        0xC4, 0x61, 0x03,  // BUILTIN ABS_I64
        0x19, 0x00, 0x00,  // STORE_VAR_I64 var[0]
        0xB5,              // RET_VOID
    ];
    let c = ContainerBuilder::new()
        .num_variables(1)
        .add_i64_constant(-7_000_000_000)
        .add_function(0, &bytecode, 16, 1)
        .build();
    let mut b = VmBuffers::from_container(&c);
    let mut vm = Vm::new()
        .load(&c, &mut b.stack, &mut b.vars, &mut b.tasks, &mut b.programs, &mut b.ready)
        .start();
    vm.run_round(0).unwrap();
    vm.stop();
    assert_eq!(b.vars[0].as_i64(), 7_000_000_000);
}

#[test]
fn execute_when_abs_i64_min_then_wraps() {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x02, 0x00, 0x00,  // LOAD_CONST_I64 pool[0]
        0xC4, 0x61, 0x03,  // BUILTIN ABS_I64
        0x19, 0x00, 0x00,  // STORE_VAR_I64 var[0]
        0xB5,              // RET_VOID
    ];
    let c = ContainerBuilder::new()
        .num_variables(1)
        .add_i64_constant(i64::MIN)
        .add_function(0, &bytecode, 16, 1)
        .build();
    let mut b = VmBuffers::from_container(&c);
    let mut vm = Vm::new()
        .load(&c, &mut b.stack, &mut b.vars, &mut b.tasks, &mut b.programs, &mut b.ready)
        .start();
    vm.run_round(0).unwrap();
    vm.stop();
    assert_eq!(b.vars[0].as_i64(), i64::MIN);
}
```

**Step 2: Write VM tests for MIN_I64, MAX_I64, LIMIT_I64, SEL_I64, EXPT_I64**

Create `compiler/vm/tests/execute_builtin_min_i64.rs`:

```rust
//! Integration tests for the BUILTIN MIN_I64 opcode.

mod common;

use common::VmBuffers;
use ironplc_container::ContainerBuilder;
use ironplc_vm::Vm;

#[test]
fn execute_when_min_i64_first_smaller_then_returns_first() {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x02, 0x00, 0x00,  // LOAD_CONST_I64 pool[0] (a = -5_000_000_000)
        0x02, 0x01, 0x00,  // LOAD_CONST_I64 pool[1] (b = 3_000_000_000)
        0xC4, 0x62, 0x03,  // BUILTIN MIN_I64
        0x19, 0x00, 0x00,  // STORE_VAR_I64 var[0]
        0xB5,              // RET_VOID
    ];
    let c = ContainerBuilder::new()
        .num_variables(1)
        .add_i64_constant(-5_000_000_000)
        .add_i64_constant(3_000_000_000)
        .add_function(0, &bytecode, 16, 1)
        .build();
    let mut b = VmBuffers::from_container(&c);
    let mut vm = Vm::new()
        .load(&c, &mut b.stack, &mut b.vars, &mut b.tasks, &mut b.programs, &mut b.ready)
        .start();
    vm.run_round(0).unwrap();
    vm.stop();
    assert_eq!(b.vars[0].as_i64(), -5_000_000_000);
}

#[test]
fn execute_when_min_i64_second_smaller_then_returns_second() {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x02, 0x00, 0x00,
        0x02, 0x01, 0x00,
        0xC4, 0x62, 0x03,  // BUILTIN MIN_I64
        0x19, 0x00, 0x00,
        0xB5,
    ];
    let c = ContainerBuilder::new()
        .num_variables(1)
        .add_i64_constant(10_000_000_000)
        .add_i64_constant(5_000_000_000)
        .add_function(0, &bytecode, 16, 1)
        .build();
    let mut b = VmBuffers::from_container(&c);
    let mut vm = Vm::new()
        .load(&c, &mut b.stack, &mut b.vars, &mut b.tasks, &mut b.programs, &mut b.ready)
        .start();
    vm.run_round(0).unwrap();
    vm.stop();
    assert_eq!(b.vars[0].as_i64(), 5_000_000_000);
}
```

Create `compiler/vm/tests/execute_builtin_max_i64.rs`:

```rust
//! Integration tests for the BUILTIN MAX_I64 opcode.

mod common;

use common::VmBuffers;
use ironplc_container::ContainerBuilder;
use ironplc_vm::Vm;

#[test]
fn execute_when_max_i64_first_larger_then_returns_first() {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x02, 0x00, 0x00,
        0x02, 0x01, 0x00,
        0xC4, 0x63, 0x03,  // BUILTIN MAX_I64
        0x19, 0x00, 0x00,
        0xB5,
    ];
    let c = ContainerBuilder::new()
        .num_variables(1)
        .add_i64_constant(10_000_000_000)
        .add_i64_constant(5_000_000_000)
        .add_function(0, &bytecode, 16, 1)
        .build();
    let mut b = VmBuffers::from_container(&c);
    let mut vm = Vm::new()
        .load(&c, &mut b.stack, &mut b.vars, &mut b.tasks, &mut b.programs, &mut b.ready)
        .start();
    vm.run_round(0).unwrap();
    vm.stop();
    assert_eq!(b.vars[0].as_i64(), 10_000_000_000);
}
```

Create `compiler/vm/tests/execute_builtin_limit_i64.rs`:

```rust
//! Integration tests for the BUILTIN LIMIT_I64 opcode.

mod common;

use common::VmBuffers;
use ironplc_container::ContainerBuilder;
use ironplc_vm::Vm;

#[test]
fn execute_when_limit_i64_in_range_then_unchanged() {
    // LIMIT(-10B, 5B, 10B) = 5B  (all values in billions)
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x02, 0x00, 0x00,  // LOAD_CONST_I64 pool[0] (mn = -10B)
        0x02, 0x01, 0x00,  // LOAD_CONST_I64 pool[1] (in = 5B)
        0x02, 0x02, 0x00,  // LOAD_CONST_I64 pool[2] (mx = 10B)
        0xC4, 0x64, 0x03,  // BUILTIN LIMIT_I64
        0x19, 0x00, 0x00,  // STORE_VAR_I64 var[0]
        0xB5,              // RET_VOID
    ];
    let c = ContainerBuilder::new()
        .num_variables(1)
        .add_i64_constant(-10_000_000_000)
        .add_i64_constant(5_000_000_000)
        .add_i64_constant(10_000_000_000)
        .add_function(0, &bytecode, 16, 1)
        .build();
    let mut b = VmBuffers::from_container(&c);
    let mut vm = Vm::new()
        .load(&c, &mut b.stack, &mut b.vars, &mut b.tasks, &mut b.programs, &mut b.ready)
        .start();
    vm.run_round(0).unwrap();
    vm.stop();
    assert_eq!(b.vars[0].as_i64(), 5_000_000_000);
}

#[test]
fn execute_when_limit_i64_below_min_then_clamped() {
    // LIMIT(0, -5B, 10B) = 0
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x02, 0x00, 0x00,
        0x02, 0x01, 0x00,
        0x02, 0x02, 0x00,
        0xC4, 0x64, 0x03,  // BUILTIN LIMIT_I64
        0x19, 0x00, 0x00,
        0xB5,
    ];
    let c = ContainerBuilder::new()
        .num_variables(1)
        .add_i64_constant(0)
        .add_i64_constant(-5_000_000_000)
        .add_i64_constant(10_000_000_000)
        .add_function(0, &bytecode, 16, 1)
        .build();
    let mut b = VmBuffers::from_container(&c);
    let mut vm = Vm::new()
        .load(&c, &mut b.stack, &mut b.vars, &mut b.tasks, &mut b.programs, &mut b.ready)
        .start();
    vm.run_round(0).unwrap();
    vm.stop();
    assert_eq!(b.vars[0].as_i64(), 0);
}
```

Create `compiler/vm/tests/execute_builtin_sel_i64.rs`:

```rust
//! Integration tests for the BUILTIN SEL_I64 opcode.

mod common;

use common::VmBuffers;
use ironplc_container::ContainerBuilder;
use ironplc_vm::Vm;

#[test]
fn execute_when_sel_i64_false_then_returns_in0() {
    // SEL(FALSE, 5B, 10B) = 5B
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (g = 0 = FALSE)
        0x02, 0x01, 0x00,  // LOAD_CONST_I64 pool[1] (in0 = 5B)
        0x02, 0x02, 0x00,  // LOAD_CONST_I64 pool[2] (in1 = 10B)
        0xC4, 0x65, 0x03,  // BUILTIN SEL_I64
        0x19, 0x00, 0x00,  // STORE_VAR_I64 var[0]
        0xB5,              // RET_VOID
    ];
    let c = ContainerBuilder::new()
        .num_variables(1)
        .add_i32_constant(0)
        .add_i64_constant(5_000_000_000)
        .add_i64_constant(10_000_000_000)
        .add_function(0, &bytecode, 16, 1)
        .build();
    let mut b = VmBuffers::from_container(&c);
    let mut vm = Vm::new()
        .load(&c, &mut b.stack, &mut b.vars, &mut b.tasks, &mut b.programs, &mut b.ready)
        .start();
    vm.run_round(0).unwrap();
    vm.stop();
    assert_eq!(b.vars[0].as_i64(), 5_000_000_000);
}

#[test]
fn execute_when_sel_i64_true_then_returns_in1() {
    // SEL(TRUE, 5B, 10B) = 10B
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (g = 1 = TRUE)
        0x02, 0x01, 0x00,  // LOAD_CONST_I64 pool[1] (in0 = 5B)
        0x02, 0x02, 0x00,  // LOAD_CONST_I64 pool[2] (in1 = 10B)
        0xC4, 0x65, 0x03,  // BUILTIN SEL_I64
        0x19, 0x00, 0x00,  // STORE_VAR_I64 var[0]
        0xB5,              // RET_VOID
    ];
    let c = ContainerBuilder::new()
        .num_variables(1)
        .add_i32_constant(1)
        .add_i64_constant(5_000_000_000)
        .add_i64_constant(10_000_000_000)
        .add_function(0, &bytecode, 16, 1)
        .build();
    let mut b = VmBuffers::from_container(&c);
    let mut vm = Vm::new()
        .load(&c, &mut b.stack, &mut b.vars, &mut b.tasks, &mut b.programs, &mut b.ready)
        .start();
    vm.run_round(0).unwrap();
    vm.stop();
    assert_eq!(b.vars[0].as_i64(), 10_000_000_000);
}
```

Create `compiler/vm/tests/execute_builtin_expt_i64.rs`:

```rust
//! Integration tests for the BUILTIN EXPT_I64 opcode.

mod common;

use common::{assert_trap, VmBuffers};
use ironplc_container::ContainerBuilder;
use ironplc_vm::error::Trap;
use ironplc_vm::Vm;

#[test]
fn execute_when_expt_i64_then_correct() {
    // EXPT(2, 40) = 1099511627776
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x02, 0x00, 0x00,  // LOAD_CONST_I64 pool[0] (base = 2)
        0x02, 0x01, 0x00,  // LOAD_CONST_I64 pool[1] (exp = 40)
        0xC4, 0x60, 0x03,  // BUILTIN EXPT_I64
        0x19, 0x00, 0x00,  // STORE_VAR_I64 var[0]
        0xB5,              // RET_VOID
    ];
    let c = ContainerBuilder::new()
        .num_variables(1)
        .add_i64_constant(2)
        .add_i64_constant(40)
        .add_function(0, &bytecode, 16, 1)
        .build();
    let mut b = VmBuffers::from_container(&c);
    let mut vm = Vm::new()
        .load(&c, &mut b.stack, &mut b.vars, &mut b.tasks, &mut b.programs, &mut b.ready)
        .start();
    vm.run_round(0).unwrap();
    vm.stop();
    assert_eq!(b.vars[0].as_i64(), 1_099_511_627_776);
}

#[test]
fn execute_when_expt_i64_negative_exponent_then_traps() {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x02, 0x00, 0x00,  // LOAD_CONST_I64 pool[0] (base = 2)
        0x02, 0x01, 0x00,  // LOAD_CONST_I64 pool[1] (exp = -1)
        0xC4, 0x60, 0x03,  // BUILTIN EXPT_I64
        0x19, 0x00, 0x00,  // STORE_VAR_I64 var[0]
        0xB5,              // RET_VOID
    ];
    let c = ContainerBuilder::new()
        .num_variables(1)
        .add_i64_constant(2)
        .add_i64_constant(-1)
        .add_function(0, &bytecode, 16, 1)
        .build();
    let mut b = VmBuffers::from_container(&c);
    let mut vm = Vm::new()
        .load(&c, &mut b.stack, &mut b.vars, &mut b.tasks, &mut b.programs, &mut b.ready)
        .start();
    assert_trap(&mut vm, Trap::NegativeExponent);
}
```

**Step 3: Run VM tests to verify they fail**

Run: `cd /workspaces/ironplc/compiler && cargo test -p ironplc-vm execute_builtin_abs_i64`
Expected: FAIL with `InvalidBuiltinFunction`

**Step 4: Add I64 dispatch handlers to `vm/src/builtin.rs`**

Insert before the `_ => Err(Trap::InvalidBuiltinFunction(...))` line:

```rust
opcode::builtin::EXPT_I64 => {
    let b = stack.pop()?.as_i64();
    let a = stack.pop()?.as_i64();
    if b < 0 {
        return Err(Trap::NegativeExponent);
    }
    stack.push(Slot::from_i64(a.wrapping_pow(b as u32)))?;
    Ok(())
}
opcode::builtin::ABS_I64 => {
    let a = stack.pop()?.as_i64();
    stack.push(Slot::from_i64(a.wrapping_abs()))?;
    Ok(())
}
opcode::builtin::MIN_I64 => {
    let b = stack.pop()?.as_i64();
    let a = stack.pop()?.as_i64();
    stack.push(Slot::from_i64(a.min(b)))?;
    Ok(())
}
opcode::builtin::MAX_I64 => {
    let b = stack.pop()?.as_i64();
    let a = stack.pop()?.as_i64();
    stack.push(Slot::from_i64(a.max(b)))?;
    Ok(())
}
opcode::builtin::LIMIT_I64 => {
    let mx = stack.pop()?.as_i64();
    let in_val = stack.pop()?.as_i64();
    let mn = stack.pop()?.as_i64();
    stack.push(Slot::from_i64(in_val.clamp(mn, mx)))?;
    Ok(())
}
opcode::builtin::SEL_I64 => {
    let in1 = stack.pop()?.as_i64();
    let in0 = stack.pop()?.as_i64();
    let g = stack.pop()?.as_i32();
    stack.push(Slot::from_i64(if g == 0 { in0 } else { in1 }))?;
    Ok(())
}
```

**Step 5: Run VM tests to verify they pass**

Run: `cd /workspaces/ironplc/compiler && cargo test -p ironplc-vm execute_builtin_abs_i64 execute_builtin_min_i64 execute_builtin_max_i64 execute_builtin_limit_i64 execute_builtin_sel_i64 execute_builtin_expt_i64`
Expected: All PASS

**Step 6: Commit**

```bash
git add compiler/vm/src/builtin.rs compiler/vm/tests/execute_builtin_*_i64.rs
git commit -m "feat: add I64 VM dispatch handlers for LINT builtins"
```

---

### Task 3: Add U32/U64 VM Dispatch Handlers + Tests

**Files:**
- Modify: `compiler/vm/src/builtin.rs` (before `_ =>` catch-all)
- Create: `compiler/vm/tests/execute_builtin_min_u32.rs`
- Create: `compiler/vm/tests/execute_builtin_max_u32.rs`
- Create: `compiler/vm/tests/execute_builtin_limit_u32.rs`
- Create: `compiler/vm/tests/execute_builtin_min_u64.rs`
- Create: `compiler/vm/tests/execute_builtin_max_u64.rs`
- Create: `compiler/vm/tests/execute_builtin_limit_u64.rs`

**Step 1: Write VM test for MIN_U32**

The critical test case: values where signed comparison gives the wrong answer.
UDINT 3_000_000_000 stored as i32 = -1_294_967_296. Signed MIN would wrongly pick it.

Create `compiler/vm/tests/execute_builtin_min_u32.rs`:

```rust
//! Integration tests for the BUILTIN MIN_U32 opcode.

mod common;

use common::{single_function_container, VmBuffers};
use ironplc_vm::Vm;

#[test]
fn execute_when_min_u32_large_values_then_unsigned_comparison() {
    // MIN(3_000_000_000_u32, 1_000_000_000_u32) = 1_000_000_000
    // As i32: 3B = -1294967296, 1B = 1000000000
    // Signed min would wrongly return -1294967296 (3B unsigned).
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (3B as i32)
        0x01, 0x01, 0x00,  // LOAD_CONST_I32 pool[1] (1B as i32)
        0xC4, 0x66, 0x03,  // BUILTIN MIN_U32
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container(&bytecode, 1, &[3_000_000_000_u32 as i32, 1_000_000_000]);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = Vm::new()
        .load(&c, &mut b.stack, &mut b.vars, &mut b.tasks, &mut b.programs, &mut b.ready)
        .start();
    vm.run_round(0).unwrap();
    assert_eq!(vm.read_variable(0).unwrap() as u32, 1_000_000_000);
}

#[test]
fn execute_when_min_u32_both_large_then_smaller_unsigned() {
    // MIN(4_000_000_000_u32, 3_000_000_000_u32) = 3_000_000_000
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,
        0x01, 0x01, 0x00,
        0xC4, 0x66, 0x03,  // BUILTIN MIN_U32
        0x18, 0x00, 0x00,
        0xB5,
    ];
    let c = single_function_container(&bytecode, 1, &[4_000_000_000_u32 as i32, 3_000_000_000_u32 as i32]);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = Vm::new()
        .load(&c, &mut b.stack, &mut b.vars, &mut b.tasks, &mut b.programs, &mut b.ready)
        .start();
    vm.run_round(0).unwrap();
    assert_eq!(vm.read_variable(0).unwrap() as u32, 3_000_000_000);
}
```

Create similar tests for MAX_U32 (`execute_builtin_max_u32.rs`), LIMIT_U32 (`execute_builtin_limit_u32.rs`), MIN_U64 (`execute_builtin_min_u64.rs`), MAX_U64 (`execute_builtin_max_u64.rs`), LIMIT_U64 (`execute_builtin_limit_u64.rs`).

For MAX_U32:
```rust
//! Integration tests for the BUILTIN MAX_U32 opcode.

mod common;

use common::{single_function_container, VmBuffers};
use ironplc_vm::Vm;

#[test]
fn execute_when_max_u32_large_values_then_unsigned_comparison() {
    // MAX(3_000_000_000_u32, 1_000_000_000_u32) = 3_000_000_000
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,
        0x01, 0x01, 0x00,
        0xC4, 0x67, 0x03,  // BUILTIN MAX_U32
        0x18, 0x00, 0x00,
        0xB5,
    ];
    let c = single_function_container(&bytecode, 1, &[3_000_000_000_u32 as i32, 1_000_000_000]);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = Vm::new()
        .load(&c, &mut b.stack, &mut b.vars, &mut b.tasks, &mut b.programs, &mut b.ready)
        .start();
    vm.run_round(0).unwrap();
    assert_eq!(vm.read_variable(0).unwrap() as u32, 3_000_000_000);
}
```

For LIMIT_U32:
```rust
//! Integration tests for the BUILTIN LIMIT_U32 opcode.

mod common;

use common::{single_function_container, VmBuffers};
use ironplc_vm::Vm;

#[test]
fn execute_when_limit_u32_below_min_then_clamped() {
    // LIMIT(1_000_000_000, 500_000_000, 3_000_000_000) = 1_000_000_000
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // mn
        0x01, 0x01, 0x00,  // in
        0x01, 0x02, 0x00,  // mx
        0xC4, 0x68, 0x03,  // BUILTIN LIMIT_U32
        0x18, 0x00, 0x00,
        0xB5,
    ];
    let c = single_function_container(&bytecode, 1, &[1_000_000_000, 500_000_000, 3_000_000_000_u32 as i32]);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = Vm::new()
        .load(&c, &mut b.stack, &mut b.vars, &mut b.tasks, &mut b.programs, &mut b.ready)
        .start();
    vm.run_round(0).unwrap();
    assert_eq!(vm.read_variable(0).unwrap() as u32, 1_000_000_000);
}

#[test]
fn execute_when_limit_u32_in_range_then_unchanged() {
    // LIMIT(1B, 2B, 3B) = 2B
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,
        0x01, 0x01, 0x00,
        0x01, 0x02, 0x00,
        0xC4, 0x68, 0x03,  // BUILTIN LIMIT_U32
        0x18, 0x00, 0x00,
        0xB5,
    ];
    let c = single_function_container(&bytecode, 1, &[1_000_000_000, 2_000_000_000_u32 as i32, 3_000_000_000_u32 as i32]);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = Vm::new()
        .load(&c, &mut b.stack, &mut b.vars, &mut b.tasks, &mut b.programs, &mut b.ready)
        .start();
    vm.run_round(0).unwrap();
    assert_eq!(vm.read_variable(0).unwrap() as u32, 2_000_000_000);
}
```

For U64 tests, use `ContainerBuilder` with `add_i64_constant` and i64 load/store opcodes:

Create `compiler/vm/tests/execute_builtin_min_u64.rs`:
```rust
//! Integration tests for the BUILTIN MIN_U64 opcode.

mod common;

use common::VmBuffers;
use ironplc_container::ContainerBuilder;
use ironplc_vm::Vm;

#[test]
fn execute_when_min_u64_large_values_then_unsigned_comparison() {
    // MIN(10_000_000_000_000_000_000_u64, 5_000_000_000_u64) = 5_000_000_000
    // 10e18 stored as i64 is negative (> i64::MAX)
    let large_val = 10_000_000_000_000_000_000_u64 as i64;
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x02, 0x00, 0x00,
        0x02, 0x01, 0x00,
        0xC4, 0x69, 0x03,  // BUILTIN MIN_U64
        0x19, 0x00, 0x00,
        0xB5,
    ];
    let c = ContainerBuilder::new()
        .num_variables(1)
        .add_i64_constant(large_val)
        .add_i64_constant(5_000_000_000)
        .add_function(0, &bytecode, 16, 1)
        .build();
    let mut b = VmBuffers::from_container(&c);
    let mut vm = Vm::new()
        .load(&c, &mut b.stack, &mut b.vars, &mut b.tasks, &mut b.programs, &mut b.ready)
        .start();
    vm.run_round(0).unwrap();
    vm.stop();
    assert_eq!(b.vars[0].as_i64() as u64, 5_000_000_000);
}
```

Create similar `execute_builtin_max_u64.rs` and `execute_builtin_limit_u64.rs`.

**Step 2: Run tests to verify they fail**

Run: `cd /workspaces/ironplc/compiler && cargo test -p ironplc-vm execute_builtin_min_u32`
Expected: FAIL with `InvalidBuiltinFunction`

**Step 3: Add U32 and U64 dispatch handlers**

In `vm/src/builtin.rs`, insert before the `_ =>` catch-all:

```rust
opcode::builtin::MIN_U32 => {
    let b = stack.pop()?.as_i32() as u32;
    let a = stack.pop()?.as_i32() as u32;
    stack.push(Slot::from_i32(a.min(b) as i32))?;
    Ok(())
}
opcode::builtin::MAX_U32 => {
    let b = stack.pop()?.as_i32() as u32;
    let a = stack.pop()?.as_i32() as u32;
    stack.push(Slot::from_i32(a.max(b) as i32))?;
    Ok(())
}
opcode::builtin::LIMIT_U32 => {
    let mx = stack.pop()?.as_i32() as u32;
    let in_val = stack.pop()?.as_i32() as u32;
    let mn = stack.pop()?.as_i32() as u32;
    stack.push(Slot::from_i32(in_val.clamp(mn, mx) as i32))?;
    Ok(())
}
opcode::builtin::MIN_U64 => {
    let b = stack.pop()?.as_i64() as u64;
    let a = stack.pop()?.as_i64() as u64;
    stack.push(Slot::from_i64(a.min(b) as i64))?;
    Ok(())
}
opcode::builtin::MAX_U64 => {
    let b = stack.pop()?.as_i64() as u64;
    let a = stack.pop()?.as_i64() as u64;
    stack.push(Slot::from_i64(a.max(b) as i64))?;
    Ok(())
}
opcode::builtin::LIMIT_U64 => {
    let mx = stack.pop()?.as_i64() as u64;
    let in_val = stack.pop()?.as_i64() as u64;
    let mn = stack.pop()?.as_i64() as u64;
    stack.push(Slot::from_i64(in_val.clamp(mn, mx) as i64))?;
    Ok(())
}
```

**Step 4: Run all new U32/U64 tests**

Run: `cd /workspaces/ironplc/compiler && cargo test -p ironplc-vm execute_builtin_min_u32 execute_builtin_max_u32 execute_builtin_limit_u32 execute_builtin_min_u64 execute_builtin_max_u64 execute_builtin_limit_u64`
Expected: All PASS

**Step 5: Commit**

```bash
git add compiler/vm/src/builtin.rs compiler/vm/tests/execute_builtin_*_u32.rs compiler/vm/tests/execute_builtin_*_u64.rs
git commit -m "feat: add U32/U64 VM dispatch handlers for unsigned UDINT/ULINT builtins"
```

---

### Task 4: Update Codegen Routing

**Files:**
- Modify: `compiler/codegen/src/compile.rs` (lines 958-997 `lookup_builtin`, lines 1598-1605 `emit_pow`)

**Step 1: Change `lookup_builtin` signature to accept signedness**

Current signature:
```rust
fn lookup_builtin(name: &str, op_width: OpWidth) -> Option<u16>
```

New signature:
```rust
fn lookup_builtin(name: &str, op_width: OpWidth, signedness: Signedness) -> Option<u16>
```

Update the body:

```rust
fn lookup_builtin(name: &str, op_width: OpWidth, signedness: Signedness) -> Option<u16> {
    match name.to_uppercase().as_str() {
        "EXPT" => Some(match op_width {
            OpWidth::W32 => opcode::builtin::EXPT_I32,
            OpWidth::W64 => opcode::builtin::EXPT_I64,
            OpWidth::F32 => opcode::builtin::EXPT_F32,
            OpWidth::F64 => opcode::builtin::EXPT_F64,
        }),
        "ABS" => Some(match op_width {
            OpWidth::W32 | OpWidth::W64 => opcode::builtin::ABS_I32,
            OpWidth::F32 => opcode::builtin::ABS_F32,
            OpWidth::F64 => opcode::builtin::ABS_F64,
        }),
        "MIN" => Some(match (op_width, signedness) {
            (OpWidth::W32, Signedness::Signed) => opcode::builtin::MIN_I32,
            (OpWidth::W32, Signedness::Unsigned) => opcode::builtin::MIN_U32,
            (OpWidth::W64, Signedness::Signed) => opcode::builtin::MIN_I64,
            (OpWidth::W64, Signedness::Unsigned) => opcode::builtin::MIN_U64,
            (OpWidth::F32, _) => opcode::builtin::MIN_F32,
            (OpWidth::F64, _) => opcode::builtin::MIN_F64,
        }),
        "MAX" => Some(match (op_width, signedness) {
            (OpWidth::W32, Signedness::Signed) => opcode::builtin::MAX_I32,
            (OpWidth::W32, Signedness::Unsigned) => opcode::builtin::MAX_U32,
            (OpWidth::W64, Signedness::Signed) => opcode::builtin::MAX_I64,
            (OpWidth::W64, Signedness::Unsigned) => opcode::builtin::MAX_U64,
            (OpWidth::F32, _) => opcode::builtin::MAX_F32,
            (OpWidth::F64, _) => opcode::builtin::MAX_F64,
        }),
        "LIMIT" => Some(match (op_width, signedness) {
            (OpWidth::W32, Signedness::Signed) => opcode::builtin::LIMIT_I32,
            (OpWidth::W32, Signedness::Unsigned) => opcode::builtin::LIMIT_U32,
            (OpWidth::W64, Signedness::Signed) => opcode::builtin::LIMIT_I64,
            (OpWidth::W64, Signedness::Unsigned) => opcode::builtin::LIMIT_U64,
            (OpWidth::F32, _) => opcode::builtin::LIMIT_F32,
            (OpWidth::F64, _) => opcode::builtin::LIMIT_F64,
        }),
        "SEL" => Some(match op_width {
            OpWidth::W32 => opcode::builtin::SEL_I32,
            OpWidth::W64 => opcode::builtin::SEL_I64,
            OpWidth::F32 => opcode::builtin::SEL_F32,
            OpWidth::F64 => opcode::builtin::SEL_F64,
        }),
        "SQRT" => match op_width {
            OpWidth::F32 => Some(opcode::builtin::SQRT_F32),
            OpWidth::F64 => Some(opcode::builtin::SQRT_F64),
            OpWidth::W32 | OpWidth::W64 => None,
        },
        _ => None,
    }
}
```

Note: ABS routes W64 to ABS_I32 still — this is intentional if LINT isn't reachable via `lookup_builtin` for ABS. But actually, we should fix it:
- ABS W32 → ABS_I32
- ABS W64 → ABS_I64

Update ABS:
```rust
"ABS" => Some(match op_width {
    OpWidth::W32 => opcode::builtin::ABS_I32,
    OpWidth::W64 => opcode::builtin::ABS_I64,
    OpWidth::F32 => opcode::builtin::ABS_F32,
    OpWidth::F64 => opcode::builtin::ABS_F64,
}),
```

**Step 2: Update the call site in `compile_generic_builtin`**

In `compile_generic_builtin` (line 1155), change:

```rust
let func_id = lookup_builtin(&func_name, op_type.0)
```

to:

```rust
let func_id = lookup_builtin(&func_name, op_type.0, op_type.1)
```

**Step 3: Fix `emit_pow` for W64**

In `emit_pow` (line 1598-1605), change:

```rust
fn emit_pow(emitter: &mut Emitter, op_type: OpType) {
    match op_type.0 {
        OpWidth::W32 => emitter.emit_builtin(opcode::builtin::EXPT_I32),
        OpWidth::W64 => emitter.emit_builtin(opcode::builtin::EXPT_I32),  // BUG: should be I64
        OpWidth::F32 => emitter.emit_builtin(opcode::builtin::EXPT_F32),
        OpWidth::F64 => emitter.emit_builtin(opcode::builtin::EXPT_F64),
    }
}
```

to:

```rust
fn emit_pow(emitter: &mut Emitter, op_type: OpType) {
    match op_type.0 {
        OpWidth::W32 => emitter.emit_builtin(opcode::builtin::EXPT_I32),
        OpWidth::W64 => emitter.emit_builtin(opcode::builtin::EXPT_I64),
        OpWidth::F32 => emitter.emit_builtin(opcode::builtin::EXPT_F32),
        OpWidth::F64 => emitter.emit_builtin(opcode::builtin::EXPT_F64),
    }
}
```

**Step 4: Verify existing tests still pass**

Run: `cd /workspaces/ironplc/compiler && cargo test -p ironplc-codegen`
Expected: All existing tests PASS (routing for W32/Signed is unchanged)

**Step 5: Commit**

```bash
git add compiler/codegen/src/compile.rs
git commit -m "feat: update codegen to route I64/U32/U64 builtins based on signedness"
```

---

### Task 5: Add End-to-End Tests

End-to-end tests compile IEC 61131-3 source code and execute it through the full pipeline.

**Files:**
- Create: `compiler/codegen/tests/end_to_end_abs_lint.rs`
- Create: `compiler/codegen/tests/end_to_end_min_lint.rs`
- Create: `compiler/codegen/tests/end_to_end_min_udint.rs`
- Create: `compiler/codegen/tests/end_to_end_min_ulint.rs`
- Create: `compiler/codegen/tests/end_to_end_max_udint.rs`
- Create: `compiler/codegen/tests/end_to_end_limit_udint.rs`
- Create: `compiler/codegen/tests/end_to_end_expt_lint.rs`

**Step 1: Write end-to-end tests for LINT functions**

Create `compiler/codegen/tests/end_to_end_abs_lint.rs`:

```rust
//! End-to-end integration tests for ABS with LINT type.

mod common;

use common::parse_and_run;

#[test]
fn end_to_end_when_abs_lint_negative_then_positive() {
    let source = "
PROGRAM main
  VAR
    x : LINT;
    y : LINT;
  END_VAR
  x := -7000000000;
  y := ABS(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[1].as_i64(), 7_000_000_000);
}
```

Create `compiler/codegen/tests/end_to_end_min_lint.rs`:

```rust
//! End-to-end integration tests for MIN with LINT type.

mod common;

use common::parse_and_run;

#[test]
fn end_to_end_when_min_lint_then_returns_smaller() {
    let source = "
PROGRAM main
  VAR
    a : LINT;
    b : LINT;
    result : LINT;
  END_VAR
  a := -5000000000;
  b := 3000000000;
  result := MIN(a, b);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[2].as_i64(), -5_000_000_000);
}
```

Create `compiler/codegen/tests/end_to_end_expt_lint.rs`:

```rust
//! End-to-end integration tests for EXPT with LINT type.

mod common;

use common::parse_and_run;

#[test]
fn end_to_end_when_expt_lint_then_correct() {
    let source = "
PROGRAM main
  VAR
    base : LINT;
    exp : LINT;
    result : LINT;
  END_VAR
  base := 2;
  exp := 40;
  result := EXPT(base, exp);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[2].as_i64(), 1_099_511_627_776);
}
```

Create `compiler/codegen/tests/end_to_end_min_udint.rs`:

```rust
//! End-to-end integration tests for MIN with UDINT type.

mod common;

use common::parse_and_run;

#[test]
fn end_to_end_when_min_udint_large_values_then_unsigned_comparison() {
    let source = "
PROGRAM main
  VAR
    a : UDINT;
    b : UDINT;
    result : UDINT;
  END_VAR
  a := 3000000000;
  b := 1000000000;
  result := MIN(a, b);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[2].as_i32() as u32, 1_000_000_000);
}
```

Create `compiler/codegen/tests/end_to_end_max_udint.rs`:

```rust
//! End-to-end integration tests for MAX with UDINT type.

mod common;

use common::parse_and_run;

#[test]
fn end_to_end_when_max_udint_large_values_then_unsigned_comparison() {
    let source = "
PROGRAM main
  VAR
    a : UDINT;
    b : UDINT;
    result : UDINT;
  END_VAR
  a := 3000000000;
  b := 1000000000;
  result := MAX(a, b);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[2].as_i32() as u32, 3_000_000_000);
}
```

Create `compiler/codegen/tests/end_to_end_limit_udint.rs`:

```rust
//! End-to-end integration tests for LIMIT with UDINT type.

mod common;

use common::parse_and_run;

#[test]
fn end_to_end_when_limit_udint_above_max_then_clamped() {
    let source = "
PROGRAM main
  VAR
    result : UDINT;
  END_VAR
  result := LIMIT(1000000000, 4000000000, 3000000000);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[0].as_i32() as u32, 3_000_000_000);
}
```

Create `compiler/codegen/tests/end_to_end_min_ulint.rs`:

```rust
//! End-to-end integration tests for MIN with ULINT type.

mod common;

use common::parse_and_run;

#[test]
fn end_to_end_when_min_ulint_large_values_then_unsigned_comparison() {
    let source = "
PROGRAM main
  VAR
    a : ULINT;
    b : ULINT;
    result : ULINT;
  END_VAR
  a := 10000000000000000000;
  b := 5000000000;
  result := MIN(a, b);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[2].as_i64() as u64, 5_000_000_000);
}
```

**Step 2: Run end-to-end tests**

Run: `cd /workspaces/ironplc/compiler && cargo test -p ironplc-codegen end_to_end_when_abs_lint end_to_end_when_min_lint end_to_end_when_expt_lint end_to_end_when_min_udint end_to_end_when_max_udint end_to_end_when_limit_udint end_to_end_when_min_ulint`
Expected: All PASS

**Step 3: Commit**

```bash
git add compiler/codegen/tests/end_to_end_*_lint.rs compiler/codegen/tests/end_to_end_*_udint.rs compiler/codegen/tests/end_to_end_*_ulint.rs
git commit -m "test: add end-to-end tests for LINT, UDINT, and ULINT stdlib functions"
```

---

### Task 6: Update Documentation

**Files:**
- Modify: `docs/reference/standard-library/functions/index.rst`
- Modify: `docs/reference/standard-library/functions/abs.rst`
- Modify: `docs/reference/standard-library/functions/min.rst`
- Modify: `docs/reference/standard-library/functions/max.rst`
- Modify: `docs/reference/standard-library/functions/limit.rst`
- Modify: `docs/reference/standard-library/functions/expt.rst`

**Step 1: Update `index.rst`**

Change the status in the index for these functions:

| Function | Old Status | New Status |
|----------|-----------|------------|
| ABS      | Not yet supported | Supported |
| SQRT     | Not yet supported | Supported |
| SEL      | Not yet supported | Supported |
| MAX      | Not yet supported | Supported |
| MIN      | Not yet supported | Supported |
| LIMIT    | Not yet supported | Supported |

**Step 2: Update individual function docs**

For `abs.rst`: Change SINT, INT, LINT rows from "Not yet supported" to "Supported".

For `min.rst`, `max.rst`, `limit.rst`: Change ALL integer rows (SINT, INT, LINT, USINT, UINT, UDINT, ULINT) from "Not yet supported" to "Supported".

For `expt.rst`: Change LINT row from "Not yet supported" to "Supported".

`sqrt.rst` and `sel.rst` already show "Supported" for all their rows — no changes needed.

**Step 3: Commit**

```bash
git add docs/reference/standard-library/functions/index.rst docs/reference/standard-library/functions/abs.rst docs/reference/standard-library/functions/min.rst docs/reference/standard-library/functions/max.rst docs/reference/standard-library/functions/limit.rst docs/reference/standard-library/functions/expt.rst
git commit -m "docs: mark ABS, MIN, MAX, LIMIT, SEL, SQRT, EXPT as fully supported"
```

---

### Task 7: Run Full CI Pipeline

Run `cd /workspaces/ironplc/compiler && just` to verify the full CI pipeline passes (compile, tests, coverage, clippy, fmt) before creating a PR.

If any check fails, fix and re-run.

---

### Task 8: Create PR

Create a feature branch and PR using `gh pr create`.
