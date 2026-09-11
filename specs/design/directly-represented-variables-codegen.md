# Design: Directly-Represented Variables Code Generation

## Overview

This design specifies how the IronPLC compiler generates bytecode for
IEC 61131-3 **directly-represented (located) variables** — `%I` inputs, `%Q`
outputs, and `%M` memory (section 2.4.1.1) — against the VM **process image**.
It covers both source forms: variables declared with an `AT` clause
(`sensor AT %IW0 : INT;`) and anonymous addresses written inline in code
(`x := %IW0;`, `REF(%IW0)`, `fb(OUT => %QX0.0)`).

The decision to implement located variables against the process image rather
than flat variable-table slots is recorded in
[ADR-0050](../adrs/0050-directly-represented-variables-via-process-image.md);
this document specifies the compiler-side realization of that decision. The
runtime-side realization it targets is already specified elsewhere.

The design builds on:

- **[ADR-0050](../adrs/0050-directly-represented-variables-via-process-image.md)**:
  the decision — located variables use the process image, not flat slots
- **[Runtime Execution Model](runtime-execution-model.md)**: the authoritative
  spec for the `%I`/`%Q`/`%M` regions, the byte-offset formulas, the
  `INPUT_FREEZE`/`OUTPUT_FLUSH` scan phases, and input-snapshot / output-staging
  semantics
- **[Bytecode Container Format](bytecode-container-format.md)**: the reserved
  `input_image_bytes` / `output_image_bytes` / `memory_image_bytes` header fields
  and the task-table image offsets
- **[FMI Co-Simulation Support](fmi-co-simulation-support.md)**: the first I/O
  driver that consumes the process image; this work is its phase 2
- **[ADR-0033](../adrs/0033-opcode-encoding-by-class-and-type.md)**: the opcode
  encoding scheme the new located-access opcodes follow

## Design Goals

1. **One resolver for both source forms** — declared-named and anonymous-inline
   located variables resolve through a single path to the same
   `(region, offset, width)` result. The "Not implemented at compile_expr.rs#L867"
   gap for anonymous addresses closes as a consequence, not a special case.
2. **Realize the spec, don't fork it** — offsets, widths, and region semantics
   come from the runtime execution model; the compiler computes what that spec
   already defines.
3. **Correct IEC I/O semantics** — reads and writes compile so that the runtime
   can present `%I` as a frozen snapshot, stage `%Q`, and treat `%M` as direct
   read-write. Semantics live in the VM; codegen must emit the instructions that
   let the VM enforce them.
4. **Verifiable safety** — every located access carries enough information for the
   bytecode verifier to bounds-check it against the region size.

## Scope

**In scope:** located `%I`/`%Q`/`%M` variables in program and function-block
bodies; declared-`AT` and anonymous-inline forms; reads, assignment targets,
`REF()` operands, and function-block output (`=>`) targets; bit/byte/word/
dword/lword size prefixes; region sizing and container header/offset population;
new located-access opcodes; verifier bounds rules; plc2plc round-trip rendering.

**Out of scope (deferred / elsewhere):**
- The platform-specific **I/O driver** that populates `%I` from hardware and
  drains `%Q` to hardware (runtime model leaves this out of scope; the FMI shim
  is the first driver).
- The VM's allocation of the regions and the `INPUT_FREEZE`/`OUTPUT_FLUSH` phase
  wiring — specified by the runtime execution model; this doc references it and
  defines the instructions that drive it.
- Located variables as **task inputs/outputs across multiple tasks** (v1 targets
  a single program / single scan task, mirroring the FMI v1 boundary).

---

## 1. Current State

The facts this design starts from:

- `resolve_variable()` returns `NotImplemented` for `Variable::Direct`
  (`compiler/codegen/src/compile_expr.rs:864-868`).
- Named located variables get a plain variable-table slot via
  `identifier.symbolic_id()` (`compiler/codegen/src/compile_fn.rs`), so they
  behave as scratch memory with no I/O semantics.
- The container header **already reserves** `input_image_bytes` /
  `output_image_bytes` / `memory_image_bytes`
  (`compiler/container/src/header.rs:79-81`), all `0`.
- Task-table `input_image_offset` / `output_image_offset` are hardcoded `0`
  (`compiler/container/src/builder.rs:342-343`).
- Scan phases `INPUT_FREEZE` / `OUTPUT_FLUSH` are `// Stub` no-ops
  (`compiler/vm/src/vm.rs`); no located-access opcodes exist.
- The DSL already models the address:
  `AddressAssignment { location: LocationPrefix, size: SizePrefix, address: Vec<u32> }`
  (`compiler/dsl/src/common.rs`), with `LocationPrefix{I,Q,M}` and
  `SizePrefix{X,B,W,D,L,Unspecified,Nil}`.
- Analyzer type resolution seeds `var_types` only for *named* located variables
  (`xform_resolve_expr_types.rs:156-160`); anonymous ones get no type.

## 2. Address Resolution

**REQ-DRV-001** Each `AddressAssignment` resolves at compile time to a
`LocatedAccess { region, byte_offset, width, signedness }`.

**REQ-DRV-002** The **region** is taken from `LocationPrefix`: `I` → input,
`Q` → output, `M` → memory.

**REQ-DRV-003** The **access width** is taken from `SizePrefix`: `X` = 1 bit,
`B` = 8 bits, `W` = 16, `D` = 32, `L` = 64.

**REQ-DRV-004** The **byte offset** is computed from the `address` vector using
the runtime-model formulas:

| Size prefix | Access width | Byte offset |
|-------------|--------------|-------------|
| `X` (bit) | 1 bit | `idx / 8` byte, `idx % 8` LSB-first bit |
| `B` (byte) | 8 bits | `idx` |
| `W` (word) | 16 bits | `idx * 2` |
| `D` (dword) | 32 bits | `idx * 4` |
| `L` (lword) | 64 bits | `idx * 8` |

**REQ-DRV-005** Both source forms resolve through the same function: the named
path (declaration binds a symbol to an `AddressAssignment`) and the anonymous
path (`resolve_variable`'s `Variable::Direct` arm, today L867) call one
`resolve_located_access(&AddressAssignment)`.

## 3. Region Layout and Sizing

**REQ-DRV-010** The compiler places every located variable into its region and
computes each region's total byte size (the maximum extent addressed, per size
prefix), emitting `input_image_bytes` / `output_image_bytes` /
`memory_image_bytes` into the container header.

**REQ-DRV-011** The per-task `input_image_offset` / `output_image_offset` are
populated from the located-variable layout, replacing the hardcoded `0`s.

**REQ-DRV-012** Aliasing — two located variables whose byte ranges overlap
(e.g. `%MW0` and `%MB1`) — is a documented IEC feature. See
[Open Design Questions](#8-open-design-questions) Q2 for the v1 policy.

## 4. Opcodes

**REQ-DRV-020** Four located-access opcodes are added, encoded per
[ADR-0033](../adrs/0033-opcode-encoding-by-class-and-type.md):
`LOAD_INPUT`, `STORE_OUTPUT`, `LOAD_MEMORY`, `STORE_MEMORY`. Each carries the
region byte offset and access width.

**REQ-DRV-021** Located reads/writes compile to these opcodes, **not** to the
scalar-slot load/store opcodes. `%I` and `%M` reads emit `LOAD_INPUT` /
`LOAD_MEMORY`; `%Q` and `%M` writes emit `STORE_OUTPUT` / `STORE_MEMORY`.

## 5. Code Generation

**REQ-DRV-030** A located read in an expression emits the region's load opcode
with the resolved offset and width.

**REQ-DRV-031** A located assignment target emits the region's store opcode.
Writing to `%I` is rejected at compile time with a clear diagnostic (`%I` is
read-only during EXECUTE).

**REQ-DRV-032** `REF(%…)` and function-block output (`=>`) targets on a located
address resolve through the same `resolve_located_access`; the `Variable::Direct`
arm of `resolve_variable` returns a located reference instead of the L867
`NotImplemented`.

**REQ-DRV-033** Bit-addressed writes to `%Q`/`%M` (`%QX0.3 := …`) use
read-modify-write on the enclosing byte. See
[Open Design Questions](#8-open-design-questions) Q4 for reuse of the existing
bit-access machinery.

## 6. Analyzer Type Resolution

**REQ-DRV-040** `var_types` is seeded for anonymous located addresses as well as
named ones, with width and signedness derived from the size prefix, so
expression type resolution knows the type of an inline `%…`.

**REQ-DRV-041** `xform_resolve_late_bound_expr_kind` gives `Variable::Direct` a
resolved type rather than `VariableType::None`.

## 7. Runtime, Verifier, and Round-Trip

**REQ-DRV-050** The VM allocates the `%I`/`%Q`/`%M` regions and the output
staging buffer, implements the four opcodes, and wires `INPUT_FREEZE` (snapshot
inputs) and `OUTPUT_FLUSH` (stage → flush, skipped on fault) per the
[Runtime Execution Model](runtime-execution-model.md). Codegen depends on this
runtime behavior but does not implement it.

**REQ-DRV-051** The bytecode verifier bounds-checks each located offset against
the region size (from the header fields) and validates opcode operands, per
[ADR-0006](../adrs/0006-bytecode-verification-requirement.md).

**REQ-DRV-052** plc2plc renders located variables back to `%…` syntax and
round-trips them. (`AddressAssignment`'s `Display` currently emits a debug-struct
form and must be fixed to emit IEC syntax.)

## 8. Open Design Questions

These are design decisions to settle before implementation. Recommendations are
noted; the reviewer confirms.

1. **Access width vs. declared type.** `%IW0`'s size prefix says 16-bit; a
   `sensor AT %IW0 : INT` agrees, but `AT %IW0 : BOOL` does not. Require the size
   prefix to match the declared type, or let the declared type win?
   *Recommend:* validate consistency in the analyzer, error on mismatch.
2. **`%M` overlap / aliasing.** Support overlapping byte ranges in v1, or reject?
   *Recommend:* allow overlap (documented IEC feature); the verifier bounds only
   against region size.
3. **Anonymous address without a size prefix** (`%I0`,
   `SizePrefix::Nil`/`Unspecified`). Default width, or require a prefix?
   *Recommend:* require a size prefix for anonymous inline use; error otherwise.
4. **Bit-addressed `%Q`/`%M` writes.** Reuse the `compile_bit_access_assignment`
   read-modify-write shape? *Recommend:* yes.
5. **Verifier region-size source.** Confirm the verifier reads region sizes from
   the header fields populated per REQ-DRV-010.

## 9. Implementation Approach

The work is naturally staged; each stage is independently testable and can land
as its own PR. This ordering front-loads the compiler front-end (no runtime
change) and defers the bytecode-surface change to the end.

1. **Address resolution** (front-end only): `resolve_located_access`, the
   offset/width formulas, region-size accounting, and analyzer type seeding
   (REQ-DRV-001..005, 040, 041). Unit-tested against every
   `LocationPrefix × SizePrefix` combination.
2. **Container population**: emit region sizes and task offsets (REQ-DRV-010,
   011); round-trip non-zero image sizes.
3. **Opcodes + VM + verifier**: define the four opcodes, emit them from codegen
   (removing the L867 TODO), implement the regions/staging and scan-phase wiring,
   and add verifier bounds rules (REQ-DRV-020..033, 050, 051). End-to-end tests:
   frozen `%I`, staged `%Q`, `%M` round-trip, and the anonymous-inline / `REF` /
   FB-output cases.
4. **Round-trip, gating, docs**: plc2plc rendering (REQ-DRV-052), problem-code
   documentation for retired/added diagnostics, `--allow-*` flag posture in the
   syntax-support guide, and a golden `%I`-in/`%Q`-out vertical slice.
