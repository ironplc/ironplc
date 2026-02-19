# Stack-Based Bytecode VM for PLC Program Execution

status: proposed
date: 2026-02-17

## Context and Problem Statement

IronPLC compiles IEC 61131-3 programs (Structured Text and other languages) and needs a runtime execution strategy. The compiler currently performs parsing and semantic analysis but does not execute programs. To support a virtual PLC runtime — running PLC programs on general-purpose hardware for simulation, testing, and soft-PLC deployment — we need to choose an execution model.

How should IronPLC execute compiled PLC programs?

## Decision Drivers

* **Portability** — the runtime must work on x86, ARM, and potentially microcontrollers without per-target compilation effort from users
* **Deterministic timing** — PLC programs run in scan cycles with hard or soft real-time constraints; execution time must be predictable and bounded
* **Inspection and debugging** — users need to observe variable values, set breakpoints, and single-step through PLC logic during development and commissioning
* **Safety and isolation** — PLC programs should not be able to corrupt the runtime, access arbitrary memory, or crash the host system
* **Startup latency** — a PLC should begin executing within milliseconds of program load, matching the behavior of hardware PLCs
* **Implementation complexity** — the execution engine must be implementable and maintainable by a small team, written in Rust to match the existing compiler

## Considered Options

* Bytecode virtual machine (stack-based)
* Bytecode virtual machine (register-based)
* Ahead-of-time compilation to native code
* Tree-walking interpreter over the AST
* Transpilation to C and host compilation

## Decision Outcome

Chosen option: "Bytecode virtual machine (stack-based)", because it provides the best balance of portability, safety, debuggability, and implementation simplicity for PLC program execution. The same compiled bytecode runs on any platform with the VM, programs are sandboxed by construction, and the interpreter loop provides natural points for breakpoints, watchpoints, and scan-cycle timing.

### Consequences

* Good, because compiled bytecode is portable — compile once, run on any platform with the VM
* Good, because the VM provides a natural sandbox — programs can only access memory through VM-mediated instructions, preventing corruption of the host
* Good, because every instruction dispatch is a point where the VM can check for breakpoints, variable watches, and scan-cycle time limits
* Good, because startup is instant — loading bytecode into the VM is a memcpy, not a compilation step
* Good, because the implementation is straightforward Rust — a match-based dispatch loop with no platform-specific code generation
* Bad, because interpreted bytecode is 10-50x slower than native code, which limits the minimum achievable scan cycle time on constrained hardware
* Bad, because stack-based bytecode produces more instructions than register-based for the same program (more push/pop traffic), trading code density for implementation simplicity

### Confirmation

Build a prototype VM that can execute a simple PLC program (a few timers, counters, and arithmetic) and measure:
1. Instructions per scan cycle for a representative program
2. Scan cycle time on target hardware (x86 desktop, ARM SBC)
3. That the performance ceiling is acceptable for the intended use cases (simulation, soft-PLC, testing) — target: 1ms scan cycle for programs under 10,000 instructions

If performance is insufficient, the bytecode design (ADR-0001 through ADR-0003) is compatible with adding a JIT compiler later — the typed instruction set provides the type information a JIT needs.

## Pros and Cons of the Options

### Bytecode Virtual Machine (Stack-Based)

Compile IEC 61131-3 to a custom bytecode instruction set. Execute with an interpreter loop that maintains an operand stack, a call stack, and variable memory. Instructions push/pop values from the operand stack.

Examples: JVM, CPython, Lua 5.0, WebAssembly (conceptually stack-based).

* Good, because compiler codegen is simple — expressions map directly to stack operations without register allocation
* Good, because the bytecode format is compact — no register operands in most instructions, just an opcode byte
* Good, because the interpreter loop is simple to implement — a single `match` statement, no register file management
* Good, because the stack discipline makes bytecode verification straightforward — track stack depth at each point, verify type consistency
* Good, because the architecture is well-understood — extensive literature and reference implementations exist
* Neutral, because performance is adequate for soft-PLC use cases (simulation, testing, development) but not for replacing hardware PLCs on tight cycle times
* Bad, because stack operations create more memory traffic than register-based — `a + b` requires 2 loads, 1 add, 1 store on a register machine but LOAD, LOAD, ADD (3 stack pops, 2 pushes, 1 pop) on a stack machine
* Bad, because lack of register allocation means the VM cannot exploit CPU registers effectively — the operand stack lives in memory (or a small register-cached window)

### Bytecode Virtual Machine (Register-Based)

Compile to bytecode where instructions reference virtual registers (e.g., `ADD r0, r1, r2`). Execute with an interpreter that maintains a register file (array of values).

Examples: Lua 5.1+, Dalvik (Android), LuaJIT bytecode.

* Good, because fewer instructions per operation — `ADD r0, r1, r2` replaces LOAD+LOAD+ADD+STORE
* Good, because research shows 25-45% fewer executed instructions than stack-based for equivalent programs (Yunhe Shi et al., 2008)
* Good, because the register file can map to CPU registers in a JIT compiler more naturally
* Bad, because instructions are wider — each instruction encodes 2-3 register operands, increasing bytecode size
* Bad, because compiler codegen is significantly more complex — requires register allocation (graph coloring or linear scan), which is a substantial implementation effort
* Bad, because bytecode verification is harder — must track liveness and types across a register file rather than a stack with simple depth tracking
* Bad, because the performance advantage over stack-based diminishes for PLC programs, which are typically simple sequential logic with few complex expressions and many I/O operations

### Ahead-of-Time Compilation to Native Code

Compile IEC 61131-3 directly to machine code (x86, ARM) using LLVM or Cranelift as a backend.

Examples: CODESYS (compiles to native), Rust/C/C++ (via LLVM), GCC-based PLC compilers.

* Good, because native execution is 10-50x faster than interpretation — enables microsecond-level scan cycles
* Good, because the compiled code can exploit CPU features (SIMD, branch prediction, register allocation) automatically
* Good, because CODESYS uses this approach, validating it for PLC use cases
* Bad, because portability requires per-target compilation — users must compile for their specific CPU architecture, or the runtime must bundle a compiler (LLVM is ~30MB)
* Bad, because startup requires compilation, adding seconds of latency before the first scan cycle
* Bad, because debugging native code is significantly harder — breakpoints require platform-specific debug APIs (ptrace on Linux, platform-specific on embedded), variable inspection requires DWARF debug info parsing
* Bad, because native code is not sandboxed — a bug in code generation can produce memory corruption, segfaults, or security vulnerabilities
* Bad, because LLVM/Cranelift is a large dependency that increases build complexity and binary size, and Cranelift's API is unstable
* Bad, because implementing a correct native code generator is a major engineering effort — register allocation, instruction selection, calling conventions, ABI compliance per platform

### Tree-Walking Interpreter Over the AST

Execute the program by directly walking the parsed AST. Each AST node has an `eval()` method that recursively evaluates sub-expressions.

Examples: early Ruby, many scripting language prototypes, some Lisp interpreters.

* Good, because implementation is trivial — add `eval()` methods to existing AST nodes
* Good, because debugging is natural — the interpreter is always at a known AST node with full source context
* Good, because no intermediate representation needed — parse and execute directly
* Bad, because it is the slowest execution strategy — 50-200x slower than native, 5-10x slower than bytecode, due to pointer chasing through heap-allocated AST nodes, virtual dispatch per node, and no data locality
* Bad, because AST nodes are heap-allocated with pointers — poor cache locality, high memory overhead per instruction
* Bad, because scan cycle determinism is poor — execution time varies with AST depth and node types in ways that are hard to bound
* Bad, because the AST representation carries parsing artifacts (source locations, formatting) that waste memory during execution
* Bad, because the IronPLC AST is designed for analysis, not execution — it would need significant restructuring to serve as an efficient execution representation

### Transpilation to C and Host Compilation

Generate C code from IEC 61131-3 programs, then compile with the host's C compiler (gcc/clang).

Examples: MATIEC (open source IEC 61131-3 to C compiler), some commercial PLC toolchains.

* Good, because generated C code runs at native speed with mature compiler optimizations
* Good, because C compilers are available for virtually every platform, including embedded targets
* Good, because the generated C code is human-readable and auditable, which matters for safety certification
* Good, because MATIEC validates this approach for IEC 61131-3 specifically
* Bad, because it requires a C compiler on the target system — not available on all deployment targets, adds a large external dependency
* Bad, because compilation latency is seconds to minutes depending on program size — unacceptable for rapid iteration during PLC development
* Bad, because debugging maps through two layers of translation (ST → C → native) — source-level debugging requires maintaining mappings through both
* Bad, because the generated C code must manage PLC-specific runtime concerns (scan cycle, process image, timer management) in C, which is error-prone and requires a C runtime library
* Bad, because each change requires recompilation through the C toolchain — slow feedback loop compared to bytecode reload

## More Information

### Why stack-based over register-based

The register-based option offers better steady-state performance (~25-45% fewer instructions). However, for this project:

1. **PLC programs are small** — typical programs are hundreds to low thousands of lines. The absolute time difference between stack-based and register-based execution is microseconds per scan cycle, well within the 1ms target.

2. **Compiler complexity matters** — the IronPLC compiler is maintained by a small team. Register allocation is a significant implementation and maintenance burden. Stack-based codegen is nearly mechanical (walk the expression tree, emit push/pop).

3. **JIT upgrade path** — if performance becomes a bottleneck, adding a JIT compiler to a stack-based VM is well-understood (see HotSpot, V8's early architecture). The typed bytecode from ADR-0001 provides the type information a JIT needs. A register-based bytecode doesn't make JIT compilation significantly easier.

4. **Verification** — the typed stack-based bytecode from ADR-0001 enables static verification by tracking stack depth and types at each instruction. Register-based verification requires dataflow analysis.

### Performance budget

For context, a 1ms scan cycle on a 1 GHz ARM Cortex-A53 (Raspberry Pi class) gives roughly 1,000,000 clock cycles per scan. A bytecode interpreter executes approximately one instruction per 10-20 cycles (including dispatch overhead). This gives a budget of 50,000-100,000 bytecode instructions per scan cycle — far more than typical PLC programs require.

The performance ceiling becomes a concern only for:
- Very complex programs (unusual for PLCs)
- Very fast scan cycles (< 100μs, which requires specialized hardware regardless)
- Microcontrollers with slow clocks (< 100 MHz, where a JIT or native compilation may be needed)

### Relationship to subsequent ADRs

This decision establishes that we are building a bytecode VM. The following ADRs define the bytecode instruction set design:
- [ADR-0001](0001-bytecode-integer-arithmetic-type-strategy.md): How integer types map to bytecode arithmetic operations
- [ADR-0002](0002-bytecode-overflow-behavior.md): How integer overflow is handled at narrowing points
- [ADR-0003](0003-plc-standard-function-blocks-as-intrinsics.md): How standard function blocks (timers, counters) are invoked
