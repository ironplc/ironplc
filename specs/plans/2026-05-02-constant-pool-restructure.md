# Restructure `ConstantPool` in-memory layout

**Date:** 2026-05-02
**Branch:** `claude/bytecode-cells-optimization-LNlzN`
**Driver:** [bytecode-dispatch-bounds-check measurement](2026-05-02-bytecode-dispatch-bounds-check-measurement.md)

## Context

The bounds-check measurement on the counter-loop workload showed `ConstantPool::get_i32` consumes **8.01% of all VM-retired instructions**, called twice per iteration at ~40 native instructions per call.

Looking at `compiler/container/src/constant_pool.rs:9-14`:
```rust
pub struct ConstEntry {
    pub const_type: ConstType,
    pub value: Vec<u8>,   // ← heap allocation per constant
}

pub struct ConstantPool {
    entries: Vec<ConstEntry>,
}
```

Every `get_i32` call walks: `entries.get(idx)?` → load `ConstEntry` → check `const_type` tag → dereference inner `Vec<u8>` → `copy_from_slice` 4 bytes into a stack array → `from_le_bytes`. Two pointer chases (entry → inner Vec → heap data), a small memcpy, and length checks on a slice whose size is statically known to be 4.

For a 4-byte i32 constant, this costs ~40 native instructions when ~5 would suffice. The on-disk container format is fine; only the **in-memory representation** is wasteful.

## Goal

Replace the `ConstEntry { const_type, value: Vec<u8> }` shape with a typed enum variant so scalar constants are stored inline and `get_i32` / `get_i64` / `get_f32` / `get_f64` collapse to a bounds-check + match + return.

**Non-goals:**
- Changing the on-disk container format (FORMAT_VERSION stays put).
- Touching `ContainerRef` (no_std zero-copy view, separate code path, no internal consumers in this repo).
- Changing the builder API (`ContainerBuilder::add_i32_constant`, etc.) or `ConstantIndex` semantics. Same single u16 index space, same callers.

Expected wins (callgrind, counter-loop workload):
- `ConstantPool::get_i32` from ~40 instr/call → ~5–8 instr/call.
- ~7% wall-clock improvement on constant-heavy code; potentially more on workloads that aren't bottlenecked by dispatch.

## Approach

### New in-memory shape

`compiler/container/src/constant_pool.rs`:

```rust
#[derive(Clone, Debug)]
pub enum ConstValue {
    I32(i32),
    U32(u32),
    I64(i64),
    U64(u64),
    F32(f32),
    F64(f64),
    Str(Vec<u8>),
}

#[derive(Clone, Debug, Default)]
pub struct ConstantPool {
    entries: Vec<ConstValue>,
}
```

`Vec<u8>` is 24 bytes, so `ConstValue` is 24 + tag = ~32 bytes per entry (worst case). That's larger per-entry than the current `ConstEntry` proper (32 bytes vs 32 bytes — equivalent!), and crucially **inlines** the i32/i64/f32/f64 payload that today sits on a separate heap allocation. Cache-line locality also improves because there's no second pointer chase.

`ConstEntry` (the public type) is renamed `ConstValue`. The public re-export at `compiler/container/src/lib.rs:51` updates accordingly. No external consumers exist (verified via grep).

### Accessor rewrites

```rust
pub fn get_i32(&self, index: ConstantIndex) -> Result<i32, ContainerError> {
    match self.entries.get(index.raw() as usize) {
        Some(ConstValue::I32(v)) => Ok(*v),
        Some(other) => Err(ContainerError::InvalidConstantType(other.const_type() as u8)),
        None => Err(ContainerError::InvalidConstantIndex(index)),
    }
}
```

Equivalent rewrites for `get_u32` (NEW — see "Missing variant" below), `get_i64`, `get_u64`, `get_f32`, `get_f64`, and `get_str`. The `get_le_bytes::<N>` private helper is deleted.

A small helper on `ConstValue`:
```rust
impl ConstValue {
    pub fn const_type(&self) -> ConstType { /* match on variant */ }
}
```

This preserves the disassembler's need to format an entry's type tag (used at `compiler/project/src/disassemble.rs:645`).

### Missing variant: `U32`

`ConstType::U32` exists in `compiler/container/src/const_type.rs:9` and is serializable, but the current pool exposes no `get_u32`. The new `ConstValue::U32` variant lights this up cleanly. Whether to wire a `get_u32` accessor and `add_u32_constant` builder method now is a small judgement call; default in this plan is to add both (parallel structure with the others, no behaviour change for code that doesn't use them).

### Builder

`compiler/container/src/builder.rs:92-134` — each `add_*_constant` method changes from
```rust
self.constant_pool.push(ConstEntry { const_type: ConstType::I32, value: value.to_le_bytes().to_vec() });
```
to
```rust
self.constant_pool.push(ConstValue::I32(value));
```

Mechanical 1:1 rewrite for all 5 (or 6, with U32) variants.

### Serialization (unchanged on disk)

`write_to` reads the variant, emits the existing `[type:1, reserved:1, size:2, payload]` byte sequence:
```rust
fn write_to(&self, w: &mut impl Write) -> Result<(), ContainerError> {
    w.write_all(&(self.entries.len() as u16).to_le_bytes())?;
    for entry in &self.entries {
        let (ty, bytes) = entry.encoded();  // helper returning (ConstType, Cow<[u8]>)
        w.write_all(&[ty as u8, 0])?;
        w.write_all(&(bytes.len() as u16).to_le_bytes())?;
        w.write_all(&bytes)?;
    }
    Ok(())
}
```

`read_from` parses the on-disk format and constructs the matching enum variant per `ConstType`. The size field is still present per entry but only used to skip past `Str` payloads (scalars have implicit fixed size).

### Iteration API

`pub fn iter(&self) -> impl Iterator<Item = &ConstValue>` — same shape as today, returning `&ConstValue` instead of `&ConstEntry`. The disassembler's call site at `disassemble.rs:645` adjusts to:
```rust
match container.constant_pool.iter().nth(pool_index as usize) {
    Some(v) => format!("= {}", format_const_value(v)),
    None => format!("= <invalid pool index {}>", pool_index),
}
```
where `format_const_value` matches on the enum directly (no separate `(ConstType, &[u8])` pair needed).

## Critical files

| File | Change |
|---|---|
| `compiler/container/src/constant_pool.rs` | Replace `ConstEntry` with `ConstValue` enum; rewrite accessors; rewrite `read_from`/`write_to`. Update tests. |
| `compiler/container/src/lib.rs:51` | Re-export `ConstValue` instead of `ConstEntry`. |
| `compiler/container/src/builder.rs:92-134` | Replace `ConstEntry { ... }` construction with `ConstValue::*(value)` pushes. |
| `compiler/project/src/disassemble.rs:645` | Switch `format_const_value` from `(ConstType, &[u8])` to `&ConstValue`. |
| `compiler/vm/src/vm.rs:611,1120` | No source change — `get_i32` / `get_str` signatures are unchanged. |

## Verification

1. **Unit tests in `constant_pool.rs`** — port all 12 existing tests; they currently exercise `get_i32`, `get_f32`, `get_f64`, `get_i64`, `get_str`, type-mismatch errors, out-of-bounds errors, write/read round-trip, and `iter`. Add 1-2 round-trip tests covering a mixed-type pool.
2. **Container round-trip** — existing tests in `container/src/container.rs` parse/emit containers; they pass iff serialization is unchanged.
3. **Codegen + VM execution tests** — the existing `compiler/vm/tests/scenarios.rs`, `steel_thread.rs`, `execute_*.rs` all run real ST programs end-to-end; they pass iff in-memory accessors still produce correct values.
4. **Disassembler tests** — `compiler/project/src/disassemble.rs` tests cover constant formatting; they pass iff the rewritten `format_const_value` produces the same output.
5. **Run `cd compiler && just`** — full CI (compile + coverage ≥85% + clippy + fmt) per CLAUDE.md.
6. **Re-run callgrind on `compiler/benchmarks/examples/vm_vs_native.rs vm 1000`** — confirm `ConstantPool::get_i32` drops from ~8% of total instructions to <2%, and total per-iter VM instruction count drops by ~7-10% (40 instr × 2 calls/iter saved out of ~500).

## Out of scope (separate plans)

- **Touching `ContainerRef`'s `get_i32_constant`** (no_std path). Different code, different consumers (none internal). Worth doing later if a no_std VM emerges; not needed now.
- **Splitting constants by type into dense per-type arrays** (e.g., `Vec<i32>`, `Vec<f32>`). Larger restructure, requires per-type sub-indexing, and the enum approach captures most of the available win without breaking the indexing scheme.
- **Constant deduplication or interning.** Orthogonal to the layout change; could be a separate codegen optimization.
- **Adding `U32` accessor and builder method.** Included in this plan as a small nicety alongside the variant; can be split out if it complicates review.

## Risk

Low. The change is mechanical and locally scoped:
- Two crates touched (`container`, `project`).
- Public API stable except `ConstEntry` → `ConstValue` rename (no external consumers).
- On-disk format unchanged — existing containers continue to load.
- Behaviour preserved (same errors on invalid index / type mismatch).
- Win is not the ~10x the dispatch loop needs — it's a single-digit-percent improvement to a hot helper. The value is that it's cheap to do, removes obvious waste, and makes the constant pool stop showing up in the profile, leaving the next investigation focused on the 90% in dispatch.
