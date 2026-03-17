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

Add new enum and struct:
```rust
/// Initial value for a REF_TO variable declaration.
/// Separate from ConstantKind because NULL is not a general-purpose constant.
pub enum ReferenceInitialValue {
    Null(SourceSpan),         // := NULL
    Ref(Variable),            // := REF(var)
}

pub struct ReferenceInitializer {
    pub referenced_type_name: TypeName,
    pub initial_value: Option<ReferenceInitialValue>,
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

- **Type specifier rule**: Add `RefTo type_spec` production for `REF_TO INT`, etc. This is a recursive production so `REF_TO REF_TO INT` parses syntactically (rejected later by the semantic analyzer — see P2036).
- **Primary expression**: Add `Ref LeftParen variable RightParen` for `REF(var)`
- **Primary expression**: Add `Null` as a constant/literal expression
- **Postfix dereference**: Modify the base-case rule that the `precedence!` macro calls (the rule producing primary/unary expressions) to add a postfix caret loop *after* parsing the primary expression but *before* returning to the precedence macro for infix operators:

```rust
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

This gives dereference the highest binding power (tighter than any infix operator). The `precedence!` macro's XOR level (`x:(@) _ tok(TokenType::Xor) _ y:@`) is unaffected because the postfix carets are consumed before the precedence macro sees them. The loop (`*`) handles future multi-level dereference even though `p^^` is currently rejected by the semantic analyzer.

### Step 2.5: Parser tests

**File**: `compiler/parser/src/tests.rs` or `compiler/resources/test/ref_to.st`

Tests to add:
- `parse_when_ref_to_int_type_decl_then_ok` — `TYPE IntRef : REF_TO INT; END_TYPE`
- `parse_when_ref_to_var_decl_then_ok` — `VAR x : REF_TO INT; END_VAR`
- `parse_when_ref_to_nested_then_ok` — `REF_TO REF_TO INT` (parses syntactically; rejected in semantic analysis)
- `parse_when_ref_to_array_type_then_ok` — `REF_TO ARRAY[1..10] OF INT` (ref to an array type)
- `parse_when_ref_operator_then_ok` — `x := REF(counter);`
- `parse_when_deref_then_ok` — `value := myRef^;`
- `parse_when_deref_assign_then_ok` — `myRef^ := 42;`
- `parse_when_null_literal_then_ok` — `myRef := NULL;`
- `parse_when_null_init_then_ok` — `VAR x : REF_TO INT := NULL; END_VAR`
- `parse_when_null_comparison_then_ok` — `IF myRef <> NULL THEN ... END_IF;`
- `parse_when_caret_xor_then_ok` — `result := a ^ b;` (verify XOR still works)
- `parse_when_deref_then_xor_then_ok` — `result := myRef^ XOR b;` (dereference + XOR in same expression)

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
- `ExprKind::Deref(expr)`: resolve to the target type of the reference (unwrap one level). If the inner expression is not a reference type, this is a type error caught by rule P2031.
- `ExprKind::Null`: resolve to `IntermediateType::Reference { target_type: Box::new(IntermediateType::Bool) }` as a placeholder. NULL's actual type compatibility is handled contextually by the semantic rules — the placeholder type is never used for dereference (dereferencing NULL traps at runtime). See the design document's "NULL Type Resolution Strategy" section for the full rationale.

### Step 3.4: Semantic rules

**New file**: `compiler/analyzer/src/rule_ref_to.rs`

Implement the following validation rules:

| Rule | Problem code | Test name (BDD) |
|------|-------------|-----------------|
| REF operand must be a simple variable | P2028 | `ref_when_operand_is_constant_then_error` |
| No REF of ephemeral variables (VAR_TEMP in FUNCTION; VAR_INPUT/VAR_OUTPUT in FUNCTION) | P2029 | `ref_when_operand_is_var_temp_then_error`, `ref_when_operand_is_function_var_input_then_error`, `ref_when_operand_is_fb_var_input_then_ok` |
| No REF of array elements | P2030 | `ref_when_operand_is_array_element_then_error` |
| Deref requires reference type | P2031 | `deref_when_type_is_not_reference_then_error`, `deref_when_type_is_reference_then_ok` |
| Reference type compatibility | P2032 | `assign_when_ref_types_incompatible_then_error`, `assign_when_ref_types_match_then_ok` |
| No arithmetic on references | P2033 | `arithmetic_when_operand_is_reference_then_error` |
| NULL only for reference types | P2034 | `null_when_assigned_to_non_reference_then_error`, `null_when_assigned_to_reference_then_ok` |
| Only = and <> on references | P2035 | `compare_when_ordering_on_reference_then_error`, `compare_when_equality_on_reference_then_ok` |
| No nested REF_TO | P2036 | `ref_to_when_nested_then_error`, `ref_to_when_single_level_then_ok` |

Note on P2029: `VAR_INPUT` and `VAR_OUTPUT` in a `FUNCTION` (not `FUNCTION_BLOCK`) are stack-allocated and destroyed when the function returns, making references to them dangling. The same variables in a `FUNCTION_BLOCK` are persistent (the FB instance lives in the variable table) and therefore safe to reference.

### Step 3.5: Wire in the new rule

**File**: `compiler/analyzer/src/stages.rs`

Add `rule_ref_to::apply` to the semantic validation pipeline.

### Step 3.6: Problem codes

**File**: `compiler/problems/resources/problem-codes.csv`

Add the following P-codes (continuing from existing P2027):

```csv
P2028,RefOperandNotVariable,REF() operand must be a simple variable
P2029,RefOfEphemeralVariable,REF() of stack-allocated variable (VAR_TEMP or FUNCTION VAR_INPUT/VAR_OUTPUT) would create a dangling reference
P2030,RefOfArrayElement,REF() of array element is not supported
P2031,DerefRequiresReferenceType,Dereference operator (^) requires a REF_TO type
P2032,ReferenceTypeMismatch,Reference type mismatch in assignment
P2033,ArithmeticOnReference,Arithmetic operations are not allowed on reference types
P2034,NullRequiresReferenceType,NULL can only be assigned to a REF_TO type
P2035,OrderingOnReference,Ordering comparison on reference types (only = and <> are allowed)
P2036,NestedRefToNotSupported,Nested REF_TO (multi-level indirection) is not supported
```

**File**: `compiler/vm/resources/problem-codes.csv`

Add the NullDereference trap code:

```csv
V4004,NullDereference,Null reference dereference during program execution,false
```

**Files**: `docs/compiler/problems/P2028.rst` through `docs/compiler/problems/P2036.rst` (one per problem code)

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

#### Expression compilation (new arms in `compile_expr` match)

Add match arms for the three new `ExprKind` variants:

- `ExprKind::Ref(var)`: Resolve variable index at compile time, emit `LOAD_CONST_I64 index`
- `ExprKind::Null`: Emit `LOAD_CONST_I64 u64::MAX`
- `ExprKind::Deref(expr)` (r-value): Compile inner expression, emit `LOAD_INDIRECT`

#### Reference variable type info

Register reference variables with `VarTypeInfo { op_width: W64, signedness: Unsigned, storage_bits: 64 }` so the existing `emit_load_var()` / `emit_store_var()` helpers select `LOAD_VAR_I64` / `STORE_VAR_I64` without changes.

#### Default initialization of reference variables

In the variable initialization phase (where the codegen emits initial values for all declared variables), add a branch for reference-type variables:

- If no explicit initializer or `:= NULL`: emit `LOAD_CONST_I64 u64::MAX` + `STORE_VAR_I64 ref_slot`
- If `:= REF(var)`: emit `LOAD_CONST_I64 var_index` + `STORE_VAR_I64 ref_slot`

**This is critical for safety.** The variable table initializes all slots to `Slot(0)`, which is a valid variable index. Without explicit null initialization, an uninitialized `REF_TO` variable would silently point to variable 0 instead of trapping on dereference.

#### L-value dereference (assignment through a reference)

Add a new code path at the top of the `StmtKind::Assignment` handler, before `resolve_variable_name()`:

1. Check if the assignment target is a `Deref` expression
2. If so:
   a. Compile the value expression (pushes value)
   b. Compile the inner expression of the `Deref` (pushes the reference index)
   c. Emit `STORE_INDIRECT` (pops ref, pops value, stores value at the referenced variable)
3. Return early — skip the normal assignment flow

This must come before the existing assignment logic because `resolve_variable_name()` cannot handle `Deref` targets (they don't resolve to a static variable index).

#### Null comparison

Null comparison: Compile ref, emit `LOAD_CONST_I64 u64::MAX`, emit `EQ_I64` or `NE_I64`. This reuses existing comparison opcodes.

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

Add `Trap::NullDereference` variant with V-code `V4004`. User-facing error (exit code 1), not internal error. Add the Display impl arm:

```rust
Trap::NullDereference => write!(f, "null reference dereference"),
```

**File**: `compiler/vm/resources/problem-codes.csv`

Add entry:
```csv
V4004,NullDereference,Null reference dereference during program execution,false
```

### Step 5.2: Slot helpers

**File**: `compiler/vm/src/value.rs`

```rust
impl Slot {
    pub fn null_ref() -> Slot { Slot(u64::MAX) }
    pub fn is_null_ref(&self) -> bool { self.0 == u64::MAX }
    pub fn as_var_index(&self) -> Result<u16, Trap> {
        if self.is_null_ref() {
            // User program error: dereferencing an uninitialized or NULL reference
            Err(Trap::NullDereference)
        } else if self.0 > u16::MAX as u64 {
            // Internal error: codegen produced an out-of-range index (compiler bug)
            Err(Trap::InvalidVariableIndex(self.0 as u16))
        } else {
            Ok(self.0 as u16)
        }
    }
}
```

The two failure modes have different severity:
- `NullDereference` (V4004, exit code 1) — user program error
- `InvalidVariableIndex` (V9005, exit code 3) — internal compiler error, should never occur

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
| `end_to_end_ref_default_init.rs` | Verify uninitialized REF_TO variable traps on dereference (default null initialization) |
| `end_to_end_ref_lvalue_deref.rs` | `ref^ := value` assignment through reference, verify value written to referenced variable |
| `end_to_end_ref_aliasing.rs` | Two references to same variable — write through one, read through the other |

### Semantic analysis tests — `compiler/analyzer/tests/` or inline

| Test | Contents |
|------|----------|
| `ref_when_operand_is_function_var_input_then_error` | `REF()` of VAR_INPUT in FUNCTION is rejected (P2029) |
| `ref_when_operand_is_fb_var_input_then_ok` | `REF()` of VAR_INPUT in FUNCTION_BLOCK is allowed |
| `ref_when_operand_is_array_element_then_error` | `REF(arr[3])` is rejected (P2030) |
| `ref_to_when_nested_then_error` | `REF_TO REF_TO INT` is rejected (P2036) |
| `deref_assign_when_target_is_deref_then_ok` | `ref^ := 42` compiles successfully |

### Integration test

Write an OSCAT-representative test program that exercises:
- `REF_TO` variable declarations (with and without `:= NULL` initializer)
- `REF()` operator to create references
- `^` dereference for reading and writing (including l-value `ref^ := value`)
- `NULL` checks (`IF ref <> NULL THEN ... END_IF`)
- Passing references as function block parameters
- Default initialization behavior (uninitialized ref traps on dereference)

---

## Files Modified (Summary)

| Phase | File | Change |
|-------|------|--------|
| 1 | `compiler/parser/src/token.rs` | Add RefTo, Ref, Null tokens |
| 1 | `compiler/parser/src/rule_token_no_std_2013.rs` | Gate new tokens behind Edition 3 flag |
| 2 | `compiler/dsl/src/common.rs` | Reference type/initializer variants, ReferenceInitialValue enum |
| 2 | `compiler/dsl/src/textual.rs` | Ref, Deref, Null expression kinds |
| 2 | `compiler/dsl/src/visitor.rs` | Visit methods for new nodes |
| 2 | `compiler/dsl/src/fold.rs` | Fold methods for new nodes |
| 2 | `compiler/parser/src/parser.rs` | Grammar rules for REF_TO syntax, postfix caret loop |
| 3 | `compiler/analyzer/src/intermediate_type.rs` | Reference variant |
| 3 | `compiler/analyzer/src/rule_ref_to.rs` | **New** — 9 semantic rules (P2028-P2036) |
| 3 | `compiler/analyzer/src/stages.rs` | Wire in new rule |
| 3 | `compiler/analyzer/src/type_environment.rs` | Reference type resolution |
| 3 | `compiler/analyzer/src/xform_resolve_expr_types.rs` | Expression type resolution (Ref, Deref, Null) |
| 3 | `compiler/problems/resources/problem-codes.csv` | 9 new P-codes (P2028-P2036) |
| 3 | `docs/compiler/problems/P2028.rst` through `P2036.rst` | Problem documentation |
| 4 | `compiler/container/src/opcode.rs` | 2 new opcodes (LOAD_INDIRECT, STORE_INDIRECT) |
| 4 | `compiler/codegen/src/compile.rs` | Reference compilation, l-value deref path, default null init |
| 4 | `compiler/codegen/src/emit.rs` | 2 new opcode emitters |
| 5 | `compiler/vm/src/error.rs` | NullDereference trap variant |
| 5 | `compiler/vm/resources/problem-codes.csv` | V4004 NullDereference |
| 5 | `compiler/vm/src/value.rs` | Null ref helpers (null_ref, is_null_ref, as_var_index) |
| 5 | `compiler/vm/src/vm.rs` | 2 new opcode handlers |

## Pre-PR Verification

```bash
cd compiler && just
```

This runs compile, test, coverage (85% threshold), and lint (clippy + fmt). All checks must pass before creating a PR.
