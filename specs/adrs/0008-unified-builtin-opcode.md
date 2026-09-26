# Unified BUILTIN Opcode for Standard Library Functions

status: accepted
date: 2026-02-19
amended: 2026-09-11 (Scope and Confirmation described a string func_id split that was never built)

## Context and Problem Statement

The bytecode instruction set must support IEC 61131-3 standard library functions: string functions (LEN, CONCAT, LEFT, RIGHT, MID, FIND, INSERT, DELETE, REPLACE, EQ, LT for both STRING and WSTRING), numeric functions (ABS, SQRT, MIN, MAX, LIMIT), and type conversion functions (INT_TO_REAL, etc.). A per-function opcode approach would consume 22 opcodes for string functions alone (11 STRING + 11 WSTRING), plus additional slots for each numeric function variant.

As the standard library grows (IEC 61131-3 defines trigonometric, logarithmic, date/time, and additional string operations), a per-function model would consume increasing numbers of opcode slots.

Should each standard library function have its own opcode, or should they share a single dispatch opcode?

## Decision Drivers

* **Opcode budget pressure** — a per-function opcode model would consume 22 slots for string functions alone (11 STRING + 11 WSTRING), plus additional slots for numeric function variants, leaving fewer slots for future extensions (OOP, pointers, new operations)
* **Standard library growth** — IEC 61131-3 defines many more functions beyond the initial set (trigonometric, logarithmic, date/time, additional string operations); each would consume an opcode slot under the per-function model
* **Consistency with FB_CALL** — function block invocation already uses a single opcode (FB_CALL) with a type_id operand for dispatch; the same pattern applies naturally to standard library functions
* **Verifier complexity** — the verifier must know the type signature of each built-in function regardless of whether it is encoded as an opcode or a func_id; the verification work is equivalent
* **Safety properties** — the type safety guarantees from ADR-0004 (separate STRING/WSTRING families) must be preserved

## Considered Options

* Per-function opcodes
* Single BUILTIN opcode with function ID dispatch
* Extend the existing CALL opcode with intrinsic recognition (like FB_CALL)

## Decision Outcome

Chosen option: "Single BUILTIN opcode with function ID dispatch", because it handles all standard library functions through a single opcode slot, provides an extensible mechanism for future functions, and preserves the type safety properties from ADR-0004 through the function ID.

The new instruction:

```
BUILTIN func_id: u16    — Call a built-in standard library function
  Stack effect: [args...] → [result] (depends on func_id; see built-in function table)
```

The `func_id` is a well-known constant shared between the compiler and VM. The verifier uses the func_id to determine the expected stack types and validate type correctness, exactly as it does for opcode-encoded type information.

### Scope

The BUILTIN opcode handles all standard library function calls:

- **String functions**: LEN, CONCAT, LEFT, RIGHT, MID, FIND, INSERT, DELETE, REPLACE, EQ, LT — for both STRING and WSTRING, distinguished by func_id range
- **Numeric functions**: ABS, SQRT, MIN, MAX, LIMIT — with type-specific func_id variants (e.g., ABS_I32, ABS_F64)

The following use dedicated opcodes, not BUILTIN:

- **String variable access** (STR_LOAD_VAR, STR_STORE_VAR, WSTR_LOAD_VAR, WSTR_STORE_VAR) — load/store operations with distinct stack semantics, not function calls
- **Type conversion opcodes** (NARROW_*, WIDEN_*, cross-domain conversions) — fundamental VM type operations that the verifier tracks for type state transitions
- **TIME arithmetic opcodes** (TIME_ADD, TIME_SUB) — dedicated for the same reason
- **FB_CALL** — function blocks use a separate dispatch mechanism because FBs have instance state and a multi-step parameter protocol (FB_LOAD_INSTANCE, FB_STORE_PARAM, FB_CALL)

**Opcode total**: 157 of 256 (61%), leaving 99 slots for future extensions

> **Two claims in this Scope were never built. See
> [Amendment: BUILTIN carries the numeric standard library, not the string one](#amendment-builtin-carries-the-numeric-standard-library-not-the-string-one-2026-09-11).**
> Strings do not dispatch through BUILTIN, and the opcode census is from the
> pre-ADR-0033 encoding.

### Consequences

* Good, because the opcode budget is 157/256 (61%), leaving 99 slots for future extensions (OOP method dispatch, pointer operations, new control flow)
* Good, because the standard library can grow without consuming opcode slots — adding a new function requires only a new func_id entry, not a new opcode
* Good, because the pattern is consistent with FB_CALL — both use a single opcode with a dispatch operand for a family of operations
* Good, because STRING/WSTRING type safety is preserved — the verifier checks func_id-specific type signatures, rejecting a buf_idx_str passed to a WSTR_* func_id or vice versa
* Good, because the verifier's job is equivalent in difficulty — it maps func_id to a type signature the same way it currently maps opcode to a type signature
* Bad, because each BUILTIN instruction is 3 bytes (1 opcode + 2 func_id) versus 1 byte for a dedicated opcode, increasing bytecode size for string-heavy programs by ~2 bytes per string operation
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

### Per-Function Opcodes

Each standard library function gets its own opcode: STR_LEN, STR_CONCAT, ABS_I32, ABS_F32, etc.

* Good, because dispatch is a single byte lookup — maximum interpreter speed
* Good, because the opcode byte directly encodes the operation and type — no secondary lookup needed
* Bad, because opcode slots are finite (256 total) and each new function consumes one — IEC 61131-3 defines dozens of standard functions, and supporting them all would exhaust the budget
* Bad, because string functions alone would consume 22 slots (14% of the total budget) for a category of operations that is not performance-critical
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

ADR-0004 requires separate type families for STRING, WSTRING, and FB instances to prevent type confusion. The BUILTIN opcode enforces this through distinct func_id ranges:

| Property | Mechanism |
|----------|-----------|
| STRING/WSTRING distinction | Different func_id ranges (0x0100–0x010A vs 0x0200–0x020A) |
| Static verifiability | func_id encodes expected stack type (`buf_idx_str` vs `buf_idx_wstr`) |
| Verifier mechanism | Map func_id → type signature; reject mismatched stack types |
| Runtime defense-in-depth | VM asserts buffer encoding tag (narrow/wide) at func_id dispatch |

The security invariant ("a STRING buf_idx can never reach a WSTRING operation") is enforced statically by the verifier through the func_id, with defense-in-depth buffer encoding tag checks at runtime.

### Interaction with ADR-0005 (Safety-First)

The safety-first principle says "encode type information and invariants in the opcode." The BUILTIN approach encodes type information in the operand (func_id) rather than the opcode byte. This is consistent with safety-first because:

1. The func_id is statically known at verification time — it is a constant in the bytecode, not a runtime value
2. The verifier's type checking is equally strong — it maps func_id to exact type signatures
3. The pattern already exists in the instruction set: FB_CALL uses type_id for dispatch, and the safety analysis accepted this
4. The defense-in-depth property is maintained: the VM checks buffer encoding tags at BUILTIN dispatch

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

Each BUILTIN instruction is 3 bytes (1 opcode + 2 func_id), compared to 1 byte for a hypothetical dedicated opcode. For a typical PLC program with 50 string operations per scan cycle, this adds ~100 bytes — negligible relative to the total bytecode size of a typical PLC program (2–20 KB).

## Amendment: BUILTIN carries the numeric standard library, not the string one (2026-09-11)

The decision this ADR makes — one `BUILTIN` opcode with a u16 `func_id` operand,
instead of an opcode per standard library function — is in force and is what the
compiler emits. `compiler/container/src/builtin.rs` is the single declaration of
every built-in, and `arg_count` from that table is what codegen and the stack
verifier both read. That part needs no correction.

Two supporting claims do.

### Strings never joined the func_id table

The Scope above says string functions dispatch through BUILTIN "for both STRING
and WSTRING, distinguished by func_id range." No such range exists. `LEN`,
`FIND` and `CONCAT` are their own op-classes (`OP_CLASS_LEN_STR`,
`OP_CLASS_FIND_STR`, `OP_CLASS_CONCAT_STR`), and the func_id table holds only the
numeric library — `EXPT`, `ABS`, `MIN`, `MAX`, `LIMIT`, `SEL`, the shifts and
rotates, and the transcendentals.

The func_id-range mechanism was overtaken before it was built. ADR-0034 decided
that STRING and WSTRING are distinguished by `char_width` travelling with the
data rather than by anything in the instruction, which removes the thing a
STRING/WSTRING func_id split existed to encode. `specs/design/bytecode-instruction-set.md`
has described BUILTIN's scope correctly throughout — "numeric functions,
conversions, shifts, and selection functions" — so the drift was confined to
this ADR.

Confirmation items 1, 2 and 3 therefore cannot be satisfied as written: each
tests the acceptance or rejection of `buf_idx_str` / `buf_idx_wstr` arguments
against STRING and WSTRING func_ids that do not exist. Read them as discharged by
ADR-0034 instead — it is that ADR's encoding check, at the operand rather than
the func_id, that now carries the STRING/WSTRING type safety this one was
reaching for.

Items 4 and 5 hold. An undefined func_id is rejected by the verifier
(`StackImbalance::UnknownBuiltin`, rule R0510) and trapped by the VM at runtime
(`V9007 InvalidBuiltinFunction`); the numeric func_ids are accepted with their
declared argument counts, which `builtin::arg_count` is the single source of.

### The opcode census is pre-ADR-0033

"157 of 256 (61%), leaving 99 slots" counts a flat 256-value opcode space that no
longer exists. ADR-0033 re-encoded the opcode byte as `[op_class:6][type:2]`; the
census today is 63 of 64 op-classes with one free. The figure appears once more
in the Consequences below, and is wrong there in the same way.

This does not weaken the decision — it strengthens it. The reason to route a
growing standard library through one opcode was always that the alternative
spends a scarce resource; the resource turned out to be scarcer than the number
in this ADR suggested. Every builtin added since (the transcendentals, the
BYTE/WORD rotates, `__TRUNC` and `__MOD` under ADR-0042) cost zero op-class
slots, which is the whole benefit this ADR was claiming.
