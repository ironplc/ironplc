# IMPLEMENTS conformance: fix the gaps found in review

## Goal

Fix the gaps in `rule_interface_conformance` that review of #1901 found
and #2370 lists:

1. `IMPLEMENTS` of an undeclared name, or of a type that is not an
   interface, is accepted silently. Before #1900 it was P9999.
2. A block that inherits an `IMPLEMENTS` through `EXTENDS` is not checked,
   so a derived block can override an interface method with a different
   signature and still convert to the interface.
3. A block that implements an interface and one that extends it gets the
   same P4067/P4068 twice.
4. The rule builds its own list of a method's parameters, and its own type
   comparison, instead of using the ones the analyzer already has. There
   should be one place for each.
5. Docs: `implements.rst` repeats P4068's matching rules, and `P4067.rst`
   does not say that a `METHOD ABSTRACT` counts.

Out of scope, by decision:

- Access specifiers: a `PRIVATE` or `PROTECTED` method still satisfies an
  interface member.
- The type comparison stays strict: `STRING` is not `STRING[80]`, and an
  alias is not its base type.
- The false P4005 for an interface method whose parameter has an interface
  type that leads back to it (#2359).

## Architecture

**Signatures.** The type environment already records the signature of each
block method (`type_environment/methods.rs`, #2345): its result and its
`VAR_INPUT`/`VAR_OUTPUT`/`VAR_IN_OUT` parameters by type id, and its edge
inputs. It also finds a block's method through its `EXTENDS` chain
(`TypeEnvironment::method`). Interfaces get the same record: each method
prototype's signature, built by the same `method_signature`, which then
accepts a prototype as well as a declaration. Properties are recorded with
their type id and accessors, for interfaces and blocks alike, and a
block's property is found through its `EXTENDS` chain as a method is. The
rule then compares recorded signatures instead of the AST, and its
`ParameterTypeKey`, `TypeKey`, `LengthKey` and `same_edge_inputs` go away.
`FunctionBlocks::resolve_property` goes too, if nothing else uses it.

**Same type.** `rule_var_external_matches_global::is_same_type` already
decides whether two type ids are the same type: equal ids, or two types
spelled in place with the same shape. It moves to
`TypeEnvironment::same_type`, and the conformance rule uses it. A type is
"spelled in place" when no name maps to its id. That covers sized strings
as well: a sized string is recorded under the name of its elementary type,
but that name does not map to it. A `VAR_EXTERNAL` cannot declare a sized
string (it is a syntax error), so the move does not change what
`rule_var_external_matches_global` reports.

**Which interfaces a block must provide.** `Supertypes` (from #1900)
already records each block's base and `IMPLEMENTS`, and each interface's
`EXTENDS`. It gets a way to list a block's supertypes transitively, each
once. The rule checks each block against every interface in that list,
which is the same set of interfaces `rule_interface_conversion` lets the
block convert to. This fixes gaps 2 and 3.

To report each problem once, a member is checked against block `B` and
interface `I` only where `B` introduces the problem:

- `B` lists `I` in its own `IMPLEMENTS`, directly or through an interface
  it lists: a missing member or a different one is reported, with the
  primary label at the `IMPLEMENTS` entry, as today.
- Otherwise `B` inherits `I` from a base. Only a member that `B` declares
  itself (an override) is checked, with the primary label at `B`'s member.
  A member `B` inherits unchanged was already checked against the base.

**Undeclared or non-interface names (gap 1).** A new problem code, P4070
("IMPLEMENTS names a type that is not a declared interface"), is reported
at the name in `IMPLEMENTS`. It covers both an undeclared name and a
declared type that is not an interface, such as a function block, with
the label saying which.

## Prefactoring

Two prefactors, each its own PR, branched from `main`. Both are
behaviour-preserving: every existing test passes unchanged.

1. Move `is_same_type` to `TypeEnvironment::same_type`, with "spelled in
   place" meaning that no name maps to the id. Its tests move with it.
2. Record interface member signatures and property signatures in the type
   environment. `method_signature` builds a signature from a prototype as
   well as a declaration. Block properties are recorded with their type
   id and accessors instead of only their names, and a block's property
   is found through its `EXTENDS` chain. Nothing reads the interface
   records yet; the core change does.

## Design doc reference

- `specs/design/beckhoff-twincat-dialect.md` §1.3 (conformance).
- ADR-0041 (interface dispatch, Phase 2): the recorded interface
  signatures are what a call through an interface binds against later.

## File map

Prefactor 1:

- `compiler/analyzer/src/type_environment.rs`: `same_type`
- `compiler/analyzer/src/rule_var_external_matches_global.rs`: use it

Prefactor 2:

- `compiler/analyzer/src/type_environment/methods.rs`: `method_signature`
  from a prototype too
- `compiler/analyzer/src/type_environment/fields.rs`: property signatures
  in the block record; interface record; property lookup through `EXTENDS`
- `compiler/analyzer/src/xform_resolve_decl_types.rs`: record interfaces
  and property signatures
- `TypeEnvironment::has_property`, the one reader of the block record's
  property names (used by `rule_use_declared_field.rs`), keeps its
  behaviour

Core change:

- `compiler/analyzer/src/supertypes.rs`: list a block's supertypes
  transitively
- `compiler/analyzer/src/rule_interface_conformance.rs` and `tests.rs`
- `compiler/analyzer/src/callee_resolution.rs`: remove `resolve_property`
  if unused
- `compiler/problems/resources/problem-codes.csv`: P4070
- `docs/reference/compiler/problems/P4070.rst` (new), `P4067.rst`,
  `P4068.rst`
- `docs/reference/language/object-orientation/implements.rst`
- `specs/design/beckhoff-twincat-dialect.md` §1.3

## Tasks

### Prefactor 1: one check for "same type"

- [ ] Move `is_same_type` to `TypeEnvironment::same_type`, and its tests
- [ ] Treat an id no name maps to as spelled in place
- [ ] `rule_var_external_matches_global` calls it; its tests pass unchanged
- [ ] `cd compiler && just`

### Prefactor 2: record interface and property signatures

- [ ] `method_signature` from a method prototype as well as a declaration
- [ ] Interface record: method signatures and property signatures
- [ ] Block record: property signatures instead of names; property lookup
      through `EXTENDS`
- [ ] Tests for the new records; existing tests pass unchanged
- [ ] `cd compiler && just`

### Core change: conformance gaps

- [ ] P4070 for an `IMPLEMENTS` name that is undeclared or not an
      interface, with tests for both
- [ ] Check every interface in a block's transitive supertypes, each
      once; report each problem once, at the block that introduces it
- [ ] Tests: a derived block overriding with a different signature, a
      derived block inheriting a member unchanged, a block implementing
      an interface and one that extends it
- [ ] Compare recorded signatures with `same_type`; remove the rule's
      own keys; existing conformance tests pass
- [ ] Docs: P4070 page; `implements.rst` links to P4068 instead of
      repeating it; `P4067.rst` says a `METHOD ABSTRACT` counts; design
      doc §1.3
- [ ] `cd compiler && just`, `cd docs && just compile duplicates`,
      `cd specs && just`
