//! Semantic rule that checks the operands of an operator against the types
//! the operator is defined for.
//!
//! Two families are checked:
//!
//! - The arithmetic operators `+`, `-`, `*`, `/` and `MOD`, and the calls
//!   `ADD`, `SUB`, `MUL` and `DIV`. IEC 61131-3 defines each as a numeric
//!   overload (Table 24) and, for the first four, a set of typed overloads
//!   on the time and date types (Table 30). The rule asks
//!   [`resolve_arithmetic_overload`] whether any overload applies to the
//!   operand types, the same question the type resolver and codegen ask, so
//!   what this rule accepts is what they type and compile. An expression no
//!   overload applies to is reported once, naming both operand types; a
//!   call is folded from the left and reported at the step that fails. See
//!   `specs/design/arithmetic-operator-overloads.md`.
//! - `AND`, `OR`, `XOR` and `NOT`, defined over `ANY_BIT` (Table 28). See
//!   [`checked_compare_form`] and [`checked_unary_form`]. Each operand
//!   outside `ANY_BIT` is reported. This family matters because codegen
//!   selects its opcode by width and signedness, which cannot separate
//!   `BOOL` from a signed integer: `d AND 3` with `d : DINT := 10` yielded 1
//!   rather than 2, and `NOT d` yielded 0 rather than -11, with no
//!   diagnostic. Issue #1567 fixed the functional spelling of the same
//!   family by declaring it `ANY_BIT`; this holds the operator spelling to
//!   that row.
//!
//! An operand whose resolved type the predicate cannot judge (a subrange,
//! an enumeration, a structure) is skipped rather than reported, as the
//! assignment check skips such targets: `p MOD 2` on a subrange of `INT`
//! compiles and this rule leaves that alone.
//!
//! ## Passes
//!
//! ```ignore
//! PROGRAM main
//! VAR
//!     d : DINT;
//! END_VAR
//!     d := d MOD 2;
//! END_PROGRAM
//! ```
//!
//! ## Fails
//!
//! ```ignore
//! PROGRAM main
//! VAR
//!     r : REAL;
//!     d : DINT;
//! END_VAR
//!     r := r MOD 2.0;    (* P4049: MOD is not defined for REAL *)
//!     d := d + r;        (* P4049: DINT does not widen to REAL *)
//!     d := d AND 3;      (* P4049: AND is not defined for DINT *)
//! END_PROGRAM
//! ```

use ironplc_dsl::{
    common::*,
    core::Located,
    diagnostic::{Diagnostic, Label},
    textual::*,
    visitor::Visitor,
};
use ironplc_parser::options::CompilerOptions;
use ironplc_problems::Problem;
use std::convert::Infallible;

use crate::intermediates::arithmetic_overload::{
    resolve_arithmetic_fold, resolve_arithmetic_overload, FoldFailure,
};
use crate::intermediates::operator_function_form::{
    form_of_operator, operator_function_form, FormOf, OperatorFunctionForm,
};
use crate::result::SemanticResult;
use crate::rule_support::{run_rule, DiagnosticVisitor};
use crate::semantic_context::SemanticContext;
use crate::type_compat::{are_types_compatible, is_checkable_type};

pub fn apply(
    lib: &Library,
    _context: &SemanticContext,
    options: &CompilerOptions,
) -> SemanticResult {
    run_rule(
        RuleOperatorOperandTypeCheck {
            options,
            diagnostics: vec![],
        },
        lib,
    )
}

/// Returns the row whose operand type the operands of `op` must have, or
/// `None` for a compare operator this rule does not check.
///
/// `AND`, `OR` and `XOR` are defined over `ANY_BIT` (IEC 61131-3 Table 28).
/// Their operator spelling reaches codegen through the same path as `BOOL`
/// operands do, so an `ANY_INT` operand is lowered to the *logical* opcode
/// and the program silently computes a truthiness, not a bit pattern:
/// `x AND 3` with `x : DINT := 10` yields 1 rather than 2. Holding them to
/// the row is what makes the operator spelling agree with the functional
/// one, which issue #1567 already fixed in the other direction.
///
/// The relational operators are declared `ANY_ELEMENTARY` in the table.
/// Holding them to that is a separate decision from this rule.
fn checked_compare_form(op: &CompareOp) -> Option<&'static OperatorFunctionForm> {
    match op {
        CompareOp::And | CompareOp::Or | CompareOp::Xor => {
            form_of_operator(&FormOf::Compare(op.clone()))
        }
        // `AND_THEN` and `OR_ELSE` are a CODESYS/TwinCAT short-circuit
        // extension rather than IEC operators, so the table has no row to
        // hold them to.
        CompareOp::AndThen | CompareOp::OrElse => None,
        CompareOp::Eq
        | CompareOp::Ne
        | CompareOp::Lt
        | CompareOp::Gt
        | CompareOp::LtEq
        | CompareOp::GtEq => None,
    }
}

/// Returns the row whose operand type the operand of `op` must have, or
/// `None` for a unary operator this rule does not check.
///
/// `NOT` is defined over `ANY_BIT` (IEC 61131-3 Table 28). Codegen selects
/// its opcode by width and signedness, which cannot tell `BOOL` from a
/// signed integer -- both are `(W32, Signed)` -- so `NOT` over `ANY_INT`
/// falls through to `BOOL_NOT` and `NOT x` with `x : DINT := 10` yields 0
/// rather than -11. Rejecting the operand here is what keeps that arm
/// reachable only for `BOOL`.
fn checked_unary_form(op: &UnaryOp) -> Option<&'static OperatorFunctionForm> {
    match op {
        UnaryOp::Not => form_of_operator(&FormOf::Not),
        // Negation is `ANY_NUM`; holding it to that is a separate decision,
        // outside the arithmetic overloads (it has no function form).
        UnaryOp::Neg => None,
    }
}

struct RuleOperatorOperandTypeCheck<'a> {
    options: &'a CompilerOptions,
    diagnostics: Vec<Diagnostic>,
}

impl DiagnosticVisitor for RuleOperatorOperandTypeCheck<'_> {
    fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

impl RuleOperatorOperandTypeCheck<'_> {
    /// Reports P4049 when no overload of the arithmetic operator applies to
    /// the operand types of `binary`, labelled at the whole expression.
    fn check_arithmetic_operator(&mut self, expr: &Expr, binary: &BinaryExpr) {
        let left = binary.left.resolved_type.as_ref();
        let right = binary.right.resolved_type.as_ref();
        if resolve_arithmetic_overload(&binary.op, left, right, self.options).is_some() {
            return;
        }
        // `None` is only returned for a pair that has both types.
        if let (Some(left), Some(right)) = (left, right) {
            self.report_arithmetic(
                Label::span(expr.span(), "Expression"),
                &binary.op.to_string(),
                left,
                right,
            );
        }
    }

    /// Reports P4049 when a call to `ADD`, `SUB`, `MUL` or `DIV`, folded from
    /// the left, has a step no overload applies to, labelled at the name.
    ///
    /// Only these four are checked here: they are the arithmetic functions
    /// with typed overloads, and the function-call rule leaves their inputs
    /// to this one. `MOD(a, b)` has no overloads and is checked against its
    /// signature by the function-call rule.
    fn check_arithmetic_call(&mut self, function: &Function) {
        let Some(form) = operator_function_form(&function.name.to_string()) else {
            return;
        };
        let FormOf::Arithmetic(op) = &form.operator else {
            return;
        };
        if form.typed_overloads().is_empty() {
            return;
        }
        let inputs: Option<Vec<Option<&TypeName>>> = function
            .param_assignment
            .iter()
            .map(|p| match p {
                ParamAssignmentKind::PositionalInput(input) => {
                    Some(input.expr.resolved_type.as_ref())
                }
                ParamAssignmentKind::NamedInput(_) | ParamAssignmentKind::Output(_) => None,
            })
            .collect();
        // A named input is left only on a call the named-argument pass has
        // already diagnosed.
        let Some(inputs) = inputs else {
            return;
        };
        if let Err(FoldFailure { left, right }) = resolve_arithmetic_fold(op, &inputs, self.options)
        {
            self.report_arithmetic(
                Label::span(function.name.span(), "Function call"),
                &function.name.original().to_string(),
                &left,
                &right,
            );
        }
    }

    /// Pushes a P4049 naming the operator (or function) and both operand
    /// types. The types are upper-cased: a fold step's left type is an
    /// overload's result type while the right is an operand's resolved type,
    /// and the two are spelled differently.
    fn report_arithmetic(
        &mut self,
        label: Label,
        operator: &str,
        left: &TypeName,
        right: &TypeName,
    ) {
        self.diagnostics.push(
            Diagnostic::problem(Problem::OperatorOperandTypeMismatch, label)
                .with_context("operator", &operator.to_string())
                .with_context("left", &left.to_string().to_uppercase())
                .with_context("right", &right.to_string().to_uppercase()),
        );
    }

    /// Reports P4049 for each operand of `op` that is not acceptable where the
    /// row's operand type is required.
    ///
    /// Every operator this rule checks asks the same question of each of its
    /// operands, so arity is just the length of `operands`: two for an
    /// arithmetic or compare expression, one for `NOT`.
    ///
    /// The row carries the operator's name as well as its operand type, so
    /// no caller passes a label: the name and the type a diagnostic reports
    /// come from the same row and cannot disagree.
    fn check_operands(&mut self, form: &OperatorFunctionForm, operands: &[&Expr]) {
        let expected = form.operand_type();
        for operand in operands {
            self.check_operand(form, &expected, operand);
        }
    }

    /// Reports P4049 when `operand`'s resolved type is one the predicate can
    /// judge and it is not acceptable where `expected` is required.
    fn check_operand(&mut self, form: &OperatorFunctionForm, expected: &TypeName, operand: &Expr) {
        let Some(actual) = operand.resolved_type.as_ref() else {
            return;
        };
        if !is_checkable_type(actual) {
            return;
        }
        if !are_types_compatible(expected, actual, self.options) {
            self.diagnostics.push(
                Diagnostic::problem(
                    Problem::OperatorOperandTypeMismatch,
                    Label::span(operand.span(), "Operand"),
                )
                .with_context("operator", &form.name.to_string())
                .with_context("expected", &expected.to_string())
                .with_context("actual", &actual.to_string()),
            );
        }
    }
}

impl Visitor<Infallible> for RuleOperatorOperandTypeCheck<'_> {
    type Value = ();

    /// Checks an operator expression at the `Expr` that holds it, rather than
    /// at the operator node, so a check has the span of the whole expression
    /// as well as each operand's.
    fn visit_expr(&mut self, node: &Expr) -> Result<Self::Value, Infallible> {
        match &node.kind {
            ExprKind::BinaryOp(binary) => self.check_arithmetic_operator(node, binary),
            ExprKind::Function(function) => self.check_arithmetic_call(function),
            ExprKind::Compare(compare) => {
                if let Some(form) = checked_compare_form(&compare.op) {
                    self.check_operands(form, &[&compare.left, &compare.right]);
                }
            }
            ExprKind::UnaryOp(unary) => {
                if let Some(form) = checked_unary_form(&unary.op) {
                    self.check_operands(form, &[&unary.term]);
                }
            }
            _ => {}
        }
        node.recurse_visit(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    rule_ctx_ok!(
        apply_when_mod_of_integer_variables_then_ok,
        "
PROGRAM main
VAR
    a : DINT;
    b : DINT;
    c : DINT;
END_VAR
    c := a MOD b;
END_PROGRAM"
    );

    rule_ctx_ok!(
        apply_when_mod_of_integer_variable_and_literal_then_ok,
        "
PROGRAM main
VAR
    a : UINT;
    c : UINT;
END_VAR
    c := a MOD 2;
END_PROGRAM"
    );

    rule_ctx_ok!(
        apply_when_mod_of_typed_integer_literals_then_ok,
        "
PROGRAM main
VAR
    c : LINT;
END_VAR
    c := LINT#100 MOD LINT#7;
END_PROGRAM"
    );

    rule_ctx_ok!(
        /// The operand type is a subrange, which the predicate cannot judge, so
        /// the rule leaves it alone rather than reporting it.
        apply_when_mod_of_subrange_variable_then_ok,
        "
TYPE
    Pct : INT (0..100);
END_TYPE

PROGRAM main
VAR
    p : Pct := 5;
    c : INT;
END_VAR
    c := p MOD 2;
END_PROGRAM"
    );

    rule_ctx_ok!(
        /// `+` on two TIME operands is the typed overload ADD_TIME.
        apply_when_add_of_time_variables_then_ok,
        "
PROGRAM main
VAR
    t1 : TIME := T#1s;
    t2 : TIME := T#2s;
    t3 : TIME;
END_VAR
    t3 := t1 + t2;
END_PROGRAM"
    );

    rule_ctx_err1!(
        /// An arithmetic expression is reported once, not once per operand.
        apply_when_mod_of_real_variables_then_p4049_once,
        "
PROGRAM main
VAR
    r1 : REAL := 7.5;
    r2 : REAL := 2.0;
    r3 : REAL;
END_VAR
    r3 := r1 MOD r2;
END_PROGRAM",
        Problem::OperatorOperandTypeMismatch
    );

    rule_ctx_err1!(
        apply_when_mod_of_integer_variable_by_real_literal_then_p4049,
        "
PROGRAM main
VAR
    d : DINT;
END_VAR
    d := d MOD 2.0;
END_PROGRAM",
        Problem::OperatorOperandTypeMismatch
    );

    rule_ctx_err1!(
        apply_when_mod_of_lreal_variable_by_integer_literal_then_p4049,
        "
PROGRAM main
VAR
    l : LREAL;
END_VAR
    l := l MOD 2;
END_PROGRAM",
        Problem::OperatorOperandTypeMismatch
    );

    rule_ctx_err1!(
        /// Real literals are not folded for MOD, so the rule sees them.
        apply_when_mod_of_real_literals_then_p4049_once,
        "
PROGRAM main
VAR
    r : REAL;
END_VAR
    r := 7.5 MOD 2.0;
END_PROGRAM",
        Problem::OperatorOperandTypeMismatch
    );

    rule_ctx_err1!(
        apply_when_mod_nested_in_larger_expression_then_p4049,
        "
PROGRAM main
VAR
    r : REAL;
    d : DINT;
END_VAR
    d := 1 + (d MOD (r * 2.0));
END_PROGRAM",
        Problem::OperatorOperandTypeMismatch
    );

    rule_ctx_err1!(
        /// No overload multiplies two durations.
        apply_when_mul_of_time_variables_then_p4049,
        "
PROGRAM main
VAR
    t1 : TIME;
    t2 : TIME;
END_VAR
    t1 := t1 * t2;
END_PROGRAM",
        Problem::OperatorOperandTypeMismatch
    );

    rule_ctx_err1!(
        /// DINT does not widen losslessly to REAL, nor REAL to DINT.
        apply_when_add_of_dint_and_real_then_p4049,
        "
PROGRAM main
VAR
    d : DINT;
    r : REAL;
END_VAR
    r := r + d;
END_PROGRAM",
        Problem::OperatorOperandTypeMismatch
    );

    rule_ctx_ok!(
        apply_when_sub_of_dates_then_ok,
        "
PROGRAM main
VAR
    d1 : DATE;
    d2 : DATE;
    t : TIME;
END_VAR
    t := d1 - d2;
END_PROGRAM"
    );

    rule_ctx_err1!(
        /// The function form is folded from the left and reported at the
        /// step that fails: TIME + REAL.
        apply_when_add_call_on_time_and_real_then_p4049,
        "
PROGRAM main
VAR
    t : TIME;
    r : REAL;
END_VAR
    t := ADD(t, t, r);
END_PROGRAM",
        Problem::OperatorOperandTypeMismatch
    );

    rule_ctx_ok!(
        apply_when_add_call_on_times_then_ok,
        "
PROGRAM main
VAR
    t : TIME;
END_VAR
    t := ADD(t, t, t);
END_PROGRAM"
    );

    rule_ctx_ok!(
        /// MOD has no overloads, so its function form is left to the
        /// function-call rule.
        apply_when_mod_call_on_real_then_not_this_rule,
        "
PROGRAM main
VAR
    r : REAL;
END_VAR
    r := MOD(r, r);
END_PROGRAM"
    );

    rule_ctx_err1!(
        /// Bit-string arithmetic needs --allow-bit-string-arithmetic.
        apply_when_add_of_byte_without_flag_then_p4049,
        "
PROGRAM main
VAR
    b : BYTE;
END_VAR
    b := b + 1;
END_PROGRAM",
        Problem::OperatorOperandTypeMismatch
    );

    #[test]
    fn apply_when_add_of_byte_with_flag_then_ok() {
        let (library, context) = crate::test_helpers::parse_and_resolve_types_with_context(
            "
PROGRAM main
VAR
    b : BYTE;
END_VAR
    b := b + 1;
END_PROGRAM",
        );
        let options = CompilerOptions {
            allow_bit_string_arithmetic: true,
            ..CompilerOptions::default()
        };
        assert!(apply(&library, &context, &options).is_ok());
    }

    #[test]
    fn apply_when_call_fails_then_diagnostic_names_function_and_failing_step() {
        let (library, context) = crate::test_helpers::parse_and_resolve_types_with_context(
            "
PROGRAM main
VAR
    t : TIME;
    r : REAL;
END_VAR
    t := MUL(t, r, t);
END_PROGRAM",
        );
        let errors = apply(&library, &context, &CompilerOptions::default()).unwrap_err();
        assert_eq!(errors.len(), 1);
        let described = &errors[0].described;
        assert!(
            described.contains(&"operator=MUL".to_owned()),
            "{described:?}"
        );
        assert!(described.contains(&"left=TIME".to_owned()), "{described:?}");
        assert!(
            described.contains(&"right=TIME".to_owned()),
            "{described:?}"
        );
    }

    #[test]
    fn apply_when_mod_of_real_then_diagnostic_names_operator_and_types() {
        let (library, context) = crate::test_helpers::parse_and_resolve_types_with_context(
            "
PROGRAM main
VAR
    d : DINT;
    r : REAL;
END_VAR
    d := d MOD r;
END_PROGRAM",
        );
        let errors = apply(&library, &context, &CompilerOptions::default()).unwrap_err();
        assert_eq!(errors.len(), 1);
        let described = &errors[0].described;
        assert!(
            described.contains(&"operator=MOD".to_owned()),
            "{described:?}"
        );
        assert!(described.contains(&"left=DINT".to_owned()), "{described:?}");
        assert!(
            described.contains(&"right=REAL".to_owned()),
            "{described:?}"
        );
    }

    #[test]
    fn analyze_when_mod_of_real_then_pipeline_reports_p4049() {
        // The rule is wired into the full `analyze` pipeline, which collects
        // semantic diagnostics into the context rather than returning Err.
        use crate::stages::analyze;
        let library = crate::test_helpers::parse_only(
            "
PROGRAM main
VAR
    r1 : REAL := 7.5;
    r2 : REAL := 2.0;
    r3 : REAL;
END_VAR
    r3 := r1 MOD r2;
END_PROGRAM
",
        );
        let (_lib, context) = analyze(&[&library], &CompilerOptions::default()).unwrap();
        assert!(context
            .diagnostics()
            .iter()
            .any(|d| d.code == Problem::OperatorOperandTypeMismatch.code()));
    }

    // --- The bit-string operators (AND, OR, XOR, NOT) ---

    rule_ctx_ok!(
        apply_when_and_of_bit_string_variables_then_ok,
        "
PROGRAM main
VAR
    b1 : BYTE;
    b2 : BYTE;
    b3 : BYTE;
END_VAR
    b3 := b1 AND b2;
END_PROGRAM"
    );

    rule_ctx_ok!(
        apply_when_or_of_word_variables_then_ok,
        "
PROGRAM main
VAR
    w1 : WORD;
    w2 : WORD;
    w3 : WORD;
END_VAR
    w3 := w1 OR w2;
END_PROGRAM"
    );

    rule_ctx_ok!(
        apply_when_xor_of_bool_variables_then_ok,
        "
PROGRAM main
VAR
    a : BOOL;
    b : BOOL;
    c : BOOL;
END_VAR
    c := a XOR b;
END_PROGRAM"
    );

    rule_ctx_ok!(
        apply_when_not_of_bool_variable_then_ok,
        "
PROGRAM main
VAR
    a : BOOL;
    b : BOOL;
END_VAR
    b := NOT a;
END_PROGRAM"
    );

    rule_ctx_ok!(
        apply_when_not_of_bit_string_variable_then_ok,
        "
PROGRAM main
VAR
    b1 : BYTE;
    b2 : BYTE;
END_VAR
    b2 := NOT b1;
END_PROGRAM"
    );

    rule_ctx_ok!(
        /// The operands are comparison results, which are BOOL, so the
        /// familiar `a > 0 AND a < 10` stays accepted.
        apply_when_and_of_comparison_results_then_ok,
        "
PROGRAM main
VAR
    d : DINT;
    c : BOOL;
END_VAR
    c := d > 0 AND d < 10;
END_PROGRAM"
    );

    rule_ctx_ok!(
        /// Relational operators are declared ANY_ELEMENTARY and this rule
        /// deliberately does not hold them to it, so an integer comparison
        /// stays accepted.
        apply_when_relational_of_integer_variables_then_ok,
        "
PROGRAM main
VAR
    d1 : DINT;
    d2 : DINT;
    c : BOOL;
END_VAR
    c := d1 > d2;
END_PROGRAM"
    );

    rule_ctx_errn!(
        /// Without this the operands are lowered to the *logical* opcode and
        /// `d1 AND d2` silently computes a truthiness: 10 AND 3 yields 1
        /// rather than 2.
        apply_when_and_of_integer_variables_then_p4049_per_operand,
        "
PROGRAM main
VAR
    d1 : DINT;
    d2 : DINT;
    d3 : DINT;
END_VAR
    d3 := d1 AND d2;
END_PROGRAM",
        2,
        Problem::OperatorOperandTypeMismatch
    );

    rule_ctx_errn!(
        apply_when_or_of_integer_variables_then_p4049_per_operand,
        "
PROGRAM main
VAR
    d1 : DINT;
    d2 : DINT;
    d3 : DINT;
END_VAR
    d3 := d1 OR d2;
END_PROGRAM",
        2,
        Problem::OperatorOperandTypeMismatch
    );

    rule_ctx_errn!(
        apply_when_xor_of_integer_variables_then_p4049_per_operand,
        "
PROGRAM main
VAR
    d1 : DINT;
    d2 : DINT;
    d3 : DINT;
END_VAR
    d3 := d1 XOR d2;
END_PROGRAM",
        2,
        Problem::OperatorOperandTypeMismatch
    );

    rule_ctx_err1!(
        /// `NOT` over an integer reached `BOOL_NOT`, so `NOT d1` with
        /// `d1 := 10` yielded 0 rather than -11.
        apply_when_not_of_integer_variable_then_p4049,
        "
PROGRAM main
VAR
    d1 : DINT;
    d2 : DINT;
END_VAR
    d2 := NOT d1;
END_PROGRAM",
        Problem::OperatorOperandTypeMismatch
    );

    rule_ctx_err1!(
        apply_when_and_of_real_variable_then_p4049,
        "
PROGRAM main
VAR
    r : REAL;
    b : BOOL;
    c : BOOL;
END_VAR
    c := b AND r > 1.0 AND r;
END_PROGRAM",
        Problem::OperatorOperandTypeMismatch
    );
}
