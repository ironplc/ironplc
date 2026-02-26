# Spec: no_std VM Implementation

## Overview

This spec defines the implementation plan for making the `ironplc-vm` and `ironplc-container` crates compile under `no_std`, enabling deployment on Arduino and other bare-metal microcontroller targets.

The core strategy is **two separate crates** instead of feature flags: `ironplc-vm` is always `#![no_std]` (the execution engine), and a new `ironplc-vm-cli` crate provides the desktop CLI binary with `std`. This makes the `no_std` constraint structural — if `ironplc-vm` compiles, it works on embedded targets. No conditional compilation in the VM.

This spec implements the decision in:

- **[ADR-0010: no_std VM for Embedded Targets](../adrs/0010-no-std-vm-for-embedded-targets.md)**: The architectural decision to support `no_std` without `alloc`

This spec builds on:

- **[Bytecode Container Format](bytecode-container-format.md)**: The container format that needs a zero-copy parsing path
- **[Runtime Execution Model](runtime-execution-model.md)**: The VM lifecycle that must work with borrowed data
- **[VM CLI](vm-cli.md)**: The CLI binary that moves to the `ironplc-vm-cli` crate

## Design Goals

1. **Zero external dependencies in embedded builds** — `ironplc-vm` depends only on `ironplc-container` (no_std) and `log`
2. **Zero-copy container loading** — bytecode is parsed in place from a `&[u8]` slice; no intermediate copies or heap buffers
3. **Fully deterministic memory** — all allocation sizes are known from the container header; no heap, no fragmentation
4. **No conditional compilation in the VM** — two crates replace feature flags; each crate is unconditionally `no_std` or `std`
5. **CI-verified on every build** — a bare-metal build target in the justfile ensures the `no_std` constraint is never accidentally broken

## Current `std` Dependency Inventory

### `ironplc-container` crate

| File | `std` usage | Category |
|---|---|---|
| `container.rs` | `std::io::{Cursor, Read, Write}` | I/O serialization |
| `header.rs` | `std::io::{Read, Write}` | I/O serialization |
| `code_section.rs` | `std::io::{Read, Write}` | I/O serialization |
| `constant_pool.rs` | `std::io::{Read, Write}` | I/O serialization |
| `error.rs` | `std::io::Error`, `std::error::Error`, `std::fmt` | Error types |
| `builder.rs` | `Vec` only | Heap allocation |
| `opcode.rs` | **None** | Ready as-is |

Heap types used (container): `Vec<u8>`, `Vec<FuncEntry>`, `Vec<ConstEntry>`.

Heap types used (VM): `Vec<Slot>` (stack, variable table), `Vec<TaskState>`, `Vec<ProgramInstanceState>`, `Vec<usize>`, `Vec<&ProgramInstanceState>` (scheduler).

### `ironplc-vm` crate

| File | `std` usage | Category |
|---|---|---|
| `vm.rs` | `std::sync::atomic::{AtomicBool, Ordering}`, `std::sync::Arc`, `std::time::Instant`, `std::thread::sleep` | Concurrency, timing |
| `value.rs` | **None** | Ready as-is |
| `stack.rs` | `Vec<Slot>` (via `Vec::with_capacity`, `push`, `pop`) | Heap allocation |
| `variable_table.rs` | `Vec<Slot>` (via `vec![]`, `get`, `get_mut`) | Heap allocation |
| `scheduler.rs` | `Vec<TaskState>`, `Vec<ProgramInstanceState>`, `Vec<usize>`, `Vec<&ProgramInstanceState>` | Heap allocation |
| `error.rs` | `std::fmt::Display`, `std::error::Error` | Error traits |
| `cli.rs` | `std::fs::File`, `std::io::Write`, `std::path::Path` | CLI only |
| `logger.rs` | `env_logger`, `std::fs::File`, `time::OffsetDateTime` | CLI only |
| `bin/main.rs` | `clap`, `std::path::PathBuf`, `println!` | CLI only |

### External dependencies

| Dependency | Used by | `no_std` compatible? |
|---|---|---|
| `clap` | `bin/main.rs` | No |
| `ctrlc` | `cli.rs` | No |
| `env_logger` | `logger.rs` | No |
| `log` | `logger.rs` | Yes |
| `time` | `logger.rs` | No |

## Phase 1: Zero-Copy Container Parsing

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
    code_bytes: &'a [u8],
    func_dir: &'a [u8],
}
```

**Constructor:**

```rust
impl<'a> ContainerRef<'a> {
    /// Parses a container from a byte slice without allocation.
    pub fn from_slice(data: &'a [u8]) -> Result<Self, ContainerError> {
        // Parse header from data[0..256]
        // Slice into const_pool_bytes, code_bytes, func_dir
        // No copies, no Vec, no std::io
    }
}
```

**Accessors** mirror the existing `Container` API but return slices:

- `header(&self) -> &FileHeader`
- `get_i32_constant(&self, index: u16) -> Result<i32, ContainerError>` — parses from `const_pool_bytes` on each call
- `get_function_bytecode(&self, id: u16) -> Option<&[u8]>` — slices into `code_bytes`

### `FileHeader::from_bytes`

Add a `from_bytes(&[u8; 256]) -> Result<FileHeader, ContainerError>` method that parses a header from a fixed-size array. The existing `read_from` already does this internally with a 256-byte buffer; extract the parsing logic into a shared function used by both paths.

### Gate existing I/O behind `std`

Wrap the following with `#[cfg(feature = "std")]`:

- `Container::read_from(impl Read)`
- `Container::write_to(impl Write)`
- `FileHeader::read_from(impl Read)` / `write_to(impl Write)`
- `CodeSection::read_from(impl Read)` / `write_to(impl Write)`
- `ConstantPool::read_from(impl Read)` / `write_to(impl Write)`
- `ContainerBuilder` (uses `Vec` internally; only needed by compiler)
- `ContainerError::Io(io::Error)` variant and `From<io::Error>` impl
- `impl std::error::Error for ContainerError`

### Container crate `lib.rs` changes

```rust
#![no_std]

#[cfg(feature = "std")]
extern crate std;

#[cfg(feature = "alloc")]
extern crate alloc;

// Always available (no_std)
pub mod opcode;
mod container_ref;
mod header;
// ...

// Only with std
#[cfg(feature = "std")]
mod builder;
#[cfg(feature = "std")]
mod container;
// ...
```

## Phase 2: Split VM into Two Crates

Instead of feature flags, the VM becomes two crates. `ironplc-vm` is always `#![no_std]` — no conditional compilation. A new `ironplc-vm-cli` crate provides the desktop CLI binary with full `std`.

### `ironplc-vm` — the no_std execution engine

This crate contains the core VM and nothing else. It is unconditionally `#![no_std]`.

**`lib.rs`:**

```rust
#![no_std]

pub mod error;
pub(crate) mod stack;
pub(crate) mod value;
pub(crate) mod variable_table;
mod vm;

pub use error::Trap;
pub use value::Slot;
pub use vm::{Vm, VmReady, VmRunning};
```

No `#[cfg]` gates. No optional dependencies. Every module is always compiled.

**`Cargo.toml`:**

```toml
[package]
name = "ironplc-vm"
# ...

[dependencies]
ironplc-container = { path = "../container", default-features = false }
log = "0.4.20"
```

No `[features]` section at all. The dependency on `ironplc-container` uses `default-features = false` to get only the no_std subset.

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

**`programs_for_task`** writes into a caller-provided buffer instead of returning a `Vec`:

```rust
pub fn programs_for_task<'b>(
    &self,
    task_id: u16,
    buf: &'b mut [&'a ProgramInstanceState],
) -> &'b [&'a ProgramInstanceState] {
    // Fill buf with matching program instances, return used slice
}
```

The caller allocates these buffers. On desktop, this is `Vec`; on embedded, stack-allocated arrays sized from the container header (which declares the number of tasks and programs).

### Move `StopHandle` and `Arc<AtomicBool>` to the CLI crate

The current VM uses `Arc<AtomicBool>` for cross-thread stop signaling (`StopHandle`). This is a desktop/CLI concern — it exists to support `ctrlc::set_handler` in `cli.rs`. On embedded, the main loop is single-threaded and the caller controls when to stop.

In the no_std VM, replace the atomic stop flag with a simple `bool`:

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

### Move `Instant` and `thread::sleep` to the CLI crate

The current VM uses `std::time::Instant` internally for scheduling and watchdog timing. The scheduler is already time-agnostic (it accepts `current_time_us: u64`), but `VmRunning::run_round` wraps this with `Instant::now().elapsed()`.

In the no_std VM, `run_round` accepts the current time as a parameter:

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

### Update the typestate VM to use `ContainerRef` and slices

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

The `Vm::load` method accepts a `ContainerRef` and caller-provided buffers:

```rust
impl Vm {
    pub fn load<'a>(
        self,
        container: ContainerRef<'a>,
        stack_buf: &'a mut [Slot],
        var_buf: &'a mut [Slot],
        task_states: &'a mut [TaskState],
        program_instances: &'a [ProgramInstanceState],
        ready_buf: &'a mut [usize],
    ) -> VmReady<'a> { ... }
}
```

### Error types

`core::fmt::Display` is available in `no_std`. `std::error::Error` is not needed in the VM crate at all — it lives in the CLI crate if required:

```rust
use core::fmt;

impl fmt::Display for Trap { ... }
```

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

**Contents:** move the existing `cli.rs`, `logger.rs`, and `bin/main.rs` from `ironplc-vm` into this crate. Add convenience wrappers that load a container from a file and allocate `Vec<Slot>` buffers from the header sizes.

### Workspace update

Add the new crate to `compiler/Cargo.toml`:

```toml
[workspace]
members = [
    # ... existing members ...
    "vm-cli",
]
```

## Phase 3: Container Crate Feature Flags

The container crate still uses feature flags because it serves two roles: the compiler writes containers (needs `std::io`, `Vec`), and the VM reads them (needs only `no_std`). This is a clean, contained split.

### `ironplc-container/Cargo.toml`

```toml
[features]
default = ["std"]
std = []
```

### Gate existing I/O behind `std`

Wrap the following with `#[cfg(feature = "std")]`:

- `Container::read_from(impl Read)`
- `Container::write_to(impl Write)`
- `FileHeader::read_from(impl Read)` / `write_to(impl Write)`
- `CodeSection::read_from(impl Read)` / `write_to(impl Write)`
- `ConstantPool::read_from(impl Read)` / `write_to(impl Write)`
- `ContainerBuilder` (uses `Vec` internally; only needed by compiler)
- `ContainerError::Io(io::Error)` variant and `From<io::Error>` impl
- `impl std::error::Error for ContainerError`

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
mod header;
// ...

// Only with std
#[cfg(feature = "std")]
mod builder;
#[cfg(feature = "std")]
mod container;
// ...
```

The `#[cfg]` usage is confined to the container crate. The VM crate and CLI crate have zero conditional compilation.

## Embedded Deployment Model

The embedded deployment does **not** require users to have a local Rust compiler or build toolchain. The application binary contains both the VM and the user's bytecode. At startup, the VM obtains a `&[u8]` reference to the bytecode and parses it via `ContainerRef::from_slice`.

### Embedded usage sketch

```rust
#![no_std]
#![no_main]

use ironplc_container::ContainerRef;
use ironplc_vm::{Slot, Vm};

#[arduino_hal::entry]
fn main() -> ! {
    let program: &[u8] = get_program_bytecode();

    let container = ContainerRef::from_slice(program).unwrap();

    let mut stack_buf = [Slot::default(); 16];
    let mut var_buf = [Slot::default(); 32];
    let mut task_states = [/* initialized from container header */];
    let program_instances = [/* initialized from container header */];
    let mut ready_buf = [0usize; 4];

    let mut vm = Vm::new()
        .load(
            container,
            &mut stack_buf,
            &mut var_buf,
            &mut task_states,
            &program_instances,
            &mut ready_buf,
        )
        .start();

    loop {
        let current_us = read_hardware_timer_us();
        vm.run_round(current_us).unwrap();
    }
}
```

## Verification

### Justfile CI integration

Add a `build-nostd` target to `compiler/justfile` and wire it into the default CI pipeline:

```just
default: compile coverage lint build-nostd

# Build the no_std VM for a bare-metal target to verify it compiles
build-nostd:
    rustup target add thumbv7em-none-eabihf
    cargo build -p ironplc-vm --target thumbv7em-none-eabihf
    cargo build -p ironplc-container --no-default-features --target thumbv7em-none-eabihf
```

This runs on every `cd compiler && just` invocation, so a PR that accidentally introduces a `std` dependency into `ironplc-vm` will fail CI.

Note: `ironplc-vm` needs no `--no-default-features` flag because it has no features — it is unconditionally `no_std`. The container crate needs `--no-default-features` to disable its `std` feature.

### What CI validates

| Build step | What it validates |
|---|---|
| `cargo build` (default) | `ironplc-vm-cli` and all std crates compile |
| `cargo test` / `coverage` | Existing tests pass, coverage stays above 85% |
| `cargo build -p ironplc-vm --target thumbv7em-none-eabihf` | VM crate is genuinely `no_std` — would fail if any `std` usage crept in |
| `cargo build -p ironplc-container --no-default-features --target thumbv7em-none-eabihf` | Container crate's no_std path compiles for bare-metal |

## Decisions

1. **Caller-provided slices** — the VM accepts `&mut [Slot]` slices rather than const generics. The container header determines sizes at runtime, so const generics would add type noise without real safety benefit. Slices keep VM types simple (`VmRunning<'a>` — one lifetime) and let each caller choose its allocation strategy.

2. **Feature flags confined to the container crate** — the container crate uses `#[cfg(feature = "std")]` to gate its I/O and builder modules. The shared types (`FileHeader`, opcodes, `ContainerError`) make a crate split awkward, and the `#[cfg]` usage is minimal and contained. The VM crate and CLI crate have zero conditional compilation.
