# Plan: Resolve Call Targets and Argument Binding in the Analyzer

Checked against `main` at `377b8b9`; line numbers are as of that commit. Every
reproduction below was rerun there.

## Goal

Codegen lowers calls; it does not decide what they mean. Today it still
decides, with lookups and precedence rules of its own:

- **what a call reaches**: which function block type (standard or
  user-defined) and which method body;
- **how each argument binds**: by position or by name;
- **what an omitted input does**;
- **the order arguments are evaluated in**.

After this work the analyzer decides all four. Codegen's route that compiles
from the AST, and lowering after it, read the analyzer's answer and decide
nothing.

## Where this sits relative to lowering

`specs/design/lowered-program.md` (#2011, `proposed`) puts a lowering stage
between analysis and the backends. Two of its decisions shape this plan:

- **ADR-0058: the analyzer decides what can make a program invalid, and
  records it in the analyzed `Library`.** A binding can make a program
  invalid (an unknown or duplicated parameter, a missing input), so the
  binding is the analyzer's. §4 of the design already lists "Argument order
  and count" among the decisions the analyzer records, "as positional
  arguments", which today covers function calls only.
- **Lowering makes the decisions no valid program can get wrong**, among them
  the passing mode (`Arg::Value`, `Ref`, `Copy`) and, under ADR-0060, how a
  standard function block runs. So neither is recorded here.

#2180 (variable binding) sets how a decision is recorded: in the language's
own terms, with one analyzer function that the rules, today's codegen and
lowering all ask, and no identity on the AST that exists only to key storage.
Its coordination note says a call binding names its parameters by `DeclRef`.

So this plan:

1. **Records the binding as the call's normal form**: named arguments, in the
   parameter declaration order. A parameter is named as the callee declares
   it; with the callee's scope, that is its `DeclRef`. This is the
   positional rewrite functions already get, extended to every kind of call.
2. **Answers "which method" with one analyzer function**, as #2180 answers
   "which variable".
3. **Changes today's codegen only as much as reading those answers takes.**
   That code is deleted as lowering covers calls. Nothing is built in codegen
   that lowering replaces: no new codegen tables, no ids keyed for codegen.

## Current behaviour

### What codegen decides

| Site | What it decides |
|---|---|
| `compile_setup.rs:213` | An FB instance's type, by `resolve_fb_type(uppercase name)` (`compile_call.rs:735`) **before** the user FB types. The table maps `CTU_INT`, `CTD_INT` and `CTUD_INT`, which the analyzer does not treat as standard names (`stdlib_function_block.rs:343`), to built-in blocks, so a user block of that name never runs. |
| `compile_stmt.rs:162` `compile_fb_call` | Stores `NamedInput`s only (`:181`), so positional inputs are dropped. Evaluates them in source order. Reads outputs in source order (`:205`) and ignores `Output::not`. |
| `compile_method.rs:298-307` | Finds the instance's FB by scanning for its VM type id, then the method by lowercase name on that block only, so an inherited method is P9999. |
| `compile_method.rs:325-346` | A positional argument goes to the first empty slot among **all** input-compatible parameters, a named one by name. Evaluation in parameter order; P9999 for an omitted input. |
| `compile_method.rs:124-137` | A `VAR_IN_OUT` parameter gets a value slot, so it is passed by value. `var-in-out-parameters.md` says an unsupported `VAR_IN_OUT` is reported as P9999, "rather than compiled with by-value semantics". |

Function calls are not on this list. Codegen finds a function by the name the
analyzer resolved, with the same case-insensitive key as the
`FunctionEnvironment`, and a user function cannot share a standard
function's name. So codegen cannot choose a different function. Lowering
replaces that lookup anyway (§4 of the design: "No stage after the analyzer
matches a function's name").

### The analyzer's binders

Five places bind arguments, by three different rules:

| Binder | Calls | Positional rule |
|---|---|---|
| `xform_named_to_positional_args.rs:76` `plan_positional_order` | `Function` | rewrites named arguments into the order of `VAR_INPUT` + `VAR_IN_OUT` |
| `function_environment.rs:138` `FunctionSignature::bind_inputs` | `Function` (5 rules) | zips positional arguments with `VAR_INPUT` + `VAR_IN_OUT` |
| `call_assignment_check.rs:73` `bind_inputs` | `FbCall`, `MethodCall` (2 rules, `write_collector.rs:295`) | `VAR_INPUT` only |
| `write_collector.rs:404` | `Function` | its own zip |
| `xform_insert_implicit_conversions/literal.rs:302` `method_arguments`, `:470` `fb_call_inputs` | `MethodCall`, `FbCall` | a position counter: `VAR_INPUT` + `VAR_IN_OUT` for methods, `VAR_INPUT` for blocks |

`callee_resolution.rs` resolves a method through `EXTENDS`
(`FunctionBlocks::resolve_method`, `:77`), but the conversion pass looks
methods up in a table of its own (`declared.rs:99`, `Declarations::method`)
that does not. `InstanceTypes` (`callee_resolution.rs:112`) is a per-rule table
of instances, which [Name and Type
Lookup](../steering/compiler-architecture.md#name-and-type-lookup) says a rule
should not build, now that `SymbolEnvironment::resolve_variable` answers it.

### Reproductions

`cargo build -p ironplc-cli -p ironplc-vm-cli`; `--dialect iec61131-3-ed3`;
`ironplcvm run --scans 1 --dump-vars=-`. Every program passes `check`.

| # | Program | Today | After |
|---|---|---|---|
| 1 | FB `a, b : DINT`, `q := a * 10 + b`; `inst(1, 2)` | `q` = 0 | 12 |
| 2 | User `FUNCTION_BLOCK CTU_INT` setting `CV := 42`; `c(CU := TRUE)` | `CV` = 1 (built-in `CTU` ran) | 42 |
| 3 | Method `Inc`, `VAR_IN_OUT v`, `v := v + 1`; `inst.Inc(v := counter)`, `counter = 5` | `counter` = 5 | 6 |
| 4 | `derived EXTENDS base`, `Get` declared in `base`; `d.Get()` | P9999 at `compile_method.rs:307` | 7 |
| 5 | Method `M(a; b := 7)`; `inst.M(a := 1)` | P9999 at `compile_method.rs:346` | P4018 from `check` |
| 6 | `inst(b := c1.Next(), a := c1.Next())` and `f(b := c2.Next(), a := c2.Next())`, both `a * 10 + b` | FB 21, function 12 | both 12 |
| 7 | `inst(a := 1, a := 3, b := 2)` on an FB, and on a method | last write wins (32) | P4024 from `check` |
| 8 | `inst(NOT z => nz)`, `z` = TRUE | `nz` = TRUE | FALSE |
| 9 | Method with `VAR_IN_OUT x`, then `VAR_INPUT a`; `inst.M(5)` | P9999 at `compile_method.rs:346`: the rule counts `5` against `VAR_INPUT`, codegen binds it to `x` | P4018 from `check`: `x` is not bound |

## Architecture

### The normal form of a call

A binding pass, `xform_bind_call_arguments`, replaces
`xform_named_to_positional_args` at the same place in `stages::resolve_types`
(after the environments are built, before expression types and the rules).
For every `Function`, `FbCall` and `MethodCall` whose arguments bind, it
rewrites the argument list into the call's **normal form**:

- **One `NamedInput` per supplied input** (`VAR_INPUT` or `VAR_IN_OUT`),
  naming the parameter as the callee declares it, **in parameter declaration
  order**. A positional argument becomes the `NamedInput` of the parameter it
  binds to, keeping its own span.
- **Then one `Output` per supplied output assignment**, in parameter
  declaration order, keeping its `not`.
- **That order is the evaluation order.** For a function it is the order the
  positional rewrite produces today. For a function block it replaces source
  order, which changes a call whose named arguments have side effects and are
  written out of declaration order (row 6).
- **An omitted parameter is absent.** For a function block input that means
  it keeps its previous value, and for an output that the call does not read
  it, as today. A function or method may not omit an input (P4018), so its
  normal form names every input.

No new AST type is needed: `NamedInput` and `Output` already say which
parameter, with which argument. Folds and visitors keep reaching the argument
expressions where they do today, and the conversions the implicit-conversion
pass records stay on those expressions. A call that does not bind keeps the
arguments it was written with, and the problem is reported; `CleanAnalysis`
keeps codegen from seeing it. This is the best-effort policy of the pass it
replaces.

`lowered-program.md` §4's row "Argument order and count" becomes: made in the
binding pass for every call; recorded as the call's normal form. §5's
"Named argument → positional `Arg`" stays: lowering translates the normal form
into `Vec<Arg>`, choosing each `Arg` by the parameter's section.

### One binder

`analyzer/src/call_binding.rs`, grown from `call_assignment_check::bind_inputs`
and `plan_positional_order`:

```rust
/// Binds `args` to `params`, the callee's parameters in declaration order,
/// by the rules of `kind`, and returns the arguments in normal form.
pub(crate) fn bind(
    kind: CallKind,              // Function | FunctionBlock | Method
    params: &[CallParameter],    // name, section and declared type of each
    args: Vec<ParamAssignmentKind>,
) -> Result<Vec<ParamAssignmentKind>, (Vec<ParamAssignmentKind>, Vec<Diagnostic>)>;
```

Its rules are the design doc's requirements:

- **Positional inputs** bind to `VAR_INPUT` + `VAR_IN_OUT` in declaration
  order for a function (unchanged), and to `VAR_INPUT` alone for a function
  block and a method, as `bind_inputs` and `method.rst` ("exactly as for a
  function block invocation") already say. See question 1.
- **Named inputs** bind by name to `VAR_INPUT` and `VAR_IN_OUT`; `=>` binds
  to `VAR_OUTPUT`. An extensible function's further inputs are named by
  counting on, as today.
- **Problems**, with the codes they have today: mixed named and positional
  (P4001), an unknown input (P4002 for blocks and methods, P4023 for
  functions), a positional count (P4003; P4018 for functions), an unknown
  output (P4004). New: a parameter named twice is P4024 for every kind
  (question 2); an omitted method input or `VAR_IN_OUT` is P4018 (question
  4); a negated output that is not `BOOL` (question 3).

`CallParameter` lists a callee's parameters once, in `callee_resolution`: the
declarations of a user block or method, the fields of a standard block's
type, a function's signature. The binding pass, the rules, the write collector
and the conversion pass all use `bind` or read the normal form it leaves. None
keeps a binding loop of its own.

### Which block, which method

- **A function block instance's type** is its `VarDecl::type_id`, which
  already exists. Whether the type is a standard block, and which, is a
  property of the type: `TypeAttributes::standard_block`, set for the types
  `intermediates/stdlib_function_block.rs` builds. A type without it is a user
  block, whatever its name (row 2). Lowering needs the same answer to pick the
  block it generates (ADR-0060).
- **A method call's body** is answered by one analyzer function, as #2180
  answers a variable:

  ```rust
  /// The block that declares the method `method` called on `receiver`, and
  /// the method: the receiver's block, then its EXTENDS chain (ADR-0041
  /// case 1). THIS^ starts at the enclosing block, SUPER^ at its base.
  pub fn resolve_method(&self, receiver: Receiver<'_>, method: &Id)
      -> Option<ResolvedMethod<'_>>;
  ```

  The rule, the binding pass, the conversion pass (in place of
  `Declarations::method`) and codegen call it. Codegen reaches it through
  `CleanAnalysis`, as it reaches the execution model's declarations since
  #2170. Lowering calls it when it exists, to allocate the method's `PouId`.
- **`THIS^` and `SUPER^`.** `resolve_method` answers them, with analyzer
  tests. `rule_method_call_declared` still reports them as not implemented
  (#1406); compiling them is #1998's codegen PR, which then asks
  `resolve_method` rather than resolving anything itself.

### Today's codegen

Each change is what reading the analyzer's answer takes, and goes when
lowering covers the construct:

| Site | Change | Rows |
|---|---|---|
| `compile_fb_call` | Walks the normal form: each `NamedInput` in order, then each `Output` in order, inverting a negated one. Fields are still found by name in codegen's layout tables, which `lowered-program.md` allows until lowering gives ids ("Storage found by name at first"). | 1, 6, 8 |
| `compile_setup.rs:213` | Asks the instance type's `standard_block` instead of matching its name; `resolve_fb_type` takes the variant, not a string. | 2 |
| `compile_method_call` | Asks `resolve_method`. An inherited method whose declaring block has fields reports not implemented: a derived block's layout has no storage for inherited fields (`compile_method.rs:255-266`). | 4 |
| `compile_method_call` | Walks the normal form; `param_names_in_order` and the first-empty-slot loop are deleted. | 5, 9 |
| `compile_user_method`, `compile_method_call` | A `VAR_IN_OUT` slot holds a reference and the call site passes `compile_reference_arg`, as `compile_fn.rs` does for a function (question 5). | 3 |
| `call_args.rs`, `compile_user_function_call` | Read the normal form's inputs in order, as they read the positional ones today. | — |

No codegen table changes its key, and no codegen id is added.

### Container bytes

A program that behaves correctly today compiles to the same bytes, with one
exception: a function block call whose named arguments are written out of
declaration order now stores and loads them in declaration order. Each core PR
compares bytes before and after and lists every difference.

## Prefactoring

One behaviour-preserving PR, from `main`:

**`callee_resolution` resolves an instance through the environments, and
lists a callee's parameters once.** `InstanceTypes` goes: an instance's block
is found by `SymbolEnvironment::resolve_variable` and its `type_id`, from the
`ScopeTracker` scope. `rule_function_block_invocation`,
`rule_method_call_declared` and `write_collector` move to it. `CallParameter`
replaces the four places that list a callee's inputs today
(`count_input_type` and `find` in `call_assignment_check.rs`,
`Declarations::inputs` in `declared.rs`, `function_block_input` in
`literal.rs`).

- **Signals:** four copies of one list, and a per-rule table that the
  steering file forbids and `resolve_variable` (#2193) now makes unnecessary.
- **If an existing test has to change**, the change is not
  behaviour-preserving and moves into core 1.

Not prefactored: walking `EXTENDS` in the conversion pass changes the
conversions recorded for an inherited method's arguments, so it goes in
core 1.

## Core change PRs

Four, in order. A tracking issue lists them.

### Core 1: which block, which method (rows 2, 4)

- analyzer: `TypeAttributes::standard_block`; `resolve_method` with
  `THIS^`/`SUPER^`, used by `rule_method_call_declared` and in place of
  `Declarations::method` in the conversion pass.
- codegen: `standard_block` instead of the name; `resolve_method` for method
  calls; the inherited-fields refusal.
- specs: `specs/design/call-binding.md` (new, `status: partially
  implemented`), with the callee requirements; listed in `analyzer/build.rs`
  and `codegen/build.rs`. ADR-0041 postscript (below).

### Core 2: function block calls (rows 1, 6 FB half, 7 FB half, 8)

- analyzer: `call_binding::bind`; the binding pass normalizes `FbCall`s;
  P4024 and the negated-output check. Readers: `rule_function_block_invocation`,
  `write_collector::visit_fb_call`, `literal.rs::fb_call_inputs`,
  `argument.rs::record_fb_call_inputs`, `xform_resolve_expr_types`.
- codegen: `compile_fb_call` walks the normal form.
- specs: evaluation order, positional inputs, outputs and negation, omission.
  `lowered-program.md` §4 row; REQ-LOW-lowering-069 says the inputs are
  assigned in the order of the normal form.
- docs: `pous/function-block.rst`: evaluation order, negated outputs.

### Core 3: method calls (rows 3, 5, 6 method half, 7 method half, 9)

- analyzer: the binding pass normalizes `MethodCall`s; P4018 for an omitted
  input. Readers: `rule_method_call_declared`, `write_collector::visit_method_call`,
  `literal.rs::method_arguments`, `argument.rs::record_method_arguments`.
- codegen: the normal form; `VAR_IN_OUT` by reference.
- specs: `var-in-out-parameters.md`, the Methods row of its status table.
- docs: `object-orientation/method.rst`: every input must be supplied.

### Core 4: function calls (one binder)

- analyzer: the binding pass normalizes `Function`s to the same named form,
  including standard, extensible and nested calls the conversion pass builds
  (`ADD(a, b, c)` folds into named `IN1`/`IN2` calls).
  `xform_named_to_positional_args` and `FunctionSignature::bind_inputs` are
  deleted. Readers: `rule_function_call_declared`,
  `rule_function_call_type_check`, `rule_function_call_in_out_argument`,
  `rule_string_encoding_compat`, `rule_constant_range`,
  `write_collector::visit_function`, `xform_resolve_expr_types`,
  `argument.rs`, `arithmetic.rs`, `inputs_of_one_type.rs`.
- codegen: `call_args` reads named inputs in order. Bytes unchanged: the
  order is the positional rewrite's.
- specs: `call-binding.md` `status: implemented`; `var-in-out-parameters.md`
  §Argument list and `constant-variable-inference.md:186` stop naming the
  deleted pass.

If core 4 passes roughly 1500 lines of diff, it splits into the analyzer
readers, then codegen.

## Design doc reference

`specs/design/call-binding.md` (new): one requirement per reproduction row,
and the rules above.

| Range | Area |
|---|---|
| `REQ-CB-analyzer-001`–`019` | Which block and which method: standard by type not name, `EXTENDS`, `THIS^`, `SUPER^` |
| `REQ-CB-analyzer-020`–`039` | Normal form: positional, named and mixed per kind; outputs and negation; duplicates |
| `REQ-CB-analyzer-040`–`049` | Evaluation order |
| `REQ-CB-analyzer-050`–`059` | Omission per kind |
| `REQ-CB-codegen-001`–`019` | One per reproduction row, end to end |

Each core PR adds only the requirements it implements, with their tests.
Related: ADR-0041 (case 1), ADR-0056, ADR-0058, ADR-0060,
`lowered-program.md` §4 and §5, `user-defined-function-calls-design.md`,
`function-block-infrastructure-design.md`, `var-in-out-parameters.md`.

**ADR-0041 postscript** (core 1): the analyzer resolves case 1 with
`resolve_method`, and codegen, then lowering, call it rather than resolve.
The ADR's Implementation Status says `THIS^` and `SUPER^` shipped through
method codegen. They are rejected as not implemented
(`rule_method_call_declared.rs:191`, #1406), so that sentence is corrected in
place with an `amended:` line.

## Coordination

- **Lowering** (`lowered-program.md`). This plan records what §4 assigns to
  the analyzer and leaves the passing mode and standard block bodies to
  lowering. Its codegen changes are on the route lowering deletes.
- **Variable binding (#2180).** Same pattern: one analyzer function, language
  terms on the AST. A parameter in normal form is named as its callee
  declares it, which is its `DeclRef` in the callee's scope.
- **#2201 (lowering regression tests)** pins "function block inputs are
  evaluated in the order the call writes them". Core 2 changes that to
  declaration order on purpose. If #2201 lands first, core 2 updates that
  case and says so; if not, #2201 should pin declaration order or leave it
  out.
- **Implicit conversions (#2050, #2125).** `literal.rs` and `argument.rs`
  change only where they find a call's parameters. How a conversion is chosen
  does not change, and their tests pass unchanged, except that an inherited
  method's arguments now get conversions (core 1, a new test).
- **`THIS^`/`SUPER^` (#1998).** Its codegen PR asks `resolve_method`. Its
  `EnclosingBlock` prefactor is what `resolve_method` uses for the enclosing
  block if it lands first; otherwise the outermost `FunctionBlock` scope of
  the `ScopeTracker`.

## File map

| File | Change | PR |
|---|---|---|
| `compiler/analyzer/src/callee_resolution.rs` | instance via `resolve_variable`; `CallParameter`; `resolve_method` with `THIS^`/`SUPER^` | P, C1 |
| `compiler/analyzer/src/call_binding.rs` | **New.** The binder | C2 |
| `compiler/analyzer/src/xform_bind_call_arguments.rs` | **New.** The binding pass; replaces `xform_named_to_positional_args.rs` (deleted in C4, tests moved) | C2-C4 |
| `compiler/analyzer/src/call_assignment_check.rs` | reports through the binder | P, C2 |
| `compiler/analyzer/src/type_attributes.rs`, `intermediates/stdlib_function_block.rs` | `standard_block` | C1 |
| `compiler/analyzer/src/clean_analysis.rs` | `resolve_method` for codegen | C1 |
| `compiler/analyzer/src/stages.rs` | the binding pass in place of the rewrite | C2 |
| `compiler/analyzer/src/function_environment.rs` | delete `bind_inputs` | C4 |
| `compiler/analyzer/src/write_collector.rs` | read the normal form | P, C2-C4 |
| `compiler/analyzer/src/rule_function_block_invocation.rs`, `rule_method_call_declared.rs` | instances via the environments; normal form | P, C1-C3 |
| `compiler/analyzer/src/rule_function_call_*.rs`, `rule_string_encoding_compat.rs`, `rule_constant_range.rs` | read the normal form | C4 |
| `compiler/analyzer/src/xform_resolve_expr_types.rs` | read the normal form | C2, C4 |
| `compiler/analyzer/src/xform_insert_implicit_conversions/{declared,literal,argument,arithmetic,inputs_of_one_type}.rs` | `resolve_method`; the normal form | P, C1-C4 |
| `compiler/codegen/src/compile_stmt.rs` | `compile_fb_call` walks the normal form | C2 |
| `compiler/codegen/src/compile_setup.rs`, `compile_call.rs` | `standard_block` instead of the name | C1 |
| `compiler/codegen/src/compile_method.rs` | `resolve_method`; normal form; `VAR_IN_OUT` by reference | C1, C3 |
| `compiler/codegen/src/call_args.rs`, `compile_call.rs` | named inputs in order | C4 |
| `compiler/codegen/tests/it/end_to_end_call_binding.rs` | **New.** One test per row | C1-C4 |
| `compiler/analyzer/build.rs`, `compiler/codegen/build.rs` | list `call-binding.md` | C1 |
| `compiler/problems/resources/problem-codes.csv`, `docs/reference/compiler/problems/P####.rst` | negated non-`BOOL` output, if question 3 says a new code; P4024 and P4018 pages name blocks and methods | C2, C3 |
| `specs/design/call-binding.md` | **New** | C1-C4 |
| `specs/design/lowered-program.md` | §4 row; REQ-LOW-lowering-069 | C2 |
| `specs/adrs/0041-staged-method-and-interface-dispatch.md` | postscript; Implementation Status corrected | C1 |
| `specs/design/var-in-out-parameters.md`, `constant-variable-inference.md` | status; pass names | C3, C4 |
| `docs/reference/language/pous/function-block.rst`, `object-orientation/method.rst`, `structured-text/function-call.rst` | evaluation order, negated outputs, omission | C2-C4 |

No new AST type, no VM, container or opcode change.

## Tasks

### Prefactor: callee resolution through the environments

- [ ] `callee_resolution`: instance via `resolve_variable` and `type_id`; delete `InstanceTypes`.
- [ ] `CallParameter` for a user FB, a standard FB, a method and a function; replace `count_input_type`, `find`, `Declarations::inputs`, `function_block_input`.
- [ ] Both rules and `write_collector` use it; every existing test unchanged.

### Core 1: which block, which method

- [ ] `standard_block` on the standard FB types; test that each of the 22 has it and that a user block named `CTU_INT` does not.
- [ ] `resolve_method`, including `THIS^`/`SUPER^`; the rule and the conversion pass use it.
- [ ] Codegen: `standard_block` and `resolve_method`; refusal for an inherited method whose block has fields, with a test.
- [ ] Analyzer tests: own, inherited and overridden method; `THIS^.M()`; `SUPER^.M()`; user block named like a standard one.
- [ ] End to end: rows 2 and 4.
- [ ] `call-binding.md`, build lists, ADR-0041.

### Core 2: function block calls

- [ ] `bind`; the binding pass for `FbCall`; P4024; negated output per question 3.
- [ ] Switch the FB readers to the normal form.
- [ ] `compile_fb_call` walks it, inverting negated outputs.
- [ ] Analyzer tests: positional, named, mixed (P4001), omitted input and output, negated output, `VAR_IN_OUT`, duplicate, a positional standard FB call (`t(TRUE, T#1S)`).
- [ ] End to end: rows 1, 6 (FB), 7 (FB), 8, and a positional standard FB call.
- [ ] Specs and docs; `lowered-program.md` §4 and REQ-LOW-lowering-069.

### Core 3: method calls

- [ ] The binding pass for `MethodCall`; P4018 for an omitted input.
- [ ] Switch the method readers.
- [ ] Codegen: normal form; `VAR_IN_OUT` by reference.
- [ ] Analyzer tests: positional, named, mixed, omitted (P4018), `VAR_IN_OUT`, duplicate, inherited and overridden, `THIS^`/`SUPER^` normal form.
- [ ] End to end: rows 3, 6 (method), 7 (method); `check` errors for rows 5 and 9.
- [ ] Specs and docs.

### Core 4: function calls

- [ ] The binding pass for `Function`, including standard, extensible and the conversion pass's nested folds.
- [ ] Delete `xform_named_to_positional_args` and `FunctionSignature::bind_inputs`; switch every reader.
- [ ] Codegen reads named inputs in order; a call left with a positional argument is an internal error, with a test.
- [ ] Analyzer tests: positional, named (reordered), mixed, extensible named (`IN3`), `VAR_IN_OUT`, omitted (P4018).
- [ ] End to end: row 6 (function); `f(b := x, a := y)` equals `f(y, x)`.
- [ ] `call-binding.md` `status: implemented`; other specs.

### Verification (every PR)

- [ ] `cd compiler && just`, including 85% coverage; `cd specs && just`.
- [ ] **Bytes.** Compile every `.st` file in the repository with the `ironplcc`
  from before and after the change, under the default options and
  `--dialect rusty` and `--dialect iec61131-3-ed3`, as #2170 did, plus a
  throwaway program (scratchpad) with every kind of call written in and out
  of declaration order. First check that one binary compiles a file to the
  same bytes twice. Prefactor: no difference. Core: only the reproduction
  rows and FB calls written out of declaration order; the PR description
  lists them.
- [ ] Test modules under 1000 lines; BDD names.

## Risks

- **A pass that builds a call after the binding pass** must build it in
  normal form. The conversion pass's nested `ADD` fold is the one known case
  (core 4). Codegen reports a positional argument in a user function call as
  an internal error, so a missed one fails a test rather than miscompiling.
- **Core 4's blast radius.** Many readers; the split above.
- **New `check` errors** (rows 5, 7, 9, question 3). Each is a program that
  compiles wrongly, or not at all, today.
- **The analyzed tree reads differently.** A call shows in normal form, not
  as written, as ADR-0056 already accepted for conversions. Diagnostics keep
  each argument's own span.

## Decided

- **Parameters are named by `DeclRef`, not by index.** The task asked for
  parameters "by index or identity, not name". In the normal form each
  parameter is named as its callee declares it. The callee is resolved and a
  name is declared once in its scope (P4014), so the name with the callee's
  scope is the declaration's `DeclRef`, as #2180 uses. For a standard block,
  the scope is its type in the type environment. Lowering allocates its own
  ids from it (`VarId`, `FieldIdx`). Alternatives considered:
  - **A position in the callee's parameter list.** Every reader would rebuild
    the list to interpret it, and lowering allocates positional ids itself.
  - **An id on each declaration in the AST.** This is #2175's `DeclId`, which
    was closed.
  - **A side table keyed by call node.** Nodes have no stable identity, and
    the spans of calls a pass builds can coincide.
  - **A positional normal form for every call.** It cannot show an omitted
    function block input without a new AST entry, and does not say which
    parameter an argument binds to.

## Questions for the reviewer

1. **Positional order for blocks and methods.** `bind_inputs` and
   `method.rst` bind positional arguments to `VAR_INPUT` only, and the plan
   keeps that. `var-in-out-parameters.md` §Argument list says block and
   method calls "also [have] to include `VAR_IN_OUT` in the positional
   order", as functions do. The plan corrects that paragraph. Is that the
   intended rule?
2. **Duplicates.** A block or method parameter named twice passes `check`
   today (row 7). P4024, as for functions?
3. **Negated outputs.** `NOT q => x` on a non-`BOOL` output is not checked.
   Reject it with a new problem code, so nothing later decides what `NOT`
   means for an `INT`?
4. **Omitted method input.** Reuse P4018 with the method in its context, or
   a new code?
5. **Method `VAR_IN_OUT`.** Pass by reference in today's codegen (row 3
   gives 6), or report P9999 as `var-in-out-parameters.md` asks of an
   unsupported site, and leave the reference to lowering? The plan passes by
   reference: `compile_fn.rs` already does it for functions, so the change is
   small.
6. **Omitted FB input.** An absent `NamedInput` means "keeps its previous
   value". Is that enough, or should the normal form mark it?

## Out of scope

- Runtime dispatch through a reference or an interface (ADR-0041 case 2,
  ADR-0063).
- Default values for omitted function and method inputs.
- Passing modes and standard block bodies: lowering's, under ADR-0058 and
  ADR-0060.
- Member access outside a call (`inst.x`), including writes to an FB's
  internal `VAR`.
- Variable references in general (#2180).
- Function block `VAR_IN_OUT` in codegen and P4061; storage for inherited
  fields; function `=>` outputs (P9999 at `rule_function_call_type_check.rs:317`);
  method `VAR_OUTPUT` storage (P9999 at `compile.rs:1251`).
