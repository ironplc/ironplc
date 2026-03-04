# Math/Trig Standard Library Functions Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add LN, LOG, EXP, SIN, COS, TAN, ASIN, ACOS, ATAN as fully supported standard library functions for REAL and LREAL types.

**Architecture:** Each function is a 1-argument F32/F64 builtin following the existing SQRT pattern. Opcodes are defined in `container`, VM dispatch in `vm/src/builtin.rs`, codegen routing in `codegen/src/compile.rs`, and analyzer signatures in `analyzer/src/intermediates/stdlib_function.rs`.

**Tech Stack:** Rust, IEC 61131-3 structured text

---

### Task 1: Add opcode constants

**Files:**
- Modify: `compiler/container/src/opcode.rs`

**Step 1: Add 18 new opcode constants**

In `compiler/container/src/opcode.rs`, inside the `pub mod builtin` block, after the `LIMIT_U64` constant (line 533), add:

```rust
    /// LN for 32-bit floats: pops one value, pushes its natural logarithm.
    pub const LN_F32: u16 = 0x036C;

    /// LN for 64-bit floats: pops one value, pushes its natural logarithm.
    pub const LN_F64: u16 = 0x036D;

    /// LOG for 32-bit floats: pops one value, pushes its base-10 logarithm.
    pub const LOG_F32: u16 = 0x036E;

    /// LOG for 64-bit floats: pops one value, pushes its base-10 logarithm.
    pub const LOG_F64: u16 = 0x036F;

    /// EXP for 32-bit floats: pops one value, pushes e raised to that power.
    pub const EXP_F32: u16 = 0x0370;

    /// EXP for 64-bit floats: pops one value, pushes e raised to that power.
    pub const EXP_F64: u16 = 0x0371;

    /// SIN for 32-bit floats: pops one value (radians), pushes its sine.
    pub const SIN_F32: u16 = 0x0372;

    /// SIN for 64-bit floats: pops one value (radians), pushes its sine.
    pub const SIN_F64: u16 = 0x0373;

    /// COS for 32-bit floats: pops one value (radians), pushes its cosine.
    pub const COS_F32: u16 = 0x0374;

    /// COS for 64-bit floats: pops one value (radians), pushes its cosine.
    pub const COS_F64: u16 = 0x0375;

    /// TAN for 32-bit floats: pops one value (radians), pushes its tangent.
    pub const TAN_F32: u16 = 0x0376;

    /// TAN for 64-bit floats: pops one value (radians), pushes its tangent.
    pub const TAN_F64: u16 = 0x0377;

    /// ASIN for 32-bit floats: pops one value, pushes its arc sine (radians).
    pub const ASIN_F32: u16 = 0x0378;

    /// ASIN for 64-bit floats: pops one value, pushes its arc sine (radians).
    pub const ASIN_F64: u16 = 0x0379;

    /// ACOS for 32-bit floats: pops one value, pushes its arc cosine (radians).
    pub const ACOS_F32: u16 = 0x037A;

    /// ACOS for 64-bit floats: pops one value, pushes its arc cosine (radians).
    pub const ACOS_F64: u16 = 0x037B;

    /// ATAN for 32-bit floats: pops one value, pushes its arc tangent (radians).
    pub const ATAN_F32: u16 = 0x037C;

    /// ATAN for 64-bit floats: pops one value, pushes its arc tangent (radians).
    pub const ATAN_F64: u16 = 0x037D;
```

**Step 2: Update `arg_count()`**

In the same file, in the `arg_count()` function (line 544), extend the 1-argument match arm:

```rust
// Change from:
ABS_I32 | ABS_F32 | ABS_F64 | ABS_I64 | SQRT_F32 | SQRT_F64 => 1,

// Change to:
ABS_I32 | ABS_F32 | ABS_F64 | ABS_I64 | SQRT_F32 | SQRT_F64
| LN_F32 | LN_F64 | LOG_F32 | LOG_F64 | EXP_F32 | EXP_F64
| SIN_F32 | SIN_F64 | COS_F32 | COS_F64 | TAN_F32 | TAN_F64
| ASIN_F32 | ASIN_F64 | ACOS_F32 | ACOS_F64 | ATAN_F32 | ATAN_F64 => 1,
```

**Step 3: Verify it compiles**

Run: `cd compiler && cargo build`
Expected: Success (no references to the new constants yet, but they should compile)

**Step 4: Commit**

```bash
git add compiler/container/src/opcode.rs
git commit -m "Add opcode constants for math/trig builtins (LN through ATAN)"
```

---

### Task 2: Add VM dispatch handlers

**Files:**
- Modify: `compiler/vm/src/builtin.rs`

**Step 1: Add 18 match arms**

In `compiler/vm/src/builtin.rs`, in the `dispatch()` function, before the default `_ => Err(...)` arm (line 294), add the following. Each pair follows the same pattern as SQRT_F32/SQRT_F64 (lines 206-214):

```rust
        opcode::builtin::LN_F32 => {
            let a = stack.pop()?.as_f32();
            stack.push(Slot::from_f32(a.ln()))?;
            Ok(())
        }
        opcode::builtin::LN_F64 => {
            let a = stack.pop()?.as_f64();
            stack.push(Slot::from_f64(a.ln()))?;
            Ok(())
        }
        opcode::builtin::LOG_F32 => {
            let a = stack.pop()?.as_f32();
            stack.push(Slot::from_f32(a.log10()))?;
            Ok(())
        }
        opcode::builtin::LOG_F64 => {
            let a = stack.pop()?.as_f64();
            stack.push(Slot::from_f64(a.log10()))?;
            Ok(())
        }
        opcode::builtin::EXP_F32 => {
            let a = stack.pop()?.as_f32();
            stack.push(Slot::from_f32(a.exp()))?;
            Ok(())
        }
        opcode::builtin::EXP_F64 => {
            let a = stack.pop()?.as_f64();
            stack.push(Slot::from_f64(a.exp()))?;
            Ok(())
        }
        opcode::builtin::SIN_F32 => {
            let a = stack.pop()?.as_f32();
            stack.push(Slot::from_f32(a.sin()))?;
            Ok(())
        }
        opcode::builtin::SIN_F64 => {
            let a = stack.pop()?.as_f64();
            stack.push(Slot::from_f64(a.sin()))?;
            Ok(())
        }
        opcode::builtin::COS_F32 => {
            let a = stack.pop()?.as_f32();
            stack.push(Slot::from_f32(a.cos()))?;
            Ok(())
        }
        opcode::builtin::COS_F64 => {
            let a = stack.pop()?.as_f64();
            stack.push(Slot::from_f64(a.cos()))?;
            Ok(())
        }
        opcode::builtin::TAN_F32 => {
            let a = stack.pop()?.as_f32();
            stack.push(Slot::from_f32(a.tan()))?;
            Ok(())
        }
        opcode::builtin::TAN_F64 => {
            let a = stack.pop()?.as_f64();
            stack.push(Slot::from_f64(a.tan()))?;
            Ok(())
        }
        opcode::builtin::ASIN_F32 => {
            let a = stack.pop()?.as_f32();
            stack.push(Slot::from_f32(a.asin()))?;
            Ok(())
        }
        opcode::builtin::ASIN_F64 => {
            let a = stack.pop()?.as_f64();
            stack.push(Slot::from_f64(a.asin()))?;
            Ok(())
        }
        opcode::builtin::ACOS_F32 => {
            let a = stack.pop()?.as_f32();
            stack.push(Slot::from_f32(a.acos()))?;
            Ok(())
        }
        opcode::builtin::ACOS_F64 => {
            let a = stack.pop()?.as_f64();
            stack.push(Slot::from_f64(a.acos()))?;
            Ok(())
        }
        opcode::builtin::ATAN_F32 => {
            let a = stack.pop()?.as_f32();
            stack.push(Slot::from_f32(a.atan()))?;
            Ok(())
        }
        opcode::builtin::ATAN_F64 => {
            let a = stack.pop()?.as_f64();
            stack.push(Slot::from_f64(a.atan()))?;
            Ok(())
        }
```

**Step 2: Verify it compiles**

Run: `cd compiler && cargo build`
Expected: Success

**Step 3: Commit**

```bash
git add compiler/vm/src/builtin.rs
git commit -m "Add VM dispatch for math/trig builtins"
```

---

### Task 3: Add VM-level tests

**Files:**
- Create: `compiler/vm/tests/execute_builtin_math_f32.rs`
- Create: `compiler/vm/tests/execute_builtin_math_f64.rs`
- Create: `compiler/vm/tests/execute_builtin_trig_f32.rs`
- Create: `compiler/vm/tests/execute_builtin_trig_f64.rs`

These test files use hand-coded bytecode to exercise the VM dispatch directly.
Follow the pattern in `compiler/vm/tests/execute_builtin_sqrt_f32.rs`.

**Bytecode pattern for F32 (each test):**
```
0x03, 0x00, 0x00,  // LOAD_CONST_F32 pool[0]
0xC4, LO,   HI,    // BUILTIN <func_id> (little-endian)
0x1A, 0x00, 0x00,  // STORE_VAR_F32 var[0]
0xB5,              // RET_VOID
```

**Bytecode pattern for F64 (each test):**
```
0x04, 0x00, 0x00,  // LOAD_CONST_F64 pool[0]
0xC4, LO,   HI,    // BUILTIN <func_id> (little-endian)
0x1B, 0x00, 0x00,  // STORE_VAR_F64 var[0]
0xB5,              // RET_VOID
```

**Opcode bytes (little-endian):**

| Function | F32 bytes | F64 bytes |
|----------|-----------|-----------|
| LN       | 0x6C, 0x03 | 0x6D, 0x03 |
| LOG      | 0x6E, 0x03 | 0x6F, 0x03 |
| EXP      | 0x70, 0x03 | 0x71, 0x03 |
| SIN      | 0x72, 0x03 | 0x73, 0x03 |
| COS      | 0x74, 0x03 | 0x75, 0x03 |
| TAN      | 0x76, 0x03 | 0x77, 0x03 |
| ASIN     | 0x78, 0x03 | 0x79, 0x03 |
| ACOS     | 0x7A, 0x03 | 0x7B, 0x03 |
| ATAN     | 0x7C, 0x03 | 0x7D, 0x03 |

**Test values:**

| Function | Input | Expected | Tolerance |
|----------|-------|----------|-----------|
| LN       | e (2.718282 f32 / std::f64::consts::E) | 1.0 | 1e-4 f32 / 1e-12 f64 |
| LN       | 1.0 | 0.0 | 1e-5 / 1e-12 |
| LOG      | 100.0 | 2.0 | 1e-5 / 1e-12 |
| LOG      | 1.0 | 0.0 | 1e-5 / 1e-12 |
| EXP      | 0.0 | 1.0 | 1e-5 / 1e-12 |
| EXP      | 1.0 | e (2.718282) | 1e-4 / 1e-12 |
| SIN      | 0.0 | 0.0 | 1e-5 / 1e-12 |
| SIN      | PI/2 (1.5707964 f32 / std::f64::consts::FRAC_PI_2) | 1.0 | 1e-5 / 1e-12 |
| COS      | 0.0 | 1.0 | 1e-5 / 1e-12 |
| COS      | PI (3.1415927 f32 / std::f64::consts::PI) | -1.0 | 1e-5 / 1e-12 |
| TAN      | 0.0 | 0.0 | 1e-5 / 1e-12 |
| TAN      | PI/4 (0.7853982 f32 / std::f64::consts::FRAC_PI_4) | 1.0 | 1e-4 / 1e-12 |
| ASIN     | 0.0 | 0.0 | 1e-5 / 1e-12 |
| ASIN     | 1.0 | PI/2 | 1e-5 / 1e-12 |
| ACOS     | 1.0 | 0.0 | 1e-5 / 1e-12 |
| ACOS     | 0.0 | PI/2 | 1e-5 / 1e-12 |
| ATAN     | 0.0 | 0.0 | 1e-5 / 1e-12 |
| ATAN     | 1.0 | PI/4 | 1e-5 / 1e-12 |

**Step 1: Create `execute_builtin_math_f32.rs`**

Tests for LN_F32, LOG_F32, EXP_F32. Use `single_function_container_f32` helper and `b.vars[0].as_f32()` to read result. Two tests per function.

**Step 2: Create `execute_builtin_math_f64.rs`**

Same tests for LN_F64, LOG_F64, EXP_F64. Use `single_function_container_f64` and `b.vars[0].as_f64()`.

**Step 3: Create `execute_builtin_trig_f32.rs`**

Tests for SIN_F32, COS_F32, TAN_F32, ASIN_F32, ACOS_F32, ATAN_F32. Two tests per function.

**Step 4: Create `execute_builtin_trig_f64.rs`**

Same tests for F64 variants.

**Step 5: Run tests**

Run: `cd compiler && cargo test -p ironplc-vm`
Expected: All new tests pass

**Step 6: Commit**

```bash
git add compiler/vm/tests/execute_builtin_math_f32.rs compiler/vm/tests/execute_builtin_math_f64.rs compiler/vm/tests/execute_builtin_trig_f32.rs compiler/vm/tests/execute_builtin_trig_f64.rs
git commit -m "Add VM-level tests for math/trig builtins"
```

---

### Task 4: Add analyzer function signatures

**Files:**
- Modify: `compiler/analyzer/src/intermediates/stdlib_function.rs`

**Step 1: Add 9 function signatures**

In `get_numeric_functions()` (line 168), before the closing `]` at line 214, add:

```rust
        // LN: natural logarithm (ANY_REAL -> ANY_REAL)
        FunctionSignature::stdlib(
            "LN",
            TypeName::from("ANY_REAL"),
            vec![input_param("IN", "ANY_REAL")],
        ),
        // LOG: base-10 logarithm (ANY_REAL -> ANY_REAL)
        FunctionSignature::stdlib(
            "LOG",
            TypeName::from("ANY_REAL"),
            vec![input_param("IN", "ANY_REAL")],
        ),
        // EXP: natural exponential (ANY_REAL -> ANY_REAL)
        FunctionSignature::stdlib(
            "EXP",
            TypeName::from("ANY_REAL"),
            vec![input_param("IN", "ANY_REAL")],
        ),
        // SIN: sine (ANY_REAL -> ANY_REAL)
        FunctionSignature::stdlib(
            "SIN",
            TypeName::from("ANY_REAL"),
            vec![input_param("IN", "ANY_REAL")],
        ),
        // COS: cosine (ANY_REAL -> ANY_REAL)
        FunctionSignature::stdlib(
            "COS",
            TypeName::from("ANY_REAL"),
            vec![input_param("IN", "ANY_REAL")],
        ),
        // TAN: tangent (ANY_REAL -> ANY_REAL)
        FunctionSignature::stdlib(
            "TAN",
            TypeName::from("ANY_REAL"),
            vec![input_param("IN", "ANY_REAL")],
        ),
        // ASIN: arc sine (ANY_REAL -> ANY_REAL)
        FunctionSignature::stdlib(
            "ASIN",
            TypeName::from("ANY_REAL"),
            vec![input_param("IN", "ANY_REAL")],
        ),
        // ACOS: arc cosine (ANY_REAL -> ANY_REAL)
        FunctionSignature::stdlib(
            "ACOS",
            TypeName::from("ANY_REAL"),
            vec![input_param("IN", "ANY_REAL")],
        ),
        // ATAN: arc tangent (ANY_REAL -> ANY_REAL)
        FunctionSignature::stdlib(
            "ATAN",
            TypeName::from("ANY_REAL"),
            vec![input_param("IN", "ANY_REAL")],
        ),
```

**Step 2: Update the test**

In the test `get_numeric_functions_when_called_then_contains_all_functions` (line 379), change:

```rust
// From:
assert_eq!(functions.len(), 6);

// To:
assert_eq!(functions.len(), 15);
```

And add assertions for the new functions:

```rust
        assert!(functions.iter().any(|f| f.name.original() == "LN"));
        assert!(functions.iter().any(|f| f.name.original() == "LOG"));
        assert!(functions.iter().any(|f| f.name.original() == "EXP"));
        assert!(functions.iter().any(|f| f.name.original() == "SIN"));
        assert!(functions.iter().any(|f| f.name.original() == "COS"));
        assert!(functions.iter().any(|f| f.name.original() == "TAN"));
        assert!(functions.iter().any(|f| f.name.original() == "ASIN"));
        assert!(functions.iter().any(|f| f.name.original() == "ACOS"));
        assert!(functions.iter().any(|f| f.name.original() == "ATAN"));
```

**Step 3: Run tests**

Run: `cd compiler && cargo test -p ironplc-analyzer`
Expected: All tests pass

**Step 4: Commit**

```bash
git add compiler/analyzer/src/intermediates/stdlib_function.rs
git commit -m "Add analyzer signatures for math/trig functions"
```

---

### Task 5: Add codegen routing

**Files:**
- Modify: `compiler/codegen/src/compile.rs`

**Step 1: Add routing in `lookup_builtin()`**

In the `lookup_builtin()` function (line 959), before the `_ => None` default (line 1008), add 9 new match arms. Each follows the SQRT pattern (lines 1003-1007):

```rust
        "LN" => match op_width {
            OpWidth::F32 => Some(opcode::builtin::LN_F32),
            OpWidth::F64 => Some(opcode::builtin::LN_F64),
            OpWidth::W32 | OpWidth::W64 => None,
        },
        "LOG" => match op_width {
            OpWidth::F32 => Some(opcode::builtin::LOG_F32),
            OpWidth::F64 => Some(opcode::builtin::LOG_F64),
            OpWidth::W32 | OpWidth::W64 => None,
        },
        "EXP" => match op_width {
            OpWidth::F32 => Some(opcode::builtin::EXP_F32),
            OpWidth::F64 => Some(opcode::builtin::EXP_F64),
            OpWidth::W32 | OpWidth::W64 => None,
        },
        "SIN" => match op_width {
            OpWidth::F32 => Some(opcode::builtin::SIN_F32),
            OpWidth::F64 => Some(opcode::builtin::SIN_F64),
            OpWidth::W32 | OpWidth::W64 => None,
        },
        "COS" => match op_width {
            OpWidth::F32 => Some(opcode::builtin::COS_F32),
            OpWidth::F64 => Some(opcode::builtin::COS_F64),
            OpWidth::W32 | OpWidth::W64 => None,
        },
        "TAN" => match op_width {
            OpWidth::F32 => Some(opcode::builtin::TAN_F32),
            OpWidth::F64 => Some(opcode::builtin::TAN_F64),
            OpWidth::W32 | OpWidth::W64 => None,
        },
        "ASIN" => match op_width {
            OpWidth::F32 => Some(opcode::builtin::ASIN_F32),
            OpWidth::F64 => Some(opcode::builtin::ASIN_F64),
            OpWidth::W32 | OpWidth::W64 => None,
        },
        "ACOS" => match op_width {
            OpWidth::F32 => Some(opcode::builtin::ACOS_F32),
            OpWidth::F64 => Some(opcode::builtin::ACOS_F64),
            OpWidth::W32 | OpWidth::W64 => None,
        },
        "ATAN" => match op_width {
            OpWidth::F32 => Some(opcode::builtin::ATAN_F32),
            OpWidth::F64 => Some(opcode::builtin::ATAN_F64),
            OpWidth::W32 | OpWidth::W64 => None,
        },
```

**Step 2: Verify it compiles**

Run: `cd compiler && cargo build`
Expected: Success

**Step 3: Commit**

```bash
git add compiler/codegen/src/compile.rs
git commit -m "Add codegen routing for math/trig builtins"
```

---

### Task 6: Add end-to-end tests

**Files:**
- Create: `compiler/codegen/tests/end_to_end_math.rs`
- Create: `compiler/codegen/tests/end_to_end_trig.rs`

These tests compile IEC 61131-3 source through the full pipeline and verify results.
Follow the pattern in `compiler/codegen/tests/end_to_end_sqrt.rs`.

**Step 1: Create `end_to_end_math.rs`**

Tests for LN, LOG, EXP with both REAL and LREAL. Template for each test:

```rust
//! End-to-end integration tests for LN, LOG, EXP functions.

mod common;

use common::parse_and_run;

#[test]
fn end_to_end_when_ln_real_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : REAL;
    y : REAL;
  END_VAR
  x := 2.718282;
  y := LN(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    let y = bufs.vars[1].as_f32();
    assert!((y - 1.0).abs() < 1e-4, "expected ~1.0, got {y}");
}

#[test]
fn end_to_end_when_ln_lreal_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : LREAL;
    y : LREAL;
  END_VAR
  x := 1.0;
  y := LN(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    let y = bufs.vars[1].as_f64();
    assert!((y - 0.0).abs() < 1e-12, "expected 0.0, got {y}");
}
```

Test values for math:

| Test | Input | Expected |
|------|-------|----------|
| LN REAL | 2.718282 | ~1.0 (tol 1e-4) |
| LN LREAL | 1.0 | 0.0 (tol 1e-12) |
| LOG REAL | 100.0 | 2.0 (tol 1e-5) |
| LOG LREAL | 1000.0 | 3.0 (tol 1e-12) |
| EXP REAL | 0.0 | 1.0 (tol 1e-5) |
| EXP LREAL | 1.0 | std::f64::consts::E (tol 1e-12) |

**Step 2: Create `end_to_end_trig.rs`**

Tests for SIN, COS, TAN, ASIN, ACOS, ATAN with both REAL and LREAL.

Test values for trig:

| Test | Input | Expected |
|------|-------|----------|
| SIN REAL | 0.0 | 0.0 (tol 1e-5) |
| SIN LREAL | PI/2 (1.5707963267948966) | 1.0 (tol 1e-12) |
| COS REAL | 0.0 | 1.0 (tol 1e-5) |
| COS LREAL | PI | -1.0 (tol 1e-12) |
| TAN REAL | 0.0 | 0.0 (tol 1e-5) |
| TAN LREAL | PI/4 (0.7853981633974483) | 1.0 (tol 1e-12) |
| ASIN REAL | 1.0 | ~1.5707964 (PI/2, tol 1e-5) |
| ASIN LREAL | 0.0 | 0.0 (tol 1e-12) |
| ACOS REAL | 1.0 | 0.0 (tol 1e-5) |
| ACOS LREAL | 0.0 | PI/2 (tol 1e-12) |
| ATAN REAL | 1.0 | ~0.7853982 (PI/4, tol 1e-4) |
| ATAN LREAL | 0.0 | 0.0 (tol 1e-12) |

**Step 3: Run tests**

Run: `cd compiler && cargo test -p ironplc-codegen`
Expected: All new tests pass

**Step 4: Commit**

```bash
git add compiler/codegen/tests/end_to_end_math.rs compiler/codegen/tests/end_to_end_trig.rs
git commit -m "Add end-to-end tests for math/trig functions"
```

---

### Task 7: Update documentation

**Files:**
- Modify: `docs/reference/standard-library/functions/index.rst`
- Modify: `docs/reference/standard-library/functions/ln.rst`
- Modify: `docs/reference/standard-library/functions/log.rst`
- Modify: `docs/reference/standard-library/functions/exp.rst`
- Modify: `docs/reference/standard-library/functions/sin.rst`
- Modify: `docs/reference/standard-library/functions/cos.rst`
- Modify: `docs/reference/standard-library/functions/tan.rst`
- Modify: `docs/reference/standard-library/functions/asin.rst`
- Modify: `docs/reference/standard-library/functions/acos.rst`
- Modify: `docs/reference/standard-library/functions/atan.rst`

**Step 1: Update individual .rst files**

In each of the 9 .rst files, change all occurrences of `Not yet supported` to `Supported`. Each file has 3 occurrences: 1 in the header table and 2 in the signatures table (REAL and LREAL rows).

**Step 2: Update the index**

In `docs/reference/standard-library/functions/index.rst`, update the "Numeric Functions" section (lines 25-33) and the "Trigonometric Functions" section (lines 48-65):

Change LN, LOG, EXP from "Not yet supported" to "Supported" in the numeric table.
Change SIN, COS, TAN, ASIN, ACOS, ATAN from "Not yet supported" to "Supported" in the trig table.

**Step 3: Commit**

```bash
git add docs/reference/standard-library/functions/
git commit -m "Mark LN/LOG/EXP/SIN/COS/TAN/ASIN/ACOS/ATAN as supported in docs"
```

---

### Task 8: Run full CI pipeline

**Step 1: Run full CI**

Run: `cd compiler && just`
Expected: All checks pass (compile, test, coverage, clippy, fmt)

**Step 2: Fix any issues**

If clippy or fmt fails, fix and recommit.
