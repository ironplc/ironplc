# Instruction-Level Profiling

## Goal

Add optional per-opcode execution counters to the VM, gated behind a Cargo
feature flag (`profiling`). Introduce an `Opcode` type alias so opcode values
are self-documenting throughout the codebase.

## Architecture

- **Type alias**: `pub type Opcode = u8` in `container/src/opcode.rs`. All 123
  primary opcode constants change from `: u8` to `: Opcode`. This is a true
  zero-cost abstraction — no runtime overhead, no breakage, just clearer intent.

- **Profile struct**: `InstructionProfile { counts: [u64; 256] }` in a new
  `vm/src/profile.rs` module. The 256-element array covers every possible `u8`
  opcode with direct indexing — no branches, no allocation.

- **Feature gate**: A `profiling` feature in `ironplc-vm/Cargo.toml`. When
  disabled (the default), the profile field and increment are compiled out
  entirely — zero cost. When enabled, each opcode dispatch increments one
  counter.

- **Integration**: The `execute()` function accepts an
  `Option<&mut InstructionProfile>` (behind `#[cfg(feature = "profiling")]`).
  `VmRunning` owns the profile and exposes it via accessor methods, also
  feature-gated. `VmStopped` and `VmFaulted` carry the profile through state
  transitions.

## File map

| File | Change |
|------|--------|
| `compiler/container/src/opcode.rs` | Add `pub type Opcode = u8`; change all `pub const …: u8` to `pub const …: Opcode` |
| `compiler/container/src/lib.rs` | Re-export `Opcode` from `opcode` module |
| `compiler/vm/Cargo.toml` | Add `profiling = []` feature |
| `compiler/vm/src/profile.rs` | New — `InstructionProfile` struct |
| `compiler/vm/src/lib.rs` | Declare `profile` module; re-export `InstructionProfile` |
| `compiler/vm/src/vm.rs` | Thread profile through `execute()`, `VmReady`, `VmRunning`, `VmStopped`, `VmFaulted` |

## Tasks

- [x] Commit plan
- [ ] Add `Opcode` type alias and update constants in `opcode.rs`
- [ ] Re-export `Opcode` from container lib
- [ ] Add `profiling` feature to VM Cargo.toml
- [ ] Create `profile.rs` with `InstructionProfile`
- [ ] Wire profile into `execute()` and VM state types
- [ ] Add tests
- [ ] Run CI
