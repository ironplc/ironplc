# Plan: Inline FB-Instance Call-Style Initializer as a Distinct AST Node

## Goal

Add support for the CODESYS/TwinCAT call-style function-block instance
initializer — passing an argument list directly after the type name, using
the same positional-or-named parameter shape as an ordinary FB call:

```iecst
FUNCTION_BLOCK FB_Example
VAR
    comm : FB_Comm(retries := 3, THIS);   -- currently a parse error
END_VAR
END_FUNCTION_BLOCK
```

Part of #1199 (TwinCAT dialect support). Supersedes the design in PR #1270,
which modeled the feature as an `Option<Vec<ParamAssignmentKind>>` field on
the existing `FunctionBlockInitialValueAssignment` node.

## Why a distinct node (not a field on the existing node)

The two initializer forms are **semantically different constructs**, not two
spellings of one thing (confirmed against the CODESYS/Beckhoff `FB_init`
docs):

- `inst : FB_Type := (member := value, ...)` — sets the instance's own
  member/`VAR_INPUT` initial values directly. This is the existing
  `InitialValueAssignmentKind::FunctionBlock` / `Structure` handling.
- `inst : FB_Type(arg, ...)` — in CODESYS, passes arguments to the FB's
  `FB_init` **method** (a constructor). The parameters are `FB_init`'s, not
  necessarily the FB's members, and positional arguments (`THIS`) are
  possible — which cannot be expressed in the member-initializer grammar at
  all.

Modeling the second form as an optional field on the first is a discriminant
flag: it encodes "which of two meanings" in a field value rather than in the
type. That forces every consumer to remember a by-convention "mutually
exclusive" invariant and leaves the ambiguity live at codegen. Two meanings
→ two nodes. With a distinct variant:

- Illegal states (both forms at once) are unrepresentable.
- Every `match InitialValueAssignmentKind` is forced by the compiler to
  decide what the call form means — exhaustiveness is the checklist.
- Codegen dispatches on the variant; no field-inspection ambiguity.

## Scope

**Params-only.** `FunctionBlockCallInitializer` carries only
`{ type_name, params }`. The combined CODESYS array form
(`FB_Sample[(initParam := 4)] := [(member := 5)]`, constructor args *and*
member inits together) is out of scope; if it is ever needed, an explicit
member-init field is added deliberately then — not as a hidden flag now.

**Deferring semantic rule, not codegen.** ironplc models no `FB_init`
method today, and applying constructor arguments is a larger feature.
Rather than silently ignore the arguments (as the existing member-init form
is silently ignored today), the call form is parsed and stored, then a
dedicated semantic rule emits the existing **P9999** (`NotImplemented`)
diagnostic — constructed via `Diagnostic::not_implemented`, which carries
the IEC 61131-3 source span while recording the compiler call site. This
mirrors the existing `rule_unsupported_stdlib_type` / P9001 pattern (no new
problem code is minted; P9004 is reserved for the planned general
`UnsupportedExtension` framework). When real `FB_init` codegen lands, the
rule is deleted and a codegen arm fills in — a one-place change.

## DSL

```rust
// compiler/dsl/src/common.rs
#[derive(Clone, PartialEq, Debug, Recurse)]
pub struct FunctionBlockCallInitializer {
    pub type_name: TypeName,
    pub params: Vec<ParamAssignmentKind>,
}

pub enum InitialValueAssignmentKind {
    // ...
    FunctionBlock(FunctionBlockInitialValueAssignment),  // unchanged (member init)
    FunctionBlockCall(FunctionBlockCallInitializer),     // new (constructor call)
    // ...
}
```

Plus `dispatch!(FunctionBlockCallInitializer);` in `dsl/src/visitor.rs` and
`dsl/src/fold.rs` (generates `visit_function_block_call_initializer` and the
fold method; the `Recurse` derive on the enum generates the dispatch arm).

## Parser

`fb_call_style_var_decl()` builds `FunctionBlockCall(FunctionBlockCallInitializer
{ type_name, params })` directly — no `Option`. Uses `var1_list()` and
requires the parens unconditionally (so a bare `name : Type;` still flows
through the late-bound path, and the `:=` member form is untouched). Added
to `var_init_decl()`'s ordered choice after `ref_to_var_init_decl()`. (The
old dead `fb_name_decl()` rule was removed from the grammar in #1275.)

## Analyzer

`FunctionBlockCall` references an FB type, so it must participate in the same
type-reference machinery as `FunctionBlock` (otherwise a call-style program
gets spurious P2011 / unknown-type errors *in addition to* the intended
P9999):

- `xform_toposort_declarations` — add the referenced-type-before-POU edge
  (same as the `FunctionBlock` arms Agent 2 fixed in #1269).
- `xform_resolve_type_decl_environment`, `xform_resolve_late_bound_expr_kind`,
  `xform_resolve_expr_types`, `intermediates/structure.rs` — treat as an FB
  type reference.
- Semantic rules that inspect `FunctionBlock` (`rule_function_block_invocation`,
  `rule_var_decl_const_not_fb`, `rule_var_decl_const_initialized`) — same
  treatment.

New `rule_function_block_call_unsupported` emits P9999 (`NotImplemented`) on
every `FunctionBlockCall` node; registered in `stages.rs::semantic()`.

## Codegen / plc2plc / MCP

- `codegen/compile_setup.rs` — the `FunctionBlockCall` node never reaches
  successful codegen because the P9999 rule fails analysis first, and the
  existing matches use wildcard arms, so no dedicated codegen arm is added
  (consistent with the "deferred" intent).
- `plc2plc/renderer.rs` — `visit_function_block_call_initializer` renders
  `Type ( params )`.
- `mcp/pou_lineage.rs` — records the FB-type lineage edge for the call form.

## Problem code

No new problem code is minted. The deferring rule reuses the existing
**P9999** (`NotImplemented`) via `Diagnostic::not_implemented`. (P9004 is
reserved for the planned general `UnsupportedExtension` framework — see
`specs/design/beckhoff-twincat-dialect.md`.)

## Testing

- Parser: call form → `InitialValueAssignmentKind::FunctionBlockCall` with
  populated `params` (named + positional); empty parens; bare decl still
  `LateResolvedType` (regression); `:= (member := value)` still parses
  (regression).
- plc2plc: call form round-trips (`FB_Comm ( retries := 3 , THIS )`).
- Deferring rule: call form yields exactly P9999.
- Analyzer end-to-end: a call form referencing an earlier-declared FB yields
  P9999 and **not** P2011 (proves toposort/resolution handle the node).
- Toposort: referenced type ordered before the referencing POU.

## Tasks

- [x] Plan (this document)
- [x] DSL node + variant + dispatch
- [x] Parser rule
- [x] Analyzer xform/rule match sites
- [x] Deferring rule (P9999 `NotImplemented`)
- [x] plc2plc + MCP
- [x] Tests
- [x] Full CI (`cd compiler && just`) + PR
