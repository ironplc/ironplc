# Plan: `ADR` operator and `POINTER TO` type (TwinCAT/CODESYS pointer model)

## Goal

Support the TwinCAT/CODESYS pointer feature family so that idiomatic code like
this compiles and runs:

```iecst
FUNCTION_BLOCK FB_Point
VAR
   pNumber : POINTER TO INT;
   iNumber1 : INT := 5;
   iNumber2 : INT;
END_VAR
pNumber := ADR(iNumber1);
iNumber2 := pNumber^;
```

Reference: [Beckhoff ADR](https://infosys.beckhoff.com/content/1033/tc3_plc_intro/2529015179.html),
[Beckhoff POINTER](https://infosys.beckhoff.com/content/1033/tc3_plc_intro/2529453451.html),
[CODESYS ADR](https://content.helpme-codesys.com/en/CODESYS%20Development%20System/_cds_operator_adr.html).

This is the follow-on work explicitly deferred by
[2026-07-21-reference-to-keyword-support.md](2026-07-21-reference-to-keyword-support.md)
("`POINTER TO` and the `ADR()`/`^` pointer model (a separate,
explicitly-dereferenced type; its own future flag)") and by
[specs/design/reference-to-twincat.md](../design/reference-to-twincat.md).

Per [ADR-0042](../adrs/0042-library-functions-over-compiler-intrinsics.md)
rule 4, `ADR` is a **language extension on the dialect axis** — a
function-like operator requiring compile-time symbol knowledge that no
library function could have — so it is implemented in the compiler and
gated by `--allow-*` flags. It must NOT be added to
`compiler/sources/resources/libs/`.

### Semantics mapping: TwinCAT memory model → IronPLC memory model

TwinCAT's `ADR(x)` yields the byte address of `x` as `PVOID` (an opaque
pointer-width integer — `DWORD` on 32-bit targets, `LWORD` on 64-bit). The
result is assigned to a `POINTER TO T` variable and dereferenced with the
postfix `^` operator.

IronPLC's VM has no byte addresses for variables
([ADR-0017](../adrs/0017-unified-data-region.md),
[ADR-0021](../adrs/0021-flat-variable-table-for-function-calls.md)): a
variable is a slot in a flat 64-bit slot table, indexed by `VarIndex`;
variable-length data lives in a data region addressed by offsets held in
slots. The existing reference backend already models "the address of a
variable" as **the variable's table index stored as a u64**
(`compile_expr.rs`, `ExprKind::Ref` → push `VarIndex`; `ExprKind::Deref` →
`LOAD_INDIRECT`; `NULL` → sentinel `u64::MAX`).

Consequently:

1. **`ADR(x)` lowers exactly like `REF(x)`** — push the argument's variable
   table index. No new opcodes, no VM changes, no wire-format changes.
2. **`POINTER TO T` maps to the existing `IntermediateType::Reference`**
   with *explicit* dereference (`^` required), i.e. the same intermediate
   type as `REF_TO T` under a third surface syntax (alongside `REF_TO` and
   `REFERENCE TO`).
3. **`PVOID` is not needed and is not introduced.** `PVOID` exists in
   TwinCAT only because its pointers are untyped machine addresses. In
   IronPLC, `ADR(x)` has the inferred type `POINTER TO typeof(x)` and is
   type-checked against the destination pointer's target type. This is
   strictly safer than TwinCAT and matches how the existing reference
   types already behave. (If a future compatibility-library interface
   needs the *name* `PVOID`, it can be added later as a type alias; out of
   scope here.)
4. **What cannot be mapped** and is diagnosed instead (see Out of scope):
   pointer arithmetic, sub-object addresses (`ADR(arr[i])`,
   `ADR(struct.field)` — slot indices cannot name a byte offset inside the
   data region), and address-as-integer conversions (`LWORD_TO_PVOID`
   etc.).

## Status & handoff notes

- Nothing is implemented yet; this plan is the first commit on the branch.
- Line anchors below are guides that will have drifted — re-locate by
  symbol, not line number.
- Automatically wired surfaces when a flag is added via
  `define_compiler_options!`: MCP (`set_flag_by_key`) and playground
  (`FEATURE_DESCRIPTORS`). Manually wired: CLI clap arg + overlay
  (`ironplc-cli/bin/main.rs`), LSP (`ironplc-cli/src/lsp.rs`
  `extract_compiler_options`), and the mandatory behavioral fixture in
  `compiler/mcp/src/feature_flag_conformance.rs`.
- The exact-set dialect tests in `parser/src/options.rs`
  (`twincat_dialect_enables...`, `codesys_dialect_enables...`) will fail
  the build until updated — that is the intended drift guard.

## Architecture

Two flags (per the syntax-support-guide rule "one flag per construct;
avoid umbrella flags like `allow_pointer_features`"), both enabled in the
**TwinCAT** and **CODESYS** dialect presets:

- `--allow-pointer-to` — parse `POINTER TO T` in variable/type
  declarations, mapped to the reference intermediate type with explicit
  `^` dereference.
- `--allow-adr` — the `ADR(...)` operator.

Mechanisms, in pipeline order:

1. **Parser — `POINTER TO T`.** Add a `RefSyntax::PointerTo` variant
   (`dsl/src/common.rs`) alongside `RefTo`/`ReferenceTo` so plc2plc can
   round-trip the exact spelling. Grammar follows the existing
   `REFERENCE TO` two-token pattern (`parser.rs` type constructors). No
   new keyword token: follow the `REFERENCE`/`TO` identifier-sequence
   approach so `POINTER` remains usable as an identifier when the flag is
   off (no `xform_demote_keywords` change expected).
2. **Parser — `ADR`.** No parser change at all: `ADR(x)` parses today as
   an ordinary function call, exactly like `SIZEOF` (which is not a
   token). plc2plc round-trips it for free.
3. **Analyzer — `ADR` typing.** `ADR`'s return type depends on its
   argument (`POINTER TO typeof(arg)`), which a stdlib
   `FunctionSignature` cannot express. Instead of a signature, rewrite
   the call early: when `allow_adr` is on, an xform (extend
   `xform_resolve_late_bound_expr_kind` or a new `xform_resolve_adr`)
   rewrites `Function("ADR", [expr])` → `ExprKind::Ref(variable)`. All
   existing reference type inference, assignment checking, and codegen
   then apply unchanged. Wrong arity / non-variable argument gets a
   dedicated diagnostic at the rewrite site.
   - The rewrite must not require `allow_ref_to`: gate the `Ref`
     validation rules on "produced by `ADR`" contexts as needed (verify
     in `rule_ref_to.rs` that its checks are keyed off syntax flags in a
     way that admits the rewritten node, and adjust gating if not).
   - With the flag off, `ADR(x)` falls through to the normal undeclared-
     function path (P4017), matching `SIZEOF` behavior.
4. **Analyzer — operand restrictions.** The existing `REF()` restrictions
   apply identically and are reused where the message text fits:
   P2028 (operand must be a simple variable — covers `ADR(arr[i])`,
   `ADR(s.field)`, `ADR(literal)`), P2029 (ephemeral/stack variable),
   P2030 (array element). Reuse the codes; if `ADR`-specific wording is
   needed, add new `P2xxx` codes (next free after P2036) with matching
   `docs/compiler/problems/P####.rst` pages.
5. **Analyzer — `POINTER TO` semantics.** `POINTER TO T` behaves as
   `REF_TO T` (explicit deref, `NULL` comparable, P2031 on deref of
   non-reference, P2032 on target-type mismatch, P2033 on arithmetic).
   `xform_insert_implicit_deref` must NOT treat it as `REFERENCE TO`
   (no implicit deref).
6. **Codegen.** Zero changes expected: after the rewrite, `ADR` is
   `ExprKind::Ref` and `POINTER TO` variables are reference-typed slots.
   Verify `resolve_variable`-based lowering and `LOAD_INDIRECT` deref
   end-to-end rather than adding a `compile_adr`.

### Design doc reference

First implementation task: write `specs/design/adr-and-pointer-to.md`
with `**REQ-PTR-<crate-slug>-NNN**` requirement IDs (pick the `PTR`
area prefix after confirming it is unused) and `#[spec_test(...)]`
conformance tests per
[Development Standards — Design Requirement](../steering/development-standards.md).
It supersedes the "out of scope" notes in
`specs/design/reference-to-twincat.md` (§ pointer model) and the sketch in
`specs/design/beckhoff-twincat-dialect.md` (§2.1, `TypeSpec::PointerTo`).

## File map

### Phase 1 — flags + `POINTER TO` declarations

| File | Change |
|---|---|
| `specs/design/adr-and-pointer-to.md` | New design doc with REQ-PTR IDs |
| `compiler/parser/src/options.rs` | Add `allow_pointer_to` and `allow_adr` to `define_compiler_options!`; presets `[Codesys, TwinCat]`; update exact-set dialect tests; fix stale "`POINTER TO` … not parsed yet" doc comments |
| `compiler/dsl/src/common.rs` | `RefSyntax::PointerTo` variant + rendering |
| `compiler/parser/src/parser.rs` | Parse `POINTER TO` type constructor (gated), following `REFERENCE TO` pattern |
| `compiler/plc2plc/src/renderer.rs` | Render `POINTER TO` for `RefSyntax::PointerTo` |
| `compiler/ironplc-cli/bin/main.rs` | Clap args + `\|=` overlays for both flags |
| `compiler/ironplc-cli/src/lsp.rs` | `extract_compiler_options` camelCase keys + tests; fix stale comment |
| `compiler/mcp/src/feature_flag_conformance.rs` | Behavioral fixtures for both flags |
| `compiler/analyzer/src/rule_ref_to.rs` (and/or new `rule_pointer_to.rs`) | Explicit-deref semantics for `PointerTo`; ensure implicit-deref xform excludes it |
| `compiler/resources/test/pointer_to.st` + `compiler/plc2plc/resources/test/pointer_to_rendered.st` | Round-trip fixtures |
| `compiler/plc2plc/src/tests/pointer_to.rs` (+ `mod` line) | Round-trip test (new file) |
| `compiler/parser/src/tests/pointer_to.rs` (+ `mod` line) | Parse accept/reject tests |

### Phase 2 — `ADR` operator

| File | Change |
|---|---|
| `compiler/analyzer/src/xform_resolve_late_bound_expr_kind.rs` (or new `xform_resolve_adr.rs`) | Rewrite `ADR(var)` call → `ExprKind::Ref` when `allow_adr`; diagnostics for bad arity/operand |
| `compiler/analyzer/src/rule_ref_to.rs` | Confirm/adjust gating so `ADR`-produced `Ref` nodes are validated without requiring `allow_ref_to` |
| `compiler/problems/resources/problem-codes.csv` + `docs/compiler/problems/P####.rst` | Only if `ADR`-specific codes are needed beyond P2028–P2030 |
| `compiler/resources/test/adr.st` + `compiler/plc2plc/resources/test/adr_rendered.st` | Round-trip fixtures |
| `compiler/plc2plc/src/tests/adr.rs` (+ `mod` line) | Round-trip test |
| `compiler/codegen/tests/it/end_to_end_adr.rs` (+ `mod` in `tests/it/main.rs`) | Execution tests: assign/deref, FB instance address, store-through (`p^ := v`), NULL guard, flag-off → P4017 |
| `compiler/codegen/tests/it/end_to_end_dialect.rs` | TwinCAT/CODESYS dialect presets enable the whole example from Goal |

### Phase 3 — docs and stale-comment sweep

| File | Change |
|---|---|
| `docs/explanation/enabling-dialects-and-features.rst` | Replace "(Pointer types `POINTER TO` with `ADR()` are not parsed yet.)"; add flags to dialect Enables lists |
| `docs/reference/compiler/ironplcc.rst` | `--allow-pointer-to`, `--allow-adr` entries |
| `docs/reference/extension-library/functions/adr.rst` + `index.rst` | Feature doc alongside `sizeof.rst`, including the not-mapped limitations |
| `docs/reference/editor/settings.rst` | Update `POINTER TO` note |
| `integrations/vscode/package.json` | Update `ironplc.dialect` markdown description |
| `specs/steering/syntax-support-guide.md` | Update TwinCAT dialect-table row |
| `specs/design/reference-to-twincat.md` | Note the deferral is now implemented (link design doc) |

## Tasks

### Phase 1 — `POINTER TO`
- [ ] Write `specs/design/adr-and-pointer-to.md` with REQ-PTR requirement IDs (own commit, before implementation)
- [ ] Add `allow_pointer_to` + `allow_adr` flags; update exact-set dialect tests (`twincat_...`, `codesys_...`)
- [ ] Wire CLI + LSP surfaces; add `feature_flag_conformance` fixtures
- [ ] `RefSyntax::PointerTo` in dsl + parser grammar + plc2plc renderer
- [ ] Parser tests: `pointer_to_when_flag_enabled_then_parses`, `pointer_to_when_flag_disabled_then_rejected`
- [ ] Analyzer: explicit-deref semantics; exclude from implicit-deref xform; type-mismatch (P2032) and deref (P2031) coverage
- [ ] plc2plc round-trip fixture + test
- [ ] `cd compiler && just` — all checks pass

### Phase 2 — `ADR`
- [ ] Analyzer rewrite `ADR(var)` → `ExprKind::Ref` gated by `allow_adr`; arity/operand diagnostics; verify `rule_ref_to` gating admits it without `allow_ref_to`
- [ ] Negative tests: `ADR` of array element / struct field / literal / call result → P2028/P2030; flag off → P4017
- [ ] End-to-end tests in `end_to_end_adr.rs`: Goal example runs (deref yields 5); `p^ := v` store-through; FB instance address; `NULL` compare
- [ ] Dialect test: TwinCAT + CODESYS presets compile the Goal example with no explicit flags
- [ ] `cd compiler && just` — all checks pass

### Phase 3 — docs
- [ ] Docs + stale-comment sweep per Phase 3 file map
- [ ] `cd compiler && just` — all checks pass

## Out of scope

- **`PVOID` as a named type.** Not needed: `ADR` returns a typed pointer.
  Revisit only if a compatibility-library interface requires the name.
- **Pointer arithmetic** (`p + 2`, `p - p`) — meaningless on table
  indices; rejected via existing P2033. TwinCAT code relying on it (byte
  walking, `MEMCPY` patterns) cannot be mapped to this memory model.
- **Sub-object addresses**: `ADR(arr[i])`, `ADR(struct.field)`,
  `ADR(string[3])` — slot indices cannot express data-region byte
  offsets; rejected via P2028/P2030. Lifting this needs a fat-pointer
  representation (slot index + byte offset) and is its own future design.
- **`BITADR`, `INDEXOF`, `__ADRINST`, address-to-integer conversions**
  (`LWORD_TO_PVOID` etc.).
- **`MEMCPY`/`MEMSET`-style Tc2_System functions** that consume `ADR`
  results — would require VM builtins taking references; separate
  compatibility-library + builtin work per ADR-0042 rule 3.
- **Online-change pointer semantics** from the Beckhoff docs — IronPLC has
  no online change; nothing to map.
