# Keyword Function Forms Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Allow MOD, AND, OR, XOR, NOT to be used as function call names in IEC 61131-3 programs.

**Architecture:** Extend the parser's `function_name()` PEG rule to accept keyword tokens as alternatives, following the existing `variable_identifier()` pattern. The lexer, analyzer, and codegen need no changes — only the parser rule and new tests.

**Tech Stack:** Rust, `peg` parser crate, `logos` lexer

---

### Task 1: Extend the `function_name()` parser rule

**Files:**
- Modify: `compiler/parser/src/parser.rs:1005`

**Step 1: Write the failing parser test**

Add to the bottom of `compiler/parser/src/tests.rs`, inside the `mod test` block:

```rust
#[test]
fn parse_when_mod_function_call_then_parses() {
    parse_text(
        "FUNCTION_BLOCK CALLER
VAR
    result : INT;
    a : INT;
    b : INT;
END_VAR
    result := MOD(a, b);
END_FUNCTION_BLOCK",
    );
}

#[test]
fn parse_when_and_function_call_then_parses() {
    parse_text(
        "FUNCTION_BLOCK CALLER
VAR
    result : BOOL;
    a : BOOL;
    b : BOOL;
END_VAR
    result := AND(a, b);
END_FUNCTION_BLOCK",
    );
}

#[test]
fn parse_when_or_function_call_then_parses() {
    parse_text(
        "FUNCTION_BLOCK CALLER
VAR
    result : BOOL;
    a : BOOL;
    b : BOOL;
END_VAR
    result := OR(a, b);
END_FUNCTION_BLOCK",
    );
}

#[test]
fn parse_when_xor_function_call_then_parses() {
    parse_text(
        "FUNCTION_BLOCK CALLER
VAR
    result : BOOL;
    a : BOOL;
    b : BOOL;
END_VAR
    result := XOR(a, b);
END_FUNCTION_BLOCK",
    );
}
```

Note: No test for `NOT(x)` as a function call because the parser will always consume `NOT` as the unary operator before reaching the function call rule. `NOT(x)` already works via the unary path (the `NOT` operator applied to the parenthesized expression `(x)`).

**Step 2: Run tests to verify they fail**

Run: `cd compiler && cargo test --package ironplc-parser parse_when_mod_function_call parse_when_and_function_call parse_when_or_function_call parse_when_xor_function_call -- --nocapture`
Expected: 4 FAIL — parse errors because keyword tokens are not accepted as function names.

**Step 3: Extend the `function_name()` rule**

In `compiler/parser/src/parser.rs`, change line 1005 from:

```rust
rule function_name() -> Id = standard_function_name() / derived_function_name()
```

to:

```rust
rule function_name() -> Id = standard_function_name() / derived_function_name() / t:tok(TokenType::Mod) { Id::from(t.text.as_str()).with_position(t.span.clone()) } / t:tok(TokenType::And) { Id::from(t.text.as_str()).with_position(t.span.clone()) } / t:tok(TokenType::Or) { Id::from(t.text.as_str()).with_position(t.span.clone()) } / t:tok(TokenType::Xor) { Id::from(t.text.as_str()).with_position(t.span.clone()) } / t:tok(TokenType::Not) { Id::from(t.text.as_str()).with_position(t.span.clone()) }
```

This follows the same pattern as `variable_identifier()` on line 229 of the same file.

**Step 4: Run tests to verify they pass**

Run: `cd compiler && cargo test --package ironplc-parser parse_when_mod_function_call parse_when_and_function_call parse_when_or_function_call parse_when_xor_function_call -- --nocapture`
Expected: 4 PASS

**Step 5: Verify existing operator tests still pass**

Run: `cd compiler && cargo test --package ironplc-parser -- --nocapture`
Expected: All parser tests pass (operator forms `a MOD b`, `a AND b`, etc. still work).

**Step 6: Commit**

```bash
git add compiler/parser/src/parser.rs compiler/parser/src/tests.rs
git commit -m "feat: allow MOD, AND, OR, XOR, NOT as function call names in parser"
```

---

### Task 2: Add compile-level bytecode tests for MOD, AND, OR, XOR

**Files:**
- Modify: `compiler/codegen/tests/compile_func_forms.rs`

**Step 1: Add compile tests**

Add to the end of `compiler/codegen/tests/compile_func_forms.rs`:

```rust
#[test]
fn compile_when_mod_function_then_produces_mod_bytecode() {
    assert_two_arg_bytecode(&two_arg_program("MOD", "DINT"), 0x34);
}

// --- Boolean functions ---
// Note: Boolean functions use BOOL type and emit BOOL_AND/OR/XOR opcodes.
// The bytecode layout differs from DINT because BOOL uses different
// LOAD/STORE opcodes (0x06/0x1D for BOOL vs 0x01/0x18 for I32).

#[test]
fn compile_when_and_function_then_produces_and_bytecode() {
    let source = "
PROGRAM main
  VAR
    x : BOOL;
    y : BOOL;
  END_VAR
  x := TRUE;
  y := AND(x, FALSE);
END_PROGRAM
";
    let library = parse(source);
    let container = compile(&library).unwrap();
    let bytecode = container.code.get_function_bytecode(1).unwrap();
    // Verify the BOOL_AND opcode (0x54) appears in the bytecode
    assert!(
        bytecode.contains(&0x54),
        "Expected BOOL_AND opcode 0x54 in bytecode: {:02X?}",
        bytecode
    );
}

#[test]
fn compile_when_or_function_then_produces_or_bytecode() {
    let source = "
PROGRAM main
  VAR
    x : BOOL;
    y : BOOL;
  END_VAR
  x := FALSE;
  y := OR(x, TRUE);
END_PROGRAM
";
    let library = parse(source);
    let container = compile(&library).unwrap();
    let bytecode = container.code.get_function_bytecode(1).unwrap();
    assert!(
        bytecode.contains(&0x55),
        "Expected BOOL_OR opcode 0x55 in bytecode: {:02X?}",
        bytecode
    );
}

#[test]
fn compile_when_xor_function_then_produces_xor_bytecode() {
    let source = "
PROGRAM main
  VAR
    x : BOOL;
    y : BOOL;
  END_VAR
  x := TRUE;
  y := XOR(x, TRUE);
END_PROGRAM
";
    let library = parse(source);
    let container = compile(&library).unwrap();
    let bytecode = container.code.get_function_bytecode(1).unwrap();
    assert!(
        bytecode.contains(&0x56),
        "Expected BOOL_XOR opcode 0x56 in bytecode: {:02X?}",
        bytecode
    );
}
```

**Step 2: Run tests to verify they pass**

Run: `cd compiler && cargo test --test compile_func_forms -- --nocapture`
Expected: All 14 tests pass (10 existing + 4 new).

**Step 3: Commit**

```bash
git add compiler/codegen/tests/compile_func_forms.rs
git commit -m "test: add compile tests for MOD, AND, OR, XOR function forms"
```

---

### Task 3: Add end-to-end tests for MOD, AND, OR, XOR, NOT

**Files:**
- Modify: `compiler/codegen/tests/end_to_end_func_forms.rs`

**Step 1: Add end-to-end tests**

Add to the end of `compiler/codegen/tests/end_to_end_func_forms.rs`:

```rust
#[test]
fn end_to_end_when_mod_function_then_returns_remainder() {
    let source = "
PROGRAM main
  VAR
    result : DINT;
  END_VAR
  result := MOD(10, 3);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), 1);
}

#[test]
fn end_to_end_when_and_function_then_returns_bool() {
    let source = "
PROGRAM main
  VAR
    true_result : DINT;
    false_result : DINT;
  END_VAR
  true_result := AND(TRUE, TRUE);
  false_result := AND(TRUE, FALSE);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), 1);
    assert_eq!(bufs.vars[1].as_i32(), 0);
}

#[test]
fn end_to_end_when_or_function_then_returns_bool() {
    let source = "
PROGRAM main
  VAR
    true_result : DINT;
    false_result : DINT;
  END_VAR
  true_result := OR(FALSE, TRUE);
  false_result := OR(FALSE, FALSE);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), 1);
    assert_eq!(bufs.vars[1].as_i32(), 0);
}

#[test]
fn end_to_end_when_xor_function_then_returns_bool() {
    let source = "
PROGRAM main
  VAR
    true_result : DINT;
    false_result : DINT;
  END_VAR
  true_result := XOR(TRUE, FALSE);
  false_result := XOR(TRUE, TRUE);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), 1);
    assert_eq!(bufs.vars[1].as_i32(), 0);
}

#[test]
fn end_to_end_when_not_parens_then_returns_negation() {
    // NOT(x) parses as unary NOT applied to parenthesized expression (x).
    // This is semantically equivalent to the NOT function form.
    let source = "
PROGRAM main
  VAR
    true_result : DINT;
    false_result : DINT;
  END_VAR
  true_result := NOT(FALSE);
  false_result := NOT(TRUE);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), 1);
    assert_eq!(bufs.vars[1].as_i32(), 0);
}
```

**Step 2: Run tests to verify they pass**

Run: `cd compiler && cargo test --test end_to_end_func_forms -- --nocapture`
Expected: All 15 tests pass (10 existing + 5 new).

**Step 3: Verify existing operator forms still work**

Run: `cd compiler && cargo test -- --nocapture`
Expected: All tests pass. Operator forms (`a MOD b`, `a AND b`, etc.) are unaffected.

**Step 4: Commit**

```bash
git add compiler/codegen/tests/end_to_end_func_forms.rs
git commit -m "test: add end-to-end tests for MOD, AND, OR, XOR, NOT function forms"
```

---

### Task 4: Run full CI and verify

**Step 1: Run full CI pipeline**

Run: `cd compiler && just`
Expected: All compile, test, coverage, clippy, and formatting checks pass.

**Step 2: Fix any issues**

If clippy or formatting fails, fix and re-run.

**Step 3: Final commit if needed**

Only if fixes were required in Step 2.
