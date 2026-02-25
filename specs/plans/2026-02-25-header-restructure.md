# Header Restructure Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Reorder the 256-byte container header for logical grouping, remove `entry_function_id`, and add `task_section_offset`/`task_section_size` fields.

**Architecture:** Pure mechanical refactor. Same struct fields with new byte offsets, one field removed, two fields added. All existing tests must continue to pass with updated assertions. The VM temporarily hardcodes function 0 as the entry point until the task table is implemented.

**Tech Stack:** Rust, ironplc-container crate, ironplc-vm crate, ironplcc crate

---

### Task 1: Restructure FileHeader struct and serialization

**Files:**
- Modify: `compiler/container/src/header.rs`

**Step 1: Update the FileHeader struct**

Reorder fields to match the new layout. Remove `entry_function_id`. Add `task_section_offset` and `task_section_size`. Change reserved from `[u8; 30]` to `[u8; 24]`.

```rust
pub struct FileHeader {
    // Region 1: Identification (bytes 0-7)
    pub magic: u32,
    pub format_version: u16,
    pub profile: u8,
    pub flags: u8,
    // Region 2: Hashes (bytes 8-135)
    pub content_hash: [u8; 32],
    pub source_hash: [u8; 32],
    pub debug_hash: [u8; 32],
    pub layout_hash: [u8; 32],
    // Region 3: Section directory (bytes 136-191)
    pub sig_section_offset: u32,
    pub sig_section_size: u32,
    pub debug_sig_offset: u32,
    pub debug_sig_size: u32,
    pub type_section_offset: u32,
    pub type_section_size: u32,
    pub task_section_offset: u32,
    pub task_section_size: u32,
    pub const_section_offset: u32,
    pub const_section_size: u32,
    pub code_section_offset: u32,
    pub code_section_size: u32,
    pub debug_section_offset: u32,
    pub debug_section_size: u32,
    // Region 4: Runtime parameters (bytes 192-231)
    pub max_stack_depth: u16,
    pub max_call_depth: u16,
    pub num_variables: u16,
    pub num_fb_instances: u16,
    pub total_fb_instance_bytes: u32,
    pub total_str_var_bytes: u32,
    pub total_wstr_var_bytes: u32,
    pub num_temp_str_bufs: u16,
    pub num_temp_wstr_bufs: u16,
    pub max_str_length: u16,
    pub max_wstr_length: u16,
    pub num_functions: u16,
    pub num_fb_types: u16,
    pub num_arrays: u16,
    pub input_image_bytes: u16,
    pub output_image_bytes: u16,
    pub memory_image_bytes: u16,
    // Reserved (bytes 232-255)
    pub reserved: [u8; 24],
}
```

**Step 2: Update Default impl**

Match the new field order. Set `task_section_offset: 0`, `task_section_size: 0`. Change `reserved: [0; 24]`. Remove `entry_function_id`.

**Step 3: Update write_to**

Reorder the `w.write_all(...)` calls to match the new struct order. The byte-level output must match the offset table in the spec. Remove the `entry_function_id` write. Add writes for `task_section_offset` and `task_section_size`.

**Step 4: Update read_from**

Update all `buf[N]` indices to the new offsets. Remove `entry_function_id` parsing. Add parsing for `task_section_offset` (bytes 160-163) and `task_section_size` (bytes 164-167). Change reserved slice from `buf[226..256]` to `buf[232..256]`.

Key offset changes for read_from:
- Section directory starts at byte 136 (was 170)
- task_section_offset at bytes 160-163 (new)
- task_section_size at bytes 164-167 (new)
- Runtime params start at byte 192 (was 136)
- I/O images at bytes 226-231 (was 218-223)
- Reserved at bytes 232-255 (was 226-255)

**Step 5: Update the roundtrip test**

Update `header_write_read_when_default_then_roundtrips` to:
- Remove `assert_eq!(decoded.entry_function_id, 0)`
- Add `assert_eq!(decoded.task_section_offset, 0)`
- Add `assert_eq!(decoded.task_section_size, 0)`
- Change `assert_eq!(decoded.reserved, [0; 30])` to `[0; 24]`

**Step 6: Run tests to verify**

Run: `cd compiler && cargo test --package ironplc-container`
Expected: All 4 tests pass (header roundtrip, invalid magic, size check, builder steel thread)

---

### Task 2: Update ContainerBuilder

**Files:**
- Modify: `compiler/container/src/builder.rs`

**Step 1: Remove entry_function_id from build()**

In the `build()` method, remove `entry_function_id: 0` from the `FileHeader` initializer. The default (from `..FileHeader::default()`) handles all zero-initialized fields including the new task section fields.

**Step 2: Run tests to verify**

Run: `cd compiler && cargo test --package ironplc-container`
Expected: All tests pass

---

### Task 3: Update Container write/read for new section ordering

**Files:**
- Modify: `compiler/container/src/container.rs`

**Step 1: Update write_to**

The section offset computation currently starts the constant pool immediately after the header. This is correct — the task table section will be inserted between type and constant pool in a future phase. For now, `task_section_offset` and `task_section_size` remain 0 (from Default). No changes needed to write_to beyond what the header struct change already covers.

Verify: read the file and confirm no explicit `entry_function_id` references exist.

**Step 2: Run the container roundtrip test**

Run: `cd compiler && cargo test --package ironplc-container`
Expected: All tests pass

---

### Task 4: Update VM to not use entry_function_id

**Files:**
- Modify: `compiler/vm/src/vm.rs`

**Step 1: Replace entry_function_id with hardcoded function 0**

In `run_single_scan()`, change:
```rust
let entry_id = self.container.header.entry_function_id;
```
to:
```rust
// TODO: Read entry point from task table once implemented.
// For now, function 0 is always the entry point.
let entry_id: u16 = 0;
```

**Step 2: Run VM tests**

Run: `cd compiler && cargo test --package ironplc-vm`
Expected: All 3 tests pass (load valid, steel thread scan, invalid opcode trap)

---

### Task 5: Update disassembler

**Files:**
- Modify: `compiler/plc2x/src/disassemble.rs`

**Step 1: Update disassemble_header()**

Remove `"entryFunctionId": h.entry_function_id` from the JSON output. Add task section info:
```rust
"taskSection": {
    "offset": h.task_section_offset,
    "size": h.task_section_size,
},
```

**Step 2: Update the entry_function_id test**

Replace `disassemble_when_steel_thread_then_header_has_entry_function_id` with a test for the task section:
```rust
#[test]
fn disassemble_when_steel_thread_then_header_has_task_section() {
    let container = steel_thread_container();
    let result = disassemble(&container);
    assert_eq!(result["header"]["taskSection"]["offset"], 0);
    assert_eq!(result["header"]["taskSection"]["size"], 0);
}
```

**Step 3: Run disassembler tests**

Run: `cd compiler && cargo test --package ironplcc -- disassemble`
Expected: All disassembler tests pass

---

### Task 6: Run full CI pipeline and commit

**Step 1: Run full CI**

Run: `cd compiler && just`
Expected: compile, coverage (85%+), and lint (clippy + fmt) all pass

**Step 2: Fix any clippy or format issues**

Run: `cd compiler && just format` if needed

**Step 3: Commit**

```bash
git add compiler/container/src/header.rs \
        compiler/container/src/builder.rs \
        compiler/container/src/container.rs \
        compiler/vm/src/vm.rs \
        compiler/plc2x/src/disassemble.rs
git commit -m "Restructure container header layout for task support

Reorder header fields into logical regions: identification, hashes,
section directory, runtime parameters. Remove entry_function_id
(replaced by task table). Add task_section_offset/size to section
directory. VM temporarily hardcodes function 0 as entry point."
```
