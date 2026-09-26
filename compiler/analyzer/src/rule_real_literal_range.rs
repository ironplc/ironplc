//! Semantic rule that a real literal names a value its type can represent.
//!
//! Parsing a real literal saturates: text beyond the largest finite value of
//! the type becomes infinity rather than an error. So `1.0E400` compiled to an
//! `LREAL` holding `inf`, and `REAL#1.0E40` to a `REAL` holding `inf`, with
//! nothing said about it (issue #1784). Neither is the number the program
//! wrote.
//!
//! A `REAL#` literal states its own type, as `INT#40000` does, and is held to
//! the `REAL` range wherever it is written. An untyped literal takes its type
//! from where it is used. This rule holds it to the widest real type, which
//! is all it can know here; `rule_constant_range` knows the type the literal
//! is stored into and holds it to a `REAL` there, reporting through
//! [`out_of_range`] so that both read as one problem.
//!
//! See section 2.2.1.
//!
//! ## Passes
//!
//! ```ignore
//! PROGRAM main
//!    VAR
//!       big : LREAL := 1.0E308;
//!       small : REAL := REAL#3.4E38;
//!    END_VAR
//! END_PROGRAM
//! ```
//!
//! ## Fails
//!
//! ```ignore
//! PROGRAM main
//!    VAR
//!       big : LREAL := 1.0E400;         (* beyond LREAL *)
//!       small : REAL := REAL#1.0E40;    (* beyond REAL *)
//!    END_VAR
//! END_PROGRAM
//! ```
use ironplc_dsl::{
    common::{Library, RealLiteral, RealTypeName},
    diagnostic::{Diagnostic, Label},
    visitor::Visitor,
};
use ironplc_parser::options::CompilerOptions;
use ironplc_problems::Problem;
use std::convert::Infallible;

use crate::{
    result::SemanticResult,
    rule_support::{run_rule, DiagnosticVisitor},
    semantic_context::SemanticContext,
};

pub fn apply(
    lib: &Library,
    _context: &SemanticContext,
    _options: &CompilerOptions,
) -> SemanticResult {
    run_rule(
        RuleRealLiteralRange {
            diagnostics: Vec::new(),
        },
        lib,
    )
}

struct RuleRealLiteralRange {
    diagnostics: Vec<Diagnostic>,
}

impl DiagnosticVisitor for RuleRealLiteralRange {
    fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

/// The type a literal is held to, and whether its value is finite in it.
fn literal_range(node: &RealLiteral) -> (RealTypeName, bool) {
    match node.data_type {
        Some(RealTypeName::REAL) => (RealTypeName::REAL, (node.value as f32).is_finite()),
        Some(RealTypeName::LREAL) | None => (RealTypeName::LREAL, node.value.is_finite()),
    }
}

/// The diagnostic for `literal` naming a value outside the range of
/// `type_name`.
pub(crate) fn out_of_range(literal: &RealLiteral, type_name: RealTypeName) -> Diagnostic {
    Diagnostic::problem(
        Problem::RealLiteralOutOfRange,
        Label::span(
            literal.span.clone(),
            format!("Value is outside the range of {type_name}"),
        ),
    )
    .with_context("type", &type_name.to_string())
}

impl Visitor<Infallible> for RuleRealLiteralRange {
    type Value = ();

    fn visit_real_literal(&mut self, node: &RealLiteral) -> Result<(), Infallible> {
        let (type_name, is_finite) = literal_range(node);
        if !is_finite {
            self.diagnostics.push(out_of_range(node, type_name));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use ironplc_problems::Problem;

    rule_ok!(
        apply_when_lreal_literal_is_largest_finite_then_ok,
        "
PROGRAM main
VAR
    b : LREAL := 1.7976931348623157E308;
END_VAR
END_PROGRAM"
    );

    rule_err1_at!(
        apply_when_untyped_literal_exceeds_lreal_then_error,
        "
PROGRAM main
VAR
    b : LREAL;
END_VAR
    b := 1.0E400;
END_PROGRAM",
        Problem::RealLiteralOutOfRange,
        "1.0E400"
    );

    rule_err1_at!(
        apply_when_negative_literal_exceeds_lreal_then_error,
        "
PROGRAM main
VAR
    b : LREAL := -1.0E400;
END_VAR
END_PROGRAM",
        Problem::RealLiteralOutOfRange,
        "1.0E400"
    );

    rule_err1_at!(
        apply_when_lreal_prefixed_literal_exceeds_lreal_then_error,
        "
PROGRAM main
VAR
    b : LREAL := LREAL#1.8E308;
END_VAR
END_PROGRAM",
        Problem::RealLiteralOutOfRange,
        "LREAL#1.8E308"
    );

    rule_ok!(
        apply_when_real_prefixed_literal_is_within_real_then_ok,
        "
PROGRAM main
VAR
    r : REAL := REAL#3.4E38;
END_VAR
END_PROGRAM"
    );

    rule_err1_at!(
        apply_when_real_prefixed_literal_exceeds_real_then_error,
        "
PROGRAM main
VAR
    r : REAL := REAL#1.0E40;
END_VAR
END_PROGRAM",
        Problem::RealLiteralOutOfRange,
        "REAL#1.0E40"
    );

    rule_ok!(
        apply_when_untyped_literal_exceeds_real_but_not_lreal_then_ok,
        "
PROGRAM main
VAR
    b : LREAL := 1.0E40;
END_VAR
END_PROGRAM"
    );
}
