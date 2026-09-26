//! Semantic rule that a declared STRING or WSTRING length fits its slot.
//!
//! A string slot records its capacity in a `u16` (ADR-0035), so 65,535 code
//! units is the most any string can be declared to hold. A larger length has
//! no representation: left unchecked it wrapped in codegen, so `STRING[70000]`
//! became a `STRING[4464]` with nothing said about it, and a value the program
//! wrote to fit was cut. The parser accepts any integer as a length, which
//! is right for it -- what fits is a property of the runtime's layout -- so
//! this rule holds the length to that layout.
//!
//! A length is declared in three places: a variable or structure field
//! (`StringInitializer`), an array element or function return
//! (`StringSpecification`), and a `TYPE` declaration (`StringDeclaration`).
//! A length written as a named constant has been folded to a literal before
//! rules run, so the literal form is the only one to check.
//!
//! See section 2.3.3.
//!
//! ## Passes
//!
//! ```ignore
//! PROGRAM main
//!    VAR
//!       s : STRING[65535];
//!    END_VAR
//! END_PROGRAM
//! ```
//!
//! ## Fails
//!
//! ```ignore
//! PROGRAM main
//!    VAR
//!       s : STRING[70000];   (* more than a slot can hold *)
//!    END_VAR
//! END_PROGRAM
//! ```
use ironplc_dsl::{
    common::{
        Integer, IntegerRef, Library, StringDeclaration, StringInitializer, StringSpecification,
    },
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

/// The most code units a string can be declared to hold: the string header
/// stores the capacity as a `u16` (ADR-0035).
const MAX_STRING_LENGTH: u128 = u16::MAX as u128;

pub fn apply(
    lib: &Library,
    _context: &SemanticContext,
    _options: &CompilerOptions,
) -> SemanticResult {
    run_rule(
        RuleStringLengthRange {
            diagnostics: Vec::new(),
        },
        lib,
    )
}

struct RuleStringLengthRange {
    diagnostics: Vec<Diagnostic>,
}

impl DiagnosticVisitor for RuleStringLengthRange {
    fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

impl RuleStringLengthRange {
    /// Reports `length` if it is a literal the header cannot record.
    fn check(&mut self, length: &IntegerRef) {
        if let IntegerRef::Literal(literal) = length {
            if literal.value > MAX_STRING_LENGTH {
                self.diagnostics.push(out_of_range(literal));
            }
        }
    }
}

/// The diagnostic for a declared length above what a string slot can hold.
fn out_of_range(length: &Integer) -> Diagnostic {
    Diagnostic::problem(
        Problem::StringLengthOutOfRange,
        Label::span(
            length.span.clone(),
            format!("Length {} is more than a string can hold", length.value),
        ),
    )
    .with_context("length", &length.value.to_string())
    .with_context("maximum", &MAX_STRING_LENGTH.to_string())
}

impl Visitor<Infallible> for RuleStringLengthRange {
    type Value = ();

    fn visit_string_initializer(&mut self, node: &StringInitializer) -> Result<(), Infallible> {
        if let Some(length) = &node.length {
            self.check(length);
        }
        Ok(())
    }

    fn visit_string_specification(&mut self, node: &StringSpecification) -> Result<(), Infallible> {
        if let Some(length) = &node.length {
            self.check(length);
        }
        Ok(())
    }

    fn visit_string_declaration(&mut self, node: &StringDeclaration) -> Result<(), Infallible> {
        self.check(&node.length);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use ironplc_parser::options::CompilerOptions;
    use ironplc_problems::Problem;

    rule_ok!(
        apply_when_length_is_the_maximum_then_ok,
        "
PROGRAM main
VAR
    s : STRING[65535];
    w : WSTRING[65535];
END_VAR
END_PROGRAM"
    );

    rule_ok!(
        apply_when_length_is_omitted_then_ok,
        "
PROGRAM main
VAR
    s : STRING;
END_VAR
END_PROGRAM"
    );

    rule_err1_at!(
        apply_when_variable_length_exceeds_maximum_then_error,
        "
PROGRAM main
VAR
    s : STRING[70000];
END_VAR
END_PROGRAM",
        Problem::StringLengthOutOfRange,
        "70000"
    );

    rule_err1_at!(
        apply_when_wstring_variable_length_exceeds_maximum_then_error,
        "
PROGRAM main
VAR
    w : WSTRING[65536];
END_VAR
END_PROGRAM",
        Problem::StringLengthOutOfRange,
        "65536"
    );

    rule_err1_at!(
        apply_when_array_element_length_exceeds_maximum_then_error,
        "
PROGRAM main
VAR
    names : ARRAY[0..1] OF STRING[70000];
END_VAR
END_PROGRAM",
        Problem::StringLengthOutOfRange,
        "70000"
    );

    rule_err1_at!(
        apply_when_structure_field_length_exceeds_maximum_then_error,
        "
TYPE Recipe : STRUCT
    note : STRING[70000];
END_STRUCT; END_TYPE

PROGRAM main
VAR
    r : Recipe;
END_VAR
END_PROGRAM",
        Problem::StringLengthOutOfRange,
        "70000"
    );

    rule_err1_at!(
        apply_when_function_return_length_exceeds_maximum_then_error,
        "
FUNCTION f : STRING[70000]
VAR_INPUT
    x : INT;
END_VAR
    f := 'a';
END_FUNCTION

PROGRAM main
VAR
    s : STRING;
END_VAR
    s := f(1);
END_PROGRAM",
        Problem::StringLengthOutOfRange,
        "70000"
    );

    rule_err1_at!(
        apply_when_type_declaration_length_exceeds_maximum_then_error,
        "
TYPE Long : STRING[70000]; END_TYPE

PROGRAM main
VAR
    s : Long;
END_VAR
END_PROGRAM",
        Problem::StringLengthOutOfRange,
        "70000"
    );

    rule_err1_with!(
        apply_when_parenthesized_length_exceeds_maximum_then_error,
        CompilerOptions {
            allow_paren_string_length: true,
            ..Default::default()
        },
        "
PROGRAM main
VAR
    s : STRING(70000);
END_VAR
END_PROGRAM",
        Problem::StringLengthOutOfRange
    );

    rule_errn!(
        apply_when_two_lengths_exceed_maximum_then_two_errors,
        "
PROGRAM main
VAR
    s : STRING[70000];
    w : WSTRING[70000];
END_VAR
END_PROGRAM",
        2,
        Problem::StringLengthOutOfRange
    );
}
