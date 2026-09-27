# WebAssembly Target

status: proposed
date: 2026-09-26

## Overview

`ironplcc compile --target wasm` compiles a project to a WebAssembly logic
module of the logic module ABI 1.1 instead of a bytecode container. The
target is optional: the bytecode container, its code generator and the VM do
not change.

The target has two layers. A WebAssembly back end that knows nothing of
IronPLC's front end: an intermediate representation (IR) with its validator
(`ironplc-wasm-ir`), the WebAssembly code generator (`ironplc-wasm-codegen`)
and the symbol map (`ironplc-wasm-symbols`). And an adapter, `ironplc-wasm`
(`compiler/wasm`), which lowers the analysed program to that IR.

```
sources -> parser -> analyzer --+--> codegen (bytecode) --> .iplc   --> ironplcvm
                                |
                                +--> ironplc-wasm (IR) --> wasm-codegen --> .wasm + .symbols.json
                                                                             --> wasmtime, wasmi, browsers
```

The ABI and the symbol map are specified in
`compiler/wasm-symbols/docs/spec/abi.md` and
`compiler/wasm-symbols/docs/spec/symbol-map.md`; the contract of the IR for
a front end is `compiler/wasm-symbols/docs/backend.md`.

## Where the analysed program comes from

The adapter takes the same inputs as the bytecode code generator, so nothing
in the analyzer changes:

| What | Where |
|------|-------|
| Analysed library and context, only when analysis reported nothing | `ironplc_project::analyze` (`compiler/project/src/compile.rs`), shared by both targets |
| Type of every expression | `Expr::resolved_type` (`compiler/dsl/src/textual.rs`) and `TypeEnvironment::representation_of_expr` |
| Layout-independent description of every type | `IntermediateType` (`compiler/analyzer/src/intermediate_type.rs`) |
| Function signatures, including the standard library | `FunctionEnvironment` |
| POUs reachable from the program | `SemanticContext::reachable` |

## Mapping of IronPLC concepts to the IR

| IronPLC | IR |
|---------|----|
| `PROGRAM` instance | one `Static` object per instance and a `Program` function using absolute addresses |
| `FUNCTION_BLOCK` | a `FunctionBlock` function; an instance is one object part (the three-part layout is not needed: IronPLC has no retentive variables in function blocks) |
| `FUNCTION` | a `Function` with a static frame holding inputs, outputs, locals and the result |
| `VAR_GLOBAL` | one `Static` object for the configuration's globals |
| Located variable `%I`, `%Q`, `%M` | objects in the `Input`, `Output`, `Marker` regions, at the byte offset of the address |
| `CONFIGURATION`, `RESOURCE`, `TASK`, `PROGRAM ... WITH` | IR tasks (name, interval, priority, programs in order) |
| No configuration | one task named after the program with the interval of the freewheeling task (0) |
| Standard function blocks (`TON`, `CTU`, `R_TRIG`, ...) | IR function blocks built by the adapter, reproducing `compiler/vm/src/intrinsic.rs` |
| Standard functions | IR operators, conversions and intrinsics (`ABS`, `MIN`, `MAX`, `LIMIT`, `SEL`, `MUX`, shifts, `SQRT`, `LN`, `LOG`, `EXP`, `EXPT`) |
| Arrays | contiguous elements; the element offset is an IR `Index` |
| Structures | members at the offsets of `IntermediateType::Structure` |
| Enumerations | stored as `DINT` ordinals, as the bytecode does |

## Values

| Requirement | IronPLC type | IR storage |
|-------------|--------------|------------|
| **REQ-WT-wasm-010** | `SINT`, `INT`, `DINT`, `LINT`, `USINT`, `UINT`, `UDINT`, `ULINT` | integers of the same width and signedness |
| **REQ-WT-wasm-011** | `BYTE`, `WORD`, `DWORD`, `LWORD` | unsigned integers of the same width |
| **REQ-WT-wasm-012** | `TIME`, `LTIME` | signed 32-bit and 64-bit integers of milliseconds (ADR-0021), not IR durations |
| **REQ-WT-wasm-013** | `DATE`, `TIME_OF_DAY`, `DATE_AND_TIME` and their long forms | unsigned integers of their width in the units of ADR-0025 |

## Semantics

The WebAssembly target reproduces the semantics IronPLC intends, decided in
its ADRs; where the VM departs from them the gate reports a VM defect.

- **REQ-WT-wasm-020** An integer expression whose type is narrower than 32
  bits is computed at 32 bits and wrapped to its type only when it is
  stored, as the bytecode does (ADR-0001, ADR-0002 in its default wrap
  policy): `(s + 50) / 2` with `s : SINT := 100` gives 75.
- **REQ-WT-wasm-021** An arithmetic operator computes at the width of its own
  result type and the result is then converted to its destination, as the
  bytecode code generator does: a `DINT` product stored in a `LINT` wraps at
  32 bits, then widens.
- **REQ-WT-wasm-022** Integer division or `MOD` by zero traps with the ABI
  code 2, as the VM traps.
- **REQ-WT-wasm-023** A conversion from a real to an integer truncates toward
  zero and saturates, and `NaN` gives 0: the IR `Truncate` mode, which is
  what the VM computes. The standard asks for rounding (issue #1813); when
  IronPLC decides, the target follows by using the IR `Round` mode.
- **REQ-WT-wasm-024** `FOR` assigns the start value, tests
  `control <= end` (`>=` for a negative step) before each iteration with the
  end expression evaluated at each test, and adds the step with the
  wrap-around of the control variable's type, as the bytecode does. A loop
  whose end is the largest value of its type therefore does not end.
- **REQ-WT-wasm-025** The timers measure elapsed time as the VM does: the
  start is kept in microseconds (`now_ns / 1000`), the elapsed time is
  `(now - start) / 1000` milliseconds, truncated; the counters saturate at
  the bounds of `INT`.

Where the IR and IronPLC differ, and which side the target follows:

| Point | IronPLC | IR default | Target |
|-------|---------|------------|--------|
| Narrow integer arithmetic | 32-bit, wrap at store | wrap after every operation | IronPLC: operands widened to 32 bits |
| Real to integer | truncate, saturate | `Round` or `Truncate` per call | `Truncate` |
| Implicit integer to real (`r := i`) | bit pattern reused (issue #1812, VM defect) | explicit `Float` conversion | the value, converted |
| `TIME` | 32-bit milliseconds | 64-bit nanoseconds | 32-bit milliseconds as an integer |
| Cycle time | microseconds given to `run_round` | `now_ns` import | microseconds from `now_ns / 1000` |
| Array index out of bounds | trap | trap with code 3 | trap with code 3 |

## Symbol map

- **REQ-WT-wasm-030** Leaves are named by SYM-010: `MAIN.X` for a variable of
  the program instance `MAIN`, `MAIN.T1.Q` for a member of a function block
  instance, the name alone for a global variable. The hidden state of the
  standard blocks is not listed (SYM-012).
- **REQ-WT-wasm-031** An array of an elementary type is one leaf with its
  dimensions and stride.
- **REQ-WT-wasm-032** A located variable is a leaf with its address and lies
  in the region of its prefix.
- **REQ-WT-wasm-033** With debug hooks, each statement is preceded by a call
  of `plc_rt.debug_hook` with the site of the statement in the IronPLC
  source; traps of divisions, array bounds and dereferences carry the site
  of their expression.

## Command line

The command line is tested in `compiler/ironplc-cli/src/wasm.rs`.

- `ironplcc compile --target wasm -o out.wasm` writes the
  module to `out.wasm` and its symbol map as JSON to `out.symbols.json`; the
  module also carries the map in its `plc.meta` section.
- `--wasm-fuel`, `--wasm-debug-hooks` and
  `--wasm-no-bounds-checks` select the instrumentation of the code
  generator.
- **REQ-WT-wasm-042** A construct the adapter does not lower is reported as
  P9999 (not implemented) at its location, and no module is written.
- **REQ-WT-wasm-043** Every IR module is checked with the IR validator before
  generation; a failure is an internal error (P9998), never a module.

## Gate

The differential test of `compiler/wasm/tests` compiles each program of the
bytecode code generator's end-to-end tests, of `compiler/resources/test`, of
`examples/` and of an optional directory of probe programs to both targets, runs the
container on the in-tree VM and the module on wasmtime and wasmi with the
same virtual clock, and compares the top-level variables of the program
after every cycle. A difference is either an adapter defect (fixed), a VM
defect (reported with a minimal program), or a defect of the back end
(fixed in the back end crates).
