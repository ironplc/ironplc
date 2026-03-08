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

## Testing

- Add parser tests for each keyword-as-function-call (`MOD(a, b)`, `AND(a, b)`, `OR(a, b)`, `XOR(a, b)`).
- Add end-to-end tests for MOD, AND, OR, XOR through the full pipeline.
- Confirm `NOT(x)` produces the correct result via the existing unary operator path.
