//! Semantic rule that only a function block or a program declares an
//! `R_EDGE` or `F_EDGE` input.
//!
//! IEC 61131-3 detects an edge by comparing an input with the value it had
//! at the previous call (see 2.5.2.2), so the input needs memory that lasts
//! from one call to the next. A function block instance and a program keep
//! their variables between calls; a function and a method do not, so an
//! edge input of either has no previous value to compare with. This rule
//! reports each edge input a function, a method or a property accessor
//! declares.
//!
//! ## Passes
//!
//! ```ignore
//! FUNCTION_BLOCK Counter
//! VAR_INPUT
//!     pulse : BOOL R_EDGE;
//! END_VAR
//! VAR_OUTPUT
//!     count : INT;
//! END_VAR
//!     IF pulse THEN
//!         count := count + 1;
//!     END_IF;
//! END_FUNCTION_BLOCK
//! ```
//!
//! ## Fails
//!
//! ```ignore
//! FUNCTION Rising : BOOL
//! VAR_INPUT
//!     pulse : BOOL R_EDGE;    (* P4092: a function has no previous call *)
//! END_VAR
//!     Rising := pulse;
//! END_FUNCTION
//! ```
use ironplc_dsl::{
    common::{EdgeVarDecl, FunctionDeclaration, Library},
    core::{Id, Located},
    diagnostic::{Diagnostic, Label},
    oop::MethodDeclaration,
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
        RuleEdgeInputOwner {
            diagnostics: Vec::new(),
        },
        lib,
    )
}

struct RuleEdgeInputOwner {
    diagnostics: Vec<Diagnostic>,
}

impl DiagnosticVisitor for RuleEdgeInputOwner {
    fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

impl RuleEdgeInputOwner {
    /// Reports each of `edge_variables`, declared by the POU named `name`
    /// of the kind `kind`.
    fn report(&mut self, kind: &str, name: &Id, edge_variables: &[EdgeVarDecl]) {
        for edge in edge_variables {
            self.diagnostics.push(
                Diagnostic::problem(
                    Problem::EdgeInputNotAllowed,
                    Label::span(edge.identifier.span(), "Edge input"),
                )
                .with_context(kind, &name.to_string()),
            );
        }
    }
}

impl Visitor<Infallible> for RuleEdgeInputOwner {
    type Value = ();

    fn visit_function_declaration(
        &mut self,
        node: &FunctionDeclaration,
    ) -> Result<Self::Value, Infallible> {
        self.report("function", &node.name, &node.edge_variables);
        node.recurse_visit(self)
    }

    fn visit_method_declaration(
        &mut self,
        node: &MethodDeclaration,
    ) -> Result<Self::Value, Infallible> {
        let kind = match node.accessor {
            Some(_) => "property",
            None => "method",
        };
        self.report(kind, &node.name, &node.edge_variables);
        node.recurse_visit(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{
        diagnostic_codes, edition3_options, fb_inheritance_options, rule_diagnostics,
    };

    rule_ok!(
        apply_when_function_block_has_edge_inputs_then_ok,
        "
FUNCTION_BLOCK Counter
VAR_INPUT
    up : BOOL R_EDGE;
    down : BOOL F_EDGE;
END_VAR
VAR_OUTPUT
    count : INT;
END_VAR
    IF up THEN
        count := count + 1;
    END_IF;
END_FUNCTION_BLOCK"
    );

    rule_err!(
        apply_when_function_has_rising_edge_input_then_p4092,
        "
FUNCTION Rising : BOOL
VAR_INPUT
    pulse : BOOL R_EDGE;
END_VAR
    Rising := pulse;
END_FUNCTION",
        [Problem::EdgeInputNotAllowed]
    );

    rule_err!(
        /// Each edge input is reported, falling as well as rising.
        apply_when_function_has_two_edge_inputs_then_p4092_for_each,
        "
FUNCTION Either : BOOL
VAR_INPUT
    up : BOOL R_EDGE;
    down : BOOL F_EDGE;
END_VAR
    Either := up OR down;
END_FUNCTION",
        [Problem::EdgeInputNotAllowed; 2]
    );

    rule_err!(
        apply_when_method_has_edge_input_then_p4092,
        "
FUNCTION_BLOCK Block
METHOD Pulse : BOOL
VAR_INPUT
    pulse : BOOL F_EDGE;
END_VAR
    Pulse := pulse;
END_METHOD
END_FUNCTION_BLOCK",
        [Problem::EdgeInputNotAllowed],
        edition3_options()
    );

    rule_err!(
        /// A property's accessor is a method too, with no previous call.
        apply_when_property_accessor_has_edge_input_then_p4092,
        "
FUNCTION_BLOCK Block
PROPERTY Ready : BOOL
GET
VAR_INPUT
    pulse : BOOL R_EDGE;
END_VAR
    Ready := pulse;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK",
        [Problem::EdgeInputNotAllowed],
        fb_inheritance_options()
    );

    #[test]
    fn apply_when_function_has_edge_input_then_p4092_at_input() {
        let program = "
FUNCTION Rising : BOOL
VAR_INPUT
    pulse : BOOL R_EDGE;
END_VAR
    Rising := pulse;
END_FUNCTION";
        let diagnostics = rule_diagnostics(apply, program, &CompilerOptions::default());

        assert_eq!(
            diagnostic_codes(&diagnostics),
            [Problem::EdgeInputNotAllowed.code()]
        );
        let start = program.find("pulse").unwrap();
        let location = &diagnostics[0].primary.location;
        assert_eq!(
            (location.start, location.end),
            (start, start + "pulse".len())
        );
    }
}
