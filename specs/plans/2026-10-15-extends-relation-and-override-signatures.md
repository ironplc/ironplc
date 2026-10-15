# Plan: One `EXTENDS` Relation, and the Checks It Makes Possible

Checked against `main` at `8250ff50`; line numbers are as of that commit.
Every reproduction below was rerun there.

Fixes [#2144](https://github.com/ironplc/ironplc/issues/2144) (an `EXTENDS` of
a function block that does not exist is not reported) and
[#2145](https://github.com/ironplc/ironplc/issues/2145) (a method override with
a different return type from its base is not reported).

## Goal

The two issues have one cause: `EXTENDS` has no representation in the
analyzer's environments. The source's `EXTENDS base` is a `TypeName` on the AST
(`dsl/src/oop.rs:204`), and every consumer resolves that name on its own, with
its own idea of what a name that resolves to nothing means. Nothing decides,
once, "this block's base is that block", so nothing is in a position to say
when it isn't (#2144). And no environment holds what a method of a block looks
like in terms of types, so nothing can compare an override with the method it
overrides (#2145).

After this work:

- **The type environment records the relation**: each function block's base,
  as a type id, entered only when the base names a function block type. Every
  analysis that follows the chain follows that relation.
- **The type environment records each method's signature**: its result type
  and its `VAR_INPUT`/`VAR_OUTPUT`/`VAR_IN_OUT` parameters, as type ids.
- **An `EXTENDS` that names no function block is reported** (new problem code),
  at the base name.
- **A method whose signature differs from the method it overrides is
  reported** (new problem code), at the overriding method.

## Current behaviour

### Who resolves the base name

| Site | Resolves `EXTENDS base` by | A name that is not a declared block |
|---|---|---|
| `xform_toposort_declarations.rs:441` | adding a graph edge from the name | adds a node with no declaration |
| `xform_resolve_type_decl_environment.rs:369` | does not; the block's type has no base | — |
| `xform_resolve_symbol_and_function_environment.rs:339` | copying the name into `SymbolInfo::extends` (`symbol_environment.rs:172`) | becomes a scope with no symbols on the lookup chain (`:659`), and `SUPER^`'s type (`:631`) |
| `callee_resolution.rs:64` `FunctionBlocks::chain` | looking the name up among the library's declarations | ends the chain |
| `intermediates/inherited_fields.rs:45,75` | the same, with its own map and memo | "silently contributes no inherited fields" (`:28`) |

`FunctionBlocks::chain` serves `resolve_method`, `declaring_block`,
`MethodOwners` (which code generation also builds, `codegen/src/compile.rs:299`)
and `BlockParameters`. `collect_inherited_fields` serves
`rule_extends_field_duplicated` and `xform_resolve_expr_types`.

### Who knows a method's types

`xform_resolve_expr_types.rs:72-123` (`MethodTypes`, `collect_method_types`)
builds a private table of each method's return type name and output type ids,
by walking `FunctionBlocks::resolve_method`, to type method calls. Nothing else
records a method's types; the open
[#1901](https://github.com/ironplc/ironplc/pull/1901) builds another private
comparison (`TypeKey`, `LengthKey`) to check interface conformance.

### Reproductions

`ironplcc check --dialect iec61131-3-ed3` and `--dialect twincat` give the same
result for every row.

| # | Program | Today | After |
|---|---|---|---|
| 1 | `FB_A EXTENDS FB_Missing` | accepted | P-base (core 1) |
| 2 | `FB_A EXTENDS Pair`, `Pair` a structure | accepted | P-base (core 1) |
| 3 | `FB_A EXTENDS TON` | accepted | accepted (see [Decisions](#decisions-for-review)) |
| 4 | `FB_Root.Describe : INT`, `FB_Mid EXTENDS FB_Root`, `FB_Mid.Describe : BOOL` | accepted | P-override (core 2) |
| 5 | Override renames `VAR_INPUT a : INT` to `b` | accepted | P-override |
| 6 | Override declares `VAR_IN_OUT a : INT` where the base has `VAR_INPUT a : INT` | accepted | P-override |
| 7 | Override adds a local `VAR tmp : INT` | accepted | accepted |
| 8 | Both return `STRING[10]` | accepted | accepted |
| 9 | `STRING[10]` overridden by `STRING[20]` | accepted | P-override |
| 10 | `FB_Leaf EXTENDS FB_Mid EXTENDS FB_Root`; `FB_Root.Describe : INT`, `FB_Leaf.Describe : BOOL` | accepted | P-override, against `FB_Root.Describe` |
| 11 | Both declare `VAR_IN_OUT a : ARRAY[1..3] OF INT` | accepted | accepted |

P-base and P-override are the next two free codes when the core change PRs
open: P4084 and P4085 on `main` today (P4066–P4068 are claimed by #1901).

### What XAE does

- Row 1: `No definition found for base class 'FB_Missing'` (#2144).
- Row 4: `Interface of overridden method 'DESCRIBE' of base 'FB_ROOT' doesn't
  match declaration` (#2145).
- #1901 probed the same message for an interface method on TwinCAT
  3.1.4024.66: a renamed, retyped, added, reordered or re-kinded parameter, or a
  different return type, is rejected; an extra local `VAR` is accepted. The
  override case was probed only for the return type.

IEC 61131-3 Ed. 3 defines a method's signature as its name, its result type,
and the names, types and order of its parameters, and requires an overriding
method to have the signature of the one it overrides.

## Architecture

### Where the relation lives

In the **type environment**, as side tables beside the ones it already keeps
per type (`alias_bases`, `fields`, `type_environment.rs:289-295`):

```rust
/// The function block each function block `EXTENDS`, by the id of the
/// derived block. Entered only when the base names a function block type,
/// so an `EXTENDS` that names anything else gives the block no base.
bases: HashMap<TypeId, TypeId>,
/// The signature of each method a function block declares, by the id of
/// the block and the method's name. An inherited method is not entered
/// under the block that inherits it.
methods: HashMap<(TypeId, Id), MethodSignature>,
```

- `base_of(block) -> Option<TypeId>` and `inheritance_chain(block)`: the block,
  its base, that block's base, and so on, stopping at a block seen before (a
  cycle is rejected by the toposort; this is a safety net, as in every walk
  today).
- `method(block, name) -> Option<(TypeId, &MethodSignature)>`: the method the
  nearest block in the chain declares, and that block. This is the answer
  `FunctionBlocks::resolve_method` gives today, in types rather than
  declarations.
- `same_type(a, b) -> bool` (core 2): the same id, or two types spelled out in
  place whose representations are equal. A sized string and an in-place array,
  enumeration or subrange get an id per declaration (ADR-0055), so two
  `STRING[10]` parameters are the same type with different ids. A `REF_TO` is
  already one id however often it is spelled.

`MethodSignature` (new, `analyzer/src/method_signature.rs`) holds the name, the
result (none, or a type id), the parameters in declaration order (name,
`ParameterSection`, type id) and the `R_EDGE`/`F_EDGE` inputs in declaration
order. A type the analyzer could not resolve is recorded as unknown; a rule
that compares it skips that pair, because the undeclared type is reported
already.

**Not `SemanticType::FunctionBlock`.** That is a value's representation, cloned
into every field of the block's type; a base and a method table are not part
of a value. **Not the symbol environment.** It already copies the name, but it
holds no types, and a signature is types. **Not a new environment.** The type
environment already answers "what does this function block type have" for
fields; a base and methods are the rest of that answer.

### Where it is entered

- **The base**, in `xform_resolve_type_decl_environment::fold_function_block_declaration`,
  when the block's type is entered. The toposort makes a base an element
  before its derived block, so the base's type is already there when it is
  declared. `insert_type` returns whether it entered the type, so a repeated
  block name (already P4013) does not give the first block the second one's
  base.
- **The methods**, in `xform_resolve_decl_types::fold_function_block_declaration`,
  where the block's fields are recorded today: each method's parameters have
  their type ids by then. A sized string result is entered as a sized string
  type, as a declaration of that type is.

### Who reads it

- `FunctionBlocks::chain`, and through it `resolve_method`, `declaring_block`,
  `MethodOwners` and `BlockParameters`, follow `inheritance_chain` instead of
  the AST name. Their constructors take the type environment.
- `collect_inherited_fields` follows `inheritance_chain`.
- `xform_resolve_expr_types` types a method call from `method(block, name)`
  instead of `collect_method_types`, which is deleted.
- The symbol environment records a block's `extends` from `base_of`, so a name
  that is not a block is not a scope on the lookup chain and not `SUPER^`'s
  type (core 1).
- The toposort keeps the AST name: it runs before the type environment exists,
  and decides the order the environment is built in.

### The rules

- **`rule_extends_base_declared` (core 1).** A block whose source says
  `EXTENDS base` but whose type has no `base_of` is reported at the base name,
  with the block as context.
- **`rule_method_override_signature` (core 2).** For each method a block
  declares, the method `method(base, name)` finds is the one it overrides. When
  the two signatures differ, it is reported at the overriding method's name,
  with the overridden method as the secondary label, and the label names the
  first difference ("returns BOOL; FB_Root.Describe returns INT"). Comparing
  with the nearest override, not the root, gives one report per broken link:
  in row 10 with an `FB_Mid.Describe : BOOL` added, only `FB_Mid` is reported.
  An abstract method is compared like any other. Locals are not compared.

Both apply on every dialect, as `rule_extends_field_duplicated` does:
`EXTENDS` is Edition 3 syntax, not a vendor extension.

## Prefactoring

Two prefactors, each behaviour-preserving with existing expectations unchanged:

1. **Resolve `EXTENDS` once, in the type environment.** `bases`, `base_of`,
   `inheritance_chain`; the walks in `FunctionBlocks` and
   `collect_inherited_fields` follow them. Today both walks stop at a name that
   is not a declared block, and so does `inheritance_chain`, so nothing they
   answer changes. Their unit tests build the library with the resolving test
   helpers instead of `parse_program`, so that the environment has the
   relation: the fixtures and assertions stay the same. The symbol
   environment's `extends` does not move here: for a name that is not a block
   it changes what `SUPER^` is, which is core 1's change to make.
2. **Record method signatures in the type environment and type method calls
   from them.** `MethodSignature`, `methods`, `method`; `MethodTypes` and
   `collect_method_types` are deleted. A call's type is the result type's
   *name* resolved again, as today, so a `STRING[10]` result still types its
   call as `STRING` and every expression type is unchanged.

Without them, core 1 would add a sixth resolution of the base name, and core 2
would build a second private method table beside `collect_method_types`, or a
third beside #1901's.

## Decisions for review

1. **Compare the whole signature, not only the return type.** #2145 asks for
   the signature to be compared, but only the return type was probed for an
   override. The same message, probed for interfaces in #1901, covers parameter
   names, types, kinds, order and count, and the standard's signature includes
   them. Recommended: the whole signature, and probe one renamed parameter on
   an override in XAE before core 2 merges. The alternative is the return type
   now and parameters later under the same code.
2. **Three fixtures on `main` override with different parameters**, and become
   programs analysis rejects: `OVERRIDES` in `callee_resolution.rs:889`, and
   `METHODS` and the `THIS^`/`SUPER^` program in `xform_bind_call_arguments.rs`
   (REQ-CB-analyzer-028, -030). They test one pass on its own, so they keep
   passing. Recommended: keep them, with a line on each saying analysis rejects
   the program (P-override) and the test pins the pass alone. `call-binding.md`
   is unchanged: a call is still bound against the method it reaches.
3. **A standard function block as a base (row 3)** names a function block type,
   so it is a base and is not reported. Nothing inherits its fields or methods
   today, before or after this work. Recommended: out of scope; open an issue.
4. **An alias is its own type.** `TYPE MyInt : INT` overriding `INT` is a
   different result type, as ADR-0055 makes every alias. Not probed in XAE.
5. **Properties are not compared.** An overriding property of another type is
   out of scope; open an issue.

## Coordination with open pull requests

- **#2000** (types for `THIS^` and `SUPER^`) pins both gaps:
  `analyze_when_super_of_undeclared_base_then_pinned` and the `leaf_codes`
  fixture, whose `FB_Mid.Describe : BOOL` overrides `FB_Root.Describe : INT`.
  Whichever lands second updates the pins; for `leaf_codes`, by giving `FB_Mid`
  an override XAE accepts.
- **#1901** (interface conformance) could compare through `method` and
  `same_type` once core 2 lands, instead of `TypeKey`. Not part of this plan.
- **#2066** (inherited property names) and **#2199** (call binding plan) touch
  `callee_resolution.rs` and `inherited_fields.rs`; prefactor 1 changes the
  constructors there.

## Design doc reference

`specs/design/beckhoff-twincat-dialect.md` §1.4 (`EXTENDS` and `IMPLEMENTS`)
says the clauses are "stored as metadata on the function block AST node". Core
1 adds what the analyzer does with `EXTENDS` (the relation, P-base) and core 2
the override rule (P-override, the XAE results). The document has no
requirement IDs; the rule tests are the conformance tests.

## File map

| File | Prefactor 1 | Prefactor 2 | Core 1 | Core 2 |
|---|---|---|---|---|
| `analyzer/src/type_environment.rs` | `bases`, `base_of`, `inheritance_chain`; `insert_type` reports whether it entered | `methods`, `method` | | `same_type` |
| `analyzer/src/method_signature.rs` (new) | | `MethodSignature` | | comparison, first difference |
| `analyzer/src/xform_resolve_type_decl_environment.rs` | record the base | | | |
| `analyzer/src/xform_resolve_decl_types.rs` | | record the methods | | |
| `analyzer/src/callee_resolution.rs` | `chain` follows the relation; constructors take the environment | | | |
| `analyzer/src/intermediates/inherited_fields.rs` | follows the relation | | | |
| `analyzer/src/xform_resolve_expr_types.rs` | | `method` replaces `collect_method_types` | | |
| `analyzer/src/{rule_method_call_declared,rule_function_block_invocation,write_collector,xform_bind_call_arguments,rule_extends_field_duplicated}.rs`, `xform_insert_implicit_conversions/declared.rs`, `codegen/src/compile.rs` | pass the environment | | | |
| `analyzer/src/xform_resolve_symbol_and_function_environment.rs` | | | `extends` from `base_of` | |
| `analyzer/src/rule_extends_base_declared.rs` (new) | | | rule + tests | |
| `analyzer/src/rule_method_override_signature.rs` (new) | | | | rule + tests |
| `analyzer/src/stages.rs`, `analyzer/src/lib.rs` | | | register | register |
| `problems/resources/problem-codes.csv` | | | P-base | P-override |
| `docs/reference/compiler/problems/P####.rst` (new) | | | P-base | P-override |
| `docs/reference/language/object-orientation/extends.rst` | | | the base is a declared function block | an override keeps the signature |
| `specs/design/beckhoff-twincat-dialect.md` §1.4 | | | the relation, P-base | the override rule, XAE results |
| `analyzer/src/callee_resolution.rs`, `xform_bind_call_arguments.rs` (fixtures) | | | | Decision 2 |

## Tasks

Tracking issue: opened when this plan is approved, listing core 1 and core 2.

### Prefactor 1: resolve `EXTENDS` once, in the type environment

- [ ] `TypeEnvironment::bases`, `record_base`, `base_of`, `inheritance_chain`,
      with unit tests: no base, one level, two levels, a repeat ends the walk.
- [ ] `insert_type` reports whether it entered the type.
- [ ] Record the base in `xform_resolve_type_decl_environment` when it names a
      function block type; test that a structure, an undeclared name and a
      repeated block name record none.
- [ ] `FunctionBlocks` follows `inheritance_chain`; `from_library`,
      `MethodOwners::of_library` and `BlockParameters::of_library` take the
      type environment; update every caller, codegen's included.
- [ ] `collect_inherited_fields` follows `inheritance_chain`.
- [ ] `cd compiler && just`.

### Prefactor 2: method signatures in the type environment

- [ ] `MethodSignature` and `ParameterSection` reuse; `TypeEnvironment::methods`,
      `record_method`, `method`, with unit tests: own, inherited, overridden
      (nearest wins), sized string result, unresolved parameter type.
- [ ] Record the methods in `xform_resolve_decl_types`.
- [ ] Type method calls from `method`; delete `MethodTypes` and
      `collect_method_types`.
- [ ] `cd compiler && just`.

### Core 1: report an `EXTENDS` that names no function block (#2144)

- [ ] P-base in `problem-codes.csv` and its page under
      `docs/reference/compiler/problems/`.
- [ ] `rule_extends_base_declared`, registered in `stages.rs`; tests for rows
      1–3, an interface as base, and a declared base.
- [ ] The symbol environment's `extends` comes from `base_of`; test that
      `SUPER^` of a block whose base is undeclared names no block.
- [ ] `extends.rst` and `beckhoff-twincat-dialect.md` §1.4.
- [ ] `cd compiler && just`, `cd docs && just`, `cd specs && just`.

### Core 2: report an override whose signature differs (#2145)

- [ ] P-override in `problem-codes.csv` and its page.
- [ ] `TypeEnvironment::same_type`, with unit tests: same id, two
      `STRING[10]`, `STRING[10]` and `STRING[20]`, two in-place arrays of one
      shape, an alias and its base.
- [ ] Signature comparison naming the first difference.
- [ ] `rule_method_override_signature`, registered in `stages.rs`; tests for
      rows 4–11, a result added or removed, a parameter added, removed,
      retyped or reordered, an `R_EDGE` input changed, an abstract method, an
      unresolved parameter type (not reported), and an undeclared base (P-base
      only).
- [ ] Decision 2's fixture notes.
- [ ] `extends.rst` and `beckhoff-twincat-dialect.md` §1.4.
- [ ] `cd compiler && just`, `cd docs && just`, `cd specs && just`.

### Cleanup

- [ ] Issues for Decisions 3 and 5.
- [ ] Close this plan PR unmerged, and the tracking issue.
