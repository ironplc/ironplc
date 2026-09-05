# Keyword Function Forms Design

## Problem

IEC 61131-3 defines function forms for operators: `MOD(a, b)`, `AND(a, b)`, `OR(a, b)`, `XOR(a, b)`, and `NOT(a)`. These are the functional equivalents of the infix/prefix operators `a MOD b`, `a AND b`, `a OR b`, `a XOR b`, and `NOT a`.

The IronPLC parser tokenizes MOD, AND, OR, XOR, and NOT as keyword tokens (`TokenType::Mod`, `TokenType::And`, etc.), not as identifiers. The `function_name()` parser rule only accepts `TokenType::Identifier`, so these keywords cannot appear as function call names. Writing `MOD(a, b)` produces a parse error.

The analyzer already registers signatures for all five functions, and codegen already routes them to the correct opcodes. The problem is purely in the parser.

## Approach

Extend the `function_name()` PEG rule to accept these keyword tokens as alternatives, following the existing `variable_identifier()` pattern in the codebase.

### Parser change

Only the `function_name()` rule in `compiler/parser/src/parser.rs` needs modification. Add `TokenType::Mod`, `TokenType::And`, `TokenType::Or`, `TokenType::Xor`, and `TokenType::Not` as alternatives after the existing `identifier()` path.

### Disambiguation

The PEG `expression()` precedence macro handles operator-vs-function disambiguation naturally. Operator rules like `x:(@) _ tok(TokenType::Mod) _ y:@` require a left-hand operand and cannot match `MOD(...)` at the start of a subexpression. The `function_expression()` at the bottom of the precedence chain matches it instead.

### NOT special case

`NOT(x)` will continue to parse as `NOT (x)` (unary operator applied to parenthesized expression) in expression context, because the `unary_expression` rule consumes the `NOT` token before `function_expression` gets a chance. This is semantically equivalent for single boolean arguments — the codegen already emits the correct NOT opcode via the unary operator path. No special handling is needed.

### Scope of changes

- **Parser:** Modify `function_name()` rule.
- **Lexer:** No changes.
- **Analyzer:** No changes (signatures already registered).
- **Codegen:** No changes (routing already exists).

## Signatures and code generation

Each function form is one row of the table in
`compiler/analyzer/src/intermediates/operator_function_form.rs`: the function
name, the operator it is a form of, the category every operand has, and whether
the result is the operand type or `BOOL`. The analyzer derives the signature
from the row, and codegen asks the row which operator to compile the call as.
Neither side keeps a second copy, so the function form cannot accept a narrower
set of operands than its operator, or compile to something other than it.

The categories are those of IEC 61131-3. The bitwise boolean functions were
once declared `BOOL`-only, which rejected `AND(w1, w2)` on `WORD` while
`w1 AND w2` was accepted ([#1567](https://github.com/ironplc/ironplc/issues/1567)).
`MOD` was once declared `ANY_NUM`, which let `MOD(r1, r2)` on `REAL` through
analysis to fail in codegen, which has no floating-point remainder opcode
([#1619](https://github.com/ironplc/ironplc/issues/1619)); IEC 61131-3 Table 24
defines `MOD` over `ANY_INT` only.

The operator spelling of `MOD` is held to the same row. The rule
`rule_operator_operand_type_check` looks the row up by operator and checks each
operand of `a MOD b` with the same type-compatibility predicate the
function-call check applies to `MOD(a, b)`, so the two spellings agree by
construction. It is the only operator checked this way: the operator spellings
of `+`, `-`, `*` and `/` also compile for `TIME` and bit-string operands, and
holding them to their rows is a separate decision
([#1621](https://github.com/ironplc/ironplc/issues/1621)).

**REQ-KF-analyzer-001** `ADD`, `SUB`, `MUL` and `DIV` accept two `ANY_NUM` operands and return the operand type.

**REQ-KF-analyzer-002** `GT`, `GE`, `EQ`, `LE`, `LT` and `NE` accept two `ANY_ELEMENTARY` operands and return `BOOL`.

**REQ-KF-analyzer-003** `AND`, `OR` and `XOR` accept two `ANY_BIT` operands (`BOOL`, `BYTE`, `WORD`, `DWORD`, `LWORD`) and return the operand type.

**REQ-KF-analyzer-004** `NOT` accepts one `ANY_BIT` operand and returns the operand type.

**REQ-KF-analyzer-005** An argument outside the category of the function form's operands is reported as P4026.

**REQ-KF-analyzer-006** `MOD` accepts two `ANY_INT` operands and returns the operand type.

**REQ-KF-analyzer-007** An operand of the `MOD` operator outside `ANY_INT` is reported as P4049, and an operand `MOD(a, b)` accepts, `a MOD b` accepts.

**REQ-KF-codegen-001** A call to the function form of an operator, assigned to a variable of its result type, compiles to the same bytecode as the operator expression with the same operands.

## Extensible forms

IEC 61131-3 marks `ADD`, `MUL`, `AND`, `OR` and `XOR` *extensible* (Tables 24
and 26): a call takes any number of inputs `IN1`, `IN2`, ..., `INn`. The row
says so in its arity column, and the analyzer registers the row through the
same extensible-signature mechanism `MUX` uses, with `IN1` and `IN2` declared
and no upper bound. A `FunctionSignature` continues its input parameters past
the declared ones for an extensible function by counting on from the last
declared name, so the argument-type check and named-argument binding treat the
third input exactly as the second. Codegen folds the arguments from the left:
`ADD(a, b, c)` is `(a + b) + c`, which for the two inputs of a binary form is
the same code as before. The remaining forms stay binary. The standard also
marks the ordered comparisons extensible, meaning a monotonic sequence
(`GT(a, b, c)` is `a > b AND b > c`); that is not a fold and is not
implemented ([#1618](https://github.com/ironplc/ironplc/issues/1618) asked
only for the five above).

**REQ-KF-analyzer-008** `ADD`, `MUL`, `AND`, `OR` and `XOR` accept any number of inputs of their operand category, two or more.

**REQ-KF-analyzer-009** Every other function form of an operator accepts exactly the inputs it declares; a call with more is reported as P4018.

**REQ-KF-analyzer-010** Every input of an extensible call is checked against the operand category, so an input beyond the second outside it is reported as P4026.

**REQ-KF-analyzer-011** The named inputs of an extensible call bind by number, `IN3` after `IN2`, so a call may name inputs beyond the declared ones and in any order.

**REQ-KF-codegen-002** A call to an extensible function form with `n` inputs compiles to the same bytecode as the operator expression folded from the left over the same inputs.

## Testing

- Add parser tests for each keyword-as-function-call (`MOD(a, b)`, `AND(a, b)`, `OR(a, b)`, `XOR(a, b)`).
- Add end-to-end tests for MOD, AND, OR, XOR through the full pipeline.
- Confirm `NOT(x)` produces the correct result via the existing unary operator path.
