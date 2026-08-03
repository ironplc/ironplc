# Implementation Plan: Directly-Represented Variables via the Process Image

**Decision:** [ADR-0037](../adrs/0037-directly-represented-variables-via-process-image.md)
**Spec:** [Runtime Execution Model](../design/runtime-execution-model.md) · [FMI Co-Simulation](../design/fmi-co-simulation-support.md) (this is FMI phase 2)

## Goal

Implement directly-represented (located) variables — `%I` inputs, `%Q` outputs,
`%M` memory — against the specified process image, replacing the current
flat-slot / `NotImplemented` behavior. This:

1. Fixes the "Not implemented at compile_expr.rs#L867" error users hit on
   anonymous inline direct addresses (`REF(%IW0)`, `fb(OUT => %QX0.0)`, `%IW0` in
   expressions).
2. Gives named located variables (`sensor AT %IW0 : INT;`) correct IEC I/O
   semantics instead of scratch-slot behavior.
3. Builds the `%I`/`%Q`/`%M` substrate the FMI shim (PR #1258) and any future I/O
   driver bind to.

## Current State (facts)

- `resolve_variable()` returns `NotImplemented` for `Variable::Direct`
  (`compiler/codegen/src/compile_expr.rs:864-868`); the SymbolicVariableKind arms
  above it also TODO on array/structured/bit/partial/deref bases.
- Named located variables get a plain variable-table slot via
  `identifier.symbolic_id()` (`compiler/codegen/src/compile_fn.rs`).
- Container header **already reserves** `input_image_bytes` / `output_image_bytes`
  / `memory_image_bytes` (`compiler/container/src/header.rs:79-81`), all `0`.
- Task-table `input_image_offset` / `output_image_offset` are hardcoded `0`
  (`compiler/container/src/builder.rs:342-343`).
- Scan phases `INPUT_FREEZE` / `OUTPUT_FLUSH` are `// Stub` no-ops in
  `compiler/vm/src/vm.rs`; no `LOAD_INPUT`/`STORE_OUTPUT`/`LOAD_MEMORY`/
  `STORE_MEMORY` opcodes exist.
- DSL already models the address: `AddressAssignment { location: LocationPrefix,
  size: SizePrefix, address: Vec<u32> }` (`compiler/dsl/src/common.rs`), with
  `LocationPrefix{I,Q,M}` and `SizePrefix{X,B,W,D,L,Unspecified,Nil}`.
- Analyzer type resolution seeds `var_types` only for *named* located variables
  (`xform_resolve_expr_types.rs:156-160`); anonymous ones get no type.

## Design Summary

Each `AddressAssignment` resolves at compile time to `(region, byte_offset,
access_width)`:

- **region** from `LocationPrefix`: `I`→input, `Q`→output, `M`→memory.
- **access_width** from `SizePrefix`: `X`=1 bit, `B`=8, `W`=16, `D`=32, `L`=64.
- **byte_offset** from the `address` vector via the runtime-model formulas
  (bit: `idx/8` byte with `idx%8` LSB-first bit; byte: `idx`; word: `idx*2`;
  dword: `idx*4`; lword: `idx*8`).

Located reads/writes emit the located-access opcodes (carrying region offset +
width), not scalar-slot opcodes. Named and anonymous forms share one resolver.

## Phases

Each phase is independently testable and lands as its own PR.

### Phase 1 — Address resolution (compiler front-end, no runtime change)

Introduce a single resolver `AddressAssignment → LocatedAccess { region, offset,
width, signedness }` and the region size accounting, without yet changing storage
or opcodes. Assign named located variables and anonymous addresses a stable
offset within their region; compute per-region byte sizes.

| File | Change |
|------|--------|
| `compiler/codegen/src/` (new `compile_located.rs`) | `LocatedAccess` type + `resolve_located_access(&AddressAssignment)`; offset formulas; region size accumulation on `CompileContext` |
| `compiler/codegen/src/compile.rs` | Add `%I`/`%Q`/`%M` region maps + running sizes to `CompileContext` |
| `compiler/analyzer/src/xform_resolve_expr_types.rs` | Seed `var_types` for *anonymous* located addresses too (width/signedness from size prefix) |
| `compiler/analyzer/src/xform_resolve_late_bound_expr_kind.rs` | Give `Variable::Direct` a resolved type instead of `VariableType::None` (line ~197) |
| unit tests | Offset/width derivation for every `LocationPrefix`×`SizePrefix`, incl. bit addresses |

### Phase 2 — Container: populate image sizes and offsets

Emit the region sizes and task offsets the runtime needs.

| File | Change |
|------|--------|
| `compiler/container/src/builder.rs` | Set `input_image_bytes`/`output_image_bytes`/`memory_image_bytes` from Phase 1 sizes; populate `input_image_offset`/`output_image_offset` (replace hardcoded `0`, lines 342-343) |
| `compiler/codegen/src/compile.rs` | Thread region sizes into the builder |
| `compiler/container/src/header.rs` tests | Round-trip non-zero image sizes |
| `specs/design/bytecode-container-format.md` | Note image-size population (no new fields; format version bump only if opcodes require it in Phase 3) |

### Phase 3 — Opcodes + VM regions and phase wiring

Add the located-access instructions and make the scan phases real.

| File | Change |
|------|--------|
| `compiler/vm/src/opcode.rs` (and shared opcode defs) | Define `LOAD_INPUT`/`STORE_OUTPUT`/`LOAD_MEMORY`/`STORE_MEMORY` per ADR-0033 encoding |
| `compiler/codegen/src/compile_expr.rs` | Located reads emit `LOAD_INPUT`/`LOAD_MEMORY`; **remove the L867 TODO** — `Variable::Direct` resolves via `resolve_located_access` |
| `compiler/codegen/src/compile_stmt.rs` | Located assignment targets emit `STORE_OUTPUT`/`STORE_MEMORY`; reject `%I` as a target with a clear diagnostic |
| `compiler/codegen/src/compile_expr.rs` (`resolve_variable`) | `Variable::Direct` returns a located reference for `REF()` / FB-output-target callers |
| `compiler/vm/src/vm.rs` | Allocate `%I`/`%Q`/`%M` regions + output staging buffer; implement `LOAD_*`/`STORE_*`; wire `INPUT_FREEZE` (snapshot) and `OUTPUT_FLUSH` (stage→flush, skip on fault) |
| `compiler/vm/src/verifier` | Bounds-check located offsets against region sizes; validate opcode operands |
| e2e tests | `%M` read-write round-trip; `%Q` staged/flushed; `%I` frozen mid-scan; anonymous inline `%IW0`, `REF(%IW0)`, `fb(=> %QX0.0)` |

### Phase 4 — Round-trip, gating, and docs

| File | Change |
|------|--------|
| `compiler/plc2plc/src/renderer.rs` + tests | Verify direct-variable rendering round-trips (`AddressAssignment` Display currently emits debug-struct form — fix to `%…` syntax if needed) |
| `docs/compiler/problems/` | Ensure the retired L867 `NotImplemented` path and any new located diagnostics (e.g. write-to-`%I`, out-of-range offset) are documented |
| `specs/steering/syntax-support-guide.md` | Confirm `--allow-*` flag posture for direct variables (are they gated?) |
| `compiler/codegen/tests/` | Golden vertical slice: `%I`-in / `%Q`-out program compiled and stepped (mirrors FMI phase 5 seed) |

## Open Questions (resolve before Phase 3)

1. **Access width vs. declared type.** `%IW0` size prefix says 16-bit; a
   `sensor AT %IW0 : INT` agrees, but `AT %IW0 : BOOL` does not. Do we require
   the size prefix to match the declared type, or let the declared type win? (IEC
   allows the prefix; recommend: validate consistency in the analyzer, error on
   mismatch.)
2. **`%M` overlap / aliasing.** Two located vars mapping to overlapping byte
   ranges (`%MW0` and `%MB1`) are legal IEC aliasing. Do we support overlap in v1
   or reject it? (Recommend: allow overlap — it is a documented IEC feature — and
   let the verifier bound only against region size.)
3. **Anonymous address without size prefix** (`%I0`, `SizePrefix::Nil/Unspecified`).
   Default width? (Recommend: require a size prefix for anonymous inline use;
   error otherwise.)
4. **Bit-addressed `%Q`/`%M` writes** need read-modify-write on the enclosing
   byte, like existing bit-access. Reuse `compile_bit_access_assignment` shape?
5. **Verifier region-size source.** Confirm the verifier reads region sizes from
   the header fields populated in Phase 2.

## Tasks

- [ ] Phase 1: `resolve_located_access` + offset/width formulas + region sizing
- [ ] Phase 1: analyzer type seeding for anonymous located addresses
- [ ] Phase 1: unit tests for all prefix combinations
- [ ] Phase 2: populate image byte sizes + task offsets in the container
- [ ] Phase 2: container round-trip tests
- [ ] Phase 3: define located-access opcodes (ADR-0033 encoding)
- [ ] Phase 3: codegen located reads/writes; **remove L867 TODO**
- [ ] Phase 3: VM regions, staging buffer, `INPUT_FREEZE`/`OUTPUT_FLUSH`, `LOAD_*`/`STORE_*`
- [ ] Phase 3: verifier bounds rules
- [ ] Phase 3: e2e tests (frozen `%I`, staged `%Q`, `%M`, anonymous inline, `REF`, FB output)
- [ ] Phase 4: plc2plc round-trip + renderer fix
- [ ] Phase 4: problem-code docs + syntax-support-guide update + golden slice
- [ ] Resolve the open questions above (before Phase 3)
- [ ] `cd compiler && just` passes for each phase
