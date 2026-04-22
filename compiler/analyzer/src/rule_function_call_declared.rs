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
use ironplc_parser::options::CompilerOptions;

pub fn apply(
    lib: &Library,
    context: &SemanticContext,
    _options: &CompilerOptions,
) -> SemanticResult {
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
    use crate::test_helpers::{assert_rule_err, assert_rule_ok};
    use rstest::rstest;

    /// Wraps `body` (a sequence of ST statements) in a FUNCTION_BLOCK `CALLER`
    /// with a fixed VAR envelope. Cuts the boilerplate in tests that only
    /// differ in the call expression being exercised.
    fn caller_fb(vars: &str, body: &str) -> String {
        format!(
            "FUNCTION_BLOCK CALLER VAR {vars} END_VAR {body} END_FUNCTION_BLOCK"
        )
    }

    // --- Stdlib / user-defined function calls that should succeed ---

    // Every case wraps the caller in a FUNCTION_BLOCK named CALLER with the
    // given VAR declarations and body. The rule only cares about whether the
    // called function is declared and has the right arity.
    #[rstest]
    // Stdlib conversion.
    #[case::stdlib_int_to_real(
        "result : REAL; value : INT;",
        "result := INT_TO_REAL(value);"
    )]
    // User-defined function (declared separately in the outer library).
    #[case::user_function(
        "result : INT;",
        "result := ADD_INTS(1, 2);"
    )]
    // Stdlib monadic/dyadic numeric functions.
    #[case::abs_call("result : INT; value : INT;", "result := ABS(value);")]
    #[case::sqrt_call("result : REAL; value : REAL;", "result := SQRT(value);")]
    #[case::min_call("result : INT; a : INT; b : INT;", "result := MIN(a, b);")]
    #[case::max_call("result : INT; a : INT; b : INT;", "result := MAX(a, b);")]
    #[case::limit_call(
        "result : INT; low : INT; value : INT; high : INT;",
        "result := LIMIT(low, value, high);"
    )]
    #[case::expt_call(
        "result : INT; base : INT; exp : INT;",
        "result := EXPT(base, exp);"
    )]
    // MUX with variable arity (3 fixed + N-1 variants).
    #[case::mux_3_args(
        "result : INT; a : INT; b : INT;",
        "result := MUX(0, a, b);"
    )]
    #[case::mux_5_args(
        "result : INT; a : INT; b : INT; c : INT; d : INT;",
        "result := MUX(2, a, b, c, d);"
    )]
    #[case::mux_17_args(
        "result : INT; a : INT; b : INT; c : INT; d : INT; e : INT; f : INT; g : INT; h : INT; i : INT; j : INT; k : INT; l : INT; m : INT; n : INT; o : INT; p : INT;",
        "result := MUX(0, a, b, c, d, e, f, g, h, i, j, k, l, m, n, o, p);"
    )]
    // Arithmetic and comparison function families exercised in one program each.
    #[case::arithmetic_family(
        "result : INT; a : INT; b : INT;",
        "result := ADD(a, b); result := SUB(a, b); result := MUL(a, b); result := DIV(a, b);"
    )]
    #[case::comparison_family(
        "result : BOOL; a : INT; b : INT;",
        "result := GT(a, b); result := GE(a, b); result := EQ(a, b); result := LE(a, b); result := LT(a, b); result := NE(a, b);"
    )]
    // Bit-string conversion matrix (BYTE↔WORD↔DWORD↔LWORD).
    #[case::bit_string_conversion_matrix(
        "b : BYTE; w : WORD; d : DWORD; l : LWORD;",
        "w := BYTE_TO_WORD(b); d := BYTE_TO_DWORD(b); l := BYTE_TO_LWORD(b); b := WORD_TO_BYTE(w); d := WORD_TO_DWORD(w); l := WORD_TO_LWORD(w); b := DWORD_TO_BYTE(d); w := DWORD_TO_WORD(d); l := DWORD_TO_LWORD(d); b := LWORD_TO_BYTE(l); w := LWORD_TO_WORD(l); d := LWORD_TO_DWORD(l);"
    )]
    fn apply_when_valid_call_then_ok(#[case] vars: &str, #[case] body: &str) {
        // Prepend the ADD_INTS declaration only when the body actually calls it;
        // otherwise the rule would flag ADD_INTS itself as undeclared.
        let prelude = if body.contains("ADD_INTS") {
            "FUNCTION ADD_INTS : INT VAR_INPUT A : INT; B : INT; END_VAR ADD_INTS := A + B; END_FUNCTION "
        } else {
            ""
        };
        let program = format!("{prelude}{}", caller_fb(vars, body));
        assert_rule_ok(apply, &program);
    }

    // --- Undeclared-function diagnostics ---

    #[rstest]
    #[case::caller_calls_undeclared(
        "result : INT;",
        "result := NONEXISTENT_FUNC(1);"
    )]
    fn apply_when_function_not_declared_then_error(#[case] vars: &str, #[case] body: &str) {
        assert_rule_err(
            apply,
            &caller_fb(vars, body),
            Problem::FunctionCallUndeclared.code(),
        );
    }

    // User-defined function that itself calls an undeclared function.
    #[test]
    fn apply_when_function_calls_undeclared_function_then_error() {
        assert_rule_err(
            apply,
            "FUNCTION MY_FUNC : INT VAR_INPUT x : INT; END_VAR MY_FUNC := UNDEFINED_HELPER(x); END_FUNCTION",
            Problem::FunctionCallUndeclared.code(),
        );
    }

    // --- Wrong-arg-count diagnostics ---

    #[rstest]
    // Too many args to a fixed-arity stdlib function.
    #[case::int_to_real_with_two_args(
        "result : REAL; value : INT;",
        "result := INT_TO_REAL(value, value);"
    )]
    // Too few args to a 3-arity stdlib function.
    #[case::limit_too_few(
        "result : INT; a : INT; b : INT;",
        "result := LIMIT(a, b);"
    )]
    // MUX with fewer than 3 args.
    #[case::mux_too_few(
        "result : INT; a : INT;",
        "result := MUX(0, a);"
    )]
    // MUX with more than 17 args (16 data + 1 selector max).
    #[case::mux_18_args(
        "result : INT; a : INT; b : INT; c : INT; d : INT; e : INT; f : INT; g : INT; h : INT; i : INT; j : INT; k : INT; l : INT; m : INT; n : INT; o : INT; p : INT; q : INT;",
        "result := MUX(0, a, b, c, d, e, f, g, h, i, j, k, l, m, n, o, p, q);"
    )]
    fn apply_when_wrong_arg_count_then_error(#[case] vars: &str, #[case] body: &str) {
        assert_rule_err(
            apply,
            &caller_fb(vars, body),
            Problem::FunctionCallWrongArgCount.code(),
        );
    }

    // User-defined function called with too few args.
    #[test]
    fn apply_when_too_few_args_then_error() {
        assert_rule_err(
            apply,
            "FUNCTION ADD_INTS : INT VAR_INPUT A : INT; B : INT; END_VAR ADD_INTS := A + B; END_FUNCTION FUNCTION_BLOCK CALLER VAR result : INT; END_VAR result := ADD_INTS(1); END_FUNCTION_BLOCK",
            Problem::FunctionCallWrongArgCount.code(),
        );
    }

    // Nested stdlib call (SHL of BYTE_TO_WORD) inside a user-defined function.
    #[test]
    fn apply_when_shl_with_byte_to_word_conversion_then_ok() {
        assert_rule_ok(
            apply,
            "FUNCTION MY_SHIFT : WORD VAR_INPUT B : BYTE; END_VAR MY_SHIFT := SHL(BYTE_TO_WORD(B), 8); END_FUNCTION PROGRAM main VAR result : WORD; END_VAR result := MY_SHIFT(B := BYTE#16#AB); END_PROGRAM",
        );
    }
}
