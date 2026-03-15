# Function Local Re-initialization Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ensure IEC 61131-3 function locals are re-initialized to their declared initial values (or type defaults) on every call, per the standard's statelessness requirement.

**Architecture:** The compiler pre-computes initial Slot values for each function's non-parameter locals and stores them in a new `init_template` container section. On each `CALL`, the VM copies the template into the variable table before executing the function body.

**Tech Stack:** Rust, IronPLC compiler pipeline (codegen → container → VM)

**Design doc:** `specs/adrs/0024-function-local-reinit-via-init-template.md`

**Prerequisites:** Create a feature branch before starting. Never commit to `main` directly.

---

## File Map

### Container crate (`compiler/container/src/`)
- **Modify:** `header.rs` — Add `init_template_offset` and `init_template_size` fields (bytes 218-225), shrink reserved
- **Create:** `init_template.rs` — New `InitTemplateSection` type with serialize/deserialize
- **Modify:** `lib.rs` — Add module declaration and re-export
- **Modify:** `container.rs` — Add `init_template` field to `Container`, update write/read
- **Modify:** `builder.rs` — Add template storage and `set_function_init_template` method
- **Modify:** `container_ref.rs` — Add init template slicing and `get_init_template` accessor

### VM crate (`compiler/vm/src/`)
- **Modify:** `value.rs` — Add `Slot::from_u64()` constructor
- **Modify:** `variable_table.rs` — Add `copy_template()` method
- **Modify:** `vm.rs` — Update `CALL` opcode handler to copy template before executing function

### Codegen crate (`compiler/codegen/src/`)
- **Modify:** `compile.rs` — Add `init_template` field to `CompiledFunction`, add `compute_initial_slot_value` helper, generate template data in `compile_user_function`, pass templates to builder

### Tests
- **Modify:** `compiler/codegen/tests/end_to_end_user_function.rs` — Add end-to-end re-initialization tests

---

## Task 1: Add `Slot::from_u64` constructor

**Files:**
- Modify: `compiler/vm/src/value.rs`

- [ ] **Step 1: Add the constructor**

Add to the `impl Slot` block, before `as_u64`:

```rust
/// Creates a slot from a raw 64-bit value.
pub fn from_u64(v: u64) -> Self {
    Slot(v)
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cd compiler && cargo build -p ironplc-vm`

- [ ] **Step 3: Commit**

```bash
git add compiler/vm/src/value.rs
git commit -m "feat: add Slot::from_u64 constructor for init template support"
```

---

## Task 2: Add init template header fields to `FileHeader`

**Files:**
- Modify: `compiler/container/src/header.rs`

The 256-byte header currently has this layout:

```
Region 1: Identification     bytes   0 -   7  (8 bytes)
Region 2: Hashes             bytes   8 - 135  (128 bytes)
Region 3: Section directory  bytes 136 - 191  (56 bytes = 7 section offset/size pairs)
Region 4: Runtime parameters bytes 192 - 217  (26 bytes)
Reserved                     bytes 218 - 255  (38 bytes, all zeros)
```

We carve 8 bytes from the reserved region:

```
bytes 218-221: init_template_offset  (u32 LE)
bytes 222-225: init_template_size    (u32 LE)
bytes 226-255: reserved              (30 bytes, shrunk from 38)
```

- [ ] **Step 1: Update `FileHeader` struct**

Change the reserved field and add two new fields:

```rust
// Region 5: Init template section directory (bytes 218-225)
pub init_template_offset: u32,
pub init_template_size: u32,
// Reserved (bytes 226-255)
pub reserved: [u8; 30],
```

- [ ] **Step 2: Update `Default` impl**

Initialize both new fields to 0, change `reserved` to `[0; 30]`.

- [ ] **Step 3: Update `write_to`**

After writing `memory_image_bytes`, write:

```rust
// Region 5: Init template section directory (bytes 218-225)
w.write_all(&self.init_template_offset.to_le_bytes())?;
w.write_all(&self.init_template_size.to_le_bytes())?;
// Reserved (bytes 226-255)
w.write_all(&self.reserved)?;
```

- [ ] **Step 4: Update `from_bytes`**

After reading `memory_image_bytes`, read:

```rust
// Region 5: Init template section directory (bytes 218-225)
let init_template_offset = u32::from_le_bytes([buf[218], buf[219], buf[220], buf[221]]);
let init_template_size = u32::from_le_bytes([buf[222], buf[223], buf[224], buf[225]]);

// Reserved (bytes 226-255)
let mut reserved = [0u8; 30];
reserved.copy_from_slice(&buf[226..256]);
```

Add both fields to the returned `FileHeader`.

- [ ] **Step 5: Update tests**

Change the reserved assertion from `[0; 38]` to `[0; 30]`. Add assertions for the new fields.

- [ ] **Step 6: Run tests**

Run: `cd compiler && cargo test -p ironplc-container`

- [ ] **Step 7: Commit**

```bash
git add compiler/container/src/header.rs
git commit -m "feat: add init_template_offset and init_template_size to FileHeader (bytes 218-225)"
```

---

## Task 3: Create `InitTemplateSection`

**Files:**
- Create: `compiler/container/src/init_template.rs`

The section is pointed to by `header.init_template_offset` and has `header.init_template_size` total bytes.

```
┌─────────────────────────────────────────────────────────────┐
│ Directory (num_functions × 8 bytes)                         │
│   For function i (i = 0..num_functions):                    │
│     template_offset: u32 LE  — byte offset into data blob   │
│     template_size:   u32 LE  — byte count                   │
├─────────────────────────────────────────────────────────────┤
│ Data blob (variable size)                                   │
│   Concatenated Slot values (u64 LE) for each function's     │
│   non-parameter locals, in declaration order.               │
│   Functions with no non-param locals have template_size=0.  │
└─────────────────────────────────────────────────────────────┘

Total section size = (num_functions × 8) + sum(template_sizes)
```

- [ ] **Step 1: Create the module**

Create `compiler/container/src/init_template.rs` with:

```rust
pub struct InitTemplateEntry {
    pub template_offset: u32,
    pub template_size: u32,
}

pub struct InitTemplateSection {
    pub entries: Vec<InitTemplateEntry>,
    pub data: Vec<u8>,
}
```

Implement:
- `section_size() -> u32`
- `write_to(&self, w: &mut impl Write) -> Result<(), ContainerError>`
- `read_from(r: &mut impl Read, num_functions: u16, section_size: u32) -> Result<Self, ContainerError>`
- `get_template_data(&self, function_id: u16) -> Option<&[u8]>` — returns `None` when `template_size == 0`

- [ ] **Step 2: Add roundtrip test**

Test writing and reading a section with two functions (one with no template, one with template data).

- [ ] **Step 3: Run tests**

Run: `cd compiler && cargo test -p ironplc-container`

- [ ] **Step 4: Commit**

```bash
git add compiler/container/src/init_template.rs
git commit -m "feat: add InitTemplateSection for function local re-initialization data"
```

---

## Task 4: Wire into Container and ContainerBuilder

**Files:**
- Modify: `compiler/container/src/lib.rs`
- Modify: `compiler/container/src/container.rs`
- Modify: `compiler/container/src/builder.rs`

- [ ] **Step 1: Add module declaration and re-export in `lib.rs`**

Under the `#[cfg(feature = "std")]` block, add:

```rust
#[cfg(feature = "std")]
mod init_template;
```

Add re-export:

```rust
#[cfg(feature = "std")]
pub use init_template::{InitTemplateEntry, InitTemplateSection};
```

- [ ] **Step 2: Add `init_template` field to `Container`**

In `compiler/container/src/container.rs`:

```rust
pub struct Container {
    pub header: FileHeader,
    pub task_table: TaskTable,
    pub constant_pool: ConstantPool,
    pub code: CodeSection,
    pub debug_section: Option<DebugSection>,
    pub init_template: Option<InitTemplateSection>,
}
```

- [ ] **Step 3: Update `Container::write_to`**

After writing the debug section (or after the code section if no debug), write the init template section if present. Set `header.init_template_offset` and `header.init_template_size`.

The init template section should be written after the last existing section. Compute its offset as the end of the previous section:

```rust
let mut next_offset = code_section_offset + code_section_size;

if let Some(debug) = &self.debug_section {
    header.debug_section_offset = next_offset;
    let debug_size = debug.section_size();
    header.debug_section_size = debug_size;
    header.flags |= 0x02;
    next_offset += debug_size;
}

if let Some(init_tmpl) = &self.init_template {
    header.init_template_offset = next_offset;
    header.init_template_size = init_tmpl.section_size();
}
```

Then write in order: header, task table, constant pool, code, debug (if present), init template (if present).

- [ ] **Step 4: Update `Container::read_from`**

After parsing the debug section, add:

```rust
let init_template = if header.init_template_size > 0 {
    let start = (header.init_template_offset - base) as usize;
    let end = start + header.init_template_size as usize;
    if end <= rest.len() {
        InitTemplateSection::read_from(
            &mut Cursor::new(&rest[start..end]),
            header.num_functions,
            header.init_template_size,
        ).ok()
    } else {
        None
    }
} else {
    None
};
```

Add `init_template` to the returned `Container`.

- [ ] **Step 5: Update `ContainerBuilder`**

Add a field:

```rust
init_templates: Vec<Vec<u8>>,
```

Initialize to empty in `new()`. Add a method:

```rust
/// Sets the init template data for the given function.
///
/// The template contains pre-computed Slot values (u64 LE) for the
/// function's non-parameter locals, used to re-initialize them on each call.
pub fn set_function_init_template(mut self, function_id: u16, template: Vec<u8>) -> Self {
    // Grow the vector if needed to accommodate the function_id index.
    if self.init_templates.len() <= function_id as usize {
        self.init_templates.resize(function_id as usize + 1, Vec::new());
    }
    self.init_templates[function_id as usize] = template;
    self
}
```

In `build()`, after constructing the `CodeSection`, build the `InitTemplateSection` if any templates are non-empty:

```rust
let init_template = if self.init_templates.iter().any(|t| !t.is_empty()) {
    let num_functions = code.functions.len();
    // Pad init_templates to match num_functions.
    let mut templates = self.init_templates;
    templates.resize(num_functions, Vec::new());

    let mut entries = Vec::with_capacity(num_functions);
    let mut data = Vec::new();
    for tmpl in &templates {
        entries.push(InitTemplateEntry {
            template_offset: data.len() as u32,
            template_size: tmpl.len() as u32,
        });
        data.extend_from_slice(tmpl);
    }
    Some(InitTemplateSection { entries, data })
} else {
    None
};
```

Add `init_template` to the returned `Container`.

- [ ] **Step 6: Update existing `Container` construction sites**

Add `init_template: None` where `Container` is constructed directly (not through builder).

- [ ] **Step 7: Add roundtrip test**

Add a test in `container.rs` that builds a container with init template data and verifies it roundtrips.

- [ ] **Step 8: Run tests**

Run: `cd compiler && cargo test -p ironplc-container`

- [ ] **Step 9: Commit**

```bash
git add compiler/container/src/lib.rs compiler/container/src/container.rs compiler/container/src/builder.rs
git commit -m "feat: wire InitTemplateSection into Container and ContainerBuilder"
```

---

## Task 5: Update `ContainerRef` (no_std zero-copy path)

**Files:**
- Modify: `compiler/container/src/container_ref.rs`

- [ ] **Step 1: Add init template fields**

Add to `ContainerRef`:

```rust
init_template_dir: &'a [u8],
init_template_data: &'a [u8],
```

- [ ] **Step 2: Update `from_slice`**

After slicing the task table, add:

```rust
// 6. Slice out init template section (if present)
let (init_template_dir, init_template_data) = if header.init_template_size > 0 {
    let it_start = header.init_template_offset as usize;
    let it_end = it_start + header.init_template_size as usize;
    if it_end > data.len() {
        return Err(ContainerError::SectionSizeMismatch);
    }
    let it_section = &data[it_start..it_end];
    let dir_size = header.num_functions as usize * 8;
    if dir_size > it_section.len() {
        return Err(ContainerError::SectionSizeMismatch);
    }
    (&it_section[..dir_size], &it_section[dir_size..])
} else {
    (&data[0..0], &data[0..0])
};
```

Add the fields to the returned `ContainerRef`.

- [ ] **Step 3: Add accessor**

```rust
/// Returns the init template data for the given function ID, or `None`
/// if the function has no template (template_size == 0 or no section).
pub fn get_init_template(&self, function_id: u16) -> Option<&'a [u8]> {
    if self.init_template_dir.is_empty() {
        return None;
    }
    let entry_offset = function_id as usize * 8;
    if entry_offset + 8 > self.init_template_dir.len() {
        return None;
    }
    let entry = &self.init_template_dir[entry_offset..entry_offset + 8];
    let tmpl_offset = u32::from_le_bytes([entry[0], entry[1], entry[2], entry[3]]) as usize;
    let tmpl_size = u32::from_le_bytes([entry[4], entry[5], entry[6], entry[7]]) as usize;
    if tmpl_size == 0 {
        return None;
    }
    let end = tmpl_offset + tmpl_size;
    if end > self.init_template_data.len() {
        return None;
    }
    Some(&self.init_template_data[tmpl_offset..end])
}
```

- [ ] **Step 4: Run tests**

Run: `cd compiler && cargo test -p ironplc-container`

- [ ] **Step 5: Commit**

```bash
git add compiler/container/src/container_ref.rs
git commit -m "feat: add init template support to ContainerRef (no_std zero-copy path)"
```

---

## Task 6: Add `copy_template` to `VariableTable`

**Files:**
- Modify: `compiler/vm/src/variable_table.rs`

- [ ] **Step 1: Add method**

```rust
/// Copies pre-computed Slot values from a template byte slice into
/// consecutive variable slots starting at `start`.
///
/// The template is a sequence of u64 LE values (8 bytes per slot).
pub fn copy_template(&mut self, start: u16, template: &[u8]) -> Result<(), Trap> {
    let num_slots = template.len() / 8;
    for i in 0..num_slots {
        let offset = i * 8;
        let raw = u64::from_le_bytes(
            template[offset..offset + 8]
                .try_into()
                .map_err(|_| Trap::InvalidVariableIndex(start + i as u16))?,
        );
        let idx = start as usize + i;
        let slot = self
            .slots
            .get_mut(idx)
            .ok_or(Trap::InvalidVariableIndex(start + i as u16))?;
        *slot = Slot::from_u64(raw);
    }
    Ok(())
}
```

- [ ] **Step 2: Add tests**

```rust
#[test]
fn variable_table_copy_template_when_valid_then_sets_slots() {
    let mut buf = [Slot::default(); 5];
    let mut table = VariableTable::new(&mut buf);
    // Template: slot 0 = 42, slot 1 = -1 (sign-extended)
    let mut template = Vec::new();
    template.extend_from_slice(&42u64.to_le_bytes());
    template.extend_from_slice(&((-1i32 as i64 as u64)).to_le_bytes());
    table.copy_template(2, &template).unwrap();

    assert_eq!(table.load(2).unwrap(), Slot::from_u64(42));
    assert_eq!(table.load(3).unwrap().as_i32(), -1);
}

#[test]
fn variable_table_copy_template_when_out_of_bounds_then_error() {
    let mut buf = [Slot::default(); 2];
    let mut table = VariableTable::new(&mut buf);
    let template = 42u64.to_le_bytes();
    // start=2 is out of bounds for a 2-slot table
    assert!(table.copy_template(2, &template).is_err());
}
```

- [ ] **Step 3: Run tests**

Run: `cd compiler && cargo test -p ironplc-vm`

- [ ] **Step 4: Commit**

```bash
git add compiler/vm/src/variable_table.rs
git commit -m "feat: add copy_template method to VariableTable for bulk local re-initialization"
```

---

## Task 7: Update VM CALL handler

**Files:**
- Modify: `compiler/vm/src/vm.rs`

- [ ] **Step 1: Add init template re-initialization**

In the `CALL` handler (around line 755), after popping parameters and before the recursive `execute()` call, add:

```rust
// Re-initialize non-parameter locals from the init template.
if let Some(ref init_tmpl) = container.init_template {
    if let Some(template) = init_tmpl.get_template_data(func_id) {
        let local_start = var_offset + func.num_params;
        variables.copy_template(local_start, template)?;
    }
}
```

- [ ] **Step 2: Run existing tests**

Run: `cd compiler && cargo test -p ironplc-vm`
All existing tests should still pass (containers without init template skip the copy).

- [ ] **Step 3: Commit**

```bash
git add compiler/vm/src/vm.rs
git commit -m "feat: re-initialize function locals from init template on each CALL"
```

---

## Task 8: Generate init template data in codegen

**Files:**
- Modify: `compiler/codegen/src/compile.rs`

- [ ] **Step 1: Add `init_template` field to `CompiledFunction`**

```rust
struct CompiledFunction {
    function_id: u16,
    bytecode: Vec<u8>,
    max_stack_depth: u16,
    num_locals: u16,
    num_params: u16,
    name: String,
    init_template: Vec<u8>,  // NEW
}
```

- [ ] **Step 2: Add `compute_initial_slot_value` helper**

Add a function that evaluates a `ConstantKind` to a raw u64 Slot value, mirroring `compile_constant` + `emit_truncation`:

```rust
/// Evaluates a constant to its raw Slot u64 representation.
///
/// This must produce the same bit pattern that `compile_constant` +
/// `emit_truncation` would produce at runtime. Used for building
/// init template data.
fn compute_initial_slot_value(
    constant: &ConstantKind,
    type_info: Option<VarTypeInfo>,
) -> Result<u64, Diagnostic> {
    let op_type = type_info
        .map(|ti| (ti.op_width, ti.signedness))
        .unwrap_or(DEFAULT_OP_TYPE);

    let raw = match constant {
        ConstantKind::IntegerLiteral(lit) => {
            match op_type {
                (OpWidth::W32, Signedness::Signed) => {
                    let value = if lit.value.is_neg {
                        -(lit.value.value.value as i128)
                    } else {
                        lit.value.value.value as i128
                    };
                    let v = i32::try_from(value).map_err(|_| {
                        Diagnostic::problem(
                            Problem::ConstantOverflow,
                            Label::span(lit.value.value.span(), "Integer literal"),
                        )
                    })?;
                    Slot::from_i32(v).as_u64()
                }
                (OpWidth::W32, Signedness::Unsigned) => {
                    let v = u32::try_from(lit.value.value.value).map_err(|_| {
                        Diagnostic::problem(
                            Problem::ConstantOverflow,
                            Label::span(lit.value.value.span(), "Integer literal"),
                        )
                    })?;
                    Slot::from_i32(v as i32).as_u64()
                }
                (OpWidth::W64, Signedness::Signed) => {
                    let value = if lit.value.is_neg {
                        -(lit.value.value.value as i128)
                    } else {
                        lit.value.value.value as i128
                    };
                    let v = i64::try_from(value).map_err(|_| {
                        Diagnostic::problem(
                            Problem::ConstantOverflow,
                            Label::span(lit.value.value.span(), "Integer literal"),
                        )
                    })?;
                    Slot::from_i64(v).as_u64()
                }
                (OpWidth::W64, Signedness::Unsigned) => {
                    let v = u64::try_from(lit.value.value.value).map_err(|_| {
                        Diagnostic::problem(
                            Problem::ConstantOverflow,
                            Label::span(lit.value.value.span(), "Integer literal"),
                        )
                    })?;
                    Slot::from_i64(v as i64).as_u64()
                }
                _ => 0u64,
            }
        }
        ConstantKind::RealLiteral(lit) => match op_type.0 {
            OpWidth::F32 => Slot::from_f32(lit.value as f32).as_u64(),
            OpWidth::F64 => Slot::from_f64(lit.value).as_u64(),
            _ => 0u64,
        },
        ConstantKind::Boolean(lit) => {
            match lit.value {
                Boolean::True => Slot::from_i32(1).as_u64(),
                Boolean::False => 0u64,
            }
        }
        _ => 0u64,
    };

    // Apply truncation for narrow types (same masks as emit_truncation).
    let truncated = if let Some(ti) = type_info {
        apply_truncation_mask(raw, ti)
    } else {
        raw
    };

    Ok(truncated)
}

/// Applies the same truncation mask that `emit_truncation` would apply
/// at runtime, producing a properly-narrowed Slot value.
fn apply_truncation_mask(raw: u64, type_info: VarTypeInfo) -> u64 {
    match (type_info.op_width, type_info.signedness, type_info.storage_bits) {
        (OpWidth::W32, Signedness::Signed, 8) => {
            Slot::from_i32(raw as i32 as i8 as i32).as_u64()
        }
        (OpWidth::W32, Signedness::Signed, 16) => {
            Slot::from_i32(raw as i32 as i16 as i32).as_u64()
        }
        (OpWidth::W32, Signedness::Unsigned, 8) => {
            Slot::from_i32((raw as u32 & 0xFF) as i32).as_u64()
        }
        (OpWidth::W32, Signedness::Unsigned, 16) => {
            Slot::from_i32((raw as u32 & 0xFFFF) as i32).as_u64()
        }
        _ => raw,
    }
}
```

Note: These use `Slot::from_i32`, `Slot::from_i64`, `Slot::from_f32`, `Slot::from_f64`, and the new `Slot::from_u64` from the VM crate. You need to add `ironplc_vm` as a dependency of `ironplc-codegen`, or alternatively, replicate the Slot value encoding inline (since the encoding is just arithmetic: `v as i64 as u64`, `v.to_bits()`, etc.). The inline approach avoids a crate dependency.

**Recommended:** Use inline arithmetic rather than importing `Slot`:

```rust
fn slot_from_i32(v: i32) -> u64 { v as i64 as u64 }
fn slot_from_i64(v: i64) -> u64 { v as u64 }
fn slot_from_f32(v: f32) -> u64 { v.to_bits() as u64 }
fn slot_from_f64(v: f64) -> u64 { v.to_bits() }
```

- [ ] **Step 3: Build template in `compile_user_function`**

After the second pass (local variables) and before compiling the function body, build the template:

```rust
// Build init template for non-parameter locals (VAR + return var).
let mut init_template = Vec::new();

// Local variables (VAR)
for decl in &func_decl.variables {
    if decl.var_type != VariableType::Var {
        continue;
    }
    if let Some(id) = decl.identifier.symbolic_id() {
        let type_info = ctx.var_type_info(id);
        let slot_value = if let InitialValueAssignmentKind::Simple(simple) = &decl.initializer {
            if let Some(constant) = &simple.initial_value {
                compute_initial_slot_value(constant, type_info.cloned())?
            } else {
                0u64
            }
        } else {
            0u64
        };
        init_template.extend_from_slice(&slot_value.to_le_bytes());
    }
}

// Return variable (always zero-initialized)
init_template.extend_from_slice(&0u64.to_le_bytes());
```

Add `init_template` to the returned `CompiledFunction`.

- [ ] **Step 4: Pass templates to builder in `compile_program_with_functions`**

After adding compiled functions to the builder, add:

```rust
for compiled in &compiled_functions {
    builder = builder.add_function(
        compiled.function_id,
        &compiled.bytecode,
        compiled.max_stack_depth,
        compiled.num_locals,
        compiled.num_params,
    );
    if !compiled.init_template.is_empty() {
        builder = builder.set_function_init_template(
            compiled.function_id,
            compiled.init_template.clone(),
        );
    }
}
```

- [ ] **Step 5: Run tests**

Run: `cd compiler && cargo test -p ironplc-codegen`

- [ ] **Step 6: Commit**

```bash
git add compiler/codegen/src/compile.rs
git commit -m "feat: generate init template data for function locals in codegen"
```

---

## Task 9: Add end-to-end tests

**Files:**
- Modify: `compiler/codegen/tests/end_to_end_user_function.rs`

- [ ] **Step 1: Test function with initial value called twice**

```rust
#[test]
fn end_to_end_when_function_with_initial_value_called_twice_then_both_correct() {
    let source = "
FUNCTION add_with_base : DINT
  VAR_INPUT a : DINT; END_VAR
  VAR counter : DINT := 10; END_VAR
  counter := counter + a;
  add_with_base := counter;
END_FUNCTION

PROGRAM main
  VAR
    r1 : DINT;
    r2 : DINT;
  END_VAR
  r1 := add_with_base(5);
  r2 := add_with_base(3);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    // counter re-initialized to 10 on each call
    assert_eq!(bufs.vars[0].as_i32(), 15);  // 10 + 5
    assert_eq!(bufs.vars[1].as_i32(), 13);  // 10 + 3, NOT 18
}
```

- [ ] **Step 2: Test function with no initial value called twice**

```rust
#[test]
fn end_to_end_when_function_local_no_initial_value_called_twice_then_zero_each_time() {
    let source = "
FUNCTION accumulate : DINT
  VAR_INPUT a : DINT; END_VAR
  VAR total : DINT; END_VAR
  total := total + a;
  accumulate := total;
END_FUNCTION

PROGRAM main
  VAR
    r1 : DINT;
    r2 : DINT;
  END_VAR
  r1 := accumulate(5);
  r2 := accumulate(3);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    // total re-initialized to 0 on each call
    assert_eq!(bufs.vars[0].as_i32(), 5);  // 0 + 5
    assert_eq!(bufs.vars[1].as_i32(), 3);  // 0 + 3, NOT 8
}
```

- [ ] **Step 3: Run all tests**

Run: `cd compiler && cargo test`

- [ ] **Step 4: Commit**

```bash
git add compiler/codegen/tests/end_to_end_user_function.rs
git commit -m "test: add end-to-end tests for function local re-initialization"
```

---

## Task 10: Run full CI pipeline

- [ ] **Step 1: Run full CI**

Run: `cd compiler && just`
Expected: Compile, coverage (85%+), and lint all pass.

- [ ] **Step 2: Fix any issues**

If clippy, format, or coverage issues arise, fix them.

- [ ] **Step 3: Final commit if needed**

```bash
git add -A
git commit -m "chore: fix lint and formatting issues"
```
