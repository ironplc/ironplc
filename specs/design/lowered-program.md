# Design: Lowered Program

status: proposed
date: 2026-10-01

## Overview

This design adds one stage between semantic analysis and code generation. The
stage, **lowering**, turns an analyzed `Library` into a **lowered program**: a
tree in which every decision the language makes has already been made and every
name already refers to the thing it names. A code generator (a **backend**)
consumes the lowered program and nothing else.

Two things motivate it.

The first is that the bytecode backend re-derives what analysis already
established. It looks variables up by name, matches standard functions by
spelling, re-checks argument counts, and asks the analyzer's overload resolver
the same question a second time. Each of those is a place where a state the
analyzer has ruled out is still representable, so each needs an error path, and
the project's rule that every enum and `Option` variant is handled directly
cannot be met without writing arms for states that cannot occur.

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
  records the implicit conversions of a comparison as
  `ExprKind::ImplicitConversion` nodes, and codegen compiles what is recorded.
  This design extends that arrangement to every decision that can make a
  program invalid; see [Relationship to ADR-0056](#relationship-to-adr-0056).

## Problem

All figures are from the non-test source of `compiler/codegen/src` at commit
`7001b4a` (about 15,000 lines, blank lines and comments included). Three
commits have changed that source since: `784ff7f` and `0a625a0`, which record
the conversions of a comparison in the analyzer (ADR-0056), and `a5bfad8`. The
figures are not re-taken, because they show the shape of the problem rather
than a target.

`codegen::compile` takes `(&Library, &SemanticContext, &CodegenOptions,
&dyn SourceLookup)`. The `Library` is the
same tree the parser built, annotated in place by the analyzer. That one type
serves three consumers with incompatible needs:

| Consumer | Needs the tree to be |
|---|---|
| `plc2plc` | Faithful to the source. `Assignment::ref_bind` exists only so the renderer can reproduce `REF=`. |
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
- Changing what an operation does. [Meaning of operations](#310-meaning-of-operations)
  records what the bytecode VM does today.
- Removing `Expr::expr_type` or `VarDecl::type_id` from the AST. The analyzer's
  rules read them.
- Any particular optimization pass. Where optimizations live, and what they
  may assume, is in scope (see [Optimization](#10-optimization)).
- A control-flow graph or any other non-tree form (see
  [Alternatives Considered](#alternatives-considered)).

---

## 1. Position in the Pipeline

```
parse ──▶ analyze ──────────▶ gate ──▶ lower ──────────────┬──▶ bytecode backend:  layout ──▶ emit
          resolve, check,     clean    translate, desugar, ├──▶ WebAssembly backend (planned)
          decide and record   only     allocate ids        └──▶ LLVM backend (possible)
          (ADR-0056)                   └── `check` ends here
```

Lowering lives in a new crate, `ironplc-lowering`, which holds both the data
model and the pass. It depends on `ironplc-analyzer` and `ironplc-dsl`. A
backend depends on `ironplc-lowering` and not on `ironplc-analyzer` or
`ironplc-parser`.

Rust lets a crate name only its direct dependencies, so a backend whose
manifest omits the analyzer cannot reach `SemanticContext`, `TypeEnvironment`
or any resolver, however convenient that would be at a given call site. The
lowered program carries its own type table (see
[Program, POUs and variables](#32-program-pous-and-variables)), so the one
analyzer type a backend needs is the intrinsic enum, which `ironplc-lowering`
re-exports.

**REQ-LOW-lowering-001** The lowering entry point takes a clean analysis (see
[The Clean-Analysis Gate](#2-the-clean-analysis-gate)) and returns either a
lowered program or a non-empty list of diagnostics.

**REQ-LOW-codegen-002** `ironplc-codegen` has no dependency on
`ironplc-analyzer` or `ironplc-parser` outside `[dev-dependencies]`.

**REQ-LOW-codegen-003** The public entry point of `ironplc-codegen` takes a
lowered program; no function in the crate takes a `Library`, a
`SemanticContext` or any type from `ironplc_dsl::common` or
`ironplc_dsl::textual`.

**REQ-LOW-project-004** `ironplc_project` runs lowering as part of what
`ironplcc check` reports, so a construct the compiler cannot generate yet is
reported without generating code.

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

The instructions themselves change during this work (see
[Backend Contract](#7-backend-contract)), so the line map is checked by what it
covers, not compared byte for byte with the one built from the `Library`.

## 2. The Clean-Analysis Gate

Today "codegen runs only on a clean analysis" is a run-time check written at
each caller: `ironplc_project::compile` tests `diagnostics.is_empty()`,
`lsp_runner.rs` tests `context.has_diagnostics()`, and the MCP server
(`mcp/src/tools/compile.rs`, `mcp/src/runner.rs`) and the benchmarks reach
codegen by their own routes. Every one of them moves behind the gate. Comments in codegen ("reaching here means analysis
was skipped") record what happens when a caller gets it wrong.

The gate makes the check a value. A clean analysis is a type that borrows the
`Library` and the `SemanticContext` and can be constructed only when the
context holds no diagnostics.

```rust
// Shape, not final API.
pub struct CleanAnalysis<'a> { library: &'a Library, context: &'a SemanticContext }

impl<'a> CleanAnalysis<'a> {
    pub fn new(library: &'a Library, context: &'a SemanticContext) -> Option<Self>;
}
```

**REQ-LOW-analyzer-010** Constructing a clean analysis from a
`SemanticContext` that holds any diagnostic fails.

**REQ-LOW-lowering-011** Lowering has no entry point that accepts a `Library`
without a clean analysis.

The gate guards against a mistake, not an adversary. It stops a caller from
forgetting the check; it does not prove that the `SemanticContext` describes
the `Library`. A caller can still build a context with no diagnostics by hand
(`SemanticContextBuilder` does exactly that for tests), or pair a context with
a library it was not built from, and the gate accepts both.

So lowering does not trust the gate further than it reaches. Behind it, a
state analysis rules out is a compiler defect, and lowering reports it as
P9998 from one place. It is never a user-facing problem, never a silent
default and never a panic, which is what keeps a hand-built or mismatched
context from crashing the compiler.

Which states analysis rules out is established case by case, not assumed.
Some of codegen's error paths are reached by programs analysis accepts. The
analyzer does not check the operand pair of a comparison, so a `DINT` compared
with a `UDINT` reaches codegen, which compiles it at the left operand's type
([Comparison Operand Type](comparison-operand-type.md)). A path like that does
not become a P9998 in lowering. The analyzer first either rejects the program
or records a decision for it.

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
    pub parameters: Vec<VarId>,        // functions and methods, in declaration order
    pub result: Option<VarId>,         // the function's or method's result variable
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
    FunctionBlock { fields: Vec<Field>, block: Block },
    Program { fields: Vec<Field>, body: PouId },
    Reference { target: TypeId },
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
`IntermediateType` carries both: `IntermediateStructField::offset` is a byte
offset, `slot_count` sizes strings with the container's `string_region_size`,
and a reference is sized as a 64-bit variable-table index. Lowering builds its
table from `IntermediateType` and carries none of those.

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
constants at compile time. Whether `init` should instead be declarative (a
value per place) is an [open question](#open-questions).

The initial values of an instance's fields are written where the instance is
declared, as statements through the place that holds it, so an instance's own
initializer (`t : MyFb := (x := 7)`) needs no further mechanism. The locals of
a function or method, and the `VAR_TEMP` of any body, are re-initialized by
statements at the head of the body.

**REQ-LOW-lowering-025** The lowered program contains only POUs reachable from
a program instance, matching what `SemanticContext::reachable` gives codegen
today.

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
| Scalar | `BOOL`, integers, reals, bit strings, time and date types, enumerations, subranges, references | `Expr`, typed by a `ScalarType` |
| String | `STRING`, `WSTRING` | `StrExpr`, typed by a `StringShape` |
| Aggregate | arrays, structures, function block and program instances | `Place` only; there is no aggregate expression |

`ScalarType` is today's `OpType`, one of four operation widths (32-bit integer,
64-bit integer, 32-bit float, 64-bit float) with a signedness, plus a
reference kind. Widths narrower than 32 bits exist only as the storage of a
place (ADR-0001). `StringShape` is today's `string_width::StringShape`: an
encoding (ADR-0034) and a capacity.

```rust
pub enum ScalarType { I32(Signedness), I64(Signedness), F32, F64, Ref }
```

A reference has no width in the lowered program. The bytecode VM stores one as
a 64-bit variable-table index (`codegen/src/type_info.rs`); a 32-bit
WebAssembly target would store a 32-bit address. Which applies is the
backend's layout.

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
to address.

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

A place says nothing about storage. Whether a variable occupies a slot, a run
of the data region or an address in linear memory is the backend's layout.
`ResolvedAccess` in `compile_array.rs` is the bytecode backend's answer to that
question and stays in that backend (see
[Existing Structures](#8-existing-structures)).

Fields are accessed in place. The bytecode VM realises that by copying an
instance's fields into slots that belong to the function block type before
running its body, and copying them back after (`compile_fn.rs`). Whether that
copy is observably different, and what follows if it is, is an
[open question](#open-questions).

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
    Null,
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
means depends on time: the expansions of the timers among the standard
function blocks (see
[Callees, arguments and intrinsics](#38-callees-arguments-and-intrinsics)), and
the step times and timed action qualifiers of a sequential function chart.
Today the time reaches only the VM's timer intrinsics (`vm.rs`), so without a
node every backend would need its own route to it. The system uptime variables
of [ADR-0030](../adrs/0030-dual-uptime-system-variables.md) hold the same time
in milliseconds, which is too coarse: the VM's TON records its start in
microseconds and converts only the elapsed time to milliseconds.

**REQ-LOW-lowering-040** Every scalar expression has a `ScalarType`; the type
is not optional and is never a generic category such as `ANY_INT`.

**REQ-LOW-lowering-041** A `Const` holds a value representable in its
`ScalarType`.

**REQ-LOW-lowering-042** The operands of a `Binary` have the type of the
`Binary` itself.

**REQ-LOW-lowering-043** The operands of a `Compare` have the same type as each
other, and the `Compare` itself has the operation type of `BOOL`.

A `ScalarType` does not say whether a value is a `BOOL`, a `UDINT` or a
`DWORD`: all three are unsigned 32-bit operations. Lowering knows which from
the source type and chooses operators accordingly (REQ-LOW-lowering-048); a
backend never needs to.

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
    Call { callee: Callee, args: Vec<Arg> },
}
```

**REQ-LOW-lowering-050** The string operands of one operation share an
encoding. Analysis reports a mismatch (`StringEncodingMismatch`, ADR-0034);
lowering reports one it meets as P9998.

**REQ-LOW-lowering-051** A string literal's characters fit its encoding
([ADR-0016](../adrs/0016-string-encoding.md)).

Temporary buffers, their pool size and their release
([ADR-0052](../adrs/0052-temp-string-buffers-released-on-consume.md)) are how
the bytecode backend evaluates a `StrExpr`. They do not appear in the lowered
program.

### 3.7 Statements

```rust
pub struct Stmt { kind: StmtKind, span: SourceSpan }

pub enum StmtKind {
    Assign { place: Place, value: Expr },
    AssignBits { place: Place, shift: u8, bits: u8, value: Expr },
    AssignStr { place: Place, value: StrExpr },
    Copy { dst: Place, src: Place },                          // whole aggregate
    Call { callee: Callee, args: Vec<Arg> },                  // result discarded
    FbCall { instance: Place, block: Block },                 // runs the body; inputs and outputs are assignments around it
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
place twice and so evaluate its subscripts twice. `compile_bit_access_assignment_on_array`
does exactly that today, emitting the flat index once for the load and again
for the store.

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

**REQ-LOW-lowering-068** `ironplc-lowering` provides the expansion of a `For`
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
pub enum Callee { User(PouId), Intrinsic(Intrinsic) }
pub enum Block  { User(PouId), Standard(StandardBlock) }   // ADR-0003

pub enum Arg {
    Value(Expr),        // scalar, by value
    Str(StrExpr),       // string, by value
    Copy(Place),        // array, structure or FB instance, by value
    Ref(Place),         // VAR_IN_OUT or a method's instance, of any class
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
default (`ParamPassing::Value(DEFAULT_OP_TYPE)` in `compile_fn.rs`). A function
whose result is an aggregate is open question 14.

A function or method may also declare `VAR_OUTPUT`, which a call assigns to a
place with `=>`. `Arg` has no mode for one. Codegen assigns outputs only for a
function block call today, and a method call ignores an output argument
(`compile_method.rs`). How an output is passed is open question 18.

A method is called like a function whose first parameter is its instance, and
the first argument of the call is a `Ref` to the receiver. `inst.m(a)` passes
`inst`; `THIS^.m(a)` passes the caller's own instance, `this^`; and
`SUPER^.m(a)` calls the base type's method with that same receiver. Each of
these is resolved statically, as the first phase of
[ADR-0041](../adrs/0041-staged-method-and-interface-dispatch.md) does today.
Calls through an interface are open question 8.

**REQ-LOW-lowering-076** The first parameter of a method is its instance
parameter, and the first argument of every call to a method is a `Ref` to the
receiver's place.

`Intrinsic` is an enum with one variant per operation the compiler implements
itself: the standard functions of IEC 61131-3 and the extensions ADR-0042
admits. The analyzer owns it, because the analyzer owns the table of standard
function signatures, and each `FunctionSignature` for a standard function names
its `Intrinsic`. Lowering copies that identity into the call. No stage after
the analyzer matches a function's name.

**REQ-LOW-analyzer-070** Every standard function signature in the
`FunctionEnvironment` identifies its `Intrinsic`.

**REQ-LOW-lowering-071** A call's arguments correspond one to one, in order, to
the callee's declared parameters.

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

`Block::Standard` names a standard function block (ADR-0003). What each one
does is stated today only by the bytecode VM's own code (`vm/src/intrinsic.rs`):
REQ-LOW-codegen-089 specifies the standard functions, not the blocks. A second
backend would need a second implementation of all ten, which is the
duplication this design exists to remove. So each standard block has an
expansion in `ironplc-lowering`: lowered statements over the fields of its
instance, with its hidden state among them as synthesized fields (see
[Synthesized state](#synthesized-state)), and `RoundTime`. The expansion is
the block's meaning. A backend implements `Block::Standard` natively, as the
bytecode VM keeps doing for speed, or by calling the expansion, as an LLVM
backend would, compiling it to native code like a user block.

**REQ-LOW-lowering-077** `ironplc-lowering` provides, for every
`StandardBlock`, an expansion into lowered statements over the block's
instance, so a backend can implement `Block::Standard` by expanding it.

**REQ-LOW-codegen-078** For every `StandardBlock`, running its expansion leaves
the instance's visible fields with the values the bytecode VM's intrinsic
leaves, for the same inputs and round times.

### 3.9 Invariants by construction

The fields of `Expr`, `StrExpr`, `Place` and `Stmt` are private to
`ironplc-lowering`. Nodes are built through constructors that check the
requirements above and return an internal error when one is violated.

**REQ-LOW-lowering-080** No lowered node can be constructed outside
`ironplc-lowering`.

**REQ-LOW-lowering-081** A constructor that receives operands violating a
requirement of this section returns a P9998 diagnostic that names the
constructor; it does not build the node.

This puts the invariants in one place, upstream of every backend. A backend
relies on them without re-checking, which is what lets its matches be total.

**REQ-LOW-lowering-082** `ironplc-lowering` and every backend crate deny
`clippy::wildcard_enum_match_arm`, so a match over a lowered enum names every
variant.

### 3.10 Meaning of operations

A lowered operation means the same on every target. Where a target's own
instruction does something else, the backend emits what makes up the
difference; the lowered program does not change. The rules below are what the
bytecode VM does today (`vm/src/vm.rs`, `vm/src/builtin.rs`), recorded so that
every other backend does the same. Each is checked by running a program, so each
belongs to the end-to-end suite that every backend runs.

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

**REQ-LOW-codegen-088** A `Read` or assignment through a `Deref` of a null
reference traps, and so does an `Index` whose subscript lies outside its
dimension's bounds ([ADR-0023](../adrs/0023-array-bounds-safety.md)).

**REQ-LOW-codegen-089** An `Intrinsic` means what the IEC 61131-3 standard
function means, with these choices where the standard leaves room: the count
of `SHL` and `SHR` is taken modulo the operation width, `SHR` fills with
zeros, and `EXPT` of an integer base by a negative exponent traps.

How precisely a real function such as `SIN` or `EXP` is computed is not
settled; see open question 17.

**REQ-LOW-codegen-096** Every `RoundTime` evaluated during one round has the
value the host gave that round.

A trap faults the whole bytecode VM today: no later statement runs, no later
task in the round runs, and the VM does not resume (`run_round`, `VmFaulted`).

**REQ-LOW-codegen-097** A trap ends the round: no statement after it runs, and
no program instance after it in the round runs.

**REQ-LOW-codegen-098** Every trap is a division by zero, a negative exponent,
a null dereference, a subscript out of bounds, a string that does not convert
or a watchdog timeout, and a backend reports each under the problem code the
bytecode VM gives it (V4001 to V4006).

What remains visible after a trap is not settled; see open question 16.

The program instances of a round run one at a time, each to completion, as the
bytecode VM's cooperative scheduler runs them (`vm/src/scheduler.rs`). The
lowered program assumes it. Two bodies that ran at once would race on the
globals they share, and the memory model of a native target, LLVM's among
them, makes such a race undefined rather than merely unordered. Whether tasks
may ever preempt each other, and what lowering would then add, is open
question 19.

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
  null and bounds checks of REQ-LOW-codegen-088 are explicit branches.
- LLVM may assume that a function marked `mustprogress` eventually returns or
  performs a volatile, atomic or I/O operation. A `FOR` whose bound is the
  largest value of its control variable's type never ends
  (REQ-LOW-lowering-066), so an LLVM backend does not mark functions
  `mustprogress`.
- `llvm.sin` and its siblings call the target's math library, and LLVM may
  fold a call with a constant operand using the build machine's library (open
  question 17).

## 4. Decisions

A decision whose outcome can make a program invalid is made by the analyzer and
recorded in the analyzed `Library`, because analysis is where `check` and the
language server look for problems. A decision no valid program can get wrong
is made by lowering. Each table's middle column is where the decision is made
today.

**Decisions the analyzer makes and records**

| Decision | Made today in | Recorded as | In the lowered program |
|---|---|---|---|
| Type of an untyped literal | The analyzer for a comparison's operands (ADR-0056); elsewhere codegen, by threading an operation type through `compile_expr`, as predicted by `rule_constant_range` | The literal's `expr_type` | A `Const` of that type |
| Implicit conversion | The analyzer for a comparison's operands (ADR-0056); `compile_arith::convert` and `compile_value_arg` for the rest | `ExprKind::ImplicitConversion` | `Convert` |
| Arithmetic overload | Analyzer's `resolve_arithmetic_overload`, asked again by `compile_arith.rs` | The expression's `expr_type`, and its operands' conversions | A `Binary` at the result type, or the desugared time arithmetic |
| Operand type of a comparison | The analyzer ([Comparison Operand Type](comparison-operand-type.md)), with a codegen fallback for a pair without one | Its operands' conversions | A `Compare` at that type |
| Argument order and count | `xform_named_to_positional_args`, then re-checked at 19 sites | Positional arguments | `Vec<Arg>` matched to parameters |

**Decisions lowering makes**

| Decision | Made today in | In the lowered program |
|---|---|---|
| Narrowing before a store | `emit_truncation` at each store site | `Truncate` |
| Logical or bitwise operator | `emit_not` and `compile_compare`, from `expr_is_bool` | Distinct operators |
| Callee | Name match in `compile_function_call` and `lookup_builtin` | `Callee` |
| Argument passing mode | `ParamPassing` in `compile.rs` | `Arg` variant |
| Variable and field identity | Nine name-keyed maps; lower-cased field names | `VarId`, `FieldIdx` |
| Enumeration ordinal | `enum_map` | `Const` |
| Default initial value | `emit_initial_values`; subrange lower bound, first enumeration value | `Assign` statements in `init` |
| Function local re-initialization ([ADR-0024](../adrs/0024-function-local-reinit-via-bytecode-prologue.md)) | `emit_function_local_prologue` | Statements at the head of the function body |
| Behaviour policy | `CodegenOptions::string_to_num` | `Intrinsic` variant |
| Target of `EXIT` and `CONTINUE` | `loop_labels` stack, with `ExitOutsideLoop` as a fallback | `LoopId` |
| String encoding and capacity of a string value | `string_width.rs` | `StringShape` |
| Temporal literal count and unit ([ADR-0021](../adrs/0021-time-32bit-ltime-64bit.md), [ADR-0025](../adrs/0025-datetime-unsigned-representation.md)) | `compile_time_count` | `Const` |

**REQ-LOW-analyzer-091** In the `Library` that analysis returns, every literal
has a concrete `expr_type`, and every operand whose type differs from the type
its operation computes at is wrapped in an `ImplicitConversion`.

**REQ-LOW-lowering-094** Lowering decides no literal type and no implicit
conversion. A `Const` has the type the analyzer recorded for its literal, a
`Convert` that stands for an implicit conversion comes from an
`ImplicitConversion` node, and an operand whose type differs from its
operation's without one is reported as P9998.

**REQ-LOW-lowering-090** Lowering reads `CompilerOptions`; a backend does not.

No analyzer rule restates a decision in order to predict its outcome. A check
that depends on a decision reads what the analyzer recorded, and so runs after
the pass that records it. That is what removes the type push-down from
`rule_constant_range`. Which of that rule's checks run after the recording
pass, and which keep checking the program as written, is an
[open question](#open-questions): ADR-0056 found that running the pass before
the rules changed what they saw.

The user-facing problems codegen raises today fall into three groups:

- **A check analysis already makes**, kept in codegen as a fallback:
  `VariableUndefined`, `ExitOutsideLoop`, `ContinueOutsideLoop`,
  `RecursiveCycle`, `StringEncodingMismatch`, and `ConstantOverflow` where
  `rule_constant_range` covers the same site. Behind the gate it is a P9998.
- **A check only codegen makes**: `ArrayIndexOutOfBounds` for a constant
  subscript, and `ConstantOverflow` where `rule_constant_range` does not cover
  the site. It moves to an analyzer rule.
- **A limit of what the bytecode backend builds**: `TaskSingleNotSupported`,
  `TaskParameterOutOfRange`, and `NoProgramDeclaration`, since a container
  needs a program to run. It stays in that backend (REQ-LOW-codegen-113).

**REQ-LOW-analyzer-095** Analysis reports a constant subscript outside its
dimension's bounds as `ArrayIndexOutOfBounds`.

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

`FOR` is not desugared; it is the `For` node, whose expansion
`ironplc-lowering` provides (REQ-LOW-lowering-068).

**REQ-LOW-lowering-100** Reading a bit or a partial access of any place lowers
to the same shift and mask over a `Read` of that place, whatever the shape of
the place.

**REQ-LOW-lowering-101** `a + b` and `ADD(a, b)` lower to the same expression.

**REQ-LOW-lowering-102** A bit or partial write lowers to one `AssignBits` that
names its place once.

The cross product in `compile_expr.rs` disappears because access kind and base
shape stop multiplying: a backend implements "address a place" once and "store
bits" once.

## 6. Diagnostics

Lowering reports two kinds of problem, neither of them a problem with the
program. It accumulates both: it finishes the walk and reports every one it
found, as rules do under ADR-0048.

| Kind | Code | Meaning |
|---|---|---|
| Construct lowering cannot express yet | P9999 | Not implemented; for example a sequential function chart body, a directly represented variable in an expression, or a `FOR` whose step is not a constant |
| Broken invariant | P9998 | Compiler defect |

**REQ-LOW-lowering-109** Lowering reports no problem code other than P9999 and
P9998.

**REQ-LOW-lowering-110** A program with two independent problems lowering can
detect reports both in one run.

**REQ-LOW-lowering-111** A lowering diagnostic about one POU does not prevent
diagnostics about another.

**REQ-LOW-lowering-112** Lowering returns a lowered program only when it
reported nothing.

A backend can then fail in exactly two ways.

**REQ-LOW-codegen-113** The bytecode backend reports a diagnostic only for a
limit of its target (a capability the VM lacks, such as `SINGLE` tasks, or a
resource bound, such as the data region size) or for a compiler defect; it
never reports that the program is invalid.

No diagnostic a user sees moves later. Every problem with the program is
reported by analysis, beside every other problem, in `check` and in the
language server. Two things move earlier: the checks only codegen makes today
become analyzer rules, and `check` reports a construct the compiler cannot
generate yet (REQ-LOW-project-004), which today surfaces only from `compile`.

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
than behaviour-preserving steps and are delivered as such.

How the three targets are expected to realise the same node:

| Lowered | Bytecode VM | WebAssembly (expected) | LLVM (expected) |
|---|---|---|---|
| `ScalarType` | I32, I64, F32 and F64 opcode families | `i32`, `i64`, `f32`, `f64` | `i32`, `i64`, `float`, `double` |
| `ScalarType::Ref` | A 64-bit variable-table index | A 32-bit address in linear memory | `ptr` |
| `Truncate` | `TRUNC_*` | Sign or zero extension from the narrow width | `trunc`, then `sext` or `zext` |
| Signed `Div` | `DIV_I32`, `DIV_I64`, which wrap the most negative value divided by -1 | `i32.div_s` behind a guard for that case | `sdiv` behind guards for a zero divisor and for that case |
| `Convert` from a real to an integer | `CONV_F32_TO_I32` and siblings, which saturate | `i32.trunc_sat_f32_s` and siblings | `llvm.fptosi.sat` and siblings |
| `Deref` | Load and store indirect, which trap on null | A load or store behind an explicit null check | A `load` or `store` behind an explicit null check |
| `Loop`, `Exit`, `Continue` | Labels and jumps | `loop`, `block`, `br` | Basic blocks and `br` |
| `For` | Labels and jumps, with a fused compare and branch | The expansion to `Loop` | Basic blocks, with the bounds left for LLVM's loop passes |
| `Case` | Compare and branch chain | `br_table` or a chain | `switch` |
| `Place` | One of seven load and store opcode families, chosen by layout; a field of the current instance is the slot the VM copied it into | An address in linear memory, from the instance parameter for a field of the current instance | A `getelementptr` from the variable's `alloca` or global, or from the instance parameter |
| `Intrinsic` | `BUILTIN func_id` (ADR-0008) | A call to a runtime function, or inline instructions | An LLVM intrinsic such as `llvm.sqrt`, or a call to a runtime function |
| `Block::Standard` | `FB_CALL` with a standard type id (ADR-0003) | A call to a runtime function, or its expansion | Its expansion, compiled like a user block |
| `RoundTime` | The `uptime_us` of `run_round` | A value the host passes in | A global the runtime writes before each round |

## 8. Existing Structures

Nothing is deleted before its last reader has moved to the lowered program.

### Analyzer

| Structure | Disposition | Change |
|---|---|---|
| `SemanticContext` | Keep | None. It is lowering's input. |
| `TypeEnvironment`, `TypeId` | Keep | Lowering builds the lowered program's type table from it. |
| `IntermediateType` | Keep | Lowering reads it and does not carry `IntermediateStructField::offset`, `slot_count` or the size of a reference into the lowered program. `slot_count` moves to the bytecode backend: it sizes strings with the container's `string_region_size`, which is VM layout. |
| `xform_insert_implicit_conversions` | Keep and extend | Records every implicit conversion and the type of every untyped literal (ADR-0056), not only those of a comparison. |
| `rule_constant_range` | Keep | Reads the recorded literal types instead of predicting them. |
| `intermediates/` | Keep | `stdlib_function.rs` gains the `Intrinsic` identity. |
| `FunctionEnvironment` | Keep | Signatures of standard functions name their `Intrinsic`. |
| `SymbolEnvironment` | Keep | Supplies the declarations `VarId`s are allocated from. |
| `Expr::expr_type`, `VarDecl::type_id`, `ExprKind::LateBound` | Keep | Lowering is their last reader. |

### `CompileContext`

`CompileContext` splits by what each field is.

| Fields | What they are | Disposition |
|---|---|---|
| `var_types`, `types`, `operand_names`, `enum_map`, `compiler_options`, `string_to_num` | Language decisions and their inputs | Move to lowering. No backend holds them. |
| `variables`, `string_vars`, `array_vars`, `struct_vars`, `struct_array_vars`, `fb_instances`, `in_out_params`, `user_functions`, `user_fb_types` | Storage layout | Keep in the bytecode backend as its layout, keyed by id, immutable during emission. |
| `constants`, `loop_labels`, `current_function_return`, `current_function_id`, `call_graph`, `data_region_offset`, `max_string_capacity`, `has_wide_string`, `debug_*` | Emission state | Keep in a smaller emit context. |

### Other codegen types

| Type | Disposition |
|---|---|
| `OpWidth`, `Signedness`, `OpType` | Move to `ironplc-lowering` as `ScalarType`. |
| `VarTypeInfo` | Dissolves. Operation type is on the expression; storage width comes from the place's type. |
| `ArrayVarInfo`, `StructVarInfo`, `StructFieldInfo`, `StructArrayVarInfo`, `FbInstanceInfo`, `UserFunctionInfo`, `UserFbTypeInfo`, `UserMethodInfo`, `StringVarInfo` | Keep in the bytecode backend's layout, without their type fields (`StructFieldInfo::op_type`, `field_op_types`, `param_op_types`, `element_var_type_info`). |
| `ResolvedAccess` | Keep in the bytecode backend. It is that backend's addressing mode: the result of asking its layout how to reach a `Place`. |
| `ParamPassing` | Dissolves into `Arg`, which adds the by-value aggregate mode `ParamPassing` lacks. |
| `SavedFbScope` | Delete. Ids do not collide across scopes, and a function block's variables are fields of its instance. |
| `type_info.rs`, `string_width.rs` | Move into lowering. |
| `TimeArith`, `StringConversion`, `ShortCircuitOp` | Become desugarings, `Intrinsic` variants and an `ExprKind` respectively. |
| `ClassifiedCmp` | Keep in the bytecode backend. The fused compare and branch is a VM optimization. |
| `for_loop_trunc_can_be_elided`, the fused `FOR` head | Keep in the bytecode backend. They read the bounds of a `For` node. |
| `Emitter`, `optimize/`, `stack_balance.rs`, `call_graph.rs`, `PoolConstant` | Keep, unchanged. |

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
(REQ-LOW-lowering-081) has a test that the constructor refuses the violating
operands.

**Operations are tested end to end.** Each requirement in
[Meaning of operations](#310-meaning-of-operations) has a program that
exercises it, such as the most negative `DINT` divided by -1, `REAL_TO_DINT`
of a NaN, and `SHL` by 33, and every backend runs the same programs.

**Structure is tested mechanically.** REQ-LOW-codegen-002 is a test over the
crate manifest. REQ-LOW-lowering-082 is enforced by clippy; its conformance
test asserts that each crate root carries the `deny` attribute, since a test
cannot observe a lint that is not configured.

**Standard block expansions are tested against the VM.** REQ-LOW-codegen-078
is asserted by running each standard function block both ways, through the
bytecode VM's intrinsic and through its expansion, over the same inputs and
round times.

**Backends are tested against each other.** Once a second backend exists, the
end-to-end helpers run each program on both and compare variable values by
name. A disagreement is a defect in one backend, because both consumed the
same lowered program, unless it is within the precision open question 17
settles on for real functions.

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
  expansion, in `ironplc-lowering`, back to the scalar `For` it replaced, so a
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

IEC 61131-3 sets `ENO` to `FALSE` when the function meets an error, while
[Meaning of operations](#310-meaning-of-operations) says such an operation
traps. Which applies is open question 20.

### Debugging

The line map is keyed by spans (REQ-LOW-lowering-063). A span into a PLCopen
XML file identifies an element, so a debugger can show which rung, network or
step a statement came from. The debug section already reserves tables for rung
and network maps (tags 7 and 8 in [Debugger Support](debugger-support.md)).

Animating the power flow of a rung needs the value on each wire. In the
lowered program that value is part of an expression, not a place; see open
question 21.

## Delivery Constraints

This section constrains the order of work; it is not a work breakdown.

- The analyzer records each decision in the first table of
  [Decisions](#4-decisions), and each check only codegen makes moves to an
  analyzer rule, before lowering covers a construct that depends on it. This
  continues ADR-0056's own order: arithmetic operands, assignments and
  arguments, then literal types everywhere.
- An error path of codegen becomes a P9998 in lowering only once it is shown
  unreachable from a program analysis accepts (see
  [The Clean-Analysis Gate](#2-the-clean-analysis-gate)).
- Places and expressions refer to each other (an `Index` holds expressions and
  a `Read` holds a place), and so do statements and expressions, so the
  lowered program cannot grow one node kind at a time while codegen reads the
  AST for the rest. It grows by POU instead. Declarations (types, variables,
  fields and POU signatures) are lowered first, for the whole program, and the
  bytecode backend builds its layout from them once. Codegen then compiles a
  POU from its lowered form when lowering supports every construct in it, and
  from the AST otherwise, reading the same layout through name-keyed views.
  Both routes emit through the same layout and the same emitter.
- While both routes exist, the end-to-end suite runs twice: once with every
  POU compiled from the AST, and once with lowering preferred. Both runs must
  pass, so the route being retired checks the route replacing it.
- Each stage is behaviour preserving and is delivered as a prefactor, with the
  two exceptions named under [Backend Contract](#7-backend-contract).
- A minimal WebAssembly path (one `PROGRAM`, integer arithmetic, assignment) is
  built as soon as a POU of that shape lowers. Its purpose is to show that
  nothing in the lowered program assumes the bytecode VM, before the rest of
  codegen migrates onto it.
- `ironplc-codegen` drops its dependency on `ironplc-analyzer` last, when the
  route that reads the AST is deleted.

## Alternatives Considered

**One tree, indexed by phase.** `Library<P: Phase>` with associated types, so a
checked library has a non-optional `expr_type` and an uninhabited `LateBound`.
GHC ("Trees That Grow") and Scala 3 (`Tree[T]`) do this. It removes the phase
leftovers and none of the name, call or typing work, and it changes every type
in the 9,400-line DSL crate, the derive macro, 44 rules and 18 transforms.
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

ADR-0056 records implicit conversions in the analyzer, and names arithmetic
operands, assignments and function arguments as the next to move there. Its
drivers were one recorded answer per expression, a language server that can
show the answer without running codegen, and backends that lower rather than
decide. This design shares all three and builds on ADR-0056:

- ADR-0056 stands, and its pass is extended past what it names: it records
  every implicit conversion and the type of every untyped literal
  (REQ-LOW-analyzer-091).
- Lowering translates each `ImplicitConversion` into a `Convert` and decides no
  conversion itself (REQ-LOW-lowering-094).
- The `ImplicitConversion` nodes and the recorded literal types stay in the
  analyzed tree, so the language server shows them from there, as ADR-0056
  intends.

The ADR that records this design's decisions amends ADR-0056 to say so; it
does not supersede it.

## Decisions to Record

The choice among the alternatives above is a decision, and belongs in ADRs
that this document then cites. Six decisions are separable:

1. Code generation consumes a separate, target-neutral lowered program.
2. The analyzer makes and records every decision whose outcome can make a
   program invalid, extending ADR-0056. Lowering makes the rest, reports no
   problem with the program, and is part of `check`.
3. A backend does not depend on the analyzer.
4. Each lowered operation has one meaning on every target, which is the
   bytecode VM's today ([Meaning of operations](#310-meaning-of-operations)).
   That meaning is total: no operation is undefined for any operand.
5. Each standard function block has one meaning, given by its expansion in
   `ironplc-lowering`, which a backend may implement natively instead.
6. Sequential function charts and the graphical languages lower to the nodes
   of the lowered program, with the state they keep as synthesized fields and
   their jumps structured by lowering
   ([Sequential and Graphical Languages](#11-sequential-and-graphical-languages)).

## Open Questions

1. **Names.** "Lowered program", the crate `ironplc-lowering` and the area code
   `LOW` are proposals. The repository already uses "intermediate" for
   `IntermediateType` and `intermediates/`, so that word is avoided here.
   "Lowering", "lowered program" and "backend" are not in the
   [glossary](../steering/glossary.md) and would be added to it. ADR-0056 and
   the doc comment of `xform_insert_implicit_conversions` call that analyzer
   transform "a lowering pass". It stays in the analyzer, so one of the two
   names has to change.
2. **One crate or two.** A backend needs `Intrinsic`, which the analyzer owns.
   This design re-exports it. The alternative is a small types crate below the
   analyzer. Relatedly, REQ-LOW-codegen-003 forbids `ironplc_dsl::common` and
   `ironplc_dsl::textual` by test; splitting `ironplc_dsl::core` and
   `diagnostic` into their own crate would forbid them by manifest.
3. **Layout-dependent values.** `SIZEOF` returns a size. If backends lay types
   out differently, either the language defines the size independently of
   layout or programs diverge. References have the same problem. The bytecode
   VM stores one as a variable-table index (`vm/src/value.rs`) and treats
   `ADR(x)` as `REF(x)`, while a native target stores a byte address. So
   pointer arithmetic, which analysis rejects today (P2033), would mean
   something different on each.
4. **`rule_constant_range`.** Its type push-down gives way to the recorded
   literal types. Which of its checks run after the pass that records them,
   and which keep checking the program as written.
5. **Unreachable POUs.** Analysis reports problems in every POU, but lowering
   covers reachable POUs (REQ-LOW-lowering-025), so `check` reports a construct
   the compiler cannot generate yet only where it is reachable. Whether `check`
   lowers every POU, so that these also appear in library code with no
   configuration.
6. **Capability checking.** Whether a backend's "the target cannot do this"
   diagnostics come from emission or from a separate pass that runs first.
7. **Strings.** Whether `StringShape::capacity` is a language property or a
   bytecode VM concern, and how a function returning a string is represented.
   The hidden `Ref` of open question 14 would answer the second.
8. **Interface calls and properties.** Static method calls are settled
   ([Callees, arguments and intrinsics](#38-callees-arguments-and-intrinsics)).
   How a call through an interface appears under the later phases of ADR-0041,
   and how a property's accessors are called.
9. **Array indexing.** Whether the flat index and stride computation is shared
   or belongs to each backend's layout.
10. **`Intrinsic` granularity.** One variant per operation with the type on the
    call, or one per operation and type as the VM's `func_id` table has today.
11. **Reference checks.** Whether a `Deref` of a `VAR_IN_OUT` parameter or of
    an instance parameter, which is never null, is distinguished from a
    `Deref` of a `REF_TO`. Distinguishing them would let an LLVM backend mark
    the pointer `nonnull` and `dereferenceable` and drop the check.
12. **Debug information.** REQ-LOW-lowering-006 lists what the debug section
    is built from today. Whether a debugger that steps through expressions, or
    shows variables an optimization removed, needs more than that.
13. **Language server.** The language server needs lowering only to show a
    construct the compiler cannot generate yet. Whether it runs lowering, and
    on which edits.
14. **Aggregate results.** A user function may return a structure today
    (`UserFunctionInfo::return_struct_desc_index`, `compile_aggregate.rs`).
    The lowered program has no aggregate expression, `Copy` takes a `Place`
    as its source, and the `Call` statement discards its result, so `s := f()`
    has no representation. One option is for lowering to give the call a
    destination place (`CallInto { dst: Place, callee, args }`); another is to
    pass the result variable as a hidden `Ref` argument. The hidden `Ref` is
    how LLVM and the C ABI return an aggregate (`sret`), and it would also
    answer a string result (open question 7).
15. **Initial values.** Whether `init` stays a list of statements, as the
    bytecode VM applies it, or becomes declarative (a value per place), which
    a backend with a data image could place without evaluating anything. A
    declarative `init` maps directly onto LLVM's global initializers. Related:
    how a warm restart, which keeps the variables declared `RETAIN`
    (REQ-LOW-lowering-024), is expressed. One `init` could skip the retained
    variables on a warm restart, or there could be separate lists for a cold
    and a warm restart. The bytecode VM performs only a cold restart today.
16. **Aliasing an instance field.** Fields are accessed in place
    ([Places](#34-places)), while the bytecode VM copies an instance's fields
    into the function block type's slots for its body and back after. The two
    agree unless something reaches an instance field by reference while that
    instance's body runs, for example a `VAR_IN_OUT` argument or a `REF_TO`
    bound to `inst.x` and used inside `inst`'s body or one of its methods.
    Whether analysis rules that out, or the bytecode backend must address
    fields in place, is open. The copy also decides what a trap leaves behind.
    The VM copies the fields back only when the body returns
    (`handle_frame_return`), so a trap inside the body leaves the instance's
    fields as they were before the call. A backend that addresses fields in
    place leaves the writes made before the trap. Whether the values written
    before a trap are part of a round's observable behaviour is open with it.
17. **Precision of real functions.** The bytecode VM computes `SIN`, `LN`,
    `EXP` and their siblings with Rust's `f64` methods, which call the
    platform's math library (`vm/src/builtin.rs`). Another backend uses
    another library. The WebAssembly target proposed in
    [pull request 1846](https://github.com/ironplc/ironplc/pull/1846) differed
    in the last bit of `LOG` and `EXP` in 2 of 942 programs. LLVM may also fold
    a call with a constant operand using the build machine's library, so one
    program can give different results at two optimization levels. Either
    every backend uses one implementation, for example one written in Rust and
    shipped with each runtime, or REQ-LOW-codegen-089 states a precision and
    the comparison of backends in [Testing](#9-testing) allows it.
18. **Function outputs.** A function or method may declare `VAR_OUTPUT`, and
    `Arg` has no mode for one. A `Ref` would let the callee write the caller's
    place while it runs, whereas an output is assigned when the call returns,
    and possibly through a `Convert`. One option treats it as a function block
    call treats an output (REQ-LOW-lowering-069): the callee writes a
    temporary passed by `Ref`, and the call is followed by an assignment from
    that temporary. An LLVM backend would realise it as a pointer to the
    temporary.
19. **Preemptive tasks.** Program instances run one at a time today (see
    [Meaning of operations](#310-meaning-of-operations)). A native runtime
    could run each task on its own thread, with a task of higher priority
    preempting one of lower priority, as many PLC runtimes do. A global used by
    two tasks would then need either a copy per task, taken at the start of its
    round and written back at its end, or accesses the target makes atomic.
    [61131 Task Support](61131-task-support.md) reserves room for per-task
    images. Lowering knows which program instance runs in which task and which
    globals each POU reaches, so it is where either would be decided.
20. **`ENO` on error.** IEC 61131-3 sets `ENO` to `FALSE` when a function
    meets an error, where [Meaning of operations](#310-meaning-of-operations)
    traps. Either `ENO` follows `EN` and an error still traps, which needs
    nothing new, or an operation called with `EN` reports its error as a value,
    which needs `Intrinsic` variants that return one.
21. **Power flow.** Showing the power flow of a rung, or the value on each wire
    of a network, needs those values while the program runs. In the lowered
    program they are parts of expressions, not places. Whether lowering keeps
    them in named temporaries when debugging is on, and what that costs an
    optimizer, is open.

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
