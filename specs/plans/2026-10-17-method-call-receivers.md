# Plan: Method Calls on Any Variable Reference

Checked against `main` at `be6c29bf1`; line numbers are as of that commit.
The reproductions below were run with `ironplcc check --dialect twincat`.

Fixes [#1422](https://github.com/ironplc/ironplc/issues/1422) (method-call
receivers are limited to a bare identifier or `THIS^`/`SUPER^`).

## Goal

A method call takes its receiver from a one-token grammar rule. Any receiver
that is itself a variable reference is a syntax error (P0002), although
TwinCAT and CODESYS accept it and ordinary variable references already parse
these chains.

After this work, the receiver of a method call is any symbolic variable
reference that names a function block instance:

```st
axis.drive.Start();        (* nested field *)
pMotor^.Start();           (* POINTER TO FB_Motor *)
motors[i].Start();         (* array element *)
THIS^.axis.Start();        (* nested under a self reference *)
r := pMotor^.Rate();       (* the same receivers in an expression *)
```

The method is resolved statically, on the declared type of the receiver:
`pMotor : POINTER TO FB_Motor` resolves `Start` on `FB_Motor` and its
`EXTENDS` chain, as `motor.Start()` does. Dynamic dispatch through a pointer
to a base type is out of scope and stays with the dispatch design. A call
that would need it is rejected (see Analysis), so the compiler never picks a
method the program might not mean.

Not in scope: a call result as a receiver, such as
`axis.drive.At(1).Start();`. That receiver is an expression, not a variable
reference, so it needs a different receiver type. I have not checked that
TwinCAT accepts it. It becomes an issue (see Cleanup), and the receiver type
below does not rule it out.

The corpus check in `~/code/brotlib` (218 TwinCAT files, one `ironplcc check`
per file) gives 91 files that fail first with P0002. About 15 of them fail
on a call such as `pPendant^.SetState(pPendant^.fbPendantErrorState);`, a
state-machine pattern where each state block holds a `POINTER TO` its owner.
That count comes from a regular-expression classification of the first
error per file, so it is approximate, and fixing the first error in a file
can expose the next one. The issue's own count from an earlier run is 19 of
85: 17 through a pointer, 1 array element, 1 nested field.

## Current behaviour

| # | Program (in `main`) | Today |
|---|---|---|
| 1 | `m.Start();` | accepted |
| 2 | `THIS^.Start();`, `SUPER^.Start();` | accepted |
| 3 | `axis.drive.Start();` | P0002 |
| 4 | `p := ADR(m); p^.Start();` | P0002 |
| 5 | `motors[i].Start();` | P0002 |
| 6 | `p := ADR(m); r := p^.Rate();` | P0002 |

Rows 3 to 6 use a block `FB_Motor` with methods `Start` and `Rate : INT`, a
block `FB_Axis` with `drive : FB_Motor`, and a program that declares
`axis : FB_Axis`, `motors : ARRAY[1..2] OF FB_Motor`, `m : FB_Motor` and
`p : POINTER TO FB_Motor`.

### Where the receiver is a single name

| Site | What it does with the receiver |
|---|---|
| `parser/src/parser.rs:2281` `method_receiver` | `self_ref()` or one `identifier()` |
| `dsl/src/textual.rs:368` `MethodReceiver` | `Instance(Id)` or `SelfRef(SelfRefVariable)` |
| `analyzer/src/callee_resolution.rs:154` `receiver_block` | the declared type of the named instance, or `SymbolEnvironment::self_type` |
| `analyzer/src/rule_method_call_declared.rs` `check_call` | `instance_type` for a name, `self_type` for `THIS^`/`SUPER^` |
| `analyzer/src/xform_resolve_expr_types.rs:235, 304` | types a call, and its output arguments, only for `Instance` |
| `analyzer/src/write_collector.rs:442` | marks the named instance as invoked |
| `analyzer/src/rule_pou_hierarchy.rs:193` | reports a function that invokes the named instance |
| `codegen/src/compile_method.rs:350` | the AST route compiles only `Instance` |
| `lowering/src/function.rs:310, 395` | lowering resolves and passes only `Instance` |
| `plc2plc/src/renderer.rs:1989` | renders the name or `THIS^`/`SUPER^` |

### What already exists

- `symbolic_variable` (`parser.rs:1087`) composes `.field`, `[i]` and `^`
  into any chain, with free-format whitespace, and `SymbolicVariableKind` has
  a `SelfRef` head, so `THIS^` needs no special case in a chain.
- `variable_type::type_id` (`analyzer/src/variable_type.rs`) walks a
  reference to the id of its type through fields, subscripts and
  dereferences, so it gives the function block type of every receiver form
  above.
- `Lowering::symbolic_place` (`lowering/src/place.rs:18`) already builds the
  place of a name, field, element or dereference. `call_args` passes the
  receiver as `Arg::Ref(place)`.

## Architecture

### The receiver is a restricted variable path

`MethodReceiver` stays, and becomes a head plus steps instead of an enum of two
spellings of one:

```rust
pub struct MethodReceiver {
    pub head: ReceiverHead,        // Named(Id) or SelfRef(SelfRefVariable)
    pub steps: Vec<ReceiverStep>,  // Field(Id), Subscript(..), Deref
}
```

A bare name is a head with no steps, and `THIS^` is a `SelfRef` head with no
steps, so no meaning has two representations. Whether the steps can reuse the
element type of `SymbolicVariableKind` is settled in the prefactor.

A receiver converts to a `SymbolicVariableKind` without failing, because every
receiver is a valid variable reference. `variable_type::type_id` and
`Lowering::symbolic_place` take that form. The reverse conversion can fail
and is done only by the parser.

Rejected: replacing `MethodReceiver` with `SymbolicVariableKind` on
`MethodCall`. `SymbolicVariableKind` has `BitAccess` and `PartialAccess`,
which cannot be receivers, so every consumer would have to reject states the
type should not allow.

Rejected: keeping the two-variant `MethodReceiver` and adding a third variant
`Variable(SymbolicVariableKind)`. Every consumer would then handle
`Instance(a)` and `Variable(Named(a))` as the same thing.

### Grammar

Parse the whole chain, then split off the last element, as the issue
suggests; it keeps one code path.

```
rule method_call() -> MethodCall =
  v:symbolic_variable() _ tok(LeftParen) _ params ... end:tok(RightParen) {?
    // v must end in a field name: split `a.b.C` into receiver `a.b`, method `C`
  }
```

A chain that does not end in `Element::Struct` (a bare name, `a[i]`, `a.3`)
is not a method call, so the action returns an error and the PEG falls
through to the function-call and function-block-call alternatives as today.
`primary_expression` already tries `method_call` before `variable` (the
comment at `parser.rs:2169` explains why), so the expression form needs no
reordering. The span of the receiver is the span of the chain.

### Analysis

`receiver_block` (in `callee_resolution.rs`) resolves a receiver with
`variable_type::type_id`, and it succeeds only when that type is a function
block: it returns the block's name. Every pass that calls `receiver_block`
therefore gets the new forms:

- `rule_method_call_declared`: resolves the method on the block as today. A
  receiver that does not resolve to a function block is reported with the
  existing P4012 (`FunctionBlockNotInScope`), at the receiver. No new problem
  code.
- `xform_resolve_expr_types`: types a method call and its output arguments
  from the receiver's block id.
- `write_collector`: marks the root variable of the receiver as invoked, the
  way `a.b := x` marks `a`.
- `rule_pou_hierarchy`: reports a function that invokes a function block
  instance through the root variable of the receiver. See open question 2.
- `call_binding` and `MethodOwners` take the block name and are unchanged.

The declared type of the receiver is always known. What is not known is the
type of the instance at run time: a `POINTER TO FB_Base` can hold an
`FB_Derived`. A static call then runs `FB_Base`'s method, and TwinCAT may run
the override. That is not verified; the dispatch design (#1870) records the
XAE check as still open.

So a call whose static target may be wrong is rejected, in the analyzer, with
a new problem code (documented in `P####.rst`; the number is taken in core
change PR 1). The call is rejected when some function block in the program
extends the receiver's block (directly or not) and declares a method of the
called name. Then the pointer can hold an instance that overrides the method.
When no function block overrides the method, the static call is exact and is
accepted. This relies on the compiler seeing every function block type in the
program, the same assumption as the dispatch design. An interface-typed
receiver is always dynamic, so it gets the same rejection; I have not
checked what it reports today.

### Code generation

- **Lowering route**: `method_call` and `call_args` use `symbolic_place` for
  the receiver and resolve the method from the receiver's block, so the new
  forms compile with no new lowering machinery. A dereference place already
  carries the reference's nullability; a null pointer receiver should trap the
  way a null dereference does elsewhere. I have not confirmed this and the
  core change PR must test it.

The AST route (`codegen/src/compile_method.rs`) is not touched. The lowered
route is replacing it (#2347).

### Gating

No new `--allow-*` flag. The call form `instance.M()` is already accepted
without a flag of its own, and the `METHOD` declarations a call resolves to
are gated by `allow_fb_inheritance`, so without that flag every such call is
P4046 or P4012. The new receivers add no syntax of their own, since the
chains already parse as variables.

## Prefactoring

One prefactor, behaviour-preserving: **widen `MethodReceiver` from two
spellings to a restricted path**, with the grammar still producing only a bare
name or `THIS^`/`SUPER^`. After it, the core change touches the grammar and
the analysis, and no consumer's shape.

`SymbolicVariableKind` is the wrong type for the field, because it also
allows `BitAccess` and `PartialAccess`, which cannot be a receiver. The
restricted type keeps them unrepresentable.

The risk is that consumers match `Instance` and `SelfRef` today. Each moves
to the head of the path, and a bare name is a head with no steps. Every visitor
and fold that walks the receiver must be audited, and where one would change
behavior the prefactor keeps it as it was. The prefactor PR states the audit.
The existing tests must pass unchanged apart from constructing the new type.

## Design doc reference

`specs/adrs/0041-staged-method-and-interface-dispatch.md` (static dispatch,
Phase 1), and the draft dispatch design in #1870, which is not on `main`. This
plan adds no dispatch behavior.

## File map

| File | Change |
|---|---|
| `compiler/dsl/src/textual.rs` | prefactor: `MethodReceiver` as a head plus steps; conversion to `SymbolicVariableKind` |
| `compiler/dsl/src/fold.rs`, `visitor.rs` | prefactor: dispatch on the new `MethodReceiver` |
| `compiler/parser/src/parser.rs` | prefactor: build the receiver; core 1: parse a chain and split it |
| `compiler/parser/src/tests/method_call_expression.rs`, `this_super.rs` | prefactor: adjust; core 1: new receiver forms |
| `compiler/analyzer/src/callee_resolution.rs` | `receiver_block` through `variable_type::type_id` |
| `compiler/analyzer/src/rule_method_call_declared.rs` | use `receiver_block` for every receiver; reject a call an override could change |
| `compiler/analyzer/src/xform_resolve_expr_types.rs` | type calls and outputs from the receiver's block |
| `compiler/analyzer/src/write_collector.rs`, `rule_pou_hierarchy.rs` | root variable of the receiver |
| `compiler/plc2plc/src/renderer.rs` and its tests | render the receiver as a variable; round-trip tests |
| `compiler/lowering/src/function.rs` | core 2: `symbolic_place` for the receiver |
| `compiler/codegen/tests/it/compile_route_lowered_method.rs`, `end_to_end_methods.rs` | core 2: end-to-end tests |
| `docs/reference/language/object-orientation/method.rst` | core 1: receivers; core 2: what compiles |

## Tasks

### Prefactor PR: the receiver is a variable reference

- [ ] Change `MethodReceiver` to a head plus steps, with a conversion to
      `SymbolicVariableKind`; update its `Display`, fold and visitor dispatch.
- [ ] Update the parser, `plc2plc`, `callee_resolution`, `write_collector`,
      `rule_pou_hierarchy`, `xform_resolve_expr_types`, `codegen`, `lowering`.
- [ ] Audit every visitor and fold that overrides a variable method; keep each
      pass's behavior; list the audit in the PR description.
- [ ] No test changes beyond constructing the new type. Run `cd compiler && just`.

### Core change PR 1: parse and analyze any receiver

- [ ] Parse `symbolic_variable() ( args )` and split the last field off as the
      method; keep `THIS^.M()` and `SUPER^.M()` as they parse.
- [ ] Parser tests: each form in the Goal, in statement and in expression
      position, with whitespace and comments between elements; a chain that
      does not end in a field (`a[i](...)`) is still not a method call.
- [ ] `receiver_block` through `variable_type::type_id`; method resolution,
      typing, write marking and the function-invocation rule for the new forms.
- [ ] Analyzer tests: rows 3 to 6; a receiver that is not a function block
      (P4012); a method that is not declared on the receiver's block (P4046); a
      method without a result used in an expression (P4057); a `THIS^.axis.M()`
      inside a method; an inherited method through a field.
- [ ] Reject a call through `POINTER TO FB_Base` (or an interface) when a
      derived block overrides the method; new problem code with its
      `P####.rst`; tests for the overridden and the not-overridden case.
- [ ] `plc2plc` round-trip tests for each form.
- [ ] Docs: `method.rst`.

### Core change PR 2: compile any receiver on the lowered route

- [ ] `method_call` and `call_args` take the receiver's place from
      `symbolic_place` and the block from the receiver's type.
- [ ] End-to-end tests: nested field, array element, pointer receiver in
      statement and expression position, an inherited method, a method with
      outputs, and a null pointer receiver (expected to trap; confirm).
- [ ] Docs: say which receivers compile.

### Cleanup

- [ ] Close #1422 when core 2 lands. Open an issue for anything this plan
      names and does not deliver: `SUPER^()` as a call of the base body,
      `FB_X(THIS)` as an initializer argument, dispatch through a pointer
      to a base type, and a call result as a receiver (`a.At(1).Start()`).
- [ ] Update `specs/plans/twincat-status.md` on `twincat-dev` with the corpus
      change. Close this plan PR unmerged.

## Open questions

1. **Pointer receiver at run time.** TwinCAT accepts `p^.M()` for
   `p : POINTER TO FB_X`. What it does when `p` holds an instance of a type that
   extends `FB_X` is unverified (the XAE check listed in the TwinCAT status
   document). This plan rejects the call when an override exists and calls the
   declared type otherwise. Is that acceptable until the dispatch design
   lands?
2. **Functions.** `rule_pou_hierarchy` reports a function that invokes a
   function block instance by its name. For `p^.M()` inside a function, where
   `p` is a pointer parameter or local, should the call also be reported? I
   propose yes, by the same rule, but I have not checked what TwinCAT does.
3. **Prefactor risk.** The audit of the passes that walk the receiver is the
   one step I cannot size without doing it. If it turns up behavior that
   cannot be preserved, the fallback is the rejected third-variant option.
4. **Class hierarchy risk.** The override rule assumes that the whole set of
   function block types is known when a call is analyzed. If that fails (for
   example, with separately compiled libraries), the rule has to move to
   lowering or become stricter. This is the place where the POU hierarchy
   assumption is most likely to break.
