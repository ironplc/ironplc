//! Semantic rule that an array initializer written after a type name,
//! `x : A3 := [1, 2, 3]`, names a declared array type.
//!
//! Only an array takes a bracketed initializer, so the parser records such a
//! declaration as an array of the named type before the type is known
//! (ADR-0050). This rule checks that guess against the declared types:
//!
//! - a name that is not declared is reported as an undeclared type, as for a
//!   bare declaration `x : NOPE`;
//! - a name that is declared as something other than an array (a structure,
//!   an enumeration, an alias of an elementary type) is reported as an
//!   initializer that does not fit the declared type.
//!
//! The rule checks a variable declaration, a structure field, a `VAR_CONFIG`
//! entry and the default of an array type alias (`TYPE B3 : A3 := [...]`).
//! An alias whose base is not declared is reported when the type environment
//! is built, so this rule does not report it again.
//!
//! See section 2.4.3.2.
//!
//! ## Passes
//!
//! ```ignore
//! TYPE
//!    A3 : ARRAY[1..3] OF DINT;
//! END_TYPE
//!
//! PROGRAM main
//!    VAR
//!       x : A3 := [1, 2, 3];
//!    END_VAR
//! END_PROGRAM
//! ```
//!
//! ## Fails
//!
//! ```ignore
//! TYPE
//!    S : STRUCT a : DINT; END_STRUCT;
//! END_TYPE
//!
//! PROGRAM main
//!    VAR
//!       x : S := [1, 2];
//!    END_VAR
//! END_PROGRAM
//! ```
use ironplc_dsl::{
    common::*,
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
    type_environment::TypeEnvironment,
};

pub fn apply(
    lib: &Library,
    context: &SemanticContext,
    _options: &CompilerOptions,
) -> SemanticResult {
    run_rule(
        RuleArrayInitializerType {
            types: context.types(),
            diagnostics: Vec::new(),
        },
        lib,
    )
}

struct RuleArrayInitializerType<'a> {
    types: &'a TypeEnvironment,
    diagnostics: Vec<Diagnostic>,
}

impl DiagnosticVisitor for RuleArrayInitializerType<'_> {
    fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

impl RuleArrayInitializerType<'_> {
    /// Reports `name` when it is declared as a type that is not an array.
    /// Returns whether `name` is declared at all.
    fn check_declared_type_is_array(&mut self, name: &TypeName) -> bool {
        let Some(attributes) = self.types.get(name) else {
            return false;
        };
        if !attributes.representation.is_array() {
            self.diagnostics.push(
                Diagnostic::problem(
                    Problem::InitializerTypeMismatch,
                    Label::span(name.span(), "Type is not an array"),
                )
                .with_context_type("type", name),
            );
        }
        true
    }
}

impl Visitor<Infallible> for RuleArrayInitializerType<'_> {
    type Value = ();

    fn visit_array_initial_value_assignment(
        &mut self,
        node: &ArrayInitialValueAssignment,
    ) -> Result<(), Infallible> {
        // A bare `x : A3` reaches here only once the resolver has found `A3`
        // to be an array, so only a declaration with values needs checking.
        if let SpecificationKind::Named(name) = &node.spec {
            if !node.initial_values.is_empty() && !self.check_declared_type_is_array(name) {
                self.diagnostics.push(
                    Diagnostic::problem(
                        Problem::UndeclaredUnknownType,
                        Label::span(name.span(), "Variable type"),
                    )
                    .with_context_type("identifier", name),
                );
            }
        }
        node.recurse_visit(self)
    }

    fn visit_array_declaration(&mut self, node: &ArrayDeclaration) -> Result<(), Infallible> {
        if let SpecificationKind::Named(base) = &node.spec {
            if !node.init.is_empty() {
                self.check_declared_type_is_array(base);
            }
        }
        node.recurse_visit(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    rule_ok!(
        apply_when_named_array_with_values_then_ok,
        "
TYPE
    A3 : ARRAY[1..3] OF DINT;
END_TYPE
PROGRAM main
VAR
    x : A3 := [1, 2, 3];
    y : A3;
END_VAR
END_PROGRAM"
    );

    rule_ok!(
        apply_when_alias_of_named_array_with_values_then_ok,
        "
TYPE
    A3 : ARRAY[1..3] OF DINT;
    B3 : A3;
    C3 : A3 := [7, 8, 9];
END_TYPE
PROGRAM main
VAR
    x : B3 := [1, 2, 3];
    y : C3 := [4, 5, 6];
END_VAR
END_PROGRAM"
    );

    rule_err_at!(
        apply_when_variable_of_undeclared_type_with_values_then_undeclared_type,
        "
PROGRAM main
VAR
    x : NOPE := [1, 2];
END_VAR
END_PROGRAM",
        Problem::UndeclaredUnknownType,
        "NOPE"
    );

    // The program comes first so that the type name's first occurrence is
    // the one the label points at.
    rule_err_at!(
        apply_when_variable_of_structure_with_values_then_mismatch,
        "
PROGRAM main
VAR
    x : POINT := [1, 2];
END_VAR
END_PROGRAM
TYPE
    POINT : STRUCT a : DINT; b : DINT; END_STRUCT;
END_TYPE",
        Problem::InitializerTypeMismatch,
        "POINT"
    );

    rule_err!(
        apply_when_variable_of_elementary_alias_with_values_then_mismatch,
        "
TYPE
    MYINT : DINT;
END_TYPE
PROGRAM main
VAR
    x : MYINT := [1, 2];
END_VAR
END_PROGRAM",
        [Problem::InitializerTypeMismatch]
    );

    rule_err!(
        apply_when_variable_of_enumeration_with_values_then_mismatch,
        "
TYPE
    COLOR : (Red, Green);
END_TYPE
PROGRAM main
VAR
    x : COLOR := [Red, Green];
END_VAR
END_PROGRAM",
        [Problem::InitializerTypeMismatch]
    );

    rule_err!(
        apply_when_variable_of_function_block_with_values_then_mismatch,
        "
PROGRAM main
VAR
    x : TON := [1];
END_VAR
END_PROGRAM",
        [Problem::InitializerTypeMismatch]
    );

    rule_err!(
        apply_when_structure_field_of_undeclared_type_with_values_then_undeclared_type,
        "
TYPE
    S : STRUCT f : NOPE := [1, 2]; END_STRUCT;
END_TYPE",
        [Problem::UndeclaredUnknownType]
    );

    rule_err!(
        apply_when_structure_field_of_structure_with_values_then_mismatch,
        "
TYPE
    INNER : STRUCT a : DINT; END_STRUCT;
    S : STRUCT f : INNER := [1, 2]; END_STRUCT;
END_TYPE",
        [Problem::InitializerTypeMismatch]
    );

    rule_err!(
        apply_when_global_of_structure_with_values_then_mismatch,
        "
TYPE
    S : STRUCT a : DINT; END_STRUCT;
END_TYPE
PROGRAM main
END_PROGRAM
CONFIGURATION config
    VAR_GLOBAL g : S := [1]; END_VAR
    RESOURCE res ON PLC
        TASK t(INTERVAL := T#100ms, PRIORITY := 1);
        PROGRAM p WITH t : main;
    END_RESOURCE
END_CONFIGURATION",
        [Problem::InitializerTypeMismatch]
    );

    rule_err!(
        apply_when_type_alias_of_structure_with_values_then_mismatch,
        "
TYPE
    S : STRUCT a : DINT; END_STRUCT;
    B : S := [1, 2];
END_TYPE",
        [Problem::InitializerTypeMismatch]
    );

    rule_ok!(
        apply_when_type_alias_of_undeclared_type_with_values_then_left_to_type_environment,
        "
TYPE
    B : NOPE := [1, 2];
END_TYPE"
    );
}
