# Spec: Bytecode Container Format

## Overview

This spec defines the binary container format for IronPLC bytecode files. The container packages compiled bytecode with metadata, type information, and cryptographic signatures into a single file that the VM loads and verifies before execution.

The format builds on:

- **[ADR-0006](../adrs/0006-bytecode-verification-requirement.md)**: Bytecode verification as a requirement — the VM must verify or signature-validate bytecode before execution
- **[ADR-0007](../adrs/0007-dual-signature-integrity-model.md)**: Dual-signature integrity model — content and debug sections have independent signatures
- **[Bytecode Instruction Set](bytecode-instruction-set.md)**: The instruction set this container packages

## Design Goals

1. **Fail-fast resource check** — the VM reads a fixed-size header and immediately knows whether it has enough RAM/flash to run the program, before allocating anything
2. **Streamable** — sections appear in a fixed order so the VM can process the file in a single forward pass
3. **Strippable** — the debug section can be removed without invalidating the content signature
4. **Self-describing** — the file contains all metadata the verifier needs; no external symbol tables or separate configuration files

## File Layout

Sections appear in this fixed order. All multi-byte values are little-endian, matching the instruction set encoding.

```
┌─────────────────────────────────────────┐  offset 0
│ File Header (256 bytes, fixed size)     │
├─────────────────────────────────────────┤  offset 256
│ Content Signature Section               │
├─────────────────────────────────────────┤
│ Debug Signature Section (optional)      │
├─────────────────────────────────────────┤
│ Type Section                            │
├─────────────────────────────────────────┤
│ Task Table Section                      │
├─────────────────────────────────────────┤
│ Constant Pool Section                   │
├─────────────────────────────────────────┤
│ Code Section                            │
├─────────────────────────────────────────┤
│ Debug Section (optional)                │
└─────────────────────────────────────────┘
```

## File Header

The header is exactly 256 bytes. The VM reads this in a single read and decides whether to proceed.

The header is organized into four logical regions:

1. **Identification** (bytes 0-7): magic, version, profile, flags
2. **Hashes** (bytes 8-135): content, source, debug, layout hashes
3. **Section directory** (bytes 136-191): offset/size pairs for each section, in file-layout order
4. **Runtime parameters** (bytes 192-231): stack/memory budgets, counts, I/O image sizes

| Offset | Field | Type | Description |
|--------|-------|------|-------------|
| 0 | magic | u32 | `0x49504C43` ("IPLC" in ASCII) |
| 4 | format_version | u16 | Container format version (initially 1) |
| 6 | profile | u8 | Reserved for future VM profile definitions; must be zero |
| 7 | flags | u8 | Bit 0: has content signature; Bit 1: has debug section; Bit 2: has type section |
| 8 | content_hash | [u8; 32] | SHA-256 over `source_hash \|\| type_section \|\| constant_pool \|\| code_section` (see Content Hash Scope) |
| 40 | source_hash | [u8; 32] | SHA-256 of the source text that produced this bytecode (all zeros if unavailable) |
| 72 | debug_hash | [u8; 32] | SHA-256 over debug section (all zeros if no debug section) |
| 104 | layout_hash | [u8; 32] | SHA-256 over the memory layout signature (see Layout Hash and Online Change) |
| 136 | sig_section_offset | u32 | Offset of content signature section (0 if absent) |
| 140 | sig_section_size | u32 | Size of content signature section |
| 144 | debug_sig_offset | u32 | Offset of debug signature section (0 if absent) |
| 148 | debug_sig_size | u32 | Size of debug signature section |
| 152 | type_section_offset | u32 | Offset of type section (0 if stripped) |
| 156 | type_section_size | u32 | Size of type section |
| 160 | task_section_offset | u32 | Offset of task table section (0 if absent; see [Task Support Design](../design/61131-task-support.md)) |
| 164 | task_section_size | u32 | Size of task table section |
| 168 | const_section_offset | u32 | Offset of constant pool section |
| 172 | const_section_size | u32 | Size of constant pool section |
| 176 | code_section_offset | u32 | Offset of code section |
| 180 | code_section_size | u32 | Size of code section |
| 184 | debug_section_offset | u32 | Offset of debug section (0 if absent) |
| 188 | debug_section_size | u32 | Size of debug section |
| 192 | max_stack_depth | u16 | Maximum operand stack depth across all functions |
| 194 | max_call_depth | u16 | Maximum call nesting depth |
| 196 | num_variables | u16 | Total variable table entries (including compiler-generated hidden variables) |
| 198 | num_fb_instances | u16 | Total function block instance slots (including array elements and nested instances) |
| 200 | total_fb_instance_bytes | u32 | Total bytes for FB instance memory (compiler-summed: `Σ(num_fields × 8)` for each instance) |
| 204 | total_str_var_bytes | u32 | Total bytes for STRING variable buffers (compiler-summed: `Σ(declared_length + 1)` for each STRING variable) |
| 208 | total_wstr_var_bytes | u32 | Total bytes for WSTRING variable buffers (compiler-summed: `Σ(declared_length × 2 + 2)` for each WSTRING variable) |
| 212 | num_temp_str_bufs | u16 | Temporary STRING buffer pool size |
| 214 | num_temp_wstr_bufs | u16 | Temporary WSTRING buffer pool size |
| 216 | max_str_length | u16 | Largest STRING(n) declaration in characters (for temp buffer sizing) |
| 218 | max_wstr_length | u16 | Largest WSTRING(n) declaration in characters (for temp buffer sizing) |
| 220 | num_functions | u16 | Number of function entries in the code section |
| 222 | num_fb_types | u16 | Number of FB type descriptors in the type section |
| 224 | num_arrays | u16 | Number of array descriptors in the type section |
| 226 | input_image_bytes | u16 | Total input process image size in bytes (%I) |
| 228 | output_image_bytes | u16 | Total output process image size in bytes (%Q) |
| 230 | memory_image_bytes | u16 | Total memory region size in bytes (%M) |
| 232 | reserved | [u8; 24] | Reserved for future use; must be zero |

Total header size: 256 bytes.

### Resource Budget Calculation

The VM uses the resource summary fields to compute the total RAM requirement before allocating:

```
ram_required =
    (max_stack_depth × slot_size)               // operand stack
  + (max_call_depth × frame_size)               // call stack
  + (num_variables × variable_slot_size)         // variable table
  + total_fb_instance_bytes                       // FB instance table (compiler-summed; includes array-of-FB elements)
  + total_str_var_bytes                          // STRING variable buffers (compiler-summed)
  + total_wstr_var_bytes                         // WSTRING variable buffers (compiler-summed)
  + (num_temp_str_bufs × (max_str_length + 1))   // temporary STRING buffers (worst-case per temp)
  + (num_temp_wstr_bufs × (max_wstr_length × 2 + 2)) // temporary WSTRING buffers (worst-case per temp)
  + input_image_bytes                            // input process image snapshot
  + output_image_bytes                           // output staging buffer
  + memory_image_bytes                           // memory region (%M)
```

If `ram_required` exceeds available RAM, the VM rejects the program at load time with a clear error, before allocating anything.

String buffers use a length-prefix format with no null terminator. The length prefix is the sole indicator of string extent. See the [Runtime Execution Model](runtime-execution-model.md) for the full memory budget and string buffer lifecycle.

## Content Signature Section

Present when `flags` bit 0 is set. The PLC rejects bytecode without a content signature (ADR-0006, ADR-0007).

| Offset | Field | Type | Description |
|--------|-------|------|-------------|
| 0 | algorithm | u8 | 0=Ed25519, 1=ECDSA-P256 |
| 1 | key_id_length | u8 | Length of key identifier (0–64) |
| 2 | key_id | [u8; N] | Key identifier (N = key_id_length); used to select the verification key |
| 2+N | signature | [u8; 64] | Signature over `content_hash` from the file header |

The `key_id` is an opaque identifier that the VM uses to look up the corresponding public key from its key store. The key store configuration is a deployment concern outside this spec.

## Debug Signature Section

Present when both `flags` bit 0 and bit 1 are set. Same format as the content signature section, but signs `debug_hash` instead of `content_hash`. May use a different algorithm or key than the content signature.

## Type Section

Present when `flags` bit 2 is set. Required for on-device verification (ADR-0006). May be stripped for constrained targets using the signature fallback.

The type section contains metadata used by the verifier for type safety checking. The interpreter does not read this section — it uses pre-computed indices from the compiler.

### Variable Table

The variable table describes the type of each variable slot. The verifier uses types to check that LOAD_VAR/STORE_VAR opcodes use the correct typed variant.

| Offset | Field | Type | Description |
|--------|-------|------|-------------|
| 0 | count | u16 | Number of variable entries (must match header `num_variables`) |
| 2 | entries | [VarEntry; count] | Variable descriptors |

Each VarEntry (4 bytes, fixed size):

| Offset | Field | Type | Description |
|--------|-------|------|-------------|
| 0 | var_type | u8 | 0=I32, 1=U32, 2=I64, 3=U64, 4=F32, 5=F64, 6=STRING, 7=WSTRING, 8=FB_INSTANCE, 9=TIME |
| 1 | flags | u8 | Bit 0: is array (see array descriptors) |
| 2 | extra | u16 | For STRING/WSTRING: max length. For FB_INSTANCE: fb_type_id. For arrays: array descriptor index. |

Variable indices are compiler-assigned. The compiler must produce deterministic indices across compilations using the ordering rules in [Deterministic Ordering](#deterministic-ordering) to ensure that the same source program (with only logic changes) produces compatible bytecode.

### Array Descriptors

Each array descriptor defines the element type and bounds for one array variable.

| Offset | Field | Type | Description |
|--------|-------|------|-------------|
| 0 | element_type | u8 | Element type (same encoding as VarEntry.var_type) |
| 1 | reserved | u8 | Reserved; must be zero |
| 2 | lower_bound | i16 | Array lower bound (IEC 61131-3 arrays can have arbitrary lower bounds) |
| 4 | upper_bound | i16 | Array upper bound (inclusive) |
| 6 | element_extra | u16 | For STRING/WSTRING elements: max length. For FB elements: fb_type_id. |

The verifier checks that every LOAD_ARRAY/STORE_ARRAY `type` byte matches the array's `element_type`.

### FB Type Descriptors

Each FB type descriptor defines the field layout for a function block type.

| Offset | Field | Type | Description |
|--------|-------|------|-------------|
| 0 | type_id | u16 | Unique type ID (matches FB_CALL operand) |
| 2 | num_fields | u8 | Number of fields |
| 3 | reserved | u8 | Reserved; must be zero |
| 4 | fields | [FieldEntry; num_fields] | Field descriptors |

Each FieldEntry (4 bytes, fixed size):

| Offset | Field | Type | Description |
|--------|-------|------|-------------|
| 0 | field_type | u8 | Field type (same encoding as VarEntry.var_type) |
| 1 | reserved | u8 | Reserved; must be zero |
| 2 | field_extra | u16 | For STRING/WSTRING: max length in characters. For FB_INSTANCE: nested fb_type_id. For other types: 0. |

The verifier checks that every FB_STORE_PARAM/FB_LOAD_PARAM `field` index is within `num_fields` for the target FB type.

Type IDs and field indices are compiler-assigned. The compiler must produce deterministic assignments across compilations using the ordering rules in [Deterministic Ordering](#deterministic-ordering).

### Function Signatures

Each function signature describes the parameter and return types for a function.

| Offset | Field | Type | Description |
|--------|-------|------|-------------|
| 0 | function_id | u16 | Function ID (matches CALL operand) |
| 2 | num_params | u8 | Number of parameters |
| 3 | return_type | u8 | Return type (same encoding as var_type); 0xFF = void |
| 4 | param_types | [u8; num_params] | Parameter types |

## Constant Pool Section

The constant pool stores literal values referenced by LOAD_CONST_* instructions.

| Offset | Field | Type | Description |
|--------|-------|------|-------------|
| 0 | count | u16 | Number of constant entries |
| 2 | entries | [ConstEntry; count] | Constant values |

Each ConstEntry:

| Offset | Field | Type | Description |
|--------|-------|------|-------------|
| 0 | const_type | u8 | 0=I32, 1=U32, 2=I64, 3=U64, 4=F32, 5=F64, 6=STRING_LITERAL, 7=WSTRING_LITERAL |
| 1 | reserved | u8 | Reserved; must be zero |
| 2 | size | u16 | Size of value in bytes (4, 8, or variable for strings; u16 to accommodate string literals exceeding 255 bytes) |
| 4 | value | [u8; size] | Little-endian value bytes. For strings: u16 length prefix followed by character bytes. |

The verifier checks that every LOAD_CONST_* index is within `count` and that the constant type matches the opcode variant (e.g., LOAD_CONST_I32 references a type-0 entry).

## Code Section

The code section contains the bytecode for all functions and FB bodies.

| Offset | Field | Type | Description |
|--------|-------|------|-------------|
| 0 | function_directory | [FuncEntry; num_functions] | Directory of function entry points |
| varies | bytecode_bodies | [u8; ...] | Concatenated bytecode bodies |

Each FuncEntry:

| Offset | Field | Type | Description |
|--------|-------|------|-------------|
| 0 | function_id | u16 | Function ID |
| 2 | bytecode_offset | u32 | Offset from start of bytecode_bodies |
| 6 | bytecode_length | u32 | Length of this function's bytecode in bytes |
| 10 | max_stack_depth | u16 | Maximum operand stack depth for this function |
| 12 | num_locals | u16 | Number of local variable slots |

The per-function `max_stack_depth` allows the verifier to check stack bounds per-function. The header's `max_stack_depth` is the maximum across all functions.

## Debug Section

Present when `flags` bit 1 is set. Can be stripped without invalidating the content signature. Has its own signature (debug signature section) when present.

| Offset | Field | Type | Description |
|--------|-------|------|-------------|
| 0 | source_text_length | u32 | Length of embedded source text (0 if not included) |
| 4 | source_text | [u8; N] | UTF-8 source text (optional) |
| 4+N | line_map_count | u16 | Number of line mapping entries |
| 6+N | line_maps | [LineMapEntry; count] | Source line mappings |
| varies | var_name_count | u16 | Number of variable name entries |
| varies | var_names | [VarNameEntry; count] | Variable name mappings |

Each LineMapEntry:

| Offset | Field | Type | Description |
|--------|-------|------|-------------|
| 0 | function_id | u16 | Function containing this mapping |
| 2 | bytecode_offset | u16 | Offset within the function's bytecode |
| 4 | source_line | u16 | Source line number (1-based) |

Each VarNameEntry:

| Offset | Field | Type | Description |
|--------|-------|------|-------------|
| 0 | var_index | u16 | Variable table index |
| 2 | name_length | u8 | Length of variable name |
| 3 | name | [u8; name_length] | UTF-8 variable name |

## Loading Sequence

The VM loads a bytecode container in this order:

```
1. Read file header (256 bytes)
2. Validate magic number ("IPLC")
3. Check format_version is supported
4. Compute RAM requirement from resource summary
5. If RAM insufficient → reject with "insufficient resources" error
6. Verify content signature:
   a. Read content signature section
   b. Look up public key by key_id
   c. Verify signature over content_hash
   d. If invalid → reject with "signature verification failed" error
7. Read type + constant + code sections
8. Compute SHA-256 over source_hash || type + constant + code sections
9. Compare computed hash to content_hash in header
    If mismatch → reject with "content hash mismatch" error
10. If on-device verification is enabled:
    a. Run bytecode verifier using type section metadata
    b. If verification fails → reject with specific verifier error
11. Mark bytecode memory as read-only (if platform supports it)
12. Allocate runtime resources (stack, variable table, buffers)
13. If debug section present and debug signature present:
    a. Verify debug signature over debug_hash
    b. Compute SHA-256 over debug section
    c. Compare to debug_hash in header
    d. If valid → load debug info; if invalid → discard debug info (non-fatal)
14. Begin execution
```

Steps 6–9 are the minimum for constrained targets (signature fallback). Step 10 adds on-device verification for capable targets. Step 13 is optional — invalid debug info is discarded, not fatal.

## Content Hash Scope

The content hash covers the type section, constant pool, and code section in file order. It does **not** cover:

- The file header itself (the header contains the hash, so including it would be circular)
- The signature sections (signatures are over the hash, not the other way around)
- The debug section (independently hashed and signed)

The source hash is embedded in the file header. Since the content hash covers the hash value (via the signed content_hash → header binding at the signature level), the source hash is transitively integrity-protected: modifying the source hash requires re-signing the content.

Note: The content hash does not directly cover the header bytes. Instead, the content signature signs the content_hash value, and the VM verifies that the content_hash in the header matches the actual hash of the type+constant+code sections (step 10 in the loading sequence). The source_hash is protected because it is part of the signed-over content_hash only if the signer includes the source_hash in the hash computation. To make this binding explicit: the content_hash is computed as `SHA-256(source_hash || type_section_bytes || const_section_bytes || code_section_bytes)`. This ensures the source_hash is covered by the content signature.

## Deterministic Ordering

The compiler must assign all numeric indices (variable indices, FB type IDs, field indices, function IDs) in a deterministic order derived from the source program's declarations. This ensures that two compilations of the same program — differing only in logic — produce identical type sections and variable tables, which in turn produce identical `layout_hash` values.

### Ordering rules

| Item | Sort key | Tie-breaking |
|------|----------|-------------|
| Global variables | Qualified name (lexicographic, UTF-8 byte order) | N/A (names must be unique) |
| Program-local variables | Program name, then variable name | N/A |
| FB type descriptors | Qualified type name (lexicographic) | N/A (type names must be unique) |
| Fields within an FB type | Declaration order in source | N/A (declaration order is deterministic) |
| Function signatures | Qualified function name (lexicographic) | N/A |
| Array descriptors | Order of first referencing variable | N/A |
| Constant pool entries | Order of first reference in bytecode | N/A |

The compiler assigns indices 0, 1, 2, ... in the sorted order. Because the sort key is derived entirely from the source declarations (names and types), adding or removing a variable, FB type, or field changes the indices of subsequent items. This is intentional — any structural change invalidates the `layout_hash`, forcing a full restart rather than silently reinterpreting memory.

### What counts as a "logic-only" change

A change is logic-only (and produces a matching `layout_hash`) if and only if:
- No variables are added, removed, renamed, or retyped
- No FB types are added, removed, or have their fields changed
- No array bounds are changed
- Function bodies, constant values, and control flow may change freely

## Layout Hash and Online Change

The `layout_hash` in the file header enables safe online change (hot reloading) of bytecode without restarting the VM or losing variable state.

### Hash computation

The layout hash is computed as:

```
layout_hash = SHA-256(
    num_variables (u16, LE) ||
    for each variable in index order:
        var_type (u8) || flags (u8) || extra (u16, LE) ||
    num_fb_types (u16, LE) ||
    for each FB type in type_id order:
        num_fields (u8) ||
        for each field in field index order:
            field_type (u8) || field_extra (u16, LE) ||
    num_arrays (u16, LE) ||
    for each array descriptor in index order:
        element_type (u8) || lower_bound (i16, LE) ||
        upper_bound (i16, LE) || element_extra (u16, LE)
)
```

The hash covers all information that determines memory layout. It excludes code, constants, and debug info — those can change freely without affecting variable memory.

### Online change protocol

When the VM receives new bytecode while running:

```
1. Read new file header
2. Compare new layout_hash to current layout_hash
3. If hashes match:
   a. Verify new bytecode (signature + optional verifier)
   b. At the end of the current scan cycle (after OUTPUT_FLUSH):
      - Swap code section to new bytecode
      - Keep all variable, FB instance, and process image memory intact
      - Resume execution with new code on next scan cycle
4. If hashes differ:
   a. Reject the online change with "layout incompatible" error
   b. The operator must perform a full stop-load-start sequence
```

The swap occurs at a safe point (between scan cycles) to ensure the program never executes a mix of old and new code within a single scan.

### Why compiler-determined ordering is sufficient

This design relies on the compiler producing deterministic output rather than embedding names in the container for runtime matching. This is the right trade-off because:

- **Logic-only changes are the common case** — most PLC online changes modify function bodies while keeping the same variables and FB types
- **All-or-nothing is simple and safe** — if any declaration changes, the hash changes, and the VM requires a full restart; there is no partial migration that could silently corrupt data
- **Smaller container format** — no variable names, type names, or field names in the type section; names belong in the debug section
- **Simpler runtime** — one 32-byte hash comparison replaces O(n) name matching and per-item layout checking

Per-variable migration (adding a variable while preserving others) is an advanced feature that can be added later if needed by extending the type section with optional name metadata.

## Versioning

The `format_version` field allows future changes to the container format. The VM must reject versions it does not support. Version 1 is defined by this spec.

Rules for version increments:
- Adding new optional sections → minor version (backward compatible)
- Changing header layout or section semantics → major version (not backward compatible)
