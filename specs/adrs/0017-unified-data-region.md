# Unified Data Region for Variable-Length Types

status: proposed
date: 2026-03-07

## Context and Problem Statement

The bytecode container header originally defined separate resource fields for each variable-length data type:

- `total_str_var_bytes` / `total_wstr_var_bytes` — string buffers
- `num_fb_instances` / `total_fb_instance_bytes` — function block instance memory
- `num_arrays` — array descriptors (with element storage implied)
- `num_temp_str_bufs` / `num_temp_wstr_bufs` / `max_str_length` / `max_wstr_length` — temporary string buffers

This per-type design prevents composability: an `ARRAY[1..10] OF STRING[20]` requires a single contiguous region that mixes array indexing with string storage, but the current header has no way to express this. Each new composable type combination would require additional header fields.

How should the VM allocate memory for variable-length data types?

## Decision Drivers

* **Composability** — IEC 61131-3 allows arrays of strings, structs containing strings, and nested function blocks; the memory model must support arbitrary nesting
* **Fail-fast resource check** — the VM must determine total memory requirements from the header alone, before allocating anything (existing design goal from the container format spec)
* **Simplicity** — fewer header fields and fewer allocation regions reduce implementation complexity and the surface area for bugs
* **Safety** — all memory must be statically determined at compile time; no heap allocation during scan cycles (ADR-0005)

## Decision Outcome

All mutable variable-length data lives in a single contiguous **data region**. The compiler computes the total size and emits it as a single `data_region_bytes` field in the container header.

### Three Storage Tiers

After this change, the VM manages three storage tiers:

1. **Slot table** — 64-bit slots for scalar values (unchanged). Each slot holds an integer, float, boolean, or a `data_offset` value that points into the data region.

2. **Data region** — a single contiguous byte region for all mutable variable-length data. STRING and WSTRING variables are stored here using the layout defined in ADR-0015 (`[max_length: u16][cur_length: u16][data]`). Future extensions (arrays, structs, FB instances) will also use this region.

3. **Constant pool** — read-only constants including string literals (unchanged). String literals are loaded via LOAD_CONST_STR / LOAD_CONST_WSTR from the constant pool into temporary buffers.

### How It Works

For each variable-length variable, the compiler assigns a `data_offset` — a byte offset into the data region where that variable's data begins. The slot table entry for the variable stores this `data_offset` value. String opcodes (STR_LOAD_VAR, STR_STORE_VAR, etc.) use the `data_offset` operand to locate the variable's data within the region.

The compiler sums all variable-length allocations to produce `data_region_bytes`. The VM allocates this many bytes at load time and rejects the program if insufficient memory is available.

### Header Changes

The following per-type fields are removed:

- `num_fb_instances` (u16)
- `total_fb_instance_bytes` (u32)
- `total_str_var_bytes` (u32)
- `total_wstr_var_bytes` (u32)
- `num_temp_str_bufs` (u16)
- `num_temp_wstr_bufs` (u16)
- `max_str_length` (u16)
- `max_wstr_length` (u16)
- `num_arrays` (u16)

Replaced by:

- `data_region_bytes` (u32) — total size of the mutable data region in bytes
- `num_temp_bufs` (u16) — number of temporary buffers for string operations
- `max_temp_buf_bytes` (u32) — size of the largest temporary buffer in bytes

### Consequences

* Good, because composability is enabled by construction — `ARRAY[1..10] OF STRING[20]` is just a contiguous block of 10 string layouts at known offsets within the data region
* Good, because the VM allocates a single region instead of multiple typed regions — simpler allocation, simpler lifetime management
* Good, because the header shrinks from 9 per-type fields to 3 unified fields — less surface area for inconsistency
* Good, because the fail-fast resource check still works — `data_region_bytes` provides the total requirement
* Neutral, because the compiler must compute `data_offset` values and sum sizes — this is straightforward compile-time arithmetic
* Bad, because the VM loses per-type size information — it cannot independently verify that (for example) the total STRING allocation matches the number of STRING variables; this validation moves to the bytecode verifier

## More Information

### Relationship to ADR-0015

ADR-0015 defines the memory layout of an individual string value: `[max_length: u16][cur_length: u16][data]`. This ADR defines *where* that layout lives: at a `data_offset` within the unified data region. The per-string layout is unchanged.

### Relationship to ADR-0016

ADR-0016 defines the character encoding for STRING (Latin-1) and WSTRING (UTF-16LE). The data region stores encoded character data using these encodings. The encoding choice is orthogonal to the storage location.

### Temporary Buffers

String operations that produce intermediate results (e.g., CONCAT, LEFT) write into temporary buffers. The header's `num_temp_bufs` and `max_temp_buf_bytes` fields size the temporary buffer pool. Temporary buffers are allocated separately from the data region because they have different lifetimes: temporary buffers are transient within an expression, while data region contents persist across scan cycles.

### Migration Path

Since no per-type regions are currently implemented in the VM (the header fields exist but are all zero), this migration is non-breaking. The header byte layout changes, but no deployed containers exist that use the old fields.
