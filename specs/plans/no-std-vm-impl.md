# Implementation Plan: no_std VM

**Design:** [no_std VM Design](../design/no-std-vm.md)

## Phase 1: Container Crate — no_std with Zero-Copy Parsing

The container crate serves two roles: the compiler writes containers (needs `std::io`, `Vec`), and the VM reads them (needs only `no_std`). This phase makes the crate `#![no_std]` at the root, gates existing I/O behind a `std` feature flag, and adds a new zero-copy `ContainerRef` type for the embedded path.

The existing `Vec`-based types — `Container`, `CodeSection`, `ConstantPool`, `ConstEntry`, `TaskTable`, and `ContainerBuilder` — remain `std`-only. They are not modified or replaced; they are simply gated behind `#[cfg(feature = "std")]`. The no_std path uses `ContainerRef` exclusively, which borrows a flat byte slice and parses on access. There is no need for no_std alternatives of the write-side types because container creation always happens on the host compiler.

### `ironplc-container/Cargo.toml`

```toml
[features]
default = ["std"]
std = []
```

### Container crate `lib.rs` changes

```rust
#![no_std]

#[cfg(feature = "std")]
extern crate std;

#[cfg(feature = "std")]
extern crate alloc;

// Always available (no_std)
pub mod opcode;
mod container_ref;
mod error;
mod header;
// ...

// Only with std
#[cfg(feature = "std")]
mod builder;
#[cfg(feature = "std")]
mod code_section;
#[cfg(feature = "std")]
mod constant_pool;
#[cfg(feature = "std")]
mod container;
#[cfg(feature = "std")]
mod task_table;
// ...
```

The `#[cfg]` usage is confined to the container crate. The VM crate and CLI crate have zero conditional compilation.

### Gate existing I/O behind `std`

Wrap the following with `#[cfg(feature = "std")]`:

- The entire `Container` type and its `read_from` / `write_to` methods
- The entire `CodeSection`, `ConstantPool`, `TaskTable` types (these use `Vec` internally)
- `FileHeader::read_from(impl Read)` / `write_to(impl Write)` methods (the type itself stays no_std)
- `ContainerBuilder` (uses `Vec` internally; only needed by compiler)
- `ContainerError::Io(io::Error)` variant and `From<io::Error>` impl
- `impl std::error::Error for ContainerError`

### `FileHeader::from_bytes`

Add a `from_bytes(&[u8; 256]) -> Result<FileHeader, ContainerError>` method that parses a header from a fixed-size array. The existing `read_from` already does this internally with a 256-byte buffer; extract the parsing logic into a shared function used by both paths.

### `ContainerRef` type

Add a new `ContainerRef<'a>` type that borrows a byte slice and provides read-only accessors. This is the only container representation the embedded VM needs.

```rust
/// A borrowed view of a bytecode container in a flat byte slice.
///
/// No heap allocation. The lifetime `'a` ties the view to the
/// underlying byte buffer.
pub struct ContainerRef<'a> {
    header: FileHeader,
    const_pool_bytes: &'a [u8],
    const_offsets: &'a [u32],
    code_bytes: &'a [u8],
    func_dir: &'a [u8],
    task_table_bytes: &'a [u8],
}
```

**Construction** is two-phase because the caller must allocate the constant offset index buffer, whose size depends on parsing the container:

```rust
impl<'a> ContainerRef<'a> {
    /// Parses a container header and returns the number of constant
    /// pool entries. The caller uses this to allocate the offset
    /// index buffer before calling `from_slice`.
    pub fn const_count(data: &[u8]) -> Result<u16, ContainerError> {
        // Parse header from data[0..256] to get const_section_offset
        // Read the u16 count from the first 2 bytes of const_pool section
    }

    /// Parses a container from a byte slice without allocation.
    ///
    /// `const_offset_buf` must have length >= the constant pool count
    /// (obtained from `const_count`). This method fills it with the
    /// byte offset of each constant entry within the constant pool
    /// section, enabling O(1) lookups at runtime.
    pub fn from_slice(
        data: &'a [u8],
        const_offset_buf: &'a mut [u32],
    ) -> Result<Self, ContainerError> {
        // Parse header from data[0..256] via FileHeader::from_bytes
        // Slice into const_pool_bytes, code_bytes, func_dir, task_table_bytes
        // Pre-scan constant pool to fill const_offset_buf:
        //   walk entries (type(1) + reserved(1) + size(2) + value(size)),
        //   record each entry's byte offset
        // Store const_offset_buf as const_offsets
    }
}
```

On embedded, the caller stack-allocates `const_offset_buf`. On desktop, the CLI crate uses a `Vec`. The cost is `num_constants * 4` bytes — typically small (e.g., 20 constants = 80 bytes).

**Accessors:**

- `header(&self) -> &FileHeader`

- `get_i32_constant(&self, index: u16) -> Result<i32, ContainerError>` — uses `const_offsets[index]` to jump directly to the entry in `const_pool_bytes`. O(1) per lookup.

- `get_function_bytecode(&self, id: u16) -> Option<&[u8]>` — the function directory has fixed-size entries (14 bytes each: `function_id(2) + offset(4) + length(4) + stack_depth(2) + num_locals(2)`). Reads the entry at `func_dir[id * 14 .. (id+1) * 14]` to get the offset and length, then slices into `code_bytes`. O(1) per lookup.

- `num_tasks(&self) -> u16` — reads from the first 2 bytes of `task_table_bytes`.

- `num_programs(&self) -> u16` — reads from bytes 2..4 of `task_table_bytes`.

- `shared_globals_size(&self) -> u16` — reads from bytes 4..6 of `task_table_bytes`.

- `task_entry(&self, index: u16) -> Result<TaskEntryRef, ContainerError>` — task entries are fixed-size (32 bytes each), starting at byte 6 of `task_table_bytes`. Returns a lightweight struct parsed from `task_table_bytes[6 + index * 32 .. 6 + (index+1) * 32]`. O(1).

- `program_entry(&self, index: u16) -> Result<ProgramEntryRef, ContainerError>` — program instance entries are fixed-size (16 bytes each), starting after all task entries. Returns a lightweight struct parsed from the appropriate offset. O(1).

`TaskEntryRef` and `ProgramEntryRef` are simple structs with the same fields as the existing `TaskEntry` and `ProgramInstanceEntry`, but they are always-available `no_std` types (no `Vec` fields). They exist alongside the `std`-gated `TaskEntry`/`ProgramInstanceEntry` which are used by the builder and `read_from` paths.

## Phase 2: Create `ironplc-vm-cli` Crate and Move CLI Code

This phase is a pure code move with no behavioral changes. The VM crate still uses `std` after this phase. All existing tests pass — they just run from a different crate.

### `ironplc-vm-cli` — the desktop CLI binary

A new crate at `compiler/vm-cli/` that provides the `ironplcvm` binary. This is a normal `std` crate with no conditional compilation.

**`Cargo.toml`:**

```toml
[package]
name = "ironplc-vm-cli"
# ...

[[bin]]
name = "ironplcvm"
path = "src/main.rs"

[dependencies]
ironplc-vm = { path = "../vm" }
ironplc-container = { path = "../container" }
clap = { version = "4.0", features = ["derive", "wrap_help"] }
ctrlc = "3"
env_logger = "0.10.0"
log = "0.4.20"
time = "0.3.17"
```

All dependencies are unconditional. No `[features]` section.

**Contents:** move the existing `cli.rs`, `logger.rs`, and `bin/main.rs` from `ironplc-vm` into this crate. The CLI code continues to use the existing `Container`, `StopHandle`, and `std::time::Instant` APIs unchanged.

### Files moved

| From (`ironplc-vm`) | To (`ironplc-vm-cli`) |
|---|---|
| `bin/main.rs` | `src/main.rs` |
| `src/cli.rs` | `src/cli.rs` |
| `src/logger.rs` | `src/logger.rs` |
| `tests/cli.rs` | `tests/cli.rs` |
| `resources/test/steel_thread.iplc` | `resources/test/steel_thread.iplc` (golden file for CLI tests) |

### `ironplc-vm` cleanup

After the move, remove from `ironplc-vm`:

- The `[[bin]]` section from `Cargo.toml`
- The `cli`, `logger` modules from `lib.rs`
- The `clap`, `ctrlc`, `env_logger`, `time` dependencies from `Cargo.toml`
- The `assert_cmd`, `predicates`, `tempfile` dev-dependencies (used only by CLI tests)
- The `bin/` directory and `tests/cli.rs`

The VM crate's `lib.rs` becomes:

```rust
pub mod error;
pub(crate) mod scheduler;
pub(crate) mod stack;
pub(crate) mod value;
pub(crate) mod variable_table;
mod vm;

pub use value::Slot;
pub use vm::{FaultContext, StopHandle, Vm, VmFaulted, VmReady, VmRunning, VmStopped};
```

Note: the VM crate still uses `std` at this point. `StopHandle`, `Arc<AtomicBool>`, `Instant`, and `Vec` remain unchanged. The only change is that CLI-specific code has moved out.

### Workspace update

Add the new crate to `compiler/Cargo.toml`:

```toml
[workspace]
members = [
    # ... existing members ...
    "vm-cli",
]
```

### What to verify

- `cd compiler && just` passes (compile, coverage, lint)
- The `ironplcvm` binary is now built from the `vm-cli` crate
- All existing VM unit tests in `vm/src/` still pass (they are unchanged)
- All CLI tests pass from their new location in `vm-cli/tests/`

## Phase 3: Refactor VM to Accept Caller-Provided Buffers

This phase changes the VM's ownership and allocation model. `Vec`-based internal storage becomes caller-provided slices, `Instant` becomes a caller-provided timestamp, and `Arc<AtomicBool>` becomes a simple `bool`. The VM crate **still uses `std`** after this phase — it borrows `&Container` rather than owning it. The no_std switch happens in Phase 4.

### Replace `Vec<Slot>` with caller-provided slices

The VM accepts `&mut [Slot]` slices for the stack and variable table. The caller is responsible for allocation — on desktop this is a `Vec`, on embedded this is a stack-allocated or static array.

**`OperandStack`:**

```rust
pub struct OperandStack<'a> {
    data: &'a mut [Slot],
    len: usize,
}

impl<'a> OperandStack<'a> {
    pub fn new(backing: &'a mut [Slot]) -> Self {
        OperandStack { data: backing, len: 0 }
    }

    pub fn push(&mut self, slot: Slot) -> Result<(), Trap> {
        if self.len >= self.data.len() {
            return Err(Trap::StackOverflow);
        }
        self.data[self.len] = slot;
        self.len += 1;
        Ok(())
    }

    pub fn pop(&mut self) -> Result<Slot, Trap> {
        if self.len == 0 {
            return Err(Trap::StackUnderflow);
        }
        self.len -= 1;
        Ok(self.data[self.len])
    }
}
```

**`VariableTable`:** same pattern — wraps `&mut [Slot]`.

### Replace `Vec` in `TaskScheduler` with caller-provided slices

The scheduler currently uses `Vec<TaskState>`, `Vec<ProgramInstanceState>`, and returns `Vec<usize>` from `collect_ready_tasks`. These must become caller-provided slices, just like the stack and variable table.

**`TaskScheduler`:**

```rust
pub struct TaskScheduler<'a> {
    pub task_states: &'a mut [TaskState],
    pub program_instances: &'a [ProgramInstanceState],
    pub shared_globals_size: u16,
}
```

**`collect_ready_tasks`** writes into a caller-provided buffer and returns the used portion:

```rust
pub fn collect_ready_tasks<'b>(
    &self,
    current_time_us: u64,
    buf: &'b mut [usize],
) -> &'b [usize] {
    // Fill buf with indices of ready tasks, sort by priority, return used slice
}
```

**`programs_for_task` is removed.** The current implementation returns `Vec<&ProgramInstanceState>`, which allocates. Instead, `run_round` iterates `self.scheduler.program_instances` directly with an inline `.filter(|p| p.task_id == task_id)`. This works because:

- The inner for loop borrows `self.scheduler.program_instances` immutably (via field-level splitting)
- Inside the loop body, `execute()` borrows `self.stack` and `self.variables` mutably — these are different fields of `VmRunning`, so the borrow checker allows the split
- After the inner for loop ends, the immutable borrow on the scheduler is released
- `self.scheduler.record_execution(...)` then mutably borrows the scheduler with no conflict

This avoids the need for a programs buffer, keeping `Vm::load`'s parameter list simpler. The filter is O(n) where n is the total number of program instances, but n is typically very small (2-10 for even complex PLC programs).

The caller allocates the `ready_buf` buffer for `collect_ready_tasks`. On desktop, this is a `Vec`; on embedded, a stack-allocated array sized from the container header.

### Replace `StopHandle` and `Arc<AtomicBool>` with a simple `bool`

The `StopHandle` type and `Arc<AtomicBool>` are removed from the VM crate. They were a desktop/CLI concern — they exist to support `ctrlc::set_handler` in `cli.rs`. On embedded, the main loop is single-threaded and the caller controls when to stop.

Replace the atomic stop flag with a simple `bool`:

```rust
pub struct VmRunning<'a> {
    // ...
    stop_requested: bool,
}

impl<'a> VmRunning<'a> {
    pub fn stop_requested(&self) -> bool {
        self.stop_requested
    }

    pub fn request_stop(&mut self) {
        self.stop_requested = true;
    }
}
```

The CLI crate wraps this with `Arc<AtomicBool>` for signal handler support:

```rust
// In ironplc-vm-cli
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

let stop_flag = Arc::new(AtomicBool::new(false));
let handle = stop_flag.clone();
ctrlc::set_handler(move || handle.store(true, Ordering::Relaxed))?;

loop {
    if stop_flag.load(Ordering::Relaxed) {
        running.request_stop();
    }
    // ...
}
```

This keeps `std::sync` entirely out of the VM crate.

### Replace `Instant` and `thread::sleep` with caller-provided time

The current VM uses `std::time::Instant` internally for scheduling and watchdog timing. The scheduler is already time-agnostic (it accepts `current_time_us: u64`), but `VmRunning::run_round` wraps this with `Instant::now().elapsed()`.

`run_round` now accepts the current time as a parameter:

```rust
impl<'a> VmRunning<'a> {
    pub fn run_round(&mut self, current_time_us: u64) -> Result<(), FaultContext> {
        let ready = self.scheduler.collect_ready_tasks(current_time_us, &mut self.ready_buf);
        // ...
    }
}
```

The CLI crate provides the time source:

```rust
// In ironplc-vm-cli
let start = std::time::Instant::now();
loop {
    let current_us = start.elapsed().as_micros() as u64;
    running.run_round(current_us)?;
    // Sleep logic lives here, not in the VM
}
```

On embedded, the caller reads a hardware timer or tick counter to supply `current_time_us`.

### Update the typestate VM to borrow `Container` and accept slices

The VM state types gain a lifetime `'a` and borrow the `Container` instead of owning it. The VM still uses the existing `Container` type (not `ContainerRef`) — that switch happens in Phase 4.

The `VmRunning` struct becomes:

```rust
pub struct VmRunning<'a> {
    container: &'a Container,
    stack: OperandStack<'a>,
    variables: VariableTable<'a>,
    scheduler: TaskScheduler<'a>,
    scan_count: u64,
    stop_requested: bool,
    ready_buf: &'a mut [usize],
}
```

The `Vm::load` method accepts a borrowed `Container` and caller-provided buffers:

```rust
impl Vm {
    pub fn load<'a>(
        self,
        container: &'a Container,
        stack_buf: &'a mut [Slot],
        var_buf: &'a mut [Slot],
        task_states: &'a mut [TaskState],
        program_instances: &'a mut [ProgramInstanceState],
        ready_buf: &'a mut [usize],
    ) -> VmReady<'a> { ... }
}
```

Inside `load`:

1. Zero-fill `var_buf` (all variables start at zero).
2. Populate `task_states` from `container.task_table.tasks` — map each `TaskEntry` to a `TaskState` with runtime fields (`next_due_us`, `scan_count`, etc.) initialized to zero and `enabled` derived from the entry's flags.
3. Populate `program_instances` from `container.task_table.programs` — map each `ProgramInstanceEntry` to a `ProgramInstanceState`.
4. Construct the `TaskScheduler`, `OperandStack`, and `VariableTable` from the now-populated slices.

Note: buffer size validation is not strictly needed here because the caller has access to `container.header` to size their buffers, and the current `Container::read_from` path is already trusted. Validation is added in Phase 4 when the API becomes the public embedded interface.

### Error types

`core::fmt::Display` is available in `no_std`. Switch `Trap`'s `Display` impl to use `core::fmt` and remove `impl std::error::Error for Trap`:

```rust
use core::fmt;

impl fmt::Display for Trap { ... }
```

The CLI crate can implement `std::error::Error for Trap` locally if needed for error handling.

### `VmReady`, `VmStopped`, `VmFaulted` updates

All VM state types gain the `'a` lifetime and borrow the `Container` instead of owning it:

```rust
pub struct VmReady<'a> {
    container: &'a Container,
    stack: OperandStack<'a>,
    variables: VariableTable<'a>,
}

pub struct VmStopped<'a> {
    container: &'a Container,
    variables: VariableTable<'a>,
    scan_count: u64,
}

pub struct VmFaulted<'a> {
    trap: Trap,
    task_id: u16,
    instance_id: u16,
    container: &'a Container,
    variables: VariableTable<'a>,
}
```

The public API (`read_variable`, `num_variables`, `scan_count`, `trap`, etc.) is unchanged — the methods work identically with borrowed data.

### CLI crate updates

The `ironplc-vm-cli` crate adapts to the new VM API:

- Read the container via `Container::read_from` as before — the `Container` is kept alive alongside the VM
- Allocate `Vec<Slot>`, `Vec<TaskState>`, `Vec<ProgramInstanceState>`, `Vec<usize>` from header sizes
- Pass `current_time_us` to `run_round` using `std::time::Instant`
- Wrap the `bool`-based `request_stop` with `Arc<AtomicBool>` for `ctrlc` signal handler support

### What to verify

- `cd compiler && just` passes (compile, coverage, lint)
- All VM unit tests pass with the new slice-based APIs
- The steel thread integration test passes with borrowed `Container`
- CLI integration tests pass with the updated CLI crate

## Phase 4: Switch to `ContainerRef` and `#![no_std]`

With the VM already using borrowed data and caller-provided buffers, this phase swaps the container representation from `&Container` to `ContainerRef` and adds `#![no_std]`. This is a smaller, focused change.

### `ironplc-vm` becomes unconditionally `#![no_std]`

**`lib.rs`:**

```rust
#![no_std]

pub mod error;
pub(crate) mod scheduler;
pub(crate) mod stack;
pub(crate) mod value;
pub(crate) mod variable_table;
mod vm;

pub use error::Trap;
pub use scheduler::{ProgramInstanceState, TaskState};
pub use value::Slot;
pub use vm::{FaultContext, Vm, VmFaulted, VmReady, VmRunning, VmStopped};
```

No `#[cfg]` gates. No optional dependencies. Every module is always compiled.

**`Cargo.toml`:**

```toml
[package]
name = "ironplc-vm"
# ...

[dependencies]
ironplc-container = { path = "../container", default-features = false }
```

No `[features]` section at all. The dependency on `ironplc-container` uses `default-features = false` to get only the no_std subset.

### Replace `&Container` with `ContainerRef` in all VM types

The `VmRunning` struct becomes:

```rust
pub struct VmRunning<'a> {
    container: ContainerRef<'a>,
    stack: OperandStack<'a>,
    variables: VariableTable<'a>,
    scheduler: TaskScheduler<'a>,
    scan_count: u64,
    stop_requested: bool,
    ready_buf: &'a mut [usize],
}
```

`VmReady`, `VmStopped`, and `VmFaulted` change similarly — `&'a Container` becomes `ContainerRef<'a>`.

### `Vm::load` accepts `ContainerRef` and validates buffers

```rust
impl Vm {
    pub fn load<'a>(
        self,
        container: ContainerRef<'a>,
        stack_buf: &'a mut [Slot],
        var_buf: &'a mut [Slot],
        task_states: &'a mut [TaskState],
        program_instances: &'a mut [ProgramInstanceState],
        ready_buf: &'a mut [usize],
    ) -> Result<VmReady<'a>, ContainerError> { ... }
}
```

Inside `load`:

1. Validate buffer sizes against the container header (e.g., `stack_buf.len() >= header.max_stack_depth`, `task_states.len() >= container.num_tasks()`). Return `ContainerError` if any buffer is too small.
2. Zero-fill `var_buf` (all variables start at zero).
3. Populate `task_states` by iterating `container.task_entry(i)` for each task. Each `TaskEntryRef` is mapped to a `TaskState` with runtime fields initialized to zero.
4. Populate `program_instances` by iterating `container.program_entry(i)` for each program. Each `ProgramEntryRef` is mapped to a `ProgramInstanceState`.
5. Construct the `TaskScheduler`, `OperandStack`, and `VariableTable` from the now-populated slices.

Note: `load` now returns `Result` because buffer size validation can fail. With caller-provided slices, undersized buffers are a realistic error that must be caught at load time.

### Update `execute()` to use `ContainerRef`

The `execute()` free function changes from `container: &Container` to `container: &ContainerRef`:

```rust
fn execute(
    bytecode: &[u8],
    container: &ContainerRef,
    stack: &mut OperandStack,
    variables: &mut VariableTable,
    scope: &VariableScope,
) -> Result<(), Trap> { ... }
```

Inside the function, the two call sites change:

- `container.constant_pool.get_i32(index)` becomes `container.get_i32_constant(index)` — same semantics, now backed by the pre-scanned constant offset index for O(1) lookup.
- `container.code.get_function_bytecode(id)` becomes `container.get_function_bytecode(id)` — same semantics, O(1) via the fixed-size function directory.

No other changes to the function body. The `execute()` function remains a free function (not a method) so the borrow checker can see independent borrows of `container` (immutable) vs. `stack`/`variables` (mutable).

### CLI crate updates

The `ironplc-vm-cli` crate switches from `Container` to `ContainerRef`:

- Read the entire container file into a `Vec<u8>` buffer
- Allocate a `Vec<u32>` for constant offsets using `ContainerRef::const_count`
- Parse via `ContainerRef::from_slice` instead of `Container::read_from`
- All other buffer allocation and VM usage remains the same as Phase 3

### What to verify

- `cd compiler && just` passes (compile, coverage, lint, build-nostd)
- `cargo build -p ironplc-vm --target thumbv7em-none-eabihf` succeeds
- All VM unit tests pass via the `ContainerRef` path
- The steel thread integration test passes via `ContainerRef::from_slice`
- CLI integration tests pass with the updated CLI crate

## Test strategy

Tests run on the host (not on embedded hardware). The bare-metal CI build validates that the code *compiles* for `no_std`; the tests below validate that it *works correctly*.

### Test helper: `ContainerBuilder` → bytes → `ContainerRef`

After the refactor, the VM accepts `ContainerRef` but test containers are built with `ContainerBuilder` (which produces `Container`). A test helper bridges this gap:

```rust
/// Serializes a Container to bytes, then parses as ContainerRef.
/// Used by VM tests to construct test containers without changing
/// the builder API.
fn container_ref_from_builder<'a>(
    builder: ContainerBuilder,
    bytes_buf: &'a mut Vec<u8>,
    const_offsets: &'a mut Vec<u32>,
) -> ContainerRef<'a> {
    let container = builder.build();
    bytes_buf.clear();
    container.write_to(bytes_buf).unwrap();
    let count = ContainerRef::const_count(bytes_buf).unwrap() as usize;
    const_offsets.resize(count, 0);
    ContainerRef::from_slice(bytes_buf, const_offsets).unwrap()
}
```

This helper lives in a `#[cfg(test)]` module (it uses `Vec`, which is fine — tests run on the host with `std`). All existing VM unit tests and integration tests use this helper to construct `ContainerRef` values from the same `ContainerBuilder` calls they use today.

### Container crate tests

| Test | What it validates |
|---|---|
| `container_ref_from_slice_when_valid_bytes_then_parses` | Happy path: serialize a `Container`, parse back via `ContainerRef::from_slice`, verify header fields match |
| `container_ref_from_slice_when_invalid_magic_then_error` | Rejects bytes with wrong magic number |
| `container_ref_from_slice_when_truncated_then_error` | Rejects byte slices shorter than 256 bytes |
| `container_ref_get_i32_constant_when_valid_index_then_returns_value` | Constant lookup via pre-scanned offset index returns correct value |
| `container_ref_get_i32_constant_when_out_of_bounds_then_error` | Out-of-bounds constant index returns error |
| `container_ref_get_function_bytecode_when_valid_id_then_returns_slice` | Function bytecode lookup via fixed-size directory returns correct bytes |
| `container_ref_task_entry_when_valid_index_then_returns_fields` | Task entry parsing returns correct task_id, priority, interval, etc. |
| `container_ref_program_entry_when_valid_index_then_returns_fields` | Program entry parsing returns correct instance_id, task_id, offsets, etc. |

### VM crate tests

Existing unit tests in `stack.rs`, `variable_table.rs`, `scheduler.rs`, and `vm.rs` are updated to use the new slice-based APIs:

| Module | Change |
|---|---|
| `stack.rs` | `OperandStack::new(4)` becomes `OperandStack::new(&mut [Slot::default(); 4])`. Same test logic, different construction. |
| `variable_table.rs` | `VariableTable::new(3)` becomes `VariableTable::new(&mut [Slot::default(); 3])`. Same test logic. |
| `scheduler.rs` | `collect_ready_tasks` tests pass a `&mut [usize]` buffer and assert on the returned slice. `programs_for_task` tests are removed (the method is replaced by inline iteration in `run_round`). The test helper tables (`freewheeling_task_table`, `two_cyclic_tasks_table`) construct `ContainerRef` values via the test helper instead of `TaskTable` structs directly. |
| `vm.rs` | Tests use `container_ref_from_builder` instead of `ContainerBuilder::build()`. `run_round` calls pass `current_time_us: 0`. `StopHandle` tests are removed (stop handle moves to CLI crate); replaced by tests for the `bool`-based `request_stop`/`stop_requested`. |

### Integration tests

| Test | Location | What it validates |
|---|---|---|
| Steel thread (no_std path) | `vm/tests/steel_thread.rs` | Build container via `ContainerBuilder` → serialize to bytes → parse via `ContainerRef::from_slice` → `Vm::load` with stack-allocated buffers → `run_round` → verify `x == 10, y == 42`. This replaces the current test which uses `Container::read_from`. |
| CLI behavior | `vm-cli/tests/cli.rs` | Moved from `vm/tests/cli.rs`. Tests the `ironplcvm` binary (run, version, dump-vars). Uses `assert_cmd` and the golden `.iplc` file. |

### What does NOT need new tests

- `FileHeader::from_bytes` — tested indirectly by every `ContainerRef::from_slice` test. Can add a direct unit test but not required for coverage.
- The `execute()` function — its logic is unchanged; only the container access pattern changes. Covered by the existing VM unit tests and steel thread integration test.
- CLI crate wrappers (buffer allocation from header sizes) — tested indirectly by the CLI integration tests.
