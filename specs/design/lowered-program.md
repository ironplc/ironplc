# Design: Lowered Program

status: proposed
date: 2026-10-01

## Overview

This design adds one stage between semantic analysis and code generation. The
stage, **lowering**, turns an analyzed `Library` into a **lowered program**: a
tree in which every decision the language makes has already been made and every
name already refers to the thing it names. A code generator (a **backend**)
consumes the lowered program and nothing else. The lowered program is written
in IronPLC's **intermediate representation (IR)**, which this design defines
(see [Names](#names)).

Two things motivate it.

The first is that the bytecode backend re-derives what analysis already
established. It looks variables up by name, matches standard function blocks
by spelling, re-checks argument counts, and asks the analyzer's overload
resolver the same question a second time. Each of those is a place where a
state the analyzer has ruled out is still representable, so each needs an error
path, and the project's rule that every enum and `Option` variant is handled
directly cannot be met without writing arms for states that cannot occur.

The second is that a backend emitting WebAssembly is planned, and one emitting
native code through LLVM is possible. Whatever the bytecode backend decides for
itself today, every other backend would have to decide again, and two
implementations of one decision can disagree. A program that
behaves differently on two targets is a safety defect in a PLC, which is what
[ADR-0005](../adrs/0005-safety-first-design-principle.md) exists to prevent.

The analyzer and lowering divide the work by one rule. A decision whose outcome
can make a program invalid is made by the analyzer and recorded in the analyzed
tree, because analysis is where problems are reported. Lowering makes the
remaining decisions, translates the recorded ones into a form a backend can
consume without checking, and never reports that a program is invalid.

The design builds on:

- **[ADR-0005](../adrs/0005-safety-first-design-principle.md)**: safety decides
  trade-offs. A decision made once and consumed twice is safer than a decision
  made twice.
- **[ADR-0001](../adrs/0001-bytecode-integer-arithmetic-type-strategy.md)**:
  two operation widths with explicit narrowing. The lowered program makes the
  narrowing an explicit node rather than something a backend remembers to emit.
- **[ADR-0013](../adrs/0013-expression-type-annotation-via-wrapper-struct.md)**
  and **[ADR-0055](../adrs/0055-concrete-type-ids-numbered-by-debug-tag.md)**:
  expressions carry a type by identity. Lowering is the last reader of that
  annotation.
- **[ADR-0048](../adrs/0048-semantic-rules-cannot-fail.md)**: a stage that only
  produces diagnostics accumulates them. Lowering reports every problem it
  finds in one run.
- **[ADR-0042](../adrs/0042-library-functions-over-compiler-intrinsics.md)**
  and **[ADR-0008](../adrs/0008-unified-builtin-opcode.md)**: the compiler's
  intrinsic surface is the IEC 61131-3 standard functions. The lowered program
  names each one by an enum, not by its spelling.
- **[Expression Type Resolution](expression-type-resolution.md)** and
  **[Arithmetic Operator Overloads](arithmetic-operator-overloads.md)**: where
  expression types and operator results are decided today.
- **[ADR-0056](../adrs/0056-analyzer-records-implicit-conversions-in-the-ast.md)**,
  **[Implicit Conversions](implicit-conversions.md)** and
  **[Comparison Operand Type](comparison-operand-type.md)**: the analyzer
  records implicit conversions, first those of a comparison and now most
  others, as `ExprKind::ImplicitConversion` nodes, and codegen compiles what
  is recorded.
  This design extends that arrangement to every decision that can make a
  program invalid; see [Relationship to ADR-0056](#relationship-to-adr-0056).

## Problem

All figures are from the non-test source of `compiler/codegen/src` at commit
`7001b4a` (about 15,000 lines, blank lines and comments included). Three
commits have changed that source since: `784ff7f` and `0a625a0`, which record
the conversions of a comparison in the analyzer (ADR-0056), and `a5bfad8`. The
figures are not re-taken, because they show the shape of the problem rather
than a target.

Prefactors toward this design have since changed part of what the figures
describe:

- Standard functions are dispatched on an enum that the analyzer attaches to
  their signatures, not on their names.
- `collect_positional_args` is defined once.
- `SavedFbScope` has given way to a `Scope` value.
- Analysis now reports every problem only codegen found. `rule_constant_range`
  reads each literal's recorded type rather than predicting it, so codegen
  raises no `ConstantOverflow`.
- Codegen takes a clean analysis, so no caller can skip the check (see
  [The Clean-Analysis Gate](#2-the-clean-analysis-gate)).
- The analyzer records most implicit conversions and the types of untyped
  literals (ADR-0056): those of operands, assigned values, the arguments of
  functions, methods and function block calls, loop bounds, and the results
  of calls and dereferences. It types a function of several inputs, such as
  `MAX`, and the bitwise operators by the type every input widens to. The
  conversions codegen still makes itself go through one function,
  `convert_to_context`. Codegen asks the overload resolver a second time only
  for an operand pair with a typed overload, such as time arithmetic.
- Bit and partial access, and assignments to most targets that occupy one
  slot, compile through one addressing path (`Place`).

The rest of this section describes the problem as it was measured, because
that is what the design answers.

`codegen::compile` reads the analyzed `Library`. The `Library` is the same
tree the parser built, annotated in place by the analyzer. That one type
serves three consumers with incompatible needs:

| Consumer | Needs the tree to be |
|---|---|
| `plc2plc` | Faithful to the source. `Assignment::ref_bind` exists so the renderer can reproduce `REF=`. |
| Language server | Able to hold a broken program. Best-effort transforms leave a declaration they could not transform unchanged or as a placeholder. |
| Code generation | Total. Every node resolved, every type known. |

A type that can hold a broken program cannot also promise a resolved one, so
code generation checks at run time. What that costs:

| Symptom | Count | Example |
|---|---|---|
| Sites raising P9999 or P9998 | 145 | `Diagnostic::todo_with_span(func.name.span())` after an argument count check |
| `_ =>` match arms | 91 | Some are over integers, where Rust requires one |
| Probes of maps keyed by variable or POU name | 43 | `ctx.struct_vars.get(&root_name)` |
| Silent `unwrap_or(DEFAULT_OP_TYPE)` fallbacks | 14 | An unknown type compiles as a signed 32-bit integer |
| `collect_positional_args` calls, each followed by a count check | 26 | The analyzer has already enforced the count. The function itself is defined twice, identically, in `compile_call.rs` and `compile_string.rs` |
| Sites raising a user-facing problem code | 23 | 14 are `ConstantOverflow` |

The fallbacks fall into five kinds, and only the first is about missing types:

1. **Phase leftovers.** `Expr::expr_type` is an `Option`, `ExprKind::LateBound`
   is compiled as a variable read, `InitialValueAssignmentKind::LateResolvedType`
   is an internal error, and `Assignment` carries four `bool` flags of which two
   are documented as mutually exclusive.
2. **Name resolution.** `CompileContext` holds nine maps keyed by name
   (`variables`, `var_types`, `string_vars`, `fb_instances`, `array_vars`,
   `struct_vars`, `struct_array_vars`, `user_functions`, `user_fb_types`), and
   a set of names (`in_out_params`). A variable's kind is whichever map it is found in. `SavedFbScope` saves and
   restores seven of them around each function block body because bare names
   collide across scopes.
3. **Call resolution.** `compile_function_call` dispatches on the lower-case
   function name, and `lookup_builtin` matches the upper-case name again to
   pick a `func_id`.
4. **Typing decisions.** An untyped literal reaches codegen as
   `ExprType::Literal(ANY_INT)`, so an operation type is threaded top-down
   through `compile_expr`. Implicit conversions outside comparisons are chosen
   in `compile_arith.rs` and `compile_value_arg`.
5. **Shape cross products.** Assignment through bit access and partial access
   is eight functions (about 540 lines of `compile_expr.rs`), one per pair of
   access kind and base shape.

Three decisions are already implemented twice, once in the analyzer and once in
codegen, and kept in step by comments and tests:

- `rule_constant_range` pushes the expected type down to literals because
  that "is how the backend compiles them: one operation type covers both
  operands", so that it predicts what `compile_constant` will do.
- `compile_arith.rs` calls `resolve_arithmetic_overload` with
  `ctx.compiler_options`, a field documented as holding the same options the
  analyzer used.
- Standard function signatures live in the analyzer's `FunctionEnvironment`
  keyed by name; their codegen lives in `lookup_builtin` keyed by name.

A second backend would add a third copy of each. The operand type of a
comparison shows the way out: since ADR-0056 the analyzer records it on the
operands, and codegen compiles what is recorded, keeping a fallback only for a
pair the analyzer leaves without one (`compile_comparison.rs`).

Two further consequences:

- `compile` returns `Result<Container, Diagnostic>`, so a program with five
  unsupported constructs reports one per run.
- `ironplcc check` stops after analysis, so a problem found only during code
  generation is not reported by `check`.

## Design Goals

1. **One implementation of each language decision.** What a program means is
   decided once, upstream of every backend, and what each lowered operation
   does is specified once, for every backend.
2. **States that cannot occur cannot be written.** A backend's matches over
   the lowered program are exhaustive without wildcard arms, and none of the
   arms is an error path for a state analysis has ruled out.
3. **Target neutral.** Nothing in the lowered program names a slot, an offset,
   a size, an opcode or a function id of the bytecode VM.
4. **Small.** Each construct in the lowered program is a construct every
   backend must implement, so source constructs that can be expressed with
   others are.
5. **Problems are reported by analysis.** Every problem with the program as
   written is reported by analysis, where `check` and the language server show
   it beside every other problem. Lowering reports only what the compiler
   cannot generate yet and compiler defects.
6. **Analysis output can still hold a broken program.** The `Library` remains
   source faithful, apart from the decisions the analyzer records in it
   (ADR-0056), and remains able to hold a broken program.
7. **Deliverable in behaviour-preserving steps.** The existing end-to-end
   tests pass unchanged at every step.

## Scope

**In scope:** the lowered program's data model and the meaning of its
operations, the lowering stage, the division of decisions between the analyzer
and lowering, the gate between analysis and lowering, the contract a backend
works to, what becomes of the structures in `compiler/codegen` today, how
the result is tested, and what the lowered program must allow so that
sequential function charts and the graphical languages can be lowered later.

**Out of scope:**

- The WebAssembly and LLVM backends themselves. They are constraints on this
  design, not part of it.
- Compiling sequential function charts, ladder diagrams, function block
  diagrams and instruction lists. They are constraints too (see
  [Sequential and Graphical Languages](#11-sequential-and-graphical-languages)).
- Any change to the bytecode instruction set, the container format or the VM.
  Two such changes are needed to meet this design fully, and each is designed
  separately:
  - addressing function block fields in place
    ([issue 2120](https://github.com/ironplc/ironplc/issues/2120); see
    [Instance fields](#instance-fields));
  - sizing each temporary string buffer from the value it holds
    ([issue 2118](https://github.com/ironplc/ironplc/issues/2118); see
    [String capacity](#string-capacity)).
- Changing what an operation does. [Meaning of operations](#310-meaning-of-operations)
  starts from what the bytecode VM does today.
- Removing `Expr::expr_type` or `VarDecl::type_id` from the AST. The analyzer's
  rules read them.
- Any particular optimization pass. Where optimizations live, and what they
  may assume, is in scope (see [Optimization](#10-optimization)).
- A control-flow graph or any other non-tree form (see
  [Alternatives Considered](#alternatives-considered)).
- How IronPLC represents time. `TIME` and `LTIME` count milliseconds
  ([ADR-0021](../adrs/0021-time-32bit-ltime-64bit.md)) and the VM's clock
  counts microseconds; that representation will change, and this design uses
  it as it stands (see [Scalar expressions](#35-scalar-expressions)).
- Operation widths other than those of
  [ADR-0001](../adrs/0001-bytecode-integer-arithmetic-type-strategy.md) (see
  [Value classes](#33-value-classes)).
- Pointer arithmetic, which lowering reports as P9999, and sizes that follow a
  backend's layout rather than the language's storage widths.
- Checking a backend's limits in a pass of their own before emission. A
  backend reports what its target cannot do as it emits
  (REQ-LOW-codegen-113).
- Debug information beyond what the bytecode debug section holds today, such
  as stepping through expressions. A debugger shows what a backend allocated,
  so a variable an optimization removed does not appear.
- Running lowering in the language server. It reports what analysis reports,
  as it does today.
- Declarative initial values and warm restart. `init` stays a list of
  statements; both are revisited with support for `RETAIN` variables.
- Tasks that preempt each other (see
  [Meaning of operations](#310-meaning-of-operations)).
- Showing the power flow of a ladder diagram, or the value on each wire of a
  function block diagram, while the program runs.
- Reporting an error in `ENO` rather than trapping (see
  [EN and ENO](#en-and-eno)).
- Making the default string capacity a compiler option that a dialect sets.
  This design keeps it possible (see [String capacity](#string-capacity)).
- How the bytecode VM dispatches a call through an interface, which
  [pull request 1870](https://github.com/ironplc/ironplc/pull/1870) designs, and
  `__QUERYINTERFACE` and `__QUERYPOINTER`.

---

## 1. Position in the Pipeline

```
parse ──▶ analyze ──────────▶ gate ──▶ lower ──────────────┬──▶ bytecode backend:  layout ──▶ emit
          resolve, check,     clean    translate, desugar, ├──▶ WebAssembly backend (planned)
          decide and record   only     allocate ids        └──▶ LLVM backend (possible)
          (ADR-0056)                   └── `check` ends here
```

Lowering is two new crates:

- **`ironplc-ir`** is the IR: its data model, the
  constructors that check it ([Invariants by construction](#39-invariants-by-construction)),
  the `Intrinsic` enum, and the expansions a backend may call instead of
  implementing a node (REQ-LOW-ir-068). It depends only on `ironplc-dsl`, for
  ids, source spans and diagnostics, and re-exports what a backend needs from
  it.
- **`ironplc-lowering`** is the pass that builds a lowered program, a program
  in the IR, from a clean analysis.

| Crate | Depends on |
|---|---|
| `ironplc-ir` | `ironplc-dsl` |
| `ironplc-analyzer` | `ironplc-dsl`, and its dependencies today |
| `ironplc-lowering` | `ironplc-dsl`, `ironplc-analyzer`, `ironplc-ir` |
| A backend | `ironplc-ir` |

The analyzer and the IR each name a built-in function with an enum of their
own, and neither crate depends on the other:

- **The analyzer's `BuiltinFunction`** says which function a call resolved to:
  `SQRT`, `CONCAT`, `INT_TO_REAL`. Each built-in function signature carries one
  (REQ-LOW-analyzer-070).
- **The IR's `Intrinsic`** says which operation at which operand types:
  `SqrtF32` or `SqrtF64` (REQ-LOW-ir-145).

The analyzer knows a call's function as soon as it resolves the call, but the
operand types only once it has recorded them. Lowering reads both, so it maps
each call to its `Intrinsic` (REQ-LOW-lowering-147). The cost is one
exhaustive match in lowering. In exchange, the analyzer does not depend on the
IR, and its dispatch does not change when an operation gains an operand type.
No stage after the analyzer matches a function's name.

Rust lets a crate name only its direct dependencies. A backend whose manifest
lists `ironplc-ir` alone cannot reach `SemanticContext`,
`TypeEnvironment`, any resolver or the AST, however convenient that would be
at a given call site, and does not compile the analyzer at all. The lowered
program carries its own type table (see
[Program, POUs and variables](#32-program-pous-and-variables)), so a backend
needs nothing more.

**REQ-LOW-lowering-001** The lowering entry point takes a clean analysis (see
[The Clean-Analysis Gate](#2-the-clean-analysis-gate)) and returns either a
lowered program or a non-empty list of diagnostics.

**REQ-LOW-codegen-002** `ironplc-codegen` has no dependency on
`ironplc-analyzer`, `ironplc-parser`, `ironplc-lowering` or `ironplc-dsl`
outside `[dev-dependencies]`.

**REQ-LOW-codegen-003** The public entry point of `ironplc-codegen` takes a
lowered program.

**REQ-LOW-project-004** `ironplc_project` runs lowering as part of what
`ironplcc check` reports, so a construct the compiler cannot generate yet, in
a POU reachable from a program instance, is reported without generating code.

**REQ-LOW-project-005** `ironplc_project::compile` passes the lowered program
that `check` produced to the backend; it does not lower a second time.

A backend still emits debug information. Everything the bytecode debug section
holds today ([Debug Info in the IPLC Container](debug-info-in-iplc-container.md))
comes from the lowered program and from the source text, which the backend
reads through `SourceLookup` by the file id of a span, as it does now. No
backend reads the AST to build it.

**REQ-LOW-lowering-006** Every variable of the lowered program carries its
source name, its owning POU, its section, its type and the span of its
declaration; every field of a structure, function block or program type carries
its name, its section where it has one and the span of its declaration, or for
a synthesized field the span of the element it was synthesized for;
every POU carries its source name and span; and the type table carries each
type's declared name and each enumeration's value names. Those, with statement
spans, are what the variable name, function name, line map and enumeration
tables of the debug section are built from. String layouts are the backend's
own layout.

**REQ-LOW-codegen-007** The debug section the bytecode backend builds from the
lowered program names every variable, field and POU of the lowered program,
maps the span of every statement to the instructions emitted for it, and lists
every source file, string layout and enumeration definition the program uses.

A debugger shows what the backend allocated. An optimization that removes a
variable removes it from the lowered program a backend receives, or from the
backend's own output, and the debugger does not show it.

The instructions themselves change during this work (see
[Backend Contract](#7-backend-contract)), so the line map is checked by what it
covers, not compared byte for byte with the one built from the `Library`.

## 2. The Clean-Analysis Gate

Code generation already runs behind the gate. A clean analysis,
`ironplc_analyzer::CleanAnalysis`, borrows the `Library` and the
`SemanticContext` and can be constructed only when the context holds no
diagnostics. `ironplc_codegen::compile` takes one, so a caller cannot forget
the check. Every caller builds one: `ironplc_project::compile`, which also
serves the MCP server and the playground, `lsp_runner.rs`, the benchmarks, and
the tests of codegen and of the VM command line.

```rust
pub struct CleanAnalysis<'a> { library: &'a Library, context: &'a SemanticContext }

impl<'a> CleanAnalysis<'a> {
    // The error is the context's diagnostics.
    pub fn new(library: &'a Library, context: &'a SemanticContext)
        -> Result<Self, &'a [Diagnostic]>;
}
```

Lowering takes the same value. Nothing about the gate changes when lowering
arrives: codegen passes the clean analysis it receives on to lowering.

**REQ-LOW-analyzer-010** Constructing a clean analysis from a
`SemanticContext` that holds any diagnostic fails.

**REQ-LOW-lowering-011** Lowering has no entry point that accepts a `Library`
without a clean analysis.

`CleanAnalysis` meets REQ-LOW-analyzer-010 today.

The gate guards against a mistake, not an adversary. It stops a caller from
forgetting the check; it does not prove that the `SemanticContext` describes
the `Library`, or that the semantic rules ran. A context built by
`SemanticContextBuilder`, or returned by `stages::resolve_types`, has been
through no rule, so it holds no diagnostics and the gate accepts it. A caller
can also pair a context with a library it was not built from. The
documentation of `CleanAnalysis` says both.

So lowering does not trust the gate further than it reaches. Behind it, a
state analysis rules out is a compiler defect, and lowering reports it as
P9998 from one place. It is never a user-facing problem, never a silent
default and never a panic, which is what keeps a hand-built or mismatched
context from crashing the compiler. Codegen keeps its own defensive error
paths ("reaching here means analysis was skipped") until the route that reads
the AST is deleted.

Which states analysis rules out is established case by case, not assumed.
Some of what codegen handles is reached by programs analysis accepts. The
analyzer does not record a function block output stored by a call yet, so
`fb(OUT => x)` reaches codegen with no conversion, and codegen stores the
output at the field's operation type
([Implicit Conversions](implicit-conversions.md),
[issue 2125](https://github.com/ironplc/ironplc/issues/2125)). A path like
that does not become a P9998 in lowering. The analyzer first either
rejects the program or records a decision for it.

## 3. The Lowered Program

The Rust in this section shows shape. Field lists are indicative; the
invariants stated as requirements are the specification.

### 3.1 Identity

Everything is referred to by an id allocated during lowering, never by name.

| Id | Names | Replaces |
|---|---|---|
| `VarId` | One variable that is not a field: a global (including each program instance), a parameter, local or result of a function or method, a `VAR_TEMP` of any body, the instance parameter of a body, or a compiler-provided temporary | `Id` looked up in `CompileContext::variables` and six sibling maps |
| `PouId` | A program, function, function block or method | The lower-case key of `user_functions`, the upper-case key of `user_fb_types` |
| `FieldIdx` | A field of a structure, function block or program, by position. Every variable a function block or program declares, other than `VAR_TEMP` and `VAR_EXTERNAL`, is a field of its instance, and so is every field lowering synthesizes for it (see [Synthesized state](#synthesized-state)) | The lower-case field name in `field_index` and `field_indices` |
| `LoopId` | One `Loop` or `For` | The top of the `loop_labels` stack |
| `TypeId` | A type (ADR-0055, unchanged) | Unchanged |

Each id is a type of its own, a newtype over an integer such as
`struct VarId(u32)`, and not a type alias. An alias such as `type VarId = u32`
would let a `PouId` be passed where a `VarId` is expected, and the compiler
would accept it; that mix-up is what separate ids exist to prevent. `TypeId`
is already a newtype.

**REQ-LOW-ir-019** `VarId`, `PouId`, `FieldIdx`, `LoopId` and `TypeId` are
distinct types, so a value of one cannot be used where another is expected.

**REQ-LOW-lowering-020** Two variables with the same name in different scopes
have different `VarId`s.

**REQ-LOW-lowering-021** No node of the lowered program holds an `Id` or a
string that a backend must resolve; names appear only on the declarations of
variables, POUs, fields and types, for debug information and diagnostics.

**REQ-LOW-lowering-022** A `VAR_EXTERNAL` declaration has no variable of its
own: every access through it names the `VarId` of the global it refers to.

### 3.2 Program, POUs and variables

```rust
pub struct Program {
    pub types: TypeTable,              // TypeId -> TypeEntry (below)
    pub variables: Vec<Variable>,      // indexed by VarId
    pub pous: Vec<Pou>,                // indexed by PouId
    pub init: Vec<Stmt>,               // initial values of every global and instance, run once
    pub instances: Vec<ProgramInstance>, // the global holding each program instance, and its task
    pub tasks: Vec<Task>,
}

pub struct Variable {
    pub name: Id,
    pub owner: Owner,                  // Global or Pou(PouId)
    pub section: Section,              // Global, Input, Output, InOut, Local, Temporary, Instance
    pub ty: TypeId,
    pub location: Option<DirectAddress>, // `AT %IX0.1`
    pub retention: Retention,          // None, Retain, NonRetain or Persistent
    pub span: SourceSpan,
}

pub struct Pou {
    pub name: Id,
    pub kind: PouKind,                 // Program, Function, FunctionBlock, Method { of: PouId }
    pub instance: Option<VarId>,       // the instance parameter of a program, function block or method
    pub parameters: Vec<VarId>,        // functions and methods: the result parameter, if any, then the declared ones
    pub result: Option<VarId>,         // the function's or method's result variable (see §3.8)
    pub locals: Vec<VarId>,            // locals of a function or method, and VAR_TEMP of any body
    pub body: Vec<Stmt>,
    pub span: SourceSpan,
}

pub struct TypeEntry { pub name: Option<Id>, pub kind: TypeKind }

pub enum TypeKind {
    Bool,
    Integer { bits: u8, signed: bool },
    BitString { bits: u8 },
    Real { bits: u8 },
    Temporal { kind: TemporalKind, bits: u8 },
    Enumeration { underlying: TypeId, values: Vec<Id> },
    Subrange { base: TypeId, min: i128, max: i128 },
    String(StringShape),
    Array { element: TypeId, dimensions: Vec<(i32, i32)> },
    Structure { fields: Vec<Field> },
    FunctionBlock { fields: Vec<Field>, body: PouId },
    Program { fields: Vec<Field>, body: PouId },
    Reference { target: TypeId, nullability: Nullability }, // Nullable or NonNull (see §3.4)
    Interface,                         // a value referring to an instance of any implementer (see §3.8)
}

pub struct Field {
    pub name: Id,
    pub section: Option<Section>,      // Internal for a synthesized field
    pub ty: TypeId,
    pub span: SourceSpan,
}
```

A program, like a function block, has an instance: the global that a program
configuration declares holds it, and the program's variables are the fields of
its type. A program, function block or method body reaches its own instance
through its instance parameter (see [Places](#34-places)). A method's instance
parameter is also its first parameter (see
[Callees, arguments and intrinsics](#38-callees-arguments-and-intrinsics)).

The type table describes types as the language defines them. The width of an
`INT` is part of the language; where a field sits in memory, how many slots a
value takes and how wide a reference is are each backend's layout. Today's
`SemanticType` carries both: `SemanticStructField::offset` is a byte
offset, `slot_count` sizes strings with the container's `string_region_size`,
and a reference is sized as a 64-bit variable-table index. Lowering builds its
table from `SemanticType` and carries none of those.

**REQ-LOW-lowering-027** The type table holds no size, offset or slot count,
and refers to the type of every field, element, enumeration, subrange and
reference target by its `TypeId`.

A variable's location and retention are part of the language, not of a
layout: `x AT %IX0.1 : BOOL` says where its value comes from, and `RETAIN`
says that it survives a warm restart. Neither has an effect on the bytecode VM
today. A named located variable is an ordinary slot, a directly represented
variable in an expression is P9999, and `RETAIN` and `PERSISTENT` are parsed
and ignored. A native backend that drives real I/O, or keeps retained variables
in non-volatile memory, needs both, so the lowered program carries them rather
than dropping them a stage earlier.

**REQ-LOW-lowering-023** A variable declared with a direct address carries
that address.

**REQ-LOW-lowering-024** A variable carries the retention qualifier of its
declaration (`RETAIN`, `NON_RETAIN` or `PERSISTENT`), or none if its
declaration has none.

Initial values are statements because that is how the bytecode VM applies
them: an init function runs once (ADR-0045), and a function re-initializes its
locals in a prologue (ADR-0024). Another backend may prefer to place constant
initial values in a data image, as a WebAssembly data segment would. Statements
do not prevent that: a backend can evaluate an `init` whose values are all
constants at compile time. `init` stays a list of statements. Whether it
should become declarative (a value per place), and how a warm restart that
keeps `RETAIN` variables is expressed, are revisited with support for
`RETAIN` variables (see [Scope](#scope)).

The initial values of an instance's fields are written where the instance is
declared, as statements through the place that holds it, so an instance's own
initializer (`t : MyFb := (x := 7)`) needs no further mechanism. The locals of
a function or method, and the `VAR_TEMP` of any body, are re-initialized by
statements at the head of the body.

**REQ-LOW-lowering-025** The lowered program contains only POUs reachable from
a program instance, matching what `SemanticContext::reachable` gives codegen
today.

An unreachable POU is never lowered. Analysis still checks every POU, so
`check` reports a problem in an unreachable POU as it does today. It does not
report a construct the compiler cannot generate yet there, because that POU
is never generated. A library with no configuration therefore lowers nothing.

**REQ-LOW-lowering-026** Every variable's `ty` is present in the type table.

A function's result is a variable, as in the source, where the result is
assigned through the function's name. `Return` carries no value.

#### Synthesized state

Some state an instance keeps from one round to the next is declared by no one:

- **Standard function blocks.** The bytecode VM's TON keeps the time it started
  and whether it is running in fields 4 and 5 of its instance
  (`vm/src/intrinsic.rs`), which the analyzer's TON does not declare.
- **Sequential function charts.** A chart keeps the activity and elapsed time
  of each step, and the state of each action's control.
- **Ladder diagrams.** A rung keeps the previous value of each edge contact.
- **Function block diagrams.** A network keeps the value on each feedback path.

Each of these is a field of the instance, because it persists across rounds,
which a compiler-provided temporary (a `VarId` of one body) does not. Lowering
synthesizes these fields. A synthesized field has section `Internal`, as the
analyzer already declares the hidden `M` of `R_TRIG`, and a name a debugger can
show: the activity of step `S1` is the field `S1.X`, which is what a debugger
of a chart most needs to show.

**REQ-LOW-lowering-028** A synthesized field has section `Internal`, a name
unique within its type, and the span of the source element it was synthesized
for.

### 3.3 Value classes

A value belongs to exactly one of three classes, and each class has its own
representation. Keeping them apart is what stops an addition of two strings or
a comparison of two arrays from being writable.

| Class | Types | Represented by |
|---|---|---|
| Scalar | `BOOL`, integers, reals, bit strings, time and date types, enumerations, subranges, references, interface values | `Expr`, typed by a `ScalarType` |
| String | `STRING`, `WSTRING` | `StrExpr`, typed by a `StringShape` |
| Aggregate | arrays, structures, function block and program instances | `Place` only; there is no aggregate expression |

`ScalarType` is today's `OpType`, one of four operation widths (32-bit integer,
64-bit integer, 32-bit float, 64-bit float) with a signedness, plus a
reference kind and an interface kind. Widths narrower than 32 bits exist only
as the storage of a place (ADR-0001). `StringShape` is today's
`string_width::StringShape`: an encoding (ADR-0034) and a capacity. Today
the capacity may be unknown; in the IR it is always stated (see
[String capacity](#string-capacity)).

```rust
pub enum ScalarType { I32(Signedness), I64(Signedness), F32, F64, Ref, Interface }
```

The operation widths are ADR-0001's, and this design assumes them. They are
part of what a program means, not a choice a target makes, because an
intermediate result wraps at its operation width (REQ-LOW-codegen-083). For
two `INT`s `a` and `b` of 30000, `(a + b) / 2` is 30000 computed at 32 bits,
and -2768 computed at 16. A backend therefore computes at these widths
whatever its target's native word is. A 16-bit microcontroller emulates a
32-bit addition, as a C compiler does for `long`, and a 64-bit target wraps at
32 bits where the lowered program says 32.

Other widths may be wanted in the future: an `INT` operation computed at 16
bits, as the standard's result types suggest, or a 64-bit operation for every
integer. Either changes what programs mean, so it would be a decision recorded
here and made in lowering, and `ScalarType` would gain the width. No backend
chooses a width on its own.

A reference has no width in the lowered program. The bytecode VM stores one as
a 64-bit variable-table index (`codegen/src/type_info.rs`); a 32-bit
WebAssembly target would store a 32-bit address. Which applies is the
backend's layout.

An interface value has no width either, and no visible parts. It refers to
one function block instance and knows that instance's concrete type (see
[Calls through an interface](#calls-through-an-interface)). How it is stored
is the backend's layout. For the bytecode VM,
[pull request 1870](https://github.com/ironplc/ironplc/pull/1870) proposes an
index into a read-only table of instances in the container. A native backend
might store a pointer and a type id.

The four numeric kinds are also the four value types of WebAssembly, with
signedness on the operation in both targets, so the scalar model maps onto
both without translating types. That the two targets then compute the same
results is a separate matter, specified in
[Meaning of operations](#310-meaning-of-operations).

### 3.4 Places

A place is where a value lives: a variable and a path into it.

```rust
pub struct Place { pub root: VarId, pub path: Vec<Projection>, pub ty: TypeId }

pub enum Projection {
    Field(FieldIdx),      // structure, function block or program field
    Index(Vec<Expr>),     // one subscript per dimension
    Deref,                // through a reference
}
```

`ty` is the type of the located value, so a backend never walks the path to
learn what it is loading.

Inside a program, function block or method body, the instance's own variables
are fields reached through the body's instance parameter, a variable of
reference type: `x` in the body of `MyFb` is
`Place { root: this, path: [Deref, Field(x)] }`. A field therefore has one
identity, whether it is named inside the body or as `inst.x` outside it, and
`THIS^` is the place `this^`. A backend that runs the body of every instance
from one copy of the code, as both targets do, receives the instance it needs
to address. `this^` is the instance itself, not a copy of it (see
[Instance fields](#instance-fields)).

**REQ-LOW-lowering-030** A `Field` projection applies only to a place whose
type is a structure, function block or program, and its index is within that
type's field list.

**REQ-LOW-lowering-031** An `Index` projection applies only to a place whose
type is an array, and carries exactly one subscript per dimension.

**REQ-LOW-lowering-032** A `Deref` projection applies only to a place whose
type is a reference.

**REQ-LOW-lowering-033** A `VAR_IN_OUT` parameter is a variable of reference
type, every access to it carries an explicit `Deref`, and the matching argument
at each call site is the place passed by reference
([VAR_IN_OUT Parameters](var-in-out-parameters.md)).

**REQ-LOW-lowering-034** Inside a program, function block or method body,
every access to a field of the current instance is a place rooted at the body's
instance parameter, followed by `Deref` and `Field`.

A reference is either nullable or non-null, and the type says which, so the
two cannot be confused:

- **Nullable.** A `REF_TO` or `POINTER TO` variable can hold `NULL`, so a
  `Deref` through one is checked.
- **Non-null.** A `VAR_IN_OUT` parameter, the instance parameter of a body,
  and the result and output parameters of
  [Callees, arguments and intrinsics](#38-callees-arguments-and-intrinsics)
  are each bound by their call to a place that exists, and are never
  assigned, so none can hold `NULL`. A `Deref` through one needs no check, and
  a backend can tell its target so, as LLVM's `nonnull` and `dereferenceable`
  attributes do.

Keeping them apart by type means a check cannot be dropped from a nullable
reference by mistake, and `NULL` cannot reach a non-null one.

**REQ-LOW-lowering-035** The type of a `REF_TO` or `POINTER TO` is a nullable
reference; the type of a `VAR_IN_OUT` parameter, an instance parameter, a
result parameter or an output parameter of a function or method is a non-null
reference.

**REQ-LOW-lowering-036** No statement assigns a variable of non-null reference
type, and no `Null` has a non-null reference type; such a variable is bound
only by an argument of a call.

A place says nothing about storage. Whether a variable occupies a slot, a run
of the data region or an address in linear memory is the backend's layout.
Codegen's `Place` (`compile_place.rs`) and `ResolvedAccess` (`compile_array.rs`)
are the bytecode backend's answer to that question, and they stay in that
backend (see [Existing Structures](#8-existing-structures)).

#### Instance fields

A program, function block or method body works on its instance's fields in
place. Each instance has one storage for its fields, which lives as long as
the program, so:

- **A write is visible at once,** through every path to the field.
- **A reference to a field stays valid** for the life of the program. That
  holds for `REF(x)` taken inside the body and for a reference to `inst.x`
  taken outside it.
- **A method called on the current instance,** through `THIS^` or `SUPER^`,
  works on the same fields.
- **When a body traps,** the writes it made before the trap stay, as writes to
  globals do.

Safety is what decides this. A reference must never be able to outlive what
it names. That is the purpose of P2029, "No REF of ephemeral variables"
([REF_TO](ref-to.md)), which accepts a reference to a function block's field
because the field is persistent. Fields in place make that true for every
backend, and they are what a WebAssembly or LLVM backend does without extra
work.

**The bytecode VM does not do this yet.** It copies an instance's fields into
slots that belong to the function block type before the body runs (`FB_CALL`
and `METHOD_CALL` in `vm/src/vm.rs`). It copies them back when the body
returns (`handle_frame_return`). Every instance of the type reuses those
slots, and a VM reference can only name such a slot. So the copy differs from
the meaning above in three ways:

- **References.** A reference to a field that outlives the body reads and
  writes whichever instance of the type ran last.
- **Traps.** A trap inside a body discards the writes the body made.
- **Shared slots.** A body that reached another instance of its own type would
  overwrite its own copy.

Removing the copy is a change to the VM of its own,
[issue 2120](https://github.com/ironplc/ironplc/issues/2120). Until it lands,
the bytecode backend keeps the copy for every statement, whichever route
compiles it. The refactor therefore changes none of this behaviour (see
[Delivery Constraints](#delivery-constraints)). The bytecode backend meets
REQ-LOW-codegen-037 and REQ-LOW-codegen-038 once issue 2120 lands.

**REQ-LOW-codegen-037** A program, function block or method body reads and
writes its instance's fields in place. A write is visible at once through
every path to the field, and a reference to a field stays valid for the life
of the program.

**REQ-LOW-codegen-038** When a trap ends the round, every write made before
the trap stays, including writes to the fields of an instance whose body was
running.

### 3.5 Scalar expressions

```rust
pub struct Expr { kind: ExprKind, ty: ScalarType, span: SourceSpan }

pub enum ExprKind {
    Const(Const),                                   // I32, I64, F32 or F64
    Read(Place),
    Convert(Box<Expr>),                             // to `ty`, from the operand's type
    Truncate { bits: u8, value: Box<Expr> },        // wrap to a storage width (ADR-0001)
    Unary { op: UnaryOp, value: Box<Expr> },        // Neg, BitNot, BoolNot
    Binary { op: BinaryOp, lhs: Box<Expr>, rhs: Box<Expr> },
    Compare { op: CompareOp, lhs: Box<Expr>, rhs: Box<Expr> },
    ShortCircuit { op: ShortCircuitOp, lhs: Box<Expr>, rhs: Box<Expr> },
    Call { callee: Callee, args: Vec<Arg> },
    RefTo(Place),
    InterfaceOf(Place),                             // the interface value for a function block instance
    Null,                                           // a null reference or a null interface value
    RoundTime,                                      // the time of the current round
}
```

`Read(Place)` is the value currently at a place. It says nothing about a
stack, a register or memory. The split it keeps is between a place, which can
be written and passed by reference, and a value, which cannot; every target has
that split. On the bytecode VM a `Read` becomes a load opcode. On WebAssembly
it becomes a local read or a load from linear memory. On a register machine it
names a register when the backend allocated the variable to one, and loads
from memory otherwise. Which of those applies is the backend's layout, so a
place the backend keeps in a register costs nothing to read.

`RoundTime` is the time the host gave the current round: an unsigned 64-bit
count of microseconds, the `uptime_us` that the bytecode VM's `run_round`
receives. No source program names it. Lowering reads it where what a construct
means depends on time: the timers among the standard function blocks, which
lowering generates (see [Standard function blocks](#standard-function-blocks)),
and the step times and timed action qualifiers of a sequential function chart.
Today the time reaches only the VM's timer intrinsics (`vm.rs`), so without a
node every backend would need its own route to it. The system uptime variables
of [ADR-0030](../adrs/0030-dual-uptime-system-variables.md) hold the same time
in milliseconds, which is too coarse: the VM's TON records its start in
microseconds and converts only the elapsed time to milliseconds.

IronPLC will change how it represents time. `TIME` and `LTIME` count
milliseconds ([ADR-0021](../adrs/0021-time-32bit-ltime-64bit.md)), the VM's
clock counts microseconds, and neither is settled. Fixing that is not part of
this design (see [Scope](#scope)). `RoundTime` is defined as the clock the
timers read today, and its unit follows whatever replaces it.

**REQ-LOW-lowering-040** Every scalar expression has a `ScalarType`; the type
is not optional and is never a generic category such as `ANY_INT`.

**REQ-LOW-lowering-041** A `Const` holds a value representable in its
`ScalarType`.

**REQ-LOW-lowering-042** The operands of a `Binary` have the type of the
`Binary` itself.

**REQ-LOW-lowering-043** The operands of a `Compare` have the same type as each
other, and the `Compare` itself has the operation type of `BOOL`.

A `ScalarType` does not say whether a value is a `BOOL`. Today codegen
computes a `BOOL` as a signed 32-bit value (`type_info.rs`), the operation
type of a `DINT`, and a `UDINT` and a `DWORD` share the unsigned one. It
chooses between a logical and a bitwise operator from the signedness alone
(`compile_expr.rs`). Lowering knows which from the source type and chooses
operators accordingly (REQ-LOW-lowering-048); a backend never needs to.

**REQ-LOW-lowering-044** A `Convert` changes type: its operand's type differs
from its own.

**REQ-LOW-lowering-045** Every implicit conversion the language permits
([ADR-0029](../adrs/0029-implicit-integer-widening.md),
[ADR-0031](../adrs/0031-expanded-implicit-type-widening.md)) appears as a
`Convert` node; a backend never converts between operation types on its own
initiative.

**REQ-LOW-lowering-046** A value stored to a place whose type is narrower than
its operation width is produced by a `Truncate` to that width, is a `Const`
within the place's range, or is a `Read` of a place whose type's range lies
within it.

**REQ-LOW-lowering-047** `Read` of a place has the operation type of the
place's type.

**REQ-LOW-lowering-048** Logical and bitwise forms of `AND`, `OR`, `XOR` and
`NOT` are distinct operators, chosen by lowering from the operand type.

### 3.6 String expressions

```rust
pub struct StrExpr { kind: StrExprKind, shape: StringShape, span: SourceSpan }

pub enum StrExprKind {
    Literal(Vec<char>),
    Read(Place),
    Call { callee: Intrinsic, args: Vec<Arg> },   // a standard string function
}
```

A user function's string result is not a `StrExpr`. It is returned through a
result parameter (see
[Callees, arguments and intrinsics](#38-callees-arguments-and-intrinsics)).

**REQ-LOW-lowering-052** The callee of a string `Call` expression is an
`Intrinsic`.

**REQ-LOW-lowering-050** The string operands of one operation share an
encoding. Analysis reports a mismatch (`StringEncodingMismatch`, ADR-0034);
lowering reports one it meets as P9998.

**REQ-LOW-lowering-051** A string literal's characters fit its encoding
([ADR-0016](../adrs/0016-string-encoding.md)).

Temporary buffers, their pool size and their release
([ADR-0052](../adrs/0052-temp-string-buffers-released-on-consume.md)) are how
the bytecode backend evaluates a `StrExpr`. They do not appear in the lowered
program.

#### String capacity

Every string type and every string expression has a capacity: the most code
units its value can hold. A `StringShape` always states it, so the lowered
program has no string of unknown capacity.

- **A declared string** has the capacity its declaration gives, as in
  `STRING[80]`. If the declaration gives none, it has the default capacity.
  The analyzer applies the default when it resolves the type, so every string
  type it records has a capacity, and lowering copies it into the type table.
- **The default capacity** is a compile-time choice. Today it is 254
  (`DEFAULT_STRING_MAX_LENGTH`); CODESYS and TwinCAT use 80, so a dialect
  would set it. A different default changes only the capacities the analyzer
  records. No backend assumes a value: a backend sizes storage from the
  capacities in the lowered program, and the bytecode VM learns from the
  container at load time how much memory to allocate. Making the default a
  compiler option is out of scope (see [Scope](#scope)); this design keeps it
  possible.
- **An intermediate result** has the capacity lowering gives it. This covers
  every `StrExpr` that is not a `Read` of a place: a literal, or the result of
  `CONCAT` or a conversion. A backend sizes the storage it evaluates the
  expression into from that capacity, never from the program's declarations.

A value is cut only when it goes into a place or an intermediate result whose
capacity is smaller than the value. It keeps its first code units.

Today codegen computes a bound for each string expression
(`string_width.rs`): a declared capacity, a literal's length, `m + n` for
`CONCAT` of a `STRING[m]` and a `STRING[n]`, and 254 code units where it knows
none. It sizes the copy of an operand that is not a plain variable from that
bound. But every temporary buffer has one size, that of the largest string the
program declares or copies, so the size of a buffer is a property of the
program, not of the expression
([issue 2118](https://github.com/ironplc/ironplc/issues/2118)).

At first, lowering gives each intermediate result the bound codegen computes
for it today, so the move is behaviour preserving. A later change to how
lowering chooses a capacity is a change to lowering and no backend.

The bytecode backend cannot meet REQ-LOW-codegen-055 yet, and lowering cannot
fix that. The VM's pool of temporary buffers is sized once, by two container
header fields, `num_temp_bufs` and `max_temp_buf_bytes`
([ADR-0052](../adrs/0052-temp-string-buffers-released-on-consume.md)), so
every buffer has the same size. Sizing each buffer from the capacity of the
value it holds is a change to the container format and the VM, which this
design does not make (see [Scope](#scope)). It needs a design of its own,
issue 2118. Until then the bytecode backend gives every buffer one size, as it
does today.

**REQ-LOW-analyzer-053** Every string type the analyzer records has a
capacity: the one its declaration gives, or the default capacity.

**REQ-LOW-lowering-054** Every `StringShape` in the lowered program states a
capacity, and no backend applies a default.

**REQ-LOW-codegen-055** A backend sizes the storage for a string value from
the capacity the lowered program gives that value, never from the capacities
of other strings in the program.

**REQ-LOW-codegen-056** A string value that goes into a place or an
intermediate result with a smaller capacity keeps its first code units, up to
that capacity.

### 3.7 Statements

```rust
pub struct Stmt { kind: StmtKind, span: SourceSpan }

pub enum StmtKind {
    Assign { place: Place, value: Expr },
    AssignBits { place: Place, shift: u8, bits: u8, value: Expr },
    AssignStr { place: Place, value: StrExpr },
    Copy { dst: Place, src: Place },                          // whole aggregate
    Call { callee: Callee, args: Vec<Arg> },                  // result discarded
    FbCall { instance: Place, block: PouId },                 // runs the body; inputs and outputs are assignments around it
    If { cond: Expr, then: Vec<Stmt>, otherwise: Vec<Stmt> },
    Case { selector: Expr, arms: Vec<CaseArm>, otherwise: Vec<Stmt> },
    For { id: LoopId, control: Place, from: Expr, to: Expr, step: Const, body: Vec<Stmt> },
    Loop { id: LoopId, body: Vec<Stmt>, continuing: Vec<Stmt> },
    Exit(LoopId),
    Continue(LoopId),
    Return,
}
```

`Loop` repeats `body` then `continuing` until an `Exit` leaves it. `Continue`
skips the rest of `body` and runs `continuing`. That is enough for `WHILE` and
`REPEAT` (see [Desugaring](#5-desugaring)), and it is the shape WebAssembly's
structured control flow takes directly.

`Loop` and `For` are the only statements that repeat, and no call is
recursive, since analysis reports a recursive cycle. So between two iterations
of any loop a body runs a bounded number of statements, and a backend that must
bound the run time of a round checks there. The bytecode VM's watchdog measures
a task only after the task finishes (`vm.rs`), and codegen never sets a
watchdog time, so an infinite loop is never interrupted. A native backend has
no interpreter loop to check from at all. It checks a deadline at each
iteration of a `Loop` or `For`.

**REQ-LOW-lowering-059** Every repetition in the lowered program is a `Loop` or
a `For`, and no POU calls itself directly or through other POUs.

**REQ-LOW-lowering-060** The type of an `Assign`'s value is the operation type of
its place's type.

**REQ-LOW-lowering-061** The two places of a `Copy` have the same type or types
of identical shape, as `value_type::check` accepts today.

**REQ-LOW-lowering-062** Every `Exit` and `Continue` names a `Loop` or `For`
that encloses it.

**REQ-LOW-lowering-063** Every statement has the span of the source statement
it was lowered from, so a backend can build a line map without the AST.

**REQ-LOW-lowering-064** The labels of a `Case` are constant values or constant
ranges of the selector's type.

**REQ-LOW-lowering-065** `AssignBits` names a bit range that lies within the
storage width of its place's type.

The arms of a `Case` are tried in order and the first arm with a matching
label runs, as `compile_case` does today.

`AssignBits` is the one read-modify-write statement. It exists as a node, and is
not desugared to an `Assign` of a masked `Read`, because that would name the
place twice and so evaluate its subscripts twice. Codegen does exactly that
today: `Place::emit_store` (`compile_place.rs`) emits a base's address again
for the store, rather than reusing the one emitted for the load.

`For` stays a node rather than becoming a `Loop`, so that its control
variable, bounds and step stay visible: the bytecode backend uses them to fuse
the loop test and to drop the control variable's truncation when the bounds
prove it in range (`compile_loop.rs`), and a vectorizing pass would match them
(see [Vectorized operations](#vectorized-operations)). Its meaning is what
`compile_loop.rs` does today, stated here so that every backend does the same.

**REQ-LOW-lowering-066** A `For` assigns `from` to `control` once and then
repeats: it evaluates `to`; it leaves the loop when `control` is greater than
`to` for a positive `step`, or less than `to` for a negative one; it runs
`body`; and it assigns `control + step`, wrapped to the storage width of
`control`'s type, to `control`. A `Continue` naming the `For` goes on to that
assignment.

**REQ-LOW-lowering-067** The `step` of a `For` is a nonzero constant of the
operation type of `control`, and `control` is a variable or a field of the
current instance. A `FOR` whose step is not a nonzero constant is reported as
P9999, as it is today.

`to` is evaluated on every iteration, and the control variable wraps, so a
`FOR` whose bound is the largest value of the control variable's type does not
end. Both are today's behaviour, recorded rather than changed.

**REQ-LOW-ir-068** `ironplc-ir` provides the expansion of a `For`
into an `Assign` and a `Loop` with the same meaning, so a backend with no use
for the bounds implements `For` by expanding it.

A function block call is three steps in order. Its inputs are assigned to the
instance's fields, the block's body runs (`FbCall`), and its outputs are
assigned from the instance's fields to their targets. Each assignment is an
ordinary `Assign`, `AssignStr` or `Copy`, and a `VAR_IN_OUT` input is an
`Assign` of a `RefTo`, so an output whose target has a wider type carries the
`Convert` the analyzer recorded. Today codegen stores an output at the field's
operation type, with no conversion (`compile_stmt.rs`).

**REQ-LOW-lowering-069** A function block call lowers to the assignments of
its inputs to the instance's fields, then an `FbCall`, then the assignments of
its outputs from the instance's fields, in that order.

### 3.8 Callees, arguments and intrinsics

```rust
pub enum Callee { User(PouId), Intrinsic(Intrinsic), Interface(InterfaceCallee) }

pub struct InterfaceCallee {
    pub value: Box<Expr>,               // the interface value the call goes through
    pub implementers: Vec<Implementer>, // every concrete type the value can hold
}

pub struct Implementer { pub ty: TypeId, pub method: PouId }

pub enum Arg {
    Value(Expr),        // scalar, by value
    Str(StrExpr),       // string, by value
    Copy(Place),        // array, structure or FB instance, by value
    Ref(Place),         // VAR_IN_OUT, a method's instance, or a result or output temporary
}
```

Arrays, structures and function block instances are never values in the
lowered program. They are places: an element or field is reached by a
`Projection`, a whole one is assigned by the `Copy` statement, and one is
passed to a call as an `Arg`:

- **`Copy`** for a `VAR_INPUT`: the callee receives its own copy, so its
  writes do not reach the caller, as IEC 61131-3 requires of an input.
- **`Ref`** for a `VAR_IN_OUT`: the callee reads and writes the caller's
  place.

Keeping the two apart matters because a backend may implement both by passing
an address. Then the difference is only whether the backend copies first, and
a node that says so cannot be forgotten. Today `ParamPassing` has no mode for a
by-value aggregate input, and such a parameter falls through to the scalar
default (`ParamPassing::Value(DEFAULT_OP_TYPE)` in `compile_fn.rs`).

A function whose result is a string or an aggregate returns it through a
result parameter: a hidden parameter of non-null reference type, after a
method's instance parameter and before the declared parameters. Every access
to the result goes through a `Deref`, as for a `VAR_IN_OUT`
(REQ-LOW-lowering-033). At each call, the argument for it is a `Ref` to a
compiler-provided temporary of the result's type: scratch space the caller
owns. The call is a `Call` statement, and an `AssignStr` or `Copy` from the
temporary then puts the result where it goes. A scalar result stays the value
of a `Call` expression. Today a user function may return a structure
(`UserFunctionInfo::return_struct_desc_index`, `compile_aggregate.rs`).

The temporary is what keeps this safe. Passing the destination itself would
let the callee write `s` while it still reads `s`, as `s := f(s)` does, and a
trap in the callee would leave `s` half written. With the temporary, the
destination changes only when the call returns, as it does when a value is
assigned. A backend that can prove nothing else reaches the destination may
write into it directly, as LLVM does when it optimizes an `sret` argument.

**REQ-LOW-lowering-079** A function whose result is a string or an aggregate
has a result parameter of non-null reference type, and every call to it passes
a `Ref` to a compiler-provided temporary of the result's type for that
parameter.

A function or method may also declare `VAR_OUTPUT`, which a call assigns to a
place with `=>`. An output is passed the same way as a result: the argument is
a `Ref` to a compiler-provided temporary of the parameter's type, and after
the call an ordinary `Assign`, `AssignStr` or `Copy` moves it to its target.
A function block call treats its outputs the same way (REQ-LOW-lowering-069).

Passing the target itself is what a `VAR_IN_OUT` does, and it would be
simpler. But it differs from an output in three ways a program can observe:

- **Aliasing.** The callee would see its own writes through a global that is
  also the target.
- **Traps.** A trap would leave the target half written.
- **Conversion.** An output whose target has a wider type would be written at
  the parameter's width rather than converted, which needs the `Convert` the
  analyzer records.

Codegen assigns outputs only for a function block call today, and a method
call ignores an output argument (`compile_method.rs`).

**REQ-LOW-lowering-092** An argument for a function's or method's
`VAR_OUTPUT` parameter is a `Ref` to a compiler-provided temporary of the
parameter's type, and the call is followed by an assignment from that
temporary to the output's target.

A method is called like a function whose first parameter is its instance, and
the first argument of the call is a `Ref` to the receiver. `inst.m(a)` passes
`inst`; `THIS^.m(a)` passes the caller's own instance, `this^`; and
`SUPER^.m(a)` calls the base type's method with that same receiver. Each of
these is resolved statically, as the first phase of
[ADR-0041](../adrs/0041-staged-method-and-interface-dispatch.md) resolves
`inst.m(a)` today. Analysis does not resolve a call through `THIS^` or
`SUPER^` yet, and reports one as not implemented.

A property is sugar. Reading `inst.P` calls its `GET` accessor, and writing
`inst.P := v` calls its `SET` accessor, each lowered as a method call on
`inst`, so the lowered program has no property node.

**REQ-LOW-lowering-076** The first parameter of a method is its instance
parameter, and the first argument of every call to a method is a `Ref` to the
receiver's place, except a call through an interface (see
[Calls through an interface](#calls-through-an-interface)).

#### Calls through an interface

An interface value refers to one function block instance whose concrete type
implements the interface. Every implementer is in the program the compiler
sees: compatibility libraries are merged before analysis, and there is no
separate compilation. So a call through an interface goes one of two ways:

- **One possible type.** The analyzer finds the concrete types each interface
  variable can hold, following the instances assigned or passed to it.
  Where only one is possible, lowering gives the variable the type of a
  nullable reference to that function block. A call through it is then a
  direct method call on the instance the reference points at. One type can
  still mean many instances, so the receiver is still found at run time. The
  analyzer makes this decision, because it decides whether a program that
  needs dynamic dispatch is reported when dynamic dispatch is not enabled.
- **Several possible types.** The call is a `Call` whose callee is
  `Callee::Interface`. It carries the interface value and, for every concrete
  type the value can hold, that type's method. Lowering makes that list from
  the whole program. The arguments do not include a receiver; the receiver is
  the instance the value refers to.

What the call means is the same on every target: the method of the value's
concrete type runs with the value's instance as its receiver, and a call
through a null interface value traps. How it dispatches is the backend's
choice. For the bytecode VM, pull request 1870 proposes reading the instance
and its type from a table in the container, then branching to a direct call.
A native backend might use a switch or a table of functions. The lowered
program fixes the meaning and the list of implementers, and leaves the
mechanism to each backend.

An interface value is made from an instance place (`InterfaceOf`). Assigning
it to a variable of an interface that it extends copies it unchanged. Two
interface values are equal when they refer to the same instance, and `Null`
is the null interface value. Analysis rejects an assignment that converts
downward. `__QUERYINTERFACE` and `__QUERYPOINTER` are out of scope (see
[Scope](#scope)).

**REQ-LOW-analyzer-107** For each call through an interface, analysis records
the concrete types the interface value can hold, or that it can hold only one.

**REQ-LOW-lowering-104** An interface variable whose value can hold only one
concrete type has the type of a nullable reference to that function block. A
call through it lowers to a direct method call whose first argument is a `Ref`
to the place the reference points at, so a null value traps there
(REQ-LOW-codegen-088).

**REQ-LOW-lowering-105** Any other call through an interface lowers to a
`Callee::Interface` that lists, for every concrete type the value can hold,
the method that implements the called method.

**REQ-LOW-codegen-106** A call through an interface runs the method of the
value's concrete type with the value's instance as its receiver, and traps
when the value is null.

`Intrinsic` is an enum with one variant per operation the compiler implements
itself, at each set of operand types it takes: the standard functions of
IEC 61131-3 and the extensions ADR-0042 admits. It is defined in `ironplc-ir`
(see [Position in the Pipeline](#1-position-in-the-pipeline)). The analyzer
owns the table of built-in function signatures, and each `FunctionSignature`
for a built-in function names its `BuiltinFunction`: which function, not at
which types. Lowering maps that function and the operand types the analyzer
recorded to one `Intrinsic`. No stage after the analyzer matches a function's
name.

Each variant names one operation at one set of operand types: `SqrtF32` and
`SqrtF64`, not `Sqrt` with the type left to the call, and `ConcatString` and
`ConcatWString` for the two encodings. A combination the language does not
allow, such as the square root of an integer, therefore cannot be written. That
is Goal 2 applied to intrinsics, and the choice
[ADR-0004](../adrs/0004-separate-type-families-over-polymorphic-opcodes.md)
made for the VM's opcodes. With the exhaustive match of REQ-LOW-codegen-074, a
backend that compiles handles every combination. The analyzer resolves a
generic function such as `ADD` on `ANY_NUM` to one type before lowering sees
it.

**REQ-LOW-ir-145** Each `Intrinsic` variant names one operation at one set of
operand types, and a call's arguments have exactly the types its variant
names.

**REQ-LOW-analyzer-070** Every built-in function signature in the
`FunctionEnvironment` identifies its `BuiltinFunction`.

**REQ-LOW-lowering-147** Lowering maps each call of a built-in function to the
one `Intrinsic` variant for its `BuiltinFunction` at the operand types the
analyzer recorded, with a match that has no wildcard arm. A combination with no
variant is P9999.

**REQ-LOW-lowering-071** A call's arguments correspond one to one, in order, to
the callee's parameters: its instance parameter, its result parameter and its
declared parameters, each where it has one.

**REQ-LOW-lowering-072** Each argument's class and type match its parameter's:
`Value` at the parameter's operation type, `Str` at its encoding, `Copy` from a
place of its type, `Ref` to a place of its type.

**REQ-LOW-lowering-075** An argument for a `VAR_INPUT` parameter of an array,
structure or function block type is a `Copy`, and an argument for a
`VAR_IN_OUT` parameter is a `Ref`.

**REQ-LOW-lowering-073** A behaviour policy
([ADR-0049](../adrs/0049-behavior-policies-selected-at-compile-time.md))
selects among `Intrinsic` variants during lowering; a backend receives no
policy options.

**REQ-LOW-codegen-074** The bytecode backend maps `Intrinsic` to a `func_id`
with a match that has no wildcard arm, so adding a variant fails to compile
until the backend handles it.

#### Standard function blocks

The standard function blocks are `TON`, `TOF`, `TP`, `SR`, `RS`, `R_TRIG`,
`F_TRIG`, and the counters `CTU`, `CTD` and `CTUD`, each with a form for every
integer width it takes (`CTU_INT` to `CTU_ULINT`). Today the bytecode VM runs
them natively ([ADR-0003](../adrs/0003-plc-standard-function-blocks-as-intrinsics.md)),
and what each does is stated only by the VM's own code (`vm/src/intrinsic.rs`).
A second backend would need a second implementation of every one, which is
the duplication this design exists to remove.

What kept a block out of reach of the lowered program was time: a timer reads
the time of the round, which no source program can name. `RoundTime` gives
lowering that time. So lowering lowers each standard function block the
program uses to an ordinary function block of the lowered program: a POU and
a type, as for a block the program declares. The block's hidden state is
among its fields as synthesized fields (see
[Synthesized state](#synthesized-state)), and a timer reads `RoundTime`. A
call is an `FbCall` like any other.

No backend sees a standard function block, so none implements one, and the
lowered program has no name for one. Each counter's forms come from one
template in lowering, generated for each width the program uses.

**REQ-LOW-lowering-077** Lowering lowers every standard function block the
program uses to a function block POU of the lowered program, and no node of
the lowered program names a standard function block.

The lowered blocks start from what the bytecode VM's intrinsics do today, so
the move changes no program, with one exception. Today codegen sends
`CTU_INT`, `CTU_DINT`, `CTU_UDINT`, `CTU_LINT` and `CTU_ULINT` to one VM
intrinsic, which reads and writes `PV` and `CV` as signed 32-bit values
(`compile_call.rs`, `vm/src/intrinsic.rs`), and the down and up-down counters
do the same. A lowered counter computes at its own width, so a wide counter
past the 32-bit range behaves differently. That is a correction, like those
under [Backend Contract](#7-backend-contract).

**REQ-LOW-codegen-078** For every standard function block, the block lowering
generates leaves the instance's visible fields with the values the bytecode
VM's intrinsic leaves, for the same inputs and round times, except where a
wide counter's value lies outside the 32-bit range.

### 3.9 Invariants by construction

The fields of `Expr`, `StrExpr`, `Place` and `Stmt` are private to
`ironplc-ir`. Nodes are built through its constructors, which check the
requirements above and return an internal error when one is violated. The
constructors are public, so `ironplc-lowering` and the target-neutral passes
of [Optimization](#10-optimization) can call them, but they are the only way
to build a node.

**REQ-LOW-ir-080** A lowered node can be constructed only through the
constructors of `ironplc-ir`.

**REQ-LOW-ir-081** A constructor that receives operands violating a
requirement of this section returns a P9998 diagnostic that names the
constructor; it does not build the node.

This puts the invariants in one place, upstream of every backend. A backend
relies on them without re-checking, which is what lets its matches be total.

**REQ-LOW-lowering-082** `ironplc-ir`, `ironplc-lowering` and every backend crate deny
`clippy::wildcard_enum_match_arm`, so a match over a lowered enum names every
variant.

### 3.10 Meaning of operations

A lowered operation means the same on every target. Where a target's own
instruction does something else, the backend emits what makes up the
difference; the lowered program does not change. The rules below start from
what the bytecode VM does today (`vm/src/vm.rs`, `vm/src/builtin.rs`), so that
moving onto the lowered program changes no program. The rules, not the VM, are
the reference: the VM has defects of its own, and where a rule turns out to
record one, the rule is corrected and every backend follows. Each rule is
checked by running a program, so each belongs to the end-to-end suite that
every backend runs.

**REQ-LOW-codegen-083** Integer `Add`, `Sub`, `Mul` and `Neg` wrap at the
operation width, in two's complement.

**REQ-LOW-codegen-084** Integer `Div` rounds toward zero, and `Mod` takes the
sign of the dividend. Both trap when the divisor is zero. A signed `Div` of the
most negative value by -1 gives the most negative value, and the matching
`Mod` gives 0.

**REQ-LOW-codegen-085** Real operations follow IEEE 754 with rounding to
nearest. A real division by zero gives an infinity or a NaN and does not trap,
and every comparison with a NaN operand is false except `<>`, which is true.

**REQ-LOW-codegen-086** A `Convert` from a real to an integer truncates toward
zero and saturates at the range of its operation type; a NaN converts to 0. A
`Convert` from an integer to a real rounds to nearest. Between integer
operation types, a `Convert` to a wider type sign-extends a signed operand and
zero-extends an unsigned one, a `Convert` to a narrower type keeps the low 32
bits, and a change of signedness alone keeps the bits.

**REQ-LOW-codegen-087** A `Truncate` keeps the low `bits` bits of its operand,
then sign-extends them for a signed type and zero-extends them for an unsigned
one (ADR-0001; narrowing wraps, ADR-0049).

An explicit conversion from a real to a narrow integer type is a `Convert`
followed by a `Truncate`, so `REAL_TO_INT(40000.0)` is -25536, as it is today.

**REQ-LOW-codegen-088** A place reached through a `Deref` of a nullable
reference that holds `NULL` traps when it is evaluated, whether it is then
read, assigned or passed by reference. A `Deref` of a non-null reference never
traps.

**REQ-LOW-codegen-099** An `Index` traps when the position its subscripts give,
counted across all of the array's dimensions, lies outside the array
([ADR-0023](../adrs/0023-array-bounds-safety.md)).

The run-time check is on that flat position, as ADR-0023 chose. It keeps every
access inside the array, but a combination of subscripts that is out of range
in one dimension and still lands inside the array does not trap: `a[0, 5]` on
`ARRAY[1..3, 1..4]` is the first element. Analysis reports a constant
subscript outside its dimension (REQ-LOW-analyzer-095). Checking every
subscript against its own dimension at run time would catch the rest, and
ADR-0023 leaves that as a future enhancement. An `Index` keeps one subscript
per dimension, so the lowered program can express either check, and changing
this requirement changes every backend at once.

How a backend turns subscripts into an address, through the flat position and
the stride, is its own layout. That is what keeps the check safe: the
component that computes the address is the one that checks the position.
[ADR-0054](../adrs/0054-explicit-element-stride-in-array-descriptors.md)
gives the bytecode VM an explicit stride for that reason, so that codegen
never hands the VM an address it has not checked. The lowered program carries
subscripts, never an address or a flat position, so no backend receives an
address it did not check.

**REQ-LOW-codegen-089** An `Intrinsic` means what the IEC 61131-3 standard
function means, with these choices where the standard leaves room: the count
of `SHL` and `SHR` is taken modulo the operation width, `SHR` fills with
zeros, and `EXPT` of an integer base by a negative exponent traps.

A real function such as `SIN` or `EXP` is computed with the target's own
primitive: Rust's `f64` methods on the bytecode VM (`vm/src/builtin.rs`), and
an LLVM intrinsic or the platform's math library on a native target. Results
can differ between targets in the last bits. So backends are compared with a
near check for real values, not bit for bit (REQ-LOW-codegen-132).

**REQ-LOW-codegen-096** Every `RoundTime` evaluated during one round has the
value the host gave that round.

A trap faults the whole bytecode VM today: no later statement runs, no later
task in the round runs, and the VM does not resume (`run_round`, `VmFaulted`).

**REQ-LOW-codegen-097** A trap ends the round: no statement after it runs, and
no program instance after it in the round runs.

**REQ-LOW-codegen-098** Every trap is a division by zero, a negative exponent,
a null dereference, a call through a null interface value, a subscript out of
bounds, a string that does not convert or a watchdog timeout, and a backend
reports each under the problem code the bytecode VM gives it (V4001 to V4006,
and the code chosen for a call through a null interface value when the VM
implements one).

After a trap, every variable keeps what was written to it before the trap,
including the fields of an instance whose body was running
(REQ-LOW-codegen-038; see [Instance fields](#instance-fields)).

The program instances of a round run one at a time, each to completion, as the
bytecode VM's cooperative scheduler runs them (`vm/src/scheduler.rs`). The
lowered program assumes it. Two bodies that ran at once would race on the
globals they share, and the memory model of a native target, LLVM's among
them, makes such a race undefined rather than merely unordered. Tasks that
preempt each other are out of scope (see [Scope](#scope)). If they come,
lowering, which knows which program instance runs in which task and which
globals each POU reaches, is where a copy of each shared global per task, or
an atomic access to it, would be decided.

These rules make every operation total: for every operand, an operation either
gives the result stated above or traps. A backend never maps an operation to a
target instruction whose result is undefined for an operand the lowered program
can supply. Where a target's instruction differs, the backend emits the
difference.

A native WebAssembly instruction differs from these rules in three places, and
a WebAssembly backend emits the difference: `i32.div_s` traps on the most
negative value divided by -1, `i32.trunc_f32_s` traps on a NaN or an
out-of-range value where `i32.trunc_sat_f32_s` does not, and address 0 is
readable memory, so a null check is explicit.

LLVM differs in more places. Where it differs, its result is undefined rather
than a trap, so its optimizer may assume the case never happens and miscompile
a program that reaches it:

- `sdiv` and `srem` are undefined for a zero divisor and for the most negative
  value divided by -1. An LLVM backend guards both (REQ-LOW-codegen-084).
- `fptosi` and `fptoui` give poison for a NaN or an out-of-range value.
  `llvm.fptosi.sat` and `llvm.fptoui.sat` give what REQ-LOW-codegen-086
  requires.
- A shift by the operand's width or more gives poison, so the count is reduced
  modulo the width first (REQ-LOW-codegen-089).
- Integer operations carry no `nsw` or `nuw` flag, because they wrap
  (REQ-LOW-codegen-083). Real operations carry no fast-math flag, `<>` is
  `fcmp une`, and every other comparison is ordered (REQ-LOW-codegen-085).
- A load or store through a null or out-of-bounds address is undefined, so the
  null and bounds checks of REQ-LOW-codegen-088 and REQ-LOW-codegen-099 are
  explicit branches.
- LLVM may assume that a function marked `mustprogress` eventually returns or
  performs a volatile, atomic or I/O operation. A `FOR` whose bound is the
  largest value of its control variable's type never ends
  (REQ-LOW-lowering-066), so an LLVM backend does not mark functions
  `mustprogress`.
- `llvm.sin` and its siblings call the target's math library, and LLVM may
  fold a call with a constant operand using the build machine's library. The
  results are compared between backends with the near check of
  REQ-LOW-codegen-132.

## 4. Decisions

A decision whose outcome can make a program invalid is made by the analyzer and
recorded in the analyzed `Library`, because analysis is where `check` and the
language server look for problems. A decision no valid program can get wrong
is made by lowering. Each table's middle column is where the decision is made
today.

**Decisions the analyzer makes and records**

| Decision | Made today in | Recorded as | In the lowered program |
|---|---|---|---|
| Type of an untyped literal | The analyzer (ADR-0056), except a member initializer of a function block instance, which codegen builds itself | The literal's `expr_type`, as `ExprType::Inferred` | A `Const` of that type |
| Implicit conversion | The analyzer for operands, assigned values, the arguments of functions, methods and function block calls, loop bounds, and the results of calls and dereferences (ADR-0056); codegen, through `convert_to_context`, for the contexts the analyzer does not record yet, such as a condition, a subscript and a function block output | `ExprKind::ImplicitConversion` | `Convert` |
| Arithmetic overload | Analyzer's `resolve_arithmetic_overload` | The expression's `expr_type`, and its operands' conversions | A `Binary` at the result type, or the desugared time arithmetic |
| Operand type of a comparison | The analyzer ([Comparison Operand Type](comparison-operand-type.md)), with a codegen fallback for a pair without one | Its operands' conversions | A `Compare` at that type |
| Argument order and count | `xform_named_to_positional_args` for a function call, then re-checked at 27 sites in codegen; codegen for a method call (`compile_method.rs`) and, by name, for a function block call (`compile_stmt.rs`) | Positional arguments | `Vec<Arg>` matched to parameters |
| Whether an interface value can hold only one concrete type | Not made; calls through an interface are not compiled | The concrete types the value can hold (REQ-LOW-analyzer-107) | A direct method call, or a `Callee::Interface` |
| Capacity of a string declared without one | Codegen and `slot_count`, from `DEFAULT_STRING_MAX_LENGTH` | The string type's capacity (REQ-LOW-analyzer-053) | The capacity of its `StringShape` |
| Enumeration ordinal | The analyzer, when the type enters the type environment (`EnumerationMembers`); codegen looks the ordinal up (`compile_enum.rs`) | Each member's ordinal, explicit (`GREEN := 5`) or counted on from the one before, and the type's default member, on `SemanticType::Enumeration` | `Const` |

**Decisions lowering makes**

| Decision | Made today in | In the lowered program |
|---|---|---|
| Narrowing before a store | `emit_truncation` at each store site | `Truncate` |
| Logical or bitwise operator | `emit_not` and the `AND`, `OR` and `XOR` emitters, from the signedness of the operation type | Distinct operators |
| Callee | The analyzer's enum on the signature (to become `BuiltinFunction`, see [Names](#names)), dispatched by `compile_intrinsic`; then `lookup_builtin` picks a `func_id` from the operation width | `Callee`, with the `Intrinsic` for the operand types (REQ-LOW-lowering-147) |
| Implementers of a call through an interface | Not made; calls through an interface are not compiled | `Callee::Interface` (REQ-LOW-lowering-105) |
| Capacity of an intermediate string result | Codegen: a bound per expression (`string_width.rs`), and a temporary buffer as large as the largest string in the program ([issue 2118](https://github.com/ironplc/ironplc/issues/2118)) | The capacity of the `StrExpr`'s `StringShape` |
| Argument passing mode | `ParamPassing` in `compile.rs` | `Arg` variant |
| Variable and field identity | Nine name-keyed maps; lower-cased field names | `VarId`, `FieldIdx` |
| Default initial value | `emit_initial_values`; subrange lower bound, the enumeration's default member as the analyzer records it | `Assign` statements in `init` |
| Function local re-initialization ([ADR-0024](../adrs/0024-function-local-reinit-via-bytecode-prologue.md)) | `emit_function_local_prologue` | Statements at the head of the function body |
| Behaviour policy | `CodegenOptions::string_to_num` | `Intrinsic` variant |
| Target of `EXIT` and `CONTINUE` | `loop_labels` stack, with a P9998 as a fallback | `LoopId` |
| String encoding and capacity of a string value | `string_width.rs` | `StringShape` |
| Temporal literal count and unit ([ADR-0021](../adrs/0021-time-32bit-ltime-64bit.md), [ADR-0025](../adrs/0025-datetime-unsigned-representation.md)) | `compile_time_count` | `Const` |

**REQ-LOW-analyzer-091** In the `Library` that analysis returns, every
literal's `expr_type` names a type rather than a generic category, and every
operand whose type differs from the type its operation computes at is wrapped
in an `ImplicitConversion`.

**REQ-LOW-lowering-094** Lowering decides no literal type and no implicit
conversion. A `Const` has the type the analyzer recorded for its literal, a
`Convert` that stands for an implicit conversion comes from an
`ImplicitConversion` node, and an operand whose type differs from its
operation's without one is reported as P9998.

**REQ-LOW-lowering-090** Lowering reads `CompilerOptions`; a backend does not.

No analyzer rule restates a decision in order to predict its outcome. A check
that depends on a decision reads what the analyzer recorded, and so runs after
the pass that records it.

ADR-0056's order stands: the recording pass runs after the semantic rules, so
a rule sees the program as written. ADR-0056 tried the other order and lost a
diagnostic. In `DINT#300 < s` with `s : SINT`, `rule_constant_range` checks
`DINT#300` against the type of `s`, and once the pass had wrapped `s` in a
conversion to `DINT` the rule saw `DINT` and stopped reporting P2026.

Today `rule_constant_range` is the one rule that depends on a recorded
decision, so it runs after the pass and reads both:

- **The type a literal ends up with** is the type the pass recorded.
  `x := 300` with `x : USINT` is checked against it. `ExprType::Inferred`
  marks a type the analyzer gave, and `ExprType::Concrete` a type the program
  states.
- **The operand types as written** are read through the `ImplicitConversion`
  that wraps an operand. `DINT#300 < s` is checked against `SINT`.

Neither predicts a decision.

**REQ-LOW-analyzer-093** Analysis reports P2026 both for `x := 300` with
`x : USINT` and for `DINT#300 < s` with `s : SINT`.

The user-facing problems codegen raises today fall into two groups:

- **A check analysis already makes**, kept in codegen as a fallback:
  `ArrayIndexOutOfBounds` for a literal subscript. Behind the gate it is a
  P9998. Codegen already reports the other checks analysis makes, such as an
  `EXIT` outside a loop, a recursive call, a string encoding mismatch or a
  constant out of range, as P9998.
- **A limit of what the bytecode backend builds**: `TaskSingleNotSupported`,
  `TaskParameterOutOfRange`, and `NoProgramDeclaration`, since a container
  needs a program to run. It stays in that backend (REQ-LOW-codegen-113).

**REQ-LOW-analyzer-095** Analysis reports a constant subscript outside its
dimension's bounds as `ArrayIndexOutOfBounds`.

`rule_array_index_range` meets REQ-LOW-analyzer-095 for a literal subscript,
as it stands after constant folding. Neither it nor codegen checks a subscript
that names a constant.

## 5. Desugaring

A source construct that can be expressed with others is, so that no backend
implements it.

| Source | Lowered to |
|---|---|
| `ELSIF` | Nested `If` |
| `WHILE c DO b` | `Loop` whose body exits when `c` is false, then `b` |
| `REPEAT b UNTIL c` | `Loop` with body `b` and a `continuing` that exits when `c` is true |
| Bit read `x.3`, partial read `x.%B1` | Shift and mask of a `Read` |
| Bit and partial write | `AssignBits` |
| `p^ := v` | `Assign` to a place ending in `Deref` |
| `S=`, `R=` | `If` on the value around an `Assign` of `TRUE` or `FALSE` |
| `REF=` | `Assign` of a `RefTo` (already the analyzer's view) |
| Function block call with inputs and outputs | Assignments to the instance's fields, `FbCall`, assignments from them (REQ-LOW-lowering-069) |
| Method call `inst.m(a)` | `Call` of the method with `Ref(inst)` first (REQ-LOW-lowering-076) |
| Operator function form `ADD(a, b, c)` | Left fold of `Binary` |
| Typed time function `ADD_DT_TIME(a, b)` | Scalar arithmetic with the unit conversion `compile_time_arith.rs` applies today |
| Explicit conversion such as `REAL_TO_INT(x)` | A `Convert` where the operation type changes, then a `Truncate` where the storage narrows; a conversion to `BOOL` is a `Compare` with zero |
| Enumerated value | `Const` |
| `MOVE(x)`, parenthesised expression | The operand |
| Comparison of strings | `Call` of a string comparison `Intrinsic` |
| Named argument | Positional `Arg` |
| `SIZEOF(x)` | A `Const`: the bytes of `x`'s storage width, or the element bytes times the element count for an array, as codegen computes it today (`compile_sizeof`) |
| Standard function block (`TON`, `CTU_DINT`, ...) | A function block POU of the lowered program, generated by lowering (REQ-LOW-lowering-077) |

`FOR` is not desugared; it is the `For` node, whose expansion
`ironplc-ir` provides (REQ-LOW-ir-068).

**REQ-LOW-lowering-100** Reading a bit or a partial access of any place lowers
to the same shift and mask over a `Read` of that place, whatever the shape of
the place.

**REQ-LOW-lowering-101** `a + b` and `ADD(a, b)` lower to the same expression.

**REQ-LOW-lowering-102** A bit or partial write lowers to one `AssignBits` that
names its place once.

**REQ-LOW-lowering-103** `SIZEOF` lowers to a `Const` computed from the
language's storage widths, never from a backend's layout; for a type whose size
codegen does not compute today, such as a structure, it is P9999.

Codegen has already folded the cross product of access kind and base shape
into one addressing path (`compile_place.rs`) and one bit write
(`compile_partial_access.rs`). The lowered program keeps it folded: a backend
implements "address a place" once and "store bits" once.

## 6. Diagnostics

Lowering reports two kinds of problem, neither of them a problem with the
program. It accumulates both: it finishes the walk and reports every one it
found, as rules do under ADR-0048.

| Kind | Code | Meaning |
|---|---|---|
| Construct lowering cannot express yet | P9999 | Not implemented; for example a sequential function chart body, a directly represented variable in an expression, arithmetic on a reference, or a `FOR` whose step is not a constant |
| Broken invariant | P9998 | Compiler defect |

**REQ-LOW-lowering-109** Lowering reports no problem code other than P9999 and
P9998.

**REQ-LOW-lowering-110** A program with two independent problems lowering can
detect reports both in one run.

**REQ-LOW-lowering-111** A lowering diagnostic about one POU does not prevent
diagnostics about another.

**REQ-LOW-lowering-112** Lowering returns a lowered program only when it
reported nothing.

Arithmetic on a reference or pointer means something different on each
target: the bytecode VM's reference is a variable-table index
(`vm/src/value.rs`) and `ADR(x)` is `REF(x)` there, while a native target's is
a byte address. Analysis rejects it (P2033) unless `--allow-ref-arithmetic` is
set, and codegen has no support for it either way. Lowering reports it as
P9999 whatever the options allow, until a design gives it one meaning.

**REQ-LOW-lowering-114** Lowering reports arithmetic on a reference or pointer
as P9999.

A backend can then fail in exactly two ways.

**REQ-LOW-codegen-113** The bytecode backend reports a diagnostic only for a
limit of its target (a capability the VM lacks, such as `SINGLE` tasks, or a
resource bound, such as the data region size) or for a compiler defect; it
never reports that the program is invalid.

No diagnostic a user sees moves later. Every problem with the program is
reported by analysis, beside every other problem, in `check` and in the
language server. One thing moves earlier: `check` reports a construct the
compiler cannot generate yet (REQ-LOW-project-004), which today surfaces only
from `compile`. The checks only codegen made are already analyzer rules.

## 7. Backend Contract

A backend receives a lowered program and decides four things: where each
variable is stored, which instructions express each node, how each intrinsic is
implemented, and how debug information is encoded. What each node computes is
not among them; that is [Meaning of operations](#310-meaning-of-operations).

**REQ-LOW-codegen-120** The bytecode backend evaluates the operands of an
expression, the arguments of a call and the subscripts of an `Index` in the
order the lowered program lists them.

**REQ-LOW-codegen-121** The bytecode backend evaluates each place once per
statement that names it once.

**REQ-LOW-codegen-122** The bytecode backend's layout is computed once per
compilation, before any instruction is emitted, and is not modified during
emission.

**REQ-LOW-codegen-123** The bytecode backend's layout is keyed by `VarId`,
`PouId`, `TypeId` and `FieldIdx`; no layout lookup takes a name.

**REQ-LOW-codegen-124** The bytecode backend evaluates the selector of a `Case`
once.

REQ-LOW-codegen-121 and REQ-LOW-codegen-124 change what the compiler does
today. `compile_case` compiles the selector expression again for every label it
is compared against, and a bit or partial write to an array element emits the
subscripts twice. Neither is visible unless the expression has a side effect,
such as a call to a function that writes a global. Both are corrections rather
than behaviour-preserving steps. The lowered program makes both by
construction: a `Case` names its selector once, and an `AssignBits` names its
place once. While both routes exist (see
[Delivery Constraints](#delivery-constraints)), the two routes disagree only on
a program whose selector or subscript has a side effect.

How the three targets are expected to realise the same node:

| Lowered | Bytecode VM | WebAssembly (expected) | LLVM (expected) |
|---|---|---|---|
| `ScalarType` | I32, I64, F32 and F64 opcode families | `i32`, `i64`, `f32`, `f64` | `i32`, `i64`, `float`, `double` |
| `ScalarType::Ref` | A 64-bit variable-table index | A 32-bit address in linear memory | `ptr` |
| `Truncate` | `TRUNC_*` | Sign or zero extension from the narrow width | `trunc`, then `sext` or `zext` |
| Signed `Div` | `DIV_I32`, `DIV_I64`, which wrap the most negative value divided by -1 | `i32.div_s` behind a guard for that case | `sdiv` behind guards for a zero divisor and for that case |
| `Convert` from a real to an integer | `CONV_F32_TO_I32` and siblings, which saturate | `i32.trunc_sat_f32_s` and siblings | `llvm.fptosi.sat` and siblings |
| `Deref` | Load and store indirect, which trap on null | A load or store, behind an explicit null check for a nullable reference | A `load` or `store`, behind an explicit null check for a nullable reference |
| `Loop`, `Exit`, `Continue` | Labels and jumps | `loop`, `block`, `br` | Basic blocks and `br` |
| `For` | Labels and jumps, with a fused compare and branch | The expansion to `Loop` | Basic blocks, with the bounds left for LLVM's loop passes |
| `Case` | Compare and branch chain | `br_table` or a chain | `switch` |
| `Place` | One of seven load and store opcode families, chosen by layout; a field of the current instance is the slot the VM copied it into, until [issue 2120](https://github.com/ironplc/ironplc/issues/2120) | An address in linear memory, from the instance parameter for a field of the current instance | A `getelementptr` from the variable's `alloca` or global, or from the instance parameter |
| `Intrinsic` | `BUILTIN func_id` (ADR-0008) | A call to a runtime function, or inline instructions | An LLVM intrinsic such as `llvm.sqrt`, or a call to a runtime function |
| `Callee::Interface` | As pull request 1870 decides; it proposes a table of instances read by `LOAD_INSTANCE`, then a branch to a `METHOD_CALL` per implementer | `br_table` or a branch over the implementers | A `switch` over the implementers, or a call through a table of functions |
| `RoundTime` | The `uptime_us` of `run_round` | A value the host passes in | A global the runtime writes before each round |

## 8. Existing Structures

Nothing is deleted before its last reader has moved to the lowered program.

### Analyzer

| Structure | Disposition | Change |
|---|---|---|
| `SemanticContext` | Keep | None. It is lowering's input. |
| `TypeEnvironment`, `TypeId` | Keep | Lowering builds the lowered program's type table from it. |
| `SemanticType` (formerly `IntermediateType`) | Keep | Lowering reads it and does not carry `SemanticStructField::offset`, `slot_count` or the size of a reference into the lowered program. `slot_count` moves to the bytecode backend: it sizes strings with the container's `string_region_size`, which is VM layout. |
| `xform_insert_implicit_conversions` | Keep and extend | Records every implicit conversion and the type of every untyped literal (ADR-0056). It already records most of them; [issue 2050](https://github.com/ironplc/ironplc/issues/2050) finishes the rest. It is the recording pass (see [Names](#names)). |
| `rule_constant_range` | Keep | It runs after the recording pass. It checks a literal against its recorded type, and reads through a conversion to check against the operand types as written (see [Decisions](#4-decisions)). |
| `intermediates/` | Keep | Its signatures carry the `BuiltinFunction` identity. |
| `FunctionEnvironment` | Keep | Signatures of built-in functions name their `BuiltinFunction` (REQ-LOW-analyzer-070). |
| `Intrinsic` (`intrinsic.rs`) | Keep, renamed | Renamed `BuiltinFunction` (see [Names](#names)), so that `Intrinsic` names only the IR's typed enum. |
| `SymbolEnvironment` | Keep | Supplies the declarations `VarId`s are allocated from. |
| `Expr::expr_type`, `VarDecl::type_id`, `ExprKind::LateBound` | Keep | Lowering is their last reader. |

### `CompileContext`

`CompileContext` splits by what each field is.

| Fields | What they are | Disposition |
|---|---|---|
| `var_types`, `types`, `operand_names`, `intrinsics`, `compiler_options`, `string_to_num` | Language decisions and their inputs | Move to lowering. No backend holds them. |
| `variables`, `string_vars`, `array_vars`, `struct_vars`, `struct_array_vars`, `fb_instances`, `in_out_params`, `user_functions`, `user_fb_types`, `next_user_fb_type_id` | Storage layout | Keep in the bytecode backend as its layout, keyed by id, immutable during emission. |
| `constants`, `loop_labels`, `current_function_return`, `current_function_id`, `call_graph`, `data_region_offset`, `max_string_capacity`, `has_wide_string`, `debug_*` | Emission state | Keep in a smaller emit context. |

### Other codegen types

| Type | Disposition |
|---|---|
| `OpWidth`, `Signedness`, `OpType` | Move to `ironplc-ir` as `ScalarType`. |
| `VarTypeInfo` | Dissolves. Operation type is on the expression; storage width comes from the place's type. |
| `ArrayVarInfo`, `StructVarInfo`, `StructFieldInfo`, `StructArrayVarInfo`, `FbInstanceInfo`, `UserFunctionInfo`, `UserFbTypeInfo`, `UserMethodInfo`, `StringVarInfo` | Keep in the bytecode backend's layout, without their type fields (`StructFieldInfo::op_type` and `field_type`, `field_op_types`, `field_type_ids`, `param_op_types`, `element_var_type_info`). |
| `Place` (`compile_place.rs`), `ResolvedAccess` | Keep in the bytecode backend. They are that backend's addressing: the result of asking its layout how to reach a lowered `Place`. |
| `ParamPassing` | Dissolves into `Arg`, which adds the by-value aggregate mode `ParamPassing` lacks. |
| `Scope` (formerly `SavedFbScope`) | Absorbed into the bytecode backend's layout. Ids do not collide across scopes, and a function block's variables are fields of its instance. While both routes exist it is how a lowered statement finds its variables' storage (see [Delivery Constraints](#delivery-constraints)). |
| `type_info.rs`, `string_width.rs` | Move into lowering. |
| `TimeArith`, `StringConversion`, `ShortCircuitOp` | Become desugarings, `Intrinsic` variants and an `ExprKind` respectively. |
| `ClassifiedCmp` | Keep in the bytecode backend. The fused compare and branch is a VM optimization. |
| `for_loop_trunc_can_be_elided`, the fused `FOR` head | Keep in the bytecode backend. They read the bounds of a `For` node. |
| `Emitter`, `optimize/`, `stack_balance.rs`, `call_graph.rs`, `PoolConstant` | Keep, unchanged. |

### Names

The analyzer called its description of a type `IntermediateType`
(`intermediate_type.rs`), with `IntermediateStructField`,
`IntermediateFunctionParameter` and `IntermediateResult`. The name said where
the type sits, between parsing and code generation, rather than what it is. It
also took the word this design needs: the usual name for what lowering
produces is an intermediate representation.

They have been renamed, in mechanical prefactors before the IR exists:

- `IntermediateType` is `SemanticType` (`semantic_type.rs`).
- `IntermediateStructField` and `IntermediateFunctionParameter` are
  `SemanticStructField` and `SemanticFunctionParameter`.
- `IntermediateResult` is `TypeResolution`.

The builders in `intermediates/` keep their module name.

The name is `SemanticType` rather than `TypeRepr`. Compilers distinguish a
type as it is written from a type as analysis understands it: Swift calls the
first a `TypeRepr` and the second a `Type`, and rustc separates `hir::Ty`, as
written, from `ty::Ty`. `SemanticType` is the second kind, so `TypeRepr` would
name the wrong one.

Two renames remain, each one mechanical prefactor:

- The analyzer's enum of built-in functions, today `Intrinsic`
  (`intrinsic.rs`), becomes `BuiltinFunction`, so that `Intrinsic` names only
  the IR's typed enum (see
  [Position in the Pipeline](#1-position-in-the-pipeline)).
- ADR-0056 and the doc comment of `xform_insert_implicit_conversions` call
  that transform "a lowering pass". The doc comment is reworded to call it the
  recording pass, as below, and ADR-0058 and a postscript to ADR-0056 record
  the new name (see [Relationship to ADR-0056](#relationship-to-adr-0056)).

With the word free:

- The **IR** is the intermediate representation this design defines, in the
  crate `ironplc-ir`.
- A **lowered program** is a program in the IR: the output of **lowering**,
  the pass in the crate `ironplc-lowering`.
- A **backend** consumes a lowered program and nothing else.
- The analyzer transform that ADR-0056 and the doc comment of
  `xform_insert_implicit_conversions` call "a lowering pass" records
  decisions rather than lowering anything, so it is the **recording pass**.

The [glossary](../steering/glossary.md) gains these terms when this design is
approved. The area code of its requirements stays `LOW`.

## 9. Testing

**The end-to-end suite is the regression net.** `compiler/codegen/tests/it`
compiles source and asserts variable values after a run.

**REQ-LOW-codegen-131** The end-to-end helpers identify a variable by its
source name, not by its slot index.

So the tests say nothing about how the compiler is structured, and they hold
across this change without edits. A step that needs one edited is not
behaviour preserving.

**Lowering is tested on the tree.** A decision in
[Decisions](#4-decisions) is asserted by lowering a small program and
inspecting the node: a literal lowers to a `Const` of the type the analyzer
recorded; the `INT` operand of `i + r` lowers to the `Convert` its
`ImplicitConversion` asks for; an operand that needs a conversion the analyzer
did not record is a P9998; a function block call lowers to its input
assignments, an `FbCall` and its output assignments.

**Recorded decisions are tested in the analyzer.** REQ-LOW-analyzer-091 is
asserted by walking the analyzed `Library` of a corpus of programs: no literal
is left at a generic category, and no operand is left unconverted.

**Constructors are tested directly.** Each invariant a constructor checks
(REQ-LOW-ir-081) has a test that the constructor refuses the violating
operands.

**Operations are tested end to end.** Each requirement in
[Meaning of operations](#310-meaning-of-operations) has a program that
exercises it, such as the most negative `DINT` divided by -1, `REAL_TO_DINT`
of a NaN, and `SHL` by 33, and every backend runs the same programs.

**Structure is tested mechanically.** REQ-LOW-codegen-002 is a test over the
crate manifest. REQ-LOW-lowering-082 is enforced by clippy; its conformance
test asserts that each crate root carries the `deny` attribute, since a test
cannot observe a lint that is not configured.

**Standard function blocks are tested against the VM.** REQ-LOW-codegen-078
is asserted by running each standard function block both ways, through the
bytecode VM's intrinsic and through the block lowering generates, over the
same inputs and round times.

**Backends are tested against each other.** Once a second backend exists, the
end-to-end helpers run each program on both and compare variable values by
name. A disagreement is a defect in one backend, because both consumed the
same lowered program. A real value is the exception: real functions use each
target's own primitive (see [Meaning of operations](#310-meaning-of-operations)),
so two real values are compared with a near check, within a tolerance the
helpers state, and every other value must be equal.

**REQ-LOW-codegen-132** When the end-to-end helpers compare two backends, a
real value passes when it lies within the helpers' stated tolerance of the
other backend's value, and every other value passes only when it is equal.

Every end-to-end test that passes before the bytecode backend consumes the
lowered program passes after, with its source and assertions unchanged. This is
a delivery constraint rather than a requirement: it says how the work is done,
and no single test can check it.

## 10. Optimization

The compiler optimizes in two places, and this design keeps both.

| Level | Input and output | Where | Examples |
|---|---|---|---|
| Target neutral | Lowered program to lowered program | `ironplc-lowering`, after lowering and before any backend | Constant folding across statements, dead branch removal, common subexpression elimination, loop-invariant code motion, vectorization |
| Target specific | The backend's own form | The backend | The bytecode peephole optimizer (`optimize/`, [Bytecode Peephole Optimizer](bytecode-peephole-optimizer.md)), the fused compare and branch of `ClassifiedCmp`, dropping a `FOR` control variable's truncation |

A target-neutral pass runs once and benefits every backend, so an optimization
belongs there unless it depends on the target's instructions. The analyzer's
constant folding (`xform_fold_constant_expressions`) is not an optimization in
this sense: it decides values the language requires to be constant, such as
array bounds, and stays in the analyzer.

A backend with an optimizer of its own, such as LLVM, gains little from the
target-neutral passes, because its own passes do the same work with more
information. It takes the unoptimized program (REQ-LOW-lowering-142). The
target-neutral passes are for the backends without an optimizer, the bytecode
VM first.

**REQ-LOW-lowering-140** A pass over the lowered program builds its output
through the same constructors as lowering, so its output meets every
requirement of [The Lowered Program](#3-the-lowered-program).

**REQ-LOW-lowering-141** A pass over the lowered program preserves the
program's observable behaviour: the values of every variable after each scan,
the run-time traps of [Meaning of operations](#310-meaning-of-operations), and
the span of each statement it keeps.

**REQ-LOW-lowering-142** Every optimization pass over the lowered program can
be turned off, and a backend produces a correct program from the unoptimized
lowered program.

Requirement REQ-LOW-lowering-142 lets the end-to-end suite run with passes off
and on, and lets a debugger present a program as written.

### Vectorized operations

No vector node is proposed now, but nothing here rules one out. A vectorizing
pass would find loops of the canonical form "for each element of these arrays,
compute an element of that array" and replace each with a node that states the
whole operation, for example an `AssignEach { dst: Place, op, srcs: Vec<Place> }`
statement over array places. WebAssembly's 128-bit SIMD and a native backend
could implement it directly.

Two constraints follow for whoever adds one:

- **A vector node must not burden every backend** (Goal 4). It comes with an
  expansion, in `ironplc-ir`, back to the scalar `For` it replaced, so a
  backend without vector instructions (the bytecode VM today) calls the
  expansion and implements nothing new.
- **The loop must stay recognizable.** A `For` keeps its control variable,
  bounds and step explicit (REQ-LOW-lowering-066), so a pass matches the node
  rather than recovering the loop from a `Loop`.

## 11. Sequential and Graphical Languages

The compiler cannot compile any of these languages today:

- **Sequential function charts (SFC)** have an AST (`ironplc_dsl::sfc`), which
  the parser and the PLCopen XML importer produce and analysis accepts. Codegen
  refuses it with P9999 (`compile_stmt.rs`).
- **Ladder diagrams (LD), function block diagrams (FBD) and instruction lists
  (IL)** have no AST. The XML importer rejects their bodies with P9003.

None of them needs a node kind of its own in the lowered program. Each lowers
to the statements and expressions of
[The Lowered Program](#3-the-lowered-program). This section says what each
needs from the lowered program, so that adding them later changes no backend.

Decisions divide as they do in [Decisions](#4-decisions):

- **What makes a chart or a network invalid is an analyzer rule.** Examples are
  a transition to an undeclared step, an action that a step names but no one
  declares, an `L` or `D` qualifier without a duration, a chart that is unsafe
  or has unreachable steps, and an FBD feedback path with no variable to hold
  it. Analysis checks none of these today.
- **What a valid chart or network does each round is decided by lowering,
  once.** Every backend receives the result as ordinary statements.

### Sequential function charts

A chart lowers to statements at the head of its POU's body, which run every
round:

1. Each transition whose source steps are all active and whose condition holds
   is cleared. The conditions are evaluated into temporaries before any step
   changes, so every transition sees the chart as the round found it.
2. The source steps of each cleared transition are deactivated and its target
   steps activated. The priority of a selection divergence decides among
   transitions that share a source step.
3. The control of each action is updated from the qualifiers of the steps that
   name it (`N`, `R`, `S`, `L`, `D`, `P`, `P0`, `P1`, `SD`, `DS`, `SL`).
4. Each action whose control is active runs.

The action control of IEC 61131-3 also allows an action a final run in the
round its control becomes inactive. Whether and when that applies is decided
once, in lowering, when SFC support is designed.

The state a chart keeps is synthesized fields of its instance (see
[Synthesized state](#synthesized-state)): the activity `X` and elapsed time
`T` of each step, the state of each action's control, and the timers its timed
qualifiers need. A step's elapsed time comes from `RoundTime`, and `init`
activates the initial step. An action is a body that runs on the same instance,
so it lowers to a `Method` with no parameter and no result, called with its
instance. A vendor dialect that lets a program call an action as `inst.A()`
then needs nothing more than a method call.

The analyzer resolves `S1.X` and `S1.T` in a program to the step, as it
resolves any other name, and lowering turns each into a `Read` of the
synthesized field.

**REQ-LOW-lowering-150** A sequential function chart lowers to statements of
[Statements](#37-statements) over synthesized fields of its instance; the
lowered program has no node for a step, a transition or an action.

**REQ-LOW-lowering-151** Every step of a chart has synthesized fields for its
activity and its elapsed time, named after the step.

**REQ-LOW-lowering-152** An action of a chart lowers to a `Method` of the
chart's POU with no parameter and no result.

### Ladder and function block diagrams

A rung or a network lowers to the statements its elements stand for, in the
order the network is evaluated:

| Element | Lowered to |
|---|---|
| Contact, normally open or closed | A `Read` of a `BOOL`, or its `BoolNot`, combined by `And` in series and by `Or` in parallel |
| Coil | `Assign` of the rung's value |
| Set and reset coil | `If` on the rung's value around an `Assign` of `TRUE` or `FALSE`, as `S=` and `R=` |
| Rising and falling edge contact or coil | The value combined by `And` with the `BoolNot` of a synthesized field holding its previous value (or the reverse for a falling edge), then an `Assign` of the value to that field |
| Function or block box | `Call` or `FbCall`, with a temporary for each wire that feeds more than one input |
| FBD feedback path | A synthesized field, read before the network and written after it |
| `RETURN` | `Return` |

The order of evaluation within a function block diagram has observable effects
when its blocks have side effects. PLCopen XML gives the order as
`executionOrderId`. Where the source leaves it open, lowering decides it, once.

**REQ-LOW-lowering-155** Every network of a ladder or function block diagram
lowers to statements of [Statements](#37-statements); the lowered program has
no node for a rung, a contact, a coil or a wire.

### Jumps

LD, FBD and IL have jumps to labels, and so does structured text in some
vendor dialects. The lowered program has none. A forward jump lowers to an
`Exit` from a `Loop` that runs its body once. A backward jump lowers to a
`Continue` of a `Loop` around the networks between the label and the jump.

Jumps between the networks of one body can form a loop with two entries, which
nested `Loop`s cannot express. Suppose network 1 jumps to a label at network 3,
and network 4 jumps back to a label at network 2. Then networks 2 to 4 repeat,
and the repetition is entered both at network 2 and at network 3. Lowering
structures such a body with a compiler-provided temporary that names the next
network to run, and a `Loop` around a `Case` on that temporary.

The nodes of [Statements](#37-statements) express this, so it needs no new
node. It is, however, an algorithm lowering must implement: the kind
WebAssembly compilers use to structure arbitrary control flow. It is the cost
of choosing a tree over a control-flow graph (see
[Alternatives Considered](#alternatives-considered)), paid once, in lowering,
rather than by each backend. The structured result still repeats only through
`Loop` and `For`, so REQ-LOW-lowering-059 holds, and a backend checks a
deadline in the same places.

**REQ-LOW-lowering-158** A jump lowers to `Loop`, `Exit` and `Continue`, and,
where the jumps of a body form a loop with more than one entry, to a `Case` on
a compiler-provided temporary; the lowered program has no jump.

### EN and ENO

Every function and block box of a diagram has an enable input `EN` and an
enable output `ENO`, and structured text may name them as arguments. A call
with `EN` lowers to an `If` on `EN` around the call, and an assignment of
`ENO`.

`ENO` follows `EN`, and an error in the call traps as it would without `EN`.
IEC 61131-3 sets `ENO` to `FALSE` when the function meets an error, and a
program can then route around the failed block. Here such an error traps
([Meaning of operations](#310-meaning-of-operations)) and ends the round, so
`ENO` is never `FALSE` because of one. This needs nothing new: every backend
already traps, and a call means the same whether or not it wires `ENO`.
Reporting an error in `ENO` instead would need `Intrinsic` variants that
return an error beside their result, a stated result for each error (what
`DIV` gives for a zero divisor), and a call whose meaning changes with whether
`EN` is wired. It is out of scope (see [Scope](#scope)).

**REQ-LOW-lowering-160** A call with `EN` lowers to an `If` on `EN` around the
call; `ENO` is `TRUE` after the call runs and `FALSE` when `EN` is `FALSE`.

### Debugging

The line map is keyed by spans (REQ-LOW-lowering-063). A span into a PLCopen
XML file identifies an element, so a debugger can show which rung, network or
step a statement came from. The debug section already reserves tables for rung
and network maps (tags 7 and 8 in [Debugger Support](debugger-support.md)).

Animating the power flow of a rung needs the value on each wire. In the
lowered program that value is part of an expression, not a place, so showing
it would need lowering to keep each wire's value in a named temporary while
debugging. That is out of scope (see [Scope](#scope)).

## Delivery Constraints

This section constrains the order of work; it is not a work breakdown.

**Before lowering covers a construct**

- **Renames.** The renames that remain in [Names](#names) each land as one
  mechanical prefactor.
- **Decisions recorded first.** The analyzer records each decision in the
  first table of [Decisions](#4-decisions), and each check only codegen makes
  moves to an analyzer rule, before lowering covers a construct that depends
  on it. The recording pass already covers most contexts: operands,
  assignments, the arguments of functions, methods and function block calls,
  loop bounds, the results of calls and dereferences, and the types of
  literals. The contexts it does not record yet, such as a condition, a
  subscript and a function block output, come next
  ([issue 2050](https://github.com/ironplc/ironplc/issues/2050)). A statement
  that needs a decision the analyzer does not record yet stays on the route
  that reads the AST.
- **P9998 only once unreachable.** An error path of codegen becomes a P9998 in
  lowering only once it is shown unreachable from a program analysis accepts
  (see [The Clean-Analysis Gate](#2-the-clean-analysis-gate)).
- **The gate is already in place.** Codegen's entry point takes a
  `CleanAnalysis`. While both routes exist, codegen passes it on to the entry
  point that lowers one statement, so the lowered route meets
  REQ-LOW-lowering-011 from its first statement, and no caller moves again.

**Growing one statement at a time**

- **The unit of migration.** Places and expressions refer to each other (an
  `Index` holds expressions and a `Read` holds a place), and so do statements
  and expressions. So the lowered program cannot grow one node kind at a time
  while codegen reads the AST for the rest. It grows one top-level statement
  at a time instead:
  1. While codegen compiles a body, it asks lowering for each top-level
     statement, together with every statement and expression inside it.
  2. If lowering supports all of them, codegen emits the lowered statement.
  3. If not, lowering answers that the statement is unsupported, and codegen
     compiles that statement from the AST.

  Both routes emit into the same body through the same emitter. A POU is not
  the unit: most POUs mix constructs, so a POU would take the new route only
  once nearly all of lowering existed.
- **Unsupported is not a defect.** While both routes exist, lowering offers an
  entry point that lowers one statement of a body. That entry point tells a
  statement it does not support yet apart from a broken invariant. An
  unsupported statement falls back and is counted. A broken invariant is a
  P9998, as always.
- **Storage found by name at first.** A lowered variable carries its name and
  its owning POU (REQ-LOW-lowering-006). So while both routes exist, the
  bytecode backend finds a lowered variable's storage in the layout codegen
  builds today: through the current body's `Scope`, by name. A field of the
  current instance is found the same way. While the VM copies fields, that is
  the slot holding the copy (see [Instance fields](#instance-fields)).
  REQ-LOW-codegen-122 and REQ-LOW-codegen-123 are met later. Once a POU's
  statements all lower, the backend builds its layout from the POU's lowered
  declarations, keyed by id and complete before emission. Both requirements
  hold when the route that reads the AST is deleted.
- **One field model per body.** Every statement of a body reaches its
  instance's fields the same way. The bytecode backend keeps the VM's copy of
  the fields for every statement, from either route, until every statement of
  a function block type's bodies takes the lowered route. Only then can it
  switch that type to fields in place
  ([issue 2120](https://github.com/ironplc/ironplc/issues/2120)). No stop-gap
  is planned for the ways the copy differs from REQ-LOW-codegen-037 and
  REQ-LOW-codegen-038: keeping it changes nothing the refactor relies on, and
  both routes behave as today.
- **A route switch.** An option chooses the route each statement takes:
  - `Ast`: every statement from the AST.
  - `PreferLowered`: the lowered statement where lowering supports it.
  - `RequireLowered`: a statement lowering does not support is an error.

  A release uses `Ast` until the routes agree. Making `PreferLowered` the
  default is a one-line change, and so is undoing it.

**Checking the new route**

- **The routes check each other.** While both routes exist, the end-to-end
  helpers compile each program both ways and compare the values of its
  variables by name (REQ-LOW-codegen-131). A disagreement fails the test. The
  corrections named under [Backend Contract](#7-backend-contract) and the
  width of the wide counters
  ([Standard function blocks](#standard-function-blocks)) are the exception:
  they differ only for a selector or subscript with a side effect, or a counter
  past the 32-bit range, and the programs that show them are listed as known
  differences.
- **Progress cannot fall.** CI reports how many top-level statements of the
  end-to-end programs took the lowered route. The number does not fall.
- **The IR is readable from its first commit.** `ironplc-ir` writes a lowered
  program as text from its first commit, and lowering's tests compare against
  that text.
- **Behaviour preserving.** Each stage is behaviour preserving and is
  delivered as a prefactor, apart from the two corrections named under
  [Backend Contract](#7-backend-contract) and the width of the wide counters.
  The lowered route makes those corrections by construction.
- **Target neutrality shown early.** A minimal WebAssembly path (one
  `PROGRAM`, integer arithmetic, assignment) is built as soon as a program of
  that shape lowers. Its purpose is to show that nothing in the lowered
  program assumes the bytecode VM, before the rest of codegen migrates onto it.

**Retiring the old route**

- **Delete as you go.** Once a construct compiles under `RequireLowered`
  across the end-to-end suite, the code that compiles it from the AST is
  deleted. New work on a construct that lowering covers goes into lowering
  only.
- **Dependencies dropped last.** `ironplc-codegen` drops its dependencies on
  `ironplc-analyzer` and `ironplc-dsl` last, when the route that reads the AST
  is deleted.

## Alternatives Considered

**One tree, indexed by phase.** `Library<P: Phase>` with associated types, so a
checked library has a non-optional `expr_type` and an uninhabited `LateBound`.
GHC ("Trees That Grow") and Scala 3 (`Tree[T]`) do this. It removes the phase
leftovers and none of the name, call or typing work, and it changes every type
in the 9,400-line DSL crate, the derive macro, 45 rules and 18 transforms.
ADR-0013 already weighed churn in the AST against benefit.

**Side tables keyed by node id.** The AST stays syntactic and analysis results
live in maps beside it, as in rustc's `TypeckResults`, `go/types.Info` and
RuSTy's `AnnotationMapImpl`. A lookup in a map is partial, which is the
property this design sets out to remove. RuSTy's code generator makes 44
annotation lookups, 19 of them through a helper that substitutes `VOID` when
the annotation is missing. rustc avoids the same outcome by building a typed
tree (THIR) from its tables before generating code.

**A control-flow graph.** Typed instructions over basic blocks, as in rustc's
MIR. The bytecode VM and WebAssembly are stack machines, and WebAssembly's
control flow is structured, so a tree maps to each directly while a graph would
have to be restructured for WebAssembly. LLVM takes a graph, but building one
from a tree is mechanical; restructuring a graph is the hard direction. The
cost of the tree falls on source languages with jumps, which lowering must
structure ([Jumps](#jumps)). A graph also needs everything this design provides
first.

**A tree in the bytecode VM's terms.** Places as slots and data-region offsets,
callees as `func_id`s. Simpler for one backend. It would leave every decision
in [Decisions](#4-decisions) to be made again by the second.

**Decisions written into the AST, with no lowered program.** The analyzer
already inserts implicit dereferences, makes named arguments positional and,
since ADR-0056, records implicit conversions this way, and this design keeps
doing so for every decision that can make a program invalid. On its own it is
not enough. The AST still refers by name, still holds broken programs and
still carries source-only forms such as `ELSIF` and `REF=`, so the name, call,
layout and desugaring work stays in every backend, along with an error path for
each state the AST can hold.

**Lowering decides as well as translates.** Lowering would choose literal
types and conversions, and report the checks that depend on them. Lowering
runs only behind the clean-analysis gate, so those problems would appear only
once every other problem was fixed. `x := 300` on a `USINT`, which analysis
reports today beside every other problem, would disappear from `check` and the
language server whenever another error existed anywhere in the project. The
language server could also show a conversion only by running lowering, and
ADR-0056 would be superseded rather than extended.

**A validator in front of codegen.** One pass that asserts the invariants, with
accessors that unwrap. It centralises the checks and leaves every state
representable.

## Relationship to ADR-0056

ADR-0056 records implicit conversions in the analyzer. It began with the
operands of a comparison, and its postscripts have since extended the pass to
most other contexts and to the types of literals. Its drivers were one recorded
answer per expression, a language server that can show the answer without
running codegen, and backends that lower rather than decide. This design shares
all three and builds on ADR-0056:

- ADR-0056 stands, and its pass is extended until it records every implicit
  conversion and the type of every untyped literal (REQ-LOW-analyzer-091).
- Lowering translates each `ImplicitConversion` into a `Convert` and decides no
  conversion itself (REQ-LOW-lowering-094).
- The `ImplicitConversion` nodes and the recorded literal types stay in the
  analyzed tree, so the language server shows them from there, as ADR-0056
  intends.
- ADR-0056's order is kept: the recording pass runs after the semantic rules.
  `rule_constant_range` alone runs after the pass, and reads the operand types
  as written through each conversion (see [Decisions](#4-decisions)).
- ADR-0056 calls the pass "a lowering pass"; it is renamed the recording pass
  (see [Names](#names)).

[ADR-0058](../adrs/0058-the-analyzer-decides-and-lowering-translates.md)
records this, and a postscript to ADR-0056 says so; it does not supersede
ADR-0056.

## Recorded Decisions

The choice among the alternatives above is a decision, and is recorded in
these ADRs:

| ADR | Decision |
|---|---|
| [ADR-0057](../adrs/0057-backends-consume-a-target-neutral-lowered-program.md) | Code generation consumes a separate, target-neutral lowered program. |
| [ADR-0058](../adrs/0058-the-analyzer-decides-and-lowering-translates.md) | The analyzer makes and records every decision whose outcome can make a program invalid, extending ADR-0056. Lowering makes the rest, reports no problem with the program, and is part of `check`. |
| [ADR-0059](../adrs/0059-backends-depend-only-on-the-ir-crate.md) | A backend depends only on `ironplc-ir`, and the analyzer does not depend on the IR. The analyzer's `BuiltinFunction` and the IR's `Intrinsic` are separate enums, and lowering maps one to the other ([Position in the Pipeline](#1-position-in-the-pipeline)). |
| [ADR-0060](../adrs/0060-lowering-generates-the-standard-function-blocks.md) | Lowering generates each standard function block as an ordinary function block of the lowered program, so no backend implements one ([Standard function blocks](#standard-function-blocks)). |
| [ADR-0061](../adrs/0061-one-name-per-built-in-operation-and-operand-type.md) | `Intrinsic` has one variant per operation and operand type. |
| [ADR-0062](../adrs/0062-a-body-works-on-its-instances-fields-in-place.md) | A body works on its instance's fields in place; the bytecode VM's copy is removed by a change of its own ([Instance fields](#instance-fields)). |
| [ADR-0063](../adrs/0063-a-call-through-an-interface-lists-its-implementers.md) | A call through an interface lists its implementers, and each backend chooses how to dispatch ([Calls through an interface](#calls-through-an-interface)). |
| [ADR-0064](../adrs/0064-every-string-has-an-explicit-capacity.md) | Every string in the IR has an explicit capacity ([String capacity](#string-capacity)). |

Two parts of this design are described but not yet decided, so neither has an
ADR:

- **How sequential function charts and the graphical languages lower**
  ([Sequential and Graphical Languages](#11-sequential-and-graphical-languages)).
  The compiler supports none of them yet. The section records what the IR
  must allow, and the decision is made when one of them is supported.
- **How `EN` and `ENO` behave** ([EN and ENO](#en-and-eno)). The section
  records the current proposal, which may change.

## Open Questions

None.

## References

- [rustc dev guide: THIR](https://rustc-dev-guide.rust-lang.org/thir.html)
- [Real World OCaml: the compiler backend](https://dev.realworldocaml.org/compiler-backend.html)
- [Trees That Grow](https://arxiv.org/abs/1610.04799)
- [RuSTy](https://github.com/PLC-lang/rusty), `src/resolver.rs` and
  `compiler/plc_lowering`, at commit `10ead7b`
- [`go/types`](https://pkg.go.dev/go/types)
- [WebAssembly 2.0 numeric instructions](https://webassembly.github.io/spec/core/exec/numerics.html)
- [LLVM Language Reference Manual](https://llvm.org/docs/LangRef.html): the
  undefined behaviour of `sdiv`, `fptosi`, shifts and `mustprogress`
- Norman Ramsey, *Beyond Relooper: Recursive Translation of Unstructured
  Control Flow to Structured Control Flow*, ICFP 2022
