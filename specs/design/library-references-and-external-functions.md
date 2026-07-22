# Design: Library References and External Library Functions

## Overview

Vendor toolchains ship their standard functions in *libraries*: Beckhoff's
`Tc2_Standard`, `Tc2_Math`, `Tc2_System`, and so on. A TwinCAT project does not
enable these with a compiler switch — it **references** a library, and that
reference brings the library's functions, function blocks, and types into scope.
Some of those POUs are ordinary Structured Text; a handful bottom out on runtime
primitives the compiler must provide.

Current work (e.g. [#1217](https://github.com/ironplc/ironplc/pull/1217),
part of #1199) registers individual library functions such as `LTRUNC`/`LMOD`
behind per-function `--allow-*` flags. This document argues that mechanism is the
wrong shape and specifies the correct one: the compiler owns a small **primitive
vocabulary** plus a **reference/scoping mechanism**; the library functions
themselves are provided *outside* the compiler — bound to a primitive or written
in ST — and are made available by *referencing* a library, resolved against the
copy the user already has installed.

### Building On

- **[ADR-0008](../adrs/0008-unified-builtin-opcode.md)** — the single `BUILTIN`
  opcode dispatched by `func_id`. This is the extension point for new primitives:
  adding one costs a `func_id` and a VM dispatch arm, **not** an opcode slot.
- **[ADR-0003](../adrs/0003-plc-standard-function-blocks-as-intrinsics.md)** —
  standard FBs recognized at dispatch and routed to native implementations. The
  same "recognize a name, route to a primitive" pattern applies to functions.
- **[ADR-0012](../adrs/0012-accept-vendor-dialect-files-as-is.md)** — accept
  vendor files as-is; this design is the semantic counterpart for referenced
  libraries.
- **[Beckhoff TwinCAT Dialect](beckhoff-twincat-dialect.md)** — parses TwinCAT
  files but scopes semantic resolution of vendor constructs as future work. This
  design is part of that follow-on.

## Guiding Invariant

> **The compiler grows *primitives* (BUILTIN `func_id`s), not *functions*.**

A new library function requires a compiler change **only if** it needs a
primitive the VM does not yet have. Every function expressible from existing
primitives (or from other library POUs) is pure library content — added outside
the compiler, with zero codegen changes. Primitives are added rarely and
deliberately; vendor function catalogs are not the compiler's concern.

This is the same line the standard library already walks — `SIZEOF` is a
compiler intrinsic because it reads type layout and *cannot* be written in ST,
whereas `LTRUNC` is a plain `LREAL`→`LREAL` truncation. The two belong on
opposite sides of this invariant, and the per-function flag put them on the same
side.

## The Three Categories

| Category | Example | Where it lives | Codegen path |
|----------|---------|----------------|--------------|
| **Compiler-only intrinsic** | `SIZEOF`, `TRUNC`→conv, `x_TO_y` | Compiler | Bespoke emit (reads compile-time info) |
| **External library function** | `LTRUNC`, `LMOD` | Library declaration + VM primitive | Inline `BUILTIN` at call site |
| **Source library POU** | most of any library | Library ST | Compile + link + reachability-prune |

The first category is small and closed. The second is where "needs a primitive"
functions land — the library supplies the *name, signature, and reference
gating*; the VM supplies the *operation*. The third is the bulk of any real
library and needs no compiler involvement at all beyond ordinary compilation.

## Current State

**Already present (the hard parts):**

- `analyze(sources: &[&Library])` merges every parsed unit into one library and
  resolves it as a set (`analyzer/src/stages.rs`).
- `xform_toposort_declarations` computes the declarations reachable from PROGRAM
  roots; codegen skips the rest. A large library can be supplied and only the
  used POUs are emitted.
- The unified `BUILTIN` opcode + `opcode::builtin::*` `func_id` table, with
  `lookup_builtin` mapping names → `func_id` and `emit_builtin` emitting the op
  (`codegen/src/compile_call.rs`). `ABS`, `SQRT`, `SIN`, … are already opcodes
  surfaced to ST by name.
- TwinCAT file parsing (`.TcPOU`/`.TcGVL`/`.TcDUT`) and `.plcproj` discovery
  (`sources/src/discovery/`).

**Missing:**

- Primitive opcodes for float→float truncation and float modulo (the VM has
  neither; verified against `opcode::builtin` and `vm/src`).
- A binding surface for functions provided *outside* the compiler.
- Library **identity** — today a `Library` is a nameless, versionless element
  list.
- **Reference declarations** — `.plcproj` discovery reads only
  `<Compile Include>` (the project's own files), not `<LibraryReference>`.
- **Reference-gated resolution** — the merged environment is one flat namespace;
  a symbol is visible whether or not its library was referenced.
- **Repository resolution** — resolving a reference to a file on a search path.
- A **library-file reader** for packaged `.library` archives.

## Design

### Part A — The intrinsic primitive surface

**Extension point.** New primitives are new `func_id`s under the existing
`BUILTIN` opcode (ADR-0008), each with a VM dispatch arm and an
`opcode::builtin::arg_count` entry. No opcode-slot cost; no dispatch-table
restructuring.

**Primitives needed now** (semantics precise, `LREAL`/`REAL` = f64/f32):

| Primitive | Signature | Semantics |
|-----------|-----------|-----------|
| `TRUNC_F64` / `TRUNC_F32` | `(x) -> x` | Truncate the fractional part toward zero, result stays float (no integer-range clamp). |
| `MOD_F64` / `MOD_F32` | `(a, b) -> float` | Floating-point remainder, e.g. `MOD_F64(400.56, 360.0) = 40.56`. Define `b = 0` behavior explicitly (recommend: propagate NaN, consistent with float `DIV`). |

**Two-tier binding.** How an out-of-compiler function reaches a primitive
depends on whether the function has logic:

- **Intrinsic alias** — the function *is* a primitive (`LTRUNC` ≡ `TRUNC_F64`).
  Bind the name to the `func_id` and inline the `BUILTIN` at the call site, the
  way `compile_trunc` already inlines its conversion. No `CALL`, no stack frame.
- **Library POU** — the function is real ST built from primitives and other
  POUs. Compile and link it through the ordinary user-function path;
  reachability pruning drops it if unused.

**Binding mechanism.** Two forms, and this design recommends supporting both:

- *(recommended for aliases)* **External declaration.** The library declares the
  function with no body and marks it external, naming the primitive:

  ```
  {external := 'TRUNC_F64'}
  FUNCTION LTRUNC : LREAL  VAR_INPUT IN : LREAL; END_VAR  END_FUNCTION
  ```

  Codegen sees the marker and inlines the `BUILTIN`. Honest for `LTRUNC`/`LMOD`,
  which have no ST logic to write.

- *(for functions with logic)* **Reserved intrinsic names in ST.** The compiler
  recognizes a fixed `__`-prefixed primitive namespace (`__TRUNC_LREAL`, …) that
  library ST may call, so a function with real logic is written in ordinary ST
  over primitives. This is how CODESYS exposes its `__` operators.

**Stability contract.** Once library source (yours, or a user's export) is
written against a primitive name, that name and its semantics are frozen like an
ABI. The intrinsic set should therefore be a **documented, versioned surface**,
which is also the nudge to promote `lookup_builtin` from a hardcoded `match` into
a declared table.

### Part B — Library identity and references

**Identity.** Introduce a library as a *named, versioned bundle* of POUs, types,
and GVLs (name, version, member sources, dependencies). A `Library` today
carries none of this.

**Reference declaration** — where a project says "I use `Tc2_Math`":

- **TwinCAT fidelity.** `.plcproj` already lists `<LibraryReference>` /
  placeholder entries; discovery parses the file but ignores them today. Extract
  them to get the real declared reference set.
- **Native projects.** A small IronPLC manifest, or a source-level
  `{library '...'}` / `USING` pragma, for non-TwinCAT trees.

This declaration is what **replaces the per-function `--allow-*` flag** for the
library case.

**Resolution — use the installed copy, never redistribute.** Resolve a reference
(name + version) to a file on a configurable **library search path / repository**
— the TwinCAT repository on a developer machine, a user-supplied path on
Linux/CI. IronPLC ships no vendor IP; it reads what the user already licensed and
installed, exactly as a C compiler reads installed system headers. Formats, in
increasing effort:

- **PLCopen XML export** — already ingested; lowest-effort path.
- **Unprotected `.library`** — a packaged archive of the same POU XML IronPLC
  already parses; the reader is *unpack the container → existing XML path*, not a
  new frontend.
- **Protected `.compiled-library`** — no source inside. Cannot be used directly;
  degrade to *signatures only, implementation missing* with a clear diagnostic
  rather than miscompiling.

**Reference-gated resolution.** Tag each function-environment entry with its
owning library, and expose a library's exports **only when the project references
it**. This is what makes references meaningful rather than a global switch — it
enables the two diagnostics a reference model must produce:

- *used but not referenced* — "`LMOD` requires a reference to `Tc2_Math`."
- *referenced but not found* — "`Tc2_Math` is referenced but not present on the
  library search path."

Namespacing (`Tc2_Math.LMOD`) and version/placeholder resolution are fidelity
layers that can follow; a referenced-set filter over bare names covers the common
case first.

### Part C — Worked example: `LTRUNC` / `LMOD`

The whole stack, applied to the two functions that motivated it:

1. **Compiler/VM (one-time):** add `TRUNC_F64` and `MOD_F64` (and `_F32`
   siblings) as `BUILTIN` `func_id`s with VM dispatch arms.
2. **Outside the compiler:** provide a `Tc2_Math` library definition declaring
   `LTRUNC`/`LMOD` as external aliases to those primitives, with their real
   `LREAL`-only signatures.
3. **In a project:** referencing `Tc2_Math` brings them into scope; reference
   gating controls visibility; codegen inlines the `BUILTIN`. They **run** — the
   current flag-gated versions type-check and then hit `Diagnostic::todo` at
   codegen because no implementation exists.

Adding the next `Tc2_Math` function that needs float-trunc/mod is then *pure
library content* — no compiler change, no new flag.

## Implementer Map

| Concern | File(s) |
|---------|---------|
| New `BUILTIN` `func_id`s + `arg_count` | `compiler/container/src/opcode` |
| VM dispatch arms for the new primitives | `compiler/vm/src` |
| Alias inlining / external-binding dispatch | `compiler/codegen/src/compile_call.rs` |
| Name → primitive table (promote from `match`) | `compiler/codegen/src/compile_call.rs` (`lookup_builtin`) |
| External-function attribute in the AST/analyzer | `compiler/dsl`, `compiler/analyzer/src/intermediates/stdlib_function.rs` |
| Reference-gated function environment | `compiler/analyzer/src/function_environment.rs`, `stages.rs` |
| `<LibraryReference>` extraction | `compiler/sources/src/discovery/` |
| Library search-path / repository resolution | `compiler/sources/` (+ CLI option) |
| Library-file (`.library`) reader | `compiler/sources/` (new, → existing XML path) |
| "used but not referenced" / "not found" diagnostics | `compiler/problems`, analyzer rules |

## Phasing

- **Phase 0 — unblock #1217's actual goal.** Add the float trunc/mod primitives
  and a minimal `Tc2_Math` alias definition, selected by reference (interim: by
  `--dialect`). `LTRUNC`/`LMOD` run. No per-function flag.
- **Phase 1 — references.** `<LibraryReference>` extraction + native manifest;
  library-tagged, reference-gated resolution; the two diagnostics. Retire the
  per-function `--allow-*` flags.
- **Phase 2 — one mechanism.** Generalize the external-binding surface and
  migrate `SIZEOF` and other flag-gated intrinsics onto it for uniformity.
- **Phase 3 — fidelity.** Unprotected `.library` reader + repository resolution;
  qualified namespaces; version/placeholder/dependency resolution.

## Non-Goals

- Redistributing vendor libraries or reimplementing their catalogs.
- Using protected `.compiled-library` source (signatures-only at best).
- A full CODESYS namespace/version resolver in the first cut.

## Relationship to #1217

This reframes that PR: `LTRUNC`/`LMOD` are not a compiler flag but **two
primitives plus a library alias**. Recommendation: do not extend the
per-function flag pattern; land the primitives and the `Tc2_Math` alias instead,
so the functions actually execute and the next such function costs nothing in the
compiler.

## Decisions to Ratify as ADRs

The core decisions here are ADR-worthy and should be recorded alongside 0003 /
0008:

1. **External library functions bind to `BUILTIN` primitives** — the compiler
   grows opcodes, not functions.
2. **Library availability is expressed by references, not global allow-flags.**
3. **Referenced libraries are consumed from the user's install; never
   redistributed** (degrade gracefully when absent or protected).
