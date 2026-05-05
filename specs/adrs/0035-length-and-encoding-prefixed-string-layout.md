# Length-and-Encoding-Prefixed String Memory Layout

status: proposed
date: 2026-05-05
supersedes: ADR-0015

## Context and Problem Statement

ADR-0015 defined the per-string memory layout as `[max_length: u16][cur_length: u16][data]` — a 4-byte header followed by character data. Lengths were specified as code units, with the implicit assumption that the encoding (and therefore the code-unit size) was known to every operation through some external mechanism — originally, a one-byte tag on each entry of the buffer table referenced by ADR-0004.

Two architectural changes since that decision require revisiting the layout:

1. **ADR-0017 dissolved the buffer table** that originally held per-buffer encoding metadata.
2. **ADR-0034 chose a single STR_\* opcode family** for both STRING and WSTRING, requiring the data itself to identify its encoding so that operations can scale offsets correctly and the runtime can detect type confusion.

The string layout must now answer: how does any operation, given only an offset into the data region, determine whether the bytes at that offset are Latin-1 (STRING) or UTF-16LE (WSTRING)?

## Decision Drivers

All drivers from ADR-0015 still apply:

* **Safety** — buffer overflows or unbounded reads can cause physical damage (ADR-0005)
* **Deterministic performance** — string operations must have predictable, bounded execution time
* **Bounded memory** — all memory must be statically computable at compile time; no heap allocation or garbage collection
* **IEC 61131-3 semantics** — both maximum and current length must be queryable

Plus one new driver:

* **Self-describing data for defense-in-depth** — operations on a string must be able to verify the data's encoding matches the operation's expectation without relying on external metadata that could be stale, corrupted, or bypassed (per ADR-0034's safety chain)

## Considered Options

* 4-byte header with encoding tracked externally (ADR-0015 original)
* 5-byte header with `char_width` as a `u8`
* 6-byte header with `char_width` as a `u16`

## Decision Outcome

**Chosen option: 6-byte header with `char_width` as a `u16`.**

The memory layout for a string variable is:

```
Offset  Size     Field
0       2 bytes  max_length (u16, declared capacity in code units)
2       2 bytes  cur_length (u16, current content length in code units)
4       2 bytes  char_width (u16, bytes per code unit: 1 for STRING, 2 for WSTRING)
6       n bytes  data (character content, not null-terminated)
```

Total size: `n * char_width + 6` bytes, fully determined at compile time.

The new `char_width` field carries the encoding distinction:

- `char_width = 1`: data is Latin-1 (STRING per ADR-0016)
- `char_width = 2`: data is UTF-16LE (WSTRING per ADR-0016)

`char_width` is set once during variable allocation and never modified — same lifecycle as `max_length`. Operations that read or write the string must verify `char_width` matches their expectation; mismatch traps.

`cur_length` and `max_length` remain in code units (characters), not bytes. The byte length of the data region is `cur_length * char_width`.

For `STRING[n]`, total size is `n + 6` bytes. For `WSTRING[n]`, total size is `n * 2 + 6` bytes.

The maximum value of `n` is 65,535 (the range of `u16`), unchanged from ADR-0015 and consistent with the IEC 61131-3 Third Edition upper bound for string length declarations.

### Why a `u16` Field Instead of a `u8`

A `u8` `char_width` would save one byte per string. The `u16` choice is preferred because:

- It maintains 2-byte alignment for the data region. On targets with strict alignment requirements, this avoids per-character unaligned access for WSTRING data.
- It leaves headroom for future encodings without a layout change.
- The savings of one byte per string is negligible against typical string sizes (20–254 code units of payload).

### Consequences

* Good, because the string is now fully self-describing — encoding, capacity, and length all live in the header
* Good, because defense-in-depth against type confusion is enforced by the data layout itself, not external metadata (per ADR-0034)
* Good, because the layout is uniform between STRING and WSTRING — code reading a string header does not need to know the encoding in advance
* Good, because the data region remains 2-byte aligned, supporting efficient WSTRING access on alignment-sensitive targets
* Good, because every other property from ADR-0015 is preserved — O(1) length and capacity queries, statically computable size, no heap allocation
* Neutral, because the per-string overhead grows from 4 to 6 bytes; for typical string lengths (20–254 code units of payload), the relative overhead is small
* Neutral, because `cur_length` and `max_length` u16 cap at 65,535 code units — unchanged from ADR-0015
* Bad, because the container format version must increment to reflect the new header size — but ADR-0015 was `status: proposed` with no deployed bytecode, so the cost is documentation and test-fixture updates only

### Assignment Semantics

On string assignment (`dest := source`), in addition to the steps from ADR-0015:

0. Verify `source.char_width == dest.char_width`; trap on mismatch (defense-in-depth — the verifier should have rejected this statically per ADR-0034, but the runtime check enforces the invariant)
1. Read `source.cur_length`
2. Compute `copy_length = min(source.cur_length, dest.max_length)` (in code units)
3. Copy `copy_length * char_width` bytes from `source.data` to `dest.data`
4. Set `dest.cur_length = copy_length`

If `source.cur_length > dest.max_length`, the value is silently truncated. This matches IEC 61131-3 semantics where string operations produce results bounded by the destination's declared length — unchanged from ADR-0015.

### Invariants

For every string variable at all times:

1. `max_length` is set once during allocation and never modified
2. `char_width ∈ {1, 2}`, set once during allocation and never modified
3. `0 <= cur_length <= max_length`
4. Only bytes `data[0..cur_length * char_width]` contain valid encoded content
5. The VM must verify `char_width` matches the operation's expected encoding before any read or write

## Pros and Cons of the Options

### 4-Byte Header (ADR-0015 original)

`[max_length: u16][cur_length: u16][data]` — encoding determined by external context.

* Good, because the per-string overhead is 33% smaller (4 vs 6 bytes)
* Bad, because the encoding must be tracked externally — by separate opcode families (ADR-0004), per-buffer tags in a buffer table (no longer exists per ADR-0017), or per-variable type information
* Bad, because operations that consume a string offset must look up encoding via a side channel; corruption of that side channel silently misinterprets data
* Bad, because under ADR-0034 (single opcode family with operand typing), the verifier and runtime have no per-string source of encoding truth without it

### 5-Byte Header with `char_width` as `u8`

`[max_length: u16][cur_length: u16][char_width: u8][data]`.

* Good, because saves one byte per string vs. the chosen 6-byte layout
* Bad, because the data region begins at offset 5, breaking 2-byte alignment for WSTRING code-unit access on alignment-sensitive targets
* Bad, because the saving is negligible against typical string sizes

### 6-Byte Header with `char_width` as `u16` (chosen)

See "Decision Outcome" above.

## More Information

### Relationship to ADR-0017

This ADR defines the per-string layout. ADR-0017 defines where strings are stored: at compile-assigned offsets within the unified data region. The relationship is unchanged from ADR-0015's relationship to ADR-0017; only the per-string layout has grown by 2 bytes.

### Relationship to ADR-0034

ADR-0034 chose a single STR_* opcode family and specified that runtime encoding tags live on string headers, constant pool entries, and temp buffer slots. This ADR provides the string header tag; ADR-0034 specifies how the VM uses it.

### Relationship to ADR-0016

ADR-0016 specifies the encoding for each `char_width` value: Latin-1 for `char_width = 1`, UTF-16LE for `char_width = 2`. The layout defined here applies uniformly to both encodings.

### Migration from ADR-0015

ADR-0015 was `status: proposed` with no deployed bytecode. The 6-byte layout supersedes the 4-byte layout in implementation; no migration of existing data is required. Test fixtures that hardcode the 4-byte header (e.g., `vm/src/string_ops.rs` tests) must be updated to the 6-byte layout.

### Container Format Version

The container format version increments to reflect the new header size. Bytecode containers produced under ADR-0015's 4-byte layout cannot be loaded by a VM expecting ADR-0035's 6-byte layout, and vice versa. This is a one-time break with no deployed users to migrate.
