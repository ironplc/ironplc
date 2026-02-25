# Spec: no_std VM Implementation

## Overview

This spec defines the implementation plan for making the `ironplc-vm` and `ironplc-container` crates compile under `no_std`, enabling deployment on Arduino and other bare-metal microcontroller targets.

This spec implements the decision in:

- **[ADR-0010: no_std VM for Embedded Targets](../adrs/0010-no-std-vm-for-embedded-targets.md)**: The architectural decision to support `no_std` without `alloc`

This spec builds on:

- **[Bytecode Container Format](bytecode-container-format.md)**: The container format that needs a zero-copy parsing path
- **[Runtime Execution Model](runtime-execution-model.md)**: The VM lifecycle that must work with borrowed data
- **[VM CLI](vm-cli.md)**: The CLI binary that will be gated behind the `std` feature

## Design Goals

1. **Zero external dependencies in embedded builds** — the `no_std` build of `ironplc-vm` depends only on `ironplc-container` (itself `no_std`)
2. **Zero-copy container loading** — on embedded targets, bytecode lives in flash and is parsed in place via `&[u8]`
3. **Fully deterministic memory** — all allocation sizes are known from the container header; no heap, no fragmentation
4. **No change for desktop users** — `default = ["std"]` preserves the current API and behavior

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

Heap types used: `Vec<u8>`, `Vec<FuncEntry>`, `Vec<ConstEntry>`.

### `ironplc-vm` crate

| File | `std` usage | Category |
|---|---|---|
| `vm.rs` | **None** | Ready as-is |
| `value.rs` | **None** | Ready as-is |
| `stack.rs` | `Vec<Slot>` (via `Vec::with_capacity`, `push`, `pop`) | Heap allocation |
| `variable_table.rs` | `Vec<Slot>` (via `vec![]`, `get`, `get_mut`) | Heap allocation |
| `error.rs` | `std::fmt::Display`, `std::error::Error` | Error traits |
| `cli.rs` | `std::fs::File`, `std::io::Write`, `std::path::Path` | CLI only |
| `logger.rs` | `env_logger`, `std::fs::File`, `time::OffsetDateTime` | CLI only |
| `bin/main.rs` | `clap`, `std::path::PathBuf`, `println!` | CLI only |

### External dependencies

| Dependency | Used by | `no_std` compatible? |
|---|---|---|
| `clap` | `bin/main.rs` | No |
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
/// underlying byte buffer (typically flash memory on embedded).
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

## Phase 2: VM Execution Core

### Replace `Vec<Slot>` with caller-provided slices

Rather than const generics (which propagate through the type signatures of `Vm`, `VmReady`, `VmRunning`), the VM accepts `&mut [Slot]` slices for the stack and variable table. The caller is responsible for allocation — on desktop this is a `Vec`, on embedded this is a stack-allocated or static array.

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

### Update the typestate VM to use `ContainerRef` and slices

The `VmRunning` struct becomes:

```rust
pub struct VmRunning<'a> {
    container: ContainerRef<'a>,
    stack: OperandStack<'a>,
    variables: VariableTable<'a>,
    scan_count: u64,
}
```

The `Vm::load` method accepts a `ContainerRef` and `&mut [Slot]` buffers:

```rust
impl Vm {
    pub fn load<'a>(
        self,
        container: ContainerRef<'a>,
        stack_buf: &'a mut [Slot],
        var_buf: &'a mut [Slot],
    ) -> VmReady<'a> { ... }
}
```

On desktop (with `std`), a convenience wrapper can allocate `Vec<Slot>` buffers from the header sizes.

### Gate CLI modules

In `ironplc-vm/src/lib.rs`:

```rust
#![no_std]

#[cfg(feature = "std")]
extern crate std;

#[cfg(feature = "std")]
pub mod cli;
#[cfg(feature = "std")]
pub mod logger;

pub mod error;
pub(crate) mod stack;
pub(crate) mod value;
pub(crate) mod variable_table;
mod vm;
```

### Error types

`core::fmt::Display` is available in `no_std`. Gate only `std::error::Error`:

```rust
use core::fmt;

impl fmt::Display for Trap { ... }

#[cfg(feature = "std")]
impl std::error::Error for Trap {}
```

Same pattern for `ContainerError`.

## Phase 3: Cargo Feature Flags

### `ironplc-container/Cargo.toml`

```toml
[features]
default = ["std"]
std = ["alloc"]
alloc = []
```

### `ironplc-vm/Cargo.toml`

```toml
[features]
default = ["std"]
std = ["alloc", "ironplc-container/std", "clap", "env_logger", "time"]
alloc = ["ironplc-container/alloc"]

[dependencies]
ironplc-container = { path = "../container", version = "...", default-features = false }
log = "0.4.20"

# std-only dependencies
clap = { version = "4.0", features = ["derive", "wrap_help"], optional = true }
env_logger = { version = "0.10.0", optional = true }
time = { version = "0.3.17", optional = true }
```

### Binary target

The `[[bin]]` target only compiles when `std` is available. Gate it with a `required-features`:

```toml
[[bin]]
name = "ironplcvm"
path = "bin/main.rs"
required-features = ["std"]
```

## Phase 4: External Dependency Audit

| Dependency | Action |
|---|---|
| `clap` | Make optional, activated by `std` feature |
| `env_logger` | Make optional, activated by `std` feature |
| `log` | Keep as always-on dependency (supports `no_std`) |
| `time` | Make optional, activated by `std` feature |

After gating, `cargo tree --no-default-features` for `ironplc-vm` shows only `ironplc-container` and `log`.

## Arduino Usage Example

```rust
#![no_std]
#![no_main]

use ironplc_container::ContainerRef;
use ironplc_vm::{Slot, Vm};

static PROGRAM: &[u8] = include_bytes!("program.iplc");

#[arduino_hal::entry]
fn main() -> ! {
    let container = ContainerRef::from_slice(PROGRAM).unwrap();

    let mut stack_buf = [Slot::default(); 16];
    let mut var_buf = [Slot::default(); 32];

    let mut vm = Vm::new()
        .load(container, &mut stack_buf, &mut var_buf)
        .start();

    loop {
        vm.run_single_scan().unwrap();
    }
}
```

## Verification

### CI checks

Add to CI:

1. `cargo build -p ironplc-container --no-default-features --target thumbv7em-none-eabihf` — verifies container crate is `no_std`
2. `cargo build -p ironplc-vm --no-default-features --target thumbv7em-none-eabihf` — verifies VM crate is `no_std`
3. Existing `cargo build` and `cargo test` (default features) — verifies no regression

### Test matrix

| Build | Features | Target | What it validates |
|---|---|---|---|
| Default | `std` | host | Existing behavior unchanged |
| `--no-default-features --features alloc` | `alloc` | host | `alloc` tier compiles and tests pass |
| `--no-default-features` | none | `thumbv7em-none-eabihf` | Fully static `no_std` compiles |

## Open Questions

1. **Const generics vs. caller-provided slices** — this plan proposes caller-provided `&mut [Slot]` slices. Const generics (`OperandStack<const N: usize>`) are an alternative that moves the size into the type system but propagate through all type signatures. Which is preferred?

2. **`alloc` tier priority** — the `alloc` feature is defined for completeness but may not be immediately useful. Should it be implemented in the first pass, or deferred until an RTOS target materializes?

3. **Desktop convenience API** — should the `std` build retain the current `Vm::load(Container)` signature as a convenience wrapper, or should all callers switch to the slice-based API?
