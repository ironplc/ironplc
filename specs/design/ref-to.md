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
| **No REF of VAR_TEMP**: `REF(temp_var)` in a FUNCTION is rejected | Prevents dangling references (temp vars don't persist across calls) |
| **NULL type restriction**: `x := NULL` where x is INT is rejected | NULL only assignable to reference types |
| **Comparison restrictions**: `ref < other_ref` is rejected | Only `=` and `<>` are meaningful for references |

### Runtime Safety (VM)

| Mechanism | How it works |
|-----------|-------------|
| **References are variable-table indices** | Not raw memory pointers — no arbitrary memory access possible |
| **Null dereference trap** | Every indirect load/store checks for null before access |
| **Bounds checking** | Variable table validates index on every access (existing mechanism) |
| **No dangling references** | Variable table is flat and stable for program lifetime; variables never deallocate during execution |

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

The caret `^` is ambiguous between XOR (infix) and dereference (postfix). The parser resolves this by position in the expression grammar:

1. Parse a primary expression (variable, function call, literal, etc.)
2. Check for postfix `^` — if present, wrap in `Deref`
3. Then check for infix operators (including `^` for XOR)

This means `a^ XOR b` parses as `(deref a) XOR b`, and `a XOR b` parses as `a XOR b`. The postfix check happens before the infix operator check.

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

// New struct:
pub struct ReferenceInitializer {
    pub referenced_type_name: TypeName,   // REF_TO <this type>
    pub initial_value: Option<...>,       // Optional := NULL or := REF(var)
}
```

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
- `NULL` has a special type compatible with any `REF_TO` (similar to how `0` is compatible with any integer type)

### Expression Type Resolution — `compiler/analyzer/src/xform_resolve_expr_types.rs`

| Expression | Resolved type |
|------------|---------------|
| `REF(var)` | `REF_TO typeof(var)` |
| `ref^` | Target type of the reference (unwraps one level) |
| `NULL` | Null reference type (compatible with any `REF_TO`) |
| `ref = NULL` / `ref <> NULL` | `BOOL` |

## Semantic Rules — `compiler/analyzer/src/rule_ref_to.rs` (new file)

| Rule | Problem code | Error condition |
|------|-------------|-----------------|
| REF operand must be a variable | P20XX | `REF(1+2)` — operand is not an addressable variable |
| No REF of VAR_TEMP | P20XX | `REF(temp)` in FUNCTION — temp variable may not persist |
| Deref requires reference type | P20XX | `x^` where x has type INT |
| Reference type compatibility | P20XX | Assigning `REF_TO INT` to `REF_TO REAL` |
| No arithmetic on references | P20XX | `ref + 1` — arithmetic operators applied to reference type |
| NULL only for reference types | P20XX | `x := NULL` where x has type INT |
| Only = and <> on references | P20XX | `ref < other_ref` — ordering comparison on references |

Problem code numbers TBD based on the existing range in `compiler/problems/resources/problem-codes.csv`.

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

### Compilation Rules

| Source | Bytecode | Notes |
|--------|----------|-------|
| `ref := REF(var)` | `LOAD_CONST_I64 var_index`, `STORE_VAR_I64 ref_slot` | Index is a compile-time constant |
| `ref := NULL` | `LOAD_CONST_I64 u64::MAX`, `STORE_VAR_I64 ref_slot` | NULL = max u64 sentinel |
| `value := ref^` | `LOAD_VAR_I64 ref_slot`, `LOAD_INDIRECT` | Load ref, then indirect load |
| `ref^ := value` | compile value, `LOAD_VAR_I64 ref_slot`, `STORE_INDIRECT` | Value then ref on stack |
| `ref = NULL` | `LOAD_VAR_I64 ref_slot`, `LOAD_CONST_I64 u64::MAX`, `EQ_I64` | Reuse existing comparison |
| `ref1 := ref2` | `LOAD_VAR_I64 ref2_slot`, `STORE_VAR_I64 ref1_slot` | Refs are just I64 values |

## VM Execution — `compiler/vm/src/`

### New Trap — `compiler/vm/src/error.rs`

Add `Trap::NullDereference` with a V-code (e.g., `V4004`). User-facing error (exit code 1), not internal error.

### Slot Helpers — `compiler/vm/src/value.rs`

```rust
impl Slot {
    pub fn null_ref() -> Slot { Slot(u64::MAX) }
    pub fn is_null_ref(&self) -> bool { self.0 == u64::MAX }
    pub fn as_var_index(&self) -> Result<u16, Trap> {
        if self.is_null_ref() {
            Err(Trap::NullDereference)
        } else if self.0 > u16::MAX as u64 {
            Err(Trap::InvalidVariableIndex)
        } else {
            Ok(self.0 as u16)
        }
    }
}
```

The range check (`self.0 > u16::MAX`) is a defense-in-depth measure. The compile-time type system prevents non-reference values from reaching `as_var_index()`, but a codegen bug could produce an out-of-range index. Without this check, `self.0 as u16` would silently truncate, causing access to the **wrong variable** with no error. The range check converts this into a clean trap using the existing `Trap::InvalidVariableIndex`.

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

## Dialect Compatibility

REF_TO tokens are added to the logos lexer as standard keywords (same pattern as LTIME), gated by `rule_token_no_std_2013.rs`. This means:

- **Siemens SCL**: The `DIALECT_KEYWORDS` entry for `REF_TO` in the [Siemens SCL design](siemens-scl-dialect.md) becomes unnecessary — `REF_TO` is already a keyword token from the lexer. The Siemens design should be updated to note this.
- **Beckhoff TwinCAT**: `REFERENCE TO` still needs dialect promotion for the `Reference` token. The parser maps both `RefTo type_spec` and `Reference To type_spec` to the same AST node (`ReferenceDeclaration` / `ReferenceInitializer`). No conflict.
- **Standard mode without Edition 3 flag**: `REF_TO` is lexed as a token but rejected by the validation rule with a clear diagnostic pointing to `--std-iec-61131-3=2013`.
