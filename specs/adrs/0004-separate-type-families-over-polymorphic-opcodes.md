# Separate Type Families Over Polymorphic Opcodes

status: proposed
date: 2026-02-18

## Context and Problem Statement

The bytecode instruction set must handle values of different kinds: numeric types (I32, U32, I64, U64, F32, F64), string buffer indices (for STRING and WSTRING variables), and function block instance references. These are fundamentally different things — a string buffer index is a lookup into a buffer table, an FB reference identifies an instance in the FB instance table, and numeric values live directly on the operand stack.

Should the instruction set use a single generic reference type (e.g., `ref`) for all non-numeric values, or should each kind of non-numeric value have its own type and dedicated opcode family?

## Decision Drivers

* **Type confusion prevention** — type confusion between string buffers and FB references is a classic VM exploitation primitive (CVE-2012-1723 in the JVM, every Lua bytecode-to-RCE exploit)
* **Static verifiability** — a bytecode verifier should be able to prove type safety by analyzing opcodes alone, without runtime type tags
* **Encoding safety for strings** — STRING (single-byte) and WSTRING (UTF-16) have different character widths; misinterpreting one as the other silently corrupts data
* **Opcode budget** — separate families consume more of the 256-opcode space
* **Interpreter complexity** — more opcodes means more dispatch handlers

## Considered Options

* Single generic `ref` type with runtime type tags
* Two reference types: `buf_idx` (strings) and `fb_ref` (FB instances), with STRING/WSTRING sharing opcodes polymorphically
* Three distinct families: `buf_idx` for STRING, `buf_idx` for WSTRING (separate opcodes), and `fb_ref` for FB instances

## Decision Outcome

Chosen option: "Three distinct families", because it makes every type distinction statically checkable by the compiler and verifier, eliminating entire classes of silent data corruption.

Specifically:

1. **`buf_idx` vs `fb_ref`**: The generic `ref` type was split into `buf_idx` (string buffer index) and `fb_ref` (FB instance reference). These use different opcode families: STR_*/WSTR_* opcodes consume `buf_idx`, FB_* opcodes consume `fb_ref`. The verifier can prove they are never mixed without runtime tags.

2. **STRING vs WSTRING**: Rather than polymorphic string opcodes that dispatch based on a runtime type tag in the buffer table, STRING and WSTRING get separate opcode families (STR_LEN vs WSTR_LEN, STR_CONCAT vs WSTR_CONCAT, etc.). The compiler emits the correct family based on the declared variable type.

### Consequences

* Good, because a `buf_idx` value can never be consumed by an FB_* opcode (or vice versa) — the verifier rejects it statically
* Good, because a STRING buffer can never be silently passed to a WSTR_* opcode — the verifier rejects it statically. As defense-in-depth, the VM maintains a one-byte encoding tag (narrow/wide) per buffer entry in the buffer table and asserts that STR_* opcodes always receive narrow-tagged buffers and WSTR_* opcodes always receive wide-tagged buffers, trapping immediately on mismatch. This tag costs one byte per buffer (not per stack value) and is only checked at string opcode entry points — a negligible overhead compared to the string operation itself.
* Good, because the verifier does not need runtime type tags to prove type safety — the opcode itself encodes the expected type
* Good, because this eliminates the entire class of "confused reference type" vulnerabilities that have led to sandbox escapes in the JVM and arbitrary code execution in Lua
* Bad, because the WSTRING family requires separate dispatch handlers for each STRING operation — though with ADR-0008's BUILTIN opcode, STRING and WSTRING functions share a single opcode with distinct func_id ranges, keeping the type safety properties while using only one opcode slot
* Bad, because the interpreter has additional dispatch handlers for STRING vs WSTRING, most of which are near-identical (differing only in character width) — with BUILTIN, these are func_id handlers within a single opcode dispatcher rather than top-level opcode handlers
* Neutral, because the opcode budget has 99 free slots, sufficient for planned future extensions (OOP method dispatch, pointer/reference operations)

### Confirmation

Verify by writing verifier test cases that:
1. Reject bytecode that passes a `buf_idx` to FB_CALL or FB_STORE_PARAM
2. Reject bytecode that passes an `fb_ref` to STR_LEN or STR_CONCAT
3. Reject bytecode that passes a STRING `buf_idx` to WSTR_CONCAT
4. Accept bytecode that correctly uses each type family in isolation

## Pros and Cons of the Options

### Single Generic `ref` Type with Runtime Type Tags

A single `ref` type on the operand stack, with a runtime type tag (string-narrow, string-wide, fb-instance) attached to each value. All string opcodes are polymorphic — STR_CONCAT checks the tag and dispatches to narrow or wide implementation.

* Good, because the opcode count is minimal — one set of string opcodes handles both STRING and WSTRING
* Good, because the interpreter has fewer dispatch handlers
* Bad, because every string operation must check the type tag at runtime — one extra branch per operation
* Bad, because a bug in the type tag (stale value, corrupted memory, verifier bypass) silently misinterprets character data — UTF-16 bytes read as single-byte characters, or vice versa
* Bad, because the verifier cannot distinguish `buf_idx` from `fb_ref` by opcode alone — it must track type tags through the abstract interpretation, which is the exact pattern that led to eBPF verifier bypasses (CVE-2020-8835, CVE-2023-2163)
* Bad, because type confusion between `ref` kinds (string vs FB) becomes a single-bug-away exploit primitive, as demonstrated in JVM CVE-2012-1723

### Two Reference Types with Polymorphic String Opcodes

Split `ref` into `buf_idx` and `fb_ref`, but keep STRING and WSTRING sharing the same STR_* opcodes. The runtime checks whether a `buf_idx` points to a narrow or wide buffer.

* Good, because `buf_idx` vs `fb_ref` confusion is eliminated statically
* Good, because the opcode count is moderate (no WSTR_* family needed)
* Bad, because STRING/WSTRING confusion requires a runtime check on every string operation
* Bad, because the verifier can prove "this is a string buffer" but not "this is a narrow string buffer" — the narrower property requires runtime enforcement
* Neutral, because this is a reasonable middle ground if opcode budget is tight

### Three Distinct Families (chosen)

Separate opcode families for STRING (`STR_*`), WSTRING (`WSTR_*`), and FB instances (`FB_*`). The operand stack carries `buf_idx` for strings and `fb_ref` for FBs, but the opcode encodes which string encoding is expected.

* Good, because all type distinctions are statically verifiable from the opcode stream alone
* Good, because defense-in-depth is trivial — the VM can assert the buffer's encoding matches the opcode family at near-zero cost
* Good, because the security analysis showed that every layer of static type checking removes an exploitation primitive
* Bad, because 13 additional opcodes are needed for WSTRING
* Bad, because interpreter code size increases (relevant for flash-constrained micro PLCs)

## More Information

### Security precedents driving this decision

| Vulnerability | Root cause | How separate type families prevent it |
|---|---|---|
| CVE-2012-1723 (JVM) | Verifier cached type info; allowed treating one reference type as another | Distinct opcodes mean the verifier never needs to cache — the opcode *is* the type check |
| Lua bytecode RCE (saelo, 2017) | No type verification; integer treated as table reference | With separate families, consuming a numeric value with a STR_* opcode is a static verification error |
| Java Card type confusion | byte[] confused with short[]; different element widths read different memory | STR_* vs WSTR_* prevents exactly this — different character widths never share opcodes |
| eBPF CVE-2020-8835 | Verifier incorrectly tracked value ranges for tagged types | Eliminating tags eliminates the tracking — the opcode encodes the type, not a mutable tag |

### Impact on opcode budget

| Component from this ADR | Opcodes | Notes |
|---|---|---|
| Split `ref` → `buf_idx` + `fb_ref` | +0 | Stack type renamed; no opcode changes |
| STRING variable access (STR_LOAD_VAR, STR_STORE_VAR) | +2 | Dedicated opcodes for value-copy semantics |
| WSTRING variable access (WSTR_LOAD_VAR, WSTR_STORE_VAR) | +2 | Dedicated opcodes for value-copy semantics |
| STRING/WSTRING function operations | +0 | Dispatched via BUILTIN func_id (ADR-0008), not separate opcodes |
| **Total from this ADR** | **+4** | |

The full instruction set uses 157 of 256 opcode slots (61%), leaving 99 for future extensions.

The type safety properties of this ADR are preserved in the BUILTIN opcode through distinct func_id ranges for STRING (0x0100–0x010A) and WSTRING (0x0200–0x020A) functions. The verifier distinguishes STRING from WSTRING operations by func_id, maintaining the same static guarantees as separate opcode families.
