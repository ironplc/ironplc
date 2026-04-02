# Syntax Support Guide

This guide describes everything needed to add support for new syntax in the IronPLC compiler. Follow this guide when adding new language features, vendor extensions, or fixing syntax-related issues.

> **Note**: This covers the full pipeline from lexer through execution. For general compiler architecture, see [compiler-architecture.md](compiler-architecture.md). For IEC 61131-3 compliance rules, see [iec-61131-3-compliance.md](iec-61131-3-compliance.md).

## Quick Checklist

When adding new syntax, ensure every applicable item is complete:

- [ ] **Lexer**: Add token types in `parser/src/token.rs` (if new keywords/operators)
- [ ] **Token transforms**: Add demotion or insertion logic (if conditionally enabled)
- [ ] **Parser**: Add grammar rules in `parser/src/parser.rs`
- [ ] **AST**: Add/modify nodes in the `dsl` crate
- [ ] **Analyzer**: Add semantic validation in `analyzer/`
- [ ] **Codegen**: Add bytecode emission in `codegen/`
- [ ] **plc2plc renderer**: Update `plc2plc/src/renderer.rs` to render the new syntax
- [ ] **plc2plc round-trip test**: Parse → render → compare against expected output
- [ ] **End-to-end execution test**: Parse → compile → run → verify variable values
- [ ] **Non-standard gating**: If not standard IEC 61131-3, gate behind `--allow-x` flag
- [ ] **LSP integration**: If a new `--allow-x` flag, add to LSP `extract_compiler_options`
- [ ] **Documentation**: If a new `--allow-x` flag, update `docs/explanation/enabling-dialects-and-features.rst`, `docs/reference/compiler/ironplcc.rst`, and the flag table in this file

Not every syntax change requires all items. A new operator might not need new tokens. A token-level fix might not need codegen changes. Use judgment, but **always** include both round-trip and execution tests when the syntax produces executable code.

## Lexer and Token Patterns

The lexer lives in `parser/src/lexer.rs` and uses the `logos` crate. Token types are defined in `parser/src/token.rs` with ~200+ variants.

### Adding New Tokens

Add token definitions to `parser/src/token.rs`:

```rust
// For keywords (case-insensitive):
#[token("MY_KEYWORD", ignore(case))]
MyKeyword,

// For operators/punctuation:
#[token("=>")]
FatArrow,
```

Keywords are case-insensitive (`ignore(case)`). Identifiers have lower priority than keywords to avoid conflicts.

### Token Demotion Pattern

**When to use**: When a keyword is only valid under certain conditions (e.g., Edition 3 mode, or a vendor extension flag) and programs may use that keyword as an identifier otherwise.

**How it works**: Define the token as a specific type in the lexer, then "demote" it to `TokenType::Identifier` in a transform pass when the feature is disabled.

**Reference implementation**: `parser/src/xform_demote_edition3_keywords.rs`

```rust
pub fn apply(tokens: &mut [Token], options: &CompilerOptions) {
    let demote_time_types = !options.allow_iec_61131_3_2013;
    let demote_ref = !options.allow_iec_61131_3_2013 && !options.allow_ref_to;

    for tok in tokens.iter_mut() {
        match tok.token_type {
            TokenType::Ltime | TokenType::Ldate | TokenType::Ltod | TokenType::Ldt
                if demote_time_types => {
                tok.token_type = TokenType::Identifier;
            }
            TokenType::RefTo | TokenType::Ref | TokenType::Null if demote_ref => {
                tok.token_type = TokenType::Identifier;
            }
            _ => {}
        }
    }
}
```

**Key points**:
- The transform runs between lexing and parsing
- When the feature is enabled, tokens keep their specific type and the parser can match on them
- When disabled, tokens become identifiers, so programs can use those names freely
- Each demotion transform is its own `xform_*.rs` module in `parser/src/`

### Validation Rule Pattern

**When to use**: When the syntax should always be recognized by the lexer but rejected unless a flag is set. Useful when the syntax cannot be confused with an identifier (e.g., `//` comments).

**How it works**: The lexer always tokenizes the syntax. A separate validation rule checks the tokens and produces diagnostics when the flag is not set.

**Reference implementation**: `parser/src/rule_token_no_c_style_comment.rs`

```rust
pub fn apply(tokens: &[Token], options: &CompilerOptions) -> Result<(), Vec<Diagnostic>> {
    if options.allow_c_style_comments {
        return Ok(());
    }

    let mut errors = Vec::new();
    for tok in tokens {
        if tok.token_type == TokenType::Comment && tok.text.starts_with("//") {
            errors.push(Diagnostic::problem(
                Problem::CStyleComment,
                Label::span(tok.span.clone(), "Comment"),
            ));
        }
    }

    if !errors.is_empty() {
        return Err(errors);
    }
    Ok(())
}
```

**Key points**:
- Returns `Result<(), Vec<Diagnostic>>` — collects multiple errors
- Uses problem codes from the shared `ironplc_problems` crate
- Always include tests for both allowed and disallowed cases

### Choosing Between Demotion and Validation

| Scenario | Use |
|----------|-----|
| New keyword that could conflict with existing identifiers | Token demotion |
| Syntax that is always distinct from standard syntax | Validation rule |
| Feature controlled by `--dialect` (edition selection) | Token demotion |
| Feature controlled by `--allow-x` vendor flag | Either, depending on conflict risk |

### Token Insertion Pattern

**When to use**: When the compiler needs to fix up the token stream to handle common non-standard patterns (e.g., missing semicolons).

**Reference implementation**: `parser/src/xform_tokens.rs`

```rust
pub fn insert_keyword_statement_terminators(
    input: Vec<Token>,
    _file_id: &FileId,
    options: &CompilerOptions,
) -> Vec<Token> {
    if !options.allow_missing_semicolon {
        return input;
    }
    // ... insert semicolons after END_IF, END_STRUCT when missing
}
```

## Non-Standard Syntax Gating (`--allow-x` Flags)

**Rule**: Anything not in the IEC 61131-3 standard **must** be gated behind an `--allow-x` flag. Using `--dialect=rusty` enables all vendor extensions.

### Before Creating a New Flag

**Always check existing flags first**. Group related extensions under one flag when they represent the same vendor behavior.

Current flags in `CompilerOptions` (`parser/src/options.rs`):

| Flag | CLI | Purpose |
|------|-----|---------|
| `allow_iec_61131_3_2013` | Set by `--dialect` | Enables Edition 3 keywords (set by `iec61131-3-ed3` dialect) |
| `allow_c_style_comments` | `--allow-c-style-comments` | Permits `//` and `/* */` comments |
| `allow_constant_type_params` | `--allow-constant-type-params` | Constants in type params (e.g., `STRING[MY_CONST]`) |
| `allow_empty_var_blocks` | `--allow-empty-var-blocks` | Empty variable blocks (VAR END_VAR etc.) |
| `allow_missing_semicolon` | `--allow-missing-semicolon` | Inserts semicolons after END_IF etc. |
| `allow_ref_to` | `--allow-ref-to` | REF_TO/REF/NULL syntax without full Edition 3 |
| `allow_sizeof` | `--allow-sizeof` | SIZEOF() operator (returns size in bytes) |
| `allow_time_as_function_name` | `--allow-time-as-function-name` | TIME as function name (OSCAT compat) |
| `allow_top_level_var_global` | `--allow-top-level-var-global` | VAR_GLOBAL outside CONFIGURATION |

### Dialects

Dialects (`--dialect`) set the base configuration. Individual `--allow-*` flags can override on top.

| Dialect | `--dialect` value | Edition 3 types | REF_TO | Vendor extensions |
|---------|-------------------|----------------|--------|-------------------|
| IEC 61131-3 Ed 2 (default) | `iec61131-3-ed2` | OFF | OFF | all OFF |
| IEC 61131-3 Ed 3 | `iec61131-3-ed3` | ON | ON | all OFF |
| RuSTy | `rusty` | OFF | ON | all ON |

### Grouping Guidance

- If the extension is a syntactic variation of something an existing flag covers, add it to that flag
- If the extension is common across multiple vendors and represents the same concept, group under one flag
- If the extension is unique to a specific vendor behavior, create a new flag
- Keep flag names descriptive: `allow_<what_it_allows>`

### Adding a New Flag

When no existing flag covers the extension, add a new one. Update these files in order:

#### 1. `CompilerOptions` struct (`parser/src/options.rs`)

```rust
#[derive(Debug, Default, Clone, Copy)]
pub struct CompilerOptions {
    // ... existing fields ...
    pub allow_my_extension: bool,
}
```

#### 2. CLI `FileArgs` (`plc2x/bin/main.rs`)

Add the clap argument:

```rust
/// Allow [description of what this enables].
/// This is a vendor extension not part of the IEC 61131-3 standard.
#[arg(long)]
allow_my_extension: bool,
```

The `compiler_options()` method uses `|=` to overlay flags on the dialect preset:

```rust
fn compiler_options(&self) -> CompilerOptions {
    let mut options = CompilerOptions::from_dialect(self.dialect.to_dialect());
    // ... existing overlays ...
    options.allow_my_extension |= self.allow_my_extension;
    options
}
```

**Also add the flag to relevant dialect presets** in `CompilerOptions::from_dialect()` (in `parser/src/options.rs`). If the extension should be on for the RuSTy dialect, add it to the `Dialect::Rusty` arm.

#### 3. LSP extraction (`plc2x/src/lsp.rs`)

Add to `extract_compiler_options()` using the `|=` pattern:

```rust
options.allow_my_extension |= flag("allowMyExtension");  // camelCase for LSP
```

Add a test for the LSP extraction.

#### 4. Playground defaults (`playground/src/lib.rs`)

If the extension should be enabled by default in the playground, set it there.

#### 5. Implement the gating

Use either the token demotion pattern, validation rule pattern, or analyzer-level check (see sections above). Always test both the allowed and disallowed cases.

#### 6. Documentation

Update these files to document the new flag:
- `docs/explanation/enabling-dialects-and-features.rst` — add to the Vendor Extensions section
- `docs/reference/compiler/ironplcc.rst` — add to the Options section
- Update the flag table in this file (syntax-support-guide.md)

## plc2plc Round-Trip Testing

**Requirement**: Every new syntax feature must have a plc2plc test that proves the compiler can parse the syntax and render it back out correctly.

### How Round-Trip Tests Work

The test pattern parses an `.st` source file, renders it back to text via the `plc2plc` renderer, and compares the output against an expected file.

### File Locations

| What | Where |
|------|-------|
| Shared input `.st` files | `compiler/resources/test/` |
| Expected rendered output | `compiler/plc2plc/resources/test/` |
| Test code | `compiler/plc2plc/src/tests.rs` |
| Renderer implementation | `compiler/plc2plc/src/renderer.rs` |

### Test Pattern

From `plc2plc/src/tests.rs`:

```rust
fn parse_and_render_resource(name: &'static str) -> String {
    let source = read_shared_resource(name);
    let library = parse_program(&source, &FileId::default(), &CompilerOptions::default()).unwrap();
    write_to_string(&library).unwrap()
}

#[test]
fn write_to_string_my_feature() {
    let rendered = parse_and_render_resource("my_feature.st");
    let expected = read_resource("my_feature_rendered.st");
    assert_eq!(rendered, expected);
}
```

### Steps to Add a Round-Trip Test

1. **Create the input file**: Add `compiler/resources/test/my_feature.st` with valid IEC 61131-3 source that uses the new syntax
2. **Create the expected output file**: Add `compiler/plc2plc/resources/test/my_feature_rendered.st` with the expected rendered output
3. **Add the test**: Add a test function in `plc2plc/src/tests.rs` following the pattern above
4. **Update the renderer**: If the new syntax requires new AST nodes, update `plc2plc/src/renderer.rs` to render them

For non-standard syntax that requires a parse option:

```rust
fn parse_and_render_with_options(name: &'static str, options: CompilerOptions) -> String {
    let source = read_shared_resource(name);
    let library = parse_program(&source, &FileId::default(), &options).unwrap();
    write_to_string(&library).unwrap()
}

#[test]
fn write_to_string_my_vendor_extension() {
    let options = CompilerOptions {
        allow_my_extension: true,
        ..CompilerOptions::default()
    };
    let rendered = parse_and_render_with_options("my_extension.st", options);
    let expected = read_resource("my_extension_rendered.st");
    assert_eq!(rendered, expected);
}
```

### What Round-Trip Tests Validate

- The parser correctly understands the syntax structure
- The AST captures all relevant information
- The renderer can reproduce the syntax from the AST
- No information is lost in the parse → AST → render pipeline

## End-to-End Execution Testing

**Requirement**: Every syntax feature that produces executable code must have an end-to-end test that compiles and runs the code, then verifies the results.

### How End-to-End Tests Work

Tests use inline IEC 61131-3 source, run the full pipeline (parse → analyze → compile → VM execute), and inspect the resulting variable buffers.

### File Locations

| What | Where |
|------|-------|
| Test helpers | `compiler/codegen/tests/common/mod.rs` |
| End-to-end tests | `compiler/codegen/tests/end_to_end_*.rs` |

### Test Helpers

From `codegen/tests/common/mod.rs`:

| Helper | Purpose |
|--------|---------|
| `parse_and_run(source)` | Full pipeline, one scan cycle, returns `(Container, VmBuffers)` |
| `parse_and_run_edition3(source)` | Same but with Edition 3 features enabled |
| `parse_and_compile(source)` | Parse + compile without running (for bytecode inspection) |
| `parse_and_try_run(source)` | Returns `Result` so you can test runtime traps |
| `parse_and_run_rounds(source, closure)` | Multi-round execution for stateful tests |

### Test Pattern

From `codegen/tests/end_to_end_if.rs`:

```rust
//! End-to-end integration tests for IF/ELSIF/ELSE statements.

mod common;
use common::parse_and_run;

#[test]
fn end_to_end_when_if_true_then_executes_body() {
    let source = "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  x := 5;
  IF x > 0 THEN
    y := 1;
  END_IF;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), 5);
    assert_eq!(bufs.vars[1].as_i32(), 1);
}
```

### Steps to Add an Execution Test

1. **Create or extend a test file**: Add `compiler/codegen/tests/end_to_end_my_feature.rs` or add tests to an existing file if the feature is closely related
2. **Write inline source**: Use valid IEC 61131-3 source that exercises the new syntax
3. **Run and inspect**: Use `parse_and_run()` and check `bufs.vars[N].as_i32()` (or appropriate type method)
4. **Test both success and edge cases**: Include tests for the happy path and boundary conditions

### Variable Buffer Inspection

Variables appear in `bufs.vars` in declaration order (0-indexed). Use the appropriate type accessor:

- `bufs.vars[N].as_i32()` — for DINT, INT, SINT, etc.
- `bufs.vars[N].as_f32()` — for REAL
- `bufs.vars[N].as_f64()` — for LREAL
- `bufs.vars[N].as_bool()` — for BOOL

### Testing Non-Standard Syntax Execution

If the syntax is behind an `--allow-x` flag, you may need to add a helper in `codegen/tests/common/mod.rs` that enables the flag:

```rust
pub fn parse_with_extension(source: &str) -> (Library, SemanticContext) {
    let options = CompilerOptions {
        allow_my_extension: true,
        ..CompilerOptions::default()
    };
    let library = parse_program(source, &FileId::default(), &options).unwrap();
    let (analyzed, ctx) = ironplc_analyzer::stages::resolve_types(&[&library]).unwrap();
    (analyzed, ctx)
}
```

### What Execution Tests Validate

- The full compiler pipeline works end-to-end for the syntax
- Generated bytecode is correct
- The VM executes the bytecode and produces expected results
- Runtime behavior matches IEC 61131-3 semantics

## Pipeline Integration Points

The token processing pipeline in `parser/src/lib.rs` (`tokenize_program` function) runs these steps in order:

1. **`preprocess()`** — normalize source text
2. **`tokenize()`** — lexer produces raw tokens (via `logos`)
3. **`insert_keyword_statement_terminators()`** — token transform (flag-gated)
4. **`xform_demote_edition3_keywords::apply()`** — token demotion (edition-gated)
5. **`check_tokens()`** — runs validation rules (flag-gated)
6. **`parse_library()`** — PEG parser consumes tokens, produces AST

New token transforms go between steps 2 and 5. New validation rules are registered in `check_tokens()`:

```rust
fn check_tokens(tokens: &[Token], options: &CompilerOptions) -> Result<(), Vec<Diagnostic>> {
    let rules: Vec<fn(&[Token], &CompilerOptions) -> Result<(), Vec<Diagnostic>>> =
        vec![rule_token_no_c_style_comment::apply];  // Add your rule here

    let mut errors = vec![];
    for rule in rules {
        match rule(tokens, options) {
            Ok(_) => {}
            Err(mut diagnostics) => errors.append(&mut diagnostics),
        };
    }
    // ...
}
```

New demotion transforms must be called in `tokenize_program()` **before** `check_tokens()` and **before** `parse_library()`. The order matters: demotion must happen before parsing so the parser sees identifiers, not keywords.

## Common Mistakes

- **Forgetting dialect presets**: Every new `--allow-x` flag must be added to the relevant dialect presets in `CompilerOptions::from_dialect()`. Without this, the RuSTy dialect (or future dialects) silently ignores the new feature.
- **Missing LSP wiring**: The flag works on the CLI but not in VS Code because `extract_compiler_options()` in `plc2x/src/lsp.rs` was not updated. Always add LSP extraction for new flags.
- **No round-trip test**: The feature parses but the renderer in `plc2plc` cannot write it back. Always add the round-trip test.
- **No execution test**: The feature parses and analyzes but was never proven to execute correctly. Always add at least one end-to-end test.
- **Creating a flag for standard syntax**: Only vendor extensions get `--allow-x` flags. Standard IEC 61131-3 syntax is always on (or gated by `--dialect`).
- **Stateful lexer changes**: The lexer (`logos`) is stateless. Use token transforms for context-dependent behavior, not lexer rules.
- **Not registering transforms**: Adding a new `xform_*.rs` module but forgetting to call it from `tokenize_program()` in `parser/src/lib.rs`, or adding a new rule module but forgetting to register it in `check_tokens()`.

## Step-by-Step Walkthrough

This walkthrough shows the typical sequence for adding a new syntax feature.

### Example: Adding Support for a Hypothetical Vendor Extension

Suppose a vendor allows `REPEAT ... UNTIL ... END_REPEAT` with an optional `LIMIT` clause (non-standard).

#### Step 1: Check Existing Flags

Review `parser/src/options.rs` — does an existing flag cover this? If not, proceed with a new flag.

#### Step 2: Add the Flag

1. Add `allow_repeat_limit` to `CompilerOptions` vendor fields in the `define_compiler_options!` macro
2. Add `--allow-repeat-limit` to CLI `FileArgs`
3. Add `|= self.allow_repeat_limit` in `compiler_options()`
4. Add to relevant dialect presets in `CompilerOptions::from_dialect()`
5. Add LSP extraction for `"allowRepeatLimit"`

#### Step 3: Add Tokens (if needed)

If the syntax uses a new keyword like `LIMIT`, add it to `parser/src/token.rs`:

```rust
#[token("LIMIT", ignore(case))]
Limit,
```

Then add a demotion transform so that `LIMIT` is treated as an identifier when the flag is off:

```rust
// In a new xform_demote_repeat_limit.rs
pub fn apply(tokens: &mut [Token], options: &CompilerOptions) {
    if options.allow_repeat_limit {
        return;
    }
    for tok in tokens.iter_mut() {
        if tok.token_type == TokenType::Limit {
            tok.token_type = TokenType::Identifier;
        }
    }
}
```

#### Step 4: Add Parser Rules

Update `parser/src/parser.rs` to handle the new syntax in the grammar.

#### Step 5: Add AST Nodes

Update the `dsl` crate to represent the new syntax in the AST.

#### Step 6: Add Analyzer Validation

Add semantic checks in `analyzer/` if needed.

#### Step 7: Add Codegen

Add bytecode emission for the new syntax in `codegen/`.

#### Step 8: Update plc2plc Renderer

Update `plc2plc/src/renderer.rs` to render the new AST nodes.

#### Step 9: Add Round-Trip Test

1. Create `compiler/resources/test/repeat_limit.st`
2. Create `compiler/plc2plc/resources/test/repeat_limit_rendered.st`
3. Add test in `plc2plc/src/tests.rs`

#### Step 10: Add Execution Test

Create `compiler/codegen/tests/end_to_end_repeat_limit.rs` with tests that compile and run programs using the new syntax, verifying correct variable values.

#### Step 11: Run CI

```bash
cd compiler && just
```

All checks must pass before creating a PR.
