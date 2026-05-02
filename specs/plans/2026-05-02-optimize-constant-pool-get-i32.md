# Plan: Optimize ConstantPool primitive lookups

## Goal

`ConstantPool::get_i32` (and the sibling `get_f32`/`get_f64`/`get_i64`) sit on
the VM hot path: every `LOAD_CONST_*` opcode and every `CMP_BR_*` comparison
calls one of them. Profiling shows `get_i32` accounts for roughly 8% of VM time
on constant-heavy workloads, with most of that cost coming from the storage
layout rather than the arithmetic.

`ConstEntry` today stores `value: Vec<u8>`. A primitive read therefore does:

1. Bounds-check the outer entry vector.
2. Pointer-chase to the `ConstEntry` (loads `const_type`, plus the Vec's `ptr`,
   `len`, `cap`).
3. Pointer-chase **again** through the inner Vec's heap pointer to the bytes.
4. `copy_from_slice` + `from_le_bytes`.

Step 3 is the painful one: the bytes live in a separately heap-allocated buffer,
so every primitive lookup eats an unrelated cache line. The fix is to inline the
primitive bytes directly into `ConstEntry`, removing the inner pointer chase.
Strings stay on the heap (they don't fit and aren't on the hot path).

## Approach

Replace the single `Vec<u8>` field with split storage:

```rust
pub struct ConstEntry {
    pub const_type: ConstType,
    primitive: [u8; 8],   // little-endian bytes for I32/U32/I64/U64/F32/F64
    str_value: Box<[u8]>, // populated only for ConstType::Str (empty otherwise)
}
```

* `[u8; 8]` covers every primitive variant exactly. Primitive reads become a
  bounds-check + a single load of the entry struct (which fits in one cache
  line) + a fixed-size memcpy.
* `Box<[u8]>` is 16 bytes (vs. `Vec<u8>`'s 24) and `Box::default()` for an empty
  slice is non-allocating, so the primitive case pays no heap cost.
* `pub` access to `value` is replaced with `bytes()` and typed constructors;
  internal callers (builder, read_from, tests) move to those helpers.

The wire format does **not** change — `write_to`/`read_from` continue to emit
type tag + reserved + length + bytes.

## File Map

| File | Change |
|------|--------|
| `compiler/container/src/constant_pool.rs` | Restructure `ConstEntry`; rewrite `get_le_bytes` to read inline; add `ConstEntry::primitive_le` / `ConstEntry::string` constructors and `bytes()` accessor; update `read_from`, `write_to`, `section_size`, and tests |
| `compiler/container/src/builder.rs` | Switch `add_*_constant` helpers to the new constructors |
| `compiler/project/src/disassemble.rs` | Replace `entry.value` reads with `entry.bytes()` |

## Tasks

- [x] Write plan
- [ ] Restructure `ConstEntry` storage and update `ConstantPool` get/read/write paths
- [ ] Update `ContainerBuilder` constant helpers
- [ ] Update `disassemble.rs` consumers
- [ ] Run full CI pipeline (`cd compiler && just`)

## Verification

* Existing `ConstantPool` and `Container` round-trip tests continue to pass —
  these exercise the new code paths against the unchanged wire format.
* VM integration tests already exercise `LOAD_CONST_*` and `CMP_BR_*`, so any
  regression in `get_i32`/`get_i64`/`get_f32`/`get_f64` would surface there.
* Disassembler tests cover the rendered constant pool output.
