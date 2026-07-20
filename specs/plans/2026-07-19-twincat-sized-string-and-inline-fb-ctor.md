# Plan: `STRING(n)`/`WSTRING(n)` Parenthesis Form + Inline FB-Instance Call-Style Initializer

## Goal

Survey item 1 from `twincat-status.md`'s "Next" list (13 files, ~7
`STRING(n)`/`WSTRING(n)` + ~5-6 inline-constructor-call) bundles two
unrelated syntax gaps. Split them apart and fix both:

1. `STRING(n)`/`WSTRING(n)` — parenthesis-delimited length, instead of the
   only currently-accepted `STRING[n]`/`WSTRING[n]` bracket form — in `VAR`
   declarations and function return types.
2. Inline FB-instance call-style initializer: `name : FB_Type(args);` —
   passing an initialization parameter list directly after the type name,
   instead of the only currently-accepted `name : FB_Type := (member :=
   value, ...);` named-struct-init form.

```
FUNCTION_BLOCK FB_Example
VAR
    hostName : STRING(255);                      // currently a parse error -- only STRING[255] parses
    comm     : FB_Comm(retries := 3, THIS);       // currently a parse error -- only FB_Comm := (...) parses
END_VAR
END_FUNCTION_BLOCK
```

## Verification against real files

Checked a private local checkout of a real TwinCAT codebase directly
before designing anything (per the standing "verify before assuming"
habit):

- `STRING(n)`/`WSTRING(n)`: 51 occurrences across 11 files (`STRING(255)`
  x45, `STRING(1)` x5, `STRING(511)` x1), all in `VAR` declarations
  (`hostName : STRING(255);`, some with `:=` string-literal initializers
  too: `FormatString : STRING(255) := '%s';`) and one `FUNCTION` return
  type (`FUNCTION NCError_TO_STRING : STRING(255)`).
- Inline FB-constructor-call: 24 occurrences (`MAIN.TcPOU`,
  `FB_CoverControl.TcPOU`, `FB_PendantControl.TcPOU`,
  `FB_DomeControl.TcPOU`, `TestProject3/MAIN.TcPOU`), using **both** named
  args (`FB_CoverControl(comm := comm)`) **and** positional args
  (`FB_CoverIdleState(THIS)`) — confirming this needs the same
  positional-or-named parameter grammar as an ordinary FB call
  (`param_assignment()`), not the named-only `member := value` shape that
  `structure_initialization()` already provides for the `:=` form.

## Key finding: the parenthesis form for `STRING`/`WSTRING` length already
## has an unconditional (no dialect flag) precedent in this codebase

`compiler/parser/src/parser.rs` already has
`string_type_declaration__parenthesis()` (line 812) sitting right next to
the bracket form `string_type_declaration()` (line 804) — for `TYPE ... :
STRING(n) ...;` alias declarations — with **no dialect gate at all**, both
unconditionally tried in `data_type_declaration()`. The gap is that the
*same* parenthesis form was never added to the other three places
`STRING`/`WSTRING` length appears with the bracket-only form:

| Rule | Used for |
|---|---|
| `single_byte_string_spec()` / `double_byte_string_spec()` (~1139, 1156) | `VAR`-declared string variables |
| `var_spec()` (~1194-1195) | `VAR` declarations going through the generic spec path |
| `function_return_type()` (~1204-1205) | `FUNCTION ... : STRING(n)` return type |

This confirms the design: extend these three sites the same way, as a
**pure grammar addition with no new dialect flag**, matching the existing
`string_type_declaration__parenthesis()` precedent exactly (parens are
just an alternate delimiter; nothing about the resulting `StringSpecification`/
`StringInitializer` DSL shape depends on which delimiter was used — both
already store only `length: Option<IntegerRef>` with no bracket/paren
marker).

## Design: FB-instance call-style initializer

### Why this also needs no new dialect flag

Following the qualified-method-call precedent (previous branch): a
construct needs a dialect gate only when it introduces a new keyword to
demote/promote at the lexer level (`EXTENDS`, pragmas, `PI`). This
construct introduces no new keyword — `(` is already a token used
everywhere. Like the qualified-call fix, `P9004`/flag-gating isn't the
right shape here either, for a different reason (see below): codegen
**already silently ignores** FB instance initializer values for the
existing, standard `:=  (member := value, ...)` form (confirmed by reading
`compile_setup.rs:122-166` — `fb_init.init` is parsed and stored on the AST
but never read by `compile_setup`, only `fb_init.type_name` is used to
determine the instance's memory layout). Flagging *only* the new
call-style form as "recognized but unsupported" would be inconsistent:
the old form is equally not wired into codegen today, and isn't flagged.
So: parse both forms the same permissive way, store the call-style
argument list on the AST, and leave codegen's behavior exactly as it
already is (ignores initializer values for FB instances either way) —
this is a pure parser fix unblocking files that today fail to parse at
all, not a new "vendor extension needs a stop-gap diagnostic" situation.

### DSL: new optional field on `FunctionBlockInitialValueAssignment`

```rust
// compiler/dsl/src/common.rs
pub struct FunctionBlockInitialValueAssignment {
    pub type_name: TypeName,
    pub init: Vec<StructureElementInit>,
    /// Present for the CODESYS/TwinCAT call-style initializer
    /// (`name : FB_Type(args)`, no `:=`) -- `args` uses the same
    /// positional-or-named shape as an ordinary FB call. `None` for the
    /// standard `:= (member := value, ...)` form (or no initializer at
    /// all). Mutually exclusive with `init` being non-empty.
    pub call_params: Option<Vec<ParamAssignmentKind>>,
}
```

5 construction sites need the new field added (`init: vec![], call_params:
None` for all but the new grammar path):
`compiler/dsl/src/common.rs` (1), `compiler/parser/src/parser.rs` (2:
`fb_name_decl()`, `global_var_decl()`),
`compiler/analyzer/src/xform_resolve_late_bound_type_initializer.rs` (3).
One more site (`compiler/mcp/src/tools/pou_lineage.rs:275`) pattern-matches
with `..` and needs no change.

### Grammar: new alternative in `fb_name_decl()`

```
rule fb_name_decl() -> Vec<UntypedVarDecl> =
  names:fb_name_list() _ tok(Colon) _ type_name:function_block_type_name() _
  init:fb_instance_init()? { ... }

rule fb_instance_init() -> FbInstanceInit =
  tok(Assignment) _ init:structure_initialization() { FbInstanceInit::Structure(init) }
  / params:fb_call_style_init_params() { FbInstanceInit::CallParams(params) }

rule fb_call_style_init_params() -> Vec<ParamAssignmentKind> =
  tok(LeftParen) _ params:param_assignment() ** (_ tok(Comma) _) _ tok(RightParen) { params }
```

No PEG ordering hazard: both alternatives inside `fb_instance_init()`
require a mandatory leading token (`:=` or `(`) with no partial-match
shortcut, so when neither is present the optional `init:fb_instance_init()?`
cleanly yields `None` (same safety check already applied to the qualified-
method-call grammar change). `fb_name_decl()` is tried before the
catch-all `var1_init_decl__with_ambiguous_struct()` in `var_init_decl()`'s
ordered choice, so a fully-matched `Type(args)` is never reached by the
fallback path.

Also confirmed no interaction with `structured_var_init_decl__without_ambiguous()`
(tried earlier in `var_init_decl()`'s ordered choice): its
`initialized_structure__without_ambiguous()` requires a mandatory `:=`
immediately after the type name, so it fails outright (not a partial
match) on `Type(args)` and correctly falls through to `fb_name_decl()`.

## Non-goals

- Any codegen change — `call_params` is stored on the AST and otherwise
  unused, matching the pre-existing behavior of `init` today. Actually
  initializing FB instance fields/inputs from either form's values is a
  separate, larger feature (would need real dataflow into `compile_setup`),
  not attempted here.
- Sized-string length validation/enforcement, or unifying the bracket and
  parenthesis grammar productions into one shared rule (would touch more
  call sites than necessary for this fix; kept as straightforward parallel
  alternatives matching the existing `string_type_declaration()` /
  `string_type_declaration__parenthesis()` precedent).
- Array-element-type STRING/WSTRING length (`ARRAY[1..10] OF STRING(n)`,
  parser.rs ~650-651) — not observed in the survey or cross-repo check;
  left as bracket-only unless a real file needs it.
- A dialect flag for either sub-feature — see rationale above for both.

## File Map

| File | Change |
|------|--------|
| `compiler/parser/src/parser.rs` | Add parenthesis alternative to `single_byte_string_spec()`, `double_byte_string_spec()`, `var_spec()`, `function_return_type()`; new `fb_instance_init()`/`fb_call_style_init_params()` rules used by `fb_name_decl()` |
| `compiler/dsl/src/common.rs` | `FunctionBlockInitialValueAssignment.call_params: Option<Vec<ParamAssignmentKind>>` |
| `compiler/analyzer/src/xform_resolve_late_bound_type_initializer.rs` | Update 3 construction sites |
| Docs | No new `--allow-x` flag; no doc change needed (matches the qualified-call precedent of not documenting flag-less parser permissiveness beyond the plan itself) |

## Testing Strategy

- Parser tests: `STRING(255)`/`WSTRING(100)` in a `VAR` declaration parses
  and matches the equivalent bracket form's AST shape (same
  `StringSpecification`/`StringInitializer`, `length` populated
  identically); `FUNCTION ... : STRING(255)` return type parses.
  Regression: bracket form still parses unchanged.
- Parser tests: `name : FB_Type(a, b := c);` parses with `call_params`
  populated (mixed positional/named, matching real test-corpus usage);
  `name : FB_Type := (member := value);` still parses unchanged
  (regression, `init` populated, `call_params: None`); `name : FB_Type;`
  (no initializer at all) still parses unchanged (regression, both empty/
  `None`).
- No semantic-rule or plc2plc-renderer test needed for `call_params`
  itself beyond round-tripping through the parser, since nothing downstream
  reads it yet (matches the existing, pre-existing `init` field's own
  status quo) — but add a plc2plc round-trip test to confirm the renderer
  doesn't silently drop the call-style form (check `visit_var_decl`/
  wherever `FunctionBlockInitialValueAssignment` is rendered today, and
  update if needed).

## Tasks

- [x] Write plan (this document)
- [x] Grammar: `STRING(n)`/`WSTRING(n)` parenthesis form (3 sites, via a
      shared `string_length_spec()` rule)
- [x] Grammar + DSL: FB-instance call-style initializer (design changed
      during implementation — see Implementation Notes: `fb_name_decl()`
      turned out to be dead code, so a new dedicated rule was added instead
      of extending it)
- [x] Update the 3 `xform_resolve_late_bound_type_initializer.rs`
      construction sites
- [x] Check plc2plc renderer for both changes; fix/extend if needed (added
      `call_params` rendering; confirmed `STRING(n)`'s renderer already
      normalizes to brackets unconditionally, no change needed)
- [x] Tests from Testing Strategy, plus two regression tests not in the
      original plan (see Implementation Notes: `commasep_oneplus` bug,
      toposort edge-direction bug)
- [x] Run full CI pipeline (`cd compiler && just`)
- [ ] Push branch to fork (no PR against `ironplc/ironplc` without explicit
      go-ahead, per standing instruction)
- [ ] Merge into `twincat-dev`, update `twincat-status.md`, push

## Implementation Notes

- **`fb_name_decl()` — the rule the plan intended to extend — turned out
  to be pre-existing dead code**, found only by testing the simplest
  possible case (`comm : FB_Comm;`, no init at all) with `peg`'s
  `trace`/`debug` cargo feature (`cargo build -p ironplc-cli --features
  trace`, which prints every rule attempt to stderr). Root cause:
  `fb_name_list()` uses a shared combinator, `commasep_oneplus()`
  (`compiler/parser/src/parser.rs` line 290),  defined as
  `x() ++ (_ comma() _) comma()` — note the **extra trailing `comma()`**
  after the already-complete comma-separated list. This requires a
  spurious trailing comma after every name list, so `fb_name_decl()` (and
  the only other caller, `prog_conf_elements()`) has never successfully
  matched any real input. Every FB instance declaration in the existing
  test suite and real files was actually being handled by the
  `var1_init_decl__with_ambiguous_struct()` fallback instead (bare
  declarations resolve to `LateResolvedType`, deferred to
  `xform_resolve_late_bound_type_initializer.rs`; declarations with a
  `:=` initializer are — somewhat incorrectly, but out of scope here —
  parsed as `InitialValueAssignmentKind::Structure` via
  `structured_var_init_decl__without_ambiguous()`, not `FunctionBlock`).
  **Did not fix `commasep_oneplus()` itself** — `fb_name_decl()` being
  unreachable is apparently load-bearing: since it eagerly commits a bare
  type name to `FunctionBlock` with no way to check whether the type
  actually *is* a function block (that's precisely why the
  `LateResolvedType`-then-resolve deferral pattern exists), fixing the
  shared combinator would have made `fb_name_decl()` newly reachable and
  changed the parsed shape for every bare FB-shaped declaration in the
  codebase from `LateResolvedType` to an eagerly-tagged `FunctionBlock` —
  a much larger, riskier, and out-of-scope change. Instead, added a new,
  narrowly-scoped rule, `fb_call_style_var_decl()`, using the *working*
  `var1_list()` combinator and requiring the call-style parens
  unconditionally (not optional) — the parens make this syntax
  unambiguous (no other declaration shape can be followed directly by
  `(`), so it never needs to defer to late-bound resolution the way a
  bare type name does, and it cannot accidentally interfere with the bare
  or `:=`-initializer cases either.
- **A second, unrelated pre-existing bug surfaced downstream, once the
  grammar fix made it possible to construct an eager `FunctionBlock`
  initializer for a real, user-declared (not stdlib) FB type for the
  first time**: `xform_toposort_declarations.rs`'s dependency-graph edges
  for `InitialValueAssignmentKind::FunctionBlock` (both the dedicated
  `visit_function_block_initial_value_assignment` and the inline arm in
  `visit_initial_value_assignment_kind`) pointed the wrong direction
  compared to the `Structure`/`LateResolvedType` arms and
  `visit_interface_declaration`'s `EXTENDS` edge (confirmed by reading
  all three side by side) — referenced-type-before-containing-POU is the
  correct convention, but the `FunctionBlock` arms had it backwards. This
  went unnoticed because eager `FunctionBlock` construction was
  previously only reachable via `global_var_decl()` (`VAR_GLOBAL`) and
  the `TYPE`-alias path, and no existing test exercised a `VAR_GLOBAL`
  or type-alias forward-reference to a POU-local, later-processed FB
  type. Confirmed via a real repro (`FB_Comm` declared first, `FB_Example`
  referencing it via the new call-style syntax second — the natural
  order) that this produced a spurious `P2011 Parent type is not
  declared`. Fixed by flipping both edges to match the established
  convention; verified with the full workspace test suite (no
  regressions) plus two new targeted regression tests (a toposort-level
  test in `xform_toposort_declarations.rs` and an end-to-end
  `resolve_types`-level test in `stages.rs`).
- **`STRING(n)`/`WSTRING(n)` needed no DSL or renderer changes beyond the
  grammar itself** — `StringSpecification`/`StringInitializer` already
  only store `length: Option<IntegerRef>` with no bracket/paren marker
  (confirmed by reading the DSL before implementing), and the renderer
  (`visit_string_initializer`) already unconditionally normalizes to the
  bracket form. This means a `STRING(255)` input round-trips to
  `STRING [ 255 ]` output (not byte-identical), which is expected
  normalization, not a bug — the plc2plc test asserts on the normalized
  bracket form rather than the original parenthesis spelling.
- **Renderer spacing**: `write_ws` inserts a space before tokens like `(`
  and `,` unless the buffer already ends in one, so the call-style
  initializer renders as `FB_Comm ( retries := 3 , THIS )` (spaced),
  matching this renderer's existing general convention (confirmed by
  checking `visit_fb_call`'s own param rendering, which has the same
  spacing) — not an inconsistency specific to this change.
