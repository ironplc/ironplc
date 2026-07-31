# Plan: Inline FB-Instance Call-Style Initializer (`name : FB_Type(args);`)

## Goal

Add support for the CODESYS/TwinCAT call-style function-block instance
initializer: passing an initialization parameter list directly after the
type name, instead of the only currently-accepted `name : FB_Type :=
(member := value, ...);` named-struct-init form.

```
FUNCTION_BLOCK FB_Example
VAR
    comm : FB_Comm(retries := 3, THIS);   // currently a parse error -- only FB_Comm := (...) parses
END_VAR
END_FUNCTION_BLOCK
```

This is split out of #1222 (which bundled two unrelated syntax gaps) and is
part of the TwinCAT dialect work tracked in #1199. The
`STRING(n)`/`WSTRING(n)` parenthesis-length half of #1222 is a separate PR
(Agent 1). The toposort edge-direction fix that this feature depends on is
also a separate, already-merged PR (#1269, Agent 2).

## Verification against real files

Per the "verify before assuming" habit, #1222 checked a private local
checkout of a real TwinCAT codebase before designing anything:

- Inline FB-constructor-call: 24 occurrences across several `.TcPOU`
  files, using **both** named args (`FB_Comm(comm := comm)`) **and**
  positional args (`FB_IdleState(THIS)`) — confirming this needs the same
  positional-or-named parameter grammar as an ordinary FB call
  (`param_assignment()`), not the named-only `member := value` shape that
  `structure_initialization()` already provides for the `:=` form.

## Dependency: toposort edge-direction fix (#1269, merged)

Constructing an eager `InitialValueAssignmentKind::FunctionBlock`
initializer for a real, user-declared FB type — which the call-style
grammar does at parse time — surfaced a pre-existing bug in
`xform_toposort_declarations.rs`: the `FunctionBlock` dependency-graph
edges pointed the opposite direction to the `Structure`/`LateResolvedType`
arms, ordering a referenced type *after* its referencing POU and producing
a spurious `P2011` "Parent type is not declared". That fix (flipping both
`FunctionBlock` edges to `add_edge(referenced, referencer)`) landed
separately in #1269 and must be merged before this branch, or this
branch's end-to-end tests fail. This PR does **not** re-include the edge
flips; it does include the call-style-syntax tests that exercise the fix.

## Design: why this needs no new dialect flag

Following the qualified-method-call precedent: a construct needs a dialect
gate only when it introduces a new keyword to demote/promote at the lexer
level (`EXTENDS`, pragmas, `PI`). This construct introduces no new keyword
— `(` is already a token used everywhere. Flag-gating isn't the right shape
here either: codegen **already silently ignores** FB instance initializer
values for the existing, standard `:= (member := value, ...)` form
(`fb_init.init` is parsed and stored on the AST but never read by
`compile_setup`; only `fb_init.type_name` is used to determine the
instance's memory layout). Flagging *only* the new call-style form as
"recognized but unsupported" would be inconsistent: the old form is equally
not wired into codegen today, and isn't flagged. So: parse both forms the
same permissive way, store the call-style argument list on the AST, and
leave codegen's behavior exactly as it already is — this is a pure parser
fix unblocking files that today fail to parse at all.

## DSL: new optional field on `FunctionBlockInitialValueAssignment`

```rust
// compiler/dsl/src/common.rs
pub struct FunctionBlockInitialValueAssignment {
    pub type_name: TypeName,
    pub init: Vec<StructureElementInit>,
    /// Present for the CODESYS/TwinCAT call-style instance initializer
    /// (`name : FB_Type(args);`, no `:=`) -- `args` uses the same
    /// positional-or-named shape as an ordinary FB call. `None` for the
    /// standard `:= (member := value, ...)` form or no initializer at all.
    /// Mutually exclusive with `init` being non-empty. Not yet wired into
    /// codegen.
    #[recurse(ignore)]
    pub call_params: Option<Vec<ParamAssignmentKind>>,
}
```

`#[recurse(ignore)]` matches the existing treatment of non-recursable
auxiliary fields on the DSL structs. Construction sites that need the new
field added (`call_params: None` for all but the new grammar path):
`compiler/dsl/src/common.rs` (1: `VarDecl::function_block`),
`compiler/parser/src/parser.rs` (2: `fb_name_decl()`, `global_var_decl()`),
`compiler/analyzer/src/xform_resolve_late_bound_type_initializer.rs` (3).
`compiler/mcp/src/tools/pou_lineage.rs` pattern-matches with `..` and needs
no change.

## Grammar: new dedicated rule in `var_init_decl()`

`fb_name_decl()` — the rule that *looks* like it should host this — is
pre-existing dead code: `fb_name_list()` uses `commasep_oneplus()`, which
requires a spurious trailing comma, so `fb_name_decl()` has never matched
real input (every FB instance declaration is actually handled by the
`var1_init_decl__with_ambiguous_struct()` fallback). Fixing
`commasep_oneplus()` is deliberately out of scope: `fb_name_decl()` being
unreachable is load-bearing (it eagerly commits a bare type name to
`FunctionBlock` with no way to check the type actually *is* a function
block — precisely why the `LateResolvedType`-then-resolve deferral exists).

Instead, add a new, narrowly-scoped rule using the *working* `var1_list()`
combinator, requiring the call-style parens **unconditionally** (not
optional):

```
rule fb_call_style_var_decl() -> Vec<UntypedVarDecl> =
  names:var1_list() _ tok(Colon) _ type_name:function_block_type_name() _
  params:fb_call_style_init_params() { ... call_params: Some(params) ... }

rule fb_call_style_init_params() -> Vec<ParamAssignmentKind> =
  tok(LeftParen) _ params:param_assignment() ** (_ tok(Comma) _) _ tok(RightParen) { params }
```

Added to `var_init_decl()`'s ordered choice immediately after
`fb_name_decl()`. The mandatory parens make this syntax unambiguous (no
other declaration shape can be followed directly by `(`), so it never needs
to defer to late-bound resolution the way a bare type name does. No PEG
ordering hazard: every earlier alternative in `var_init_decl()` requires
its own mandatory leading token (`ARRAY`, `REF_TO`, `STRING`/`WSTRING`, or
a literal `:=`) and fails outright (not a partial match) on a bare type
name followed by `(`.

## Renderer

`plc2plc`'s `visit_function_block_initial_value_assignment` renders the
call params in parentheses when `call_params` is `Some`, using
`visit_comma_separated!` (same as the ordinary FB-call param rendering).
`write_ws` inserts spaces, so `FB_Comm(retries := 3, THIS)` round-trips to
`FB_Comm ( retries := 3 , THIS )` (spaced) — normalization, not a bug,
matching the renderer's existing convention.

## Non-goals

- Any codegen change — `call_params` is stored on the AST and otherwise
  unused, matching the pre-existing behavior of `init` today.
- Fixing `commasep_oneplus()` / making `fb_name_decl()` reachable — see
  above; deliberately out of scope.
- The `STRING(n)`/`WSTRING(n)` parenthesis-length grammar (Agent 1's PR).
- The toposort edge-direction fix itself (Agent 2's PR #1269, merged).

## File Map

| File | Change |
|------|--------|
| `compiler/dsl/src/common.rs` | `FunctionBlockInitialValueAssignment.call_params` field + `VarDecl::function_block` site |
| `compiler/parser/src/parser.rs` | New `fb_call_style_var_decl()`/`fb_call_style_init_params()` rules; add to `var_init_decl()`; `call_params: None` in `fb_name_decl()` and `global_var_decl()` |
| `compiler/analyzer/src/xform_resolve_late_bound_type_initializer.rs` | `call_params: None` at 3 construction sites |
| `compiler/plc2plc/src/renderer.rs` | Render call params in parentheses when `Some` |

## Testing Strategy

- Parser (`compiler/parser/src/tests/var_declarations.rs`): mixed
  named+positional call params populate `call_params`; empty parens →
  `Some(vec![])`; bare declaration (no init) still resolves to
  `LateResolvedType` (`call_params` path not taken); `:= (member := value)`
  struct-init form still parses (regression).
- plc2plc (`compiler/plc2plc/src/tests/declarations.rs`): call-style
  initializer round-trips through parse → render → parse.
- Analyzer (`compiler/analyzer/src/stages.rs`): end-to-end confirmation
  that a call-style initializer referencing an earlier-declared FB produces
  no diagnostics (the P2011 regression, green with #1269 merged).
- Toposort (`compiler/analyzer/src/xform_toposort_declarations.rs`): a
  call-style-grammar-driven test asserting the referenced type is ordered
  first (test only — the production edge flip is #1269's).

## Tasks

- [x] Write plan (this document)
- [x] DSL: add `call_params` field + update `VarDecl` construction site
- [x] Parser: new rules + `var_init_decl()` alternative + `call_params: None`
      at existing sites
- [x] Analyzer: `call_params: None` at 3 `xform_resolve_late_bound` sites
- [x] Renderer: render call params
- [x] Tests (parser, plc2plc, analyzer stages, toposort)
- [x] Run full CI pipeline (`cd compiler && just`)
- [ ] Open PR against `ironplc/ironplc:main`
