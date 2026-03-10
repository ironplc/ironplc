# no_std VM Design

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

1. **Zero external dependencies in embedded builds** — `ironplc-vm` depends only on `ironplc-container` (no_std); no other crates
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

## Embedded Deployment Model

The embedded deployment does **not** require users to have a local Rust compiler or build toolchain. The application binary contains both the VM and the user's bytecode. At startup, the VM obtains a `&[u8]` reference to the bytecode and parses it via `ContainerRef::from_slice`.

### Embedded usage sketch

Buffer sizes are **known at build time** on embedded because the bytecode is baked into the binary. The programmer sizes arrays to match the compiled program (the IronPLC compiler can emit these constants). `Vm::load` validates that all buffers are large enough and returns an error if not, so an undersized array is caught at startup rather than silently corrupting memory.

```rust
#![no_std]
#![no_main]

use ironplc_container::ContainerRef;
use ironplc_vm::{Slot, Vm};
use ironplc_vm::scheduler::{TaskState, ProgramInstanceState};

// Bytecode baked into flash at compile time.
static PROGRAM: &[u8] = include_bytes!("my_program.iplc");

// Buffer sizes match the compiled program's header values.
// The IronPLC compiler can emit these as constants.
const MAX_STACK: usize = 16;
const NUM_VARS: usize = 32;
const NUM_TASKS: usize = 1;
const NUM_PROGRAMS: usize = 1;
const NUM_CONSTANTS: usize = 2;

#[arduino_hal::entry]
fn main() -> ! {
    // Phase 1: parse container (two-phase for constant offset index).
    let mut const_offsets = [0u32; NUM_CONSTANTS];
    let container = ContainerRef::from_slice(PROGRAM, &mut const_offsets)
        .unwrap();

    // Phase 2: allocate buffers on the stack.
    let mut stack_buf = [Slot::default(); MAX_STACK];
    let mut var_buf = [Slot::default(); NUM_VARS];
    let mut task_states = [TaskState::default(); NUM_TASKS];
    let mut program_instances = [ProgramInstanceState::default(); NUM_PROGRAMS];
    let mut ready_buf = [0usize; NUM_TASKS];

    // Phase 3: load and run. Vm::load populates task_states and
    // program_instances from the container, then validates all sizes.
    let ready = Vm::new()
        .load(
            container,
            &mut stack_buf,
            &mut var_buf,
            &mut task_states,
            &mut program_instances,
            &mut ready_buf,
        )
        .unwrap();

    let mut running = ready.start();

    loop {
        let current_us = read_hardware_timer_us();
        running.run_round(current_us).unwrap();
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

## Implementation Plan

See [Implementation Plan: no_std VM](../plans/no-std-vm-impl.md) for the phased implementation.
