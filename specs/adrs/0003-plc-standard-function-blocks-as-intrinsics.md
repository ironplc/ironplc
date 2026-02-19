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
* **User-defined overrides of standard FBs** — IEC 61131-3 edition 3 introduced OOP features (`EXTENDS`, `OVERRIDE`) that allow users to derive from standard function blocks and override their behavior; the execution model must correctly handle derived FBs without silently applying the base intrinsic

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
* Good, because user programs that wrap standard FBs via composition work naturally through the same mechanism
* Good, because derived FBs (via EXTENDS) get distinct type IDs from the compiler, so they are never incorrectly matched to the base intrinsic — they fall through to bytecode interpretation automatically
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

### Interaction with FB inheritance (EXTENDS / OVERRIDE)

IEC 61131-3 edition 3 introduced object-oriented features that allow users to derive from function blocks:

```iecst
FUNCTION_BLOCK FB_DebugTON EXTENDS TON
VAR
  callCount : UDINT;
END_VAR
```

In environments like CODESYS and TwinCAT, users can use `EXTENDS` to inherit from a base FB and `OVERRIDE` to replace method behavior. Some runtimes mark standard FBs as `FINAL` to prevent this, but the language itself permits it, and not all runtimes restrict it.

This creates a correctness risk for intrinsic dispatch: if the VM sees a call to an FB that extends TON and routes it to the native TON intrinsic, the user's overridden behavior is silently skipped. This would be a serious bug — the program appears to work but the custom logic never executes.

**The solution is that intrinsic matching is by exact type ID, not by inheritance.**

The compiler assigns each FB type (including derived types) a unique type ID. The intrinsic table in the VM maps specific, well-known type IDs to native implementations:

```
intrinsic_table = {
  TYPE_ID_TON  => native_ton_handler,
  TYPE_ID_TOF  => native_tof_handler,
  TYPE_ID_TP   => native_tp_handler,
  ...
}
```

When `FB_DebugTON EXTENDS TON` is compiled, the compiler assigns it a new type ID (e.g., `TYPE_ID_FB_DEBUGTON`) that is distinct from `TYPE_ID_TON`. The FB_CALL instruction carries this derived type ID. At dispatch, the VM looks up `TYPE_ID_FB_DEBUGTON` in the intrinsic table, finds no match, and falls through to interpreting the FB's bytecode body — which includes the user's overridden behavior.

This means:

| Call target | Type ID | Intrinsic match? | Execution path |
|-------------|---------|-------------------|---------------|
| `TON` (standard) | `TYPE_ID_TON` | Yes | Native intrinsic |
| `FB_DebugTON EXTENDS TON` | `TYPE_ID_FB_DEBUGTON` | No | Bytecode interpretation |
| `FB_WrappedTON` (composition, not extends) | `TYPE_ID_FB_WRAPPEDTON` | No | Bytecode interpretation (internally calls TON, which hits the intrinsic) |

The correctness invariant is: **only exact type matches receive intrinsic treatment.** The compiler enforces this by never reusing a standard FB's type ID for a derived type. No runtime inheritance check is needed.

#### Derived FBs that don't override behavior

A subtlety: `FB_DebugTON EXTENDS TON` might only *add* variables without overriding any behavior. In that case, the base TON logic is correct, and the intrinsic *could* safely execute. However, the VM cannot know this without inspecting the derived FB's bytecode — which defeats the purpose of fast-path dispatch. The simple rule (exact match only) sacrifices this optimization opportunity in favor of correctness and simplicity. If profiling shows this pattern is common, a future optimization could let the compiler annotate derived FBs that are "intrinsic-compatible" (no overrides), but this is not needed for the initial implementation.

#### Name shadowing

A user could also define a completely new FB named `TON` that shadows the standard library's `TON`. The compiler's namespace resolution handles this — the user's `TON` gets a different type ID in their project namespace, so it does not match the intrinsic table entry for the standard `TON`. This is correct: the user explicitly replaced the standard FB and should get their custom implementation.
