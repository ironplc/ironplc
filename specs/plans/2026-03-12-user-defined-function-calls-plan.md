# User-Defined Function Calls Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Enable compilation and execution of user-defined IEC 61131-3 function calls, including type-checking analysis, codegen, and VM support.

**Architecture:** The analyzer validates argument and return types via a new rule. Codegen compiles each user-defined function as a separate bytecode function in the container, using `CALL`/`RET` opcodes. The VM executes `CALL` by scoping into the function's variable region and recursively executing its bytecode.

**Tech Stack:** Rust, IronPLC compiler pipeline (analyzer → codegen → container → VM)

**Design doc:** `specs/design/2026-03-12-user-defined-function-calls-design.md`

**Prerequisites:** Create a feature branch before starting. Never commit to `main` directly.

---

## File Map

### Container crate (`compiler/container/src/`)
- **Modify:** `opcode.rs` — Add `CALL` (0xB3) and `RET` (0xB4) opcode constants
- **Modify:** `code_section.rs` — Add `num_params: u16` to `FuncEntry`, update serialization (FUNC_ENTRY_SIZE 14→16)
- **Modify:** `builder.rs` — Update `add_function()` signature to include `num_params`

### Analyzer crate (`compiler/analyzer/src/`)
- **Create:** `rule_function_call_type_check.rs` — New type-checking rule for function arguments and return types
- **Modify:** `stages.rs` — Register the new rule in the `semantic()` function
- **Modify:** `lib.rs` — Add `mod rule_function_call_type_check`

### Problem codes (`compiler/problems/`)
- **Modify:** `resources/problem-codes.csv` — Add `P4023` (FunctionCallArgTypeMismatch) and `P4024` (FunctionCallReturnTypeMismatch)
- **Create:** `docs/reference/compiler/problems/P4023.rst` — Problem code documentation
- **Create:** `docs/reference/compiler/problems/P4024.rst` — Problem code documentation

### Codegen crate (`compiler/codegen/src/`)
- **Modify:** `compile.rs` — Accept `FunctionEnvironment` + `TypeEnvironment`, compile user-defined function bodies, emit `CALL`/`RET` at call sites
- **Modify:** `emit.rs` — Add `emit_call()` and `emit_ret()` methods
- **Modify:** `lib.rs` — Update `compile()` public signature

### VM crate (`compiler/vm/src/`)
- **Modify:** `vm.rs` — Implement `CALL` and `RET` opcode handlers in `execute()`

### Callers of `compile()`
- **Modify:** `compiler/codegen/tests/common/mod.rs` — Update `parse()` and `parse_and_try_run()` to pass environments
- **Modify:** `compiler/plc2x/src/cli.rs` — Pass `FunctionEnvironment` and `TypeEnvironment` to `compile()`
- **Modify:** `compiler/playground/src/lib.rs` — Pass environments to `compile()`
- **Modify:** `compiler/benchmarks/benches/st_benchmark.rs` — Pass environments to `compile()`

### Tests
- **Create:** `compiler/codegen/tests/end_to_end_user_function.rs` — End-to-end codegen+VM tests for user-defined functions
- Tests for the analysis rule are inline in `rule_function_call_type_check.rs`

---

## Task 1: Add CALL and RET opcode constants to container

**Files:**
- Modify: `compiler/container/src/opcode.rs`

- [ ] **Step 1: Add opcode constants**

In `compiler/container/src/opcode.rs`, add after the `RET_VOID` constant (line ~135):

```rust
/// Call function by index. Pops arguments, executes function body,
/// pushes return value.
/// Operand: u16 function_id (little-endian).
pub const CALL: u8 = 0xB3;

/// Return from function with a value on the stack.
pub const RET: u8 = 0xB4;
```

- [ ] **Step 2: Verify it compiles**

Run: `cd /workspaces/ironplc/compiler && cargo build -p ironplc-container`
Expected: Compiles successfully.

- [ ] **Step 3: Commit**

```bash
git add compiler/container/src/opcode.rs
git commit -m "feat: add CALL and RET opcode constants (0xB3, 0xB4)"
```

---

## Task 2: Add num_params to FuncEntry

**Files:**
- Modify: `compiler/container/src/code_section.rs`
- Modify: `compiler/container/src/builder.rs`

- [ ] **Step 1: Update FuncEntry struct**

In `compiler/container/src/code_section.rs`, add `num_params` field to `FuncEntry`:

```rust
pub struct FuncEntry {
    pub function_id: u16,
    pub bytecode_offset: u32,
    pub bytecode_length: u32,
    pub max_stack_depth: u16,
    pub num_locals: u16,
    pub num_params: u16,
}
```

Update `FUNC_ENTRY_SIZE` from 14 to 16.

- [ ] **Step 2: Update serialization in `write_to`**

After the `num_locals` write, add:
```rust
w.write_all(&func.num_params.to_le_bytes())?;
```

- [ ] **Step 3: Update deserialization in `read_from`**

Update the entry buffer size and parsing to read `num_params`:
```rust
let mut entry_buf = [0u8; FUNC_ENTRY_SIZE];
// ... existing reads ...
num_params: u16::from_le_bytes([entry_buf[14], entry_buf[15]]),
```

- [ ] **Step 4: Update all FuncEntry construction sites**

Add `num_params: 0` to all existing `FuncEntry` literals in test code within `code_section.rs`.

- [ ] **Step 5: Update `add_function` in builder.rs**

In `compiler/container/src/builder.rs`, update the `add_function` method signature:

```rust
pub fn add_function(
    mut self,
    function_id: u16,
    bytecode: &[u8],
    max_stack_depth: u16,
    num_locals: u16,
    num_params: u16,
) -> Self {
```

Add `num_params` to the `FuncEntry` construction inside `add_function`.

- [ ] **Step 6: Update all callers of add_function**

All existing callers pass `num_params: 0` (init/scan functions have no params). Search for `.add_function(` across the codebase and add the trailing `, 0` argument. Key files:
- `compiler/container/src/builder.rs` (tests)
- `compiler/container/src/container.rs` (tests)
- `compiler/container/src/container_ref.rs` (tests)
- `compiler/codegen/src/compile.rs`
- `compiler/vm/src/vm.rs` (test helpers)
- `compiler/vm/tests/common/mod.rs` (test helpers)

- [ ] **Step 7: Run tests**

Run: `cd /workspaces/ironplc/compiler && cargo test -p ironplc-container`
Expected: All tests pass.

- [ ] **Step 8: Commit**

```bash
git add compiler/container/src/code_section.rs compiler/container/src/builder.rs \
  compiler/container/src/container.rs compiler/container/src/container_ref.rs \
  compiler/codegen/src/compile.rs compiler/vm/src/vm.rs compiler/vm/tests/
git commit -m "feat: add num_params field to FuncEntry for CALL opcode support"
```

---

## Task 3: Add emit_call and emit_ret to Emitter

**Files:**
- Modify: `compiler/codegen/src/emit.rs`

- [ ] **Step 1: Write failing tests**

Add tests at the bottom of the `#[cfg(test)]` module in `emit.rs`:

```rust
#[test]
fn emitter_when_call_then_correct_bytecode() {
    let mut em = Emitter::new();
    em.emit_load_const_i32(0); // arg 1
    em.emit_load_const_i32(1); // arg 2
    em.emit_call(2, 2);        // CALL function 2, 2 params

    // LOAD_CONST pool:0, LOAD_CONST pool:1, CALL func:2
    assert_eq!(
        em.bytecode(),
        &[0x01, 0x00, 0x00, 0x01, 0x01, 0x00, 0xB3, 0x02, 0x00]
    );
}

#[test]
fn emitter_when_call_then_tracks_stack_depth() {
    let mut em = Emitter::new();
    em.emit_load_const_i32(0); // stack: 1
    em.emit_load_const_i32(1); // stack: 2
    em.emit_call(2, 2);        // pop 2 args, push 1 result = stack: 1
    em.emit_store_var_i32(0);  // stack: 0

    assert_eq!(em.max_stack_depth(), 2);
}

#[test]
fn emitter_when_ret_then_correct_bytecode() {
    let mut em = Emitter::new();
    em.emit_load_var_i32(0);
    em.emit_ret();

    assert_eq!(em.bytecode(), &[0x10, 0x00, 0x00, 0xB4]);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd /workspaces/ironplc/compiler && cargo test -p ironplc-codegen --lib -- emitter_when_call`
Expected: FAIL — `emit_call` method does not exist.

- [ ] **Step 3: Implement emit_call and emit_ret**

Add to the `impl Emitter` block in `emit.rs`:

```rust
/// Emits CALL with a function ID.
/// Pops `num_params` arguments and pushes one return value.
pub fn emit_call(&mut self, function_id: u16, num_params: u16) {
    self.bytecode.push(opcode::CALL);
    self.bytecode.extend_from_slice(&function_id.to_le_bytes());
    // Net effect: pop num_params, push 1 result
    if num_params > 0 {
        self.pop_stack(num_params);
    }
    self.push_stack(1);
}

/// Emits RET (return with value on stack).
pub fn emit_ret(&mut self) {
    self.bytecode.push(opcode::RET);
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd /workspaces/ironplc/compiler && cargo test -p ironplc-codegen --lib -- emitter_when_call && cargo test -p ironplc-codegen --lib -- emitter_when_ret`
Expected: All 3 tests PASS.

- [ ] **Step 5: Commit**

```bash
git add compiler/codegen/src/emit.rs
git commit -m "feat: add emit_call and emit_ret to bytecode emitter"
```

---

## Task 4: Add problem codes for type checking

**Files:**
- Modify: `compiler/problems/resources/problem-codes.csv`

- [ ] **Step 1: Add problem codes**

Add these lines to `compiler/problems/resources/problem-codes.csv` after the `P4022` line:

```
P4023,FunctionCallArgTypeMismatch,Function call argument type does not match parameter type
P4024,FunctionCallReturnTypeMismatch,Function return type does not match assignment destination type
```

- [ ] **Step 2: Create P4023.rst documentation**

Create `docs/reference/compiler/problems/P4023.rst`:

```rst
=====
P4023
=====

.. problem-summary:: P4023

This error occurs when a function call passes an argument whose type does not match the
declared parameter type. IronPLC requires exact type matching for user-defined function arguments.

Example
-------

The following code will generate error P4023:

.. code-block::

   FUNCTION DOUBLE_REAL : REAL
   VAR_INPUT
       A : REAL;
   END_VAR
       DOUBLE_REAL := A + A;
   END_FUNCTION

   PROGRAM main
   VAR
       result : REAL;
       x : INT;
   END_VAR
       result := DOUBLE_REAL(x);  (* Error: INT argument for REAL parameter *)
   END_PROGRAM

The variable ``x`` is ``INT``, but parameter ``A`` expects ``REAL``.

To fix this error, use an explicit type conversion:

.. code-block::

   result := DOUBLE_REAL(INT_TO_REAL(x));
```

- [ ] **Step 3: Create P4024.rst documentation**

Create `docs/reference/compiler/problems/P4024.rst`:

```rst
=====
P4024
=====

.. problem-summary:: P4024

This error occurs when a function's return type does not match the type of the variable
being assigned to. IronPLC requires exact type matching for function return values.

Example
-------

The following code will generate error P4024:

.. code-block::

   FUNCTION GET_VALUE : REAL
   VAR_INPUT
       A : REAL;
   END_VAR
       GET_VALUE := A;
   END_FUNCTION

   PROGRAM main
   VAR
       result : INT;
       x : REAL;
   END_VAR
       result := GET_VALUE(x);  (* Error: REAL return assigned to INT variable *)
   END_PROGRAM

The function ``GET_VALUE`` returns ``REAL``, but ``result`` is ``INT``.

To fix this error, use an explicit type conversion:

.. code-block::

   result := REAL_TO_INT(GET_VALUE(x));
```

- [ ] **Step 4: Verify it compiles**

Run: `cd /workspaces/ironplc/compiler && cargo build -p ironplc-problems`
Expected: Compiles successfully. The build script generates `Problem::FunctionCallArgTypeMismatch` and `Problem::FunctionCallReturnTypeMismatch`.

- [ ] **Step 5: Commit**

```bash
git add compiler/problems/resources/problem-codes.csv \
  docs/reference/compiler/problems/P4023.rst \
  docs/reference/compiler/problems/P4024.rst
git commit -m "feat: add P4023 and P4024 problem codes for function call type checking"
```

---

## Task 5: Implement rule_function_call_type_check

**Files:**
- Create: `compiler/analyzer/src/rule_function_call_type_check.rs`
- Modify: `compiler/analyzer/src/stages.rs`
- Modify: `compiler/analyzer/src/lib.rs`

- [ ] **Step 1: Write the rule with inline tests**

Create `compiler/analyzer/src/rule_function_call_type_check.rs`. The rule visits every `Function` node (function call in the AST), looks up the signature from `FunctionEnvironment`, and compares each positional argument's `resolved_type` against the parameter's declared type. Skips stdlib functions (which use `ANY_*` types).

```rust
//! Semantic rule that validates function call argument types match parameter types
//! and return types match assignment destinations.
//!
//! This rule only checks user-defined functions. Standard library functions are
//! skipped because they use ANY_* generic types which require different handling.
//!
//! ## Passes
//!
//! ```ignore
//! FUNCTION ADD_INTS : INT
//! VAR_INPUT
//!     A : INT;
//!     B : INT;
//! END_VAR
//!     ADD_INTS := A + B;
//! END_FUNCTION
//!
//! PROGRAM main
//! VAR
//!     result : INT;
//! END_VAR
//!     result := ADD_INTS(1, 2);
//! END_PROGRAM
//! ```
//!
//! ## Fails (Argument Type Mismatch)
//!
//! ```ignore
//! FUNCTION ADD_REALS : REAL
//! VAR_INPUT
//!     A : REAL;
//! END_VAR
//!     ADD_REALS := A;
//! END_FUNCTION
//!
//! PROGRAM main
//! VAR
//!     result : REAL;
//!     x : INT;
//! END_VAR
//!     result := ADD_REALS(x);
//! END_PROGRAM
//! ```

use std::collections::HashMap;

use ironplc_dsl::{
    common::*,
    core::Located,
    diagnostic::{Diagnostic, Label},
    textual::*,
    visitor::Visitor,
};
use ironplc_problems::Problem;

use crate::{result::SemanticResult, semantic_context::SemanticContext};

pub fn apply(lib: &Library, context: &SemanticContext) -> SemanticResult {
    let mut visitor = RuleFunctionCallTypeCheck {
        context,
        diagnostics: vec![],
        var_types: HashMap::new(),
    };

    visitor.walk(lib).map_err(|e| vec![e])?;

    if visitor.diagnostics.is_empty() {
        Ok(())
    } else {
        Err(visitor.diagnostics)
    }
}

struct RuleFunctionCallTypeCheck<'a> {
    context: &'a SemanticContext,
    diagnostics: Vec<Diagnostic>,
    /// Maps variable name (lowercase) to declared type for the current scope.
    /// Populated as the visitor enters VAR blocks.
    var_types: HashMap<String, TypeName>,
}

impl Visitor<Diagnostic> for RuleFunctionCallTypeCheck<'_> {
    type Value = ();

    fn visit_var_decl(&mut self, node: &VarDecl) -> Result<Self::Value, Diagnostic> {
        // Track variable types for return type checking in assignments.
        if let VariableIdentifier::Symbol(ref id) = node.identifier {
            if let TypeReference::Named(ref type_name) = node.type_name() {
                self.var_types.insert(id.original().to_lowercase(), type_name.clone());
            }
        }
        node.recurse_visit(self)
    }

    fn visit_assignment(&mut self, node: &Assignment) -> Result<Self::Value, Diagnostic> {
        // Check return type: if RHS is a user-defined function call, verify its
        // return type matches the target variable's declared type.
        if let ExprKind::Function(ref func_call) = node.value.kind {
            if let Some(signature) = self.context.functions.get(&func_call.name) {
                if !signature.is_stdlib() {
                    if let Variable::Symbolic(SymbolicVariableKind::Named(ref nv)) = node.target {
                        let target_key = nv.name.original().to_lowercase();
                        if let Some(target_type) = self.var_types.get(&target_key) {
                            if let Some(ref return_type) = node.value.resolved_type {
                                let target_lower = target_type.to_string().to_lowercase();
                                let return_lower = return_type.to_string().to_lowercase();
                                if target_lower != return_lower {
                                    self.diagnostics.push(
                                        Diagnostic::problem(
                                            Problem::FunctionCallReturnTypeMismatch,
                                            Label::span(node.value.span(), "Function call"),
                                        )
                                        .with_context("function", &func_call.name.original().to_string())
                                        .with_context("return_type", &return_type.to_string())
                                        .with_context("target_type", &target_type.to_string()),
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
        node.recurse_visit(self)
    }

    fn visit_function(&mut self, node: &Function) -> Result<Self::Value, Diagnostic> {
        let func_sig = self.context.functions.get(&node.name);

        if let Some(signature) = func_sig {
            // Skip stdlib functions — they use ANY_* types
            if signature.is_stdlib() {
                return node.recurse_visit(self);
            }

            // Check each positional argument type against the parameter type
            let input_params: Vec<_> = signature
                .parameters
                .iter()
                .filter(|p| p.is_input)
                .collect();

            let positional_args: Vec<_> = node
                .param_assignment
                .iter()
                .filter_map(|p| match p {
                    ParamAssignmentKind::PositionalInput(pos) => Some(&pos.expr),
                    _ => None,
                })
                .collect();

            for (i, arg_expr) in positional_args.iter().enumerate() {
                if i >= input_params.len() {
                    break;
                }
                let param = &input_params[i];

                if let Some(ref arg_type) = arg_expr.resolved_type {
                    let param_type_lower = param.param_type.to_string().to_lowercase();
                    let arg_type_lower = arg_type.to_string().to_lowercase();

                    if param_type_lower != arg_type_lower {
                        self.diagnostics.push(
                            Diagnostic::problem(
                                Problem::FunctionCallArgTypeMismatch,
                                Label::span(arg_expr.span(), "Argument"),
                            )
                            .with_context("function", &node.name.original().to_string())
                            .with_context("parameter", &param.name.original().to_string())
                            .with_context("expected", &param.param_type.to_string())
                            .with_context("actual", &arg_type.to_string()),
                        );
                    }
                }
            }
        }

        // Continue visiting children (arguments may contain nested function calls)
        node.recurse_visit(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::parse_and_resolve_types_with_context;

    #[test]
    fn apply_when_matching_types_then_ok() {
        let program = "
FUNCTION ADD_INTS : INT
VAR_INPUT
    A : INT;
    B : INT;
END_VAR
    ADD_INTS := A + B;
END_FUNCTION

PROGRAM main
VAR
    result : INT;
END_VAR
    result := ADD_INTS(1, 2);
END_PROGRAM";

        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context);
        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_arg_type_mismatch_then_error() {
        let program = "
FUNCTION DOUBLE_REAL : REAL
VAR_INPUT
    A : REAL;
END_VAR
    DOUBLE_REAL := A;
END_FUNCTION

PROGRAM main
VAR
    result : REAL;
    x : INT;
END_VAR
    result := DOUBLE_REAL(x);
END_PROGRAM";

        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context);
        assert!(result.is_err());
        let diagnostics = result.unwrap_err();
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].code,
            Problem::FunctionCallArgTypeMismatch.code()
        );
    }

    #[test]
    fn apply_when_stdlib_function_then_skipped() {
        let program = "
PROGRAM main
VAR
    result : REAL;
    x : INT;
END_VAR
    result := INT_TO_REAL(x);
END_PROGRAM";

        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context);
        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_multiple_args_one_mismatch_then_one_error() {
        let program = "
FUNCTION MY_FUNC : INT
VAR_INPUT
    A : INT;
    B : DINT;
END_VAR
    MY_FUNC := A;
END_FUNCTION

PROGRAM main
VAR
    result : INT;
    x : INT;
END_VAR
    result := MY_FUNC(x, x);
END_PROGRAM";

        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context);
        assert!(result.is_err());
        let diagnostics = result.unwrap_err();
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].code,
            Problem::FunctionCallArgTypeMismatch.code()
        );
    }

    #[test]
    fn apply_when_return_type_mismatch_then_error() {
        let program = "
FUNCTION GET_VALUE : REAL
VAR_INPUT
    A : REAL;
END_VAR
    GET_VALUE := A;
END_FUNCTION

PROGRAM main
VAR
    result : INT;
    x : REAL;
END_VAR
    result := GET_VALUE(x);
END_PROGRAM";

        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context);
        assert!(result.is_err());
        let diagnostics = result.unwrap_err();
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].code,
            Problem::FunctionCallReturnTypeMismatch.code()
        );
    }

    #[test]
    fn apply_when_nested_function_call_types_match_then_ok() {
        let program = "
FUNCTION DOUBLE : INT
VAR_INPUT
    A : INT;
END_VAR
    DOUBLE := A + A;
END_FUNCTION

PROGRAM main
VAR
    result : INT;
END_VAR
    result := DOUBLE(DOUBLE(5));
END_PROGRAM";

        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context);
        assert!(result.is_ok());
    }
}
```

- [ ] **Step 2: Register the rule in stages.rs**

In `compiler/analyzer/src/stages.rs`, add to the imports:
```rust
rule_function_call_type_check,
```

Add to the `functions` vector in the `semantic()` function:
```rust
rule_function_call_type_check::apply,
```

- [ ] **Step 3: Add module declaration in lib.rs**

In `compiler/analyzer/src/lib.rs`, add:
```rust
mod rule_function_call_type_check;
```

- [ ] **Step 4: Run tests**

Run: `cd /workspaces/ironplc/compiler && cargo test -p ironplc-analyzer -- rule_function_call_type_check`
Expected: All tests pass.

- [ ] **Step 5: Commit**

```bash
git add compiler/analyzer/src/rule_function_call_type_check.rs \
  compiler/analyzer/src/stages.rs compiler/analyzer/src/lib.rs
git commit -m "feat: add rule_function_call_type_check for argument and return type validation"
```

---

## Task 6: Update compile() signature to accept environments

This task threads `FunctionEnvironment` and `TypeEnvironment` through the `compile()` public API and all callers. No behavior change yet — just plumbing.

**Files:**
- Modify: `compiler/codegen/src/compile.rs`
- Modify: `compiler/codegen/src/lib.rs`
- Modify: `compiler/codegen/tests/common/mod.rs`
- Modify: `compiler/plc2x/src/cli.rs`
- Modify: `compiler/playground/src/lib.rs`
- Modify: `compiler/benchmarks/benches/st_benchmark.rs`

- [ ] **Step 1: Update compile() signature**

In `compiler/codegen/src/compile.rs`, change:

```rust
pub fn compile(library: &Library) -> Result<Container, Diagnostic> {
```
to:
```rust
pub fn compile(
    library: &Library,
    functions: &FunctionEnvironment,
    types: &TypeEnvironment,
) -> Result<Container, Diagnostic> {
```

Add the necessary imports at the top:
```rust
use ironplc_analyzer::function_environment::FunctionEnvironment;
use ironplc_analyzer::type_environment::TypeEnvironment;
```

For now, the function body ignores the new parameters — just pass them through. Store references in `CompileContext` for later use.

- [ ] **Step 2: Update lib.rs re-export**

In `compiler/codegen/src/lib.rs`, update the import if needed. The public API now requires `FunctionEnvironment` and `TypeEnvironment`.

- [ ] **Step 3: Update codegen tests common helper**

In `compiler/codegen/tests/common/mod.rs`, update `parse()` to return the context and `parse_and_try_run()` to pass environments:

```rust
use ironplc_codegen::compile;
use ironplc_container::Container;
use ironplc_dsl::common::Library;
use ironplc_dsl::core::FileId;
use ironplc_parser::options::ParseOptions;
use ironplc_parser::parse_program;
use ironplc_vm::test_support::load_and_start;
use ironplc_vm::FaultContext;
pub use ironplc_vm::VmBuffers;
use ironplc_analyzer::semantic_context::SemanticContext;

pub fn parse(source: &str) -> (Library, SemanticContext) {
    let library = parse_program(source, &FileId::default(), &ParseOptions::default()).unwrap();
    let (analyzed, ctx) = ironplc_analyzer::stages::resolve_types(&[&library]).unwrap();
    (analyzed, ctx)
}

pub fn parse_and_run(source: &str) -> (Container, VmBuffers) {
    let (container, bufs) = parse_and_try_run(source).expect("VM execution trapped unexpectedly");
    (container, bufs)
}

pub fn parse_and_try_run(source: &str) -> Result<(Container, VmBuffers), FaultContext> {
    let (library, context) = parse(source);
    let container = compile(&library, context.functions(), context.types()).unwrap();
    let mut bufs = VmBuffers::from_container(&container);
    {
        let mut vm = load_and_start(&container, &mut bufs)?;
        vm.run_round(0)?;
    }
    Ok((container, bufs))
}
```

- [ ] **Step 4: Update all codegen test files**

Every test file in `compiler/codegen/tests/` that calls `parse(source)` now gets a tuple. Update each one. The pattern changes from:
```rust
let library = parse(source);
let container = compile(&library).unwrap();
```
to:
```rust
let (library, context) = parse(source);
let container = compile(&library, context.functions(), context.types()).unwrap();
```

Search for all files with `use common::parse` and `compile(&library)` in `compiler/codegen/tests/`.

- [ ] **Step 5: Update plc2x/src/cli.rs**

Change the compile call (around line 115-122):
```rust
let (analyzed, context) =
    ironplc_analyzer::stages::resolve_types(&[&combined]).map_err(|errs| {
        handle_diagnostics(&errs, Some(&project), suppress_output);
        String::from("Error during type resolution")
    })?;

let container = ironplc_codegen::compile(&analyzed, context.functions(), context.types()).map_err(|err| {
    handle_diagnostics(&[err], Some(&project), suppress_output);
    String::from("Error during code generation")
})?;
```

- [ ] **Step 6: Update playground/src/lib.rs**

Update the `codegen_compile` call (around line 299) to pass environments from the analysis context. The playground already calls `analyze()` and has the context available.

- [ ] **Step 7: Update benchmarks**

In `compiler/benchmarks/benches/st_benchmark.rs`, update the `compile_st` function to pass environments:
```rust
fn compile_st(source: &str) -> Container {
    let library = parse_program(source, &FileId::default(), &ParseOptions::default()).unwrap();
    let (analyzed, context) = ironplc_analyzer::stages::analyze(&[&library]).unwrap();
    assert!(
        !context.has_diagnostics(),
        "Benchmark source has semantic diagnostics"
    );
    compile(&analyzed, context.functions(), context.types()).unwrap()
}
```

- [ ] **Step 8: Run full test suite**

Run: `cd /workspaces/ironplc/compiler && cargo test`
Expected: All tests pass. No behavior change, just plumbing.

- [ ] **Step 9: Commit**

```bash
git add compiler/codegen/src/compile.rs compiler/codegen/src/lib.rs \
  compiler/codegen/tests/ compiler/plc2x/src/cli.rs \
  compiler/playground/src/lib.rs compiler/benchmarks/benches/st_benchmark.rs
git commit -m "refactor: thread FunctionEnvironment and TypeEnvironment through compile()"
```

---

## Task 7: Implement CALL and RET in the VM

**Files:**
- Modify: `compiler/vm/src/vm.rs`

- [ ] **Step 1: Write a failing VM test**

Add a test in the `#[cfg(test)]` module of `vm.rs` that manually constructs a container with a user-defined function and calls it:

```rust
#[test]
fn execute_when_call_user_function_then_returns_value() {
    // Function 0 (init): RET_VOID
    // Function 1 (scan): LOAD_CONST 3, LOAD_CONST 7, CALL 2, STORE_VAR 0, RET_VOID
    // Function 2 (add):  LOAD_VAR 0, LOAD_VAR 1, ADD_I32, STORE_VAR 2, LOAD_VAR 2, RET
    //   num_params=2, num_locals=3
    #[rustfmt::skip]
    let scan_bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (3)
        0x01, 0x01, 0x00,  // LOAD_CONST_I32 pool[1] (7)
        0xB3, 0x02, 0x00,  // CALL function 2
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0] (result)
        0xB5,              // RET_VOID
    ];
    #[rustfmt::skip]
    let func_bytecode: Vec<u8> = vec![
        0x10, 0x00, 0x00,  // LOAD_VAR_I32 var[0] (A)
        0x10, 0x01, 0x00,  // LOAD_VAR_I32 var[1] (B)
        0x30,              // ADD_I32
        0x18, 0x02, 0x00,  // STORE_VAR_I32 var[2] (return)
        0x10, 0x02, 0x00,  // LOAD_VAR_I32 var[2]
        0xB4,              // RET
    ];

    let c = ContainerBuilder::new()
        .num_variables(4)  // 1 program var + 3 function vars
        .add_i32_constant(3)
        .add_i32_constant(7)
        .add_function(0, &[0xB5], 0, 1, 0)         // init
        .add_function(1, &scan_bytecode, 2, 1, 0)   // scan
        .add_function(2, &func_bytecode, 2, 3, 2)   // add (num_params=2)
        .init_function_id(0)
        .entry_function_id(1)
        .build();
    let mut b = VmBuffers::from_container(&c);
    let mut vm = Vm::new()
        .load(
            &c,
            &mut b.stack,
            &mut b.vars,
            &mut b.data_region,
            &mut b.temp_buf,
            &mut b.tasks,
            &mut b.programs,
            &mut b.ready,
        )
        .start()
        .unwrap();
    vm.run_round(0).unwrap();

    // result should be 3 + 7 = 10
    assert_eq!(vm.read_variable(0).unwrap(), 10);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd /workspaces/ironplc/compiler && cargo test -p ironplc-vm -- execute_when_call_user_function`
Expected: FAIL — unknown opcode 0xB3.

- [ ] **Step 3: Implement CALL and RET handlers**

In `compiler/vm/src/vm.rs`, add to the `execute()` match block, after the `BUILTIN` handler:

```rust
opcode::CALL => {
    let func_id = read_u16_le(bytecode, &mut pc);
    let func = container
        .code
        .get_function(func_id)
        .ok_or(Trap::InvalidFunctionId(func_id))?;
    let func_bytecode = container
        .code
        .get_function_bytecode(func_id)
        .ok_or(Trap::InvalidFunctionId(func_id))?;

    // Allocate variable scope for the called function.
    // Function variables start after the caller's variables.
    let func_var_offset = scope.instance_offset + scope.instance_count;
    let func_scope = VariableScope {
        shared_globals_size: 0,
        instance_offset: func_var_offset,
        instance_count: func.num_locals,
    };

    // Pop arguments from stack into function's parameter slots (reverse order).
    for i in (0..func.num_params).rev() {
        let val = stack.pop()?;
        variables.store(func_var_offset + i, val)?;
    }

    // Recursively execute the function body.
    execute(
        func_bytecode,
        container,
        stack,
        variables,
        data_region,
        temp_buf,
        max_temp_buf_bytes,
        &func_scope,
        current_time_us,
    )?;
}
opcode::RET => {
    // Return value is already on the stack; just return from execute().
    return Ok(());
}
```

You will also need to add `InvalidFunctionId(u16)` to the `Trap` enum in `compiler/vm/src/error.rs`, and add a `get_function` method to `CodeSection` that returns `Option<&FuncEntry>`.

- [ ] **Step 4: Add get_function to CodeSection**

In `compiler/container/src/code_section.rs`:
```rust
/// Returns the FuncEntry for the given function ID, if it exists.
pub fn get_function(&self, function_id: u16) -> Option<&FuncEntry> {
    self.functions.get(function_id as usize)
}
```

- [ ] **Step 5: Add InvalidFunctionId trap**

In `compiler/vm/src/error.rs`, add to the `Trap` enum:
```rust
InvalidFunctionId(u16),
```

- [ ] **Step 6: Run test to verify it passes**

Run: `cd /workspaces/ironplc/compiler && cargo test -p ironplc-vm -- execute_when_call_user_function`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add compiler/vm/src/vm.rs compiler/vm/src/error.rs \
  compiler/container/src/code_section.rs
git commit -m "feat: implement CALL and RET opcodes in VM"
```

---

## Task 8: Compile user-defined function bodies and call sites

This is the core codegen task. The compiler needs to:
1. Find all user-defined functions via `FunctionEnvironment`
2. Compile each function body into its own bytecode function
3. At call sites, emit arguments + `CALL` instead of falling through to `compile_generic_builtin`

**Files:**
- Modify: `compiler/codegen/src/compile.rs`

- [ ] **Step 1: Add function ID tracking to CompileContext**

Add fields to `CompileContext`:
```rust
/// Maps user-defined function name (lowercase) to assigned function ID.
user_functions: HashMap<String, u16>,
/// The next available function ID (starts at 2: 0=init, 1=scan).
next_function_id: u16,
```

Initialize in `CompileContext::new()`:
```rust
user_functions: HashMap::new(),
next_function_id: 2,
```

- [ ] **Step 2: Update compile() to accept and store environments**

Store references to `FunctionEnvironment` and `TypeEnvironment` so they're available during function compilation. The `compile()` function should:

1. Find the program (existing)
2. Iterate `FunctionEnvironment` to find user-defined functions, assign function IDs, and compile each
3. Compile the program (existing)
4. Build the container with all functions

This requires restructuring `compile()` to compile functions before the program, so call sites can look up function IDs.

- [ ] **Step 3: Implement user-defined function body compilation**

Add a function like `compile_user_function()` that:
1. Creates a fresh `CompileContext` for the function (variables start at 0)
2. Assigns variable slots for parameters (input vars) and locals
3. Compiles the function body statements
4. Emits `LOAD_VAR <return_slot>` + `RET` at the end (the return variable has the same name as the function)
5. Returns the emitter's bytecode, max stack depth, num_locals, and num_params

- [ ] **Step 4: Update compile_function_call to handle user-defined functions**

In `compile_function_call()`, before the `_` fallthrough arm, check if the function name is in `ctx.user_functions`. If so:
1. Look up the function signature from `FunctionEnvironment` to get parameter types
2. Compile each positional argument expression with the correct `op_type` based on the parameter's resolved elementary type (using `TypeEnvironment`)
3. Emit `CALL func_id`

- [ ] **Step 5: Wire up the container building**

Update `compile_program` (or the top-level `compile`) to add all compiled user functions to the container via `builder.add_function(...)`, including `num_params`.

- [ ] **Step 6: Add debug entries**

For each user-defined function:
- Add `FuncNameEntry` with the function's name and ID
- Add `VarNameEntry` for each parameter and local, scoped to the function's ID

- [ ] **Step 7: Run existing tests**

Run: `cd /workspaces/ironplc/compiler && cargo test`
Expected: All existing tests still pass (no regressions).

- [ ] **Step 8: Commit**

```bash
git add compiler/codegen/src/compile.rs
git commit -m "feat: compile user-defined function bodies and emit CALL at call sites"
```

---

## Task 9: End-to-end integration tests

**Files:**
- Create: `compiler/codegen/tests/end_to_end_user_function.rs`

- [ ] **Step 1: Write basic function call test**

```rust
mod common;

use common::{parse, parse_and_run};
use ironplc_codegen::compile;
use ironplc_vm::Slot;

#[test]
fn compile_when_user_function_returns_int_then_correct_result() {
    let source = "
FUNCTION ADD_INTS : INT
VAR_INPUT
    A : INT;
    B : INT;
END_VAR
    ADD_INTS := A + B;
END_FUNCTION

PROGRAM main
VAR
    result : INT;
END_VAR
    result := ADD_INTS(3, 7);
END_PROGRAM
";
    let (_container, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[0], Slot::from_i32(10));
}
```

- [ ] **Step 2: Write test for DINT function**

```rust
#[test]
fn compile_when_user_function_returns_dint_then_correct_result() {
    let source = "
FUNCTION MULTIPLY : DINT
VAR_INPUT
    A : DINT;
    B : DINT;
END_VAR
    MULTIPLY := A * B;
END_FUNCTION

PROGRAM main
VAR
    result : DINT;
END_VAR
    result := MULTIPLY(6, 7);
END_PROGRAM
";
    let (_container, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[0], Slot::from_i32(42));
}
```

- [ ] **Step 3: Write test for REAL function**

```rust
#[test]
fn compile_when_user_function_returns_real_then_correct_result() {
    let source = "
FUNCTION HALF : REAL
VAR_INPUT
    X : REAL;
END_VAR
    HALF := X / 2.0;
END_FUNCTION

PROGRAM main
VAR
    result : REAL;
END_VAR
    result := HALF(10.0);
END_PROGRAM
";
    let (_container, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[0], Slot::from_f32(5.0));
}
```

- [ ] **Step 4: Write test for nested function calls**

```rust
#[test]
fn compile_when_nested_function_calls_then_correct_result() {
    let source = "
FUNCTION DOUBLE : INT
VAR_INPUT
    X : INT;
END_VAR
    DOUBLE := X + X;
END_FUNCTION

PROGRAM main
VAR
    result : INT;
END_VAR
    result := DOUBLE(DOUBLE(3));
END_PROGRAM
";
    let (_container, bufs) = parse_and_run(source);
    // DOUBLE(3) = 6, DOUBLE(6) = 12
    assert_eq!(bufs.vars[0], Slot::from_i32(12));
}
```

- [ ] **Step 5: Write test for function with local variables**

```rust
#[test]
fn compile_when_function_has_locals_then_correct_result() {
    let source = "
FUNCTION CLAMP : INT
VAR_INPUT
    value : INT;
    low : INT;
    high : INT;
END_VAR
VAR
    temp : INT;
END_VAR
    IF value < low THEN
        temp := low;
    ELSIF value > high THEN
        temp := high;
    ELSE
        temp := value;
    END_IF;
    CLAMP := temp;
END_FUNCTION

PROGRAM main
VAR
    result : INT;
END_VAR
    result := CLAMP(50, 0, 10);
END_PROGRAM
";
    let (_container, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[0], Slot::from_i32(10));
}
```

- [ ] **Step 6: Write test for multiple different functions**

```rust
#[test]
fn compile_when_multiple_functions_then_correct_results() {
    let source = "
FUNCTION ADD_INTS : INT
VAR_INPUT
    A : INT;
    B : INT;
END_VAR
    ADD_INTS := A + B;
END_FUNCTION

FUNCTION NEGATE : INT
VAR_INPUT
    X : INT;
END_VAR
    NEGATE := -X;
END_FUNCTION

PROGRAM main
VAR
    pos : INT;
    neg : INT;
END_VAR
    pos := ADD_INTS(3, 7);
    neg := NEGATE(pos);
END_PROGRAM
";
    let (_container, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[0], Slot::from_i32(10));
    assert_eq!(bufs.vars[1], Slot::from_i32(-10));
}
```

- [ ] **Step 7: Run all tests**

Run: `cd /workspaces/ironplc/compiler && cargo test -p ironplc-codegen`
Expected: All tests pass.

- [ ] **Step 8: Commit**

```bash
git add compiler/codegen/tests/end_to_end_user_function.rs
git commit -m "test: add end-to-end integration tests for user-defined function calls"
```

---

## Task 10: Run full CI pipeline

- [ ] **Step 1: Run full CI**

Run: `cd /workspaces/ironplc/compiler && just`
Expected: Compile, coverage (85%+), and lint all pass.

- [ ] **Step 2: Fix any issues**

If clippy, format, or coverage issues arise, fix them.

- [ ] **Step 3: Final commit if needed**

```bash
git add -A
git commit -m "chore: fix lint and formatting issues"
```
