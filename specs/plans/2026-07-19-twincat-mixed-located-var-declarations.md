# Plan: Mixed Located/Plain Variable Declarations in One VAR Block

## Goal

Allow a single `VAR`/`VAR_INPUT`/`VAR_OUTPUT` block to mix ordinary
(symbolic) variable declarations with `AT`-located declarations
(complete addresses like `AT %IX0.0` or incomplete/wildcard addresses
like `AT %I*`), instead of requiring located variables to live in their
own dedicated block:

```
VAR
    tempSensorM1   AT%I*: INT;   // currently requires its own VAR block
    fbComm         : I_Comm;     // currently requires its own VAR block
END_VAR
```

This is real, common TwinCAT usage — not a new address syntax (`AT %I*`
already parses fine on its own), but a structural relaxation of which
block the same already-supported declaration forms can appear in.

## Verified against real project files

Checked `/home/husser/code/brotlib` (same TwinCAT codebase used for prior
plans), scripted rather than eyeballed, since the previous session's
lesson ("survey counts aren't cost estimates") applies here too:

- `AT %I*`/`AT %Q*` (bare wildcard, no size prefix) appear **243 times**
  across the codebase — 233 inside plain `VAR` blocks, 4 inside
  `VAR_INPUT`, 3 inside `VAR_OUTPUT`, 3 inside `VAR_GLOBAL`. No sized
  wildcards (`%IB*`, `%IW*`, etc.) and no complete addresses (`AT %IX0.0`)
  found anywhere — real usage is exclusively the bare wildcard form.
- **28 files** have at least one `VAR`/`VAR_INPUT`/`VAR_OUTPUT` block that
  mixes a located declaration with a plain one in the same block —
  confirmed via a script that tracks the currently-open block and flags
  when both kinds appear before the matching `END_VAR`.
- Directly reproduced the failure against `ironplcc check --dialect
  codesys`: a `VAR` block with two `AT%I*` declarations followed by one
  plain declaration fails with `P0002` ("Expected ... 'AT' ... Found
  text ':'") at the plain declaration — the parser, once it commits to
  the incomplete-located-var grammar path (because earlier declarations
  had `AT`), then requires every subsequent declaration in the same block
  to also have `AT`. Removing the plain declaration (or making the whole
  block all-located) already parses successfully today — confirming the
  *address syntax itself* isn't the gap, only the block-level structure.
- 22 of the 158 surveyed files hit this specific error signature when
  checked directly (more than the original issue's "14 files" estimate
  for "AT %I*/%Q* shorthand" — likely because that count only tallied
  files where this was the *first* reported error, undercounting files
  where it's one of several).

## Why this needs a grammar change, not just a new token

`AT %I*` and `AT %IX0.0` already lex and parse correctly today, in their
own dedicated block types:

- `located_var_declarations()` (`parser.rs`) — complete addresses,
  `VAR ... END_VAR`, but **only reachable from `program_declaration()`**
  (PROGRAM bodies only, confirmed by grepping every call site of the
  rule — it's not used from `function_block_declaration()` at all today,
  a separate small gap noted in Non-goals).
- `incompl_located_var_declarations()` — wildcard addresses, reachable
  from `other_var_declarations()` (used by both PROGRAM and
  FUNCTION_BLOCK/FUNCTION bodies).

Both are **all-or-nothing** block types: every declaration in a
`located_var_declarations()`/`incompl_located_var_declarations()` block
must itself have an `AT` clause (`semisep_or_empty(<located_var_decl()>)`
requires every item to satisfy `located_var_decl()`, which always expects
`AT`). There is no existing per-declaration mechanism inside the ordinary
`var_declarations()`/`input_declarations()`/`output_declarations()` path
(built on `UntypedVarDecl` via `var_init_decl()`) to carry a location.

The DSL itself has no such limitation — `VariableIdentifier::Direct`
already represents a located variable regardless of context, and
`AddressAssignment` already unifies complete and incomplete addresses
(confirmed: `incompl_location()`'s grammar action calls the exact same
`AddressAssignment::try_from` as `location()`, just matching a different
token regex — `SizePrefix::Unspecified`/empty `address` for the wildcard
case). So this is purely a parser-level gap, not an AST/analyzer/codegen
one.

## Design

### Grammar: one new per-declaration rule, tried within `var_init_decl()`

```
rule located_var1_init_decl() -> Vec<UntypedVarDecl> =
  name:variable_name() _ loc:(location() / incompl_location()) _
  tok(TokenType::Colon) _
  init:simple_or_enumerated_or_subrange_ambiguous_struct_spec_init() {
    vec![UntypedVarDecl { name, location: Some(loc), initializer: init }]
  }
```

Added as an alternative in `var_init_decl()` (the rule shared by
`input_declarations()`, `output_declarations()`, and
`var_declarations()`/`retentive_var_declarations()`), tried before the
existing alternatives. No ambiguity risk: `AT` is a keyword unconditionally
recognized already, and none of the other `var_init_decl()` alternatives
can match input starting with `name AT ...` (they all expect `,` or `:`
immediately after the name(s)), so PEG's ordered choice needs no special
handling here — this isn't the same "greedy partial match" hazard as the
`constant()`/`expression()` case in the previous plan, since `AT` is not
itself parseable as a continuation of any other alternative.

Deliberately singular (one name per declaration, not `var1_list()`'s
comma-separated form) — real usage always declares one located variable
per line (matches the existing `located_var_decl()`/
`incompl_located_var_decl()` shape, which are also singular), and a
shared address across multiple names wouldn't make sense anyway.

### `UntypedVarDecl` gets an optional location

```rust
// compiler/parser/src/vars.rs
pub struct UntypedVarDecl {
    pub name: Id,
    pub location: Option<AddressAssignment>,  // new
    pub initializer: InitialValueAssignmentKind,
}

impl UntypedVarDecl {
    pub fn into_var_decl(self, var_type: VariableType) -> VarDecl {
        let identifier = match self.location {
            Some(loc) => VariableIdentifier::Direct(DirectVariableIdentifier {
                name: Some(self.name),
                address_assignment: loc,
                span: SourceSpan::default(),
                in_mixed_var_block: true,  // new field, see below
            }),
            None => VariableIdentifier::Symbol(self.name),
        };
        VarDecl { identifier, var_type, qualifier: DeclarationQualifier::Unspecified, initializer: self.initializer }
    }
}
```

The other ~10 call sites that construct `UntypedVarDecl` (structured,
array, ref_to, string, fb_name, etc.) get `location: None` — mechanical,
they don't have a location to carry (none of the real files show a
located struct/array/ref_to/string declaration, and the existing
`located_var_spec_init()` used by the dedicated located-var blocks only
supports `Array`/`Simple` anyway, so this isn't a capability regression).

### Flag-gating: a marker field, not a grammar-level check

Decision (see conversation): gate this behind a new
`--allow-mixed-located-var-declarations` flag, consistent with every
other vendor extension in this codebase, even though it costs a bit more
than the alternative (no flag at all).

**The hard part**: the AST shape produced by the new grammar path
(`VariableIdentifier::Direct` with `var_type: VariableType::Var`) is
**byte-identical** to what the *already-unconditional, un-flagged*
`located_var_decl()` (`var_type: VariableType::Var` too — confirmed by
reading its construction) and `incompl_located_var_decl()` already
produce today in their own dedicated blocks. There is no existing way to
tell "came from a mixed block" from "came from its own dedicated block"
after parsing — and by the time a `FunctionBlockDeclaration`/
`ProgramDeclaration` is built, all three sources are flattened into one
`variables: Vec<VarDecl>` list anyway, losing any rule-level provenance.

Fix: add `in_mixed_var_block: bool` to `DirectVariableIdentifier`
(`#[recurse(ignore)]`, like `is_neg`/`op` elsewhere), defaulting `false`
at the two existing construction sites (`VariableIdentifier::new_direct`,
used by `located_var_decl()`; the `IncomplVarDecl → VarDecl` conversion,
used by `incompl_located_var_decl()`) and `true` only at the new
`into_var_decl()` path. Small blast radius — only 2 existing construction
sites plus the 1 new one.

### Semantic rule: `rule_mixed_located_var_declarations.rs`

New rule, visits every `VarDecl`, checks
`VariableIdentifier::Direct { in_mixed_var_block: true, .. }`; if
`!options.allow_mixed_located_var_declarations`, emits a new problem code.
Mirrors the existing `allow_top_level_var_global` pattern (grammar always
accepts, `xform_toposort_declarations.rs` semantically gates and emits
P4028) — same shape, new rule file since this isn't about declaration
ordering.

### Dialect flag: `allow_mixed_located_var_declarations`

`[Rusty, Codesys]`, same placement as every other vendor-extension flag
in this session's work.

## Non-goals

- `located_var_declarations()` (complete addresses) is currently only
  reachable from `program_declaration()`, not
  `function_block_declaration()`/`function_declaration()` — a real,
  separate gap (found while reading the grammar), but **no evidence from
  the real files it's needed**: zero complete-address (`AT %IX0.0`) usage
  found anywhere in the survey, only wildcards. Not fixing it here; flag
  for a future pass if a real file ever needs it.
- Sized wildcards (`%IB*`, `%IW*`, `%ID*`, `%IL*`) — not found in any real
  file, `DirectAddressIncomplete`'s token regex (`%[IQM]\*`) doesn't
  support them today. No evidence this is needed.
- `VAR_GLOBAL` located declarations — 3 occurrences in the survey, but
  `global_var_decl()`'s existing handling of locations is already broken
  independent of this change (confirmed: `global_var_spec()`'s
  location-bearing alternative literally discards the parsed
  `AddressAssignment` and returns a garbage placeholder name — marked
  with pre-existing `TODO: this is clearly wrong` comments). Fixing that
  is a separate, unrelated bug, out of scope here.
- `VAR_IN_OUT` — no evidence of `AT`-located in-out variables in the
  survey; IEC 61131-3 doesn't permit located in-out parameters anyway.

## File Map

| File | Change |
|------|--------|
| `compiler/dsl/src/common.rs` | New `in_mixed_var_block: bool` field on `DirectVariableIdentifier` |
| `compiler/parser/src/vars.rs` | `UntypedVarDecl.location: Option<AddressAssignment>`; update `into_var_decl` |
| `compiler/parser/src/parser.rs` | New `located_var1_init_decl()` rule wired into `var_init_decl()`; `location: None` at ~10 other `UntypedVarDecl` construction sites |
| `compiler/analyzer/src/rule_mixed_located_var_declarations.rs` (new) | Semantic gate, mirrors `xform_toposort_declarations.rs`'s `allow_top_level_var_global` check |
| `compiler/analyzer/src/stages.rs` | Wire the new rule into `semantic()` |
| `compiler/parser/src/options.rs` | New `allow_mixed_located_var_declarations` flag |
| `compiler/problems/resources/problem-codes.csv` + new doc page | New problem code |
| `compiler/ironplc-cli/src/lsp.rs` | LSP flag wiring |
| `compiler/plc2plc/src/renderer.rs` | Confirm/verify rendering of a `Direct` identifier inside a plain `VAR` block round-trips (renderer already handles `VariableIdentifier::Direct` generically for the dedicated-block case; verify the mixed case renders back into the same block, not a separate one) |
| Docs (3 files, same as every prior PR) | Document the new flag |

## Testing Strategy

- Parser tests: mixed block parses (located-then-plain, plain-then-located,
  interleaved); pure located-only and pure plain-only blocks unaffected
  (regression); wildcard and complete addresses both work in the mixed
  position; `VAR_INPUT`/`VAR_OUTPUT` mixed blocks parse too.
- Semantic tests: mixed block passes with the flag on, diagnoses with it
  off; pure dedicated located/incompl-located blocks are *never* flagged
  regardless of the option (regression proving the marker distinguishes
  correctly).
- plc2plc round-trip test for a mixed block.
- End-to-end execution test: a mixed block compiles and the plain
  variable's value is readable/writable normally (the located variable's
  own runtime behavior is unchanged from the existing, already-tested
  dedicated-block case — codegen doesn't special-case
  `VariableIdentifier::Direct` at all, confirmed by grep, so no new
  codegen test is needed for the located variable itself, only for
  proving the *plain* sibling in the same block still works).

## Tasks

- [x] Write plan (this document)
- [ ] `in_mixed_var_block` field on `DirectVariableIdentifier`
- [ ] `UntypedVarDecl.location` + `into_var_decl` update + ~10 call-site updates
- [ ] `located_var1_init_decl()` grammar rule wired into `var_init_decl()`
- [ ] New `allow_mixed_located_var_declarations` flag
- [ ] New `rule_mixed_located_var_declarations.rs` semantic rule + problem code
- [ ] Wire into `stages.rs`, `lsp.rs`
- [ ] All tests from Testing Strategy
- [ ] Update docs
- [ ] Run full CI pipeline (`cd compiler && just`)
- [ ] Push branch to fork (no PR against `ironplc/ironplc` without explicit
      go-ahead, per standing instruction)
