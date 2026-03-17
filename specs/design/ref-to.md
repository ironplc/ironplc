# Design: REF_TO Reference Types (IEC 61131-3 Edition 3)

## Overview

This document describes the design for REF_TO reference type support in IronPLC. REF_TO is an IEC 61131-3 Edition 3 (2013) feature that provides strongly-typed, safe references to variables. The primary motivation is enabling compilation of the OSCAT library, which uses REF_TO for passing references to function blocks.

This design covers the full pipeline: lexer, parser, AST, semantic analysis, code generation, and VM execution.

Related documents:
- [ADR-0022: Edition 3 compiler flag](../adrs/0022-edition-3-compiler-flag.md) — gating mechanism for Edition 3 features
- [Beckhoff TwinCAT dialect](beckhoff-twincat-dialect.md) — `REFERENCE TO` syntax (maps to same AST)
- [Siemens SCL dialect](siemens-scl-dialect.md) — `REF_TO` as dialect keyword (superseded by this design)
- Implementation plan: [specs/plans/ref-to.md](../plans/ref-to.md)

## Scope

**In scope:**
- `REF_TO type` type constructor in variable and type declarations
- `REF(variable)` operator to obtain a reference
- `ref^` dereference operator (postfix caret)
- `NULL` literal for uninitialized references
- Null comparison (`ref = NULL`, `ref <> NULL`)
- Compile-time and runtime safety checks

**Out of scope (deferred):**
- `?=` assign-attempt operator (OSCAT does not rely on it)
- `REF_TO` of function block instance fields (restrict to simple variables initially)
- `REF()` of array elements (e.g., `REF(arr[3])`) — array elements use descriptors, not simple variable indices; requires a (var_index, flat_offset) pair which the current reference representation cannot encode
- `REF()` of structure fields (e.g., `REF(my_struct.field)`) — depends on how structure fields map to variable slots
- Nested `REF_TO REF_TO` (multi-level indirection) — the grammar and type system support it, but codegen/VM testing is deferred to a follow-up; the semantic analyzer rejects nested `REF_TO` in this initial implementation
- Beckhoff `REFERENCE TO` / `POINTER TO` dialect syntax — maps to the same AST but requires dialect-specific parser productions (deferred to dialect implementation work)

## IEC 61131-3 Edition 3 REF_TO Features

| Feature | Syntax | Example |
|---------|--------|---------|
| Reference type declaration | `REF_TO type` | `myRef : REF_TO INT;` |
| Named reference type | `TYPE ... END_TYPE` | `TYPE IntRef : REF_TO INT; END_TYPE` |
| Reference operator | `REF(variable)` | `myRef := REF(counter);` |
| Dereference (read) | `ref^` | `value := myRef^;` |
| Dereference (write) | `ref^` | `myRef^ := 42;` |
| Null literal | `NULL` | `myRef := NULL;` |
| Null comparison | `= NULL` / `<> NULL` | `IF myRef <> NULL THEN ... END_IF;` |
| Reference assignment | `:=` | `ref1 := ref2;` |

### Key Constraints from the Standard

- **No pointer arithmetic**: `ref + 1` is illegal. References cannot be incremented, decremented, or used in arithmetic.
- **Strongly typed**: `REF_TO INT` is not assignable to `REF_TO REAL`. The target type must match exactly.
- **Comparison restrictions**: Only `=` and `<>` are allowed on references (for null checks and identity comparison). No ordering operators (`<`, `>`, `<=`, `>=`).

## Safety Design

Safety is the guiding principle. PLCs control physical machinery — null dereferences or memory corruption can cause injury or equipment damage.

### Compile-Time Safety (Analyzer)

| Rule | Rationale |
|------|-----------|
| **Strong typing**: `REF_TO INT` cannot be assigned to `REF_TO REAL` | Prevents type confusion at dereference site |
| **No pointer arithmetic**: `ref + 1` is a compile error | Eliminates buffer overflows and out-of-bounds access |
| **Addressable operands only**: `REF(1+2)` is rejected | Only variables have stable addresses |
| **No REF of ephemeral variables**: `REF(temp)` in a FUNCTION is rejected for VAR_TEMP; `REF(input)` in a FUNCTION is rejected for VAR_INPUT/VAR_OUTPUT | Prevents dangling references — these variables are stack-allocated in FUNCTIONs (not FUNCTION_BLOCKs) and destroyed when the function returns. VAR_INPUT/VAR_OUTPUT in FUNCTION_BLOCKs are persistent and therefore safe. |
| **No REF of array elements**: `REF(arr[i])` is rejected | Array elements use descriptor-based indexing, not simple variable-table indices; a reference cannot encode the element offset |
| **No nested REF_TO**: `REF_TO REF_TO INT` is rejected | Multi-level indirection is deferred; single-level only in this implementation |
| **NULL type restriction**: `x := NULL` where x is INT is rejected | NULL only assignable to reference types |
| **Comparison restrictions**: `ref < other_ref` is rejected | Only `=` and `<>` are meaningful for references |

### Runtime Safety (VM)

| Mechanism | How it works |
|-----------|-------------|
| **References are variable-table indices** | Not raw memory pointers — no arbitrary memory access possible |
| **Null dereference trap** | Every indirect load/store checks for null before access |
| **Bounds checking** | Variable table validates index on every access (existing mechanism) |
| **No dangling references** | Variable table is flat and stable for program lifetime; variables never deallocate during execution |
| **Default null initialization** | All reference variables are initialized to `u64::MAX` (null sentinel) by the codegen at program start. The variable table defaults slots to `Slot(0)`, which is a *valid* variable index — without explicit initialization, an uninitialized reference would silently point to variable 0 instead of trapping. The codegen emits `LOAD_CONST_I64 u64::MAX` + `STORE_VAR_I64` for every reference variable during the initialization phase. |

### Aliasing Model

References allow aliasing: two references can point to the same variable, and modifications through one are visible through the other. This is correct behavior per the IEC 61131-3 standard and is safe in the variable-table-index model because both references access the same slot through the same bounds-checked mechanism. The compiler does not assume no-aliasing for optimization purposes.

### Scope and Reference Validity

All variable-table indices are global to a program instance. References created inside a function block scope remain valid after the function block call returns, because the referenced variable persists in the flat variable table. The `scope.check_access(index)` call in the VM validates that the index is within the variable table bounds, not that it belongs to the current scope. This means cross-scope references are valid — a reference obtained in one function block can be used in another.

### Why Variable-Table Indices (Not Raw Pointers)

The VM already uses a `VariableTable` for all variable storage. Storing references as indices into this table provides:

1. **Bounds checking for free** — the variable table already validates indices on every access
2. **No pointer arithmetic** — there is no opcode for index + offset
3. **Bounded index space** — u16 indices, max 65535 variables
4. **Null sentinel** — `u64::MAX` cannot collide with a valid variable index

## Token Design

### New Tokens — `compiler/parser/src/token.rs`

Add as standard keywords with `#[token(...)]` attributes, following the LTIME pattern:

| Token | Lexer rule | Notes |
|-------|-----------|-------|
| `RefTo` | `#[token("REF_TO", ignore(case))]` | Type constructor keyword |
| `Ref` | `#[token("REF", ignore(case))]` | Reference operator keyword |
| `Null` | `#[token("NULL", ignore(case))]` | Null literal keyword |

The `^` caret for dereference does **not** need a new token. It already exists as `Xor` in the lexer. The parser disambiguates by position:
- **Postfix** (after a primary/variable expression): dereference
- **Infix** (between two expressions): XOR

This is standard IEC 61131-3 Edition 3 behavior. Dereference binds tighter than any binary operator.

### Edition 3 Gating — `compiler/parser/src/rule_token_no_std_2013.rs`

Add `RefTo`, `Ref`, and `Null` to the existing validation rule alongside `Ltime`. When `allow_iec_61131_3_2013` is `false`, these tokens are rejected with a diagnostic:

```
REF_TO requires --std-iec-61131-3=2013 flag
```

The `Xor`/`^` token does NOT need gating — it already exists for XOR in Edition 2. Only its interpretation as dereference (in postfix position) is Edition 3.

### Logos Longest-Match Behavior

`REF_TO` is a compound keyword containing `REF`. Logos uses longest-match, so `REF_TO` is lexed as a single `RefTo` token, not `Ref` + identifier `_TO`. Similarly, `REF(` produces `Ref` + `LeftParen`, and `REF_TO_SOMETHING` (if someone tried it as an identifier) would need verification.

## Parser Grammar — `compiler/parser/src/parser.rs`

### Type Specifier

Add `RefTo type_spec` production to the type specification rule:

```
type_spec = RefTo type_spec   // REF_TO INT, REF_TO REF_TO INT, etc.
           | ... existing productions ...
```

For Beckhoff compatibility, also accept `Reference To type_spec` (where `Reference` is a dialect-promoted token). Both produce the same AST node.

### Expressions

- **`REF(variable)`**: Primary expression — `Ref LeftParen variable RightParen`
- **`NULL`**: Primary expression — `Null` token produces a null literal
- **`expr^`**: Postfix — after parsing a primary expression, check for `Xor`/`^` in postfix position. Produces a `Deref` expression wrapping the base expression.

### Caret Disambiguation

The caret `^` is ambiguous between XOR (infix) and dereference (postfix). The parser resolves this by position in the expression grammar.

**Implementation approach:** The current parser uses a `precedence!` macro for infix operators, which calls a base-case rule for primary/unary expressions. Postfix dereference is handled *inside* the base-case rule, before the `precedence!` macro sees any infix operators. This gives dereference the highest precedence (tighter than any binary operator), matching the IEC 61131-3 Edition 3 standard.

Specifically, modify the rule that produces primary expressions (the base case called by the `precedence!` macro) to add a postfix loop after the primary is parsed:

```
rule primary_with_deref() -> ExprKind =
    base:primary_expression()
    carets:(tok(TokenType::Xor))*
    {
        let mut expr = base;
        for _ in carets {
            expr = ExprKind::Deref(Box::new(Expr::new(expr)));
        }
        expr
    }
```

This naturally handles:
- `a^` → `Deref(a)` — single dereference
- `a XOR b` → `BinaryOp(Xor, a, b)` — XOR still works (no postfix `^` consumed)
- `a^ XOR b` → `BinaryOp(Xor, Deref(a), b)` — dereference binds tighter

The loop (`*`) also handles the case where nested REF_TO is supported in the future: `p^^` would parse as `Deref(Deref(p))`. Even though nested REF_TO is rejected by the semantic analyzer in this initial implementation, parsing it correctly avoids confusing error messages.

## AST Representation

### Type Declarations — `compiler/dsl/src/common.rs`

```rust
// New variant in DataTypeDeclarationKind:
DataTypeDeclarationKind::Reference(ReferenceDeclaration)

// New struct:
pub struct ReferenceDeclaration {
    pub type_name: TypeName,              // The declared type name
    pub referenced_type_name: TypeName,   // The target type (e.g., INT)
}
```

### Variable Declarations — `compiler/dsl/src/common.rs`

```rust
// New variant in InitialValueAssignmentKind:
InitialValueAssignmentKind::Reference(ReferenceInitializer)

// New enum for reference initial values:
pub enum ReferenceInitialValue {
    Null(SourceSpan),         // := NULL
    Ref(Variable),            // := REF(var)  (rare in declarations, but valid)
}

// New struct:
pub struct ReferenceInitializer {
    pub referenced_type_name: TypeName,                // REF_TO <this type>
    pub initial_value: Option<ReferenceInitialValue>,   // Optional initializer
}
```

Note: `ConstantKind` cannot be used for the initial value because `NULL` is not a constant kind in the existing AST. A dedicated `ReferenceInitialValue` enum cleanly separates reference initialization from constant expressions and avoids polluting `ConstantKind` with a reference-specific concept.

### Expressions — `compiler/dsl/src/textual.rs`

```rust
// New variants in ExprKind:
ExprKind::Ref(Box<Variable>)     // REF(var) — reference operator
ExprKind::Deref(Box<Expr>)      // expr^    — dereference
ExprKind::Null(SourceSpan)      // NULL     — null literal
```

### Visitor/Fold — `compiler/dsl/src/visitor.rs`, `compiler/dsl/src/fold.rs`

Add visit/fold methods for new AST nodes following the existing pattern.

## Intermediate Type Representation — `compiler/analyzer/src/intermediate_type.rs`

```rust
// New variant in IntermediateType:
IntermediateType::Reference {
    target_type: Box<IntermediateType>,
}
```

New methods:
- `is_reference() -> bool`
- `referenced_type() -> Option<&IntermediateType>`

### Type Resolution

- `REF_TO INT` → `IntermediateType::Reference { target_type: Box::new(IntermediateType::Int { size: B32 }) }`
- `REF_TO` of user-defined types (structures, FBs, enumerations) → `Reference { target_type: ... }` with the resolved inner type

### NULL Type Resolution Strategy

NULL uses **contextual typing** (inferred from the assignment target or comparison operand), not a dedicated `NullRef` intermediate type. This avoids adding a special type variant that would need to be handled in every type-checking path.

**How it works:** `ExprKind::Null` is assigned `IntermediateType::Reference { target_type: Box::new(IntermediateType::Bool) }` as a placeholder during expression type resolution. The actual type compatibility check happens in the semantic rules:

- **Assignment** (`ref := NULL`): The rule checks that the target is a reference type. The NULL expression is valid regardless of its placeholder type because the codegen always emits `LOAD_CONST_I64 u64::MAX` — the same bit pattern for all reference types.
- **Comparison** (`ref = NULL`, `ref <> NULL`): The rule checks that one operand is a reference type. NULL is valid as the other operand. The codegen emits `EQ_I64`/`NE_I64` which compares raw u64 values — type-agnostic.
- **Standalone NULL**: Using NULL outside of an assignment or comparison context (e.g., `NULL + 1`) is rejected by the "no arithmetic on references" rule.

This approach works because references are type-erased at the bytecode level (all are u64 indices). The compile-time type only matters for dereference operations, and you cannot dereference NULL (it traps at runtime).

### Expression Type Resolution — `compiler/analyzer/src/xform_resolve_expr_types.rs`

| Expression | Resolved type |
|------------|---------------|
| `REF(var)` | `IntermediateType::Reference { target_type: typeof(var) }` |
| `ref^` | Target type of the reference (unwraps one level) |
| `NULL` | `IntermediateType::Reference { target_type: Bool }` (placeholder; see NULL Type Resolution Strategy above) |
| `ref = NULL` / `ref <> NULL` | `BOOL` |

## Semantic Rules — `compiler/analyzer/src/rule_ref_to.rs` (new file)

| Rule | Problem code | Error condition |
|------|-------------|-----------------|
| REF operand must be a simple variable | P2028 | `REF(1+2)` — operand is not an addressable variable |
| No REF of ephemeral variables | P2029 | `REF(temp)` for VAR_TEMP in any FUNCTION; `REF(input)` for VAR_INPUT/VAR_OUTPUT in a FUNCTION (not FB) — these are stack-allocated and destroyed when the function returns |
| No REF of array elements | P2030 | `REF(arr[3])` — array elements use descriptor-based access, not simple variable indices |
| Deref requires reference type | P2031 | `x^` where x has type INT — dereference is only valid on reference types |
| Reference type compatibility | P2032 | Assigning `REF_TO INT` to `REF_TO REAL` — target types must match exactly |
| No arithmetic on references | P2033 | `ref + 1` — arithmetic operators applied to reference type |
| NULL only for reference types | P2034 | `x := NULL` where x has type INT — NULL is only assignable to reference types |
| Only = and <> on references | P2035 | `ref < other_ref` — ordering comparison on references (only `=` and `<>` are allowed) |
| No nested REF_TO | P2036 | `REF_TO REF_TO INT` — multi-level indirection is not supported in this implementation |

These codes are in the P2000-P3999 range (type system errors), continuing from the existing highest code P2027.

## Opcode Design — `compiler/container/src/opcode.rs`

### Opcode Reuse Strategy

Key insight: all `LOAD_VAR_*` / `STORE_VAR_*` variants execute identically in the VM — they all manipulate 64-bit Slots. The type suffix is documentation only. References are just I64 values (variable-table indices). This allows **reusing almost all existing opcodes**:

| Need | Existing opcode | Notes |
|------|----------------|-------|
| `REF(var)` — push var's index | `LOAD_CONST_I64` | Index is a compile-time constant |
| `NULL` — push null sentinel | `LOAD_CONST_I64` | Value is `u64::MAX` |
| Load ref variable | `LOAD_VAR_I64` | Refs stored as I64 slots |
| Store ref variable | `STORE_VAR_I64` | Refs stored as I64 slots |
| Ref comparison (`=`, `<>`) | `EQ_I64` / `NE_I64` | Refs are just integer indices |

### New Opcodes (2 only)

Only indirect memory access requires new opcodes — there is no existing mechanism to load/store through a stack-provided index:

| Opcode | Value | Stack effect | Description |
|--------|-------|-------------|-------------|
| `LOAD_INDIRECT` | `0x14` | [ref] → [value] | Pop ref (var index), null-check, load value at that index |
| `STORE_INDIRECT` | `0x15` | [value, ref] → [] | Pop ref then value, null-check, store value at index |

Both opcodes are untyped — they operate on 64-bit Slots, same as all existing load/store opcodes.

The null sentinel is `u64::MAX`. It cannot collide with a valid variable index because the variable table uses u16 indices (max 65535), and `u64::MAX` is far outside that range.

## Code Generation — `compiler/codegen/src/compile.rs`

### VarTypeInfo Integration

Reference variables use `VarTypeInfo { op_width: W64, signedness: Unsigned, storage_bits: 64 }`. This causes the existing `emit_load_var()` and `emit_store_var()` helpers to select `LOAD_VAR_I64` / `STORE_VAR_I64` for reference variables without any changes to those helpers.

### Expression Compilation

New arms in the `compile_expr` match on `ExprKind`:

- **`ExprKind::Ref(var)`**: Resolve the variable's index at compile time, emit `LOAD_CONST_I64 index`. This is a compile-time constant — the variable index is known statically.
- **`ExprKind::Null`**: Emit `LOAD_CONST_I64 u64::MAX`.
- **`ExprKind::Deref(expr)`** (r-value — reading through a reference): Compile the inner expression (pushes the reference index onto the stack), then emit `LOAD_INDIRECT`.

### Default Initialization

During variable initialization (the codegen phase that emits initial values for all variables), reference variables without an explicit initializer must be initialized to NULL:

```
LOAD_CONST_I64 u64::MAX
STORE_VAR_I64 ref_slot
```

Reference variables with `:= NULL` emit the same code. Reference variables with `:= REF(var)` emit:

```
LOAD_CONST_I64 var_index
STORE_VAR_I64 ref_slot
```

### L-Value Dereference (Assignment Through a Reference)

The assignment `ref^ := value` requires a new code path in the assignment compiler (`StmtKind::Assignment`). The existing assignment flow resolves the target to a static variable index via `resolve_variable_name()` and emits `STORE_VAR`. But `ref^` as a target doesn't resolve to a static index — it requires evaluating the reference expression at runtime.

**Detection:** Before the normal assignment flow, check if the assignment target is a `Deref` expression (i.e., the outermost node of the target is `ExprKind::Deref`). If so, use the indirect store path.

**Indirect store path:**
1. Compile the value expression (pushes value onto the stack)
2. Compile the inner expression of the `Deref` (pushes the reference index)
3. Emit `STORE_INDIRECT` (pops ref, pops value, stores value at the referenced index)

This is a new branch at the top of the assignment handler, before `resolve_variable_name()`.

### Compilation Rules Summary

| Source | Bytecode | Notes |
|--------|----------|-------|
| `ref := REF(var)` | `LOAD_CONST_I64 var_index`, `STORE_VAR_I64 ref_slot` | Index is a compile-time constant |
| `ref := NULL` | `LOAD_CONST_I64 u64::MAX`, `STORE_VAR_I64 ref_slot` | NULL = max u64 sentinel |
| `value := ref^` | `LOAD_VAR_I64 ref_slot`, `LOAD_INDIRECT` | Load ref, then indirect load |
| `ref^ := value` | compile value, `LOAD_VAR_I64 ref_slot`, `STORE_INDIRECT` | Value then ref on stack; see L-Value Dereference above |
| `ref = NULL` | `LOAD_VAR_I64 ref_slot`, `LOAD_CONST_I64 u64::MAX`, `EQ_I64` | Reuse existing comparison |
| `ref1 := ref2` | `LOAD_VAR_I64 ref2_slot`, `STORE_VAR_I64 ref1_slot` | Refs are just I64 values |
| (init) `ref : REF_TO INT;` | `LOAD_CONST_I64 u64::MAX`, `STORE_VAR_I64 ref_slot` | Default null initialization |

## VM Execution — `compiler/vm/src/`

### New Trap — `compiler/vm/src/error.rs`

Add `Trap::NullDereference` with V-code `V4004`. This is a user-facing error (exit code 1), not an internal error — it represents a program logic error (dereferencing an uninitialized or explicitly-nulled reference), analogous to `DivideByZero` (V4001).

Update `compiler/vm/resources/problem-codes.csv` to add:
```
V4004,NullDereference,Null reference dereference during program execution,false
```

### Slot Helpers — `compiler/vm/src/value.rs`

```rust
impl Slot {
    pub fn null_ref() -> Slot { Slot(u64::MAX) }
    pub fn is_null_ref(&self) -> bool { self.0 == u64::MAX }
    pub fn as_var_index(&self) -> Result<u16, Trap> {
        if self.is_null_ref() {
            Err(Trap::NullDereference)
        } else if self.0 > u16::MAX as u64 {
            Err(Trap::InvalidVariableIndex(self.0 as u16))
        } else {
            Ok(self.0 as u16)
        }
    }
}
```

The range check (`self.0 > u16::MAX`) is a defense-in-depth measure. The compile-time type system prevents non-reference values from reaching `as_var_index()`, but a codegen bug could produce an out-of-range index. Without this check, `self.0 as u16` would silently truncate, causing access to the **wrong variable** with no error. The range check converts this into a clean trap using the existing `Trap::InvalidVariableIndex` (V9005, exit code 3 — internal error), which correctly classifies an out-of-range index as a compiler bug rather than a user program error.

**Failure mode severity distinction:**
- `Trap::NullDereference` (V4004, exit code 1) — user program error, recoverable by fixing the program logic
- `Trap::InvalidVariableIndex` (V9005, exit code 3) — internal compiler error, should never occur if codegen is correct

### Opcode Handlers — `compiler/vm/src/vm.rs`

**`LOAD_INDIRECT`**:
1. Pop slot from stack
2. Call `as_var_index()` — traps if null sentinel or out-of-range index
3. `scope.check_access(index)` — bounds check (existing mechanism)
4. `variables.load(index)` — load value (existing mechanism)
5. Push value onto stack

**`STORE_INDIRECT`**:
1. Pop ref slot from stack
2. Call `as_var_index()` — traps if null sentinel or out-of-range index
3. Pop value slot from stack
4. `scope.check_access(index)` — bounds check
5. `variables.store(index, value)` — store value

Both follow the exact same safety pattern as existing `LOAD_VAR`/`STORE_VAR` (scope check + variable table access), plus the null and range checks.

## Performance Considerations

### Indirect Access Overhead

Each `LOAD_INDIRECT` / `STORE_INDIRECT` performs: one stack pop, one null check, one range check, one scope check, and one variable table access. A direct `LOAD_VAR_I64` performs only a variable table access. The indirect path is roughly 3-4x more instructions per access.

This is the correct trade-off for PLC safety: preventing null dereferences and out-of-bounds access is more important than raw throughput. For time-critical inner loops, users should dereference the reference once and work with the resulting value directly.

### Future Optimization Opportunities (Not in Scope)

- **Null check hoisting**: When a reference provably does not change within a loop, the null check could be hoisted to the loop entry. This requires dataflow analysis and is not planned for the initial implementation.
- **Direct access promotion**: When the compiler can prove which variable a reference points to (e.g., `ref := REF(x); ref^`), it could emit `LOAD_VAR_I64` instead of `LOAD_INDIRECT`. This is a straightforward peephole optimization but is deferred.

## Keyword Collision Notes

The new keywords `NULL` and `REF` may conflict with existing programs that use them as identifiers. Since these keywords are gated behind the Edition 3 flag (`--std-iec-61131-3=2013`), Edition 2 programs are unaffected. However, programs migrating to Edition 3 that use `NULL` or `REF` as variable or type names will need to rename those identifiers.

`REF_TO` is less likely to collide because the underscore makes it an unusual identifier, and logos longest-match ensures it is always lexed as a single token.

## Dialect Compatibility

REF_TO tokens are added to the logos lexer as standard keywords (same pattern as LTIME), gated by `rule_token_no_std_2013.rs`. This means:

- **Siemens SCL**: The `DIALECT_KEYWORDS` entry for `REF_TO` in the [Siemens SCL design](siemens-scl-dialect.md) becomes unnecessary — `REF_TO` is already a keyword token from the lexer. The Siemens design should be updated to note this.
- **Beckhoff TwinCAT**: `REFERENCE TO` still needs dialect promotion for the `Reference` token. The parser would map both `RefTo type_spec` and `Reference To type_spec` to the same AST node (`ReferenceDeclaration` / `ReferenceInitializer`). **This is deferred** — the Beckhoff parser production is not part of this implementation plan. The standard `REF_TO` syntax is sufficient for OSCAT compatibility.
- **Standard mode without Edition 3 flag**: `REF_TO` is lexed as a token but rejected by the validation rule with a clear diagnostic pointing to `--std-iec-61131-3=2013`.
