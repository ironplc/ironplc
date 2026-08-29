# Plan: METHOD declarations and static dispatch (ADR-0041 Phase 1, slice 1)

## Goal

Start ADR-0041 Phase 1 ("Staged Method/Property Dispatch"): let a
`FUNCTION_BLOCK` declare `METHOD`s, and let calls to those methods be
resolved and compiled. Scoped to a first PR-sized slice, matching how the
rest of the OOP work (EXTENDS/IMPLEMENTS/ABSTRACT, `FunctionBlockOop`) has
already been sliced into small, independently-mergeable PRs.

## Scope of this slice

In scope:
- Parse `METHOD name (: return_type)? ... END_METHOD` blocks nested inside
  `FUNCTION_BLOCK ... END_FUNCTION_BLOCK`. A method has its own
  parameter/local `VAR` blocks and its own statement body, matching a
  `FunctionDeclaration` in shape but living on a function block and,
  unlike a function, allowed to have no return type (acts as a procedure).
- `instance.MethodName(args)` call syntax, where `instance` is a variable
  of function-block type (a `StructuredVariable`-shaped access, same
  syntax already used for struct field access).
- Static resolution: given the *declared* type of `instance`, find
  `MethodName` by checking that type's own methods, then its `EXTENDS`
  base, then that base's `EXTENDS` base, and so on (ADR-0041's Phase 1
  algorithm). One diagnostic (new problem code) when no method by that
  name is found anywhere in the chain.
- Codegen: a method compiles to an ordinary function with a mangled name
  (`<FbTypeName>.<MethodName>`) taking an implicit leading parameter that
  is a pointer to the receiver's fields, so the method body can read/write
  the FB instance's `VAR` fields the same way struct field access already
  works. A call site compiles to a call to that mangled function, passing
  a pointer to `instance`.

Out of scope (follow-up slices, tracked below):
- `THIS^` / `SUPER^` (needs the receiver pointer to be an addressable
  pseudo-variable inside a method body, plus `SUPER^` needs to skip to the
  immediate base's own method without re-running the chain walk).
- `PROPERTY ... GET ... SET ... END_PROPERTY` (rewrites property
  reads/writes to GET/SET calls; naturally layers on top of the method
  call machinery this slice adds).
- Unqualified self-calls from inside another method of the same FB
  (`MethodName(args)` with no `instance.` prefix) — needs `THIS^` to exist
  first, since that's what an unqualified call implicitly binds to.
- Any dynamic/polymorphic dispatch (ADR-0041 Phase 2 — not decided, needs
  its own ADR).
- `ABSTRACT` methods (no body) / interface method signatures — interfaces
  today only parse their header (see `InterfaceDeclaration` doc comment in
  `dsl/src/common.rs`); giving them method signatures is separate work.

## Design

### AST (`compiler/dsl/src/common.rs`)

```rust
pub struct MethodDeclaration {
    pub name: Id,
    pub return_type: Option<FunctionReturnType>,
    pub variables: Vec<VarDecl>,
    pub edge_variables: Vec<EdgeVarDecl>,
    pub body: Vec<StmtKind>,
    pub span: SourceSpan,
}
```

Mirrors `FunctionDeclaration` (`common.rs:2731`), except `return_type` is
`Option` (a method with no return type is valid IEC 61131-3 and acts like
a procedure — same modeling already used for `FunctionSignature` in
`analyzer/src/function_environment.rs:32`).

`FunctionBlockDeclaration` gets one new field:

```rust
pub methods: Vec<MethodDeclaration>,
```

Not folded into `FunctionBlockOop` — `FunctionBlockOop` is specifically
the `EXTENDS`/`IMPLEMENTS`/`ABSTRACT` *header* facet (see its doc comment),
and an FB can have methods without using any of those. `methods` stays a
plain field, empty `Vec` for the common non-OOP case, same pattern as
`variables`.

### Parser (`compiler/parser/src/parser.rs`)

New tokens needed: `METHOD` / `END_METHOD` (check `TokenType` /
`compiler/parser/src/tokens/*` first — if TwinCAT lexing already special
cases method-call dots this may partially exist; confirm before adding).

New rule, modeled directly on `function_declaration()` (`parser.rs:1281`)
and `function_block_declaration()` (`parser.rs:1311`):

```
rule method_declaration() -> MethodDeclaration =
  start:tok(TokenType::Method) _ name:identifier() _
  rt:(tok(TokenType::Colon) _ rt:function_return_type() {rt})? _
  var_decls:(...) ** _ _
  body:function_body() _
  end:tok(TokenType::EndMethod) { ... }
```

`function_block_declaration()`'s `decls` sequence gains an interleaved
`methods:method_declaration() ** _` component (methods can appear anywhere
among the FB's var blocks, per IEC 61131-3 ed3 / TwinCAT convention),
collected into the new `methods: Vec<MethodDeclaration>` field.

Call-site syntax: `instance.MethodName(args)` needs a new expression/
statement form. Check `StructuredVariable` (`textual.rs:170`) and how
`FbCall` (`textual.rs:293`) currently drives `var_name(args)` — a method
call is structurally "structured-variable-as-call-target" rather than a
bare `var_name`. Likely a `MethodCall { instance: Variable, method: Id,
params: Vec<ParamAssignmentKind>, span }` variant alongside `FbCall`,
reusing `ParamAssignmentKind` for arguments. Confirm exact shape while
implementing — this is the one piece of the design most likely to need
adjustment once in the grammar.

### Visitor/Fold (`compiler/dsl/src/visitor.rs`, `compiler/dsl/src/fold.rs`)

`dispatch!(MethodDeclaration);` and the new call-site node, in both files
— the `FunctionBlockOop` restructure plan hit exactly this omission
(`dsl/src/fold.rs` wasn't in its original file map, surfaced by
`cargo build`), so budget for it here too.

### plc2plc renderer (`compiler/plc2plc/src/renderer.rs`)

Round-trip rendering for `METHOD ... END_METHOD` and the new call syntax —
required by this project's plc2plc round-trip test convention (see
`specs/steering/syntax-support-guide.md`).

### Analyzer

- New rule (name TBD, likely `rule_method_call_declared.rs`, modeled on
  `rule_function_call_declared.rs`): walks `instance`'s declared FB type
  through its own `methods`, then `oop.base` chain, erroring (new problem
  code, e.g. `P4046`) if the method name isn't found anywhere in the
  chain, or if it's found but argument arity/types don't match (mirror
  `rule_function_call_type_check.rs`'s approach, reusing as much of its
  parameter-matching logic as can be shared rather than duplicated).
- `xform_toposort_declarations.rs` and any other pass that walks
  `oop.base` for field inheritance (`intermediates/inherited_fields.rs`)
  should be checked for whether method resolution needs the same
  toposort ordering guarantee (a method body calling a not-yet-declared
  type's method).

### Codegen

- `compile_fn.rs` (function compilation) is the closest existing template
  — a method compiles the same way a function does, plus one synthesized
  leading parameter (pointer to the receiver's struct layout, which
  already exists for the FB itself via whatever `compile.rs` uses to lay
  out FB instance memory).
- `compile_call.rs` needs a new call-compilation path for the
  `instance.MethodName(args)` form: evaluate `instance` to a pointer
  (already possible — struct field access does this), pass it as the
  hidden first argument, then compile the rest exactly like a function
  call.
- Symbol mangling: `<FbTypeName>.<MethodName>` (or whatever separator
  `call_graph.rs` / `emit.rs` already reserve — check for collisions with
  existing name-mangling conventions before picking one).

### Problem codes

New diagnostic(s) documented in `docs/compiler/problems/P####.rst` per
project convention — exact numbers assigned when the rule is written
(check `docs/compiler/problems/` for the next free number at that time).

## Files (expected, confirm while implementing)

| File | Change |
|---|---|
| `compiler/dsl/src/common.rs` | `MethodDeclaration` struct; `methods` field on `FunctionBlockDeclaration` |
| `compiler/dsl/src/textual.rs` | New method-call AST node (exact shape TBD) |
| `compiler/dsl/src/visitor.rs`, `fold.rs` | `dispatch!` entries for both new nodes |
| `compiler/parser/src/tokens/*` | `METHOD`/`END_METHOD` tokens (if not already present) |
| `compiler/parser/src/parser.rs` | `method_declaration()` rule; call-site grammar; wire into `function_block_declaration()` |
| `compiler/plc2plc/src/renderer.rs` | Render `METHOD...END_METHOD` and call syntax |
| `compiler/analyzer/src/rule_method_call_declared.rs` (new) | Resolve + arity/type-check method calls across the `EXTENDS` chain |
| `compiler/analyzer/src/function_environment.rs` or new `method_environment.rs` | Registry of method signatures per FB type |
| `compiler/codegen/src/compile_fn.rs` | Compile `MethodDeclaration` bodies (as functions with a receiver param) |
| `compiler/codegen/src/compile_call.rs` | Compile `instance.Method(args)` call sites |
| `docs/compiler/problems/P####.rst` | New problem code(s) |
| `compiler/sources/src/xml/transform.rs` | TwinCAT `.TcPOU` methods are separate `<Method>` XML elements per FB — decide whether this slice reads them or defers to a follow-up (see Open question below) |

## Open question to resolve before/while implementing

TwinCAT's actual on-disk format stores each method as a *separate*
`<Method Name="...">` element alongside the FB's own `<Declaration>`/
`<Implementation>`, not as inline `METHOD...END_METHOD` text inside the
FB body (that textual form is the plain-ST / CODESYS-export shape). The
grammar work above covers the textual form. Whether `.TcPOU` XML methods
get wired up in this slice or a follow-up depends on how much the parser
work reveals about shared structure — decide once `MethodDeclaration`
exists and it's clear whether `transform.rs` can reuse the same AST node
directly (likely yes, same pattern as `transform_function`/
`transform_function_block` already reusing `FunctionDeclaration`/
`FunctionBlockDeclaration`).

## Codegen design (researched 2026-08-12, for the codegen slice)

The VM (`ironplc-vm`) has no heap and no true pointers (ADR-0021 "flat
variable table"). Confirmed by reading `vm/src/vm.rs`'s `FB_CALL` handler
and `vm/src/variable_table.rs`:

- Each FB **type** (not instance) gets ONE fixed, shared scratch region
  in the VM's flat `variables` table (`var_off..var_off+num_fields`),
  reused sequentially by whichever instance is currently executing —
  never concurrently, never recursively.
- Each FB **instance**'s actual persistent field values live in a
  separate `data_region`, addressed by that instance's own `VarIndex`
  (`fb_ref`).
- `FB_CALL`'s "user-defined FB" branch: copies the instance's fields from
  `data_region` into the type's shared `variables[var_off..]` scratch
  ("copy-in"), pushes a `Frame` with `fb_return: Some(FbCallReturn {
  instance_start, var_offset, num_fields })`, runs the body. On
  `RET`/`RET_VOID`, `handle_frame_return` (the single choke point for
  both) copies the scratch back out to `data_region` ("copy-out") if
  `fb_return.is_some()`.
- `VariableScope { instance_offset, instance_count }` is a **bounds
  check only** (`check_access`), not an address-translation mechanism —
  `VarIndex` operands in bytecode are genuine absolute indices into the
  flat table, fixed at compile time per function/FB-type.
- Plain function `CALL` passes args via the shared operand stack (push
  args, `CALL` pops them into `var_offset..var_offset+num_params`) and
  returns a value by leaving it on the stack before `RET` —
  `handle_frame_return` doesn't care whether it was `RET` or `RET_VOID`,
  so a frame can have both a stack return value *and* `fb_return`
  copy-out at once.

This means a method can reuse the exact same copy-in/copy-out machinery
FB bodies already use, with one addition: a method needs its own
param/local slots *in addition to* the type's field slots. Design:

- **One shared "method scratch" region per FB type**, sized to the max
  param+local count across all its methods (methods never run
  concurrently for the same type, same non-reentrancy assumption as
  fields), allocated immediately after that type's field region so
  `[var_off, var_off + num_fields + max_method_locals)` is one
  contiguous block. Every method of that type compiles against the same
  `param_var_off = var_off + num_fields` and reports
  `num_locals = num_fields + own_locals` on its `CompiledFunction`, so
  the pushed `Frame`'s single contiguous `VariableScope` covers both the
  type's fields (for `self` access) and the method's own params/locals.
- **New opcode `METHOD_CALL(function_id: u16, field_var_off: u16,
  num_fields: u16, param_var_off: u16)`** — deliberately simpler than
  `FB_CALL` (no intrinsic-FB-type branch; a method call is always
  user-defined). At runtime: copy-in fields (same loop as `FB_CALL`),
  pop `num_params` (read from the resolved function's own metadata) args
  off the stack into `param_var_off..`, push a `Frame` with
  `fb_return: Some(FbCallReturn { instance_start: fb_ref, var_offset:
  field_var_off, num_fields })`, run. A method with a return type ends
  its body with `RET` (value on stack, exactly like a plain function);
  a method with no return type ends with `RET_VOID`, matching FB bodies.
- **Call site** (`instance.Method(args)`): `FB_LOAD_INSTANCE(var_index)`
  (identical to `FbCall`) to push `fb_ref`, push args in the method's
  declared `VAR_INPUT` order (named args resolved to position at codegen
  time, mirroring how `compile_fb_call` already resolves named args
  against field declarations), `METHOD_CALL`, then pop the discarded
  return value (if any, since this slice is statement-only and always
  discards it) and finally pop `fb_ref` — mirroring `compile_fb_call`'s
  trailing `emit_pop()` for the same purpose.
- No container/type-section schema changes needed — `function_id`,
  `field_var_off`, `num_fields`, and `param_var_off` are all literal
  constants baked into the opcode operands at compile time, since the
  instance's static FB type (and therefore which method function to
  call) is already resolved by `rule_method_call_declared`.

## Testing Strategy

- Parser: new `parser/src/tests/` cases for a method with no params/no
  return, with params and a return, multiple methods in one FB, a method
  calling another method on `THIS`-typed args (still just parse-level).
- plc2plc round-trip tests per `syntax-support-guide.md`.
- Analyzer: method found on the FB itself; found only via one level of
  `EXTENDS`; found via two levels; not found anywhere (diagnostic);
  wrong arg count/type (diagnostic).
- Codegen/e2e: a `.st` program instantiating an FB, calling a method that
  mutates one of the FB's own fields, asserting the field's value after —
  proves the receiver pointer is wired correctly, not just that it
  compiles.
- Full CI (`cd compiler && just`) before any PR.

## Tasks

- [x] `MethodDeclaration` AST node + `methods` field on `FunctionBlockDeclaration`
- [x] `dispatch!` entries (`visitor.rs`, `fold.rs`)
- [x] Method-call AST node (`textual.rs`) — `MethodCall`, statement position only (decided with Tim: ship statement-only first, expression-position calls are a follow-up)
- [x] Tokens + `method_declaration()` parser rule + call-site grammar (`METHOD`/`END_METHOD` gated by `allow_fb_inheritance`, same as the other OOP keywords)
- [x] plc2plc renderer support (also fixed a pre-existing gap: `FbCall` rendering had no trailing `;`, breaking round-trip for any FB-call statement followed by another statement — `MethodCall` shares the fix)
- [x] Parser unit tests (`parser/src/tests/methods.rs`, 7 tests) + plc2plc round-trip tests (`plc2plc/src/tests/methods.rs`, 4 tests)
- [x] Method resolution analyzer rule (own methods, then `EXTENDS` chain) + problem code P4046 `MethodNotFound` (`rule_method_call_declared.rs`)
- [x] Method call arity/type-check (reuses the existing `FunctionCallMixedArgTypes`/`FunctionInvocationMissingInput`/`FunctionInvocationRequiresFormal`/`FunctionInvocationUndefinedOutput` diagnostics, same as `rule_function_block_invocation.rs` does for plain FB calls)
- [x] Codegen: compile method bodies (`compile_method.rs`, new `METHOD_CALL` opcode — see "Codegen design" above)
- [x] Codegen: compile call sites (`compile_stmt.rs::compile_method_call`)
- [x] e2e test proving field mutation through a method call (`codegen/tests/it/end_to_end_methods.rs`, 4 tests, real VM execution)
- [x] Decide + implement (or explicitly defer) `.TcPOU` XML method wiring — **deferred**; `transform.rs` still always sets `methods: vec![]` for TwinCAT XML POUs, unchanged from slice 1
- [x] Full CI clean (compile + coverage ≥85% + clippy + fmt + dupes) via `cd compiler && just`
- [x] Update `specs/plans/twincat-status.md`

### Status (2026-08-12, codegen slice)

Codegen is done: a method call now actually executes and mutates the FB
instance's own persistent field, proven end-to-end through real VM
execution (not just unit-tested opcode plumbing). Key implementation
notes, in case of future debugging:

- Confirmed by reading the VM (`vm/src/vm.rs`'s `FB_CALL` handler,
  `vm/src/variable_table.rs`) that there is no heap/pointers at all
  (ADR-0021 flat variable table): each FB *type* gets one shared scratch
  region in the flat table, reused sequentially across all its
  instances via copy-in/copy-out with the instance's own persistent
  `data_region` storage.
- New opcode `METHOD_CALL` (`ironplc_container::opcode`, byte `0xF8`,
  pinned in `codegen/tests/it/wire_format.rs`) reuses that exact
  copy-in/copy-out machinery (`FbCallReturn`/`handle_frame_return`,
  unchanged) rather than inventing a new one.
- A method's own params/locals are allocated *contiguously after* its
  owning type's field region specifically so the VM's single
  contiguous `VariableScope` bounds check can cover both the fields
  (for `self` access) and the method's own slots in one window — see
  `compile_method.rs`'s `num_locals` computation, which deliberately
  reports a span starting at the type's field offset, not the method's
  own param offset.
- **Scoped to methods declared directly on the instance's own type.**
  A method reached only via `EXTENDS` (already accepted by the
  analyzer) is explicitly NOT supported by codegen yet: a derived FB
  type's data-region layout doesn't currently reserve storage for a
  base type's fields at all (confirmed — `inherited_fields.rs` is used
  only for name-duplication checking, never to flatten storage), so
  there's nothing correct to copy in/out for an inherited method. That
  gap predates this work and is a separate, deeper fix. Calling an
  inherited method today hits `Diagnostic::todo` in
  `compile_method_call`.
- Statement-position only, as scoped: the return value (if any) is
  always discarded at the call site.
