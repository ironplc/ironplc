# Standard Function Blocks as VM Intrinsics via FB_CALL

status: proposed
date: 2026-02-17

## Context and Problem Statement

IEC 61131-3 defines standard function blocks for timers (TON, TOF, TP), counters (CTU, CTD, CTUD), and other common PLC operations. These are used pervasively in PLC programs and interact with runtime services (hardware timers, scan cycle timing). The bytecode instruction set must support calling these.

Should timers, counters, and other standard function blocks be dedicated opcodes in the instruction set, or should they be handled through the general function block call mechanism?

## Decision Drivers

* **Instruction set stability** — adding new standard FB support should not require new opcodes or changes to the VM dispatch table
* **Performance of timer/counter operations** — these execute every scan cycle for every active instance, so they are hot-path operations
* **Standard FB API complexity** — TON has 4 parameters (IN, PT, Q, ET), CTUD has 7 (CU, CD, R, LD, PV, QU, QD); dedicated opcodes would need to encode all of these
* **Future extensibility** — IEC 61131-3 edition 3 added new standard FBs; users may also want custom FB types with similar performance characteristics
* **VM implementation simplicity** — fewer opcode categories means a simpler interpreter loop

## Considered Options

* Dedicated opcodes per standard function block (TIMER_START, TIMER_ELAPSED, COUNTER_INC, etc.)
* Standard function blocks as VM intrinsics recognized at FB_CALL dispatch
* Standard function blocks compiled to ordinary bytecode (no special treatment)

## Decision Outcome

Chosen option: "Standard function blocks as VM intrinsics recognized at FB_CALL dispatch", because it keeps the instruction set clean (no timer/counter opcodes) while still allowing the VM to fast-path these critical operations. The bytecode uses the same FB_CALL instruction for both user-defined and standard function blocks; the VM recognizes standard FB type IDs at dispatch time and routes them to optimized native implementations.

### Consequences

* Good, because the instruction set is stable — adding support for a new standard FB (e.g., R_TRIG, F_TRIG, SR, RS) requires only a new intrinsic handler in the VM, not a new opcode
* Good, because the bytecode for calling TON looks identical to calling any user FB — the compiler doesn't need special codegen paths for standard FBs
* Good, because the VM can still use highly optimized native code for timers (direct hardware timer access, no interpretation overhead for the FB body)
* Good, because user programs that subclass or wrap standard FBs work naturally through the same mechanism
* Bad, because FB_CALL dispatch now has a conditional check: "is this a known intrinsic?" — one extra branch per FB_CALL, including for user-defined FBs that are not intrinsics
* Bad, because the list of recognized intrinsics is a VM implementation detail not visible in the bytecode, which makes bytecode behavior dependent on the VM version

### Confirmation

Verify by implementing TON (on-delay timer) both as an intrinsic and as a bytecode-compiled FB. Confirm that:
1. Both produce identical output behavior for the same inputs
2. The intrinsic version meets scan cycle timing requirements on the target hardware
3. The bytecode uses the same FB_CALL instruction in both cases — only the VM dispatch differs

## Pros and Cons of the Options

### Dedicated Opcodes Per Standard Function Block

TIMER_START, TIMER_ELAPSED, COUNTER_INC, COUNTER_RESET, and similar opcodes for each standard FB operation.

* Good, because dispatch is a single opcode lookup — no conditional "is this an intrinsic?" check
* Good, because the VM handler for each timer/counter operation is a dedicated, inlinable function
* Bad, because IEC 61131-3 has many standard FBs (TON, TOF, TP, CTU, CTD, CTUD, R_TRIG, F_TRIG, SR, RS, and more); each needs multiple opcodes to cover its full API, leading to 20-30+ dedicated opcodes
* Bad, because the standard FB APIs are complex — TON has 4 parameters with specific timing semantics; encoding this in opcodes requires multi-operand instructions or sequences of parameter-loading opcodes, which is just reinventing the FB_CALL protocol
* Bad, because the instruction set must change whenever a new standard FB is added, breaking backward compatibility
* Bad, because the compiler needs special codegen for each standard FB type, increasing compiler complexity

### Standard Function Blocks as VM Intrinsics via FB_CALL

The compiler emits the same FB_CALL sequence for all function blocks. The VM maintains a table mapping standard FB type IDs to native implementations. At FB_CALL dispatch, the VM checks if the callee is a known intrinsic and routes to the native handler if so; otherwise it interprets the FB's bytecode body.

* Good, because the instruction set has exactly one FB invocation mechanism — no special cases
* Good, because intrinsic implementations can access hardware directly (timers, I/O) without the overhead of bytecode interpretation
* Good, because new intrinsics can be added to the VM without changing the instruction set
* Good, because non-intrinsic FBs (user-defined) pay only one extra branch at dispatch — a predictable branch since most calls in a typical program are to the same FB types repeatedly
* Neutral, because the intrinsic table is a VM implementation detail — different VM versions may optimize different FBs, but behavior should be identical
* Bad, because there is no way to guarantee at the bytecode level that a standard FB call will be fast-pathed — it depends on the VM implementation

### Standard Function Blocks Compiled to Ordinary Bytecode

No special treatment. Standard FBs like TON are distributed as pre-compiled bytecode libraries. The VM interprets their bodies like any other FB.

* Good, because the VM is maximally simple — no intrinsic table, no special dispatch logic
* Good, because standard FB behavior is fully defined by bytecode, making it portable and auditable
* Bad, because timer FBs need access to wall-clock time, which requires either a system call mechanism in the bytecode (adding complexity elsewhere) or a special CLOCK opcode (adding a dedicated opcode anyway)
* Bad, because interpreted timer/counter code is significantly slower than native implementation — on a microcontroller, this may push scan times over budget
* Bad, because standard FB bytecode bodies would need to be distributed with every compiled program or bundled into the VM, adding deployment complexity

## More Information

### How intrinsic dispatch works

```
FB_CALL handler:
  fb_type_id = operand
  fb_instance = get_instance(stack)
  if intrinsic_table.contains(fb_type_id):
    intrinsic_table[fb_type_id](fb_instance)  // native call
  else:
    push_frame(fb_instance.code_offset)        // interpret bytecode body
```

The branch predictor learns the pattern quickly because most programs call the same FB types repeatedly. In profiling of typical PLC programs, timer and counter calls account for 5-15% of total instructions, so the fast-path matters.

### Standard FBs that should be intrinsics in the initial implementation

| FB | Parameters | Rationale for intrinsic |
|----|-----------|------------------------|
| TON | IN, PT, Q, ET | Needs hardware timer access |
| TOF | IN, PT, Q, ET | Needs hardware timer access |
| TP | IN, PT, Q, ET | Needs hardware timer access |
| CTU | CU, R, PV, Q, CV | Hot-path, simple logic |
| CTD | CD, LD, PV, Q, CV | Hot-path, simple logic |
| CTUD | CU, CD, R, LD, PV, QU, QD, CV | Hot-path, complex parameter set |
| R_TRIG | CLK, Q | Edge detection, called every scan |
| F_TRIG | CLK, Q | Edge detection, called every scan |

Additional FBs (SR, RS, string functions, etc.) can be added as intrinsics later based on profiling, without instruction set changes.
