# Bytecode Test Strategy

status: proposed
date: 2026-03-09

## Context and Problem Statement

The IronPLC test suite has grown to include three categories of tests for bytecode operations:

1. **VM bytecode tests** (`compiler/vm/tests/execute_*.rs`) — hand-assemble bytecodes with hardcoded hex bytes, load into the VM, execute, and assert results
2. **Compile bytecode tests** (`compiler/codegen/tests/compile_*.rs`) — compile IEC 61131-3 source and assert the exact bytecode byte sequence produced
3. **End-to-end tests** (`compiler/codegen/tests/end_to_end_*.rs`) — compile IEC 61131-3 source, execute in the VM, and assert variable values

Many operations are tested at all three levels, with significant overlap. For example, `execute_sub_i32.rs` tests that `SUB_I32` produces the right answer for basic inputs like `10 - 3 = 7`, while `end_to_end_sub.rs` tests the same scenario via `result := 10 - 3`.

Upcoming VM performance work (superinstructions, opcode reorganization, VM API changes) will modify bytecode encodings and VM internals. Hand-assembled bytecode tests are expensive to maintain through such changes — each test manually encodes opcode bytes, operand bytes, and the setup/execute/assert boilerplate. The more tests that depend on specific bytecode encodings, the larger the blast radius of VM changes.

Which test category should be used when, and what is the role of hardcoded bytecode bytes?

## Decision Drivers

* **Backwards compatibility** — changing opcode byte values breaks compatibility with previously compiled bytecode containers; tests must catch unintended changes
* **Hardcoded bytes catch silent compatibility breaks** — if tests use symbolic opcode constants (e.g., `opcode::ADD_I32`) and someone changes the constant's value, the tests still pass but compatibility is broken; hardcoded bytes (`0x30`) catch this
* **Maintenance cost during VM evolution** — hand-assembled bytecode tests require updating every hex byte when encodings change; end-to-end tests are immune to encoding changes
* **Test abstraction level** — tests should be written at the highest abstraction level that covers the behavior being verified; lower-level tests should only exist when higher-level tests cannot cover the scenario
* **Edge cases and error paths** — some VM behaviors (stack underflow traps, invalid opcode traps, overflow wrapping semantics) cannot be triggered through valid IEC 61131-3 source code

## Considered Options

* Test everything at all three levels (current state)
* Test correctness via end-to-end, compatibility via compile tests, VM edge cases via bytecode tests
* Test only via end-to-end, with no bytecode-level tests

## Decision Outcome

Chosen option: "Test correctness via end-to-end, compatibility via compile tests, VM edge cases via bytecode tests", because it assigns each test category a clear, non-overlapping purpose and minimizes the number of tests that depend on specific bytecode encodings.

### Test categories and their purposes

**End-to-end tests** (`end_to_end_*.rs`) verify **correctness** — that a given IEC 61131-3 program produces the expected results. They exercise the full pipeline (parse → compile → execute) and are completely immune to bytecode encoding changes. Every operation that can be expressed in IEC 61131-3 source should have end-to-end test coverage.

**Compile bytecode tests** (`compile_*.rs`) verify **backwards compatibility** — that the compiler produces the exact same bytecode byte sequence for a given input. These tests use **hardcoded hex bytes**, not symbolic opcode constants, because the purpose is to detect when byte values change. If an opcode is renumbered, these tests must fail. These tests are the single source of truth for the bytecode encoding contract.

**VM bytecode tests** (`execute_*.rs`) verify **VM-specific edge cases** that cannot be triggered through valid IEC 61131-3 source:
- Stack underflow / overflow traps
- Invalid opcode or invalid builtin function ID traps
- Division by zero traps and negative exponent traps
- Integer overflow wrapping semantics (e.g., `i32::MAX + 1` wrapping to `i32::MIN`)
- Other error paths that require malformed or edge-case bytecode

These tests necessarily use hardcoded bytecode bytes. Because their purpose is narrow (edge cases only), there are relatively few of them, keeping the maintenance cost manageable.

### What NOT to write as a VM bytecode test

Do not write a hand-assembled bytecode test for basic correctness of an operation (e.g., "ADD_I32 of 10 and 3 produces 13") when the same scenario can be covered by an end-to-end test (e.g., `result := 10 + 3`). The end-to-end test covers the same behavior with no dependency on bytecode encoding.

Do not use symbolic opcode constants in tests whose purpose is backwards compatibility. The entire point of those tests is to catch when byte values change — symbolic constants defeat this purpose.

### Consequences

* Good, because each test category has a single, clear purpose — correctness, compatibility, or edge cases
* Good, because adding a new operation requires only end-to-end tests (for correctness) and compile tests (for compatibility), not hand-assembled bytecode tests
* Good, because VM bytecode changes (renumbering, superinstructions, encoding changes) primarily affect compile tests (which must be updated to reflect the new encoding) and a small number of edge-case VM tests, rather than dozens of basic-correctness VM tests
* Good, because hardcoded bytes in compile tests serve as a tripwire for unintended compatibility breaks
* Bad, because when VM internals change, the compile tests must be manually updated with new byte sequences — this is intentional (the update forces an explicit decision about compatibility) but is still work
* Neutral, because edge-case VM tests still use hardcoded bytes, but there are few enough that the maintenance cost is low

### Confirmation

After applying this strategy:
1. Every operation expressible in IEC 61131-3 has at least one end-to-end test
2. Every opcode byte value is asserted by at least one compile test with hardcoded hex
3. VM bytecode tests only exist for scenarios that cannot be expressed as end-to-end tests
4. No VM bytecode test duplicates coverage that an end-to-end test already provides for basic correctness

## Pros and Cons of the Options

### Test Everything at All Three Levels (Current State)

Every operation has VM bytecode tests, compile tests, and end-to-end tests, with significant overlap in what they verify.

* Good, because any single layer of tests can be deleted without losing coverage
* Bad, because ~40 VM bytecode test files duplicate basic correctness already covered by end-to-end tests
* Bad, because bytecode encoding changes require updating tests at two levels (compile tests AND VM tests) instead of one
* Bad, because the sheer volume of hand-assembled bytecode tests obscures which tests exist for compatibility vs correctness

### Test Correctness via E2E, Compatibility via Compile, Edge Cases via Bytecode (Chosen)

End-to-end tests cover correctness, compile tests cover backwards compatibility with hardcoded bytes, VM tests cover only edge cases.

* Good, because bytecode encoding changes affect compile tests (must update) and ~10-15 edge-case VM tests, not ~40+ basic-correctness VM tests
* Good, because new operations only need end-to-end + compile tests — no hand-assembled bytecode needed for basic correctness
* Good, because the role of each test file is immediately clear from its category
* Bad, because migrating existing VM tests to end-to-end requires writing new IEC source programs
* Neutral, because the total number of test assertions stays roughly the same — they just move to the appropriate level

### Test Only via End-to-End

Remove all bytecode-level tests, relying entirely on end-to-end tests for all validation.

* Good, because tests are completely immune to bytecode encoding changes
* Bad, because there is no backwards compatibility check — opcode byte values could change silently without any test failing
* Bad, because VM edge cases (traps, overflow wrapping) cannot be tested — IEC 61131-3 source cannot trigger stack underflow or invalid opcodes
* Bad, because compile correctness is only checked indirectly — if the compiler generates wrong bytecode that happens to produce the right result for the test inputs, the bug goes undetected

## More Information

### Relationship to existing tests

As of this ADR, the test suite has:
- ~57 VM bytecode test files in `compiler/vm/tests/`
- ~20 compile bytecode test files in `compiler/codegen/tests/compile_*.rs`
- ~60 end-to-end test files in `compiler/codegen/tests/end_to_end_*.rs`

Applying this strategy will:
- Delete ~40 VM bytecode test files whose scenarios are already covered by end-to-end tests
- Create ~9 new end-to-end test files to cover scenarios that previously only had VM bytecode tests
- Retain ~15 VM bytecode test files for edge cases (traps, overflow wrapping, control flow)
- Leave compile tests and existing end-to-end tests unchanged

### Helper extraction

The remaining VM bytecode tests share common setup boilerplate (build container, allocate buffers, load VM, execute, read result). This boilerplate should be extracted into helper functions in `compiler/vm/tests/common/mod.rs` (e.g., `run_and_read_i32`, `run_and_expect_trap`) to reduce duplication. The `VmBuffers` struct used in both `vm/tests/` and `codegen/tests/` should be shared via a `test-support` feature in `ironplc_vm` to ensure VM API changes only need updating in one place.
