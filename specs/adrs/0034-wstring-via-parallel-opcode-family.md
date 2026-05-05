# WSTRING Implementation via Parallel Opcode Family

status: proposed
date: 2026-05-05

## Context and Problem Statement

The compiler parses STRING and WSTRING declarations, but only STRING is implemented in codegen and the VM. Completing WSTRING support requires deciding how the VM distinguishes a STRING value (Latin-1, one byte per code unit) from a WSTRING value (UTF-16LE, two bytes per code unit) at every site that allocates, reads, writes, indexes, or compares string data.

Three ADRs already constrain the answer:

- **ADR-0004** mandates *separate type families* for STRING and WSTRING. The verifier and VM distinguish them statically via dedicated opcodes (STR_LOAD_VAR vs WSTR_LOAD_VAR) and func_id ranges within BUILTIN. The driver is type-confusion prevention — silent misinterpretation of UTF-16 bytes as Latin-1 (or vice versa) corrupts data and is a known VM exploitation primitive.
- **ADR-0015** fixes the per-string memory layout at `[max_length: u16][cur_length: u16][data]` (4-byte header). Lengths are in code units, not bytes.
- **ADR-0017** places all string storage in a single unified data region addressed by `data_offset`.

The current implementation has STR_* opcodes only. WSTR_* opcodes do not exist; codegen hardcodes `char_width = 1` and panics on WSTRING literal encoding (`compile.rs:147`). Where should width information live so that WSTRING operations can be added without violating ADR-0004's separation principle, without amending ADR-0015's header layout, and without duplicating the body of every string operation in source code?

## Decision Drivers

* **Safety / defense-in-depth** — STRING/WSTRING type confusion corrupts data silently; PLC programs drive physical actuators, so silent corruption can cause physical damage (ADR-0005)
* **Static verifiability** — ADR-0004 requires that the bytecode verifier prove type safety from opcodes alone, with no reliance on runtime tags being correct
* **Deterministic performance** — string ops must have predictable, bounded execution time (ADR-0015 driver)
* **Source-code deduplication** — STR and WSTR operation bodies differ only in character width; duplicating 16+ opcode handlers per family is a maintenance hazard
* **Container format stability** — ADR-0015's 4-byte header is referenced by existing tests, the bytecode verifier, and deployed tooling; amending it would invalidate spec conformance tests and bump the format version

## Considered Options

* **A.** Polymorphic STR_* opcodes with width tracked at runtime in the string header (header grows 4 → 6 bytes)
* **B.** Polymorphic STR_* opcodes with width tracked via an opcode operand or a per-variable type tag
* **C.** Parallel WSTR_* opcode family, monomorphized per width, sharing helper functions parameterized by `char_width`
* **D.** Two separate data regions (one for STRING storage, one for WSTRING storage) with parallel opcode families

## Decision Outcome

**Chosen option: C — Parallel WSTR_\* opcode family with shared, width-parameterized helpers.**

This option is the only one consistent with all three prior ADRs:
- ADR-0004 already mandates separate families; adopting C is a direct realization, not a deviation
- ADR-0015's 4-byte header is preserved unchanged
- ADR-0017's unified data region is preserved unchanged

Source-code deduplication is achieved by extracting the body of each string operation into a helper function parameterized by `char_width: usize`. The WSTR_* opcode handler and the STR_* opcode handler each delegate to the same helper with `char_width = 2` and `char_width = 1` respectively. The handlers themselves are short dispatch shims; the operation logic lives in one place per logical operation.

### Opcodes Added

For variable access and storage (parallel to existing STR_* family):

- `WSTR_INIT` — initialize a WSTRING variable's header in the data region
- `LOAD_CONST_WSTR` — load a WSTRING literal from the constant pool into a temp buffer
- `WSTR_LOAD_VAR` / `WSTR_STORE_VAR` — copy WSTRING between data region and temp buffer
- `WSTR_INIT_ARRAY` / `WSTR_LOAD_ARRAY_ELEM` / `WSTR_STORE_ARRAY_ELEM` — array element access for WSTRING

For string functions (`LEN`, `FIND`, `CONCAT`, `LEFT`, `RIGHT`, `MID`, `INSERT`, `DELETE`, `REPLACE`):

- ADR-0004 specifies dispatch via BUILTIN func_id ranges (0x0100–0x010A for STRING, 0x0200–0x020A for WSTRING). The current implementation uses dedicated opcodes (`LEN_STR`, `FIND_STR`, etc.) instead. Adding `LEN_WSTR`, `FIND_WSTR`, etc. as dedicated opcodes is consistent with the existing pattern; migration to BUILTIN dispatch is out of scope for this ADR.

### Width Information Lives in the Opcode, Not the Data

The opcode encodes the width unambiguously:

- A STR_* opcode operating on offset `O` always interprets the bytes at `O` as a STRING (1 byte/code unit, Latin-1)
- A WSTR_* opcode operating on offset `O` always interprets the bytes at `O` as a WSTRING (2 bytes/code unit, UTF-16LE)

Width is never a runtime property of the data. The compiler emits the correct opcode based on the variable's declared type; the verifier confirms the operand types match the opcode family. This is the static guarantee ADR-0004 requires.

### Defense-in-Depth: Encoding Tag on Temp Buffers

ADR-0004 specifies a one-byte narrow/wide encoding tag attached to each entry in the (then-named) buffer table, asserted at every operation entry. The unified data region (ADR-0017) replaced the buffer table for *persistent* string storage, but the **temp buffer pool** (used by string ops to materialize intermediate results) remains and carries the same tagging requirement.

Each temp buffer slot gains a one-byte `encoding` field (0 = narrow, 1 = wide). Allocation records the encoding; STR_* operations assert `encoding == narrow` on entry; WSTR_* operations assert `encoding == wide` on entry. Mismatch traps. This catches:

- Codegen bugs that emit a STR_* opcode on a buf_idx produced by `LOAD_CONST_WSTR`
- Memory corruption that flips the tag (unlikely but cheap to detect)
- Verifier bypasses, should one ever exist

The cost is one byte per temp buffer slot (not per byte of string data) and one comparison per op entry — well below 1% of any string operation's runtime.

For string variables in the *data region*, no per-variable tag is needed: the data_offset is constant for the program's lifetime, the variable's declared type is constant, and the opcode emitted by codegen is therefore constant. Any STR_* opcode operating on a WSTRING variable's data_offset is a verifier-detectable codegen bug, not a runtime fault.

### Source-Code Sharing Pattern

Each string operation has a body parameterized on `char_width`:

```rust
fn do_str_store_var(
    char_width: usize,
    bytecode: &[u8], pc: &mut usize,
    stack: &mut Stack, data_region: &mut [u8], temp_buf: &[u8],
    max_temp_buf_bytes: usize,
) -> Result<(), Trap> {
    let data_offset = read_u32_le(bytecode, pc)? as usize;
    let buf_idx = stack.pop()?.as_i32() as usize;

    // ... shared logic that scales offsets by char_width ...
    // copy_bytes = min(src_cur_len, dest_max_len) * char_width
}

opcode::STR_STORE_VAR  => do_str_store_var(1, /* ... */)?,
opcode::WSTR_STORE_VAR => do_str_store_var(2, /* ... */)?,
```

Const-generic monomorphization (`fn do_str_store_var<const W: usize>(...)`) is an option if profiling later shows the runtime `char_width` parameter is hot, but is unnecessary up front. The two opcode handlers are one line each; the operation lives in one place.

`encode_string_literal` follows the same pattern: replace the current `unreachable!` arm with UTF-16LE encoding, gated on `char_width == 2`.

### Consequences

* Good, because the architecture realizes ADR-0004 directly — no exception, no deviation, no future migration debt
* Good, because the bytecode verifier can reject every STRING/WSTRING type-confusion attempt from the opcode stream alone, without runtime tag tracking (the eBPF verifier failure mode cited in ADR-0004)
* Good, because ADR-0015's 4-byte string header is preserved — no format-version bump, no breaking change to deployed tooling, no spec conformance test churn
* Good, because the temp buffer encoding tag provides ADR-0004's defense-in-depth at near-zero cost (one byte per slot, one comparison per op)
* Good, because the per-op runtime cost is identical to the existing STR_* family — there is no width-determination overhead because `char_width` is a compile-time constant from the dispatch site
* Good, because source-code deduplication via parameterized helpers means each string operation has one implementation; STR_* and WSTR_* handlers cannot drift apart through independent edits
* Neutral, because the opcode count grows by approximately 16 (one per STR_* opcode) — well within the budget noted in ADR-0004 (85 free slots)
* Neutral, because the codegen layer must select STR_* or WSTR_* based on `StringInitializer.width` / `StringSpecification.width` / `FunctionReturnType::WString` / `ArrayElementType::WString`; this is a switch on a field that already exists in the AST
* Bad, because adding ~16 opcodes increases the icache footprint of the dispatch loop slightly; mitigated by the parameterized-helper pattern, which keeps the actual code size growth small (each new opcode adds only a one-line dispatch arm)

### Confirmation

Verify by:

1. End-to-end execution test: WSTRING variable declaration, assignment from a literal, comparison, and `LEN`. Confirm the variable's data region holds UTF-16LE bytes.
2. End-to-end execution test: a program declaring both a STRING and a WSTRING with the same maximum length, exercising both. Confirm the results match the encoding-specific expected outputs.
3. Verifier test: a hand-crafted bytecode sequence that emits `STR_STORE_VAR` against a buf_idx produced by `LOAD_CONST_WSTR` — must trap with an encoding-mismatch error from the temp buffer tag check.
4. Source review: confirm that each string operation has exactly one body, with the STR_* and WSTR_* opcodes dispatching to it with `char_width = 1` and `char_width = 2`.

## Pros and Cons of the Options

### A. Polymorphic STR_* with Width in the Header

Extend the per-string header from 4 bytes to 6: `[max_length][cur_length][char_width][reserved]`. Each STR_* opcode reads the width from the header and scales offsets accordingly.

* Good, because the data is self-describing — corrupt or misrouted offsets are detectable at runtime
* Good, because no new opcodes are required
* Bad, because it amends ADR-0015 — the 4-byte header is referenced in spec conformance tests, the bytecode verifier, and the container format documentation
* Bad, because it forces a container format-version bump (2 → 3), invalidating any deployed bytecode
* Bad, because it directly contradicts ADR-0004: the VM would distinguish STRING from WSTRING via a runtime tag (the header field) rather than statically via the opcode — the eBPF verifier-bypass class of vulnerability that ADR-0004 explicitly rejects
* Bad, because every STR_* op gains a runtime branch on width (predictable and cheap, but unnecessary)

### B. Polymorphic STR_* with Width via Opcode Operand or Variable Tag

Keep one set of STR_* opcodes; pass width as an immediate operand on `STR_INIT` (or store it in a per-variable type tag in the slot table).

* Good, because no header change
* Good, because no new opcodes
* Bad, because the verifier must propagate width information through abstract interpretation to prove type safety — exactly the pattern ADR-0004 cites as the eBPF verifier failure mode
* Bad, because every consumer of a string offset must read width from somewhere (operand, slot table) — more bytecode bloat or runtime lookup
* Bad, because the temp buffer pool still needs encoding tags (since intermediate results have no slot table entry), so the implementation ends up paying for both mechanisms

### C. Parallel WSTR_* Opcode Family with Shared Helpers (chosen)

See "Decision Outcome" above.

### D. Two Separate Data Regions

A second, independent data region for WSTRING storage, with parallel opcode families addressing it.

* Good, because the regions cannot be cross-addressed at all
* Bad, because it duplicates ADR-0017's unified-region machinery without a corresponding safety gain (the parallel opcode families already provide static separation)
* Bad, because composability suffers — `ARRAY[1..10] OF WSTRING[20]` and `ARRAY[1..10] OF STRING[20]` would need separate region accounting; nested types containing both kinds become awkward
* Bad, because two regions means two `data_region_bytes` budgets, two allocator paths, more bookkeeping in the file header — directly opposing ADR-0017's simplification driver
* Bad, because the constant pool and temp buffer pool still need width awareness, so the duplication is not even total

## More Information

### Relationship to ADR-0004

ADR-0004 chose "three distinct type families" with separate STR_* and WSTR_* opcodes for variable access and distinct func_id ranges for string functions. This ADR is a direct realization of that choice for the variable-access opcode family. The string-function dispatch path (BUILTIN func_ids vs. dedicated opcodes) is consistent with the existing implementation pattern (dedicated opcodes); reconciling that with ADR-0004's BUILTIN-dispatch description is out of scope here.

### Relationship to ADR-0015 and ADR-0017

This ADR makes no changes to either. The 4-byte string header (ADR-0015) and the unified data region (ADR-0017) are unchanged. Width information is encoded in the opcode at compile time and verified by the bytecode verifier, never stored in the string header or the data region itself.

### Relationship to ADR-0016

ADR-0016 specifies that WSTRING uses UTF-16LE. This ADR is consistent: WSTR_* opcodes treat the bytes following the header as UTF-16LE code units. `encode_string_literal` produces UTF-16LE bytes for `char_width = 2`.

### What the Codegen Layer Must Do

1. In `compile_setup.rs`, when encountering an `InitialValueAssignmentKind::String(string_init)` with `string_init.width == StringType::WString`, emit `WSTR_INIT` (with the data region sized for `max_length * 2` bytes plus header) and tag the variable with `iec_type_tag::WSTRING`.
2. In `compile_expr.rs` and `compile_string.rs`, route literal encoding and operation emission through the WSTR_* family when the operand type is WSTRING.
3. In `encode_string_literal`, replace the `unreachable!` arm with UTF-16LE LE encoding (`(ch as u16).to_le_bytes()` per character).
4. In `compile_array.rs` and `compile_fn.rs`, use `StringSpecification.width` and `FunctionReturnType::WString` to select the correct opcode family.

### What the Analyzer Layer Must Do

`IntermediateType::String { max_len }` currently collapses both encodings into one variant. Add a `char_width: u8` field (or split into `String` and `WString` variants) so codegen can distinguish them when consuming the intermediate type. The user-visible test that asserts WSTRING becomes `IntermediateType::String` (`analyzer/src/intermediates/string.rs:62-82`) is updated to assert the width is preserved.
