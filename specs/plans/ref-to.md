# Plan: REF_TO Reference Types Implementation

## Summary

Implement IEC 61131-3 Edition 3 REF_TO reference types across the full compiler pipeline: lexer, parser, semantic analysis, code generation, and VM execution. The primary goal is enabling compilation and execution of the OSCAT library. Safety is the guiding principle — references use variable-table indices (not raw pointers), strong typing is enforced at compile time, and null dereferences trap at runtime.

Design document: [specs/design/ref-to.md](../design/ref-to.md)

## Phase 1: Lexer and Tokens

### Step 1.1: Add new token types

**File**: `compiler/parser/src/token.rs`

Add three new keyword tokens with `#[token(...)]` attributes:

```rust
#[token("REF_TO", ignore(case))]
RefTo,

#[token("REF", ignore(case))]
Ref,

#[token("NULL", ignore(case))]
Null,
```

Update `TokenType::describe()` for each new variant:
- `TokenType::RefTo => "'REF_TO'"`
- `TokenType::Ref => "'REF'"`
- `TokenType::Null => "'NULL'"`

Verify logos longest-match behavior: `REF_TO` must lex as a single `RefTo` token, not `Ref` + identifier. Write a lexer test for this.

### Step 1.2: Gate behind Edition 3 flag

**File**: `compiler/parser/src/rule_token_no_std_2013.rs`

Add `RefTo`, `Ref`, and `Null` to the existing token check alongside `Ltime`:

```rust
if tok.token_type == TokenType::RefTo
    || tok.token_type == TokenType::Ref
    || tok.token_type == TokenType::Null
{
    errors.push(Diagnostic::problem(
        ironplc_problems::Problem::Std2013Feature,
        Label::span(
            tok.span.clone(),
            format!("{} requires --std-iec-61131-3=2013 flag",
                tok.text.to_uppercase()),
        ),
    ));
}
```

Add tests:
- `apply_when_has_ref_to_and_not_allowed_then_error`
- `apply_when_has_ref_to_and_allowed_then_ok`
- Same pattern for `Ref` and `Null`

### Step 1.3: Verification

- All existing tests pass
- New tokens are lexed correctly
- `REF_TO` is rejected without Edition 3 flag
- `REF_TO` is accepted with Edition 3 flag

---

## Phase 2: AST and Parser

### Step 2.1: AST type declarations

**File**: `compiler/dsl/src/common.rs`

Add to `DataTypeDeclarationKind`:
```rust
Reference(ReferenceDeclaration),
```

Add new struct:
```rust
pub struct ReferenceDeclaration {
    pub type_name: TypeName,
    pub referenced_type_name: TypeName,
}
```

Add to `InitialValueAssignmentKind`:
```rust
Reference(ReferenceInitializer),
```

Add new struct:
```rust
pub struct ReferenceInitializer {
    pub referenced_type_name: TypeName,
    pub initial_value: Option<ConstantKind>,  // NULL or other ref expression
}
```

### Step 2.2: AST expressions

**File**: `compiler/dsl/src/textual.rs`

Add to `ExprKind`:
```rust
Ref(Box<Variable>),       // REF(var)
Deref(Box<Expr>),         // expr^
Null(SourceSpan),         // NULL
```

### Step 2.3: Visitor and Fold

**Files**: `compiler/dsl/src/visitor.rs`, `compiler/dsl/src/fold.rs`

Add visit/fold methods for new AST nodes following the existing derive macro pattern. The `Recurse` derive macro should handle most of this automatically for structs with `#[derive(Recurse)]`.

### Step 2.4: Parser grammar

**File**: `compiler/parser/src/parser.rs`

- **Type specifier rule**: Add `RefTo type_spec` production
- **Primary expression**: Add `Ref LeftParen variable RightParen` for `REF(var)`
- **Primary expression**: Add `Null` as a constant/literal expression
- **Postfix expression**: After parsing a primary expression, check for `Xor`/`^` in postfix position. If the caret follows a variable or function call expression (not in infix position between two operands), wrap in `Deref`

### Step 2.5: Parser tests

**File**: `compiler/parser/src/tests.rs` or `compiler/resources/test/ref_to.st`

Tests to add:
- `parse_when_ref_to_int_type_decl_then_ok` — `TYPE IntRef : REF_TO INT; END_TYPE`
- `parse_when_ref_to_var_decl_then_ok` — `VAR x : REF_TO INT; END_VAR`
- `parse_when_ref_to_nested_then_ok` — `REF_TO REF_TO INT`
- `parse_when_ref_to_array_then_ok` — `REF_TO ARRAY[1..10] OF INT`
- `parse_when_ref_operator_then_ok` — `x := REF(counter);`
- `parse_when_deref_then_ok` — `value := myRef^;`
- `parse_when_deref_assign_then_ok` — `myRef^ := 42;`
- `parse_when_null_literal_then_ok` — `myRef := NULL;`
- `parse_when_null_comparison_then_ok` — `IF myRef <> NULL THEN ... END_IF;`
- `parse_when_caret_xor_then_ok` — `result := a ^ b;` (verify XOR still works)

### Step 2.6: Verification

- All existing parser tests pass
- New REF_TO syntax parses correctly
- Caret disambiguation works (XOR vs dereference)

---

## Phase 3: Semantic Analysis

### Step 3.1: Intermediate type

**File**: `compiler/analyzer/src/intermediate_type.rs`

Add variant:
```rust
Reference {
    target_type: Box<IntermediateType>,
},
```

Add methods:
```rust
pub fn is_reference(&self) -> bool {
    matches!(self, IntermediateType::Reference { .. })
}

pub fn referenced_type(&self) -> Option<&IntermediateType> {
    match self {
        IntermediateType::Reference { target_type } => Some(target_type),
        _ => None,
    }
}
```

Update `size_in_bytes()`: references are 8 bytes (I64).
Update `alignment_bytes()`: references align to 8 bytes.

### Step 3.2: Type resolution

**File**: `compiler/analyzer/src/type_environment.rs`

Handle `InitialValueAssignmentKind::Reference` and `DataTypeDeclarationKind::Reference` in the type resolution pipeline. Resolve `REF_TO INT` to `IntermediateType::Reference { target_type: Int { size: B32 } }`.

### Step 3.3: Expression type resolution

**File**: `compiler/analyzer/src/xform_resolve_expr_types.rs`

- `ExprKind::Ref(var)`: resolve to `IntermediateType::Reference { target_type: typeof(var) }`
- `ExprKind::Deref(expr)`: resolve to the target type of the reference (unwrap one level)
- `ExprKind::Null`: resolve to a special null reference type compatible with any `REF_TO`

### Step 3.4: Semantic rules

**New file**: `compiler/analyzer/src/rule_ref_to.rs`

Implement the following validation rules (each with a new problem code):

| Rule | Test name (BDD) |
|------|-----------------|
| REF operand must be a variable | `ref_when_operand_is_constant_then_error` |
| No REF of VAR_TEMP | `ref_when_operand_is_var_temp_then_error` |
| Deref requires reference type | `deref_when_type_is_not_reference_then_error` |
| Reference type compatibility | `assign_when_ref_types_incompatible_then_error` |
| No arithmetic on references | `arithmetic_when_operand_is_reference_then_error` |
| NULL only for reference types | `null_when_assigned_to_non_reference_then_error` |
| Only = and <> on references | `compare_when_ordering_on_reference_then_error` |

### Step 3.5: Wire in the new rule

**File**: `compiler/analyzer/src/stages.rs`

Add `rule_ref_to::apply` to the semantic validation pipeline.

### Step 3.6: Problem codes

**File**: `compiler/problems/resources/problem-codes.csv`

Add ~6 new P-codes for the semantic rules above. Choose numbers based on the existing range.

**Files**: `docs/compiler/problems/P####.rst` (one per problem code)

Each problem code documentation file includes:
- Description of when the error occurs
- Example code that triggers the error
- Explanation of why it's an error
- Corrected example

### Step 3.7: Verification

- All existing analyzer tests pass
- Each semantic rule has passing positive and negative test cases
- Problem codes are documented

---

## Phase 4: Code Generation

### Step 4.1: New opcodes

**File**: `compiler/container/src/opcode.rs`

Add 2 new opcodes:

```rust
/// Pop a reference (variable index) from stack, null-check, load value at that index.
/// Traps with NullDereference if the reference is null.
pub const LOAD_INDIRECT: u8 = 0x14;

/// Pop a reference then a value from stack, null-check, store value at that index.
/// Traps with NullDereference if the reference is null.
pub const STORE_INDIRECT: u8 = 0x15;
```

All other operations reuse existing opcodes:
- `REF(var)` → `LOAD_CONST_I64` (variable index as compile-time constant)
- `NULL` → `LOAD_CONST_I64 u64::MAX`
- Ref load/store → `LOAD_VAR_I64` / `STORE_VAR_I64`
- Ref comparison → `EQ_I64` / `NE_I64`

### Step 4.2: Compilation rules

**File**: `compiler/codegen/src/compile.rs`

Handle new expression kinds in the expression compiler:

- `ExprKind::Ref(var)`: Resolve variable index at compile time, emit `LOAD_CONST_I64 index`
- `ExprKind::Null`: Emit `LOAD_CONST_I64 u64::MAX`
- `ExprKind::Deref(expr)` as r-value: Compile inner expression, emit `LOAD_INDIRECT`
- `ExprKind::Deref(expr)` as l-value (assignment target): Compile value, compile ref expression, emit `STORE_INDIRECT`
- Reference variable load/store: Use `LOAD_VAR_I64` / `STORE_VAR_I64` (refs are I64 slots)
- Null comparison: Compile ref, emit `LOAD_CONST_I64 u64::MAX`, emit `EQ_I64` or `NE_I64`

### Step 4.3: Emitter

**File**: `compiler/codegen/src/emit.rs`

Add emit methods for `LOAD_INDIRECT` and `STORE_INDIRECT` with correct stack depth tracking:
- `LOAD_INDIRECT`: net stack change = 0 (pop 1, push 1)
- `STORE_INDIRECT`: net stack change = -2 (pop 2)

### Step 4.4: Verification

- All existing codegen tests pass
- New bytecode tests verify correct instruction sequences for REF_TO operations

---

## Phase 5: Virtual Machine

### Step 5.1: New trap

**File**: `compiler/vm/src/error.rs`

Add `Trap::NullDereference` variant with a V-code (e.g., `V4004`). User-facing error (exit code 1), not internal error.

### Step 5.2: Slot helpers

**File**: `compiler/vm/src/value.rs`

```rust
impl Slot {
    pub fn null_ref() -> Slot { Slot(u64::MAX) }
    pub fn is_null_ref(&self) -> bool { self.0 == u64::MAX }
    pub fn as_var_index(&self) -> Result<u16, Trap> {
        if self.is_null_ref() {
            Err(Trap::NullDereference)
        } else {
            Ok(self.0 as u16)
        }
    }
}
```

### Step 5.3: Opcode handlers

**File**: `compiler/vm/src/vm.rs`

Add to the opcode dispatch match:

```rust
opcode::LOAD_INDIRECT => {
    let ref_slot = stack.pop()?;
    let index = ref_slot.as_var_index()?;
    scope.check_access(index)?;
    let value = variables.load(index)?;
    stack.push(value)?;
}
opcode::STORE_INDIRECT => {
    let ref_slot = stack.pop()?;
    let index = ref_slot.as_var_index()?;
    let value = stack.pop()?;
    scope.check_access(index)?;
    variables.store(index, value)?;
}
```

### Step 5.4: Verification

- All existing VM tests pass
- New opcode tests verify load/store through references
- Null dereference produces correct trap

---

## Phase 6: Testing

### VM tests — `compiler/vm/tests/`

| Test file | Contents |
|-----------|----------|
| `execute_load_indirect.rs` | LOAD_INDIRECT with valid ref, verify correct value loaded |
| `execute_store_indirect.rs` | STORE_INDIRECT round-trip, verify value stored through ref |
| `execute_null_deref.rs` | LOAD_INDIRECT/STORE_INDIRECT with null ref, verify NullDereference trap |

### Codegen tests — `compiler/codegen/tests/`

| Test file | Contents |
|-----------|----------|
| `compile_ref.rs` | Bytecode assertions for REF_TO expressions |
| `end_to_end_ref.rs` | Parse → compile → VM run → verify values through references |
| `end_to_end_null_deref.rs` | Parse → compile → VM run → verify null deref produces trap |

### Integration test

Write an OSCAT-representative test program that exercises:
- `REF_TO` variable declarations
- `REF()` operator to create references
- `^` dereference for reading and writing
- `NULL` checks
- Passing references as function block parameters

---

## Files Modified (Summary)

| Phase | File | Change |
|-------|------|--------|
| 1 | `compiler/parser/src/token.rs` | Add RefTo, Ref, Null tokens |
| 1 | `compiler/parser/src/rule_token_no_std_2013.rs` | Gate new tokens behind Edition 3 flag |
| 2 | `compiler/dsl/src/common.rs` | Reference type/initializer variants |
| 2 | `compiler/dsl/src/textual.rs` | Ref, Deref, Null expression kinds |
| 2 | `compiler/dsl/src/visitor.rs` | Visit methods for new nodes |
| 2 | `compiler/dsl/src/fold.rs` | Fold methods for new nodes |
| 2 | `compiler/parser/src/parser.rs` | Grammar rules for REF_TO syntax |
| 3 | `compiler/analyzer/src/intermediate_type.rs` | Reference variant |
| 3 | `compiler/analyzer/src/rule_ref_to.rs` | **New** — semantic rules |
| 3 | `compiler/analyzer/src/stages.rs` | Wire in new rule |
| 3 | `compiler/analyzer/src/type_environment.rs` | Reference type resolution |
| 3 | `compiler/analyzer/src/xform_resolve_expr_types.rs` | Expression type resolution |
| 3 | `compiler/problems/resources/problem-codes.csv` | ~6 new P-codes |
| 3 | `docs/compiler/problems/P####.rst` | Problem documentation |
| 4 | `compiler/container/src/opcode.rs` | 2 new opcodes |
| 4 | `compiler/codegen/src/compile.rs` | Reference compilation |
| 4 | `compiler/codegen/src/emit.rs` | 2 new opcode emitters |
| 5 | `compiler/vm/src/error.rs` | NullDereference trap |
| 5 | `compiler/vm/src/value.rs` | Null ref helpers |
| 5 | `compiler/vm/src/vm.rs` | 2 new opcode handlers |

## Pre-PR Verification

```bash
cd compiler && just
```

This runs compile, test, coverage (85% threshold), and lint (clippy + fmt). All checks must pass before creating a PR.
