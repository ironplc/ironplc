# Length-Prefixed String Memory Layout

status: proposed
date: 2026-03-07

## Context and Problem Statement

IEC 61131-3 defines STRING and WSTRING as variable-length character sequences with a declared maximum length:

```
VAR
  name : STRING[20] := 'hello';
  label : STRING;            (* default max length *)
END_VAR
```

The compiler and VM need a memory layout for string variables. The layout must support:

- Declaration with a maximum length
- Assignment that may be shorter than the maximum
- O(1) queries for both maximum capacity and current length
- Bounds enforcement to prevent buffer overflows

How should string variables be represented in memory?

## Decision Drivers

* **Safety** -- PLC programs control physical processes; a buffer overflow or unbounded memory access can cause physical damage (ADR-0005)
* **Deterministic performance** -- string operations must have predictable, bounded execution time for scan cycle guarantees
* **Bounded memory** -- PLC runtimes run on resource-constrained targets without heap allocation or garbage collection; all memory must be statically determinable at compile time
* **IEC 61131-3 semantics** -- strings have a declared maximum length and a current length that varies at runtime; both must be queryable

## Considered Options

* Null-terminated (C-style)
* Length-prefixed with current length only
* Length-prefixed with maximum length and current length

## Decision Outcome

Chosen option: "Length-prefixed with maximum length and current length", because it provides O(1) access to both capacity and content length, enables bounds checking without external metadata, and prevents buffer overflows by construction.

The memory layout for a `STRING[n]` variable is:

```
Offset  Size     Field
0       2 bytes  max_length (u16, declared capacity in code units)
2       2 bytes  cur_length (u16, current content length in code units)
4       n bytes  data (character content, not null-terminated)
```

Total size: `n + 4` bytes, fully determined at compile time.

The `max_length` field is set once during variable allocation and never modified. The `cur_length` field is updated on every write operation. The `data` region contains exactly `cur_length` valid code units; bytes beyond `cur_length` are undefined and must not be read.

For `STRING`, one code unit is one byte. For `WSTRING`, one code unit is two bytes, so the data region is `n * 2` bytes and total size is `n * 2 + 4` bytes.

The maximum value of `n` is 65,535 (the range of `u16`), which matches the IEC 61131-3 Third Edition upper bound for string length declarations.

### Consequences

* Good, because `cur_length` provides O(1) length queries -- no scanning required
* Good, because `max_length` enables bounds checking at the point of every write without external metadata -- the string is self-describing
* Good, because the total size is statically computable from the declaration, enabling compile-time memory allocation with no heap
* Good, because truncation is detectable: after assignment, `cur_length < max_length` of the source indicates the value was truncated to fit the destination
* Good, because the 4-byte header is small relative to typical string content
* Good, because `u16` fields match the IEC 61131-3 maximum string length of 65,535
* Neutral, because 4 bytes of overhead per string variable is slightly more than a single-length-field design (2 bytes more)
* Bad, because `max_length` is redundant with information the compiler already knows -- it exists purely as a runtime safety check

### Assignment Semantics

On string assignment (`dest := source`):

1. Read `source.cur_length`
2. Compute `copy_length = min(source.cur_length, dest.max_length)`
3. Copy `copy_length` code units from `source.data` to `dest.data`
4. Set `dest.cur_length = copy_length`

If `source.cur_length > dest.max_length`, the value is silently truncated. This matches IEC 61131-3 semantics where string operations produce results bounded by the destination's declared length.

### Default Maximum Length

When no length is specified (`x : STRING`), the compiler uses a default maximum length. IEC 61131-3 does not mandate a specific default; implementations commonly use values between 80 and 254. The choice of default is outside the scope of this ADR.

## Pros and Cons of the Options

### Null-Terminated (C-Style)

Store character data followed by a zero byte. The end of the string is marked by the first zero-valued code unit.

* Good, because no header overhead -- all bytes are content (plus one terminator)
* Good, because interoperability with C APIs is trivial
* Bad, because length queries require scanning the entire string -- O(n), not O(1)
* Bad, because O(n) length queries make execution time dependent on string content, violating deterministic performance requirements
* Bad, because a missing or corrupted terminator causes unbounded reads beyond the allocated region -- a safety hazard
* Bad, because embedded zero bytes are not representable -- the string is silently truncated at the first zero
* Bad, because bounds checking on write requires external metadata (the allocated size must be tracked separately from the string itself)

### Length-Prefixed with Current Length Only

Store the current content length followed by character data. No maximum length field.

```
[cur_length: u16] [data: n bytes]
```

* Good, because O(1) length queries
* Good, because lower overhead than the two-field design (2 bytes instead of 4)
* Bad, because bounds checking on write requires the compiler or VM to maintain a separate mapping from variable to maximum capacity -- the string alone is not self-describing
* Bad, because a bug in the external capacity tracking leads to buffer overflow with no defense-in-depth -- the string cannot protect itself
* Bad, because the bytecode verifier cannot validate string bounds from the string alone; it must cross-reference external metadata

### Length-Prefixed with Maximum Length and Current Length (chosen)

Store both the declared maximum capacity and the current content length, followed by character data.

```
[max_length: u16] [cur_length: u16] [data: n bytes]
```

* Good, because O(1) for both length and capacity queries
* Good, because the string is self-describing -- bounds checking requires no external metadata
* Good, because defense-in-depth: even if the compiler has a bug, the VM can independently verify that `cur_length <= max_length` before any write
* Good, because the bytecode verifier can validate string operations using only the string's own header
* Good, because `max_length` is immutable after allocation, providing a stable invariant that simplifies reasoning about correctness
* Neutral, because 4 bytes of overhead per string is modest for typical string lengths (20-254 bytes of content)
* Bad, because `max_length` duplicates information the compiler already has -- it is redundant by design, trading 2 bytes of memory for runtime safety

## More Information

### Storage Location: Unified Data Region

String variables are stored in the unified data region (ADR-0017). Each STRING or WSTRING variable occupies a contiguous block at a compiler-assigned `data_offset` within the data region. The variable's slot table entry holds this `data_offset` value. The memory layout defined in this ADR (`[max_length][cur_length][data]`) describes the bytes at that offset.

### Relationship to the Variable Table

This ADR defines the memory layout of an individual string value. The slot table entry for a string variable holds a `data_offset` (byte offset into the unified data region) where the string's `[max_length][cur_length][data]` layout begins. See ADR-0017 for the unified data region design.

### WSTRING Encoding

IEC 61131-3 Third Edition specifies WSTRING as UTF-16. The byte order (little-endian vs. big-endian) within WSTRING code units is an implementation decision outside the scope of this ADR. The memory layout defined here applies to both STRING and WSTRING; only the code unit size differs (1 byte vs. 2 bytes).

### Invariants

The following invariants hold for every string variable at all times:

1. `max_length` is set once during allocation and never modified
2. `0 <= cur_length <= max_length`
3. Only bytes `data[0..cur_length]` (in code units) contain valid content
4. The VM must verify `cur_length <= max_length` before any write that updates `cur_length`
