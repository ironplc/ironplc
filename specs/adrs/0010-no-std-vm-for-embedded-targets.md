# no_std VM for Embedded Targets

status: proposed
date: 2026-02-25

## Context and Problem Statement

The IronPLC bytecode VM is designed to execute PLC programs compiled from IEC 61131-3 source. PLCs are inherently embedded devices, and a natural deployment target is microcontroller hardware such as Arduino boards. Rust can target these platforms, but only if the code compiles without the Rust standard library (`no_std`), since `std` assumes an operating system with a heap allocator, filesystem, and I/O abstractions that do not exist on bare-metal hardware.

Today, both the `ironplc-vm` and `ironplc-container` crates depend on `std`. Should the VM be refactored to support `no_std`, and if so, how should the crate boundaries, feature flags, and data structures change?

## Decision Drivers

* **PLC programs run on embedded hardware** — the primary use case for a PLC runtime is bare-metal or RTOS environments, not desktop machines
* **Arduino boards are a concrete near-term target** — Arduino Mega (AVR, 8 KB SRAM) and Arduino Due (ARM Cortex-M3, 96 KB SRAM) represent realistic deployment scenarios
* **The VM execution core is already nearly `no_std`-clean** — `vm.rs` and `value.rs` have zero `std` imports today; the `std` dependency is concentrated in I/O, logging, and CLI code
* **Compilation happens on the host, execution happens on the target** — the compiler (`plc2x`) always runs on a full OS; only the VM needs to be embeddable
* **The container format is already designed for static allocation** — the header declares `max_stack_depth`, `num_variables`, and `num_functions` up front, enabling fixed-size allocation without a heap

## Considered Options

* Status quo — keep `std` dependency, target only desktop/server environments
* `no_std` with `alloc` — use `#![no_std]` but require a global allocator for `Vec`-based storage
* `no_std` without `alloc` (fully static) — replace all heap allocation with fixed-size arrays and borrowed slices, enabling deployment on targets with no allocator

## Decision Outcome

Chosen option: "`no_std` without `alloc` (fully static)", because it maximizes the range of deployable targets (including AVR with 8 KB SRAM) and aligns with the PLC philosophy of deterministic, bounded resource usage. The `alloc` and `std` tiers are preserved as Cargo feature flags for convenience on platforms that support them.

### Implementation Plan

#### Phase 1: Split the `ironplc-container` crate into read and write concerns

The container crate currently serves two roles: building/writing containers (compiler side) and reading/interpreting containers (VM side). These have different `std` requirements.

**Step 1a.** Add a `from_slice(&[u8]) -> Result<ContainerRef, ContainerError>` constructor to the container crate that parses a container from a byte slice without any `std::io` or heap allocation. The resulting `ContainerRef` type borrows the input slice and provides accessors that return sub-slices for the constant pool, code section, and function directory. This is the only path the embedded VM needs.

On an Arduino, the bytecode would be embedded in flash at compile time:

```rust
static PROGRAM: &[u8] = include_bytes!("program.iplc");

fn main() -> ! {
    let container = ContainerRef::from_slice(PROGRAM).unwrap();
    let mut vm = Vm::new().load(container).start();
    loop {
        vm.run_single_scan().unwrap();
    }
}
```

**Step 1b.** Gate the existing `read_from(impl Read)`, `write_to(impl Write)`, `ContainerBuilder`, and the `Io(io::Error)` error variant behind `#[cfg(feature = "std")]`. These are only needed by the compiler and the desktop CLI.

#### Phase 2: Make the VM execution core `no_std`

**Step 2a.** Add `#![no_std]` to `ironplc-vm/src/lib.rs`. Gate the CLI-only modules (`cli`, `logger`) and the binary target behind `#[cfg(feature = "std")]`.

**Step 2b.** In `error.rs`, replace `impl std::error::Error for Trap` with a conditional compilation gate:

```rust
// core::fmt::Display is available in no_std
impl core::fmt::Display for Trap { ... }

#[cfg(feature = "std")]
impl std::error::Error for Trap {}
```

**Step 2c.** Replace `Vec<Slot>` in `OperandStack` and `VariableTable` with fixed-size arrays. The container header already provides `max_stack_depth` and `num_variables`, so the sizes are known at load time. On embedded targets, these become stack-allocated or statically-allocated arrays. The concrete approach:

```rust
pub struct OperandStack<const N: usize> {
    data: [Slot; N],
    len: usize,
}
```

Alternatively, if const generics create ergonomic issues at call sites, use a runtime-sized backing store behind a trait or accept a `&mut [Slot]` slice provided by the caller (the Arduino `main` function would allocate it on the stack or in a static).

**Step 2d.** Update `vm.rs` to work with `ContainerRef` (borrowed slices) instead of the owned `Container`. The execution loop already operates on `&[u8]` bytecode slices, so this is a small change to how the container is threaded through the typestate lifecycle.

#### Phase 3: Cargo feature flags

Define the feature hierarchy in both crates:

```toml
[features]
default = ["std"]
std = ["alloc"]     # Full std: File I/O, CLI, logging, std::error::Error
alloc = []          # Heap allocation (Vec) but no OS — for RTOS targets
                    # No features = fully static, no allocator required
```

The `std` feature implies `alloc`. Code that only needs `Vec` uses `#[cfg(feature = "alloc")]`. Code that needs `File`, `io::Read`, or `clap` uses `#[cfg(feature = "std")]`. The default is `std` so existing users see no change.

#### Phase 4: External dependency audit

| Dependency | Used by | `no_std` compatible? | Action |
|---|---|---|---|
| `clap` | `bin/main.rs` | No | Gate behind `std` feature; not needed on embedded |
| `env_logger` | `logger.rs` | No | Gate behind `std` feature; not needed on embedded |
| `log` | `logger.rs` | Yes (`no_std` supported) | Keep, but gate the logger initialization behind `std` |
| `time` | `logger.rs` | No | Gate behind `std` feature; only used for log timestamps |
| `ironplc-container` | `vm.rs` | After Phase 1, yes | Use `default-features = false` in the VM's `Cargo.toml` for embedded builds |

After gating, the embedded build of `ironplc-vm` has **zero external dependencies** beyond `ironplc-container` (itself `no_std`).

### Consequences

* Good, because the VM can run on Arduino and other bare-metal targets without modification — the same crate compiles for both desktop and embedded
* Good, because the execution core becomes truly zero-allocation — deterministic memory usage with no heap fragmentation, which aligns with PLC safety requirements
* Good, because the `include_bytes!` + `ContainerRef::from_slice` pattern is zero-copy — bytecode stays in flash, no RAM duplication
* Good, because existing desktop users see no change — `default = ["std"]` preserves current behavior
* Good, because the container header already declares all sizes up front — the design was already implicitly targeting static allocation
* Bad, because const generic or slice-based storage adds API complexity — `OperandStack<N>` is less simple than `OperandStack` with an internal `Vec`
* Bad, because the VM gains two ways to load a container (`from_slice` for embedded, `read_from` for desktop) — but these serve genuinely different use cases
* Bad, because `#[cfg(feature = ...)]` gates throughout the code increase maintenance burden and make it easier to accidentally break one configuration
* Neutral, because `clap` does not need a `no_std` replacement — it simply does not apply on embedded targets where there is no command line

### Confirmation

The implementation is confirmed when:

1. `cargo build --no-default-features --target thumbv7em-none-eabihf` succeeds for both `ironplc-container` and `ironplc-vm`
2. A minimal Arduino Due example using `include_bytes!` loads a container from flash and executes one scan cycle
3. `cargo build` (with default features) still succeeds and all existing tests pass
4. The embedded build produces no linker errors for `std` symbols

## Pros and Cons of the Options

### Status Quo

Keep the `std` dependency and only target desktop/server environments.

* Good, because no refactoring work is needed
* Good, because the code remains simpler without conditional compilation
* Bad, because the VM cannot run on the hardware that PLCs actually target — the primary use case is unserved
* Bad, because it contradicts the project's mission of being a PLC toolchain — a PLC runtime that only runs on desktops is incomplete

### `no_std` with `alloc`

Use `#![no_std]` but require a global allocator. Replace `std::vec::Vec` with `alloc::vec::Vec`, `std::string::String` with `alloc::string::String`, etc.

* Good, because it is the smallest code change — mostly import path substitutions
* Good, because it enables deployment on RTOS targets that provide an allocator (e.g., FreeRTOS, Zephyr)
* Bad, because AVR Arduino boards (8 KB SRAM) do not have practical allocator support — the most constrained targets are still excluded
* Bad, because `Vec` growth during execution is non-deterministic — a PLC runtime should have bounded, predictable memory usage
* Bad, because heap fragmentation in long-running PLC programs can cause unpredictable allocation failures

### `no_std` without `alloc` (chosen)

Replace all heap allocation with fixed-size arrays and borrowed slices.

* Good, because it targets the widest range of hardware — from AVR (8 KB) to ARM Cortex-M (96+ KB) to desktop
* Good, because memory usage is fully deterministic — all sizes are declared in the container header and allocated once at startup
* Good, because zero-copy container parsing (`from_slice`) is the most efficient approach for flash-resident bytecode
* Good, because it aligns with PLC industry practice — real PLCs do not use dynamic allocation at runtime
* Bad, because the implementation requires more invasive refactoring than the `alloc` option
* Bad, because fixed-size arrays may need const generics or caller-provided buffers, adding API complexity

## More Information

### Memory Budget on Target Hardware

| Board | MCU | SRAM | Flash | Notes |
|---|---|---|---|---|
| Arduino Uno | ATmega328P | 2 KB | 32 KB | Too constrained for most PLC programs |
| Arduino Mega | ATmega2560 | 8 KB | 256 KB | Viable for small programs |
| Arduino Due | ATSAM3X8E (Cortex-M3) | 96 KB | 512 KB | Comfortable for moderate programs |
| STM32 Nucleo-F446RE | Cortex-M4 | 128 KB | 512 KB | Typical industrial target |

A minimal VM instance needs: header (256 bytes) + stack (`max_stack_depth * 8` bytes) + variables (`num_variables * 8` bytes) + bytecode (in flash, zero RAM). For a small program with stack depth 16 and 32 variables, that is 256 + 128 + 256 = 640 bytes of RAM — well within Arduino Mega's 8 KB budget.

### Relationship to Other ADRs

* **ADR-0000 (Stack-based bytecode VM)** — the stack-based architecture is particularly well-suited for `no_std` because the operand stack has a known maximum depth
* **ADR-0005 (Safety-first design)** — static allocation is safer than dynamic allocation for PLC runtimes; bounded resource usage prevents runtime allocation failures
* **ADR-0009 (Typestate VM lifecycle)** — the typestate pattern works unchanged in `no_std`; the generic parameters may need to carry const generic sizes for the stack and variable table
