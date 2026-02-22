# Typestate VM Lifecycle

status: proposed
date: 2026-02-22

## Context and Problem Statement

The bytecode VM has a linear lifecycle: it is created empty, loaded with a bytecode container, started, and then runs scan cycles in a loop. Different data exists at different lifecycle stages — the container, operand stack, and variable table only exist after loading. How should the Rust implementation enforce that VM operations are called in the correct order?

This decision applies ADR-0005 (safety-first design principle) to the Rust implementation layer, not just the instruction set.

## Decision Drivers

* **PLC programs control physical processes** — calling `run_single_scan()` on an unloaded VM must be impossible, not just an error
* **The scan loop is the hot path** — `run_single_scan()` is called thousands of times per second; the API must not impose overhead or ownership transfer on every call
* **State-dependent data** — `Container`, `OperandStack`, and `VariableTable` only exist after loading; representing them as `Option<T>` introduces `unwrap()` hazards that could panic at runtime
* **Borrow checker interaction** — the interpreter needs simultaneous immutable access to the container (constant pool) and mutable access to the stack and variables; the design must make this natural rather than requiring workarounds
* **Future extensibility** — diagnostic interfaces, online change, and I/O drivers will need to interact with the VM while it is running

## Considered Options

* Runtime state enum — a single `Vm` struct with a `state: VmState` field and runtime checks
* Full typestate — generic type parameter per state (`Vm<Empty>`, `Vm<Ready>`, `Vm<Running>`), consuming `self` on every transition
* Hybrid typestate — distinct types for each lifecycle stage, consuming `self` for setup transitions, `&mut self` for the scan loop

## Decision Outcome

Chosen option: "Hybrid typestate", because it eliminates invalid states at compile time while keeping the scan loop ergonomic and zero-cost.

The lifecycle is encoded as three distinct types:

```
Vm  ──load(self)──>  VmReady  ──start(self)──>  VmRunning
                                                    │
                                          run_single_scan(&mut self)
                                                    │
                                                    ▼
                                              (stays VmRunning)
```

Setup transitions (`load`, `start`) consume `self` and return the next type. The scan loop (`run_single_scan`) takes `&mut self` because the VM remains in the same state on success.

### Consequences

* Good, because invalid state transitions are compile-time errors — you cannot call `run_single_scan()` on a `Vm` or `VmReady`; the method does not exist on those types
* Good, because state-dependent data is stored directly in each type's fields — no `Option` wrappers, no `unwrap()` calls, no runtime panics from missing data
* Good, because the `VmError::InvalidState` variant is eliminated entirely — there is no error type for "wrong state" because the type system prevents it
* Good, because the scan loop uses `&mut self`, which is idiomatic Rust for in-place mutation and allows external code to hold references to the VM
* Good, because the borrow conflict between container (immutable) and stack/variables (mutable) is resolved naturally — `execute()` is a free function that takes split borrows of the `VmRunning` fields
* Good, because the pattern aligns with ADR-0005 (safety-first) — compile-time guarantees are stronger than runtime checks
* Bad, because there is no single "VM in any state" type — code that needs to store a VM at an unknown lifecycle stage (e.g., a management interface) must use an enum wrapper, which reintroduces matching
* Bad, because methods valid across multiple states (e.g., `read_variable` on both `VmReady` and `VmRunning`) must be implemented on each type separately or extracted into a shared trait
* Neutral, because the setup path (`load`, `start`) is called once per VM lifetime, so consuming `self` has no performance impact — LLVM optimizes the move to in-place mutation

### Confirmation

Verify that the following code **does not compile**:

```rust
let vm = Vm::new();
vm.run_single_scan(); // ERROR: no method named `run_single_scan` on `Vm`
```

Verify that the following code **does compile and produces correct results**:

```rust
let mut vm = Vm::new().load(container).start();
vm.run_single_scan().unwrap();
assert_eq!(vm.read_variable(0).unwrap(), 10);
```

## Pros and Cons of the Options

### Runtime State Enum

A single `Vm` struct with `state: VmState`, `container: Option<Container>`, and runtime checks at the start of each method.

* Good, because it is simple to implement and familiar — a single type with runtime guards
* Good, because a single type can be stored anywhere without enum wrappers
* Bad, because `Option<Container>` requires `unwrap()` in every method that uses the container — a logic error causes a panic, not a compile error
* Bad, because `VmError::InvalidState` is a runtime error that can only be caught by tests, not by the compiler
* Bad, because dummy-initialized fields (`OperandStack::new(0)`) waste memory and obscure intent — an empty VM should have no stack, not a zero-sized stack

### Full Typestate

Each state is a type parameter: `Vm<Empty>`, `Vm<Ready>`, `Vm<Running>`. Every method consumes `self` and returns the next state.

* Good, because all transitions are compile-time checked, same as the hybrid approach
* Good, because the `From` trait can express transitions declaratively
* Bad, because `run_single_scan(self) -> Result<Vm<Running>, Vm<Faulted>>` consumes `self` on every scan cycle — the caller must rebind the variable on every iteration
* Bad, because external code cannot hold a `&Vm<Running>` across a `run_single_scan` call, since the value is moved
* Bad, because storing "a VM in any state" requires a wrapper enum (`AnyVm`), which reintroduces runtime matching and negates the typestate benefit at the storage boundary
* Bad, because methods valid across states require either code duplication per `impl Vm<S>` block or a trait with a blanket impl

### Hybrid Typestate (chosen)

Distinct struct types (`Vm`, `VmReady`, `VmRunning`) with consuming `self` for setup and `&mut self` for the hot path.

* Good, because setup transitions (`load`, `start`) consume `self` — the old state cannot be used after transition, preventing stale-data bugs
* Good, because the scan loop (`run_single_scan(&mut self)`) is idiomatic — no ownership transfer, references remain valid, standard loop patterns work
* Good, because each struct contains exactly the fields valid for its state — compile-time data integrity without `Option`
* Bad, because it introduces three types instead of one — slightly more API surface to learn
* Neutral, because the "three types" cost is offset by the API being self-documenting — the type name tells you what operations are available

## More Information

### Why `&mut self` for the scan loop

The scan loop mutates internal state (operand stack, variable table) but does not change the VM's lifecycle state. In Rust, `&mut self` is the idiomatic signature for methods that mutate without changing identity. Consuming `self` is for methods that transform a value into a different type, which does not apply when `VmRunning` stays `VmRunning`.

Concretely, consuming `self` would force this loop pattern:

```rust
let mut vm = vm_running;
loop {
    vm = match vm.run_single_scan() {
        Ok(still_running) => still_running,
        Err(faulted) => break,
    };
}
```

With `&mut self`, the loop is:

```rust
loop {
    if let Err(trap) = vm.run_single_scan() {
        break;
    }
}
```

The second form is shorter, more idiomatic, and allows other code to hold `&vm` references.

### Relationship to ADR-0005

ADR-0005 establishes safety-first as a standing principle for design trade-offs. This ADR applies that principle to the implementation layer: when a Rust type system feature can prevent an error class at compile time (invalid state transitions, missing data), we use it, even at the cost of a more complex API surface (three types instead of one).
