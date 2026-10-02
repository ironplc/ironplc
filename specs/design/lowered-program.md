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

The second is that a backend emitting WebAssembly is planned. Whatever the
bytecode backend decides for itself today, a second backend would have to decide
again, and two implementations of one decision can disagree. A program that
behaves differently on two targets is a safety defect in a PLC, which is what
[ADR-0005](../adrs/0005-safety-first-design-principle.md) exists to prevent.

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
- **[ADR-0056](../adrs/0056-analyzer-records-implicit-conversions-in-the-ast.md)**
  and **[Implicit Conversions](implicit-conversions.md)**: the analyzer records
  the implicit conversions of a comparison as `ExprKind::ImplicitConversion`
  nodes, and planned to move arithmetic operands, assignments and arguments to
  the same pass. This design changes where the rest of that work lands; see
  [Relationship to ADR-0056](#relationship-to-adr-0056).

## Problem

All figures are from the non-test source of `compiler/codegen/src` at commit
`7001b4a` (about 15,000 lines, blank lines and comments included). Of the
commits since, only `784ff7f` (ADR-0056) touched that source, and it changed
none of the counts below.

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
   through `compile_expr`. Implicit conversions are chosen in `compile_arith.rs`
   and `compile_value_arg`.
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

A second backend would add a third copy of each.

Two further consequences:

- `compile` returns `Result<Container, Diagnostic>`, so a program with five
  unsupported constructs reports one per run.
- `ironplcc check` stops after analysis, so a problem found only during code
  generation is not reported by `check`.

## Design Goals

1. **One implementation of each language decision.** What a program means is
   decided once, upstream of every backend.
2. **States that cannot occur cannot be written.** A backend's matches over
   the lowered program are exhaustive without wildcard arms, and none of the
   arms is an error path for a state analysis has ruled out.
3. **Target neutral.** Nothing in the lowered program names a slot, an offset,
   an opcode or a function id of the bytecode VM.
4. **Small.** Each construct in the lowered program is a construct every
   backend must implement, so source constructs that can be expressed with
   others are.
5. **Analysis output stays what it is.** The `Library` remains source faithful
   and remains able to hold a broken program. The analyzer's passes and rules
   are not restructured by this design.
6. **Deliverable in behaviour-preserving steps.** The existing end-to-end
   tests pass unchanged at every step.

## Scope

**In scope:** the lowered program's data model, the lowering stage, the gate
between analysis and lowering, the contract a backend works to, what becomes of
the structures in `compiler/codegen` today, and how the result is tested.

**Out of scope:**

- The WebAssembly backend itself. It is a constraint on this design, not part
  of it.
- Any change to the bytecode instruction set, the container format or the VM.
- Removing `Expr::expr_type` or `VarDecl::type_id` from the AST. The analyzer's
  rules read them.
- Optimization passes over the lowered program.
- A control-flow graph or any other non-tree form (see
  [Alternatives Considered](#alternatives-considered)).

---

## 1. Position in the Pipeline

```
parse ──▶ analyze ──▶ gate ──▶ lower ──────────────┬──▶ bytecode backend:  layout ──▶ emit
          Library +   clean    decide, desugar,    │
          Semantic    only     check, accumulate   └──▶ WebAssembly backend (planned)
          Context              diagnostics
                               └── `check` ends here
```

Lowering lives in a new crate, `ironplc-lowering`, which holds both the data
model and the pass. It depends on `ironplc-analyzer` and `ironplc-dsl`. A
backend depends on `ironplc-lowering` and not on `ironplc-analyzer` or
`ironplc-parser`.

Rust lets a crate name only its direct dependencies, so a backend whose
manifest omits the analyzer cannot reach `SemanticContext`, `TypeEnvironment`
or any resolver, however convenient that would be at a given call site. The
types a backend legitimately needs from the analyzer (the representation of a
type, the intrinsic enum) are re-exported by `ironplc-lowering`.

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
`ironplcc check` and the language server report, so a problem lowering finds is
reported without generating code.

**REQ-LOW-project-005** `ironplc_project::compile` passes the lowered program
that `check` produced to the backend; it does not lower a second time.

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

**REQ-LOW-analyzer-010** A clean analysis cannot be constructed from a
`SemanticContext` that holds any diagnostic.

**REQ-LOW-lowering-011** Lowering has no entry point that accepts a `Library`
without a clean analysis.

Because lowering runs only behind the gate, a state analysis rules out is a
compiler defect when lowering meets it, and is reported as P9998 from one
place. It is never a user-facing problem and never a silent default.

## 3. The Lowered Program

The Rust in this section shows shape. Field lists are indicative; the
invariants stated as requirements are the specification.

### 3.1 Identity

Everything is referred to by an id allocated during lowering, never by name.

| Id | Names | Replaces |
|---|---|---|
| `VarId` | One declared variable in one scope, or a compiler-provided temporary | `Id` looked up in `CompileContext::variables` and six sibling maps |
| `PouId` | A program, function, function block or method | The lower-case key of `user_functions`, the upper-case key of `user_fb_types` |
| `FieldIdx` | A field of a structure or function block, by position | The lower-case field name in `field_index` and `field_indices` |
| `LoopId` | One loop | The top of the `loop_labels` stack |
| `TypeId` | A type (ADR-0055, unchanged) | Unchanged |

**REQ-LOW-lowering-020** Two variables with the same name in different scopes
have different `VarId`s.

**REQ-LOW-lowering-021** No node of the lowered program holds an `Id` or a
string that a backend must resolve; names appear only on the declarations of
variables, POUs, fields and types, for debug information and diagnostics.

### 3.2 Program, POUs and variables

```rust
pub struct Program {
    pub types: TypeTable,              // TypeId -> representation and declared name
    pub variables: Vec<Variable>,      // indexed by VarId
    pub pous: Vec<Pou>,                // indexed by PouId
    pub global_init: Vec<Stmt>,        // initial values of globals
    pub instances: Vec<ProgramInstance>,
    pub tasks: Vec<Task>,
}

pub struct Variable {
    pub name: Id,
    pub owner: Owner,                  // Global or Pou(PouId)
    pub section: Section,              // Local, Input, Output, InOut, Global, External, Temporary
    pub ty: TypeId,
    pub span: SourceSpan,
}

pub struct Pou {
    pub name: Id,
    pub kind: PouKind,                 // Program, Function, FunctionBlock, Method { of: PouId }
    pub parameters: Vec<VarId>,        // in declaration order
    pub result: Option<VarId>,         // the function's result variable
    pub locals: Vec<VarId>,
    pub init: Vec<Stmt>,               // initial values (ADR-0045)
    pub body: Vec<Stmt>,
    pub span: SourceSpan,
}
```

**REQ-LOW-lowering-025** The lowered program contains only POUs reachable from
a program instance, matching what `SemanticContext::reachable` gives codegen
today.

**REQ-LOW-lowering-026** Every variable's `ty` is present in the type table.

A function's result is a variable, as in the source, where the result is
assigned through the function's name. `Return` carries no value.

### 3.3 Value classes

A value belongs to exactly one of three classes, and each class has its own
representation. Keeping them apart is what stops an addition of two strings or
a comparison of two arrays from being writable.

| Class | Types | Represented by |
|---|---|---|
| Scalar | `BOOL`, integers, reals, bit strings, time and date types, enumerations, subranges, references | `Expr`, typed by a `ScalarType` |
| String | `STRING`, `WSTRING` | `StrExpr`, typed by a `StringShape` |
| Aggregate | arrays, structures, function block instances | `Place` only; there is no aggregate expression |

`ScalarType` is today's `OpType`: one of four operation widths (32-bit integer,
64-bit integer, 32-bit float, 64-bit float) and a signedness. Widths narrower
than 32 bits exist only as the storage of a place (ADR-0001). `StringShape` is
today's `string_width::StringShape`: an encoding (ADR-0034) and a capacity.

These are also the four value types of WebAssembly, with signedness on the
operation in both targets, which is why the scalar model needs no translation
layer for the planned backend.

### 3.4 Places

A place is where a value lives: a variable and a path into it.

```rust
pub struct Place { pub root: VarId, pub path: Vec<Projection>, pub ty: TypeId }

pub enum Projection {
    Field(FieldIdx),      // structure or function block field
    Index(Vec<Expr>),     // one subscript per dimension
    Deref,                // through a reference
}
```

`ty` is the type of the located value, so a backend never walks the path to
learn what it is loading.

**REQ-LOW-lowering-030** A `Field` projection applies only to a place whose
type is a structure or function block, and its index is within that type's
field list.

**REQ-LOW-lowering-031** An `Index` projection applies only to a place whose
type is an array, and carries exactly one subscript per dimension.

**REQ-LOW-lowering-032** A `Deref` projection applies only to a place whose
type is a reference.

**REQ-LOW-lowering-033** A `VAR_IN_OUT` parameter is a variable of reference
type, every access to it carries an explicit `Deref`, and the matching argument
at each call site is the place passed by reference
([VAR_IN_OUT Parameters](var-in-out-parameters.md)).

A place says nothing about storage. Whether a variable occupies a slot, a run
of the data region or an address in linear memory is the backend's layout.
`ResolvedAccess` in `compile_array.rs` is the bytecode backend's answer to that
question and stays in that backend (see
[Existing Structures](#8-existing-structures)).

### 3.5 Scalar expressions

```rust
pub struct Expr { kind: ExprKind, ty: ScalarType, span: SourceSpan }

pub enum ExprKind {
    Const(Const),                                   // I32, I64, F32 or F64
    Load(Place),
    Convert(Box<Expr>),                             // to `ty`, from the operand's type
    Truncate { bits: u8, value: Box<Expr> },        // wrap to a storage width (ADR-0001)
    Unary { op: UnaryOp, value: Box<Expr> },        // Neg, BitNot, BoolNot
    Binary { op: BinaryOp, lhs: Box<Expr>, rhs: Box<Expr> },
    Compare { op: CompareOp, lhs: Box<Expr>, rhs: Box<Expr> },
    ShortCircuit { op: ShortCircuitOp, lhs: Box<Expr>, rhs: Box<Expr> },
    Call { callee: Callee, args: Vec<Arg> },
    RefTo(Place),
    Null,
}
```

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
its operation width is produced by a `Truncate` to that width or is a `Const`
already in range.

**REQ-LOW-lowering-047** `Load` of a place has the operation type of the
place's type.

**REQ-LOW-lowering-048** Logical and bitwise forms of `AND`, `OR`, `XOR` and
`NOT` are distinct operators, chosen by lowering from the operand type.

### 3.6 String expressions

```rust
pub struct StrExpr { kind: StrExprKind, shape: StringShape, span: SourceSpan }

pub enum StrExprKind {
    Literal(Vec<char>),
    Load(Place),
    Call { callee: Callee, args: Vec<Arg> },
}
```

**REQ-LOW-lowering-050** The string operands of one operation share an
encoding; a mismatch is reported by lowering as `StringEncodingMismatch` and no
lowered program is produced.

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
    Store { place: Place, value: Expr },
    StoreBits { place: Place, shift: u8, bits: u8, value: Expr },
    StoreStr { place: Place, value: StrExpr },
    Copy { dst: Place, src: Place },                          // whole aggregate
    Call { callee: Callee, args: Vec<Arg> },                  // result discarded
    FbCall { instance: Place, block: Block,
             inputs: Vec<(FieldIdx, Arg)>, outputs: Vec<(Place, FieldIdx)> },
    If { cond: Expr, then: Vec<Stmt>, otherwise: Vec<Stmt> },
    Case { selector: Expr, arms: Vec<CaseArm>, otherwise: Vec<Stmt> },
    Loop { id: LoopId, body: Vec<Stmt>, continuing: Vec<Stmt> },
    Exit(LoopId),
    Continue(LoopId),
    Return,
}
```

`Loop` repeats `body` then `continuing` until an `Exit` leaves it. `Continue`
skips the rest of `body` and runs `continuing`. That is enough for `WHILE`,
`REPEAT` and `FOR` (see [Desugaring](#5-desugaring)), and it is the shape
WebAssembly's structured control flow takes directly.

**REQ-LOW-lowering-060** The type of a `Store`'s value is the operation type of
its place's type.

**REQ-LOW-lowering-061** The two places of a `Copy` have the same type or types
of identical shape, as `value_type::check` accepts today.

**REQ-LOW-lowering-062** Every `Exit` and `Continue` names a `Loop` that
encloses it.

**REQ-LOW-lowering-063** Every statement has the span of the source statement
it was lowered from, so a backend can build a line map without the AST.

**REQ-LOW-lowering-064** The labels of a `Case` are constant values or constant
ranges of the selector's type.

**REQ-LOW-lowering-065** `StoreBits` names a bit range that lies within the
storage width of its place's type.

The arms of a `Case` are tried in order and the first arm with a matching
label runs, as `compile_case` does today.

`StoreBits` is the one read-modify-write statement. It exists as a node, and is
not desugared to a `Store` of a masked `Load`, because that would name the
place twice and so evaluate its subscripts twice. `compile_bit_access_assignment_on_array`
does exactly that today, emitting the flat index once for the load and again
for the store.

### 3.8 Callees, arguments and intrinsics

```rust
pub enum Callee { User(PouId), Intrinsic(Intrinsic) }
pub enum Block  { User(PouId), Standard(StandardBlock) }   // ADR-0003

pub enum Arg {
    Value(Expr),        // scalar, by value
    Str(StrExpr),       // string, by value
    Ref(Place),         // VAR_IN_OUT, and aggregates
}
```

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
`Value` at the parameter's operation type, `Str` at its encoding, `Ref` to a
place of its type.

**REQ-LOW-lowering-073** A behaviour policy
([ADR-0049](../adrs/0049-behavior-policies-selected-at-compile-time.md))
selects among `Intrinsic` variants during lowering; a backend receives no
policy options.

**REQ-LOW-codegen-074** The bytecode backend maps `Intrinsic` to a `func_id`
with a match that has no wildcard arm, so adding a variant fails to compile
until the backend handles it.

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

## 4. Decisions Lowering Owns

Each row is a decision about what the program means. The middle column is where
it is made today.

| Decision | Made today in | In the lowered program |
|---|---|---|
| Type of an untyped literal | Codegen, by threading an operation type through `compile_expr`; predicted by `rule_constant_range` | A `Const` of a concrete type, checked in range when built |
| Implicit conversion | `compile_arith::convert`, `compile_value_arg` | `Convert` |
| Arithmetic overload | Analyzer's `resolve_arithmetic_overload`, asked again by `compile_arith.rs` | A `Binary` at the result type, or the desugared time arithmetic |
| Narrowing before a store | `emit_truncation` at each store site | `Truncate` |
| Logical or bitwise operator | `emit_not` and `compile_compare`, from `expr_is_bool` | Distinct operators |
| Callee | Name match in `compile_function_call` and `lookup_builtin` | `Callee` |
| Argument order and count | `xform_named_to_positional_args`, then re-checked at 19 sites | `Vec<Arg>` matched to parameters |
| Argument passing mode | `ParamPassing` in `compile.rs` | `Arg` variant |
| Variable and field identity | Nine name-keyed maps; lower-cased field names | `VarId`, `FieldIdx` |
| Enumeration ordinal | `enum_map` | `Const` |
| Default initial value | `emit_initial_values`; subrange lower bound, first enumeration value | `Store` statements in `init` |
| Function local re-initialization ([ADR-0024](../adrs/0024-function-local-reinit-via-bytecode-prologue.md)) | `emit_function_local_prologue` | Statements at the head of the function body |
| Behaviour policy | `CodegenOptions::string_to_num` | `Intrinsic` variant |
| Target of `EXIT` and `CONTINUE` | `loop_labels` stack, with `ExitOutsideLoop` as a fallback | `LoopId` |
| String encoding and capacity of a string value | `string_width.rs` | `StringShape` |
| Temporal literal count and unit ([ADR-0021](../adrs/0021-time-32bit-ltime-64bit.md), [ADR-0025](../adrs/0025-datetime-unsigned-representation.md)) | `compile_time_count` | `Const` |
| Constant subscript within bounds | `ArrayIndexOutOfBounds` in `compile_array.rs` | Checked when the `Index` is built |

**REQ-LOW-lowering-090** Lowering reads `CompilerOptions`; a backend does not.

**REQ-LOW-analyzer-091** No analyzer rule restates a decision lowering makes in
order to predict its outcome. A check that depends on such a decision is made
by lowering, on the decided value.

REQ-LOW-analyzer-091 is what removes the type push-down from
`rule_constant_range`. Which of that rule's other checks move with it is an
[open question](#open-questions).

## 5. Desugaring

A source construct that can be expressed with others is, so that no backend
implements it.

| Source | Lowered to |
|---|---|
| `ELSIF` | Nested `If` |
| `WHILE c DO b` | `Loop` whose body exits when `c` is false, then `b` |
| `REPEAT b UNTIL c` | `Loop` with body `b` and a `continuing` that exits when `c` is true |
| `FOR` | A `Store` of the initial value, then a `Loop` whose body exits past the bound and whose `continuing` is the increment, with the semantics `compile_loop.rs` implements today |
| Bit read `x.3`, partial read `x.%B1` | Shift and mask of a `Load` |
| Bit and partial write | `StoreBits` |
| `p^ := v` | `Store` to a place ending in `Deref` |
| `S=`, `R=` | `If` on the value around a `Store` of `TRUE` or `FALSE` |
| `REF=` | `Store` of a `RefTo` (already the analyzer's view) |
| Operator function form `ADD(a, b, c)` | Left fold of `Binary` |
| Typed time function `ADD_DT_TIME(a, b)` | Scalar arithmetic with the unit conversion `compile_time_arith.rs` applies today |
| Enumerated value | `Const` |
| `MOVE(x)`, parenthesised expression | The operand |
| Comparison of strings | `Call` of a string comparison `Intrinsic` |
| Named argument | Positional `Arg` |

**REQ-LOW-lowering-100** Reading a bit or a partial access of any place lowers
to the same shift and mask over a `Load` of that place, whatever the shape of
the place.

**REQ-LOW-lowering-101** `a + b` and `ADD(a, b)` lower to the same expression.

**REQ-LOW-lowering-102** A bit or partial write lowers to one `StoreBits` that
names its place once.

The cross product in `compile_expr.rs` disappears because access kind and base
shape stop multiplying: a backend implements "address a place" once and "store
bits" once.

## 6. Diagnostics

Lowering accumulates. It finishes the walk and reports every problem it found,
as rules do under ADR-0048.

| Kind | Code | Meaning |
|---|---|---|
| Language problem that depends on a lowered decision | Existing codes: `ConstantOverflow`, `ArrayIndexOutOfBounds`, `StringEncodingMismatch` | The program is invalid |
| Construct lowering cannot express yet | P9999 | Not implemented; for example a sequential function chart body or a directly represented variable |
| Broken invariant | P9998 | Compiler defect |

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

One behaviour changes. A check that moves into lowering runs only on a program
that passed analysis, because lowering sits behind the gate. A user with an
undeclared variable and an out-of-range constant sees the first, fixes it, and
then sees the second. Compilers that build a typed tree only for error-free
bodies behave the same way.

## 7. Backend Contract

A backend receives a lowered program and decides four things: where each
variable is stored, which instructions express each node, how each intrinsic is
implemented, and how debug information is encoded.

**REQ-LOW-codegen-120** The bytecode backend evaluates the operands of an
expression, the arguments of a call and the subscripts of an `Index` in the
order the lowered program lists them.

**REQ-LOW-codegen-121** The bytecode backend evaluates each place once per
statement that names it once.

**REQ-LOW-codegen-122** The bytecode backend's layout is computed once per
compilation, before any instruction is emitted, and is not modified during
emission.

**REQ-LOW-codegen-123** The bytecode backend's layout is keyed by `VarId`,
`PouId` and `TypeId`; no layout lookup takes a name.

**REQ-LOW-codegen-124** The bytecode backend evaluates the selector of a `Case`
once.

REQ-LOW-codegen-121 and REQ-LOW-codegen-124 change what the compiler does
today. `compile_case` compiles the selector expression again for every label it
is compared against, and a bit or partial write to an array element emits the
subscripts twice. Neither is visible unless the expression has a side effect,
such as a call to a function that writes a global. Both are corrections rather
than behaviour-preserving steps and are delivered as such.

How the two targets are expected to realise the same node:

| Lowered | Bytecode VM | WebAssembly (expected) |
|---|---|---|
| `ScalarType` | I32, I64, F32 and F64 opcode families | `i32`, `i64`, `f32`, `f64` |
| `Truncate` | `TRUNC_*` | Sign or zero extension from the narrow width |
| `Loop`, `Exit`, `Continue` | Labels and jumps | `loop`, `block`, `br` |
| `Case` | Compare and branch chain | `br_table` or a chain |
| `Place` | One of seven load and store opcode families, chosen by layout | An address in linear memory and a typed load or store |
| `Intrinsic` | `BUILTIN func_id` (ADR-0008) | A call to a runtime function, or inline instructions |
| `Block::Standard` | `FB_CALL` with a standard type id (ADR-0003) | A call to a runtime function |

Run-time checks are part of what a program means: an out-of-range subscript
([ADR-0023](../adrs/0023-array-bounds-safety.md)), a null dereference and an
integer division by zero trap under the same conditions on every target. Where
those conditions are specified for more than one backend is an
[open question](#open-questions).

## 8. Existing Structures

Nothing is deleted before its last reader has moved to the lowered program.

### Analyzer

| Structure | Disposition | Change |
|---|---|---|
| `SemanticContext` | Keep | None. It is lowering's input. |
| `TypeEnvironment`, `TypeId` | Keep | Supplies the lowered program's type table. |
| `IntermediateType` | Keep | `slot_count` moves to the bytecode backend: it sizes strings with the container's `string_region_size`, which is VM layout. |
| `intermediates/` | Keep | `stdlib_function.rs` gains the `Intrinsic` identity. |
| `FunctionEnvironment` | Keep | Signatures of standard functions name their `Intrinsic`. |
| `SymbolEnvironment` | Keep | Supplies the declarations `VarId`s are allocated from. |
| `Expr::expr_type`, `VarDecl::type_id`, `ExprKind::LateBound` | Keep | Lowering is their last reader. |
| `type_table.rs` | Delete | On success its result is only logged (`stages.rs`); on failure it adds diagnostics, which would move to the rule or transform that owns them. Unrelated to this design, listed because the name invites confusion. |

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
| `ParamPassing` | Dissolves into `Arg`. |
| `SavedFbScope` | Delete. Ids do not collide across scopes. |
| `type_info.rs`, `string_width.rs` | Move into lowering. |
| `TimeArith`, `StringConversion`, `ShortCircuitOp` | Become desugarings, `Intrinsic` variants and an `ExprKind` respectively. |
| `ClassifiedCmp` | Keep in the bytecode backend. The fused compare and branch is a VM optimization. |
| `Emitter`, `optimize/`, `stack_balance.rs`, `call_graph.rs`, `PoolConstant` | Keep, unchanged. |

## 9. Testing

**The end-to-end suite is the regression net.** `compiler/codegen/tests/it`
compiles source and asserts variable values after a run (143 `end_to_end_*`
files). Those tests say nothing about how the compiler is structured, so they
hold across this change without edits. A step that needs one edited is not
behaviour preserving.

**Lowering is tested on the tree.** A decision in
[Decisions Lowering Owns](#4-decisions-lowering-owns) is asserted by lowering a
small program and inspecting the node: the literal in `x := 300` for a `USINT`
`x` is reported; the `INT` operand of `i + r` is a `Convert`.

**Constructors are tested directly.** Each requirement in
[The Lowered Program](#3-the-lowered-program) has a test that the constructor
refuses the violating operands.

**Structure is tested mechanically.** REQ-LOW-codegen-002 is a test over the
crate manifest. REQ-LOW-lowering-082 is enforced by clippy; its conformance
test asserts that each crate root carries the `deny` attribute, since a test
cannot observe a lint that is not configured.

**Backends are tested against each other.** Once a second backend exists, the
end-to-end helpers run each program on both and compare variable values. A
disagreement is a defect in one backend, because both consumed the same lowered
program.

Every end-to-end test that passes before the bytecode backend consumes the
lowered program passes after, with its source and assertions unchanged. This is
a delivery constraint rather than a requirement: it says how the work is done,
and no single test can check it.

## Delivery Constraints

This section constrains the order of work; it is not a work breakdown.

- The lowered program grows from the leaves: places, then callees, then typed
  expressions, then statements. At each stage the bytecode backend consumes
  what exists and the rest of codegen is unchanged.
- Each stage is behaviour preserving and is delivered as a prefactor, with the
  two exceptions named under [Backend Contract](#7-backend-contract).
- A minimal WebAssembly path (one `PROGRAM`, integer arithmetic, assignment) is
  built as soon as expressions are lowered. Its purpose is to show that nothing
  in the lowered program assumes the bytecode VM, before the rest of codegen
  migrates onto it.
- `ironplc-codegen` drops its dependency on `ironplc-analyzer` last, when
  nothing in it reads the AST.

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
MIR. Both targets are stack machines, and WebAssembly's control flow is
structured, so a tree maps to each directly while a graph would have to be
restructured for WebAssembly. A graph also needs everything this design
provides first.

**A tree in the bytecode VM's terms.** Places as slots and data-region offsets,
callees as `func_id`s. Simpler for one backend. It would leave every decision
in [Decisions Lowering Owns](#4-decisions-lowering-owns) to be made again by
the second.

**Decisions written into the AST by analyzer transforms.** The analyzer already
inserts implicit dereferences, makes named arguments positional and, since
ADR-0056, records the implicit conversions of a comparison this way. Extending
it to every conversion and literal type would add node kinds the parser never
produces to a tree `plc2plc` renders, which trades one set of states that are
illegal in some phase for another. It also leaves the name, call and layout
work in the backend, because the AST still refers by name.

**A validator in front of codegen.** One pass that asserts the invariants, with
accessors that unwrap. It centralises the checks and leaves every state
representable.

## Relationship to ADR-0056

ADR-0056 chose this alternative for comparisons, on the day this design is dated,
and named arithmetic operands, assignments and function arguments as the next
conversions to move into `xform_insert_implicit_conversions`. Its drivers were
one recorded answer per expression, a language server that can show the
answer without running codegen, and backends that lower rather than decide.
This design shares the first and third. It does not, by itself, give the
language server what ADR-0056 gives it, unless the language server reads the
lowered program by span.

The two cannot both be the long-term home of implicit conversions. One of
these holds, and the ADR this design calls for must say which:

- **Lowering decides; ADR-0056 is superseded.** The `ImplicitConversion` pass
  is removed once lowering produces `Convert` for comparisons, and no further
  conversions move into the analyzer. The language server shows conversions
  from the lowered program.
- **The analyzer decides; lowering translates.** ADR-0056 stands and is
  extended to every conversion. Lowering turns each `ImplicitConversion` into a
  `Convert` and decides no conversion itself, and the "Implicit conversion" row
  of [Decisions Lowering Owns](#4-decisions-lowering-owns) moves to the
  analyzer. Goal 1 still holds: the decision is made once, upstream of every
  backend.

Until that is settled, no further conversions should be moved into
`xform_insert_implicit_conversions`, so that the work is not done twice.

## Decisions to Record

The choice among the alternatives above is a decision, and belongs in an ADR
that this document then cites. Three decisions are separable:

1. Code generation consumes a separate, target-neutral lowered program.
2. Lowering owns every implicit language decision, and `check` includes it.
3. A backend does not depend on the analyzer.
4. Where implicit conversions are decided, which supersedes or extends
   ADR-0056 (see [Relationship to ADR-0056](#relationship-to-adr-0056)).

## Open Questions

1. **Names.** "Lowered program", the crate `ironplc-lowering` and the area code
   `LOW` are proposals. The repository already uses "intermediate" for
   `IntermediateType` and `intermediates/`, so that word is avoided here.
   "Lowering", "lowered program" and "backend" are not in the
   [glossary](../steering/glossary.md) and would be added to it. The doc
   comment of `xform_insert_implicit_conversions` already calls that analyzer
   transform "a lowering pass", which would have to change.
2. **One crate or two.** A backend needs `IntermediateType` and `Intrinsic`,
   which the analyzer owns. This design re-exports them. The alternative is a
   small types crate below the analyzer. Relatedly, REQ-LOW-codegen-003 forbids
   `ironplc_dsl::common` and `ironplc_dsl::textual` by test; splitting
   `ironplc_dsl::core` and `diagnostic` into their own crate would forbid them
   by manifest.
3. **`FOR`.** Desugaring it decides its semantics once. It may also cost the
   bytecode backend the `FOR`-specific peepholes it has today. The alternative
   is a `For` node with its semantics specified here.
4. **Layout-dependent values.** `SIZEOF` returns a size. If backends lay types
   out differently, either the language defines the size independently of
   layout or programs diverge.
5. **`rule_constant_range`.** The type push-down moves to lowering. Whether the
   checks of typed literals and initial values move with it or stay as a rule.
6. **Unreachable POUs.** Whether `check` lowers every POU, so that problems in
   unreferenced functions are reported, while `compile` lowers reachable ones.
7. **Capability checking.** Whether a backend's "the target cannot do this"
   diagnostics come from emission or from a separate pass that runs first.
8. **Strings.** Whether `StringShape::capacity` is a language property or a
   bytecode VM concern, and how a function returning a string is represented.
9. **Methods.** How `THIS^`, `SUPER^` and the staged dispatch of
   [ADR-0041](../adrs/0041-staged-method-and-interface-dispatch.md) appear.
10. **Array indexing.** Whether the flat index and stride computation is shared
    or belongs to each backend's layout.
11. **Run-time checks.** Where trap conditions common to all backends are
    specified.
12. **`Intrinsic` granularity.** One variant per operation with the type on the
    call, or one per operation and type as the VM's `func_id` table has today.
13. **Reference checks.** Whether a `Deref` of a `VAR_IN_OUT` parameter, which
    is never null, is distinguished from a `Deref` of a `REF_TO`.
14. **Debug information.** Whether the bytecode backend's debug section needs
    anything from the AST that the lowered program as described does not carry.
15. **Language server cost.** Lowering would run on every edit that analyzes
    cleanly.
16. **Aggregate results.** A user function may return a structure today
    (`UserFunctionInfo::return_struct_desc_index`, `compile_aggregate.rs`).
    The lowered program has no aggregate expression, `Copy` takes a `Place`
    as its source, and the `Call` statement discards its result, so `s := f()`
    has no representation. One option is for lowering to give the call a
    destination place (`CallInto { dst: Place, callee, args }`); another is to
    pass the result variable as a hidden `Ref` argument.

## References

- [rustc dev guide: THIR](https://rustc-dev-guide.rust-lang.org/thir.html)
- [Real World OCaml: the compiler backend](https://dev.realworldocaml.org/compiler-backend.html)
- [Trees That Grow](https://arxiv.org/abs/1610.04799)
- [RuSTy](https://github.com/PLC-lang/rusty), `src/resolver.rs` and
  `compiler/plc_lowering`, at commit `10ead7b`
- [`go/types`](https://pkg.go.dev/go/types)
