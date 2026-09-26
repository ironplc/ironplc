//! Semantic rule that gates an explicit per-member value in an enumeration
//! declaration (e.g. `TYPE E_Mode : (Deutsch := 1, English := 2); END_TYPE`)
//! behind `--allow-enum-explicit-values`.
//!
//! The IEC 61131-3:2003 (Edition 2) grammar for an enumerated specification
//! (Annex B) is
//! `enumerated_specification ::= '(' enumerated_value {',' enumerated_value}
//! ')' | enumerated_type_name`, where an `enumerated_value` is a name and
//! nothing else. Assigning a member its own integer value is an addition of
//! IEC 61131-3:2013 (Edition 3) and an extension the vendor dialects accept;
//! which dialects enable it is defined by the dialect mapping in
//! `options.rs`, not restated here.
//!
//! Per ADR-0040 the grammar recognizes the superset unconditionally: `:=` is
//! the same token in a dozen unrelated positions, so there is no
//! distinguishing token to demote or reject pre-parse, and the illegality is
//! visible only in the parsed structure. The parser records the value the
//! user wrote in `EnumeratedValue::explicit_value`, and this rule is what
//! enforces the flag.
//!
//! Only the enum *declaration* grammar path sets `explicit_value`; every
//! other position an `EnumeratedValue` appears in -- a default
//! (`(A, B) := A`), a `CASE` label, an expression operand -- leaves it
//! `None`, so a plain visit over every `EnumeratedValue` cannot flag one of
//! those by mistake.
//!
//! ## Fails (without the flag)
//!
//! ```ignore
//! TYPE
//!     E_ModeLanguage : (Deutsch := 1, English := 2);
//! END_TYPE
//! ```
use ironplc_dsl::{
    common::EnumeratedValue,
    core::Located,
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
    lib: &ironplc_dsl::common::Library,
    _context: &SemanticContext,
    options: &CompilerOptions,
) -> SemanticResult {
    if options.allow_enum_explicit_values {
        return Ok(());
    }

    run_rule(
        RuleEnumExplicitValue {
            diagnostics: Vec::new(),
        },
        lib,
    )
}

struct RuleEnumExplicitValue {
    diagnostics: Vec<Diagnostic>,
}

impl DiagnosticVisitor for RuleEnumExplicitValue {
    fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

impl Visitor<Infallible> for RuleEnumExplicitValue {
    type Value = ();

    fn visit_enumerated_value(
        &mut self,
        node: &EnumeratedValue,
    ) -> Result<Self::Value, Infallible> {
        if node.explicit_value.is_some() {
            self.diagnostics.push(Diagnostic::problem(
                Problem::EnumExplicitValueNotAllowed,
                Label::span(node.span(), "Enumeration member with an explicit value"),
            ));
        }
        node.recurse_visit(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts_flag() -> CompilerOptions {
        CompilerOptions {
            allow_enum_explicit_values: true,
            ..CompilerOptions::default()
        }
    }

    const SOURCE: &str = "
TYPE
E_ModeLanguage : (Deutsch := 1, English := 2);
END_TYPE";

    rule_errn!(
        apply_when_enum_explicit_values_and_flag_disabled_then_error,
        SOURCE,
        2,
        Problem::EnumExplicitValueNotAllowed
    );

    rule_ok_with!(
        apply_when_enum_explicit_values_and_flag_enabled_then_ok,
        opts_flag(),
        SOURCE
    );

    // The label has to name the member that carries the value, or the user
    // cannot tell which one to remove in a long declaration.
    rule_err1_at!(
        apply_when_one_member_has_explicit_value_then_label_names_that_member,
        "
TYPE
E_AssertionType : (Type_UNDEFINED := 0, Type_ANY, Type_BOOL);
END_TYPE",
        Problem::EnumExplicitValueNotAllowed,
        "Type_UNDEFINED"
    );

    // A declaration whose members are all bare names is Edition 2 syntax and
    // is never flagged, whatever the flag says.
    rule_ok!(
        apply_when_enum_has_no_explicit_values_then_never_flagged,
        "
TYPE
COLOR : (RED, GREEN, BLUE);
END_TYPE"
    );

    // A default value (`:= RED`) is standard Edition 2 syntax that puts an
    // `EnumeratedValue` on the right of a `:=`. It carries no
    // `explicit_value`, so it must not be mistaken for the extension.
    rule_ok!(
        apply_when_enum_has_default_value_then_never_flagged,
        "
TYPE
COLOR : (RED, GREEN, BLUE) := GREEN;
END_TYPE"
    );

    // An enumerated value used as a `CASE` label or read in an expression is
    // a reference, not a declaration, and is never flagged.
    rule_ok!(
        apply_when_enum_value_referenced_then_never_flagged,
        "
TYPE
COLOR : (RED, GREEN, BLUE) := RED;
END_TYPE
PROGRAM main
VAR
    c : COLOR;
    result : DINT;
END_VAR
    c := GREEN;
    CASE c OF
    RED: result := 10;
    GREEN: result := 20;
    END_CASE;
END_PROGRAM"
    );
}
