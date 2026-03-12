# Flat Variable Table for Function Calls

status: proposed
date: 2026-03-12

## Context and Problem Statement

User-defined IEC 61131-3 functions need local variables (parameters and temporaries) during execution. The VM uses a single variable table shared across the program. How should function locals be allocated: as dedicated regions in the shared variable table (flat), or via dynamic stack frames pushed/popped at each call?

## Decision Drivers

* **IEC 61131-3 prohibits recursion** — directly or indirectly recursive function calls are forbidden by the standard, so each function has at most one activation at a time
* **VM simplicity** — the VM targets embedded (no_std) environments where dynamic allocation is undesirable
* **Performance** — function calls are common in scan cycle code and should be fast
* **Existing infrastructure** — the VM already has `VariableScope` for scoped access to variable table regions

## Considered Options

* Flat variable table — each function gets a statically-assigned region of the shared variable table
* Stack frame isolation — save/restore a variable base pointer on each call, with dynamic frame allocation

## Decision Outcome

Chosen option: "Flat variable table", because IEC 61131-3 prohibits recursion, guaranteeing each function has exactly one activation. Static allocation eliminates dynamic frame management, keeps the VM simple, and reuses the existing `VariableScope` mechanism. The compiler assigns each function a region of the variable table at compile time; the `CALL` opcode simply adjusts the scope to the pre-assigned region.

### Consequences

* Good, because no dynamic allocation or stack frame save/restore is needed in the VM
* Good, because the existing `VariableScope` mechanism handles scoped variable access without changes
* Good, because variable slot assignment is a compile-time decision, keeping the VM simple
* Bad, because the total variable table size must account for all function locals, even if not all functions are active simultaneously — but IEC 61131-3 programs are typically small enough that this is not a concern
* Neutral, because if recursion support were ever needed (non-standard extension), this design would need to be revisited — but the standard explicitly forbids it
