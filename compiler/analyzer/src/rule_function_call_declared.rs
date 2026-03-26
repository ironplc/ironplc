//! Semantic rule that validates function calls reference declared functions
//! and have the correct number of arguments.
//!
//! ## Passes
//!
//! ```ignore
//! FUNCTION_BLOCK CALLER
//! VAR
//!     result : REAL;
//!     value : INT;
//! END_VAR
//!     result := INT_TO_REAL(value);
//! END_FUNCTION_BLOCK
//! ```
//!
//! ## Fails (Function Not Declared)
//!
//! ```ignore
//! FUNCTION_BLOCK CALLER
//! VAR
//!     result : REAL;
//!     value : INT;
//! END_VAR
//!     result := NONEXISTENT_FUNC(value);
//! END_FUNCTION_BLOCK
//! ```

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
    let mut visitor = RuleFunctionCallDeclared {
        context,
        diagnostics: vec![],
    };

    visitor.walk(lib).map_err(|e| vec![e])?;

    if visitor.diagnostics.is_empty() {
        Ok(())
    } else {
        Err(visitor.diagnostics)
    }
}

struct RuleFunctionCallDeclared<'a> {
    context: &'a SemanticContext,
    diagnostics: Vec<Diagnostic>,
}

impl Visitor<Diagnostic> for RuleFunctionCallDeclared<'_> {
    type Value = ();

    fn visit_function(&mut self, node: &Function) -> Result<Self::Value, Diagnostic> {
        // Look up the function in the function environment
        let func_sig = self.context.functions.get(&node.name);

        match func_sig {
            None => {
                // Function is not declared
                self.diagnostics.push(
                    Diagnostic::problem(
                        Problem::FunctionCallUndeclared,
                        Label::span(node.name.span(), "Function call"),
                    )
                    .with_context("function", &node.name.original().to_string()),
                );
            }
            Some(signature) => {
                // Function exists, check argument count
                // Count positional input arguments in the call
                let call_input_count = node
                    .param_assignment
                    .iter()
                    .filter(|p| matches!(p, ParamAssignmentKind::PositionalInput(_)))
                    .count();

                // Count named input arguments
                let call_named_count = node
                    .param_assignment
                    .iter()
                    .filter(|p| matches!(p, ParamAssignmentKind::NamedInput(_)))
                    .count();

                // Total input arguments provided
                let total_inputs = call_input_count + call_named_count;

                // Expected input parameter count
                let expected_inputs = signature.input_parameter_count();

                let args_valid = if signature.is_extensible {
                    let above_min = total_inputs >= expected_inputs;
                    let below_max = signature.max_inputs.is_none_or(|max| total_inputs <= max);
                    above_min && below_max
                } else {
                    total_inputs == expected_inputs
                };

                if !args_valid {
                    self.diagnostics.push(
                        Diagnostic::problem(
                            Problem::FunctionCallWrongArgCount,
                            Label::span(node.name.span(), "Function call"),
                        )
                        .with_context("function", &node.name.original().to_string())
                        .with_context("expected", &expected_inputs.to_string())
                        .with_context("actual", &total_inputs.to_string()),
                    );
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
    fn apply_when_stdlib_function_called_then_ok() {
        let program = "
FUNCTION_BLOCK CALLER
VAR
    result : REAL;
    value : INT;
END_VAR
    result := INT_TO_REAL(value);
END_FUNCTION_BLOCK";

        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context);

        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_user_function_called_then_ok() {
        let program = "
FUNCTION ADD_INTS : INT
VAR_INPUT
    A : INT;
    B : INT;
END_VAR
    ADD_INTS := A + B;
END_FUNCTION

FUNCTION_BLOCK CALLER
VAR
    result : INT;
END_VAR
    result := ADD_INTS(1, 2);
END_FUNCTION_BLOCK";

        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context);

        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_function_not_declared_then_error() {
        let program = "
FUNCTION_BLOCK CALLER
VAR
    result : INT;
END_VAR
    result := NONEXISTENT_FUNC(1);
END_FUNCTION_BLOCK";

        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context);

        assert!(result.is_err());
        let diagnostics = result.unwrap_err();
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, Problem::FunctionCallUndeclared.code());
    }

    #[test]
    fn apply_when_function_calls_undeclared_function_then_error() {
        let program = "
FUNCTION MY_FUNC : INT
VAR_INPUT
    x : INT;
END_VAR
    MY_FUNC := UNDEFINED_HELPER(x);
END_FUNCTION";

        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context);

        assert!(result.is_err());
        let diagnostics = result.unwrap_err();
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, Problem::FunctionCallUndeclared.code());
    }

    #[test]
    fn apply_when_wrong_arg_count_then_error() {
        let program = "
FUNCTION_BLOCK CALLER
VAR
    result : REAL;
    value : INT;
END_VAR
    result := INT_TO_REAL(value, value);
END_FUNCTION_BLOCK";

        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context);

        assert!(result.is_err());
        let diagnostics = result.unwrap_err();
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].code,
            Problem::FunctionCallWrongArgCount.code()
        );
    }

    #[test]
    fn apply_when_abs_called_then_ok() {
        let program = "
FUNCTION_BLOCK CALLER
VAR
    result : INT;
    value : INT;
END_VAR
    result := ABS(value);
END_FUNCTION_BLOCK";

        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context);

        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_sqrt_called_then_ok() {
        let program = "
FUNCTION_BLOCK CALLER
VAR
    result : REAL;
    value : REAL;
END_VAR
    result := SQRT(value);
END_FUNCTION_BLOCK";

        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context);

        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_min_called_then_ok() {
        let program = "
FUNCTION_BLOCK CALLER
VAR
    result : INT;
    a : INT;
    b : INT;
END_VAR
    result := MIN(a, b);
END_FUNCTION_BLOCK";

        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context);

        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_max_called_then_ok() {
        let program = "
FUNCTION_BLOCK CALLER
VAR
    result : INT;
    a : INT;
    b : INT;
END_VAR
    result := MAX(a, b);
END_FUNCTION_BLOCK";

        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context);

        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_limit_called_then_ok() {
        let program = "
FUNCTION_BLOCK CALLER
VAR
    result : INT;
    low : INT;
    value : INT;
    high : INT;
END_VAR
    result := LIMIT(low, value, high);
END_FUNCTION_BLOCK";

        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context);

        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_limit_called_with_wrong_arg_count_then_error() {
        let program = "
FUNCTION_BLOCK CALLER
VAR
    result : INT;
    a : INT;
    b : INT;
END_VAR
    result := LIMIT(a, b);
END_FUNCTION_BLOCK";

        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context);

        assert!(result.is_err());
        let diagnostics = result.unwrap_err();
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].code,
            Problem::FunctionCallWrongArgCount.code()
        );
    }

    #[test]
    fn apply_when_expt_called_then_ok() {
        let program = "
FUNCTION_BLOCK CALLER
VAR
    result : INT;
    base : INT;
    exp : INT;
END_VAR
    result := EXPT(base, exp);
END_FUNCTION_BLOCK";

        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context);

        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_mux_called_with_3_args_then_ok() {
        let program = "
FUNCTION_BLOCK CALLER
VAR
    result : INT;
    a : INT;
    b : INT;
END_VAR
    result := MUX(0, a, b);
END_FUNCTION_BLOCK";

        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context);

        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_mux_called_with_5_args_then_ok() {
        let program = "
FUNCTION_BLOCK CALLER
VAR
    result : INT;
    a : INT;
    b : INT;
    c : INT;
    d : INT;
END_VAR
    result := MUX(2, a, b, c, d);
END_FUNCTION_BLOCK";

        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context);

        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_mux_called_with_too_few_args_then_error() {
        let program = "
FUNCTION_BLOCK CALLER
VAR
    result : INT;
    a : INT;
END_VAR
    result := MUX(0, a);
END_FUNCTION_BLOCK";

        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context);

        assert!(result.is_err());
        let diagnostics = result.unwrap_err();
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].code,
            Problem::FunctionCallWrongArgCount.code()
        );
    }

    #[test]
    fn apply_when_mux_called_with_17_args_then_ok() {
        let program = "
FUNCTION_BLOCK CALLER
VAR
    result : INT;
    a : INT;
    b : INT;
    c : INT;
    d : INT;
    e : INT;
    f : INT;
    g : INT;
    h : INT;
    i : INT;
    j : INT;
    k : INT;
    l : INT;
    m : INT;
    n : INT;
    o : INT;
    p : INT;
END_VAR
    result := MUX(0, a, b, c, d, e, f, g, h, i, j, k, l, m, n, o, p);
END_FUNCTION_BLOCK";

        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context);

        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_mux_called_with_18_args_then_error() {
        let program = "
FUNCTION_BLOCK CALLER
VAR
    result : INT;
    a : INT;
    b : INT;
    c : INT;
    d : INT;
    e : INT;
    f : INT;
    g : INT;
    h : INT;
    i : INT;
    j : INT;
    k : INT;
    l : INT;
    m : INT;
    n : INT;
    o : INT;
    p : INT;
    q : INT;
END_VAR
    result := MUX(0, a, b, c, d, e, f, g, h, i, j, k, l, m, n, o, p, q);
END_FUNCTION_BLOCK";

        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context);

        assert!(result.is_err());
        let diagnostics = result.unwrap_err();
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].code,
            Problem::FunctionCallWrongArgCount.code()
        );
    }

    #[test]
    fn apply_when_too_few_args_then_error() {
        let program = "
FUNCTION ADD_INTS : INT
VAR_INPUT
    A : INT;
    B : INT;
END_VAR
    ADD_INTS := A + B;
END_FUNCTION

FUNCTION_BLOCK CALLER
VAR
    result : INT;
END_VAR
    result := ADD_INTS(1);
END_FUNCTION_BLOCK";

        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context);

        assert!(result.is_err());
        let diagnostics = result.unwrap_err();
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].code,
            Problem::FunctionCallWrongArgCount.code()
        );
    }

    #[test]
    fn apply_when_arithmetic_function_called_then_ok() {
        // Note: MOD is excluded because the parser treats it as a keyword (the MOD operator).
        // MOD(a, b) requires parser changes to allow keywords in function call position.
        let program = "
FUNCTION_BLOCK CALLER
VAR
    result : INT;
    a : INT;
    b : INT;
END_VAR
    result := ADD(a, b);
    result := SUB(a, b);
    result := MUL(a, b);
    result := DIV(a, b);
END_FUNCTION_BLOCK";

        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context);

        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_comparison_function_called_then_ok() {
        let program = "
FUNCTION_BLOCK CALLER
VAR
    result : BOOL;
    a : INT;
    b : INT;
END_VAR
    result := GT(a, b);
    result := GE(a, b);
    result := EQ(a, b);
    result := LE(a, b);
    result := LT(a, b);
    result := NE(a, b);
END_FUNCTION_BLOCK";

        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context);

        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_bit_string_conversion_called_then_ok() {
        let program = "
FUNCTION_BLOCK CALLER
VAR
    b : BYTE;
    w : WORD;
    d : DWORD;
    l : LWORD;
END_VAR
    w := BYTE_TO_WORD(b);
    d := BYTE_TO_DWORD(b);
    l := BYTE_TO_LWORD(b);
    b := WORD_TO_BYTE(w);
    d := WORD_TO_DWORD(w);
    l := WORD_TO_LWORD(w);
    b := DWORD_TO_BYTE(d);
    w := DWORD_TO_WORD(d);
    l := DWORD_TO_LWORD(d);
    b := LWORD_TO_BYTE(l);
    w := LWORD_TO_WORD(l);
    d := LWORD_TO_DWORD(l);
END_FUNCTION_BLOCK";

        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context);

        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_shl_with_byte_to_word_conversion_then_ok() {
        let program = "
FUNCTION MY_SHIFT : WORD
VAR_INPUT
    B : BYTE;
END_VAR
    MY_SHIFT := SHL(BYTE_TO_WORD(B), 8);
END_FUNCTION

PROGRAM main
VAR
    result : WORD;
END_VAR
    result := MY_SHIFT(B := BYTE#16#AB);
END_PROGRAM";

        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context);

        assert!(result.is_ok());
    }
}
