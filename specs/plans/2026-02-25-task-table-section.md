# Task Table Section Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add the task table binary section to the container format, including serialization, deserialization, builder integration, disassembly, and VS Code cleanup.

**Architecture:** New `task_table` module following the same pattern as `constant_pool` and `code_section` — structs with `write_to`/`read_from`. The Container struct gains a `task_table` field written between the type section and constant pool. The ContainerBuilder auto-synthesizes a default task table (1 freewheeling task, 1 program instance pointing to function 0) when no explicit task config is provided. No VM changes (Phase 2).

**Tech Stack:** Rust, ironplc-container crate, ironplcc crate, TypeScript (VS Code extension)

---

### Task 1: Create task_table module with data structures and serialization

**Files:**
- Create: `compiler/container/src/task_table.rs`
- Modify: `compiler/container/src/lib.rs`
- Modify: `compiler/container/src/error.rs`

**Step 1: Add InvalidTaskType error variant**

In `compiler/container/src/error.rs`, add a new variant to `ContainerError`:

```rust
/// A task entry has an unrecognized task type tag.
InvalidTaskType(u8),
```

Add the Display match arm:
```rust
ContainerError::InvalidTaskType(t) => {
    write!(f, "invalid task type tag: {t}")
}
```

**Step 2: Create task_table.rs with types and serialization**

Create `compiler/container/src/task_table.rs` with the complete module. The format matches the spec in `specs/design/61131-task-support.md` § Task Table Format.

```rust
use std::io::{Read, Write};

use crate::ContainerError;

/// Size of a single task entry in bytes.
const TASK_ENTRY_SIZE: usize = 32;

/// Size of a single program instance entry in bytes.
const PROGRAM_INSTANCE_ENTRY_SIZE: usize = 16;

/// Size of the task table header in bytes (num_tasks + num_program_instances + shared_globals_size).
const TASK_TABLE_HEADER_SIZE: usize = 6;

/// Task type tags.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum TaskType {
    Cyclic = 0,
    Event = 1,
    Freewheeling = 2,
}

impl TaskType {
    fn from_u8(v: u8) -> Result<Self, ContainerError> {
        match v {
            0 => Ok(TaskType::Cyclic),
            1 => Ok(TaskType::Event),
            2 => Ok(TaskType::Freewheeling),
            _ => Err(ContainerError::InvalidTaskType(v)),
        }
    }

    /// Returns the human-readable name for this task type.
    pub fn as_str(&self) -> &'static str {
        match self {
            TaskType::Cyclic => "Cyclic",
            TaskType::Event => "Event",
            TaskType::Freewheeling => "Freewheeling",
        }
    }
}

/// A task entry in the task table (32 bytes, fixed size).
#[derive(Clone, Debug)]
pub struct TaskEntry {
    pub task_id: u16,
    pub priority: u16,
    pub task_type: TaskType,
    pub flags: u8,
    pub interval_us: u64,
    pub single_var_index: u16,
    pub watchdog_us: u64,
    pub input_image_offset: u16,
    pub output_image_offset: u16,
    pub reserved: [u8; 4],
}

/// A program instance entry in the task table (16 bytes, fixed size).
#[derive(Clone, Debug)]
pub struct ProgramInstanceEntry {
    pub instance_id: u16,
    pub task_id: u16,
    pub entry_function_id: u16,
    pub var_table_offset: u16,
    pub var_table_count: u16,
    pub fb_instance_offset: u16,
    pub fb_instance_count: u16,
    pub reserved: u16,
}

/// The task table section of a bytecode container.
#[derive(Clone, Debug, Default)]
pub struct TaskTable {
    pub shared_globals_size: u16,
    pub tasks: Vec<TaskEntry>,
    pub programs: Vec<ProgramInstanceEntry>,
}

impl TaskTable {
    /// Returns the serialized size of this task table section in bytes.
    pub fn section_size(&self) -> u32 {
        (TASK_TABLE_HEADER_SIZE
            + self.tasks.len() * TASK_ENTRY_SIZE
            + self.programs.len() * PROGRAM_INSTANCE_ENTRY_SIZE) as u32
    }

    /// Writes the task table to the given writer.
    pub fn write_to(&self, w: &mut impl Write) -> Result<(), ContainerError> {
        // Header
        w.write_all(&(self.tasks.len() as u16).to_le_bytes())?;
        w.write_all(&(self.programs.len() as u16).to_le_bytes())?;
        w.write_all(&self.shared_globals_size.to_le_bytes())?;

        // Task entries
        for task in &self.tasks {
            w.write_all(&task.task_id.to_le_bytes())?;
            w.write_all(&task.priority.to_le_bytes())?;
            w.write_all(&[task.task_type as u8])?;
            w.write_all(&[task.flags])?;
            w.write_all(&task.interval_us.to_le_bytes())?;
            w.write_all(&task.single_var_index.to_le_bytes())?;
            w.write_all(&task.watchdog_us.to_le_bytes())?;
            w.write_all(&task.input_image_offset.to_le_bytes())?;
            w.write_all(&task.output_image_offset.to_le_bytes())?;
            w.write_all(&task.reserved)?;
        }

        // Program instance entries
        for prog in &self.programs {
            w.write_all(&prog.instance_id.to_le_bytes())?;
            w.write_all(&prog.task_id.to_le_bytes())?;
            w.write_all(&prog.entry_function_id.to_le_bytes())?;
            w.write_all(&prog.var_table_offset.to_le_bytes())?;
            w.write_all(&prog.var_table_count.to_le_bytes())?;
            w.write_all(&prog.fb_instance_offset.to_le_bytes())?;
            w.write_all(&prog.fb_instance_count.to_le_bytes())?;
            w.write_all(&prog.reserved.to_le_bytes())?;
        }

        Ok(())
    }

    /// Reads a task table from the given reader.
    pub fn read_from(r: &mut impl Read) -> Result<Self, ContainerError> {
        let mut hdr = [0u8; TASK_TABLE_HEADER_SIZE];
        r.read_exact(&mut hdr)?;

        let num_tasks = u16::from_le_bytes([hdr[0], hdr[1]]) as usize;
        let num_programs = u16::from_le_bytes([hdr[2], hdr[3]]) as usize;
        let shared_globals_size = u16::from_le_bytes([hdr[4], hdr[5]]);

        let mut tasks = Vec::with_capacity(num_tasks);
        for _ in 0..num_tasks {
            let mut buf = [0u8; TASK_ENTRY_SIZE];
            r.read_exact(&mut buf)?;

            let mut reserved = [0u8; 4];
            reserved.copy_from_slice(&buf[28..32]);

            tasks.push(TaskEntry {
                task_id: u16::from_le_bytes([buf[0], buf[1]]),
                priority: u16::from_le_bytes([buf[2], buf[3]]),
                task_type: TaskType::from_u8(buf[4])?,
                flags: buf[5],
                interval_us: u64::from_le_bytes([
                    buf[6], buf[7], buf[8], buf[9], buf[10], buf[11], buf[12], buf[13],
                ]),
                single_var_index: u16::from_le_bytes([buf[14], buf[15]]),
                watchdog_us: u64::from_le_bytes([
                    buf[16], buf[17], buf[18], buf[19], buf[20], buf[21], buf[22], buf[23],
                ]),
                input_image_offset: u16::from_le_bytes([buf[24], buf[25]]),
                output_image_offset: u16::from_le_bytes([buf[26], buf[27]]),
                reserved,
            });
        }

        let mut programs = Vec::with_capacity(num_programs);
        for _ in 0..num_programs {
            let mut buf = [0u8; PROGRAM_INSTANCE_ENTRY_SIZE];
            r.read_exact(&mut buf)?;

            programs.push(ProgramInstanceEntry {
                instance_id: u16::from_le_bytes([buf[0], buf[1]]),
                task_id: u16::from_le_bytes([buf[2], buf[3]]),
                entry_function_id: u16::from_le_bytes([buf[4], buf[5]]),
                var_table_offset: u16::from_le_bytes([buf[6], buf[7]]),
                var_table_count: u16::from_le_bytes([buf[8], buf[9]]),
                fb_instance_offset: u16::from_le_bytes([buf[10], buf[11]]),
                fb_instance_count: u16::from_le_bytes([buf[12], buf[13]]),
                reserved: u16::from_le_bytes([buf[14], buf[15]]),
            });
        }

        Ok(TaskTable {
            shared_globals_size,
            tasks,
            programs,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn default_task_entry() -> TaskEntry {
        TaskEntry {
            task_id: 0,
            priority: 0,
            task_type: TaskType::Freewheeling,
            flags: 0x01,
            interval_us: 0,
            single_var_index: 0xFFFF,
            watchdog_us: 0,
            input_image_offset: 0,
            output_image_offset: 0,
            reserved: [0; 4],
        }
    }

    fn default_program_instance_entry() -> ProgramInstanceEntry {
        ProgramInstanceEntry {
            instance_id: 0,
            task_id: 0,
            entry_function_id: 0,
            var_table_offset: 0,
            var_table_count: 2,
            fb_instance_offset: 0,
            fb_instance_count: 0,
            reserved: 0,
        }
    }

    #[test]
    fn task_table_section_size_when_empty_then_header_only() {
        let table = TaskTable::default();
        assert_eq!(table.section_size(), 6);
    }

    #[test]
    fn task_table_section_size_when_one_task_one_program_then_correct() {
        let table = TaskTable {
            shared_globals_size: 0,
            tasks: vec![default_task_entry()],
            programs: vec![default_program_instance_entry()],
        };
        // 6 (header) + 32 (task) + 16 (program) = 54
        assert_eq!(table.section_size(), 54);
    }

    #[test]
    fn task_table_write_read_when_one_task_one_program_then_roundtrips() {
        let original = TaskTable {
            shared_globals_size: 4,
            tasks: vec![TaskEntry {
                task_id: 1,
                priority: 5,
                task_type: TaskType::Cyclic,
                flags: 0x01,
                interval_us: 10_000,
                single_var_index: 0xFFFF,
                watchdog_us: 100_000,
                input_image_offset: 0,
                output_image_offset: 0,
                reserved: [0; 4],
            }],
            programs: vec![ProgramInstanceEntry {
                instance_id: 0,
                task_id: 1,
                entry_function_id: 0,
                var_table_offset: 4,
                var_table_count: 10,
                fb_instance_offset: 0,
                fb_instance_count: 0,
                reserved: 0,
            }],
        };

        let mut buf = Vec::new();
        original.write_to(&mut buf).unwrap();

        assert_eq!(buf.len(), original.section_size() as usize);

        let decoded = TaskTable::read_from(&mut Cursor::new(&buf)).unwrap();

        assert_eq!(decoded.shared_globals_size, 4);
        assert_eq!(decoded.tasks.len(), 1);
        assert_eq!(decoded.tasks[0].task_id, 1);
        assert_eq!(decoded.tasks[0].priority, 5);
        assert_eq!(decoded.tasks[0].task_type, TaskType::Cyclic);
        assert_eq!(decoded.tasks[0].flags, 0x01);
        assert_eq!(decoded.tasks[0].interval_us, 10_000);
        assert_eq!(decoded.tasks[0].single_var_index, 0xFFFF);
        assert_eq!(decoded.tasks[0].watchdog_us, 100_000);
        assert_eq!(decoded.programs.len(), 1);
        assert_eq!(decoded.programs[0].instance_id, 0);
        assert_eq!(decoded.programs[0].task_id, 1);
        assert_eq!(decoded.programs[0].entry_function_id, 0);
        assert_eq!(decoded.programs[0].var_table_offset, 4);
        assert_eq!(decoded.programs[0].var_table_count, 10);
    }

    #[test]
    fn task_table_write_read_when_empty_then_roundtrips() {
        let original = TaskTable::default();

        let mut buf = Vec::new();
        original.write_to(&mut buf).unwrap();

        let decoded = TaskTable::read_from(&mut Cursor::new(&buf)).unwrap();

        assert_eq!(decoded.shared_globals_size, 0);
        assert_eq!(decoded.tasks.len(), 0);
        assert_eq!(decoded.programs.len(), 0);
    }

    #[test]
    fn task_table_read_when_invalid_task_type_then_error() {
        let mut buf = vec![0u8; 6 + 32]; // header + one task entry
        // num_tasks = 1
        buf[0] = 1;
        // Set task_type byte (offset 6 + 4 = 10) to invalid value
        buf[10] = 0xFF;

        let result = TaskTable::read_from(&mut Cursor::new(&buf));
        assert!(matches!(result, Err(ContainerError::InvalidTaskType(0xFF))));
    }

    #[test]
    fn task_type_as_str_when_cyclic_then_returns_cyclic() {
        assert_eq!(TaskType::Cyclic.as_str(), "Cyclic");
    }

    #[test]
    fn task_type_as_str_when_freewheeling_then_returns_freewheeling() {
        assert_eq!(TaskType::Freewheeling.as_str(), "Freewheeling");
    }
}
```

**Step 3: Register the module in lib.rs**

In `compiler/container/src/lib.rs`, add the module declaration and public exports:

```rust
mod task_table;
```

And add to the public exports:

```rust
pub use task_table::{ProgramInstanceEntry, TaskEntry, TaskTable, TaskType};
```

**Step 4: Run tests**

Run: `cd compiler && cargo test --package ironplc-container`
Expected: All tests pass (existing + 7 new task_table tests)

---

### Task 2: Integrate TaskTable into Container

**Files:**
- Modify: `compiler/container/src/container.rs`

**Step 1: Add task_table field and update write_to**

Add `task_table: TaskTable` to the `Container` struct. Update `write_to` to write the task table between the header and constant pool, computing its offset. The section order is: header → task table → constant pool → code section.

```rust
use crate::task_table::TaskTable;
```

Add to struct:
```rust
pub struct Container {
    pub header: FileHeader,
    pub task_table: TaskTable,
    pub constant_pool: ConstantPool,
    pub code: CodeSection,
}
```

Update `write_to`:
```rust
pub fn write_to(&self, w: &mut impl Write) -> Result<(), ContainerError> {
    let task_section_offset = HEADER_SIZE as u32;
    let task_section_size = self.task_table.section_size();
    let const_section_offset = task_section_offset + task_section_size;
    let const_section_size = self.constant_pool.section_size();
    let code_section_offset = const_section_offset + const_section_size;
    let code_section_size = self.code.section_size();

    let mut header = self.header.clone();
    header.task_section_offset = task_section_offset;
    header.task_section_size = task_section_size;
    header.const_section_offset = const_section_offset;
    header.const_section_size = const_section_size;
    header.code_section_offset = code_section_offset;
    header.code_section_size = code_section_size;
    header.num_functions = self.code.functions.len() as u16;

    header.write_to(w)?;
    self.task_table.write_to(w)?;
    self.constant_pool.write_to(w)?;
    self.code.write_to(w)?;
    Ok(())
}
```

Update `read_from` to parse the task table section:
```rust
pub fn read_from(r: &mut impl Read) -> Result<Self, ContainerError> {
    let header = FileHeader::read_from(r)?;

    let mut rest = Vec::new();
    r.read_to_end(&mut rest)?;

    let base = HEADER_SIZE as u32;

    let task_start = (header.task_section_offset - base) as usize;
    let task_end = task_start + header.task_section_size as usize;
    let task_table =
        TaskTable::read_from(&mut Cursor::new(&rest[task_start..task_end]))?;

    let const_start = (header.const_section_offset - base) as usize;
    let const_end = const_start + header.const_section_size as usize;
    let constant_pool =
        ConstantPool::read_from(&mut Cursor::new(&rest[const_start..const_end]))?;

    let code_start = (header.code_section_offset - base) as usize;
    let code_end = code_start + header.code_section_size as usize;
    let code = CodeSection::read_from(
        &mut Cursor::new(&rest[code_start..code_end]),
        header.num_functions,
        header.code_section_size,
    )?;

    Ok(Container {
        header,
        task_table,
        constant_pool,
        code,
    })
}
```

**Step 2: Update the container roundtrip test**

The existing test in `container.rs` creates a container via ContainerBuilder. After Task 3 updates the builder, this test will automatically include the task table. For now, add `task_table` to the manual Container construction:

In the test `container_write_read_when_steel_thread_program_then_roundtrips`, add assertions after the existing ones:

```rust
assert_eq!(decoded.task_table.tasks.len(), 1);
assert_eq!(decoded.task_table.programs.len(), 1);
assert_eq!(decoded.task_table.programs[0].entry_function_id, 0);
```

**Step 3: Run tests (expect compile errors until Task 3)**

The builder doesn't create a task_table yet, so the container tests will fail to compile. That's expected — Task 3 fixes this.

---

### Task 3: Update ContainerBuilder with default task table synthesis

**Files:**
- Modify: `compiler/container/src/builder.rs`

**Step 1: Update the builder to include task_table in Container**

Import the task table types. Add task/program storage to the builder. In `build()`, if no tasks were added, synthesize a default: 1 freewheeling task (id=0, priority=0, type=Freewheeling, flags=0x01 enabled) + 1 program instance (id=0, task_id=0, entry_function_id=0, var_table_offset=0, var_table_count=num_variables).

```rust
use crate::task_table::{ProgramInstanceEntry, TaskEntry, TaskTable, TaskType};
```

Add fields to `ContainerBuilder`:
```rust
pub struct ContainerBuilder {
    num_variables: u16,
    max_stack_depth: u16,
    constant_pool: ConstantPool,
    functions: Vec<FuncEntry>,
    bytecode: Vec<u8>,
    tasks: Vec<TaskEntry>,
    programs: Vec<ProgramInstanceEntry>,
    shared_globals_size: u16,
}
```

Update `new()`:
```rust
pub fn new() -> Self {
    ContainerBuilder {
        num_variables: 0,
        max_stack_depth: 0,
        constant_pool: ConstantPool::default(),
        functions: Vec::new(),
        bytecode: Vec::new(),
        tasks: Vec::new(),
        programs: Vec::new(),
        shared_globals_size: 0,
    }
}
```

Add builder methods:
```rust
/// Adds a task entry to the task table.
pub fn add_task(mut self, task: TaskEntry) -> Self {
    self.tasks.push(task);
    self
}

/// Adds a program instance entry to the task table.
pub fn add_program_instance(mut self, program: ProgramInstanceEntry) -> Self {
    self.programs.push(program);
    self
}

/// Sets the number of shared global variable slots.
pub fn shared_globals_size(mut self, n: u16) -> Self {
    self.shared_globals_size = n;
    self
}
```

Update `build()` to synthesize default task table when none provided:
```rust
pub fn build(self) -> Container {
    let constant_pool = self.constant_pool;
    let code = CodeSection {
        functions: self.functions,
        bytecode: self.bytecode,
    };

    let task_table = if self.tasks.is_empty() {
        // Synthesize default: 1 freewheeling task + 1 program instance
        TaskTable {
            shared_globals_size: 0,
            tasks: vec![TaskEntry {
                task_id: 0,
                priority: 0,
                task_type: TaskType::Freewheeling,
                flags: 0x01, // enabled at start
                interval_us: 0,
                single_var_index: 0xFFFF,
                watchdog_us: 0,
                input_image_offset: 0,
                output_image_offset: 0,
                reserved: [0; 4],
            }],
            programs: vec![ProgramInstanceEntry {
                instance_id: 0,
                task_id: 0,
                entry_function_id: 0,
                var_table_offset: 0,
                var_table_count: self.num_variables,
                fb_instance_offset: 0,
                fb_instance_count: 0,
                reserved: 0,
            }],
        }
    } else {
        TaskTable {
            shared_globals_size: self.shared_globals_size,
            tasks: self.tasks,
            programs: self.programs,
        }
    };

    let header = FileHeader {
        num_variables: self.num_variables,
        max_stack_depth: self.max_stack_depth,
        num_functions: code.functions.len() as u16,
        ..FileHeader::default()
    };

    Container {
        header,
        task_table,
        constant_pool,
        code,
    }
}
```

**Step 2: Update builder test assertions**

In `builder_when_steel_thread_program_then_builds_valid_container`, add:
```rust
assert_eq!(container.task_table.tasks.len(), 1);
assert_eq!(container.task_table.tasks[0].task_type, TaskType::Freewheeling);
assert_eq!(container.task_table.tasks[0].flags, 0x01);
assert_eq!(container.task_table.programs.len(), 1);
assert_eq!(container.task_table.programs[0].entry_function_id, 0);
assert_eq!(container.task_table.programs[0].var_table_count, 2);
```

**Step 3: Run all container tests**

Run: `cd compiler && cargo test --package ironplc-container`
Expected: All tests pass (header, constant_pool, code_section, task_table, container, builder)

---

### Task 4: Update disassembler

**Files:**
- Modify: `compiler/plc2x/src/disassemble.rs`

**Step 1: Add task table disassembly**

Import the task table type:
```rust
use ironplc_container::{ConstType, Container, TaskType};
```

Add a `disassemble_task_table` function:
```rust
/// Converts the task table into a JSON object.
fn disassemble_task_table(container: &Container) -> Value {
    let tt = &container.task_table;

    let tasks: Vec<Value> = tt
        .tasks
        .iter()
        .map(|t| {
            json!({
                "taskId": t.task_id,
                "priority": t.priority,
                "taskType": t.task_type.as_str(),
                "enabled": (t.flags & 0x01) != 0,
                "intervalUs": t.interval_us,
                "singleVarIndex": t.single_var_index,
                "watchdogUs": t.watchdog_us,
            })
        })
        .collect();

    let programs: Vec<Value> = tt
        .programs
        .iter()
        .map(|p| {
            json!({
                "instanceId": p.instance_id,
                "taskId": p.task_id,
                "entryFunctionId": p.entry_function_id,
                "varTableOffset": p.var_table_offset,
                "varTableCount": p.var_table_count,
                "fbInstanceOffset": p.fb_instance_offset,
                "fbInstanceCount": p.fb_instance_count,
            })
        })
        .collect();

    json!({
        "sharedGlobalsSize": tt.shared_globals_size,
        "tasks": tasks,
        "programs": programs,
    })
}
```

Update the `disassemble()` function to include task table:
```rust
pub fn disassemble(container: &Container) -> Value {
    let header = disassemble_header(container);
    let task_table = disassemble_task_table(container);
    let constants = disassemble_constants(container);
    let functions = disassemble_functions(container);

    json!({
        "header": header,
        "taskTable": task_table,
        "constants": constants,
        "functions": functions,
    })
}
```

**Step 2: Add disassembler tests**

Add tests for the task table output:
```rust
#[test]
fn disassemble_when_steel_thread_then_has_task_table() {
    let container = steel_thread_container();
    let result = disassemble(&container);
    assert!(result["taskTable"].is_object());
}

#[test]
fn disassemble_when_steel_thread_then_task_table_has_one_task() {
    let container = steel_thread_container();
    let result = disassemble(&container);
    let tasks = result["taskTable"]["tasks"].as_array().unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0]["taskType"], "Freewheeling");
    assert_eq!(tasks[0]["enabled"], true);
}

#[test]
fn disassemble_when_steel_thread_then_task_table_has_one_program() {
    let container = steel_thread_container();
    let result = disassemble(&container);
    let programs = result["taskTable"]["programs"].as_array().unwrap();
    assert_eq!(programs.len(), 1);
    assert_eq!(programs[0]["entryFunctionId"], 0);
    assert_eq!(programs[0]["varTableCount"], 2);
}
```

**Step 3: Update the existing task section test**

The `disassemble_when_steel_thread_then_header_has_task_section` test should now show non-zero offsets since the task table is present. Update it:

```rust
#[test]
fn disassemble_when_steel_thread_then_header_has_task_section() {
    let container = steel_thread_container();
    let result = disassemble(&container);
    // Task section starts immediately after the 256-byte header
    assert_eq!(result["header"]["taskSection"]["offset"], 256);
    assert!(result["header"]["taskSection"]["size"].as_u64().unwrap() > 0);
}
```

**Step 4: Run disassembler tests**

Run: `cd compiler && cargo test --package ironplcc -- disassemble`
Expected: All disassembler tests pass

---

### Task 5: Clean up VS Code extension

**Files:**
- Modify: `integrations/vscode/src/iplcRendering.ts`
- Modify: `integrations/vscode/src/test/unit/testHelpers.ts`

**Step 1: Remove entryFunctionId from DisassemblyHeader**

In `integrations/vscode/src/iplcRendering.ts`, remove the `entryFunctionId` field from the `DisassemblyHeader` interface (line 20).

**Step 2: Remove Entry Function ID from renderHeader**

In the `renderHeader` function, remove the line:
```html
<tr><td>Entry Function ID</td><td>${header.entryFunctionId}</td></tr>
```

**Step 3: Update test helpers**

In `integrations/vscode/src/test/unit/testHelpers.ts`, remove `entryFunctionId: 0,` from `createTestHeader()`.

**Step 4: Run VS Code extension tests**

Run: `cd integrations/vscode && just ci`
Expected: All tests pass

---

### Task 6: Regenerate golden test file, run full CI, and commit

**Step 1: Regenerate the golden test file**

The steel_thread.iplc binary will have different bytes because the task table section is now included between the header and constant pool.

Run: `cd compiler && cargo test -p ironplc-vm --test cli generate_golden -- --ignored --nocapture`

**Step 2: Run full CI pipeline**

Run: `cd compiler && just`
Expected: compile, coverage (85%+), and lint (clippy + fmt) all pass

**Step 3: Fix any issues**

Run: `cd compiler && just format` if needed for formatting.

**Step 4: Run VS Code CI**

Run: `cd integrations/vscode && just ci`
Expected: All checks pass

**Step 5: Commit**

```bash
git add compiler/container/src/task_table.rs \
        compiler/container/src/lib.rs \
        compiler/container/src/error.rs \
        compiler/container/src/container.rs \
        compiler/container/src/builder.rs \
        compiler/plc2x/src/disassemble.rs \
        compiler/vm/resources/test/steel_thread.iplc \
        integrations/vscode/src/iplcRendering.ts \
        integrations/vscode/src/test/unit/testHelpers.ts
git commit -m "Add task table section to container format

Implement Phase 1 of task support: binary task table section with
TaskEntry (32 bytes) and ProgramInstanceEntry (16 bytes) records.
ContainerBuilder auto-synthesizes a default freewheeling task when
no explicit task configuration is provided. Remove stale
entryFunctionId from VS Code extension."
```
