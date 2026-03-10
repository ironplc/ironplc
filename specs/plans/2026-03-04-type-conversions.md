# Type Conversion Standard Library Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Implement all 90 numeric type conversion functions (`*_TO_*`) so that IEC 61131-3 programs can explicitly convert between integer and floating-point types.

**Architecture:** Type conversions use 19 new BUILTIN opcodes for cross-domain conversions (int↔float, float↔float, unsigned zero-extend). Same-domain integer conversions are no-ops at the VM level — the Slot's sign-extension and truncation opcodes handle them. The codegen parses the function name to determine source/target types and routes to a dedicated `compile_type_conversion` handler.

**Tech Stack:** Rust, IronPLC compiler (container, codegen, vm crates)

---

### Task 1: Add 19 conversion opcode constants

**Files:**
- Modify: `compiler/container/src/opcode.rs` (inside `pub mod builtin`)

**Step 1: Add the 19 new constants**

Add after `ATAN_F64` (0x037D) inside `pub mod builtin`:

```rust
    // --- Type conversion opcodes ---

    /// Convert signed 32-bit integer to 32-bit float.
    pub const CONV_I32_TO_F32: u16 = 0x037E;

    /// Convert signed 32-bit integer to 64-bit float.
    pub const CONV_I32_TO_F64: u16 = 0x037F;

    /// Convert signed 64-bit integer to 32-bit float.
    pub const CONV_I64_TO_F32: u16 = 0x0380;

    /// Convert signed 64-bit integer to 64-bit float.
    pub const CONV_I64_TO_F64: u16 = 0x0381;

    /// Convert unsigned 32-bit integer to 32-bit float.
    pub const CONV_U32_TO_F32: u16 = 0x0382;

    /// Convert unsigned 32-bit integer to 64-bit float.
    pub const CONV_U32_TO_F64: u16 = 0x0383;

    /// Convert unsigned 64-bit integer to 32-bit float.
    pub const CONV_U64_TO_F32: u16 = 0x0384;

    /// Convert unsigned 64-bit integer to 64-bit float.
    pub const CONV_U64_TO_F64: u16 = 0x0385;

    /// Convert 32-bit float to signed 32-bit integer (truncating).
    pub const CONV_F32_TO_I32: u16 = 0x0386;

    /// Convert 32-bit float to signed 64-bit integer (truncating).
    pub const CONV_F32_TO_I64: u16 = 0x0387;

    /// Convert 64-bit float to signed 32-bit integer (truncating).
    pub const CONV_F64_TO_I32: u16 = 0x0388;

    /// Convert 64-bit float to signed 64-bit integer (truncating).
    pub const CONV_F64_TO_I64: u16 = 0x0389;

    /// Convert 32-bit float to unsigned 32-bit integer (truncating).
    pub const CONV_F32_TO_U32: u16 = 0x038A;

    /// Convert 32-bit float to unsigned 64-bit integer (truncating).
    pub const CONV_F32_TO_U64: u16 = 0x038B;

    /// Convert 64-bit float to unsigned 32-bit integer (truncating).
    pub const CONV_F64_TO_U32: u16 = 0x038C;

    /// Convert 64-bit float to unsigned 64-bit integer (truncating).
    pub const CONV_F64_TO_U64: u16 = 0x038D;

    /// Widen 32-bit float to 64-bit float.
    pub const CONV_F32_TO_F64: u16 = 0x038E;

    /// Narrow 64-bit float to 32-bit float.
    pub const CONV_F64_TO_F32: u16 = 0x038F;

    /// Zero-extend unsigned 32-bit integer to 64-bit integer.
    pub const CONV_U32_TO_I64: u16 = 0x0390;
```

**Step 2: Update `arg_count` to include the new IDs**

In the `arg_count` function, add all 19 new IDs to the `=> 1` arm (they all pop 1 argument):

```rust
            ABS_I32 | ABS_F32 | ABS_F64 | ABS_I64 | SQRT_F32 | SQRT_F64 | LN_F32 | LN_F64
            | LOG_F32 | LOG_F64 | EXP_F32 | EXP_F64 | SIN_F32 | SIN_F64 | COS_F32 | COS_F64
            | TAN_F32 | TAN_F64 | ASIN_F32 | ASIN_F64 | ACOS_F32 | ACOS_F64 | ATAN_F32
            | ATAN_F64 | CONV_I32_TO_F32 | CONV_I32_TO_F64 | CONV_I64_TO_F32 | CONV_I64_TO_F64
            | CONV_U32_TO_F32 | CONV_U32_TO_F64 | CONV_U64_TO_F32 | CONV_U64_TO_F64
            | CONV_F32_TO_I32 | CONV_F32_TO_I64 | CONV_F64_TO_I32 | CONV_F64_TO_I64
            | CONV_F32_TO_U32 | CONV_F32_TO_U64 | CONV_F64_TO_U32 | CONV_F64_TO_U64
            | CONV_F32_TO_F64 | CONV_F64_TO_F32 | CONV_U32_TO_I64 => 1,
```

**Step 3: Verify it compiles**

Run: `cargo build -p ironplc-container`
Expected: Compiles with no errors.

**Step 4: Commit**

```bash
git add compiler/container/src/opcode.rs
git commit -m "feat: add 19 type conversion BUILTIN opcode constants"
```

---

### Task 2: Add VM dispatch for all 19 conversion opcodes

**Files:**
- Modify: `compiler/vm/src/builtin.rs` (inside `dispatch` function, before the `_ => Err(Trap::InvalidBuiltinFunction(func_id))` arm)

**Step 1: Add the 19 match arms**

Add before the `_ =>` arm in the `dispatch` function:

```rust
        // --- Type conversion opcodes ---
        opcode::builtin::CONV_I32_TO_F32 => {
            let a = stack.pop()?.as_i32();
            stack.push(Slot::from_f32(a as f32))?;
            Ok(())
        }
        opcode::builtin::CONV_I32_TO_F64 => {
            let a = stack.pop()?.as_i32();
            stack.push(Slot::from_f64(a as f64))?;
            Ok(())
        }
        opcode::builtin::CONV_I64_TO_F32 => {
            let a = stack.pop()?.as_i64();
            stack.push(Slot::from_f32(a as f32))?;
            Ok(())
        }
        opcode::builtin::CONV_I64_TO_F64 => {
            let a = stack.pop()?.as_i64();
            stack.push(Slot::from_f64(a as f64))?;
            Ok(())
        }
        opcode::builtin::CONV_U32_TO_F32 => {
            let a = stack.pop()?.as_i32() as u32;
            stack.push(Slot::from_f32(a as f32))?;
            Ok(())
        }
        opcode::builtin::CONV_U32_TO_F64 => {
            let a = stack.pop()?.as_i32() as u32;
            stack.push(Slot::from_f64(a as f64))?;
            Ok(())
        }
        opcode::builtin::CONV_U64_TO_F32 => {
            let a = stack.pop()?.as_i64() as u64;
            stack.push(Slot::from_f32(a as f32))?;
            Ok(())
        }
        opcode::builtin::CONV_U64_TO_F64 => {
            let a = stack.pop()?.as_i64() as u64;
            stack.push(Slot::from_f64(a as f64))?;
            Ok(())
        }
        opcode::builtin::CONV_F32_TO_I32 => {
            let a = stack.pop()?.as_f32();
            stack.push(Slot::from_i32(a as i32))?;
            Ok(())
        }
        opcode::builtin::CONV_F32_TO_I64 => {
            let a = stack.pop()?.as_f32();
            stack.push(Slot::from_i64(a as i64))?;
            Ok(())
        }
        opcode::builtin::CONV_F64_TO_I32 => {
            let a = stack.pop()?.as_f64();
            stack.push(Slot::from_i32(a as i32))?;
            Ok(())
        }
        opcode::builtin::CONV_F64_TO_I64 => {
            let a = stack.pop()?.as_f64();
            stack.push(Slot::from_i64(a as i64))?;
            Ok(())
        }
        opcode::builtin::CONV_F32_TO_U32 => {
            let a = stack.pop()?.as_f32();
            stack.push(Slot::from_i32(a as u32 as i32))?;
            Ok(())
        }
        opcode::builtin::CONV_F32_TO_U64 => {
            let a = stack.pop()?.as_f32();
            stack.push(Slot::from_i64(a as u64 as i64))?;
            Ok(())
        }
        opcode::builtin::CONV_F64_TO_U32 => {
            let a = stack.pop()?.as_f64();
            stack.push(Slot::from_i32(a as u32 as i32))?;
            Ok(())
        }
        opcode::builtin::CONV_F64_TO_U64 => {
            let a = stack.pop()?.as_f64();
            stack.push(Slot::from_i64(a as u64 as i64))?;
            Ok(())
        }
        opcode::builtin::CONV_F32_TO_F64 => {
            let a = stack.pop()?.as_f32();
            stack.push(Slot::from_f64(a as f64))?;
            Ok(())
        }
        opcode::builtin::CONV_F64_TO_F32 => {
            let a = stack.pop()?.as_f64();
            stack.push(Slot::from_f32(a as f32))?;
            Ok(())
        }
        opcode::builtin::CONV_U32_TO_I64 => {
            let a = stack.pop()?.as_i32() as u32;
            stack.push(Slot::from_i64(a as u64 as i64))?;
            Ok(())
        }
```

**Step 2: Verify it compiles**

Run: `cargo build -p ironplc-vm`
Expected: Compiles with no errors.

**Step 3: Commit**

```bash
git add compiler/vm/src/builtin.rs
git commit -m "feat: add VM dispatch for 19 type conversion opcodes"
```

---

### Task 3: Write VM tests for conversion opcodes

**Files:**
- Create: `compiler/vm/tests/execute_builtin_conv_int_to_float.rs`
- Create: `compiler/vm/tests/execute_builtin_conv_float_to_int.rs`
- Create: `compiler/vm/tests/execute_builtin_conv_float_to_float.rs`
- Create: `compiler/vm/tests/execute_builtin_conv_zero_extend.rs`

These tests directly exercise the VM opcodes by constructing bytecode manually, following the pattern in `execute_builtin_math_f32.rs`.

**Step 1: Create int-to-float VM tests**

Create `compiler/vm/tests/execute_builtin_conv_int_to_float.rs`:

```rust
//! VM tests for integer-to-float conversion opcodes.

mod common;

use common::{single_function_container, single_function_container_i64, VmBuffers};
use ironplc_vm::Vm;

#[test]
fn execute_when_conv_i32_to_f32_then_correct() {
    // Load i32 constant 42, convert to f32, store
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] = 42
        0xC4, 0x7E, 0x03,  // BUILTIN CONV_I32_TO_F32
        0x1A, 0x00, 0x00,  // STORE_VAR_F32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container(&bytecode, 1, &[42]);
    let mut b = VmBuffers::from_container(&c);
    {
        let mut vm = Vm::new()
            .load(&c, &mut b.stack, &mut b.vars, &mut b.tasks, &mut b.programs, &mut b.ready)
            .start();
        vm.run_round(0).unwrap();
    }
    let result = b.vars[0].as_f32();
    assert!((result - 42.0).abs() < 1e-5, "expected 42.0, got {result}");
}

#[test]
fn execute_when_conv_i32_to_f32_negative_then_correct() {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] = -7
        0xC4, 0x7E, 0x03,  // BUILTIN CONV_I32_TO_F32
        0x1A, 0x00, 0x00,  // STORE_VAR_F32 var[0]
        0xB5,
    ];
    let c = single_function_container(&bytecode, 1, &[-7]);
    let mut b = VmBuffers::from_container(&c);
    {
        let mut vm = Vm::new()
            .load(&c, &mut b.stack, &mut b.vars, &mut b.tasks, &mut b.programs, &mut b.ready)
            .start();
        vm.run_round(0).unwrap();
    }
    assert!((b.vars[0].as_f32() - (-7.0)).abs() < 1e-5);
}

#[test]
fn execute_when_conv_i32_to_f64_then_correct() {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] = 100
        0xC4, 0x7F, 0x03,  // BUILTIN CONV_I32_TO_F64
        0x1B, 0x00, 0x00,  // STORE_VAR_F64 var[0]
        0xB5,
    ];
    let c = single_function_container(&bytecode, 1, &[100]);
    let mut b = VmBuffers::from_container(&c);
    {
        let mut vm = Vm::new()
            .load(&c, &mut b.stack, &mut b.vars, &mut b.tasks, &mut b.programs, &mut b.ready)
            .start();
        vm.run_round(0).unwrap();
    }
    assert!((b.vars[0].as_f64() - 100.0).abs() < 1e-12);
}

#[test]
fn execute_when_conv_i64_to_f32_then_correct() {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x02, 0x00, 0x00,  // LOAD_CONST_I64 pool[0] = 1000
        0xC4, 0x80, 0x03,  // BUILTIN CONV_I64_TO_F32
        0x1A, 0x00, 0x00,  // STORE_VAR_F32 var[0]
        0xB5,
    ];
    let c = single_function_container_i64(&bytecode, 1, &[1000]);
    let mut b = VmBuffers::from_container(&c);
    {
        let mut vm = Vm::new()
            .load(&c, &mut b.stack, &mut b.vars, &mut b.tasks, &mut b.programs, &mut b.ready)
            .start();
        vm.run_round(0).unwrap();
    }
    assert!((b.vars[0].as_f32() - 1000.0).abs() < 1e-3);
}

#[test]
fn execute_when_conv_i64_to_f64_then_correct() {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x02, 0x00, 0x00,  // LOAD_CONST_I64 pool[0] = 123456789
        0xC4, 0x81, 0x03,  // BUILTIN CONV_I64_TO_F64
        0x1B, 0x00, 0x00,  // STORE_VAR_F64 var[0]
        0xB5,
    ];
    let c = single_function_container_i64(&bytecode, 1, &[123456789]);
    let mut b = VmBuffers::from_container(&c);
    {
        let mut vm = Vm::new()
            .load(&c, &mut b.stack, &mut b.vars, &mut b.tasks, &mut b.programs, &mut b.ready)
            .start();
        vm.run_round(0).unwrap();
    }
    assert!((b.vars[0].as_f64() - 123456789.0).abs() < 1.0);
}

#[test]
fn execute_when_conv_u32_to_f32_then_correct() {
    // Store unsigned value 200 (fits in positive i32)
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] = 200
        0xC4, 0x82, 0x03,  // BUILTIN CONV_U32_TO_F32
        0x1A, 0x00, 0x00,  // STORE_VAR_F32 var[0]
        0xB5,
    ];
    let c = single_function_container(&bytecode, 1, &[200]);
    let mut b = VmBuffers::from_container(&c);
    {
        let mut vm = Vm::new()
            .load(&c, &mut b.stack, &mut b.vars, &mut b.tasks, &mut b.programs, &mut b.ready)
            .start();
        vm.run_round(0).unwrap();
    }
    assert!((b.vars[0].as_f32() - 200.0).abs() < 1e-5);
}

#[test]
fn execute_when_conv_u32_to_f64_large_then_correct() {
    // UDINT max: 0xFFFFFFFF stored as i32 = -1
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] = -1 (unsigned 4294967295)
        0xC4, 0x83, 0x03,  // BUILTIN CONV_U32_TO_F64
        0x1B, 0x00, 0x00,  // STORE_VAR_F64 var[0]
        0xB5,
    ];
    let c = single_function_container(&bytecode, 1, &[-1]);
    let mut b = VmBuffers::from_container(&c);
    {
        let mut vm = Vm::new()
            .load(&c, &mut b.stack, &mut b.vars, &mut b.tasks, &mut b.programs, &mut b.ready)
            .start();
        vm.run_round(0).unwrap();
    }
    assert!((b.vars[0].as_f64() - 4294967295.0).abs() < 1.0);
}
```

**Step 2: Create float-to-int VM tests**

Create `compiler/vm/tests/execute_builtin_conv_float_to_int.rs`:

```rust
//! VM tests for float-to-integer conversion opcodes.

mod common;

use common::{single_function_container_f32, single_function_container_f64, VmBuffers};
use ironplc_vm::Vm;

#[test]
fn execute_when_conv_f32_to_i32_then_truncates() {
    // REAL 3.14 -> INT = 3 (truncated)
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x03, 0x00, 0x00,  // LOAD_CONST_F32 pool[0] = 3.14
        0xC4, 0x86, 0x03,  // BUILTIN CONV_F32_TO_I32
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,
    ];
    let c = single_function_container_f32(&bytecode, 1, &[3.14]);
    let mut b = VmBuffers::from_container(&c);
    {
        let mut vm = Vm::new()
            .load(&c, &mut b.stack, &mut b.vars, &mut b.tasks, &mut b.programs, &mut b.ready)
            .start();
        vm.run_round(0).unwrap();
    }
    assert_eq!(b.vars[0].as_i32(), 3);
}

#[test]
fn execute_when_conv_f32_to_i32_negative_then_truncates() {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x03, 0x00, 0x00,  // LOAD_CONST_F32 pool[0] = -2.9
        0xC4, 0x86, 0x03,  // BUILTIN CONV_F32_TO_I32
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,
    ];
    let c = single_function_container_f32(&bytecode, 1, &[-2.9]);
    let mut b = VmBuffers::from_container(&c);
    {
        let mut vm = Vm::new()
            .load(&c, &mut b.stack, &mut b.vars, &mut b.tasks, &mut b.programs, &mut b.ready)
            .start();
        vm.run_round(0).unwrap();
    }
    assert_eq!(b.vars[0].as_i32(), -2);
}

#[test]
fn execute_when_conv_f64_to_i32_then_truncates() {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x04, 0x00, 0x00,  // LOAD_CONST_F64 pool[0] = 99.9
        0xC4, 0x88, 0x03,  // BUILTIN CONV_F64_TO_I32
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,
    ];
    let c = single_function_container_f64(&bytecode, 1, &[99.9]);
    let mut b = VmBuffers::from_container(&c);
    {
        let mut vm = Vm::new()
            .load(&c, &mut b.stack, &mut b.vars, &mut b.tasks, &mut b.programs, &mut b.ready)
            .start();
        vm.run_round(0).unwrap();
    }
    assert_eq!(b.vars[0].as_i32(), 99);
}

#[test]
fn execute_when_conv_f64_to_i64_then_truncates() {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x04, 0x00, 0x00,  // LOAD_CONST_F64 pool[0] = 1e15 + 0.7
        0xC4, 0x89, 0x03,  // BUILTIN CONV_F64_TO_I64
        0x19, 0x00, 0x00,  // STORE_VAR_I64 var[0]
        0xB5,
    ];
    let c = single_function_container_f64(&bytecode, 1, &[1_000_000_000_000_000.7]);
    let mut b = VmBuffers::from_container(&c);
    {
        let mut vm = Vm::new()
            .load(&c, &mut b.stack, &mut b.vars, &mut b.tasks, &mut b.programs, &mut b.ready)
            .start();
        vm.run_round(0).unwrap();
    }
    assert_eq!(b.vars[0].as_i64(), 1_000_000_000_000_000);
}

#[test]
fn execute_when_conv_f32_to_u32_then_correct() {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x03, 0x00, 0x00,  // LOAD_CONST_F32 pool[0] = 200.0
        0xC4, 0x8A, 0x03,  // BUILTIN CONV_F32_TO_U32
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,
    ];
    let c = single_function_container_f32(&bytecode, 1, &[200.0]);
    let mut b = VmBuffers::from_container(&c);
    {
        let mut vm = Vm::new()
            .load(&c, &mut b.stack, &mut b.vars, &mut b.tasks, &mut b.programs, &mut b.ready)
            .start();
        vm.run_round(0).unwrap();
    }
    assert_eq!(b.vars[0].as_i32() as u32, 200);
}
```

**Step 3: Create float-to-float VM tests**

Create `compiler/vm/tests/execute_builtin_conv_float_to_float.rs`:

```rust
//! VM tests for float-to-float conversion opcodes.

mod common;

use common::{single_function_container_f32, single_function_container_f64, VmBuffers};
use ironplc_vm::Vm;

#[test]
fn execute_when_conv_f32_to_f64_then_correct() {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x03, 0x00, 0x00,  // LOAD_CONST_F32 pool[0] = 3.14
        0xC4, 0x8E, 0x03,  // BUILTIN CONV_F32_TO_F64
        0x1B, 0x00, 0x00,  // STORE_VAR_F64 var[0]
        0xB5,
    ];
    let c = single_function_container_f32(&bytecode, 1, &[3.14]);
    let mut b = VmBuffers::from_container(&c);
    {
        let mut vm = Vm::new()
            .load(&c, &mut b.stack, &mut b.vars, &mut b.tasks, &mut b.programs, &mut b.ready)
            .start();
        vm.run_round(0).unwrap();
    }
    let result = b.vars[0].as_f64();
    // f32 3.14 promoted to f64 should be close to 3.14 but not exact
    assert!((result - 3.14).abs() < 0.001, "expected ~3.14, got {result}");
}

#[test]
fn execute_when_conv_f64_to_f32_then_correct() {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x04, 0x00, 0x00,  // LOAD_CONST_F64 pool[0] = 2.718281828
        0xC4, 0x8F, 0x03,  // BUILTIN CONV_F64_TO_F32
        0x1A, 0x00, 0x00,  // STORE_VAR_F32 var[0]
        0xB5,
    ];
    let c = single_function_container_f64(&bytecode, 1, &[2.718281828]);
    let mut b = VmBuffers::from_container(&c);
    {
        let mut vm = Vm::new()
            .load(&c, &mut b.stack, &mut b.vars, &mut b.tasks, &mut b.programs, &mut b.ready)
            .start();
        vm.run_round(0).unwrap();
    }
    let result = b.vars[0].as_f32();
    assert!((result - 2.718282).abs() < 1e-4, "expected ~2.718282, got {result}");
}
```

**Step 4: Create zero-extend VM test**

Create `compiler/vm/tests/execute_builtin_conv_zero_extend.rs`:

```rust
//! VM tests for unsigned 32-to-64 zero-extension conversion opcode.

mod common;

use common::{single_function_container, VmBuffers};
use ironplc_vm::Vm;

#[test]
fn execute_when_conv_u32_to_i64_small_then_correct() {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] = 42
        0xC4, 0x90, 0x03,  // BUILTIN CONV_U32_TO_I64
        0x19, 0x00, 0x00,  // STORE_VAR_I64 var[0]
        0xB5,
    ];
    let c = single_function_container(&bytecode, 1, &[42]);
    let mut b = VmBuffers::from_container(&c);
    {
        let mut vm = Vm::new()
            .load(&c, &mut b.stack, &mut b.vars, &mut b.tasks, &mut b.programs, &mut b.ready)
            .start();
        vm.run_round(0).unwrap();
    }
    assert_eq!(b.vars[0].as_i64(), 42);
}

#[test]
fn execute_when_conv_u32_to_i64_large_then_zero_extends() {
    // UDINT max 0xFFFFFFFF stored as i32 = -1
    // After zero-extend should be 4294967295, NOT -1
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] = -1 (0xFFFFFFFF unsigned)
        0xC4, 0x90, 0x03,  // BUILTIN CONV_U32_TO_I64
        0x19, 0x00, 0x00,  // STORE_VAR_I64 var[0]
        0xB5,
    ];
    let c = single_function_container(&bytecode, 1, &[-1]);
    let mut b = VmBuffers::from_container(&c);
    {
        let mut vm = Vm::new()
            .load(&c, &mut b.stack, &mut b.vars, &mut b.tasks, &mut b.programs, &mut b.ready)
            .start();
        vm.run_round(0).unwrap();
    }
    // Key assertion: must be positive 4294967295, not -1
    assert_eq!(b.vars[0].as_i64(), 4294967295_i64);
}
```

**Step 5: Run VM tests**

Run: `cargo test -p ironplc-vm`
Expected: All tests pass (including existing ones).

**Step 6: Commit**

```bash
git add compiler/vm/tests/execute_builtin_conv_int_to_float.rs
git add compiler/vm/tests/execute_builtin_conv_float_to_int.rs
git add compiler/vm/tests/execute_builtin_conv_float_to_float.rs
git add compiler/vm/tests/execute_builtin_conv_zero_extend.rs
git commit -m "test: add VM tests for type conversion opcodes"
```

---

### Task 4: Add codegen `compile_type_conversion` handler

This is the core task. It adds a new handler in `compile_function_call` that detects conversion function names and compiles them with the correct source/target types.

**Files:**
- Modify: `compiler/codegen/src/compile.rs`

**Step 1: Add the `is_type_conversion` helper function**

Add near the other helper functions (after `resolve_type_name` around line 364):

```rust
/// Checks if a function name is a type conversion (e.g., "INT_TO_REAL").
/// Returns `Some((source_type_info, target_type_info))` if both parts are
/// recognized type names, `None` otherwise.
fn parse_type_conversion(name: &str) -> Option<(VarTypeInfo, VarTypeInfo)> {
    let upper = name.to_uppercase();
    let parts: Vec<&str> = upper.splitn(2, "_TO_").collect();
    if parts.len() != 2 {
        return None;
    }
    let source = resolve_type_name(&Id::from(parts[0]))?;
    let target = resolve_type_name(&Id::from(parts[1]))?;
    Some((source, target))
}
```

**Step 2: Add the `compile_type_conversion` function**

Add after `compile_generic_builtin` (around line 1249):

```rust
/// Compiles a type conversion function call (e.g., INT_TO_REAL).
///
/// Unlike generic builtins, conversion functions have different source and
/// target types. The argument is compiled with the source type's OpType,
/// then a conversion opcode (if needed) transforms the value to the target
/// representation.
fn compile_type_conversion(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    func: &Function,
    source: VarTypeInfo,
    target: VarTypeInfo,
) -> Result<(), Diagnostic> {
    // Compile the single argument with the source type's operation type.
    let source_op_type: OpType = (source.op_width, source.signedness);

    let args: Vec<&Expr> = func
        .param_assignment
        .iter()
        .filter_map(|p| match p {
            ParamAssignmentKind::PositionalInput(pos) => Some(&pos.expr),
            _ => None,
        })
        .collect();

    if args.len() != 1 {
        return Err(Diagnostic::todo_with_span(
            func.name.span(),
            file!(),
            line!(),
        ));
    }

    compile_expr(emitter, ctx, args[0], source_op_type)?;

    // Emit conversion opcode based on source/target domain crossing.
    emit_conversion_opcode(emitter, &source, &target);

    // Emit truncation if the target is a sub-width type (e.g., SINT is 8-bit within W32).
    emit_truncation(emitter, target);

    Ok(())
}

/// Emits the appropriate conversion BUILTIN opcode for the source→target
/// type transition. Does nothing for same-domain integer conversions that
/// are handled by the Slot's sign-extension and truncation.
fn emit_conversion_opcode(emitter: &mut Emitter, source: &VarTypeInfo, target: &VarTypeInfo) {
    use OpWidth::*;
    use Signedness::*;

    match (source.op_width, source.signedness, target.op_width, target.signedness) {
        // Same OpWidth: no conversion needed (truncation handles sub-width)
        (W32, _, W32, _) => {}
        (W64, _, W64, _) => {}

        // W32 signed → W64: sign extension already in Slot, no-op
        (W32, Signed, W64, _) => {}

        // W32 unsigned → W64: need zero-extension
        (W32, Unsigned, W64, _) => {
            emitter.emit_builtin(opcode::builtin::CONV_U32_TO_I64);
        }

        // W64 → W32: as_i32() truncation at store time, no-op
        (W64, _, W32, _) => {}

        // Integer → Float
        (W32, Signed, F32, _) => emitter.emit_builtin(opcode::builtin::CONV_I32_TO_F32),
        (W32, Signed, F64, _) => emitter.emit_builtin(opcode::builtin::CONV_I32_TO_F64),
        (W64, Signed, F32, _) => emitter.emit_builtin(opcode::builtin::CONV_I64_TO_F32),
        (W64, Signed, F64, _) => emitter.emit_builtin(opcode::builtin::CONV_I64_TO_F64),
        (W32, Unsigned, F32, _) => emitter.emit_builtin(opcode::builtin::CONV_U32_TO_F32),
        (W32, Unsigned, F64, _) => emitter.emit_builtin(opcode::builtin::CONV_U32_TO_F64),
        (W64, Unsigned, F32, _) => emitter.emit_builtin(opcode::builtin::CONV_U64_TO_F32),
        (W64, Unsigned, F64, _) => emitter.emit_builtin(opcode::builtin::CONV_U64_TO_F64),

        // Float → Integer
        (F32, _, W32, Signed) => emitter.emit_builtin(opcode::builtin::CONV_F32_TO_I32),
        (F32, _, W64, Signed) => emitter.emit_builtin(opcode::builtin::CONV_F32_TO_I64),
        (F64, _, W32, Signed) => emitter.emit_builtin(opcode::builtin::CONV_F64_TO_I32),
        (F64, _, W64, Signed) => emitter.emit_builtin(opcode::builtin::CONV_F64_TO_I64),
        (F32, _, W32, Unsigned) => emitter.emit_builtin(opcode::builtin::CONV_F32_TO_U32),
        (F32, _, W64, Unsigned) => emitter.emit_builtin(opcode::builtin::CONV_F32_TO_U64),
        (F64, _, W32, Unsigned) => emitter.emit_builtin(opcode::builtin::CONV_F64_TO_U32),
        (F64, _, W64, Unsigned) => emitter.emit_builtin(opcode::builtin::CONV_F64_TO_U64),

        // Float → Float
        (F32, _, F64, _) => emitter.emit_builtin(opcode::builtin::CONV_F32_TO_F64),
        (F64, _, F32, _) => emitter.emit_builtin(opcode::builtin::CONV_F64_TO_F32),

        // Float → Float same width (shouldn't happen, but handle gracefully)
        (F32, _, F32, _) | (F64, _, F64, _) => {}
    }
}
```

**Step 3: Update `compile_function_call` routing**

Modify `compile_function_call` (line 1187) to detect and route conversion functions:

```rust
fn compile_function_call(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    func: &Function,
    op_type: OpType,
) -> Result<(), Diagnostic> {
    let name = func.name.lower_case();
    match name.as_str() {
        "shl" | "shr" | "rol" | "ror" => {
            compile_shift_rotate(emitter, ctx, func, op_type, name.as_str())
        }
        _ => {
            // Check if this is a type conversion function (e.g., INT_TO_REAL).
            if let Some((source, target)) = parse_type_conversion(&name) {
                compile_type_conversion(emitter, ctx, func, source, target)
            } else {
                compile_generic_builtin(emitter, ctx, func, op_type)
            }
        }
    }
}
```

**Step 4: Verify it compiles**

Run: `cargo build -p ironplc-codegen`
Expected: Compiles with no errors.

**Step 5: Commit**

```bash
git add compiler/codegen/src/compile.rs
git commit -m "feat: add compile_type_conversion codegen handler for *_TO_* functions"
```

---

### Task 5: Write end-to-end tests for type conversions

These tests exercise the full pipeline: parse → analyze → compile → run.

**Files:**
- Create: `compiler/codegen/tests/end_to_end_conv_int_to_real.rs`
- Create: `compiler/codegen/tests/end_to_end_conv_real_to_int.rs`
- Create: `compiler/codegen/tests/end_to_end_conv_real_to_real.rs`
- Create: `compiler/codegen/tests/end_to_end_conv_int_widening.rs`
- Create: `compiler/codegen/tests/end_to_end_conv_int_narrowing.rs`

**Step 1: Create int-to-real end-to-end tests**

Create `compiler/codegen/tests/end_to_end_conv_int_to_real.rs`:

```rust
//! End-to-end tests for integer-to-real type conversions.

mod common;

use common::parse_and_run;

#[test]
fn end_to_end_when_int_to_real_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : INT;
    y : REAL;
  END_VAR
  x := 42;
  y := INT_TO_REAL(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert!((bufs.vars[1].as_f32() - 42.0).abs() < 1e-5);
}

#[test]
fn end_to_end_when_dint_to_lreal_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : DINT;
    y : LREAL;
  END_VAR
  x := -100;
  y := DINT_TO_LREAL(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert!((bufs.vars[1].as_f64() - (-100.0)).abs() < 1e-12);
}

#[test]
fn end_to_end_when_sint_to_real_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : SINT;
    y : REAL;
  END_VAR
  x := -7;
  y := SINT_TO_REAL(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert!((bufs.vars[1].as_f32() - (-7.0)).abs() < 1e-5);
}

#[test]
fn end_to_end_when_lint_to_lreal_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : LINT;
    y : LREAL;
  END_VAR
  x := 123456789;
  y := LINT_TO_LREAL(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert!((bufs.vars[1].as_f64() - 123456789.0).abs() < 1.0);
}

#[test]
fn end_to_end_when_uint_to_real_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : UINT;
    y : REAL;
  END_VAR
  x := 40000;
  y := UINT_TO_REAL(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert!((bufs.vars[1].as_f32() - 40000.0).abs() < 1.0);
}
```

**Step 2: Create real-to-int end-to-end tests**

Create `compiler/codegen/tests/end_to_end_conv_real_to_int.rs`:

```rust
//! End-to-end tests for real-to-integer type conversions.

mod common;

use common::parse_and_run;

#[test]
fn end_to_end_when_real_to_int_then_truncates() {
    let source = "
PROGRAM main
  VAR
    x : REAL;
    y : INT;
  END_VAR
  x := 3.14;
  y := REAL_TO_INT(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[1].as_i32(), 3);
}

#[test]
fn end_to_end_when_real_to_dint_negative_then_truncates() {
    let source = "
PROGRAM main
  VAR
    x : REAL;
    y : DINT;
  END_VAR
  x := -7.9;
  y := REAL_TO_DINT(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[1].as_i32(), -7);
}

#[test]
fn end_to_end_when_lreal_to_lint_then_truncates() {
    let source = "
PROGRAM main
  VAR
    x : LREAL;
    y : LINT;
  END_VAR
  x := 99.9;
  y := LREAL_TO_LINT(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[1].as_i64(), 99);
}

#[test]
fn end_to_end_when_real_to_sint_then_truncates_to_range() {
    let source = "
PROGRAM main
  VAR
    x : REAL;
    y : SINT;
  END_VAR
  x := 50.7;
  y := REAL_TO_SINT(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[1].as_i32(), 50);
}

#[test]
fn end_to_end_when_lreal_to_udint_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : LREAL;
    y : UDINT;
  END_VAR
  x := 1000.0;
  y := LREAL_TO_UDINT(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[1].as_i32() as u32, 1000);
}
```

**Step 3: Create real-to-real end-to-end tests**

Create `compiler/codegen/tests/end_to_end_conv_real_to_real.rs`:

```rust
//! End-to-end tests for real-to-real type conversions.

mod common;

use common::parse_and_run;

#[test]
fn end_to_end_when_real_to_lreal_then_widens() {
    let source = "
PROGRAM main
  VAR
    x : REAL;
    y : LREAL;
  END_VAR
  x := 3.14;
  y := REAL_TO_LREAL(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    let y = bufs.vars[1].as_f64();
    assert!((y - 3.14).abs() < 0.01, "expected ~3.14, got {y}");
}

#[test]
fn end_to_end_when_lreal_to_real_then_narrows() {
    let source = "
PROGRAM main
  VAR
    x : LREAL;
    y : REAL;
  END_VAR
  x := 2.718281828;
  y := LREAL_TO_REAL(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    let y = bufs.vars[1].as_f32();
    assert!((y - 2.718282).abs() < 1e-4, "expected ~2.718282, got {y}");
}
```

**Step 4: Create integer widening end-to-end tests**

Create `compiler/codegen/tests/end_to_end_conv_int_widening.rs`:

```rust
//! End-to-end tests for integer widening type conversions.

mod common;

use common::parse_and_run;

#[test]
fn end_to_end_when_sint_to_int_then_widens() {
    let source = "
PROGRAM main
  VAR
    x : SINT;
    y : INT;
  END_VAR
  x := -100;
  y := SINT_TO_INT(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[1].as_i32(), -100);
}

#[test]
fn end_to_end_when_int_to_dint_then_widens() {
    let source = "
PROGRAM main
  VAR
    x : INT;
    y : DINT;
  END_VAR
  x := -30000;
  y := INT_TO_DINT(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[1].as_i32(), -30000);
}

#[test]
fn end_to_end_when_dint_to_lint_then_widens() {
    let source = "
PROGRAM main
  VAR
    x : DINT;
    y : LINT;
  END_VAR
  x := -1000000;
  y := DINT_TO_LINT(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[1].as_i64(), -1000000);
}

#[test]
fn end_to_end_when_usint_to_uint_then_widens() {
    let source = "
PROGRAM main
  VAR
    x : USINT;
    y : UINT;
  END_VAR
  x := 200;
  y := USINT_TO_UINT(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[1].as_i32() as u16, 200);
}

#[test]
fn end_to_end_when_uint_to_ulint_then_widens() {
    let source = "
PROGRAM main
  VAR
    x : UINT;
    y : ULINT;
  END_VAR
  x := 50000;
  y := UINT_TO_ULINT(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[1].as_i64() as u64, 50000);
}

#[test]
fn end_to_end_when_int_to_uint_then_reinterprets() {
    let source = "
PROGRAM main
  VAR
    x : INT;
    y : UINT;
  END_VAR
  x := 1000;
  y := INT_TO_UINT(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[1].as_i32() as u16, 1000);
}
```

**Step 5: Create integer narrowing end-to-end tests**

Create `compiler/codegen/tests/end_to_end_conv_int_narrowing.rs`:

```rust
//! End-to-end tests for integer narrowing type conversions.

mod common;

use common::parse_and_run;

#[test]
fn end_to_end_when_dint_to_int_then_narrows() {
    let source = "
PROGRAM main
  VAR
    x : DINT;
    y : INT;
  END_VAR
  x := 1000;
  y := DINT_TO_INT(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[1].as_i32(), 1000);
}

#[test]
fn end_to_end_when_lint_to_dint_then_narrows() {
    let source = "
PROGRAM main
  VAR
    x : LINT;
    y : DINT;
  END_VAR
  x := 42;
  y := LINT_TO_DINT(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[1].as_i32(), 42);
}

#[test]
fn end_to_end_when_dint_to_sint_overflow_then_wraps() {
    let source = "
PROGRAM main
  VAR
    x : DINT;
    y : SINT;
  END_VAR
  x := 300;
  y := DINT_TO_SINT(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    // 300 mod 256 = 44 (wrapping to i8 range)
    assert_eq!(bufs.vars[1].as_i32() as i8, 44);
}

#[test]
fn end_to_end_when_lint_to_sint_then_narrows() {
    let source = "
PROGRAM main
  VAR
    x : LINT;
    y : SINT;
  END_VAR
  x := 50;
  y := LINT_TO_SINT(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[1].as_i32(), 50);
}

#[test]
fn end_to_end_when_ulint_to_udint_then_narrows() {
    let source = "
PROGRAM main
  VAR
    x : ULINT;
    y : UDINT;
  END_VAR
  x := 1000;
  y := ULINT_TO_UDINT(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[1].as_i32() as u32, 1000);
}
```

**Step 6: Run all tests**

Run: `cargo test -p ironplc-codegen`
Expected: All tests pass.

**Step 7: Commit**

```bash
git add compiler/codegen/tests/end_to_end_conv_int_to_real.rs
git add compiler/codegen/tests/end_to_end_conv_real_to_int.rs
git add compiler/codegen/tests/end_to_end_conv_real_to_real.rs
git add compiler/codegen/tests/end_to_end_conv_int_widening.rs
git add compiler/codegen/tests/end_to_end_conv_int_narrowing.rs
git commit -m "test: add end-to-end tests for type conversion functions"
```

---

### Task 6: Update documentation

**Files:**
- Modify: `docs/reference/standard-library/functions/type-conversions.rst`
- Modify: `docs/reference/standard-library/functions/index.rst`

**Step 1: Update type-conversions.rst**

Change all "Not yet supported" entries to "Supported" for the numeric conversion categories (Integer Widening, Integer Narrowing, Signed/Unsigned, Integer to Real, Real to Integer, Real to Real). Leave string conversion categories as "Not yet supported".

Also update the header support status:

```rst
   * - **Support**
     - Supported (numeric conversions)
```

**Step 2: Update index.rst**

Change the Type Conversions entry from "Not yet supported" to "Supported (numeric)":

```rst
   * - :doc:`Type conversions <type-conversions>`
     - Type conversion functions (``*_TO_*``)
     - Supported (numeric)
```

**Step 3: Commit**

```bash
git add docs/reference/standard-library/functions/type-conversions.rst
git add docs/reference/standard-library/functions/index.rst
git commit -m "docs: mark numeric type conversions as supported"
```

---

### Task 7: Run full CI pipeline

**Step 1: Run the full CI**

Run: `cd compiler && just`
Expected: All compile, test, coverage, clippy, and fmt checks pass.

**Step 2: If clippy or fmt fails, fix and recommit**

Run: `cd compiler && just format` to auto-fix formatting if needed.

**Step 3: If all passes, the branch is ready for PR**

```bash
git push -u origin feature/type-conversion-stdlib
gh pr create --title "Add numeric type conversion functions" --body "$(cat <<'EOF'
## Summary
- Implements all 90 numeric type conversion functions (`*_TO_*`) for IEC 61131-3
- Adds 19 new BUILTIN opcodes for cross-domain conversions (int↔float, float↔float, zero-extend)
- Same-domain integer conversions are handled as no-ops via Slot sign-extension and truncation
- New `compile_type_conversion` codegen handler detects `*_TO_*` function names and routes appropriately

## Test plan
- [ ] VM-level tests for all 19 conversion opcodes
- [ ] End-to-end tests covering int→real, real→int, real→real, int widening, int narrowing
- [ ] Full CI pipeline passes (`cd compiler && just`)

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"
```
