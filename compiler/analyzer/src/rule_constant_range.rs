//! Semantic rule that a constant fits the type it is stored into.
//!
//! A declared type states the values a variable can hold. `USINT` holds 0
//! through 255, so `300` is not a value it can take, and storing one there is
//! a mistake rather than a request for the 44 that two's-complement
//! truncation would leave behind. Nothing in the source says the value
//! changes, so the compiler says it instead.
//!
//! The type is pushed down through operators to the literals beneath them,
//! which is how the backend compiles them: one operation type covers both
//! operands, so `b := 300 + 0` stores the same wrapped value as `b := 300`
//! and is diagnosed the same way.
//!
//! Bit string *types* are deliberately not checked. `BYTE` and `WORD` are
//! patterns rather than magnitudes, and wrapping one is a legitimate thing
//! for a program to want.
//!
//! How a literal was spelled makes no difference: `16#1FF` is 511 whichever
//! radix it was written in, and 511 is not a `USINT`. The radix does not
//! survive parsing in any case.
//!
//! See section 2.2.1.
//!
//! ## Passes
//!
//! ```ignore
//! PROGRAM main
//!    VAR
//!       count : USINT := 255;
//!       total : SINT;
//!       pattern : BYTE;
//!    END_VAR
//!    total := -128;
//!    pattern := 300;      (* a bit string wraps by design *)
//! END_PROGRAM
//! ```
//!
//! ## Fails
//!
//! ```ignore
//! PROGRAM main
//!    VAR
//!       count : USINT := 300;   (* USINT holds 0..255 *)
//!       total : SINT;
//!    END_VAR
//!    total := 200;               (* SINT holds -128..127 *)
//!    count := 255 + 1;           (* the operator does not widen the type *)
//! END_PROGRAM
//! ```
use ironplc_dsl::{
    common::*,
    core::Located,
    diagnostic::{Diagnostic, Label},
    scope::ScopeNode,
    textual::*,
    visitor::Visitor,
};
use ironplc_parser::options::CompilerOptions;
use ironplc_problems::Problem;
use std::convert::Infallible;

use crate::{
    intermediate_type::{ByteSized, IntermediateType},
    result::SemanticResult,
    rule_support::{run_rule, DiagnosticVisitor},
    semantic_context::SemanticContext,
    type_environment::TypeEnvironment,
    value_range,
    variable_type::{self, Declarations, Declared},
};

pub fn apply(
    lib: &Library,
    context: &SemanticContext,
    _options: &CompilerOptions,
) -> SemanticResult {
    run_rule(
        RuleConstantRange {
            type_environment: context.types(),
            // `Declarations::new` opens the base scope, where declarations
            // made outside any POU land. Opening another here would leave the
            // stack unbalanced when the table drops.
            declarations: Declarations::new(),
            diagnostics: Vec::new(),
        },
        lib,
    )
}

struct RuleConstantRange<'a> {
    type_environment: &'a TypeEnvironment,
    /// The declared type of every variable in scope.
    declarations: Declarations<'a>,
    diagnostics: Vec<Diagnostic>,
}

impl DiagnosticVisitor for RuleConstantRange<'_> {
    fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

/// The value of an integer literal, or `None` when it is too large to be one.
///
/// A literal beyond `i128` cannot be stored in any IEC 61131-3 type, so the
/// caller reports it against whatever range it was checked against.
fn literal_value(literal: &IntegerLiteral) -> Option<i128> {
    let magnitude = i128::try_from(literal.value.value.value).ok()?;
    Some(if literal.value.is_neg {
        -magnitude
    } else {
        magnitude
    })
}

impl RuleConstantRange<'_> {
    /// Reports `constant` when the type it is stored into cannot hold it.
    fn check_constant(&mut self, constant: &ConstantKind, expected: &IntermediateType) {
        // Every integer literal arrives here as a value, whatever radix it
        // was written in. A `ConstantKind` that is not one -- a duration, a
        // string -- has no integer range to check.
        let ConstantKind::IntegerLiteral(literal) = constant else {
            return;
        };
        let Some((minimum, maximum)) = value_range::of(expected) else {
            return;
        };

        let value = literal_value(literal);
        if value.is_some_and(|value| value >= minimum && value <= maximum) {
            return;
        }

        // A literal too large for `i128` has no printable value of its own,
        // so it is reported by the magnitude the source spelled.
        let reported = value.map_or_else(
            || format!("-{}", literal.value.value.value),
            |value| value.to_string(),
        );
        self.diagnostics.push(
            Diagnostic::problem(
                Problem::ConstantOverflow,
                Label::span(
                    constant.span(),
                    format!("Value must be in the range {minimum} to {maximum}"),
                ),
            )
            .with_context("value", &reported)
            .with_context("minimum", &minimum.to_string())
            .with_context("maximum", &maximum.to_string()),
        );
    }

    /// Pushes `expected` down to the literals within `expr`.
    ///
    /// The walk follows the operators the backend compiles at one operation
    /// type, and stops at anything that introduces a type of its own: a
    /// function's arguments are its parameters' business, and a variable
    /// carries its own declaration.
    ///
    /// A negated literal needs no handling here. Constant folding turns
    /// `-200` into one signed literal before any rule runs, so a `Neg` that
    /// survives has an operand this walk would descend into anyway.
    fn check_expr(&mut self, expr: &Expr, expected: &IntermediateType) {
        match &expr.kind {
            ExprKind::Const(constant) => self.check_constant(constant, expected),
            ExprKind::BinaryOp(binary) => {
                self.check_expr(&binary.left, expected);
                self.check_expr(&binary.right, expected);
            }
            ExprKind::UnaryOp(unary) => self.check_expr(&unary.term, expected),
            ExprKind::Expression(inner) => self.check_expr(inner, expected),
            _ => {}
        }
    }

    /// The type an expression resolves to, when the analyzer gave it one.
    ///
    /// A bare literal resolves to a generic type (`ANY_INT`), which is not in
    /// the type environment, so it answers `None` rather than a range of its
    /// own.
    fn expr_type(&self, expr: &Expr) -> Option<IntermediateType> {
        let resolved = expr.resolved_type.as_ref()?;
        Some(self.type_environment.get(resolved)?.representation.clone())
    }

    /// The type an assignment writes through `target`.
    ///
    /// This is the value written, not the variable selected from: `x.3 := v`
    /// writes a `BOOL` and `w.%B1 := v` writes a byte, whatever `x` and `w`
    /// are declared as.
    fn assignment_target_type(&self, target: &Variable) -> Option<IntermediateType> {
        let Variable::Symbolic(kind) = target else {
            // A directly represented variable (`%IW0`) has no declaration to
            // take a range from.
            return None;
        };
        match kind {
            SymbolicVariableKind::BitAccess(_) => Some(IntermediateType::Bool),
            SymbolicVariableKind::PartialAccess(partial) => Some(IntermediateType::Bytes {
                size: match partial.size {
                    PartialAccessSize::Byte => ByteSized::B8,
                    PartialAccessSize::Word => ByteSized::B16,
                    PartialAccessSize::DWord => ByteSized::B32,
                    PartialAccessSize::LWord => ByteSized::B64,
                },
            }),
            _ => variable_type::of(kind, &self.declarations, self.type_environment),
        }
    }

    /// Checks a comparison's literals against the type of the other side.
    ///
    /// `IF c = 200` compares at `c`'s type, so a literal that `c` can never
    /// hold makes the comparison unsatisfiable rather than false.
    fn check_compare(&mut self, compare: &CompareExpr) {
        if let Some(left) = self.expr_type(&compare.left) {
            self.check_expr(&compare.right, &left);
        }
        if let Some(right) = self.expr_type(&compare.right) {
            self.check_expr(&compare.left, &right);
        }
    }

    /// Checks a `CASE` label against the selector's type.
    ///
    /// A label the selector can never equal selects a group that can never
    /// run.
    fn check_case(&mut self, node: &Case) {
        let Some(selector) = self.expr_type(&node.selector) else {
            return;
        };
        let Some((minimum, maximum)) = value_range::of(&selector) else {
            return;
        };

        let labels: Vec<&SignedInteger> = node
            .statement_groups
            .iter()
            .flat_map(|group| group.selectors.iter())
            .filter_map(|selection| match selection {
                CaseSelectionKind::SignedInteger(value) => Some(value),
                // A subrange label's bounds are checked against the base type
                // by `rule_decl_subrange_limits`, and a bit-string label is a
                // pattern.
                _ => None,
            })
            .collect();

        for label in labels {
            let value = match i128::try_from(label.value.value) {
                Ok(magnitude) if label.is_neg => -magnitude,
                Ok(magnitude) => magnitude,
                Err(_) => continue,
            };
            if value < minimum || value > maximum {
                self.diagnostics.push(
                    Diagnostic::problem(
                        Problem::ConstantOverflow,
                        Label::span(
                            label.value.span(),
                            format!("Value must be in the range {minimum} to {maximum}"),
                        ),
                    )
                    .with_context("value", &value.to_string())
                    .with_context("minimum", &minimum.to_string())
                    .with_context("maximum", &maximum.to_string()),
                );
            }
        }
    }
}

impl Visitor<Infallible> for RuleConstantRange<'_> {
    type Value = ();

    /// Opens a declaration's scope.
    ///
    /// Every kind contributes the same thing -- a frame its own declarations
    /// go into -- but the match stays exhaustive so that a new kind of scope
    /// has to say so rather than silently sharing the enclosing
    /// declaration's frame.
    fn enter_scope(&mut self, node: ScopeNode<'_>) -> Result<(), Infallible> {
        match node {
            ScopeNode::Function(_)
            | ScopeNode::FunctionBlock(_)
            | ScopeNode::Program(_)
            | ScopeNode::Method(_) => self.declarations.enter(),
        }
        Ok(())
    }

    fn exit_scope(&mut self) {
        self.declarations.exit();
    }

    fn visit_var_decl(&mut self, node: &VarDecl) -> Result<(), Infallible> {
        self.declarations.add_if(
            node.identifier.symbolic_id(),
            Declared(node.initializer.clone()),
        );

        if let InitialValueAssignmentKind::Simple(simple) = &node.initializer {
            if let Some(constant) = &simple.initial_value {
                if let Some(attributes) = self.type_environment.get(&simple.type_name) {
                    let declared = attributes.representation.clone();
                    self.check_constant(constant, &declared);
                }
            }
        }

        node.recurse_visit(self)
    }

    fn visit_assignment(&mut self, node: &Assignment) -> Result<(), Infallible> {
        // A write through a reference stores into whatever the reference
        // points at, which this rule cannot see.
        if !node.deref {
            if let Some(target) = self.assignment_target_type(&node.target) {
                self.check_expr(&node.value, &target);
            }
        }
        node.recurse_visit(self)
    }

    fn visit_compare_expr(&mut self, node: &CompareExpr) -> Result<(), Infallible> {
        self.check_compare(node);
        node.recurse_visit(self)
    }

    fn visit_case(&mut self, node: &Case) -> Result<(), Infallible> {
        self.check_case(node);
        node.recurse_visit(self)
    }
}

#[cfg(test)]
mod tests {
    use crate::stages::analyze;
    use ironplc_dsl::core::FileId;
    use ironplc_parser::{options::CompilerOptions, parse_program};
    use ironplc_problems::Problem;
    use rstest::rstest;

    /// Analyzes `program`, returning how many out-of-range constants it
    /// reported. Naming the problem keeps a diagnostic from another rule
    /// from passing for one of ours.
    fn out_of_range_count(program: &str) -> usize {
        let options = CompilerOptions::default();
        let library = parse_program(program, &FileId::default(), &options).unwrap();
        let (_library, context) = analyze(&[&library], &options).unwrap();
        context
            .diagnostics()
            .iter()
            .filter(|d| d.code == Problem::ConstantOverflow.code())
            .count()
    }

    fn program_with(declarations: &str, body: &str) -> String {
        format!("PROGRAM main\nVAR\n{declarations}END_VAR\n{body}END_PROGRAM\n")
    }

    // --- Every integer type's boundaries ---
    //
    // For each type: the extremes it can hold are accepted, and one step
    // beyond either is reported.

    #[rstest]
    #[case::sint_low("SINT", "-128", true)]
    #[case::sint_high("SINT", "127", true)]
    #[case::sint_below("SINT", "-129", false)]
    #[case::sint_above("SINT", "128", false)]
    #[case::int_high("INT", "32767", true)]
    #[case::int_above("INT", "32768", false)]
    #[case::dint_high("DINT", "2147483647", true)]
    #[case::dint_above("DINT", "2147483648", false)]
    #[case::lint_high("LINT", "9223372036854775807", true)]
    #[case::lint_above("LINT", "9223372036854775808", false)]
    #[case::usint_low("USINT", "0", true)]
    #[case::usint_high("USINT", "255", true)]
    #[case::usint_below("USINT", "-1", false)]
    #[case::usint_above("USINT", "256", false)]
    #[case::uint_above("UINT", "65536", false)]
    #[case::udint_above("UDINT", "4294967296", false)]
    #[case::ulint_high("ULINT", "18446744073709551615", true)]
    #[case::ulint_above("ULINT", "18446744073709551616", false)]
    fn apply_when_initializer_at_boundary_then_ok_or_err(
        #[case] declared_type: &str,
        #[case] value: &str,
        #[case] expected_ok: bool,
    ) {
        let program = program_with(&format!("x : {declared_type} := {value};\n"), "");

        assert_eq!(out_of_range_count(&program) == 0, expected_ok);
    }

    // --- The contexts a constant is checked in ---

    #[test]
    fn apply_when_assignment_out_of_range_then_err() {
        let codes = out_of_range_count(&program_with("x : USINT;\n", "x := 300;\n"));

        assert_eq!(codes, 1);
    }

    /// The operator does not widen the type, so a folded constant is checked
    /// exactly as a written one is.
    #[test]
    fn apply_when_folded_operand_out_of_range_then_err() {
        let codes = out_of_range_count(&program_with("x : USINT;\n", "x := 255 + 1;\n"));

        assert_eq!(codes, 1);
    }

    /// A comparison happens at the variable's type, so a literal it can never
    /// equal is a mistake rather than a false condition.
    #[test]
    fn apply_when_comparison_constant_out_of_range_then_err() {
        let codes = out_of_range_count(&program_with(
            "x : SINT;\ny : DINT;\n",
            "IF x = 200 THEN y := 0; END_IF;\n",
        ));

        assert_eq!(codes, 1);
    }

    /// A `CASE` label the selector can never equal selects a group that can
    /// never run.
    #[test]
    fn apply_when_case_label_out_of_range_then_err() {
        let codes = out_of_range_count(&program_with(
            "x : SINT;\ny : DINT;\n",
            "CASE x OF\n200: y := 1;\nEND_CASE;\n",
        ));

        assert_eq!(codes, 1);
    }

    #[test]
    fn apply_when_struct_field_out_of_range_then_err() {
        let codes = out_of_range_count(
            "TYPE
Counts : STRUCT
    small : USINT;
END_STRUCT;
END_TYPE

PROGRAM main
VAR
    counts : Counts;
END_VAR
    counts.small := 300;
END_PROGRAM",
        );

        assert_eq!(codes, 1);
    }

    #[test]
    fn apply_when_array_element_out_of_range_then_err() {
        let codes = out_of_range_count(&program_with(
            "readings : ARRAY[1..2] OF USINT;\ni : DINT;\n",
            "readings[i] := 300;\n",
        ));

        assert_eq!(codes, 1);
    }

    #[test]
    fn apply_when_global_out_of_range_then_err() {
        let codes = out_of_range_count(
            "PROGRAM main
    g := 300;
END_PROGRAM

CONFIGURATION config
VAR_GLOBAL
    g : USINT;
END_VAR
RESOURCE res ON PLC
    TASK plc_task(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM inst WITH plc_task : main;
END_RESOURCE
END_CONFIGURATION",
        );

        assert_eq!(codes, 1);
    }

    // --- A subrange states its own range ---

    #[test]
    fn apply_when_subrange_initializer_out_of_range_then_err() {
        let codes = out_of_range_count(
            "TYPE
Ratio : INT(0..10);
END_TYPE

PROGRAM main
VAR
    r : Ratio := 20;
END_VAR
END_PROGRAM",
        );

        assert_eq!(codes, 1);
    }

    #[test]
    fn apply_when_subrange_initializer_in_range_then_ok() {
        let codes = out_of_range_count(
            "TYPE
Ratio : INT(0..10);
END_TYPE

PROGRAM main
VAR
    r : Ratio := 10;
END_VAR
END_PROGRAM",
        );

        assert_eq!(codes, 0);
    }

    // --- What is deliberately not checked ---
    //
    // A bit string is a pattern rather than a magnitude, so wrapping one is
    // a legitimate thing to want. The type decides that, not how the literal
    // was spelled.

    #[rstest]
    #[case::byte("BYTE", "300")]
    #[case::word("WORD", "70000")]
    #[case::dword("DWORD", "5000000000")]
    fn apply_when_bit_string_overflows_then_ok(#[case] declared_type: &str, #[case] value: &str) {
        let program = program_with(
            &format!("x : {declared_type};\n"),
            &format!("x := {value};\n"),
        );

        assert_eq!(out_of_range_count(&program), 0);
    }

    /// A radix does not change a value: `16#1FF` is 511, which no `USINT`
    /// can hold.
    #[test]
    fn apply_when_radix_literal_out_of_range_then_err() {
        let codes = out_of_range_count(&program_with("x : USINT;\n", "x := 16#1FF;\n"));

        assert_eq!(codes, 1);
    }

    /// The same literal against a type that can hold it stays silent, so the
    /// check is about the value rather than the spelling.
    #[test]
    fn apply_when_radix_literal_in_range_then_ok() {
        let codes = out_of_range_count(&program_with("x : UINT;\n", "x := 16#1FF;\n"));

        assert_eq!(codes, 0);
    }

    #[rstest]
    #[case::real("REAL", "3.4")]
    #[case::time("TIME", "T#1s")]
    fn apply_when_not_integer_storage_then_ok(#[case] declared_type: &str, #[case] value: &str) {
        let program = program_with(&format!("x : {declared_type} := {value};\n"), "");

        assert_eq!(out_of_range_count(&program), 0);
    }
}
