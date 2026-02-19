# Separate Type Families Over Polymorphic Opcodes

status: proposed
date: 2026-02-18

## Context and Problem Statement

The bytecode instruction set must handle values of different kinds: numeric types (I32, U32, I64, U64, F32, F64), string buffer indices (for STRING and WSTRING variables), and function block instance references. These are fundamentally different things — a string buffer index is a lookup into a buffer table, an FB reference identifies an instance in the FB instance table, and numeric values live directly on the operand stack.

Should the instruction set use a single generic reference type (e.g., `ref`) for all non-numeric values, or should each kind of non-numeric value have its own type and type-specific dispatch?

## Decision Drivers

* **Type confusion prevention** — type confusion between string buffers and FB references is a classic VM exploitation primitive (CVE-2012-1723 in the JVM, every Lua bytecode-to-RCE exploit)
* **Static verifiability** — a bytecode verifier should be able to prove type safety by analyzing opcodes and operands alone, without runtime type tags
* **Encoding safety for strings** — STRING (single-byte) and WSTRING (UCS-2) have different character widths; misinterpreting one as the other silently corrupts data
* **Opcode budget** — separate families consume more of the 256-opcode space
* **Interpreter complexity** — more type-specific dispatch paths means more handlers

## Considered Options

* Single generic `ref` type with runtime type tags
* Two reference types: `buf_idx` (strings) and `fb_ref` (FB instances), with STRING/WSTRING sharing dispatch polymorphically
* Three distinct type families: separate stack types for STRING (`buf_idx_str`), WSTRING (`buf_idx_wstr`), and FB instances (`fb_ref`), with type-specific dispatch at every level

## Decision Outcome

Chosen option: "Three distinct type families", because it makes every type distinction statically checkable by the compiler and verifier, eliminating entire classes of silent data corruption.

Specifically:

1. **`buf_idx_str` / `buf_idx_wstr` vs `fb_ref`**: The generic `ref` type is split into `buf_idx_str` (STRING buffer index), `buf_idx_wstr` (WSTRING buffer index), and `fb_ref` (FB instance reference). These use different dispatch families: STRING and WSTRING operations consume `buf_idx_str` and `buf_idx_wstr` respectively, FB_* opcodes consume `fb_ref`. The verifier can prove they are never mixed without runtime tags.

2. **STRING vs WSTRING**: Rather than polymorphic dispatch that checks a runtime type tag in the buffer table, STRING and WSTRING are distinguished at every level:
   - **Variable access**: Separate opcodes — STR_LOAD_VAR / STR_STORE_VAR for STRING, WSTR_LOAD_VAR / WSTR_STORE_VAR for WSTRING. These push `buf_idx_str` and `buf_idx_wstr` respectively.
   - **Function operations**: The BUILTIN opcode uses distinct func_id ranges — 0x0100–0x010A for STRING functions (LEN, CONCAT, LEFT, etc.) and 0x0200–0x020A for WSTRING functions. The verifier checks the func_id to determine the expected stack types, rejecting a `buf_idx_str` passed to a WSTRING func_id or vice versa.

   The compiler emits the correct family based on the declared variable type.

### Consequences

* Good, because a `buf_idx_str` or `buf_idx_wstr` value can never be consumed by an FB_* opcode (or vice versa) — the verifier rejects it statically
* Good, because a STRING buffer can never be silently passed to a WSTRING operation — the verifier rejects it statically. As defense-in-depth, the VM maintains a one-byte encoding tag (narrow/wide) per buffer entry in the buffer table and asserts that STRING operations always receive narrow-tagged buffers and WSTRING operations always receive wide-tagged buffers, trapping immediately on mismatch. This tag costs one byte per buffer (not per stack value) and is only checked at operation entry points — a negligible overhead compared to the string operation itself.
* Good, because the verifier does not need runtime type tags to prove type safety — the opcode and func_id encode the expected type
* Good, because this eliminates the entire class of "confused reference type" vulnerabilities that have led to sandbox escapes in the JVM and arbitrary code execution in Lua
* Bad, because the WSTRING family requires separate dispatch handlers for each STRING operation — these are func_id handlers within the BUILTIN dispatcher, keeping the type safety properties while using only one opcode slot
* Bad, because the interpreter has additional dispatch handlers for STRING vs WSTRING, most of which are near-identical (differing only in character width)
* Neutral, because the opcode budget has 85 free slots, sufficient for planned future extensions (OOP method dispatch)

### Confirmation

Verify by writing verifier test cases that:
1. Reject bytecode that passes a `buf_idx_str` or `buf_idx_wstr` to FB_CALL or FB_STORE_PARAM
2. Reject bytecode that passes an `fb_ref` to a STRING or WSTRING BUILTIN func_id
3. Reject bytecode that passes a `buf_idx_str` to a WSTRING BUILTIN func_id (e.g., WSTR_CONCAT)
4. Reject bytecode that passes a `buf_idx_wstr` to a STRING BUILTIN func_id (e.g., STR_CONCAT)
5. Accept bytecode that correctly uses each type family in isolation

## Pros and Cons of the Options

### Single Generic `ref` Type with Runtime Type Tags

A single `ref` type on the operand stack, with a runtime type tag (string-narrow, string-wide, fb-instance) attached to each value. All string operations are polymorphic — CONCAT checks the tag and dispatches to narrow or wide implementation.

* Good, because the opcode count is minimal — one set of string operations handles both STRING and WSTRING
* Good, because the interpreter has fewer dispatch handlers
* Bad, because every string operation must check the type tag at runtime — one extra branch per operation
* Bad, because a bug in the type tag (stale value, corrupted memory, verifier bypass) silently misinterprets character data — UCS-2 bytes read as single-byte characters, or vice versa
* Bad, because the verifier cannot distinguish `buf_idx` from `fb_ref` by opcode alone — it must track type tags through the abstract interpretation, which is the exact pattern that led to eBPF verifier bypasses (CVE-2020-8835, CVE-2023-2163)
* Bad, because type confusion between `ref` kinds (string vs FB) becomes a single-bug-away exploit primitive, as demonstrated in JVM CVE-2012-1723

### Two Reference Types with Polymorphic String Dispatch

Split `ref` into `buf_idx` and `fb_ref`, but keep STRING and WSTRING sharing the same dispatch path. The runtime checks whether a `buf_idx` points to a narrow or wide buffer.

* Good, because `buf_idx` vs `fb_ref` confusion is eliminated statically
* Good, because the dispatch path count is moderate (no WSTRING-specific handlers needed)
* Bad, because STRING/WSTRING confusion requires a runtime check on every string operation
* Bad, because the verifier can prove "this is a string buffer" but not "this is a narrow string buffer" — the narrower property requires runtime enforcement
* Neutral, because this is a reasonable middle ground if opcode budget is tight

### Three Distinct Type Families (chosen)

Separate stack types for STRING (`buf_idx_str`), WSTRING (`buf_idx_wstr`), and FB instances (`fb_ref`). Variable access uses dedicated opcodes per string encoding. Function operations use the BUILTIN opcode with distinct func_id ranges for STRING and WSTRING. The verifier statically checks type correctness from the opcode and func_id.

* Good, because all type distinctions are statically verifiable from the opcode stream and operands alone
* Good, because defense-in-depth is trivial — the VM can assert the buffer's encoding matches the expected type at near-zero cost
* Good, because the security analysis showed that every layer of static type checking removes an exploitation primitive
* Bad, because separate STRING/WSTRING dispatch handlers are needed (differing only in character width)
* Bad, because interpreter code size increases (relevant for flash-constrained micro PLCs)

## More Information

### Security precedents driving this decision

| Vulnerability | Root cause | How separate type families prevent it |
|---|---|---|
| CVE-2012-1723 (JVM) | Verifier cached type info; allowed treating one reference type as another | Distinct type families mean the verifier never needs to cache — the opcode/func_id *is* the type check |
| Lua bytecode RCE (saelo, 2017) | No type verification; integer treated as table reference | With separate families, consuming a numeric value with a string operation is a static verification error |
| Java Card type confusion | byte[] confused with short[]; different element widths read different memory | STRING vs WSTRING type families prevent exactly this — different character widths never share dispatch paths |
| eBPF CVE-2020-8835 | Verifier incorrectly tracked value ranges for tagged types | Eliminating tags eliminates the tracking — the opcode/func_id encodes the type, not a mutable tag |

### Impact on opcode budget

| Component | Opcodes | Notes |
|---|---|---|
| Split `ref` → `buf_idx_str` + `buf_idx_wstr` + `fb_ref` | +0 | Stack type split; no opcode changes |
| STRING variable access (STR_LOAD_VAR, STR_STORE_VAR) | +2 | Dedicated opcodes for value-copy semantics |
| WSTRING variable access (WSTR_LOAD_VAR, WSTR_STORE_VAR) | +2 | Dedicated opcodes for value-copy semantics |
| STRING/WSTRING function operations | +0 | Dispatched via BUILTIN func_id ranges, not separate opcodes |
| **Total** | **+4** | |

The full instruction set uses 171 of 256 opcode slots (67%), leaving 85 for future extensions.

The type safety properties are enforced through distinct func_id ranges for STRING (0x0100–0x010A) and WSTRING (0x0200–0x020A) functions within the BUILTIN opcode. The verifier distinguishes STRING from WSTRING operations by func_id, providing the same static guarantees as if they were separate opcode families.
