# Plan: Codegen Test Sweep — Wire-Format vs. Behavioural Split

## Context

The codegen test suite has ~80 callsites of `get_function_bytecode()`
across 34 test files in `compiler/codegen/tests/`. Each callsite
inlines a raw byte literal like:

```rust
&[
    0x10, 0x00, 0x00, // LOAD_VAR_I32 var:0
    0x01, 0x00, 0x00, // LOAD_CONST_I32 pool:0 (0)
    0x6C,             // GT_I32
    0xB2, 0x0D, 0x00, // JMP_IF_NOT offset:+13
    ...
]
```
(see `compiler/codegen/tests/compile_loops.rs` for the dominant
pattern).

These literals are doing **two distinct jobs at once**:

- **Job A — Behavioural:** verifying that codegen emits the right
  *sequence of opcodes in the right order* for each language
  construct. This is the dominant case across most tests.
- **Job B — Wire-format compatibility:** ensuring that the
  *byte-level encoding* doesn't drift accidentally — opcode
  renumbering, endianness flips, operand-width changes. Because
  tests use internally-defined constants, naive symbolic-only
  assertions would pass silently while the wire format silently
  broke.

The cost of conflating them is concrete and ongoing:

- The eight-wave opcode-encoding migration
  (`specs/plans/2026-05-01-opcode-encoding-wave-{2..8}.md`) has
  already paid this tax repeatedly — every renumber wave touches
  ~30 codegen test files because the wire-format byte values are
  smeared across each test. This plan removes that tax for future
  encoding experiments (including but not limited to a possible
  cell-based encoding migration described in
  `specs/plans/2026-04-30-vm-predecoded-instructions.md`).
- A renumber today fails ~30 tests with confusingly different
  diffs across files. The actual contract violation — "this opcode
  constant changed" — is buried.

The fix is to **separate the two jobs explicitly**. Job B becomes
*more* tightly enforced, not less.

## Scope

- **Add** `compiler/codegen/tests/wire_format.rs` — the canonical
  wire-format test file (opcode-byte pinning + golden encoding
  tests).
- **Add** `assert_bytecode!` helper in
  `compiler/codegen/tests/common/mod.rs`.
- **Sweep** the ~30+ `compile_*.rs` test files to use the helper
  for the behavioural assertions, leaving wire-format guarantees
  centralized in `wire_format.rs`.

## What this plan does NOT do

- **No production code changes.** This is purely test
  infrastructure. No changes to `Emitter`, the VM dispatch loop,
  the container format, or any opcode definitions.
- **No commitment to cell-based encoding.** This work facilitates
  benchmarking and testing of future encoding experiments without
  prescribing which experiment.
- **No removal of `end_to_end_*.rs` round-trip tests.** Those
  exercise complete program execution and are a separate
  behavioural layer.

## Steps

### 1. `compiler/codegen/tests/wire_format.rs` — new

Two kinds of tests live here:

**(a) Opcode-byte pinning.** A test asserts every public
opcode constant in `compiler/container/src/opcode.rs` matches its
expected byte value. The existing structured `[op_class:6][type:2]`
encoding already documents what each value should be, so the test
can be organized by op-class. A separate test asserts the
**encoding scheme** — i.e., for op-classes with multiple type
variants, the low 2 bits map to the documented type tag, and the
high 6 bits are constant within a family. This catches accidental
violations of the encoding contract, not just byte drift.

**(b) Golden encoding tests** (~10-15 tests). One test per
*encoding shape* taken from `opcode::instruction_size()`:

| Shape                       | Example opcode(s)                | Test name                               |
| --------------------------- | -------------------------------- | --------------------------------------- |
| 1-byte                      | ADD_I32                          | `wire_when_1byte_op_then_one_byte`      |
| 2-byte (op + u8)            | FB_STORE_PARAM                   | `wire_when_fb_param_then_two_bytes`     |
| 3-byte (op + u16)           | LOAD_VAR_I32                     | `wire_when_load_var_then_three_bytes`   |
| 3-byte (op + i16)           | JMP, JMP_IF_NOT                  | `wire_when_jmp_then_three_bytes_le_i16` |
| 5-byte (op + u16 + u16)     | CALL, LOAD_ARRAY                 | `wire_when_call_then_five_bytes`        |
| 5-byte (op + u32)           | STR_LOAD_VAR, LEN_STR            | `wire_when_str_load_then_five_bytes`    |
| 7-byte (op + u32 + u16)     | STR_INIT                         | `wire_when_str_init_then_seven_bytes`   |
| 9-byte (op + u32 + u32)     | FIND_STR / REPLACE_STR / etc.    | `wire_when_find_str_then_nine_bytes`    |

Each test compiles a minimal source program and asserts the exact
raw bytes. These are the canonical guards for operand widths,
endianness (little-endian per
`compiler/container/src/code_section.rs`), and per-shape layout.

### 2. `compiler/codegen/tests/common/mod.rs` — `assert_bytecode!` helper

A `macro_rules!` macro that:

- Accepts a sequence of opcode-with-operand tokens like
  `LOAD_VAR_I32(0)`, `JMP_IF_NOT(13)`, `CALL { func: 1, vars: 0 }`,
  `RET_VOID`.
- Renders both the **expected** sequence and the **actual**
  bytecode into a normalized symbolic form (`OP_NAME operand` per
  line) and `assert_eq!`s them.
- Walks the actual byte stream using `opcode::instruction_size()`
  (single source of truth at `compiler/container/src/opcode.rs`) so
  no per-opcode width logic is duplicated.
- Handles peephole-DUP forms by accepting bare `DUP` tokens.
- Uses informative failure output: when the assertion fails it
  prints both sequences side-by-side so the diff identifies which
  *instruction* differs, not which byte.

The macro is opcode-name-symbolic, so a future opcode renumber
does not touch any callsite.

### 3. Sweep `compile_*.rs` tests

Mechanical conversion. Each call site replaces a byte-literal
`assert_eq!(bytecode, &[ ... ])` with an `assert_bytecode!(bytecode,
[ ... ])`. The translation is line-for-line:

```rust
// Before:
assert_eq!(
    bytecode,
    &[
        0x10, 0x00, 0x00, // LOAD_VAR_I32 var:0
        0x01, 0x00, 0x00, // LOAD_CONST_I32 pool:0 (0)
        0x6C,             // GT_I32
        0xB2, 0x0D, 0x00, // JMP_IF_NOT offset:+13
        0xB5,             // RET_VOID
    ]
);

// After:
assert_bytecode!(bytecode, [
    LOAD_VAR_I32(0),
    LOAD_CONST_I32(0),
    GT_I32,
    JMP_IF_NOT(13),
    RET_VOID,
]);
```

Files affected (~30+, identifiable via `git grep -l
'get_function_bytecode' compiler/codegen/tests/`):
`compile_abs*.rs`, `compile_add.rs`, `compile_array.rs`,
`compile_bitwise.rs`, `compile_bool.rs`, `compile_case.rs`,
`compile_cmp.rs`, `compile_dup.rs`, `compile_exit_return.rs`,
`compile_float.rs`, `compile_func_forms.rs`, `compile_if.rs`,
`compile_limit*.rs`, `compile_loops.rs`, `compile_max*.rs`,
`compile_min*.rs`, `compile_mod.rs`, `compile_mul.rs`,
`compile_mux*.rs`, `compile_neg.rs`, `compile_pow.rs`,
`compile_sel*.rs`, `compile_shift.rs`, `compile_sqrt.rs`,
`compile_struct.rs`, `compile_sub.rs`, `compile_types.rs`.

### 4. Verification

- Land `wire_format.rs` first (step 1). Verify it actually catches
  drift by deliberately mutating one opcode constant in a scratch
  branch and confirming exactly the wire-format test fails with a
  clear message. Revert the mutation.
- Land the `assert_bytecode!` helper (step 2). Self-test the macro
  with a small fixed bytecode in `common/mod.rs` itself.
- Sweep `compile_*.rs` files (step 3) — each commit is mechanical
  and reviewable in isolation. After the sweep, repeat the
  scratch-branch opcode mutation and confirm only `wire_format.rs`
  fails (proving the partition is clean).
- `cd compiler && just` passes throughout.

## Test naming

All new tests follow the project BDD convention
`function_when_condition_then_result`:

- `opcode_constants_when_pinned_then_match_canonical_bytes`
- `wire_when_load_var_i32_then_three_bytes_le_u16`
- etc.

## Out of scope

- The cell-based encoding migration itself
  (`specs/plans/2026-04-30-vm-predecoded-instructions.md`). This
  test infrastructure makes that decision *cheaper to test*, not
  inevitable.
- Reorganizing the `compile_*.rs` test file layout.
- Replacing the existing `end_to_end_*.rs` round-trip tests.
