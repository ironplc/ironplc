# Plan: Compatibility-Library Bindings and TwinCAT Math/Utility Functions

## Goal

Deliver the support primitives that let the stalled vendor-function PRs
([#1217](https://github.com/ironplc/ironplc/pull/1217) `LTRUNC`/`LMOD`,
[#1218](https://github.com/ironplc/ironplc/pull/1218) `MODABS`,
[#1246](https://github.com/ironplc/ironplc/pull/1246)
`BOOL_TO_STRING`/`LREAL_TO_FMTSTR`/`ADR`) be resolved the way review asked:
vendor functions **externalized as compatibility libraries**, with native code
in the compiler only where IEC 61131-3 source cannot express the semantics.

The primitives, from the deferred *Future Goals* of the
[Compatibility Libraries design](../design/compatibility-libraries.md):

1. **Bindings** — a per-version `library.toml` table mapping a library POU to
   a **VM builtin** or to **declare-only**.
2. **Fail-if-unimplemented** — a call to a declare-only POU is a dedicated
   compile error (new problem code), never silently-wrong codegen and never a
   runtime trap. `check` still passes (the declaration resolves), matching the
   corpus-check motivation of the original PRs.
3. **Two new BUILTIN func_ids** for semantics inexpressible in ST: LREAL
   truncation and LREAL modulo.
4. **[ADR-0042](../adrs/0042-library-functions-over-compiler-intrinsics.md)**
   codifying when a function may be compiler-provided versus must be a library
   function (this plan's companion; reviewed in the same PR).

Under the ADR-0042 rule each function lands as:

| Function | Mechanism |
|---|---|
| `LTRUNC(IN: LREAL): LREAL` | `Tc2_Math` POU bound to new `trunc_lreal` VM builtin — `TRUNC` clamps to `ANY_INT`, so LREAL-preserving truncation is inexpressible in ST |
| `LMOD(IN1: LREAL, IN2: LREAL): LREAL` | `Tc2_Math` POU bound to new `fmod_lreal` VM builtin — no floating modulo exists in ST (`MOD` is integer) |
| `MODABS(IN: LREAL, IM: LREAL): LREAL` | `Tc2_Math` POU with a math-dictated **ST body** calling `LMOD` — no builtin |
| `BOOL_TO_STRING(IN: BOOL): STRING` | library **ST body** (`IF IN THEN BOOL_TO_STRING := 'TRUE'; ELSE ... 'FALSE'`) — trivially expressible, so no intrinsic and no func_id |
| `LREAL_TO_FMTSTR(in: LREAL, iPrecision: INT, bRound: BOOL): STRING(255)` | `Tc2_Utilities` POU, **declare-only** — surface lands now, calls fail compile with the new code; native formatting is a follow-up plan |
| `ADR` | **non-goal** — a function-like operator on the dialect axis, deferred to the future `POINTER TO` family work (interacts with ADR-0017/ADR-0021; a declared signature today would have to lie about its return type) |

The mechanism is not TwinCAT-specific: bindings, declare-only, and the new
problem code are generic library-format capabilities. The Tc2 libraries are
the first consumers because TwinCAT is a target.

## Non-goals

- The `ADR` address operator and the `POINTER TO` family (dialect axis; its
  own future design).
- A native `LREAL_TO_FMTSTR` implementation (declare-only now; follow-up
  plan adds a formatting builtin and flips the binding).
- Collision/precedence between libraries or vs. the base stdlib (design
  *Future Goals*; none of these names collide).
- F32/REAL variants of the new builtins — Beckhoff signatures are LREAL-only
  ([ADR-0022](../adrs/0022-exact-type-matching-for-function-arguments.md)
  exact matching); ADR-0004 permits adding F32 func_ids later.
- Migrating any existing compiler-seeded standard function to a library
  (ADR-0042 governs additions only).

## Design doc reference

[compatibility-libraries.md](../design/compatibility-libraries.md)
(`REQ-CL-*`) and
[compatibility-library-format.md](../design/compatibility-library-format.md)
(`REQ-LF-*`) — Phase 1 promotes *Bindings* out of both docs' future sections
and adds the new requirement markers listed there. Owning crate slugs:
`sources`, `codegen`, `analyzer`.

## Architecture

**Bindings are manifest data.** A per-version table in `library.toml` (the
version key must be quoted — `["1.0.0".bindings]`; unquoted `[1.0.0.bindings]`
is three nested TOML tables and is rejected by shape validation):

```toml
["1.0.0".bindings]
LTRUNC = { intrinsic = "trunc_lreal" }
LMOD   = { intrinsic = "fmod_lreal" }
```

or `POU = "declare-only"`. A bound or declare-only POU still appears in the
version's `.st` with its full interface and a body of exactly `;` (parses to
an empty statement list today — verified against `parser.rs`
`statements_or_empty()`); that body is never compiled.

**Binding info travels out of band.** The analyze merge erases which POUs came
from a library, and ADR-0040 rule 4 forbids provenance markers in the AST. So
bindings ride a side-table: new `ironplc_dsl::bindings` module with
`PouBinding::{Intrinsic { name }, DeclareOnly}` and `LibraryBindings`
(uppercased-POU-name map + the set of library-source `FileId`s), produced by
the `sources` loader (`CompatLibrary` gains a `bindings` field), threaded by
the CLI into a new `CodegenOptions::library_bindings` (`Default` = empty, so
untouched consumers — playground, benchmarks, MCP — compile unchanged and
fail closed to P9999 rather than ever lowering wrongly). The analyzer never
sees bindings — that is exactly what makes a declare-only call pass `check`.

**Codegen consumes bindings at two points.** (1) Body compilation: skip any
`FunctionDeclaration` whose name is bound *and* whose `FileId` is a library
file — the `FileId` check preserves user shadowing (REQ-CL-analyzer-004): a
user-defined `LMOD` still compiles as a user function. (2) Call lowering, in
`compile_function_call`'s fallthrough *after* the existing `user_functions`
check: `Intrinsic` compiles each argument at the parameter's op type and emits
`BUILTIN func_id` (per ADR-0008); `DeclareOnly` returns the new diagnostic at
the call site. Intrinsic names resolve via a single name→func_id table beside
the func_id constants (`container::opcode::builtin::intrinsic_func_id`), so
`sources` needs no new dependency for validation; unresolvable names in a
bundled manifest are caught by a conformance test, and defensively at codegen
with P6010 anchored on the manifest file.

**New VM builtins** (next free func_ids; pure-stack, so they go in
`vm/src/builtin.rs::dispatch`, not the inline string region):
`TRUNC_F64 = 0x03A3` (`f64::trunc`) and `FMOD_F64 = 0x03A4` (Rust `%`,
IEEE-754 sign-of-dividend; `x % 0.0` is NaN, not a trap — pinned in the
clean-room spec). Plus `arg_count` arms, disassembler arms, and wire-format
pins.

**New problem code** `P4046 LibraryFunctionNotImplemented` — "call to a
declare-only compatibility library function", naming the library and POU.

## File map

**New**
- `specs/adrs/0042-library-functions-over-compiler-intrinsics.md` — the intrinsic-vs-library rule (this PR).
- `specs/design/library-interfaces/tc2-math.md`, `tc2-utilities.md`, `bool-to-string.md` — clean-room interface specs from public Beckhoff InfoSys docs, each merged as its own non-squashed commit *before* implementation (authoring policy).
- `compiler/dsl/src/bindings.rs` — `PouBinding`, `LibraryBindings` (+ `lib.rs` export).
- `compiler/codegen/src/compile_library.rs` — bound-function pre-resolution and lowering helpers (`compile_call.rs` is at the 1000-line cap).
- `compiler/sources/resources/libs/Tc2_Math/{library.toml, 1.0.0/Tc2_Math.st}` — LTRUNC/LMOD (bound, `;` bodies), MODABS (ST body).
- `compiler/sources/resources/libs/Tc2_Utilities/{library.toml, 1.0.0/Tc2_Utilities.st}` — LREAL_TO_FMTSTR (declare-only).
- `docs/reference/compiler/problems/P4046.rst` (+ index entry).

**Modified**
- `specs/design/compatibility-library-format.md` — §Bindings specified (quoted-version-key shape, value forms, `;`-body rule, P6010 on malformed); new `REQ-LF-sources-005..007`.
- `specs/design/compatibility-libraries.md` — fail-if-unimplemented specified; new `REQ-CL-codegen-001` (intrinsic-bound call lowers to BUILTIN), `REQ-CL-codegen-002` (declare-only call fails compile with P4046), `REQ-CL-analyzer-007` (declare-only call passes check).
- `compiler/container/src/opcode.rs` — func_ids, `arg_count`, `intrinsic_func_id`.
- `compiler/vm/src/builtin.rs`, `compiler/project/src/disassemble.rs`, `compiler/codegen/tests/it/wire_format.rs`.
- `compiler/sources/src/libraries/manifest.rs` — bindings table parse + shape validation of *all* version tables (catches the unquoted-key trap); `mod.rs` — `CompatLibrary.bindings`, library `FileId` set.
- `compiler/sources/src/project.rs` — `load_activated_libraries` carries `CompatLibrary`s; adapt callers in `compiler/project/src/project.rs`, `compiler/ironplc-cli/src/cli.rs`.
- `compiler/codegen/src/compile.rs` (skip-body filter, `CodegenOptions.library_bindings`), `compile_call.rs` (fallthrough hook).
- `compiler/problems/resources/problem-codes.csv` — P4046.
- `compiler/playground/src/lib.rs` + `playground/` frontend — payload `{manifest, files[]}` (old raw-string form still accepted), bindings threading.
- `compiler/{sources,codegen,analyzer}/src/spec_conformance.rs` + `codegen/build.rs` — new `#[spec_test]`s.

## Testing strategy

- Each new `REQ-*` marker gets a `#[spec_test]` per the reconcile-spec
  convention; `#[ignore]` for markers landing in a later phase.
- Manifest: bindings happy path; each malformed shape → P6010 (including the
  unquoted version key); non-default-version tables validated but inert.
- Parser regression pinning the `;`-body idiom.
- Codegen (temp registry root, no bundled dependency): intrinsic call emits
  the exact func_id; declare-only call → P4046 on compile and clean on check
  (CLI-level split test); user function shadowing a bound name compiles the
  user body; bound `;` bodies never compiled.
- VM: `f64::trunc` ±, fmod negatives, fmod-by-zero → NaN not trap.
- End-to-end (`codegen/tests/it/`, VM-run, epsilon asserts):
  `LMOD(400.56, 360.0) ≈ 40.56`, `MODABS(-400.56, 360.0) ≈ 319.44`,
  `LTRUNC(3.7) = 3.0`, `LTRUNC(-3.7) = -3.0`, `BOOL_TO_STRING(TRUE) = 'TRUE'`;
  a `.plcproj`-driven activation test (the exact contributor scenario);
  `plc2plc` round-trip of a file calling `LTRUNC` is byte-identical.
- Conformance: every bundled intrinsic binding resolves via
  `intrinsic_func_id`; provenance test already covers the new manifests.
- `cd compiler && just` green per phase.

## Tasks

### Phase 0 — ADR + this plan (docs-only PR, for review)
- [ ] ADR-0042 and this plan committed and PR opened

### Phase 1 — Clean-room interface specs + design-doc promotion (docs)
- [ ] `library-interfaces/` specs from public Beckhoff references, each its own non-squashed commit (LMOD pinned to fmod sign-of-dividend; `LMOD(x, 0.0)` → NaN; `MODABS(-400.56, 360) = 319.44`; `BOOL_TO_STRING` → `'TRUE'`/`'FALSE'`)
- [ ] Promote §Bindings in both design docs; add `REQ-LF-sources-005..007`, `REQ-CL-codegen-001/002`, `REQ-CL-analyzer-007`

### Phase 2 — Container + VM primitives
- [ ] `TRUNC_F64`/`FMOD_F64` func_ids, `arg_count`, `intrinsic_func_id` table
- [ ] `vm/builtin.rs` arms; disassembler arms; wire-format pins; VM unit tests
- [ ] Verify STRING return from user-defined functions (needed by `BOOL_TO_STRING`'s ST body); implement as a general codegen capability if missing
- [ ] `cd compiler && just` green

### Phase 3 — Bindings model: dsl type, manifest parse, loader threading
- [ ] `dsl/src/bindings.rs`; manifest bindings parse + shape validation → P6010 — **REQ-LF-sources-005/006/007**
- [ ] `CompatLibrary.bindings` + library `FileId` set; thread through `load_activated_libraries` callers
- [ ] Parser `;`-body regression test; spec tests
- [ ] `cd compiler && just` green

### Phase 4 — Codegen: builtin lowering, declare-only P4046, skip-body
- [ ] P4046 CSV row + `P4046.rst`
- [ ] `CodegenOptions.library_bindings`; `compile_library.rs` pre-resolution; skip-body filter; fallthrough hook — **REQ-CL-codegen-001/002**, **REQ-CL-analyzer-007**
- [ ] CLI threads bindings; check/compile split test; shadowing test; bundled-bindings-resolve conformance test
- [ ] `cd compiler && just` green

### Phase 5 — Ship `Tc2_Math` + `Tc2_Utilities`; `BOOL_TO_STRING`; end-to-end
- [ ] `Tc2_Math` package (manifest references per function; MODABS ST body `MODABS := LMOD(IN, IM); IF MODABS < 0.0 THEN MODABS := MODABS + ABS(IM); END_IF;`)
- [ ] `Tc2_Utilities` package (LREAL_TO_FMTSTR declare-only)
- [ ] `BOOL_TO_STRING` ST function — **placement to confirm in review**: recommend the bundled `Tc2_System` library (TwinCAT treats it as a compiler operator, so no vendor library is its true home; `Tc2_System` is referenced by real projects by default, so it lights up on the paved path)
- [ ] End-to-end VM-run tests; `.plcproj` activation test; plc2plc round-trip
- [ ] `cd compiler && just` green

### Phase 6 — Playground bindings threading
- [ ] Serve `{manifest, files[]}`; build `LibraryBindings` in `playground/src/lib.rs`; thread through `CodegenOptions`
- [ ] `cd compiler && just` green

## Implementation notes

- Order matters in `compile_function_call`: `ctx.user_functions` first, then
  bindings — user shadowing stays intact with zero new mechanism.
- Interim playground behavior (before Phase 6) is safe: an intrinsic-bound
  call falls through to `compile_generic_builtin` → P9999 — fails closed,
  never silently wrong.
- Risk: `MODABS`-via-ST assumes function-name return assignment works in
  library bodies (believed supported by `compile_user_function`); verify at
  the start of Phase 5 — fallback is a `modabs_lreal` builtin at `0x03A5`.
- Risk: STRING-returning user functions may be unimplemented in codegen; if
  so it becomes a Phase 2 general capability (benefits every future library),
  and `BOOL_TO_STRING` ships declare-only until it lands — never a builtin.
- Once the phases land, PRs #1217/#1218/#1246 are superseded: same names,
  same behavior, delivered per ADR-0042 (`ADR` deferred to the `POINTER TO`
  work; native `LREAL_TO_FMTSTR` as a follow-up plan).
