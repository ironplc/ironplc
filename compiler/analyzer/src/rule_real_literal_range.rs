//! Semantic rule that a real literal names a value its type can represent.
//!
//! Parsing a real literal saturates: text beyond the largest finite value of
//! the type becomes infinity rather than an error. So `1.0E400` compiled to an
//! `LREAL` holding `inf`, and `REAL#1.0E40` to a `REAL` holding `inf`, with
//! nothing said about it (issue #1784). Neither is the number the program
//! wrote.
//!
//! An untyped literal is held to the `LREAL` range, since that is how it is
//! parsed. A `REAL#` literal states its own type, as `INT#40000` does, and is
//! held to the `REAL` range wherever it is written.
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

impl Visitor<Infallible> for RuleRealLiteralRange {
    type Value = ();

    fn visit_real_literal(&mut self, node: &RealLiteral) -> Result<(), Infallible> {
        let (type_name, is_finite) = literal_range(node);
        if !is_finite {
            self.diagnostics.push(
                Diagnostic::problem(
                    Problem::RealLiteralOutOfRange,
                    Label::span(
                        node.span.clone(),
                        format!("Value is outside the range of {type_name}"),
                    ),
                )
                .with_context("type", &type_name.to_string()),
            );
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
