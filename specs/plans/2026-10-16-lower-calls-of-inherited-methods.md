# Plan: Lower Calls of Inherited Methods

Checked against `main` at `f4086e4` (after #2319). File and line references are
as of that commit.

## Goal

`r := d.Get()` compiles from the AST when `derived EXTENDS base` and `base`
declares `Get` and no fields
(`codegen/tests/it/end_to_end_call_binding.rs`,
`end_to_end_when_method_inherited_from_base_without_fields_then_runs_base_method`).
After this work it takes the lowered route, and the bytecode is the same as the
AST route's. `just lowered-count` rises by 1. That is the only end-to-end
statement that calls a method of a fieldless base.

These stay P9999 on both routes:

- a call of a method of a base that declares fields
  (`compile_when_method_inherited_from_base_with_fields_then_not_implemented`);
- the inherited-method cases in `compile_extends_inheritance.rs`;
- `compile_when_inherited_method_called_then_lowered_route_leaves_it`
  (`compile_route_lowered_method.rs`), whose base, `Counter`, declares a field.

To make that possible, the plan decides how `EXTENDS` appears in the lowered
program.

## Current behaviour

| Where | What it does with `d.Get()` |
|---|---|
| Analyzer | `MethodOwners::declaring_block` resolves the call to `base.Get` (REQ-CB-analyzer-003). The type environment records each block's base. `inheritance_chain` walks it (`type_environment/fields.rs`, `pub(crate)`), and since #2319 `instance_fields` lists an instance's fields from the root of the chain down, each block's in declaration order. |
| Lowering's types (`lowering/src/types.rs`, `block_fields`) | A block's lowered type has the fields `TypeEnvironment::block_fields` gives: those the block declares itself. |
| Lowering (`lowering/src/function.rs`, `method_call`) | Finds the declaring block, then gives up (`declaring != block_name`): it only lowers a method the receiver's own block declares. |
| IR (`ir/src/lowered/program.rs:190`, `declare_method`; `expr.rs:605`, `check_user_call`) | A method's first parameter must be a non-null reference to an instance of the method's block, and a call's arguments must pass exactly the parameters' types (`passed != params`). A `Ref` to a `derived` does not pass for a reference to a `base`. |
| Design (`lowered-program.md`) | The fields a block inherits through `EXTENDS` are not in its lowered type yet (before REQ-LOW-lowering-170). Under REQ-LOW-ir-173, §3.8 lists "a call of a method the receiver's block inherits through `EXTENDS`" as not lowered. |
| AST route (`codegen/src/compile_method.rs:342-490`) | Finds the instance by name (`fb_instances`) and the declaring block through `method_owners`, then the method in the declaring block's layout. When the declaring block is not the instance's block and declares fields, it reports P9999: a derived block's layout holds no inherited fields. Otherwise it emits `FB_LOAD_INSTANCE` of the instance, the inputs, and `METHOD_CALL` with the declaring block's field offset and field count (0). |
| Bridge (`codegen/src/lowered/method.rs`, `method_site`) | Finds the method by the names of its block and itself, and the receiver by name. It refuses a receiver whose block is not the method's (`instance.type_id != block.type_id`). |

## Decisions for review

### 1. How `EXTENDS` appears: the type table records a block's base, and a reference to a derived instance passes for one to its base

- **The base relation.** `TypeKind::FunctionBlock` gains `base: Option<TypeId>`,
  the function block type the block extends.
- **The prefix invariant (new REQ-LOW-ir-176).** A block type that records a
  base begins with the base type's fields: the same types, in the same order.
  The type table refuses one that does not (P9998). So field `i` of a `base`
  is field `i` of a `derived`, and a body that reaches field `i` through its
  instance parameter reaches the same field whichever of the two the instance
  is.
- **The upcast (REQ-LOW-ir-173, widened).** A `Ref` to a place whose type
  extends `T`, directly or through its bases, passes for a parameter that
  refers to an instance of the function block type `T`. `check_user_call`
  compares through `TypeTable::extends` instead of `==`. Nothing converts:
  under the prefix invariant, the reference to the derived instance *is* a
  reference to its base part. A backend lays a derived block's fields out so
  that its base's fields come first, laid out as the base lays them out.
- **The call.** `d.Get()` is the `Call` of `base.Get`'s POU with `Ref(d)`
  first (REQ-LOW-lowering-076, unchanged). The method's declaration is
  unchanged too: its instance parameter refers to a `base`.

**When lowering records a base (new REQ-LOW-lowering-177).** A block's
lowered type records its base when the block's fields begin with the base
type's fields. Today a block's lowered type has only the fields it declares
itself, so that holds only when the base type has no fields. In that case the
base's own type records a base only when its own base has no fields, and so on
up the chain. Otherwise the type records no base, nothing extends, and a call
of an inherited method is not lowered:

| Program | `d.Get()` lowered | AST route |
|---|---|---|
| `derived EXTENDS base`, `base` declares `Get` and no field | yes | compiles |
| `derived` declares fields, `base` none | yes (`derived`'s fields begin with none) | compiles |
| `base` declares a field | no | P9999 |
| `derived EXTENDS mid EXTENDS base`, `mid` declares a field, `base` declares `Get` and no field | no | compiles; stays on the AST route |
| `derived EXTENDS mid EXTENDS base`, none declares a field | yes | compiles |

The analyzer's relation is the one lowering reads: `inheritance_chain`
becomes `pub`, and lowering takes a block's base from it. It does not resolve
the `EXTENDS` name again. This is the relation PR #2302 plans to make every
analysis follow (`base_of`, `inheritance_chain`). If that plan's `base_of`
lands first, lowering reads it instead. Nothing here duplicates #2302, which
stays inside the analyzer and does not touch the lowered program.

**Alternative A: the base as the first field of the derived type.** Each
derived block's type would get a hidden first field holding its base part, and
a call would pass `Ref(d.<base>)`, a `Field` projection. That needs no IR
change for the call. But an inherited field `d.k` would then lower to
`d.<base>.k`, which is not how the language names it. Every derived type's
field indices would shift by one, which the function block calls and `Copy`
statements lowered today would see. And it moves the base relation out of the
type into a naming convention. Not recommended.

**Alternative B: an explicit upcast projection** (`Projection::Base`). This
says the conversion out loud and leaves a backend free to put the base part
anywhere. But it needs the same base relation and invariant in the type table
to type-check, and every backend has to handle a new projection that, under
the prefix layout, emits nothing. Not recommended unless a backend needs a
base part at an offset other than 0.

### 2. The general rule or only the instance parameter

The upcast is a property of references to function block instances. It could
be limited to a method's instance parameter, which is the only argument
lowering makes it for in this change. Recommended: state it for any `Ref`
argument for a function block parameter, because that is what the type
relation means, and `SUPER^` and in-outs of a block type will need it. Only
the receiver produces it here.

### 3. How the bytecode bridge finds the method and the receiver

As `compile_method.rs` does:

- **Method.** `method_site` already finds it in the layout of the block whose
  method the POU is (`pous.method_of(pou)`). That is the declaring block,
  `base`, as the AST route's `method_owners.declaring_block` gives.
- **Receiver.** By name (`variable_name`, then `fb_instances`), unchanged.
- **The check.** The instance may be of another block than the method's when
  the method's block declares no fields (`num_fields == 0`), as the AST route
  requires. The IR has already checked that the receiver's block extends the
  method's block. With fields, `method_site` returns `None` and the statement
  falls back to the AST route, which reports P9999. That holds even after
  lowering gains inherited fields, until this backend's layout holds them.
- **Emission.** Unchanged: `FB_LOAD_INSTANCE` of `d`, the inputs, then
  `METHOD_CALL` with `base.Get`'s function, `base`'s field offset and 0
  fields. This is the AST route's sequence.

### 4. How it fits inherited fields later

When lowering gains inherited fields, a block's lowered type takes its
fields from `TypeEnvironment::instance_fields` (#2319) instead of
`block_fields`. That list already puts the root's fields first and each
block's own after its base's, so the prefix invariant holds for every block
by construction. (A redeclared inherited name is P4044, so no clean analysis
has one.) REQ-LOW-lowering-170 changes then.

From then on, every block records its base and every inherited method call
lowers. Nothing in the method body changes: it reaches `k` as field `i` of
its instance parameter, and that is field `i` of every derived instance. A
statement naming an inherited field `k` lowers to the field at `k`'s index in
the derived type. The bytecode bridge keeps refusing a base with fields
(decision 3) until codegen's layout places inherited fields
([#1438](https://github.com/ironplc/ironplc/issues/1438)). ADR-0041's later
dynamic dispatch reads the same relation.

## Architecture

- **IR** (`ir/src/lowered/types.rs`, `expr.rs`): the `base` field, the prefix
  check in `TypeTable::push`, `TypeTable::extends`, and the comparison in
  `check_user_call`.
- **Analyzer** (`type_environment/fields.rs`): `inheritance_chain` becomes
  `pub`.
- **Lowering** (`lowering/src/types.rs`): records a block type's base when
  the prefix holds. In `lowering/src/function.rs`, `method_call` resolves the
  method in the declaring block and accepts it when the receiver's lowered
  type extends that block's. `call_args` compares through `extends`.
- **Bridge** (`codegen/src/lowered/method.rs`): `method_site`'s check as in
  decision 3.

## Prefactoring

1. **The base relation in the IR's type table.** `TypeKind::FunctionBlock`'s
   `base`, the prefix check, `TypeTable::extends`, and `check_user_call`
   comparing through it. Lowering records no base yet, so every type and
   every call is as today, and every existing test passes unchanged. New IR
   unit tests cover a base whose fields are a prefix, one that is not
   (refused), `extends` over two levels, and a call that passes a `Ref` to a
   derived instance for a base's instance parameter and one to an unrelated
   block (refused). Without it, the core change would mix a new IR invariant
   with the lowering that first uses it.

No other prefactor: lowering's `method_call` already finds the declaring
block, and the bridge already finds the method by its block.

## Design doc reference

`specs/design/lowered-program.md`. The core change updates, in the same PR:

- §3.2, the paragraph before REQ-LOW-lowering-170: a block type records its
  base when its fields begin with the base's, which today means a base
  without fields; inherited fields remain to come, as in decision 4;
- **REQ-LOW-ir-176** (new, §3.2 or §3.9): the prefix invariant, refused
  otherwise;
- **REQ-LOW-ir-173**, widened: the first argument of a method's call is a
  `Ref` to an instance of the method's block or of a block that extends it;
- **REQ-LOW-lowering-177** (new, §3.8): a call of an inherited method lowers
  to the `Call` of the declaring block's method, with a `Ref` to the receiver
  first, when the receiver's type extends that block;
- §3.8's "Not lowered yet" bullet narrows to a method of a base whose chain
  declares fields;
- the bridge paragraph after REQ-LOW-ir-173, with **REQ-LOW-codegen-178**
  (new): while both routes exist, the bytecode backend emits such a call as
  the AST route does, and leaves to it a call of a method of a block that
  declares fields;
- §7 (Backend Contract): a derived block's layout begins with its base's.

The numbers start at 176 because the plan for calls with outputs inside an
expression ([#2324](https://github.com/ironplc/ironplc/pull/2324)) claims 174
and 175. Whichever core change lands second
renumbers if needed. `call-binding.md`'s REQ-CB-codegen-003 and
REQ-CB-codegen-005 describe behaviour that does not change.

## File map

| File | Prefactor | Core |
|---|---|---|
| `ir/src/lowered/types.rs` | `base`, prefix check, `extends` | |
| `ir/src/lowered/expr.rs` | `check_user_call` through `extends` | |
| `ir/src/lowered/tests/methods.rs`, `tests/fb_calls.rs` | `base: None` in fixtures; new tests | |
| `lowering/src/types.rs` | `base: None` | record the base when the prefix holds |
| `analyzer/src/type_environment/fields.rs` | | `inheritance_chain` is `pub` |
| `lowering/src/function.rs` | | `method_call` accepts an inherited method; `call_args` through `extends` |
| `lowering/src/method_tests.rs` | | the IR of `r := d.Get()`, a derived block with fields, and `unsupported` for a base or middle block with fields |
| `codegen/src/lowered/method.rs` | | `method_site`'s check |
| `codegen/tests/it/compile_route_lowered_method.rs` | | same code as the AST route for `r := d.Get();`, `d.Get();` and a call with an input; values |
| `specs/design/lowered-program.md` | | as above |

## Tasks

### Prefactor: the base relation in the IR's type table

- [ ] `TypeKind::FunctionBlock { base }`; `None` everywhere it is built.
- [ ] Prefix check in the type table; `TypeTable::extends`.
- [ ] `check_user_call` compares a `Ref` argument through `extends`.
- [ ] IR unit tests as above; `cd compiler && just`.

### Core: lower calls of inherited methods

- [ ] `inheritance_chain` is `pub`; lowering records a block's base when the
      prefix holds.
- [ ] `method_call` accepts a method of a block the receiver's type extends.
- [ ] `method_site` accepts a receiver of a derived block when the method's
      block declares no fields.
- [ ] Lowering and route tests as in the file map; the with-fields tests
      still fail on both routes.
- [ ] Design text and requirements; conformance tests for REQ-LOW-ir-173,
      -176, REQ-LOW-lowering-177 and REQ-LOW-codegen-178.
- [ ] `cd compiler && just`, `cd specs && just`; `just lowered-count` rises
      by 1.

### Cleanup

- [ ] Close this plan PR unmerged.
