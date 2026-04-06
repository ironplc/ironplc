# Plan: Add DUP and SWAP Opcodes

## Context

The IronPLC bytecode VM currently has no stack-manipulation opcodes beyond POP.
Adding DUP and SWAP enables future peephole optimizations that eliminate
redundant variable loads.

## Steps

1. **Opcode constants** (`container/src/opcode.rs`) — Add `DUP = 0xA1` and `SWAP = 0xA2` after POP.
2. **OperandStack methods** (`vm/src/stack.rs`) — Add `dup()` and `swap()` methods with unit tests.
3. **VM dispatch** (`vm/src/vm.rs`) — Add dispatch cases after the POP arm.
4. **Emitter methods** (`codegen/src/emit.rs`) — `emit_push_op!(emit_dup, ...)` and `emit_unaryop!(emit_swap, ...)`.
5. **Disassembler** (`plc2x/src/disassemble.rs`) — Add DUP and SWAP decode cases.
6. **Integration tests** (`vm/tests/execute_dup_swap.rs`) — End-to-end tests for both opcodes.

## Verification

`cd compiler && just` — all checks must pass.
