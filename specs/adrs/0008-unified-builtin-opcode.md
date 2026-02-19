# Unified BUILTIN Opcode for Standard Library Functions

status: proposed
date: 2026-02-19

## Context and Problem Statement

The bytecode instruction set uses dedicated opcodes for each string function: STR_LEN, STR_CONCAT, STR_LEFT, STR_RIGHT, STR_MID, STR_FIND, STR_INSERT, STR_DELETE, STR_REPLACE, STR_EQ, STR_LT (11 opcodes), and mirrored WSTR_* variants (11 more). This consumes 22 opcodes for string functions alone.

Additionally, IEC 61131-3 defines numeric standard library functions (ABS, SQRT, MIN, MAX, LIMIT) and type conversion functions (INT_TO_REAL, etc.) that the compiler must support. As the standard library grows, each new function would consume additional opcode slots under the current approach.

Should each standard library function continue to have its own opcode, or should they share a single dispatch opcode?

## Decision Drivers

* **Opcode budget pressure** — 178 of 256 opcodes were in use, leaving 78 for all future extensions (OOP, pointers, new operations); string functions alone consumed 22 slots
* **Standard library growth** — IEC 61131-3 defines many more functions beyond the initial set (trigonometric, logarithmic, date/time, additional string operations); each would consume an opcode slot under the per-function model
* **Consistency with FB_CALL** — function block invocation already uses a single opcode (FB_CALL) with a type_id operand for dispatch; the same pattern applies naturally to standard library functions
* **Verifier complexity** — the verifier must know the type signature of each built-in function regardless of whether it is encoded as an opcode or a func_id; the verification work is equivalent
* **Safety properties** — the type safety guarantees from ADR-0004 (separate STRING/WSTRING families) must be preserved

## Considered Options

* Per-function opcodes (current approach)
* Single BUILTIN opcode with function ID dispatch
* Extend the existing CALL opcode with intrinsic recognition (like FB_CALL)

## Decision Outcome

Chosen option: "Single BUILTIN opcode with function ID dispatch", because it consolidates 22 string function opcodes into one, provides an extensible mechanism for all standard library functions, and preserves the type safety properties from ADR-0004 through the function ID rather than the opcode byte.

The new instruction:

```
BUILTIN func_id: u16    — Call a built-in standard library function
  Stack effect: [args...] → [result] (depends on func_id; see built-in function table)
```

The `func_id` is a well-known constant shared between the compiler and VM. The verifier uses the func_id to determine the expected stack types and validate type correctness, exactly as it does for opcode-encoded type information.

### What changes

- **Removed**: 22 individual string function opcodes (STR_LEN through STR_LT, WSTR_LEN through WSTR_LT)
- **Added**: 1 BUILTIN opcode at 0xC4
- **Net**: 21 opcode slots freed (22 removed - 1 added)
- **New opcode total**: 157 of 256 (61%), leaving 99 slots for future extensions

### What stays unchanged

- **String variable access opcodes** (STR_LOAD_VAR, STR_STORE_VAR, WSTR_LOAD_VAR, WSTR_STORE_VAR) remain as dedicated opcodes because they are load/store operations with distinct stack semantics, not function calls
- **Type conversion opcodes** (NARROW_*, WIDEN_*, cross-domain conversions) remain as dedicated opcodes because they are fundamental VM type operations that the verifier tracks for type state transitions
- **TIME arithmetic opcodes** (TIME_ADD, TIME_SUB) remain as dedicated opcodes for the same reason
- **FB_CALL** remains unchanged — function blocks and standard library functions use separate dispatch mechanisms because FBs have instance state and a multi-step parameter protocol (FB_LOAD_INSTANCE, FB_STORE_PARAM, FB_CALL)

### Consequences

* Good, because 21 opcode slots are freed for future extensions (OOP method dispatch, pointer operations, new control flow)
* Good, because the standard library can grow without consuming opcode slots — adding a new function requires only a new func_id entry, not a new opcode
* Good, because the pattern is consistent with FB_CALL — both use a single opcode with a dispatch operand for a family of operations
* Good, because STRING/WSTRING type safety is preserved — the verifier checks func_id-specific type signatures, rejecting a buf_idx_str passed to a WSTR_* func_id or vice versa
* Good, because the verifier's job is equivalent in difficulty — it maps func_id to a type signature the same way it currently maps opcode to a type signature
* Bad, because each BUILTIN instruction is 3 bytes (1 opcode + 2 func_id) versus 1 byte for the former dedicated opcodes, increasing bytecode size for string-heavy programs by ~2 bytes per string operation
* Bad, because the VM dispatch for BUILTIN requires a table lookup or switch on func_id, which is slightly slower than direct opcode dispatch — though string operations themselves (buffer copies, searches) dominate execution time
* Neutral, because the security model is equivalent — the verifier statically checks type correctness whether the type is encoded in the opcode byte or the func_id operand

### Confirmation

Verify by writing verifier test cases that:
1. Accept BUILTIN with a valid STRING func_id and correct buf_idx_str arguments
2. Reject BUILTIN with a STRING func_id and buf_idx_wstr arguments
3. Reject BUILTIN with a WSTRING func_id and buf_idx_str arguments
4. Reject BUILTIN with an undefined func_id
5. Accept BUILTIN with numeric function func_ids and correct numeric type arguments

## Pros and Cons of the Options

### Per-Function Opcodes (previous approach)

Each standard library function gets its own opcode: STR_LEN, STR_CONCAT, ABS_I32, ABS_F32, etc.

* Good, because dispatch is a single byte lookup — maximum interpreter speed
* Good, because the opcode byte directly encodes the operation and type — no secondary lookup needed
* Bad, because opcode slots are finite (256 total) and each new function consumes one — IEC 61131-3 defines dozens of standard functions, and supporting them all would exhaust the budget
* Bad, because string functions alone consumed 22 slots (14% of the total budget) for a category of operations that is not performance-critical
* Bad, because any addition to the standard library requires a new opcode, changing the instruction set and requiring VM updates

### Single BUILTIN Opcode with Function ID Dispatch (chosen)

One opcode dispatches to all standard library functions via a u16 func_id operand. The func_id table is well-known to compiler, verifier, and VM.

* Good, because the opcode budget impact is exactly 1 slot regardless of how many functions are supported
* Good, because new standard library functions can be added by allocating a func_id, without changing the instruction set encoding
* Good, because the verifier handles BUILTIN the same way it handles any typed opcode — look up the expected types from the func_id, check the stack
* Bad, because the instruction is 3 bytes instead of 1, increasing bytecode size
* Bad, because dispatch requires a func_id lookup instead of direct opcode jump
* Neutral, because the safety properties are equivalent to per-function opcodes

### Extend CALL with Intrinsic Recognition

Use the existing CALL instruction for standard library functions. The VM recognizes well-known function IDs and routes to native implementations, similar to how FB_CALL recognizes standard FB type IDs.

* Good, because no new opcode is needed at all
* Bad, because CALL currently indexes into the code section's function directory, which contains bytecode offsets — standard library functions have no bytecode body, so the function directory would need stub entries or special sentinel values
* Bad, because it overloads the semantics of CALL — a single opcode would mean "call user bytecode OR call VM-native function," and the distinction is invisible in the bytecode
* Bad, because the verifier would need different validation paths for user functions (check against function signature in type section) vs built-in functions (check against hardcoded signatures), behind the same opcode

## More Information

### Interaction with ADR-0004 (Separate Type Families)

ADR-0004 decided on separate STRING and WSTRING opcode families to prevent encoding confusion. The BUILTIN opcode preserves this safety property through a different mechanism:

| Property | ADR-0004 approach | BUILTIN approach |
|----------|-------------------|------------------|
| STRING/WSTRING distinction | Different opcode bytes | Different func_id values |
| Static verifiability | Opcode encodes expected type | func_id encodes expected type |
| Verifier mechanism | Map opcode → type signature | Map func_id → type signature |
| Runtime defense-in-depth | VM asserts buffer encoding tag at opcode entry | VM asserts buffer encoding tag at func_id dispatch |

The security invariant ("a STRING buf_idx can never reach a WSTRING operation") is preserved identically. The encoding mechanism changes from opcode-level to operand-level, but the verifier's ability to statically prove type safety is unchanged.

### Interaction with ADR-0005 (Safety-First)

The safety-first principle says "encode type information and invariants in the opcode." The BUILTIN approach encodes type information in the operand (func_id) rather than the opcode byte. This is consistent with safety-first because:

1. The func_id is statically known at verification time — it is a constant in the bytecode, not a runtime value
2. The verifier's type checking is equally strong — it maps func_id to exact type signatures
3. The pattern already exists in the instruction set: FB_CALL uses type_id for dispatch, and the safety analysis accepted this
4. The defense-in-depth property is maintained: the VM checks buffer encoding tags at BUILTIN dispatch, just as it did at per-function opcode entry

### Built-in function ID ranges

| Range | Category | Description |
|-------|----------|-------------|
| 0x0000–0x00FF | Reserved | Future use |
| 0x0100–0x010A | STRING functions | LEN, CONCAT, LEFT, RIGHT, MID, FIND, INSERT, DELETE, REPLACE, EQ, LT |
| 0x0200–0x020A | WSTRING functions | LEN, CONCAT, LEFT, RIGHT, MID, FIND, INSERT, DELETE, REPLACE, EQ, LT |
| 0x0300–0x03FF | Numeric functions | ABS, SQRT, MIN, MAX, LIMIT (with type-specific variants) |
| 0x0400–0xFFFF | Reserved | Future standard library extensions |

The func_id ranges are organized by category to enable efficient dispatch (range check → category handler → individual function). The reserved ranges allow future growth without fragmentation.

### Bytecode size impact

For a typical PLC program with 50 string operations per scan cycle:
- Previous: 50 bytes (50 × 1-byte opcode)
- New: 150 bytes (50 × 3-byte BUILTIN instruction)
- Increase: 100 bytes

This is negligible relative to the total bytecode size of a typical PLC program (2–20 KB) and the code section size limits of the container format.
