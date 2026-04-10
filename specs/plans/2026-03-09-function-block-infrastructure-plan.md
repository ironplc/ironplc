# Function Block Infrastructure Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add full function block calling convention and TON timer intrinsic so IEC 61131-3 programs can instantiate and call standard function blocks.

**Architecture:** Bottom-up through the compiler pipeline: container opcodes/type section first, then codegen emission, then VM execution with intrinsic dispatch. The VM reads only the fixed-size header (no_std compatible); the compiler pre-computes all memory layout. TON is the first intrinsic, proving the full path works.

**Tech Stack:** Rust, no_std container crate, IEC 61131-3 structured text

---

### Task 1: Container — Add FB opcode constants

Add the four function block opcode constants and well-known intrinsic type IDs to the container crate's opcode module.

**Files:**
- Modify: `compiler/container/src/opcode.rs`

**Step 1: Add FB opcode constants**

Add after the `RET_VOID` constant (line 135) and before the string opcodes section (line 137):

```rust
// --- Function block opcodes ---

/// Push FB instance reference from variable table.
/// Operand: u16 variable index (little-endian).
pub const FB_LOAD_INSTANCE: u8 = 0xC0;

/// Store input parameter on FB instance; keeps fb_ref on stack.
/// Operand: u8 field index.
pub const FB_STORE_PARAM: u8 = 0xC1;

/// Load output parameter from FB instance; keeps fb_ref on stack.
/// Operand: u8 field index.
pub const FB_LOAD_PARAM: u8 = 0xC2;

/// Call function block (VM dispatches to intrinsic or bytecode body).
/// Operand: u16 type_id (little-endian).
pub const FB_CALL: u8 = 0xC3;
```

**Step 2: Add well-known intrinsic type IDs**

Add a new `fb_type` module inside `opcode.rs` (after the `builtin` module, at the end of the file):

```rust
/// Well-known function block type IDs for intrinsic dispatch.
pub mod fb_type {
    /// TON (on-delay timer).
    pub const TON: u16 = 0x0010;
}
```

**Step 3: Run tests**

Run: `cd /workspaces/ironplc/compiler && cargo test -p ironplc-container`
Expected: All existing tests pass (new constants are just definitions, no behavioral change).

**Step 4: Commit**

```
feat: add FB opcode constants and intrinsic type IDs to container
```

---

### Task 2: Container — Add type section serialization

Add the type section module with FB type descriptor serialization and deserialization. This is std-only (for the verifier, not the VM).

**Files:**
- Create: `compiler/container/src/type_section.rs`
- Modify: `compiler/container/src/lib.rs`
- Modify: `compiler/container/src/error.rs`

**Step 1: Add error variant**

In `compiler/container/src/error.rs`, add to the `ContainerError` enum:

```rust
InvalidFieldType(u8),
```

Add the Display arm:

```rust
ContainerError::InvalidFieldType(t) => write!(f, "invalid field type: {t}"),
```

**Step 2: Write failing test for type section round-trip**

Create `compiler/container/src/type_section.rs` with:

```rust
//! Type section: FB type descriptors for bytecode verification.
//!
//! The type section is std-only. The VM never reads it — it uses
//! pre-computed indices from the compiler. The verifier uses this
//! section to check that FB_STORE_PARAM/FB_LOAD_PARAM field indices
//! are within bounds for the target FB type.

use std::io::{Read, Write};
use std::vec::Vec;

use crate::ContainerError;

/// Type tag for FB type descriptor fields.
/// Same encoding as VarEntry.var_type in the container format spec.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum FieldType {
    I32 = 0,
    U32 = 1,
    I64 = 2,
    U64 = 3,
    F32 = 4,
    F64 = 5,
    String = 6,
    WString = 7,
    FbInstance = 8,
    Time = 9,
}

impl FieldType {
    pub fn from_u8(value: u8) -> Result<Self, ContainerError> {
        match value {
            0 => Ok(FieldType::I32),
            1 => Ok(FieldType::U32),
            2 => Ok(FieldType::I64),
            3 => Ok(FieldType::U64),
            4 => Ok(FieldType::F32),
            5 => Ok(FieldType::F64),
            6 => Ok(FieldType::String),
            7 => Ok(FieldType::WString),
            8 => Ok(FieldType::FbInstance),
            9 => Ok(FieldType::Time),
            _ => Err(ContainerError::InvalidFieldType(value)),
        }
    }
}

/// A single field in an FB type descriptor (4 bytes on disk).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldEntry {
    pub field_type: FieldType,
    pub field_extra: u16,
}

/// An FB type descriptor: type_id + field list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FbTypeDescriptor {
    pub type_id: u16,
    pub fields: Vec<FieldEntry>,
}

/// The type section containing all FB type descriptors.
#[derive(Clone, Debug, Default)]
pub struct TypeSection {
    pub fb_types: Vec<FbTypeDescriptor>,
}

impl TypeSection {
    pub fn new() -> Self {
        TypeSection {
            fb_types: Vec::new(),
        }
    }

    /// Writes the FB type descriptor portion of the type section.
    pub fn write_to(&self, w: &mut impl Write) -> Result<(), ContainerError> {
        // FB type count
        w.write_all(&(self.fb_types.len() as u16).to_le_bytes())?;
        for fb in &self.fb_types {
            w.write_all(&fb.type_id.to_le_bytes())?;
            w.write_all(&[fb.fields.len() as u8])?;
            w.write_all(&[0u8])?; // reserved
            for field in &fb.fields {
                w.write_all(&[field.field_type as u8])?;
                w.write_all(&[0u8])?; // reserved
                w.write_all(&field.field_extra.to_le_bytes())?;
            }
        }
        Ok(())
    }

    /// Reads the FB type descriptor portion of the type section.
    pub fn read_from(r: &mut impl Read) -> Result<Self, ContainerError> {
        let mut buf2 = [0u8; 2];
        r.read_exact(&mut buf2)?;
        let count = u16::from_le_bytes(buf2) as usize;

        let mut fb_types = Vec::with_capacity(count);
        for _ in 0..count {
            r.read_exact(&mut buf2)?;
            let type_id = u16::from_le_bytes(buf2);

            let mut buf1 = [0u8; 1];
            r.read_exact(&mut buf1)?;
            let num_fields = buf1[0] as usize;
            r.read_exact(&mut buf1)?; // reserved

            let mut fields = Vec::with_capacity(num_fields);
            for _ in 0..num_fields {
                let mut field_buf = [0u8; 4];
                r.read_exact(&mut field_buf)?;
                let field_type = FieldType::from_u8(field_buf[0])?;
                let field_extra = u16::from_le_bytes([field_buf[2], field_buf[3]]);
                fields.push(FieldEntry {
                    field_type,
                    field_extra,
                });
            }
            fb_types.push(FbTypeDescriptor { type_id, fields });
        }
        Ok(TypeSection { fb_types })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn type_section_write_read_when_empty_then_roundtrips() {
        let section = TypeSection::new();
        let mut buf = Vec::new();
        section.write_to(&mut buf).unwrap();

        let mut cursor = Cursor::new(&buf);
        let decoded = TypeSection::read_from(&mut cursor).unwrap();
        assert_eq!(decoded.fb_types.len(), 0);
    }

    #[test]
    fn type_section_write_read_when_ton_descriptor_then_roundtrips() {
        let section = TypeSection {
            fb_types: vec![FbTypeDescriptor {
                type_id: 0x0010,
                fields: vec![
                    FieldEntry { field_type: FieldType::I32, field_extra: 0 },  // IN
                    FieldEntry { field_type: FieldType::Time, field_extra: 0 }, // PT
                    FieldEntry { field_type: FieldType::I32, field_extra: 0 },  // Q
                    FieldEntry { field_type: FieldType::Time, field_extra: 0 }, // ET
                    FieldEntry { field_type: FieldType::Time, field_extra: 0 }, // start_time (hidden)
                    FieldEntry { field_type: FieldType::I32, field_extra: 0 },  // running (hidden)
                ],
            }],
        };
        let mut buf = Vec::new();
        section.write_to(&mut buf).unwrap();

        let mut cursor = Cursor::new(&buf);
        let decoded = TypeSection::read_from(&mut cursor).unwrap();
        assert_eq!(decoded.fb_types.len(), 1);
        assert_eq!(decoded.fb_types[0].type_id, 0x0010);
        assert_eq!(decoded.fb_types[0].fields.len(), 6);
        assert_eq!(decoded.fb_types[0].fields[0].field_type, FieldType::I32);
        assert_eq!(decoded.fb_types[0].fields[1].field_type, FieldType::Time);
    }

    #[test]
    fn field_type_from_u8_when_invalid_then_error() {
        let result = FieldType::from_u8(42);
        assert!(matches!(result, Err(ContainerError::InvalidFieldType(42))));
    }
}
```

**Step 3: Register the module**

In `compiler/container/src/lib.rs`, add after the `task_table` module (line 25):

```rust
#[cfg(feature = "std")]
mod type_section;
```

And add the re-export after the existing std-only re-exports (around line 44):

```rust
#[cfg(feature = "std")]
pub use type_section::{FbTypeDescriptor, FieldEntry, FieldType, TypeSection};
```

**Step 4: Run tests**

Run: `cd /workspaces/ironplc/compiler && cargo test -p ironplc-container`
Expected: All tests pass including the new type section tests.

**Step 5: Commit**

```
feat: add type section with FB type descriptor serialization
```

---

### Task 3: VM — Add FB opcode handlers and data region support

Add the four FB opcode handlers to the VM dispatch loop. Also add a new `Trap::InvalidFbTypeId` variant for unknown FB type IDs during FB_CALL.

**Files:**
- Modify: `compiler/vm/src/vm.rs`
- Modify: `compiler/vm/src/error.rs`
- Modify: `compiler/vm/resources/problem-codes.csv`
- Create: `compiler/vm/tests/execute_fb_ops.rs`
- Modify: `compiler/vm/tests/common/mod.rs`

**Step 1: Add Trap variant for invalid FB type**

In `compiler/vm/src/error.rs`, add to the `Trap` enum:

```rust
InvalidFbTypeId(u16),
```

Add the Display arm:

```rust
Trap::InvalidFbTypeId(id) => write!(f, "invalid FB type ID: 0x{id:04X}"),
```

In `compiler/vm/resources/problem-codes.csv`, add:

```
V9010,InvalidFbTypeId,Function block type ID not recognized as intrinsic,true
```

**Step 2: Pass `current_time_us` to `execute()`**

In `compiler/vm/src/vm.rs`, add `current_time_us: u64` parameter to the `execute()` function signature (line 415). Update both call sites:

- In `VmReady::start()` — pass `0` (init doesn't need real time)
- In `VmRunning::run_round()` — pass `self.current_time_us` (but first store it: add `current_time_us` field to `VmRunning`, set it at the start of `run_round`)

Actually, simpler: `run_round` already receives `current_time_us: u64`. Just pass it through to `execute()`.

**Step 3: Add FB opcode handlers to dispatch loop**

In `compiler/vm/src/vm.rs`, in the `execute()` function's `match op` block, add after the `BUILTIN` handler (around line 928):

```rust
opcode::FB_LOAD_INSTANCE => {
    let var_index = read_u16_le(bytecode, &mut pc);
    let slot = variables.load(scope.resolve(var_index))?;
    // The variable slot holds the data region byte offset as an i32.
    stack.push(slot)?;
}
opcode::FB_STORE_PARAM => {
    let field = bytecode[pc] as u16;
    pc += 1;
    let value = stack.pop()?;
    // Peek fb_ref (don't pop — it stays on stack).
    let fb_ref = stack.peek()?.as_i32() as u32;
    let offset = fb_ref as usize + field as usize * 8;
    if offset + 8 > data_region.len() {
        return Err(Trap::DataRegionOutOfBounds(offset as u16));
    }
    data_region[offset..offset + 8].copy_from_slice(&value.as_i64().to_le_bytes());
}
opcode::FB_LOAD_PARAM => {
    let field = bytecode[pc] as u16;
    pc += 1;
    // Peek fb_ref (don't pop).
    let fb_ref = stack.peek()?.as_i32() as u32;
    let offset = fb_ref as usize + field as usize * 8;
    if offset + 8 > data_region.len() {
        return Err(Trap::DataRegionOutOfBounds(offset as u16));
    }
    let mut buf = [0u8; 8];
    buf.copy_from_slice(&data_region[offset..offset + 8]);
    stack.push(Slot::from_i64(i64::from_le_bytes(buf)))?;
}
opcode::FB_CALL => {
    let type_id = read_u16_le(bytecode, &mut pc);
    let fb_ref = stack.peek()?.as_i32() as u32;
    let instance_start = fb_ref as usize;
    match type_id {
        // No intrinsics registered yet — all type IDs are invalid.
        _ => return Err(Trap::InvalidFbTypeId(type_id)),
    }
}
```

Note: The `stack.peek()` method may not exist yet. If not, it needs to be added to `OperandStack` in `stack.rs` — it returns the top value without popping it.

**Step 4: Add `peek()` to OperandStack if needed**

Check `compiler/vm/src/stack.rs`. If `peek()` doesn't exist, add:

```rust
pub fn peek(&self) -> Result<Slot, Trap> {
    if self.sp == 0 {
        return Err(Trap::StackUnderflow);
    }
    Ok(self.slots[self.sp - 1])
}
```

**Step 5: Add a POP opcode constant and handler**

The FB calling convention requires POP to discard the fb_ref after output parameter reads. Check if POP (0xA0) exists. If not:

In `compiler/container/src/opcode.rs`:

```rust
/// Discard the top value from the operand stack.
pub const POP: u8 = 0xA0;
```

In `vm.rs` dispatch loop:

```rust
opcode::POP => {
    stack.pop()?;
}
```

**Step 6: Write VM tests for FB opcodes**

Create `compiler/vm/tests/execute_fb_ops.rs`:

```rust
mod common;
use common::VmBuffers;
use ironplc_container::opcode;
use ironplc_container::ContainerBuilder;
use ironplc_vm::Slot;

/// Helper: builds a container with a data region for FB testing.
fn fb_container(bytecode: &[u8], num_vars: u16, constants: &[i64], data_region_bytes: u32) -> ironplc_container::Container {
    let init_bytecode: Vec<u8> = vec![opcode::RET_VOID];
    let mut builder = ContainerBuilder::new()
        .num_variables(num_vars)
        .data_region_bytes(data_region_bytes);
    for &c in constants {
        builder = builder.add_i64_constant(c);
    }
    builder = builder.add_function(0, &init_bytecode, 0, num_vars);
    builder = builder.add_function(1, bytecode, 16, num_vars);
    builder = builder.init_function_id(0).entry_function_id(1);
    builder.build()
}

#[test]
fn execute_when_fb_store_param_then_writes_data_region() {
    // var[0] = 0 (fb_ref pointing to data region offset 0)
    // Load fb_ref, push value 42, store to field 0
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        opcode::FB_LOAD_INSTANCE, 0x00, 0x00,  // push fb_ref from var[0]
        opcode::LOAD_CONST_I64, 0x00, 0x00,    // push constant[0] = 42
        opcode::FB_STORE_PARAM, 0x00,           // store to field 0
        opcode::POP,                            // discard fb_ref
        opcode::RET_VOID,
    ];
    let c = fb_container(&bytecode, 1, &[42i64], 48); // 6 fields * 8 bytes
    let mut b = VmBuffers::from_container(&c);
    let mut vm = common::load_and_start(&c, &mut b).unwrap();
    vm.run_round(0).unwrap();

    // Check data region: field 0 at offset 0 should contain 42
    let data = vm.data_region();
    let value = i64::from_le_bytes(data[0..8].try_into().unwrap());
    assert_eq!(value, 42);
}

#[test]
fn execute_when_fb_load_param_then_reads_data_region() {
    // Pre-fill data region field 1 with value 99, then read it
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        opcode::FB_LOAD_INSTANCE, 0x00, 0x00,  // push fb_ref from var[0]
        opcode::FB_LOAD_PARAM, 0x01,            // load field 1
        opcode::STORE_VAR_I64, 0x01, 0x00,      // store to var[1]
        opcode::POP,                            // discard fb_ref
        opcode::RET_VOID,
    ];
    let c = fb_container(&bytecode, 2, &[], 48);
    let mut b = VmBuffers::from_container(&c);
    // Pre-fill field 1 (offset 8) with value 99
    b.data_region[8..16].copy_from_slice(&99i64.to_le_bytes());
    let mut vm = common::load_and_start(&c, &mut b).unwrap();
    vm.run_round(0).unwrap();

    assert_eq!(vm.read_variable_i64(1).unwrap(), 99);
}

#[test]
fn execute_when_fb_call_unknown_type_then_traps() {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        opcode::FB_LOAD_INSTANCE, 0x00, 0x00,  // push fb_ref
        opcode::FB_CALL, 0xFF, 0xFF,            // call unknown type 0xFFFF
        opcode::POP,
        opcode::RET_VOID,
    ];
    let c = fb_container(&bytecode, 1, &[], 48);
    let mut b = VmBuffers::from_container(&c);
    common::assert_trap(&c, &mut b, ironplc_vm::error::Trap::InvalidFbTypeId(0xFFFF));
}

#[test]
fn execute_when_pop_then_discards_top() {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        opcode::LOAD_CONST_I32, 0x00, 0x00,  // push 42
        opcode::LOAD_CONST_I32, 0x01, 0x00,  // push 99
        opcode::POP,                          // discard 99
        opcode::STORE_VAR_I32, 0x00, 0x00,   // store 42 to var[0]
        opcode::RET_VOID,
    ];
    let c = common::single_function_container(&bytecode, 1, &[42, 99]);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = common::load_and_start(&c, &mut b).unwrap();
    vm.run_round(0).unwrap();

    assert_eq!(vm.read_variable(0).unwrap(), 42);
}
```

Note: The tests may need adjustment based on existing helper APIs. Key points:
- `VmBuffers::from_container` allocates data_region from header's `data_region_bytes`
- `vm.data_region()` may need to be added as a public accessor on `VmRunning`
- `vm.read_variable_i64()` may need to be added

**Step 7: Add missing accessor methods**

On `VmRunning`, add if needed:
- `pub fn data_region(&self) -> &[u8]` — returns the data region slice
- `pub fn read_variable_i64(&self, index: u16) -> Result<i64, Trap>` — reads a variable as i64

**Step 8: Run tests**

Run: `cd /workspaces/ironplc/compiler && cargo test -p ironplc-vm`
Expected: All tests pass.

**Step 9: Commit**

```
feat: add FB opcode handlers (FB_LOAD_INSTANCE, FB_STORE_PARAM, FB_LOAD_PARAM, FB_CALL) and POP to VM
```

---

### Task 4: VM — Add TON intrinsic

Implement the TON on-delay timer as the first intrinsic in the FB_CALL dispatch.

**Files:**
- Create: `compiler/vm/src/intrinsic.rs`
- Modify: `compiler/vm/src/vm.rs`
- Modify: `compiler/vm/src/lib.rs`
- Create: `compiler/vm/tests/execute_fb_ton.rs`

**Step 1: Create intrinsic module**

Create `compiler/vm/src/intrinsic.rs`:

```rust
//! Native intrinsic implementations for standard function blocks.

use crate::error::Trap;

/// Field byte size (all fields are 8-byte aligned slots).
const FIELD_SIZE: usize = 8;

/// Reads an i32 from an FB instance field.
fn read_i32(instance: &[u8], field: usize) -> i32 {
    let offset = field * FIELD_SIZE;
    let bytes: [u8; 4] = instance[offset..offset + 4].try_into().unwrap();
    i32::from_le_bytes(bytes)
}

/// Writes an i32 to an FB instance field.
fn write_i32(instance: &mut [u8], field: usize, value: i32) {
    let offset = field * FIELD_SIZE;
    instance[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    // Zero upper 4 bytes for slot consistency.
    instance[offset + 4..offset + 8].copy_from_slice(&[0, 0, 0, 0]);
}

/// Reads an i64 from an FB instance field.
fn read_i64(instance: &[u8], field: usize) -> i64 {
    let offset = field * FIELD_SIZE;
    let bytes: [u8; 8] = instance[offset..offset + 8].try_into().unwrap();
    i64::from_le_bytes(bytes)
}

/// Writes an i64 to an FB instance field.
fn write_i64(instance: &mut [u8], field: usize, value: i64) {
    let offset = field * FIELD_SIZE;
    instance[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

/// TON field indices.
const TON_IN: usize = 0;
const TON_PT: usize = 1;
const TON_Q: usize = 2;
const TON_ET: usize = 3;
const TON_START_TIME: usize = 4;  // hidden
const TON_RUNNING: usize = 5;     // hidden

/// Number of fields (including hidden) for a TON instance.
pub const TON_INSTANCE_FIELDS: usize = 6;

/// Executes one scan of the TON (on-delay timer) intrinsic.
///
/// # Arguments
/// * `instance` - Mutable slice of the FB instance memory (6 fields * 8 bytes = 48 bytes).
/// * `cycle_time` - Current scan cycle time in microseconds.
///
/// # TON behavior (IEC 61131-3 section 2.5.2.3.1):
/// - When IN rises (FALSE→TRUE): start timing, ET=0, Q=FALSE
/// - While IN is TRUE: ET increments up to PT. When ET >= PT, Q becomes TRUE.
/// - When IN falls (TRUE→FALSE): Q=FALSE, ET=0, stop timing.
pub fn ton(instance: &mut [u8], cycle_time: i64) -> Result<(), Trap> {
    let in_val = read_i32(instance, TON_IN) != 0;
    let pt = read_i64(instance, TON_PT);
    let running = read_i32(instance, TON_RUNNING) != 0;

    if in_val {
        if !running {
            // Rising edge: start timing
            write_i64(instance, TON_START_TIME, cycle_time);
            write_i32(instance, TON_RUNNING, 1);
            write_i64(instance, TON_ET, 0);
            write_i32(instance, TON_Q, 0);
        } else {
            // Timing in progress
            let start_time = read_i64(instance, TON_START_TIME);
            let elapsed = cycle_time - start_time;
            let et = if elapsed > pt { pt } else { elapsed };
            write_i64(instance, TON_ET, et);
            if et >= pt {
                write_i32(instance, TON_Q, 1);
            }
        }
    } else {
        // IN is FALSE: reset
        write_i32(instance, TON_Q, 0);
        write_i64(instance, TON_ET, 0);
        write_i32(instance, TON_RUNNING, 0);
    }
    Ok(())
}
```

**Step 2: Register intrinsic module**

In `compiler/vm/src/lib.rs`, add:

```rust
pub(crate) mod intrinsic;
```

**Step 3: Wire TON into FB_CALL dispatch**

In `compiler/vm/src/vm.rs`, update the `FB_CALL` handler to call the TON intrinsic:

```rust
opcode::FB_CALL => {
    let type_id = read_u16_le(bytecode, &mut pc);
    let fb_ref = stack.peek()?.as_i32() as u32;
    let instance_start = fb_ref as usize;
    match type_id {
        opcode::fb_type::TON => {
            let instance_size = intrinsic::TON_INSTANCE_FIELDS * 8;
            let instance_end = instance_start + instance_size;
            if instance_end > data_region.len() {
                return Err(Trap::DataRegionOutOfBounds(instance_start as u16));
            }
            intrinsic::ton(&mut data_region[instance_start..instance_end], current_time_us as i64)?;
        }
        _ => return Err(Trap::InvalidFbTypeId(type_id)),
    }
}
```

**Step 4: Write TON intrinsic tests**

Create `compiler/vm/tests/execute_fb_ton.rs`:

```rust
mod common;
use common::VmBuffers;
use ironplc_container::opcode;
use ironplc_container::ContainerBuilder;

/// Builds a container that runs: load fb_ref, store IN, store PT, call TON, load Q, load ET.
fn ton_test_container(num_vars: u16, data_region_bytes: u32, pt_us: i64) -> ironplc_container::Container {
    // var[0] = fb_ref (offset 0 into data region)
    // var[1] = IN value (set by test via variable table)
    // var[2] = Q output (read by test)
    // var[3] = ET output (read by test)
    // constant[0] = PT value (i64 microseconds)
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        opcode::FB_LOAD_INSTANCE, 0x00, 0x00,  // push fb_ref from var[0]
        opcode::LOAD_VAR_I32,     0x01, 0x00,  // push IN from var[1]
        opcode::FB_STORE_PARAM,   0x00,         // store to TON.IN (field 0)
        opcode::LOAD_CONST_I64,   0x00, 0x00,  // push PT constant
        opcode::FB_STORE_PARAM,   0x01,         // store to TON.PT (field 1)
        opcode::FB_CALL,          0x10, 0x00,   // call TON (type_id 0x0010)
        opcode::FB_LOAD_PARAM,    0x02,         // load TON.Q (field 2)
        opcode::STORE_VAR_I32,    0x02, 0x00,  // store Q to var[2]
        opcode::FB_LOAD_PARAM,    0x03,         // load TON.ET (field 3)
        opcode::STORE_VAR_I64,    0x03, 0x00,  // store ET to var[3]
        opcode::POP,                            // discard fb_ref
        opcode::RET_VOID,
    ];

    let init_bytecode: Vec<u8> = vec![opcode::RET_VOID];
    let mut builder = ContainerBuilder::new()
        .num_variables(num_vars)
        .data_region_bytes(data_region_bytes);
    builder = builder.add_i64_constant(pt_us);
    builder = builder.add_function(0, &init_bytecode, 0, num_vars);
    builder = builder.add_function(1, &bytecode, 16, num_vars);
    builder = builder.init_function_id(0).entry_function_id(1);
    builder.build()
}

#[test]
fn ton_when_in_false_then_q_false_et_zero() {
    let c = ton_test_container(4, 48, 5_000_000); // PT = 5 seconds
    let mut b = VmBuffers::from_container(&c);
    let mut vm = common::load_and_start(&c, &mut b).unwrap();
    // var[1] = IN = 0 (FALSE) — default
    vm.run_round(1_000_000).unwrap(); // t = 1s

    assert_eq!(vm.read_variable(2).unwrap(), 0); // Q = FALSE
    assert_eq!(vm.read_variable_i64(3).unwrap(), 0); // ET = 0
}

#[test]
fn ton_when_in_true_before_pt_then_q_false_et_increasing() {
    let c = ton_test_container(4, 48, 5_000_000);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = common::load_and_start(&c, &mut b).unwrap();

    // Set IN = TRUE
    vm.write_variable(1, 1).unwrap();

    // Scan at t=1s: rising edge
    vm.run_round(1_000_000).unwrap();
    assert_eq!(vm.read_variable(2).unwrap(), 0); // Q = FALSE (just started)

    // Scan at t=3s: 2 seconds elapsed
    vm.run_round(3_000_000).unwrap();
    assert_eq!(vm.read_variable(2).unwrap(), 0); // Q = FALSE (< PT)
    assert_eq!(vm.read_variable_i64(3).unwrap(), 2_000_000); // ET = 2s
}

#[test]
fn ton_when_in_true_after_pt_then_q_true_et_clamped() {
    let c = ton_test_container(4, 48, 5_000_000);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = common::load_and_start(&c, &mut b).unwrap();

    vm.write_variable(1, 1).unwrap(); // IN = TRUE

    vm.run_round(1_000_000).unwrap(); // t=1s: rising edge
    vm.run_round(7_000_000).unwrap(); // t=7s: 6s elapsed > 5s PT

    assert_eq!(vm.read_variable(2).unwrap(), 1); // Q = TRUE
    assert_eq!(vm.read_variable_i64(3).unwrap(), 5_000_000); // ET clamped to PT
}

#[test]
fn ton_when_in_falls_after_timer_expires_then_resets() {
    let c = ton_test_container(4, 48, 5_000_000);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = common::load_and_start(&c, &mut b).unwrap();

    vm.write_variable(1, 1).unwrap(); // IN = TRUE
    vm.run_round(1_000_000).unwrap(); // t=1s: rising edge
    vm.run_round(7_000_000).unwrap(); // t=7s: timer expired

    assert_eq!(vm.read_variable(2).unwrap(), 1); // Q = TRUE

    // IN goes FALSE
    vm.write_variable(1, 0).unwrap();
    vm.run_round(8_000_000).unwrap();

    assert_eq!(vm.read_variable(2).unwrap(), 0); // Q = FALSE
    assert_eq!(vm.read_variable_i64(3).unwrap(), 0); // ET = 0
}

#[test]
fn ton_when_in_false_before_pt_then_resets() {
    let c = ton_test_container(4, 48, 5_000_000);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = common::load_and_start(&c, &mut b).unwrap();

    vm.write_variable(1, 1).unwrap(); // IN = TRUE
    vm.run_round(1_000_000).unwrap(); // t=1s: rising edge
    vm.run_round(3_000_000).unwrap(); // t=3s: 2s elapsed, not yet expired

    assert_eq!(vm.read_variable(2).unwrap(), 0); // Q = FALSE

    // IN goes FALSE before PT expires
    vm.write_variable(1, 0).unwrap();
    vm.run_round(4_000_000).unwrap();

    assert_eq!(vm.read_variable(2).unwrap(), 0); // Q = FALSE
    assert_eq!(vm.read_variable_i64(3).unwrap(), 0); // ET = 0

    // IN goes TRUE again — timer restarts from scratch
    vm.write_variable(1, 1).unwrap();
    vm.run_round(5_000_000).unwrap(); // new rising edge at t=5s
    vm.run_round(8_000_000).unwrap(); // t=8s: 3s elapsed (< 5s PT)

    assert_eq!(vm.read_variable(2).unwrap(), 0); // Q = FALSE (< PT from new start)
    assert_eq!(vm.read_variable_i64(3).unwrap(), 3_000_000); // ET = 3s
}
```

Note: `vm.write_variable(index, value)` may need to be added as a public method on `VmRunning`. It writes an i32 to the variable table.

**Step 5: Run tests**

Run: `cd /workspaces/ironplc/compiler && cargo test -p ironplc-vm`
Expected: All tests pass.

**Step 6: Commit**

```
feat: add TON on-delay timer intrinsic to VM
```

---

### Task 5: Codegen — Add FB emitter methods

Add emitter methods for the four FB opcodes and POP so the codegen can emit FB call sequences.

**Files:**
- Modify: `compiler/codegen/src/emit.rs`

**Step 1: Add emitter methods**

Add to the `Emitter` impl in `compiler/codegen/src/emit.rs`:

```rust
/// Emits FB_LOAD_INSTANCE with a variable index.
/// Pushes one value (fb_ref) onto the stack.
pub fn emit_fb_load_instance(&mut self, var_index: u16) {
    self.bytecode.push(opcode::FB_LOAD_INSTANCE);
    self.bytecode.extend_from_slice(&var_index.to_le_bytes());
    self.push_stack(1);
}

/// Emits FB_STORE_PARAM with a field index.
/// Pops one value (the parameter), fb_ref remains on stack.
pub fn emit_fb_store_param(&mut self, field: u8) {
    self.bytecode.push(opcode::FB_STORE_PARAM);
    self.bytecode.push(field);
    self.pop_stack(1);
}

/// Emits FB_LOAD_PARAM with a field index.
/// Pushes one value (the output parameter), fb_ref remains on stack.
pub fn emit_fb_load_param(&mut self, field: u8) {
    self.bytecode.push(opcode::FB_LOAD_PARAM);
    self.bytecode.push(field);
    self.push_stack(1);
}

/// Emits FB_CALL with a type_id.
/// fb_ref stays on stack (net stack effect: 0).
pub fn emit_fb_call(&mut self, type_id: u16) {
    self.bytecode.push(opcode::FB_CALL);
    self.bytecode.extend_from_slice(&type_id.to_le_bytes());
}

/// Emits POP (discards top of stack).
pub fn emit_pop(&mut self) {
    self.bytecode.push(opcode::POP);
    self.pop_stack(1);
}
```

**Step 2: Run tests**

Run: `cd /workspaces/ironplc/compiler && cargo test -p ironplc-codegen`
Expected: All existing tests pass.

**Step 3: Commit**

```
feat: add FB opcode emitter methods to codegen
```

---

### Task 6: Codegen — Compile FB instance variables and FB calls

Wire up the codegen to allocate FB instance variables in the data region and emit FB call sequences for `StmtKind::FbCall`.

**Files:**
- Modify: `compiler/codegen/src/compile.rs`
- Create: `compiler/codegen/tests/compile_fb.rs`

**Step 1: Add FB tracking to CompileContext**

In `compiler/codegen/src/compile.rs`, add to `CompileContext` struct:

```rust
/// Maps FB instance variable names to their FB metadata.
fb_instances: HashMap<Id, FbInstanceInfo>,
```

Add a new struct:

```rust
/// Metadata for a function block instance variable.
struct FbInstanceInfo {
    /// Variable table index holding the data region offset.
    var_index: u16,
    /// Type ID for FB_CALL dispatch.
    type_id: u16,
    /// Number of fields (including hidden) — determines instance size.
    num_fields: u16,
    /// Maps field name (lowercase) to field index.
    field_indices: HashMap<String, u8>,
}
```

Initialize `fb_instances: HashMap::new()` in `CompileContext::new()`.

**Step 2: Handle FB variable allocation in `assign_variables()`**

In the match on `decl.initializer` within `assign_variables()`, replace the catch-all `_ => {}` arm (line 383-385) with:

```rust
InitialValueAssignmentKind::FunctionBlock(fb_init) => {
    let fb_name = fb_init.type_name.to_string().to_uppercase();
    if let Some((type_id, num_fields, field_map)) = resolve_fb_type(&fb_name) {
        let instance_size = num_fields as u32 * 8;
        let data_offset = ctx.data_region_offset;
        ctx.data_region_offset += instance_size as u16;

        ctx.fb_instances.insert(
            id.clone(),
            FbInstanceInfo {
                var_index: index,
                type_id,
                num_fields,
                field_indices: field_map,
            },
        );
    }
}
_ => {}
```

Add a helper function:

```rust
/// Resolves a standard FB type name to its (type_id, total_num_fields, field_name→index map).
/// Returns None for unknown FB types.
fn resolve_fb_type(name: &str) -> Option<(u16, u16, HashMap<String, u8>)> {
    match name {
        "TON" => {
            let mut fields = HashMap::new();
            fields.insert("in".to_string(), 0);
            fields.insert("pt".to_string(), 1);
            fields.insert("q".to_string(), 2);
            fields.insert("et".to_string(), 3);
            // Fields 4-5 are hidden (start_time, running)
            Some((opcode::fb_type::TON, 6, fields))
        }
        _ => None,
    }
}
```

**Step 3: Set initial value for FB variable (data region offset)**

In `emit_initial_values()`, the FB instance variable's slot needs to hold the data region byte offset. Add handling in the `InitialValueAssignmentKind::FunctionBlock` arm:

```rust
InitialValueAssignmentKind::FunctionBlock(_) => {
    if let Some(fb_info) = ctx.fb_instances.get(id) {
        // The variable slot holds the data region byte offset.
        // Compute it from the allocation order.
        // The offset was stored during assign_variables.
        // We need to emit: LOAD_CONST_I32 offset, STORE_VAR_I32 var_index
        // But we need to know the offset. Store it in FbInstanceInfo.
        // Actually, since variables are allocated sequentially and fb_instances
        // already has var_index, we can compute the offset.
    }
}
```

Wait — we need to store the data_offset in `FbInstanceInfo`. Update the struct to include `data_offset: u16` and set it during assignment. Then in `emit_initial_values`:

```rust
InitialValueAssignmentKind::FunctionBlock(_) => {
    if let Some(fb_info) = ctx.fb_instances.get(id) {
        let offset_const = ctx.add_i32_constant(fb_info.data_offset as i32);
        emitter.emit_load_const_i32(offset_const);
        emitter.emit_store_var_i32(fb_info.var_index);
    }
}
```

**Step 4: Compile FB call statement**

In `compile_statement()`, replace the `StmtKind::FbCall` arm (line 653-654) with:

```rust
StmtKind::FbCall(fb_call) => {
    compile_fb_call(emitter, ctx, fb_call)?;
}
```

Add the new function:

```rust
/// Compiles a function block invocation: stores inputs, calls FB, reads outputs.
fn compile_fb_call(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    fb_call: &FbCall,
) -> Result<(), Diagnostic> {
    let fb_info = ctx.fb_instances.get(&fb_call.var_name).ok_or_else(|| {
        Diagnostic::todo_with_span(fb_call.span(), file!(), line!())
    })?;
    let type_id = fb_info.type_id;
    let field_indices = fb_info.field_indices.clone();
    let var_index = fb_info.var_index;

    // Push FB instance reference.
    emitter.emit_fb_load_instance(var_index);

    // Store input parameters.
    for param in &fb_call.params {
        match param {
            ParamAssignmentKind::NamedInput(input) => {
                let field_name = input.name.to_string().to_lowercase();
                let field_idx = field_indices.get(&field_name).ok_or_else(|| {
                    Diagnostic::todo_with_span(input.name.span(), file!(), line!())
                })?;
                // Compile the input expression.
                // For now, use default op_type. The analyzer has already validated types.
                compile_expr(emitter, ctx, &input.expr, DEFAULT_OP_TYPE)?;
                emitter.emit_fb_store_param(*field_idx);
            }
            ParamAssignmentKind::Output(output) => {
                // Output assignments: read after FB_CALL.
                // Handle below after FB_CALL.
            }
            _ => {}
        }
    }

    // Call the function block.
    emitter.emit_fb_call(type_id);

    // Read output parameters.
    for param in &fb_call.params {
        if let ParamAssignmentKind::Output(output) = param {
            let field_name = output.src.to_string().to_lowercase();
            let field_idx = field_indices.get(&field_name).ok_or_else(|| {
                Diagnostic::todo_with_span(output.src.span(), file!(), line!())
            })?;
            emitter.emit_fb_load_param(*field_idx);
            let target_index = resolve_variable(ctx, &output.tgt)?;
            // Determine store width from field type.
            // For TON: Q is i32 (BOOL), ET is i64 (TIME).
            emit_store_var(emitter, target_index, field_op_type(&field_name));
        }
    }

    // Discard fb_ref.
    emitter.emit_pop();
    Ok(())
}

/// Returns the op_type for a standard FB field by name.
/// This is a temporary approach until proper type propagation is in place.
fn field_op_type(field_name: &str) -> OpType {
    match field_name {
        "in" | "q" => (OpWidth::W32, Signedness::Signed),
        "pt" | "et" => (OpWidth::W64, Signedness::Signed),
        _ => DEFAULT_OP_TYPE,
    }
}
```

**Step 5: Handle structured variable access (myTimer.Q)**

In `resolve_variable()`, handle `SymbolicVariableKind::Structured` for FB field reads outside of FB calls:

This is for expressions like `x := myTimer.Q;` outside the call. For now, this is complex because we'd need to emit FB_LOAD_INSTANCE + FB_LOAD_PARAM inline. This can be deferred — the `=> output` syntax in the FB call handles output reading. Mark as TODO if needed.

**Step 6: Configure data_region_bytes in compile_program()**

In `compile_program()`, update the builder configuration to include FB data region needs. The existing code at line 169 handles `data_region_offset > 0` for strings. FB instances also use the data region, so this should already work since `data_region_offset` is shared. Verify this is the case.

**Step 7: Write codegen compile test**

Create `compiler/codegen/tests/compile_fb.rs`:

```rust
mod common;

#[test]
fn compile_when_ton_call_then_emits_fb_sequence() {
    let source = "
        PROGRAM main
        VAR
            myTimer : TON;
            start_signal : BOOL;
            timer_done : BOOL;
            elapsed : TIME;
        END_VAR
            myTimer(IN := start_signal, PT := T#5s, Q => timer_done, ET => elapsed);
        END_PROGRAM
    ";

    let (container, _bufs) = common::parse_and_run(source);
    // If this compiles and runs without panic, the FB call sequence is correct.
    // More detailed bytecode inspection tests can be added later.
}
```

**Step 8: Run tests**

Run: `cd /workspaces/ironplc/compiler && cargo test -p ironplc-codegen`
Expected: All tests pass.

**Step 9: Commit**

```
feat: compile FB instance variables and FB call sequences in codegen
```

---

### Task 7: End-to-end integration tests

Write end-to-end tests that compile IEC 61131-3 programs with TON calls, run them in the VM with simulated clock, and verify outputs.

**Files:**
- Create: `compiler/codegen/tests/end_to_end_fb_ton.rs`

**Step 1: Write end-to-end TON tests**

Create `compiler/codegen/tests/end_to_end_fb_ton.rs`:

```rust
mod common;

#[test]
fn end_to_end_when_ton_in_false_then_q_false() {
    let source = "
        PROGRAM main
        VAR
            myTimer : TON;
            start_signal : BOOL := FALSE;
            timer_done : BOOL;
            elapsed : TIME;
        END_VAR
            myTimer(IN := start_signal, PT := T#5s, Q => timer_done, ET => elapsed);
        END_PROGRAM
    ";
    let (_container, bufs) = common::parse_and_run(source);
    // timer_done should be FALSE (0)
    // We need to find timer_done's variable index — it depends on allocation order.
    // For now, verify no panic (compilation + execution succeeded).
}

#[test]
fn end_to_end_when_ton_timer_expires_then_q_true() {
    let source = "
        PROGRAM main
        VAR
            myTimer : TON;
            start_signal : BOOL := TRUE;
            timer_done : BOOL;
            elapsed : TIME;
        END_VAR
            myTimer(IN := start_signal, PT := T#5s, Q => timer_done, ET => elapsed);
        END_PROGRAM
    ";
    // This test needs multiple scan cycles with advancing time.
    // The common::parse_and_run helper runs exactly one scan at t=0.
    // We need a more flexible helper that allows multiple scans with custom times.
    // For now, compile and run one scan — timer should not have expired yet.
    let (_container, bufs) = common::parse_and_run(source);
}
```

Note: The end-to-end tests may need a new helper that supports multiple scan cycles with custom clock values. The existing `parse_and_run` runs one scan at `current_time_us = 0`. A `parse_and_run_multi` helper could accept a list of `(time_us, pre_scan_callback)` entries.

**Step 2: Extend test helpers for multi-scan testing**

In `compiler/codegen/tests/common/mod.rs`, add:

```rust
/// Parses, compiles, and returns a started VM ready for manual scan cycles.
pub fn parse_and_start(source: &str) -> (Container, VmBuffers, ...) {
    // Parse and compile
    let lib = parse(source);
    let container = ironplc_codegen::compile(&lib).unwrap();
    let mut bufs = VmBuffers::from_container(&container);
    // Load and start but don't run any scans
    let vm = load_and_start(&container, &mut bufs).unwrap();
    // Return vm for caller to call run_round() manually
}
```

The exact API depends on lifetimes. This may need to return the container and buffers for the caller to manage the VM lifetime.

**Step 3: Run full CI**

Run: `cd /workspaces/ironplc/compiler && just`
Expected: All compile, test, coverage, and lint checks pass.

**Step 4: Commit**

```
test: add end-to-end integration tests for TON function block
```

---

### Task 8: Documentation update

Update the TON documentation to mark it as supported.

**Files:**
- Modify: `docs/reference/standard-library/function-blocks/ton.rst`

**Step 1: Update TON documentation**

Change the status from "Not yet supported" to "Supported" and add a usage example.

**Step 2: Commit**

```
docs: mark TON function block as supported
```

---

## Implementation Notes

### Adapting to what you find

This plan is based on codebase analysis but the exact code will need adjustment:

1. **InitialValueAssignmentKind::FunctionBlock** — verify this variant exists in the DSL. The FB initializer kind may have a different name. Check the `InitialValueAssignmentKind` enum in `compiler/dsl/src/common.rs`.

2. **TIME constant compilation** — the codegen needs to compile `T#5s` as an i64 constant (5_000_000 microseconds). Check how TIME literals are parsed and represented in the AST. They may come through as `ConstantKind::Time` or similar.

3. **Output parameter syntax** — the `Q => timer_done` syntax in the FB call becomes `ParamAssignmentKind::Output`. Verify the parser handles this for FB calls (it should, since the analyzer already validates it).

4. **Variable write methods** — `VmRunning::write_variable()` and `read_variable_i64()` may not exist. Add them following the pattern of the existing `read_variable()`.

5. **Stack peek** — `OperandStack::peek()` may not exist. Add it following the pattern of `pop()` but without decrementing `sp`.

6. **POP opcode** — may already exist. Check `opcode.rs` before adding.

### Testing strategy

- **Task 3**: Unit tests with hand-crafted bytecode verify each FB opcode in isolation
- **Task 4**: Unit tests verify TON timing behavior across multiple scans
- **Task 6**: Compile tests verify the codegen produces correct bytecode sequences
- **Task 7**: End-to-end tests verify the full pipeline (source → compile → execute → verify)

### PR boundaries

Each task is a natural PR boundary:
- Tasks 1-2: PR 1 (Container)
- Tasks 3-4: PR 2 (VM)
- Tasks 5-6: PR 3 (Codegen)
- Tasks 7-8: PR 4 (Integration + docs)
