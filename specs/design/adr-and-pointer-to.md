# Design: `ADR` Operator and `POINTER TO` Type

## Overview

This document describes support for the TwinCAT/CODESYS pointer feature
family — the `POINTER TO T` type, the postfix `^` dereference, and the
`ADR()` address-of operator — so that idiomatic code like this compiles and
runs:

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

It is the follow-on work deferred by
[reference-to-twincat.md](reference-to-twincat.md) and supersedes the
"out of scope" pointer notes there and the parse-only sketch in
[beckhoff-twincat-dialect.md](beckhoff-twincat-dialect.md) §2.1
(`TypeSpec::PointerTo`).

References: [Beckhoff ADR](https://infosys.beckhoff.com/content/1033/tc3_plc_intro/2529015179.html),
[Beckhoff POINTER](https://infosys.beckhoff.com/content/1033/tc3_plc_intro/2529453451.html),
[CODESYS ADR](https://content.helpme-codesys.com/en/CODESYS%20Development%20System/_cds_operator_adr.html).

## Semantics mapping: TwinCAT memory model → IronPLC memory model

TwinCAT's `ADR(x)` yields the byte address of `x` as `PVOID` (an opaque
pointer-width integer). The result is assigned to a `POINTER TO T` variable
and dereferenced with the postfix `^` operator.

IronPLC's VM has no byte addresses for variables
([ADR-0017](../adrs/0017-unified-data-region.md),
[ADR-0021](../adrs/0021-flat-variable-table-for-function-calls.md)): a
variable is a slot in a flat 64-bit slot table, indexed by `VarIndex`. The
existing reference backend already models "the address of a variable" as the
variable's table index stored as a `u64` (`ExprKind::Ref` pushes the
`VarIndex`; `ExprKind::Deref` emits `LOAD_INDIRECT`; `NULL` is the sentinel
`u64::MAX`). Consequently:

1. **`ADR(x)` lowers exactly like `REF(x)`** — push the argument's variable
   table index. No new opcodes, no VM changes, no wire-format changes.
2. **`POINTER TO T` maps to the existing reference intermediate type** with
   *explicit* dereference (`^` required) — the same intermediate type as
   `REF_TO T`, under a third surface syntax (alongside `REF_TO` and
   `REFERENCE TO`).
3. **`PVOID` is not introduced.** In IronPLC, `ADR(x)` has the inferred type
   `POINTER TO typeof(x)` and is type-checked against the destination
   pointer's target type. This is strictly safer than TwinCAT and matches how
   the existing reference types already behave.
4. **What cannot be mapped** is diagnosed instead (see Out of scope):
   pointer arithmetic, sub-object addresses (`ADR(arr[i])`,
   `ADR(struct.field)`), and address-as-integer conversions.

## Delivery

The feature is delivered in phases:

- **Phase 1 — `POINTER TO` declarations (this document's requirements
  `0xx`–`3xx` and `6xx`).** The `allow_pointer_to` flag, the `POINTER`
  keyword with demotion, parser productions reusing the `REF_TO` grammar and
  AST (`RefSyntax::PointerTo` tag), explicit-`^` semantics in the analyzer,
  and plc2plc round-tripping. In this phase a `POINTER TO` variable is bound
  via the existing `REF()` initializer/assignment forms (available when
  `allow_ref_to` is also on, as in the `codesys` dialect) — `ADR` itself is
  Phase 2.
- **Phase 2 — the `ADR()` operator and `allow_adr` flag (requirements
  `4xx`–`5xx`).** An analyzer rewrite of `Function("ADR", [expr])` →
  `ExprKind::Ref(variable)` when `allow_adr` is on, with diagnostics for
  invalid operands. The `allow_adr` flag lands together with the rewrite
  (not in Phase 1) because every feature flag must gate real behavior the
  moment it is declared — the MCP `feature_flag_conformance` suite has no
  "declared but not yet enforced" escape hatch, and a flag whose behavior
  does not exist yet would be a dead flag.
- **Phase 3 — documentation sweep.** User docs, dialect tables, and stale
  comments.

## Gating & coexistence

`POINTER TO` is gated behind `--allow-pointer-to`, following the established
token-demotion pattern used by `REFERENCE TO`: a `POINTER` keyword token is
demoted to `Identifier` unless the flag is set, so standard programs may use
`POINTER` as an ordinary name. The `twincat` and `codesys` dialect presets
enable the flag.

Per [ADR-0042](../adrs/0042-library-functions-over-compiler-intrinsics.md)
rule 4, `ADR` is a language extension on the dialect axis — a function-like
operator requiring compile-time symbol knowledge that no library function
could have — so it is implemented in the compiler (Phase 2) and must NOT be
added to the bundled libraries.

Per [ADR-0038](../adrs/0038-no-restrictions-on-flag-combinations.md), no
flag combination is rejected: `REF_TO`, `REFERENCE TO`, and `POINTER TO`
may all be enabled together. Coexistence stays well-defined because each
declaration carries a `RefSyntax` tag; the implicit-dereference transform
keys on `RefSyntax::ReferenceTo` only, so `POINTER TO` (like `REF_TO`)
always keeps explicit-`^` semantics.

### Options & dialects

- **REQ-PTR-parser-001** The `codesys` dialect preset enables `allow_pointer_to`.
- **REQ-PTR-parser-002** The `twincat` dialect preset enables `allow_pointer_to`.
- **REQ-PTR-parser-003** The `iec61131-3-ed2`, `iec61131-3-ed3`, and `rusty` dialect presets do not enable `allow_pointer_to`.

## Lexer & keyword demotion

`POINTER` becomes a keyword token, demoted to an identifier when the flag is
off — exactly how `REFERENCE` is handled. The `TO` keyword already exists;
the type constructor is the two-token sequence `POINTER TO`.

- **REQ-PTR-parser-100** `POINTER` lexes as a single `Pointer` keyword token (distinct from any identifier).
- **REQ-PTR-parser-101** With `allow_pointer_to` off, `POINTER` is demoted to `Identifier`.
- **REQ-PTR-parser-102** With `allow_pointer_to` on, `POINTER` stays the `Pointer` keyword.
- **REQ-PTR-parser-103** `POINTER` is a valid identifier (e.g. a variable name) in standard mode.

## Parser productions & AST tagging

The grammar reuses the existing reference productions: the `ref_to_keyword`
rule gains a `POINTER TO` alternative returning a new `RefSyntax::PointerTo`
tag, which flows into the shared `ReferenceDeclaration`,
`ReferenceInitializer`, and `ArraySubranges.ref_to` nodes. No new AST node
shapes are introduced, so every declaration position that accepts `REF_TO`
(variable declarations, `TYPE` declarations, array element types) accepts
`POINTER TO` under the flag.

```rust
pub enum RefSyntax {
    RefTo,        // IEC 61131-3 REF_TO
    ReferenceTo,  // TwinCAT / CODESYS REFERENCE TO
    PointerTo,    // TwinCAT / CODESYS POINTER TO
}
```

- **REQ-PTR-parser-200** `p : POINTER TO INT;` yields a reference initializer tagged `RefSyntax::PointerTo`.
- **REQ-PTR-parser-201** `TYPE T : POINTER TO INT; END_TYPE` yields a reference declaration tagged `RefSyntax::PointerTo`.
- **REQ-PTR-parser-210** `ARRAY [..] OF POINTER TO T` tags the element type `Some(RefSyntax::PointerTo)`.
- **REQ-PTR-parser-211** With `allow_pointer_to` off, `p : POINTER TO INT;` is a syntax error.

## Analyzer semantics

`POINTER TO T` behaves as `REF_TO T`: explicit `^` dereference, `NULL`
comparable and assignable, and the existing reference rules apply unchanged
(P2031 deref of non-reference, P2032 target-type mismatch, P2033
arithmetic, P2035 ordering comparisons). The implicit-dereference transform
(`xform_insert_implicit_deref`) keys on `RefSyntax::ReferenceTo` and must
not treat `PointerTo` as auto-dereferencing.

- **REQ-PTR-analyzer-300** Reading through an explicit dereference of a `POINTER TO` variable (`value := p^;`) is accepted.
- **REQ-PTR-analyzer-301** A bare use of a `POINTER TO` variable is not implicitly dereferenced, even when `allow_reference_to` is also enabled.
- **REQ-PTR-analyzer-302** Binding a `POINTER TO T` variable to a reference of a different base type is rejected (P2032) unless `allow_ref_type_punning` is set.
- **REQ-PTR-analyzer-303** Arithmetic on a `POINTER TO` value is rejected (P2033) unless `allow_ref_arithmetic` is set.
- **REQ-PTR-analyzer-304** `NULL` may be assigned to a `POINTER TO` variable (when the `NULL` keyword is available via `allow_ref_to`).

## The `ADR()` operator (Phase 2)

`ADR` needs no parser change: `ADR(x)` parses as an ordinary function call,
exactly like `SIZEOF` (which is not a token), and plc2plc round-trips it for
free. `ADR`'s return type depends on its argument (`POINTER TO typeof(x)`),
which a stdlib `FunctionSignature` cannot express, so instead of registering
a signature the analyzer rewrites the call early: when `allow_adr` is on, a
transform rewrites `Function("ADR", [expr])` → `ExprKind::Ref(variable)`.
All existing reference type inference, assignment checking (P2032), and
codegen then apply unchanged.

The rewrite does not require `allow_ref_to`: the reference semantic rules
validate `ExprKind::Ref` nodes unconditionally, so an `ADR`-produced node is
checked (and accepted) even when the `REF_TO`/`REF()`/`NULL` keywords are
unavailable — as in the `twincat` dialect.

Invalid operands reuse the `REF()` diagnostics: P2028 (operand must be a
simple variable — covers literals, call results, struct fields, and wrong
arity), P2029 (ephemeral/stack variable), and P2030 (array element). With
the flag off, `ADR` stays an ordinary identifier and the call falls through
to the normal undeclared-function diagnostic (P4017), matching `SIZEOF`
behavior.

### Options & dialects (Phase 2)

- **REQ-PTR-parser-400** The `codesys` dialect preset enables `allow_adr`.
- **REQ-PTR-parser-401** The `twincat` dialect preset enables `allow_adr`.
- **REQ-PTR-parser-402** The `iec61131-3-ed2`, `iec61131-3-ed3`, and `rusty` dialect presets do not enable `allow_adr`.

### Analyzer rewrite & diagnostics

- **REQ-PTR-analyzer-410** With `allow_adr` on, `p := ADR(x);` binding a `POINTER TO T` variable to a variable of type `T` is accepted, without requiring `allow_ref_to`.
- **REQ-PTR-analyzer-411** With `allow_adr` off, `ADR(x)` is reported as an undeclared function (P4017).
- **REQ-PTR-analyzer-412** An `ADR` call with a number of arguments other than one is rejected (P2028).
- **REQ-PTR-analyzer-413** An `ADR` operand that is not a variable (a literal or a call result) is rejected (P2028).
- **REQ-PTR-analyzer-414** `ADR` of an array element is rejected (P2030); `ADR` of a structure field is rejected (P2028) — slot indices cannot name a sub-object.
- **REQ-PTR-analyzer-415** Binding `ADR(x)` to a pointer whose target type differs from `typeof(x)` is rejected (P2032) unless `allow_ref_type_punning` is set.
- **REQ-PTR-analyzer-416** `ADR` of a stack-allocated variable (`VAR_TEMP`) is rejected (P2029) unless `allow_ref_stack_variables` is set.

### Execution (codegen)

`ADR` lowers exactly like `REF()` — the rewrite happens before codegen, so
codegen needs zero changes and these requirements pin the end-to-end
behavior.

- **REQ-PTR-codegen-500** The Goal example executes: inside a function-block instance called from a `PROGRAM`, `pNumber := ADR(iNumber1); iNumber2 := pNumber^;` yields the addressed member's value.
- **REQ-PTR-codegen-501** Storing through an `ADR`-bound pointer (`p^ := v`) updates the addressed variable.
- **REQ-PTR-codegen-502** An `ADR`-bound pointer compares non-equal to `NULL`, and an unbound pointer defaults to `NULL` (dereferencing it traps).
- **REQ-PTR-codegen-510** The `twincat` and `codesys` dialect presets compile and run the Goal example with no explicit flags.

## Rendering (plc2plc)

The renderer reproduces the surface spelling from the `RefSyntax` tag, so
source using `POINTER TO` round-trips without being normalized to `REF_TO`.

- **REQ-PTR-plc2plc-600** A `PointerTo`-tagged declaration renders as `POINTER TO`.
- **REQ-PTR-plc2plc-601** `REF_TO`, `REFERENCE TO`, and `POINTER TO` declarations in one program each render with their own spelling preserved.

## Out of scope

- **`PVOID` as a named type.** Not needed: `ADR` returns a typed pointer.
  Revisit only if a compatibility-library interface requires the name.
- **Pointer arithmetic** (`p + 2`, `p - p`) — meaningless on table indices;
  rejected via existing P2033. TwinCAT code relying on it (byte walking,
  `MEMCPY` patterns) cannot be mapped to this memory model.
- **Sub-object addresses**: `ADR(arr[i])`, `ADR(struct.field)`,
  `ADR(string[3])` — slot indices cannot express data-region byte offsets;
  rejected via P2028/P2030. Lifting this needs a fat-pointer representation
  (slot index + byte offset) and is its own future design.
- **`BITADR`, `INDEXOF`, `__ADRINST`, address-to-integer conversions**
  (`LWORD_TO_PVOID` etc.).
- **`MEMCPY`/`MEMSET`-style Tc2_System functions** that consume `ADR`
  results — separate compatibility-library + builtin work per ADR-0042
  rule 3.
- **Online-change pointer semantics** — IronPLC has no online change.
