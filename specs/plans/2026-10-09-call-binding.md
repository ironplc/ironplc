# Plan: Resolve Call Targets and Argument Binding in the Analyzer

Checked against `main` at `4480575`; line numbers are as of that commit. The
reproductions below were rerun there (the task cites `c150fb1`, and none of
them changed in between).

## Goal

Codegen lowers calls; it does not decide what they mean. Today it still
decides, with lookups of its own:

- **what a call reaches**: which function, which function block type
  (standard or user-defined), which method body;
- **how each argument binds**: by position or by name, by value or by
  reference;
- **what an omitted input does**;
- **the order arguments are evaluated in**.

After this work the analyzer decides all four and records the answer on the
call node, the way it records `expr_type` and `ImplicitConversion`
(ADR-0056). Codegen lowers exactly the recorded callee and binding, so a
second backend cannot decide differently.

Two constraints hold at the end:

- **Codegen never resolves a callee by name.** The analyzer records an
  identity: a function's signature, a function block's `TypeId`, a method's
  declaring block and position. `resolve_fb_type`'s name table is deleted.
- **Codegen never binds an argument.** Every `Function`, `FbCall` and
  `MethodCall` carries one entry per parameter, in evaluation order.

## Current behaviour

### What codegen decides

| Site | What it decides |
|---|---|
| `compile_call.rs:53` `compile_function_call` | The intrinsic table by name (`ctx.intrinsics`, built by `intrinsics_by_name`, `:42`), then the user function by lowercase name. `string_width.rs:239,250` and `compile_aggregate.rs:132` repeat the lookups. |
| `compile_setup.rs:120` | An FB instance's type, by `resolve_fb_type(uppercase name)` (`compile_call.rs:746`) **before** the user FB types. The table maps `CTU_INT`, `CTD_INT` and `CTUD_INT`, which the analyzer does not treat as standard names (`stdlib_function_block.rs:343`), to the built-in blocks. |
| `compile_stmt.rs:162` `compile_fb_call` | Stores `NamedInput`s only, so positional inputs are dropped. Evaluates them in source order. Looks fields up by lowercase name. Leaves an omitted input unchanged. Reads outputs in source order and ignores `Output::not`. |
| `compile_method.rs:290-309` | Finds the instance's FB by scanning `user_fb_types` for its VM type id, then the method by lowercase name on that block only, so an inherited method is P9999. |
| `compile_method.rs:328-352` | A positional argument goes to the first empty slot among **all** input-compatible parameters, a named one by name, evaluation in parameter order, P9999 for an omitted input. |
| `compile_method.rs:126-140` | A `VAR_IN_OUT` parameter gets a value slot, so it is passed by value. |
| `compile_fb_init.rs:35` | A field's operation type, by scanning `user_fb_types` for a VM type id and the field by name. |

### The analyzer's binders

The analyzer binds arguments in five places, by three different rules:

| Binder | Calls | Positional rule |
|---|---|---|
| `xform_named_to_positional_args.rs:76` `plan_positional_order` | `Function` | rewrites named arguments into the order of `VAR_INPUT` + `VAR_IN_OUT` |
| `function_environment.rs:138` `FunctionSignature::bind_inputs` | `Function` (5 rules) | zips positional arguments with `VAR_INPUT` + `VAR_IN_OUT` |
| `call_assignment_check.rs:72` `bind_inputs` | `FbCall`, `MethodCall` (2 rules, `write_collector.rs:295`) | `VAR_INPUT` only |
| `write_collector.rs:400-411` | `Function` | its own zip |
| `xform_insert_implicit_conversions/literal.rs:289` `method_arguments`, `:457` `fb_call_inputs` | `MethodCall`, `FbCall` | a position counter over `VAR_INPUT` + `VAR_IN_OUT` for methods, `VAR_INPUT` for blocks |

Callee resolution is in `callee_resolution.rs` (`FunctionBlocks::resolve_method`
walks `EXTENDS`), but the conversion pass looks methods up in a table of its
own (`xform_insert_implicit_conversions/declared.rs`, `Declarations::method`)
that does not walk `EXTENDS`. `InstanceTypes` is a per-rule table of
instances, which [Name and Type
Lookup](../steering/compiler-architecture.md#name-and-type-lookup) says a
rule should not build.

### Reproductions

`cargo build -p ironplc-cli -p ironplc-vm-cli`; `--dialect iec61131-3-ed3`
for methods; `ironplcvm run --scans 1 --dump-vars=-`. Every program passes
`check`.

| # | Program | Today | Expected |
|---|---|---|---|
| 1 | FB `a, b : DINT`, `q := a * 10 + b`; `inst(1, 2)` | `q` = 0 | 12 |
| 2 | User `FUNCTION_BLOCK CTU_INT` setting `CV := 42`; `c(CU := TRUE)` | `CV` = 1 (built-in `CTU` ran) | 42 |
| 3 | Method `Inc`, `VAR_IN_OUT v`, `v := v + 1`; `inst.Inc(v := counter)`, `counter = 5` | `counter` = 5 | 6 |
| 4 | `derived EXTENDS base`, `Get` declared in `base`; `d.Get()` | P9999 at `compile_method.rs:309` | 7 |
| 5 | Method `M(a; b := 7)`; `inst.M(a := 1)` | P9999 at `compile_method.rs:349` | a `check` error (P4018) |
| 6 | `inst(b := c1.Next(), a := c1.Next())` and `f(b := c2.Next(), a := c2.Next())`, both `a * 10 + b` | FB 21, function 12 | both 12 |

Found while reproducing; also in scope, because a complete binding has to
decide them:

| # | Program | Today | After |
|---|---|---|---|
| 7 | `inst(a := 1, a := 3, b := 2)` on an FB; the same on a method | passes `check`; last write wins (32) | P4024, as for a function |
| 8 | `inst(NOT z => nz)`, `z` = TRUE | `nz` = TRUE | FALSE |
| 9 | Method `M` with `VAR_IN_OUT x` then `VAR_INPUT a`; `inst.M(5)` | passes `check` (the rule counts `5` against `VAR_INPUT` alone); P9999 in codegen, which binds it to `x` and leaves `a` unbound | P4018: `x` is not bound |

## Architecture

### What is recorded

New types in `compiler/dsl/src/call_binding.rs`. They are identities and
indices only, `#[recurse(ignore)]` on the call nodes like `expr_type`, so the
derived folds move them through unchanged (`dsl_macro_derive/src/lib.rs:581`)
and no visitor descends into them.

```rust
/// The identity of a function signature within one compilation. Only the
/// analyzer's `FunctionEnvironment` allocates one, as only the
/// `TypeEnvironment` allocates a `TypeId`.
pub struct SignatureId(u32);

/// What a call reaches, as the analyzer resolved it.
pub enum Callee {
    /// A standard or user-defined function.
    Function(SignatureId),
    /// The function block the instance is declared as: the instance
    /// `VarDecl`'s `type_id`.
    FunctionBlock(TypeId),
    /// The method `method` (its position in the declaring block's `methods`)
    /// of the block `block` that declares it, found through `EXTENDS`.
    Method { block: TypeId, method: u16 },
}

/// One parameter of the callee and what the call does with it.
pub struct BoundParameter {
    /// The parameter's position in the callee's parameter list (below).
    pub param: u16,
    pub argument: BoundArgument,
}

pub enum BoundArgument {
    /// The input at `params[arg]` of the call, passed by value.
    Value { arg: u16 },
    /// The input at `params[arg]`, passed by reference (`VAR_IN_OUT`).
    Reference { arg: u16 },
    /// An omitted function block input: it keeps its previous value.
    Unchanged,
    /// The output assignment at `params[arg]`: its target and its `not`.
    Output { arg: u16 },
    /// An output the call does not read.
    NotRead,
    /// An omitted function block `VAR_IN_OUT`. Codegen does not compile a
    /// function block `VAR_IN_OUT` yet (P9999 today and after); P4061 is not
    /// implemented (`var-in-out-parameters.md`).
    Unbound,
}

pub struct CallBinding {
    pub callee: Callee,
    /// One entry per parameter, in the order the arguments are evaluated.
    pub parameters: Vec<BoundParameter>,
}
```

`Function`, `FbCall` and `MethodCall` each get `pub binding: Option<CallBinding>`
(`None` from the parser). The arguments stay where the program wrote them, in
`params`/`param_assignment`; the binding points at them by index. So every
fold and visitor keeps reaching the argument expressions where it does today,
and the conversions the implicit-conversion pass records stay on the
expressions the binding points at. (Moving the expressions into the binding
is the alternative; see question 5.)

**The parameter list** of a callee is defined once, in the analyzer:

| Callee | Parameters, in order |
|---|---|
| User FB | its `VAR_INPUT`, `VAR_IN_OUT` and `VAR_OUTPUT` declarations, in declaration order |
| Standard FB | its `Input` and `Output` fields, in the order of `SemanticType::FunctionBlock::fields` |
| Method | its `VAR_INPUT`, `VAR_IN_OUT` and `VAR_OUTPUT` declarations, in declaration order |
| Function | `FunctionSignature::parameters`, continued past the declared ones for an extensible function as `input_parameters` does |

`param` indexes that list. Codegen maps it to a field index or a parameter
slot from tables it builds when it lays out the callee, not by name.

### Where it is decided

**One binder**, `analyzer/src/call_binding.rs`, grown from
`call_assignment_check::bind_inputs` and `plan_positional_order`:

```rust
/// Binds `args` to `params`, the callee's parameter list, by the rules of
/// `kind`. Returns the problems the call has instead when it cannot be bound.
pub(crate) fn bind(
    kind: CallKind,                 // Function | FunctionBlock | Method
    params: &[CallParameter],       // the list above, with each section
    args: &[ParamAssignmentKind],
) -> Result<Vec<BoundParameter>, Vec<BindProblem>>;
```

The rules it applies, which the design doc states as requirements:

- **Positional inputs** bind to `VAR_INPUT` + `VAR_IN_OUT` in declaration
  order for a function (unchanged); to `VAR_INPUT` alone for a function block
  and a method, as `bind_inputs` and `method.rst` ("exactly as for a function
  block invocation") already say. See question 1.
- **Named inputs** bind by name to `VAR_INPUT` and `VAR_IN_OUT`; `=>` to
  `VAR_OUTPUT`.
- **Problems**, each reported where it is today: mixed named and positional
  (P4001), an unknown input name (P4002 for blocks and methods, P4023 for
  functions), a positional count (P4003; P4018 for functions), an unknown
  output (P4004), and a parameter named twice (P4024, now for every kind;
  question 2).
- **Omission**: a function block input is `Unchanged`, a function block
  `VAR_IN_OUT` `Unbound`, an unread output `NotRead`. A function or method
  input or `VAR_IN_OUT` is P4018 (question 4).
- **Passing mode**: `Reference` for a `VAR_IN_OUT`, `Value` otherwise.
- **Evaluation order**: the parameter list's order. Inputs come in
  declaration order, then (after the call) outputs in declaration order. For
  a function this is the order the positional rewrite produces today. For a
  function block it replaces source order, which changes a call whose named
  arguments have side effects and are written out of declaration order
  (row 6).

**One callee resolution**, `callee_resolution.rs`: a function by
`FunctionEnvironment`, an instance by the symbol environment (prefactor 2), a
method through `EXTENDS` (`resolve_method`), `THIS^` from the enclosing block
and `SUPER^` from its base.

**One recording pass**, `xform_bind_calls`, replaces
`xform_named_to_positional_args` at the same place in `stages::resolve_types`
(after the environments are built, before expression types): it records
`binding` on every call whose callee resolves and whose arguments bind. It
does not rewrite arguments. A call it cannot bind is left without a binding
and the rules report why; `CleanAnalysis` keeps codegen from seeing it.
Best effort, per call, like the pass it replaces.

The rules, the write collector and the implicit-conversion pass read the
recorded binding, or call `bind` when they run on a tree the pass has not
seen. None keeps a binding loop of its own.

### Standard function block identity

A standard block's identity is a property of its type, not of its spelling.
`TypeAttributes` gets `standard_block: Option<StandardFunctionBlock>`, set by
`intermediates/stdlib_function_block.rs` for the 22 types it builds:

```rust
pub enum StandardFunctionBlock {
    Ton, Tof, Tp, Sr, Rs, RTrig, FTrig,
    Ctu(CounterWidth), Ctd(CounterWidth), Ctud(CounterWidth),
}
```

Codegen gets an instance's `TypeId` from its `VarDecl::type_id`, asks the
type environment for `standard_block`, and maps the variant to the VM block
in a new `codegen/src/standard_fb.rs` (VM type id and field count; every
counter width runs the 32-bit block, as today). A field's index is its
parameter's position in the type's field list, which matches the VM's field
order for every standard block (checked by a test, not assumed). A type
without `standard_block` is a user block, found in `user_fb_types` by
`TypeId`. So a user block named `CTU_INT` is a user block (row 2).

### Codegen

- **Function calls**: `binding.callee` → `FunctionEnvironment::get_by_id` →
  the intrinsic, or the user function in `user_functions`, keyed by
  `SignatureId`. `intrinsics_by_name` and `ctx.intrinsics` go away, and so do
  the name lookups in `string_width.rs` and `compile_aggregate.rs`.
  `call_args::collect_positional_args`/`fixed_args` read the binding's input
  arguments in order.
- **FB calls**: walk `binding.parameters`. `Value` compiles the argument and
  stores the field; `Unchanged` emits nothing; `Reference`/`Unbound` report
  not implemented, as a function block `VAR_IN_OUT` does today; after the
  call, `Output` loads the field, applies `NOT` when the assignment is
  negated, and stores the target.
- **Method calls**: lower `Callee::Method { block, method }`. The method's
  body, field region and parameter slots are the declaring block's. A
  `VAR_IN_OUT` slot holds a reference, as in `compile_fn.rs:125-134`, and the
  call site passes `compile_reference_arg`. An inherited method whose
  declaring block has fields reports not implemented: a derived block's
  layout has no storage for inherited fields yet (`compile_method.rs:261-268`
  says so today), so lowering the call would copy the wrong slots.
- **Missing binding**: a call node with `binding: None` is an internal error
  (`Diagnostic::internal_error_at`), never a name lookup.
- **Names left on call paths**: none for a callee, field or parameter.
  Variable references (an argument variable, an output target, the instance
  variable itself) are still bound by name; that is out of scope.

### Container bytes

A program that behaves correctly today compiles to the same bytes, with one
exception: a function block call whose named inputs or outputs are written
out of declaration order now stores and loads them in declaration order (the
evaluation-order rule). Each core PR compares bytes before and after (see
Verification) and lists every difference.

## Prefactoring

Two behaviour-preserving PRs, each from `main`:

1. **Codegen keys user function blocks by identity.** `user_fb_types` becomes
   `HashMap<TypeId, UserFbTypeInfo>` (the id from the type environment, read
   once while laying out the block), `FbInstanceInfo` carries the instance's
   `TypeId` from `decl.type_id`, and `UserFbTypeInfo::methods` becomes a `Vec`
   in declaration order. The three scans by VM type id
   (`compile_stmt.rs:198`, `compile_method.rs:300`, `compile_fb_init.rs:42`)
   become lookups. Standard blocks still resolve through `resolve_fb_type`,
   and a method is still found by name, so the bytes are unchanged. Signal:
   the same scan written three times, and core PR 1 needs the `TypeId` key.

2. **`callee_resolution` resolves an instance through the environments.**
   `InstanceTypes` goes; an instance's block is found by
   `SymbolEnvironment::find` and its `type_id`, from the `ScopeTracker` scope,
   as the steering file asks. Its two rules and the write collector move to
   it. The callee's parameter list (`CallParameter`, above) moves here too,
   from the three places that list a callee's inputs today
   (`count_input_type` and `find` in `call_assignment_check.rs`,
   `Declarations::inputs` in `declared.rs`, `function_block_input` in
   `literal.rs`). Signal: three copies of one list, and a table the steering
   file forbids. If any existing test has to change, the change is not
   behaviour-preserving and moves into core PR 1.

Not prefactored, to keep the prefactors behaviour-preserving: walking
`EXTENDS` in the conversion pass's method lookup (it changes the conversions
recorded for an inherited method's arguments; core PR 1) and the dsl types
(an unused type is speculative; core PR 1 and 2 add them when they are used).

## Core change PRs

Four, in this order, each stacked on nothing but `main` and the prefactors.
A tracking issue lists them.

### Core 1: callee identity (rows 2, 4)

- dsl: `SignatureId`, `Callee`, and `binding` on the three call nodes, with
  `CallBinding::parameters` empty until core 2-4 fill it. (If a reviewer
  prefers, core 1 adds a `callee: Option<Callee>` field and core 2 folds it
  into `binding`; question 5.)
- analyzer: `FunctionEnvironment` allocates a `SignatureId` per signature
  (`get_by_id`); `TypeAttributes::standard_block`; `xform_bind_calls`
  records the callee of every call, including `THIS^` and `SUPER^`
  receivers. The conversion pass reads the recorded method callee instead of
  `Declarations::method`, so an inherited method's arguments are converted
  (this is the conversion pass's binder changing, not how it chooses a
  conversion).
- codegen: function calls by `SignatureId`; FB instance types by `TypeId` and
  `standard_block`; `resolve_fb_type` and its field maps deleted for
  `standard_fb.rs`; methods by the recorded block and position.
- docs and specs: `specs/design/call-binding.md` (new, `status: partially
  implemented`) with the callee requirements; listed in `analyzer/build.rs`
  and `codegen/build.rs`. ADR-0041 postscript. `docs/` pages for function
  blocks and methods if they name the standard counter variants.

### Core 2: function block call binding (rows 1, 6 FB half, 7 FB half, 8)

- analyzer: `call_binding::bind` and `BindProblem`; `check_assignments`
  reports the binder's problems; `xform_bind_calls` records the binding of
  every `FbCall`. Readers switched: `rule_function_block_invocation`,
  `write_collector::visit_fb_call`, `literal.rs::fb_call_inputs`,
  `argument.rs::record_fb_call_inputs`, `xform_resolve_expr_types.rs:784`.
  P4024 for a duplicated FB parameter. A negated non-`BOOL` output is
  rejected (question 3).
- codegen: `compile_fb_call` lowers the binding.
- spec: evaluation order, positional FB inputs, outputs and negation,
  omission (`Unchanged`, `NotRead`, `Unbound`), passing mode.
- docs: `docs/reference/language/pous/function-block.rst` states the
  evaluation order and that a negated output is inverted.

### Core 3: method call binding (rows 3, 5, 6 method half, 7 method half, 9)

- analyzer: `xform_bind_calls` records method bindings with the same binder.
  An omitted method input or `VAR_IN_OUT` is P4018. Readers switched:
  `rule_method_call_declared`, `write_collector::visit_method_call`,
  `literal.rs::method_arguments`, `argument.rs::record_method_arguments`.
  The rule still reports `THIS^`/`SUPER^` receivers as not implemented.
- codegen: the method's `VAR_IN_OUT` slots hold references; the call site
  walks the binding; `param_names_in_order` and the first-empty-slot loop are
  deleted.
- specs: `var-in-out-parameters.md`, Methods row of Implementation Status.
- docs: `method.rst` states that every input must be supplied (P4018) and
  that `VAR_IN_OUT` is by reference.

### Core 4: function call binding (row 6 function half; one binder)

- analyzer: `xform_bind_calls` records function bindings, standard and
  user-defined, including extensible ones and the nested calls the
  conversion pass builds for `ADD(a, b, c)`.
  `xform_named_to_positional_args` and `FunctionSignature::bind_inputs` are
  deleted; their readers (`rule_function_call_declared`,
  `rule_function_call_type_check`, `rule_function_call_in_out_argument`,
  `rule_string_encoding_compat`, `rule_constant_range`,
  `write_collector::visit_function`, `xform_resolve_expr_types.rs:127,276,471`,
  `argument.rs::record_argument_conversions`, `arithmetic.rs`,
  `inputs_of_one_type.rs`) read the binding.
- codegen: `compile_user_function_call` and `call_args` read the binding.
- specs: `call-binding.md` `status: implemented`;
  `var-in-out-parameters.md` §Argument list and
  `constant-variable-inference.md:186` no longer name the deleted pass.

Core 4 is the largest by file count, but each reader changes from "zip the
positional arguments" to "walk the binding". If its diff passes roughly 1500
lines, it splits into analyzer readers, then codegen.

## Design doc reference

`specs/design/call-binding.md` (new). One requirement per row of the
reproduction table, and the rules above. Numbering sketch:

| Range | Area |
|---|---|
| `REQ-CB-analyzer-001`–`019` | Callee identity: function signature, FB type by `type_id`, standard block by type not name, method through `EXTENDS`, `THIS^`, `SUPER^` |
| `REQ-CB-analyzer-020`–`039` | Binding: one entry per parameter, positional/named/mixed per kind, passing mode, outputs and negation, duplicates |
| `REQ-CB-analyzer-040`–`049` | Evaluation order |
| `REQ-CB-analyzer-050`–`059` | Omission per kind |
| `REQ-CB-codegen-001`–`019` | Codegen lowers the recorded callee; missing binding is an internal error |
| `REQ-CB-codegen-020`–`039` | One per reproduction row, end to end |

Each core PR adds only the requirements it implements, with their tests,
since the build enforces both directions. Related:
[ADR-0041](../adrs/0041-staged-method-and-interface-dispatch.md) (this is its
case 1),
[ADR-0056](../adrs/0056-analyzer-records-implicit-conversions-in-the-ast.md)
(the principle), `user-defined-function-calls-design.md`,
`function-block-infrastructure-design.md`, `var-in-out-parameters.md`.

**ADR-0041 postscript** (core 1): the analyzer resolves case 1 statically
and records the declaring block and method on the call; codegen lowers the
recorded method and resolves nothing. The ADR's Implementation Status says
`THIS^` and `SUPER^` shipped through method codegen; they are rejected as
not implemented (`rule_method_call_declared.rs:191`, #1406). That is a
defect in the record, so it is corrected in place with an `amended:` line.

## Coordination

- **Implicit conversions (#2133).** `literal.rs` and `argument.rs` change
  only where they find a call's parameters: they read the binding instead of
  counting positions. How a conversion is chosen does not change, and the
  `ImplicitConversion` nodes stay on the argument expressions, which do not
  move. Their tests (`xform_insert_implicit_conversions/tests.rs`,
  `stored_tests.rs`) must pass unchanged, except where an inherited method's
  arguments now get conversions (core 1), which is a new test.
- **Initial values.** No dependency: omitted function and method inputs are
  an error, not a default.
- **`THIS^`/`SUPER^` (plan PR #1998).** That plan's codegen PR lowers
  `THIS^.M()`/`SUPER^.M()`. This work records their callee (analyzer tests
  only) and leaves the not-implemented diagnostic in place, so #1998 lowers
  the recorded method rather than resolving it. The recording needs the
  enclosing block: it uses #1998's `EnclosingBlock` if that prefactor has
  landed, and otherwise the outermost `FunctionBlock` scope of the
  `ScopeTracker`.
- **Execution model.** No overlap.

## File map

| File | Change | PR |
|---|---|---|
| `compiler/codegen/src/compile.rs` | `user_fb_types` by `TypeId`; `FbInstanceInfo` gets `TypeId`; methods `Vec`; drop `intrinsics`; `user_functions` by `SignatureId` | P1, C1 |
| `compiler/codegen/src/compile_setup.rs` | instance type from `decl.type_id` and `standard_block` | P1, C1 |
| `compiler/codegen/src/compile_stmt.rs` | `compile_fb_call` lowers the binding | P1, C2 |
| `compiler/codegen/src/compile_method.rs` | recorded method; binding; `VAR_IN_OUT` by reference | P1, C1, C3 |
| `compiler/codegen/src/compile_fb_init.rs` | field op type by `TypeId` | P1 |
| `compiler/codegen/src/compile_call.rs` | callee by `SignatureId`; delete `resolve_fb_type`, field maps, `intrinsics_by_name` | C1, C4 |
| `compiler/codegen/src/standard_fb.rs` | **New.** `StandardFunctionBlock` → VM block | C1 |
| `compiler/codegen/src/string_width.rs`, `compile_aggregate.rs` | callee by `SignatureId` | C1 |
| `compiler/codegen/src/call_args.rs` | read the binding | C4 |
| `compiler/codegen/tests/it/end_to_end_call_binding.rs` | **New.** One test per reproduction row | C1-C4 |
| `compiler/dsl/src/call_binding.rs`, `lib.rs`, `textual.rs` | **New** types; `binding` field on three nodes; constructors updated (about 35 sites, mechanical) | C1, C2 |
| `compiler/analyzer/src/callee_resolution.rs` | instance via environments; `CallParameter` list; `THIS^`/`SUPER^` | P2, C1 |
| `compiler/analyzer/src/call_binding.rs` | **New.** The binder | C2 |
| `compiler/analyzer/src/call_assignment_check.rs` | reports the binder's problems | C2 |
| `compiler/analyzer/src/xform_bind_calls.rs` | **New.** Recording pass, replaces `xform_named_to_positional_args.rs` | C1-C4 |
| `compiler/analyzer/src/xform_named_to_positional_args.rs` | **Deleted** (tests move to `call_binding` and `xform_bind_calls`) | C4 |
| `compiler/analyzer/src/function_environment.rs` | `SignatureId`, `get_by_id`; delete `bind_inputs` | C1, C4 |
| `compiler/analyzer/src/type_attributes.rs`, `intermediates/stdlib_function_block.rs` | `standard_block` | C1 |
| `compiler/analyzer/src/stages.rs` | `xform_bind_calls` in place of the rewrite | C1 |
| `compiler/analyzer/src/write_collector.rs` | read the binding | P2, C2-C4 |
| `compiler/analyzer/src/rule_function_block_invocation.rs`, `rule_method_call_declared.rs` | instance via environments; binder problems | P2, C2, C3 |
| `compiler/analyzer/src/rule_function_call_*.rs`, `rule_string_encoding_compat.rs`, `rule_constant_range.rs` | read the binding | C4 |
| `compiler/analyzer/src/xform_resolve_expr_types.rs` | read the binding | C2, C4 |
| `compiler/analyzer/src/xform_insert_implicit_conversions/{declared,literal,argument,arithmetic,inputs_of_one_type}.rs` | read the recorded callee and binding | C1-C4 |
| `compiler/analyzer/build.rs`, `compiler/codegen/build.rs` | list `call-binding.md` | C1 |
| `compiler/problems/resources/problem-codes.csv`, `docs/reference/compiler/problems/P####.rst` | negated non-`BOOL` output, if question 3 says a new code; P4024 and P4018 pages name blocks and methods | C2, C3 |
| `specs/design/call-binding.md` | **New** | C1-C4 |
| `specs/adrs/0041-staged-method-and-interface-dispatch.md` | postscript; Implementation Status corrected | C1 |
| `specs/design/var-in-out-parameters.md`, `constant-variable-inference.md` | status and pass names | C3, C4 |
| `docs/reference/language/pous/function-block.rst`, `object-orientation/method.rst`, `structured-text/function-call.rst` | evaluation order, negated outputs, omission | C2-C4 |

No VM, container or opcode change.

## Tasks

### Prefactor 1: codegen keys user function blocks by identity

- [ ] Build `ironplcc` on clean `main`; keep it as the "before" binary.
- [ ] `user_fb_types: HashMap<TypeId, UserFbTypeInfo>`; `FbInstanceInfo::fb_type: TypeId`; `UserFbTypeInfo::methods: Vec<UserMethodInfo>`.
- [ ] Replace the three scans by VM type id with lookups.
- [ ] `compiler/codegen/tests/it` unchanged; bytes identical (Verification).

### Prefactor 2: callee resolution through the environments

- [ ] `callee_resolution`: instance lookup via `SymbolEnvironment::find` and `type_id`; delete `InstanceTypes`.
- [ ] `CallParameter` list for a user FB, a standard FB, a method and a function; replace `count_input_type`, `find`, `Declarations::inputs`, `function_block_input`.
- [ ] Move both rules and `write_collector` to it; every existing test unchanged.

### Core 1: callee identity

- [ ] dsl types and fields; constructors.
- [ ] `SignatureId` in `FunctionEnvironment`; `standard_block` in the type environment.
- [ ] `xform_bind_calls` records callees, including `THIS^`/`SUPER^`.
- [ ] Conversion pass reads the recorded method callee.
- [ ] Codegen lowers recorded callees; delete `resolve_fb_type`, `intrinsics_by_name`; add `standard_fb.rs` with a test that its field order matches each standard type's field list.
- [ ] Inherited method with fields in the declaring block: not implemented, with a test.
- [ ] Analyzer tests: user function, standard function, user FB, standard FB, user FB named `CTU_INT`, own method, inherited method, overridden method, `THIS^.M()`, `SUPER^.M()`.
- [ ] End to end: rows 2 and 4.
- [ ] `call-binding.md` callee section; build lists; ADR-0041 postscript.

### Core 2: function block call binding

- [ ] `call_binding::bind`; `BindProblem`; `check_assignments` on top of it.
- [ ] Record FB bindings; switch the FB readers.
- [ ] P4024 for a duplicated FB parameter; negated non-`BOOL` output per question 3.
- [ ] `compile_fb_call` lowers the binding, including negation.
- [ ] Analyzer tests: positional, named, mixed (P4001), omitted input (`Unchanged`), omitted output (`NotRead`), outputs including negated, `VAR_IN_OUT` (`Reference`, `Unbound`), duplicate, standard FB positional (`t(TRUE, T#1S)`).
- [ ] End to end: rows 1, 6 (FB), 7 (FB), 8; a positional standard FB call.
- [ ] Spec and docs.

### Core 3: method call binding

- [ ] Record method bindings; switch the method readers; P4018 for an omitted input.
- [ ] Method `VAR_IN_OUT` slots as references; call site by reference.
- [ ] Analyzer tests: positional, named, mixed, omitted (P4018), `VAR_IN_OUT`, outputs, duplicate, inherited and overridden, `THIS^`/`SUPER^` bindings.
- [ ] End to end: rows 3, 6 (method), 7 (method); `check` errors for rows 5 and 9.
- [ ] Spec and docs.

### Core 4: function call binding

- [ ] Record function bindings (standard, user, extensible, nested folds).
- [ ] Delete `xform_named_to_positional_args` and `FunctionSignature::bind_inputs`; switch every reader.
- [ ] Codegen reads the binding; a call without one is an internal error, with a test.
- [ ] Analyzer tests: positional, named (reordered), mixed, extensible named (`IN3`), `VAR_IN_OUT` (`Reference`), omitted (P4018).
- [ ] End to end: row 6 (function); `f(b := x, a := y)` against `f(y, x)`.
- [ ] `call-binding.md` `status: implemented`; other specs.

### Verification (every PR)

- [ ] `cd compiler && just` passes, including 85% coverage.
- [ ] `cd specs && just` passes; nothing outside `specs/plans/` names this plan.
- [ ] **Bytes.** Compile with the "before" and "after" `ironplcc` and compare
  the `.iplc` files: `compiler/resources/test/*.st`, the playground examples,
  and a throwaway program (scratchpad, not committed) with every kind of
  call, each written in and out of declaration order. First confirm that two
  compiles with one binary are identical. Prefactors: no differences. Core:
  the differences are only the reproduction rows and FB calls written out of
  declaration order, and the PR description lists them.
- [ ] Test modules under 1000 lines; BDD names.

## Risks

- **Synthetic calls.** A pass after `xform_bind_calls` that builds a
  `Function` (the conversion pass's nested `ADD` fold) must bind it, or
  codegen reports an internal error. Core 4 lists every constructor of a
  `Function` after the pass and binds each; the end-to-end suite covers them.
- **Indices go stale.** The binding indexes `params`; a later pass that
  reorders or drops arguments would break it. No pass does today. A debug
  assertion in codegen checks that each index points at an argument of the
  recorded kind.
- **Blast radius of core 4.** Many readers; mitigated by the split above.
- **`compile.rs` is already 2150 lines.** These changes remove more than
  they add there; nothing new goes into it.
- **New `check` errors** (rows 5, 7, 9; and question 3). Each is a program
  that compiled wrongly or not at all today.

## Questions for the reviewer

1. **Positional order for blocks and methods.** The task, `bind_inputs`
   and `method.rst` bind positional block and method arguments to
   `VAR_INPUT` only. `var-in-out-parameters.md` §Argument list says block and
   method calls "also [have] to include `VAR_IN_OUT` in the positional
   order", as functions do. The plan follows the task and corrects that
   paragraph. Is that the intended rule?
2. **Duplicates.** A block or method parameter named twice passes `check`
   today (row 7). The plan makes it P4024, as for functions. Acceptable as
   a new error?
3. **Negated outputs.** `NOT q => x` on a non-`BOOL` output is not checked
   today. The plan rejects it with a new problem code, so codegen never
   decides what `NOT` means for an `INT`. Or should it be bitwise, recorded
   by the analyzer?
4. **Omitted method input.** Reuse P4018 ("wrong number of arguments") with
   the method in its context, or a new code?
5. **Binding shape.** Indices into the arguments as written (folds, visitors
   and conversions untouched), or move the expressions into the binding?
   And one `binding` field from core 1, or a `callee` field in core 1 that
   core 2 folds in?
6. **Standard block identity**: a field on `TypeAttributes`, or on
   `SemanticType::FunctionBlock` (more match sites)?
7. **`THIS^`/`SUPER^`**: is recording the callee here, with lowering left to
   #1998, the split you want?

## Out of scope

- Runtime dispatch through a reference or an interface (ADR-0041 case 2).
- Default values for omitted function and method inputs.
- 64-bit standard counters in the VM.
- Member access outside a call (`inst.x`), including writes to an FB's
  internal `VAR`.
- Variable references in general, still bound by name.
- Function block `VAR_IN_OUT` lowering and P4061; storage for inherited
  fields; function `=>` outputs (P9999 at `rule_function_call_type_check.rs:313`
  today and after).
