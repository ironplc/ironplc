//! Semantic rule that gates general (non-constant) expressions used as the
//! value in a structured/call-style initializer's `name := value` pairs
//! (e.g. `tonDelta : TON := (PT := pDevice^.Delta);`) behind
//! `--allow-struct-initializer-expressions`.
//!
//! The IEC 61131-3 standard grammar for a structured initializer value
//! (Annex B) is
//! `structure_element_initialization ::= structure_element_name ':=' (constant
//! | enumerated_value | array_initialization | structure_initialization)`.
//! A general expression -- and in particular a pointer dereference plus
//! member access like `pDevice^.Delta` -- is deliberately not one of those
//! productions, so accepting it is an extension. The
//! parser always accepts the broader form; this rule is what enforces the
//! flag.
//!
//! ## Fails (without the flag)
//!
//! ```ignore
//! FUNCTION_BLOCK FB_Example
//! VAR
//!     pDevice : REF_TO FB_Device;
//!     tonDelta : TON := (PT := pDevice^.Delta);
//! END_VAR
//! END_FUNCTION_BLOCK
//! ```
use ironplc_dsl::{
    common::StructInitialValueAssignmentKind,
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
    if options.allow_struct_initializer_expressions {
        return Ok(());
    }

    run_rule(
        RuleStructInitializerExpression {
            diagnostics: Vec::new(),
        },
        lib,
    )
}

struct RuleStructInitializerExpression {
    diagnostics: Vec<Diagnostic>,
}

impl DiagnosticVisitor for RuleStructInitializerExpression {
    fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

impl Visitor<Infallible> for RuleStructInitializerExpression {
    type Value = ();

    fn visit_struct_initial_value_assignment_kind(
        &mut self,
        node: &StructInitialValueAssignmentKind,
    ) -> Result<Self::Value, Infallible> {
        if let StructInitialValueAssignmentKind::Expression(expr) = node {
            self.diagnostics.push(Diagnostic::problem(
                Problem::StructInitializerExpressionNotAllowed,
                Label::span(
                    expr.span(),
                    "Expression-valued struct/FB-instance initializer",
                ),
            ));
        }
        node.recurse_visit(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts_ref_to() -> CompilerOptions {
        CompilerOptions {
            allow_ref_to: true,
            ..CompilerOptions::default()
        }
    }

    fn opts_ref_to_and_flag() -> CompilerOptions {
        CompilerOptions {
            allow_ref_to: true,
            allow_struct_initializer_expressions: true,
            ..CompilerOptions::default()
        }
    }

    const SOURCE: &str = "FUNCTION_BLOCK FB_Device VAR_INPUT Delta : INT; END_VAR END_FUNCTION_BLOCK TYPE MyStruct : STRUCT x : INT; END_STRUCT; END_TYPE PROGRAM main VAR pDevice : REF_TO FB_Device; s : MyStruct := (x := pDevice^.Delta); END_VAR END_PROGRAM";

    rule_err1_with!(
        apply_when_struct_init_expression_and_flag_disabled_then_error,
        opts_ref_to(),
        SOURCE,
        Problem::StructInitializerExpressionNotAllowed
    );

    rule_ok_with!(
        apply_when_struct_init_expression_and_flag_enabled_then_ok,
        opts_ref_to_and_flag(),
        SOURCE
    );

    // A struct initializer whose value is an ordinary constant parses as
    // StructInitialValueAssignmentKind::Constant, not Expression, so it is
    // standard syntax and must never be flagged regardless of the option.
    rule_ok!(
        apply_when_plain_constant_struct_init_then_never_flagged,
        "
TYPE MyStruct :
STRUCT
    x : INT;
END_STRUCT;
END_TYPE

PROGRAM main
VAR
    s : MyStruct := (x := 5);
END_VAR
END_PROGRAM"
    );

    // A bare identifier naming a variable in scope is a runtime value, and
    // the gate must see it. The parser cannot tell it from an enumeration
    // value -- `xform_resolve_late_bound_expr_kind` is what decides, once
    // declarations are known.
    const BARE_VARIABLE_SOURCE: &str = "
TYPE MyStruct :
STRUCT
    x : INT;
END_STRUCT;
END_TYPE

PROGRAM main
VAR
    g : INT;
    s : MyStruct := (x := g);
END_VAR
END_PROGRAM";

    rule_err1!(
        apply_when_struct_init_is_bare_variable_and_flag_disabled_then_error,
        BARE_VARIABLE_SOURCE,
        Problem::StructInitializerExpressionNotAllowed
    );

    rule_ok_with!(
        apply_when_struct_init_is_bare_variable_and_flag_enabled_then_ok,
        CompilerOptions {
            allow_struct_initializer_expressions: true,
            ..CompilerOptions::default()
        },
        BARE_VARIABLE_SOURCE
    );

    // A bare identifier naming an enumeration value is standard syntax and
    // must never be flagged -- the reclassification keys on whether the name
    // resolves to a variable, not on its shape.
    rule_ok!(
        apply_when_struct_init_is_enumerated_value_then_never_flagged,
        "
TYPE Color : (RED, GREEN); END_TYPE

TYPE MyStruct :
STRUCT
    c : Color;
END_STRUCT;
END_TYPE

PROGRAM main
VAR
    s : MyStruct := (c := RED);
END_VAR
END_PROGRAM"
    );

    // A function block instance's member initializer goes through the same
    // gate: the declaration is rewritten to an FB instance before this rule
    // runs, and the rule must still reach the member values.
    rule_err1!(
        apply_when_fb_instance_member_init_is_expression_then_error,
        "
FUNCTION_BLOCK FB_Example
VAR
    delta : TIME;
    tonDelta : TON := (PT := delta);
END_VAR
END_FUNCTION_BLOCK",
        Problem::StructInitializerExpressionNotAllowed
    );
}
