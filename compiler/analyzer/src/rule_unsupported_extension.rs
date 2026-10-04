//! Semantic rule that flags non-standard language extensions that
//! are parsed and represented in the AST but not yet semantically analyzed.
//!
//! See `ironplc_dsl::extension::LanguageExtension` and
//! `specs/design/beckhoff-twincat-dialect.md` §1.4. Plain `EXTENDS` with no
//! `IMPLEMENTS`/`ABSTRACT` no longer flags, since field inheritance is
//! fully resolved.
//!
//! ## Fails
//!
//! ```ignore
//! FUNCTION_BLOCK FB_AdvancedMotor IMPLEMENTS I_Drivable
//! END_FUNCTION_BLOCK
//! ```
//!
//! ```ignore
//! INTERFACE I_Drivable
//! END_INTERFACE
//! ```
use ironplc_dsl::{
    common::*,
    diagnostic::{Diagnostic, Label},
    extension::LanguageExtension,
    visitor::Visitor,
};
use std::convert::Infallible;

use crate::{
    result::SemanticResult,
    rule_support::{run_rule, DiagnosticVisitor},
    semantic_context::SemanticContext,
};
use ironplc_parser::options::CompilerOptions;

pub fn apply(
    lib: &Library,
    _context: &SemanticContext,
    _options: &CompilerOptions,
) -> SemanticResult {
    run_rule(
        RuleUnsupportedExtension {
            diagnostics: Vec::new(),
        },
        lib,
    )
}

struct RuleUnsupportedExtension {
    diagnostics: Vec<Diagnostic>,
}

impl DiagnosticVisitor for RuleUnsupportedExtension {
    fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

impl RuleUnsupportedExtension {
    fn flag(&mut self, ext: &dyn LanguageExtension) {
        self.diagnostics
            .push(Diagnostic::not_implemented(Label::span(
                ext.extension_span(),
                format!(
                    "{} is recognized but not yet supported by IronPLC",
                    ext.extension_name(),
                ),
            )));
    }
}

impl Visitor<Infallible> for RuleUnsupportedExtension {
    type Value = ();

    fn visit_function_block_declaration(
        &mut self,
        node: &FunctionBlockDeclaration,
    ) -> Result<Self::Value, Infallible> {
        // Most function blocks are standard IEC 61131-3 — only flag when
        // something genuinely unsupported is present. Plain EXTENDS (no
        // IMPLEMENTS, not ABSTRACT) is no longer flagged: field
        // inheritance through the EXTENDS chain is fully resolved, so
        // there's nothing left unsupported for that shape. IMPLEMENTS
        // (interface dispatch) remains unimplemented and still flags.
        // ABSTRACT still flags because it is not executed: instantiation
        // legality is enforced by `rule_abstract_not_instantiated`
        // (P4045) for a direct variable declaration, but indirect
        // instantiation (as an array element type) is still unchecked.
        if let Some(oop) = &node.oop {
            if !oop.implements.is_empty() || oop.qualifiers.is_abstract() {
                self.flag(oop);
            }
        }
        node.recurse_visit(self)
    }

    /// `THIS^.member` and `SUPER^.member` are analyzed; only the head of
    /// the reference is a self reference, so it is not flagged here.
    fn visit_structured_variable(
        &mut self,
        node: &ironplc_dsl::textual::StructuredVariable,
    ) -> Result<Self::Value, Infallible> {
        if matches!(
            node.record.as_ref(),
            ironplc_dsl::textual::SymbolicVariableKind::SelfRef(_)
        ) {
            return Ok(());
        }
        node.recurse_visit(self)
    }

    /// `THIS^.M()` and `SUPER^.M()` are analyzed, so the receiver is not
    /// flagged.
    fn visit_method_receiver(
        &mut self,
        node: &ironplc_dsl::textual::MethodReceiver,
    ) -> Result<Self::Value, Infallible> {
        if let ironplc_dsl::textual::MethodReceiver::SelfRef(_) = node {
            return Ok(());
        }
        node.recurse_visit(self)
    }

    fn visit_self_ref_variable(
        &mut self,
        node: &ironplc_dsl::textual::SelfRefVariable,
    ) -> Result<Self::Value, Infallible> {
        // Only a bare THIS^/SUPER^ gets here: one used as a value on its
        // own (passed, assigned, compared), which is not analyzed.
        self.flag(node);
        node.recurse_visit(self)
    }

    fn visit_interface_declaration(
        &mut self,
        node: &InterfaceDeclaration,
    ) -> Result<Self::Value, Infallible> {
        // An InterfaceDeclaration only exists when INTERFACE syntax was
        // used, so it is always an extension.
        self.flag(node);
        node.recurse_visit(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::diagnostic_codes;
    use crate::test_helpers::fb_inheritance_options;
    use crate::test_helpers::rule_diagnostics;

    use crate::test_helpers::NOT_IMPLEMENTED_CODE;

    rule_ok!(
        apply_when_plain_function_block_then_ok,
        "
FUNCTION_BLOCK FB_Motor
VAR
    bRunning : BOOL;
END_VAR
END_FUNCTION_BLOCK"
    );

    // Plain EXTENDS (no IMPLEMENTS, not ABSTRACT) no longer flags --
    // field inheritance through the EXTENDS chain is fully resolved.
    rule_ok!(
        apply_when_plain_extends_then_ok,
        "
FUNCTION_BLOCK FB_Motor
VAR
    bRunning : BOOL;
END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_AdvancedMotor EXTENDS FB_Motor
VAR
    bTurbo : BOOL;
END_VAR
END_FUNCTION_BLOCK",
        fb_inheritance_options()
    );

    #[test]
    fn apply_when_implements_then_p9999() {
        let program = "
FUNCTION_BLOCK FB_AdvancedMotor IMPLEMENTS I_Drivable
VAR
    bRunning : BOOL;
END_VAR
END_FUNCTION_BLOCK";

        let errors = rule_diagnostics(apply, program, &fb_inheritance_options());
        assert_eq!(diagnostic_codes(&errors), [NOT_IMPLEMENTED_CODE]);
    }

    /// `THIS^`/`SUPER^` with a member or a method call after it is
    /// analyzed, so it is not an unsupported extension.
    #[rstest::rstest]
    #[case::this("    THIS^.count := 1;")]
    #[case::super_("    count := SUPER^.count;")]
    #[case::this_method_call("    THIS^.Start();")]
    fn apply_when_self_ref_member_then_ok(#[case] body: &str) {
        let errors = rule_diagnostics(apply, &motor_with(body), &fb_inheritance_options());
        assert!(errors.is_empty());
    }

    #[test]
    fn apply_when_bare_self_ref_then_p9999() {
        let errors = rule_diagnostics(
            apply,
            &motor_with("    THIS^ := THIS^;"),
            &fb_inheritance_options(),
        );
        assert_eq!(
            diagnostic_codes(&errors),
            [NOT_IMPLEMENTED_CODE, NOT_IMPLEMENTED_CODE]
        );
    }

    fn motor_with(body: &str) -> String {
        format!(
            "
FUNCTION_BLOCK FB_Motor
VAR
    count : INT;
END_VAR
METHOD Start
END_METHOD
METHOD Run
{body}
END_METHOD
END_FUNCTION_BLOCK"
        )
    }

    #[test]
    fn apply_when_abstract_then_p9999() {
        let program = "
FUNCTION_BLOCK ABSTRACT FB_BaseAxis
VAR
    bEnabled : BOOL;
END_VAR
END_FUNCTION_BLOCK";

        let errors = rule_diagnostics(apply, program, &fb_inheritance_options());
        assert_eq!(diagnostic_codes(&errors), [NOT_IMPLEMENTED_CODE]);
    }

    #[test]
    fn apply_when_abstract_and_implements_then_only_one_p9999() {
        let program = "
FUNCTION_BLOCK ABSTRACT FB_BaseAxis IMPLEMENTS I_BaseAxis
VAR
    bEnabled : BOOL;
END_VAR
END_FUNCTION_BLOCK";

        let errors = rule_diagnostics(apply, program, &fb_inheritance_options());
        // One diagnostic for the whole FB, not one per clause.
        assert_eq!(diagnostic_codes(&errors), [NOT_IMPLEMENTED_CODE]);
    }

    #[test]
    fn apply_when_interface_declaration_then_p9999() {
        let program = "
INTERFACE I_Drivable
END_INTERFACE";

        let errors = rule_diagnostics(apply, program, &fb_inheritance_options());
        assert_eq!(diagnostic_codes(&errors), [NOT_IMPLEMENTED_CODE]);
    }

    #[test]
    fn apply_when_extends_and_interface_then_both_flagged() {
        let program = "
INTERFACE I_Drivable
END_INTERFACE

FUNCTION_BLOCK FB_AdvancedMotor EXTENDS FB_Motor IMPLEMENTS I_Drivable
VAR
    bRunning : BOOL;
END_VAR
END_FUNCTION_BLOCK";

        let errors = rule_diagnostics(apply, program, &fb_inheritance_options());
        // One for the INTERFACE declaration, one for the FB's IMPLEMENTS
        // clause (EXTENDS alone wouldn't flag, but IMPLEMENTS still does).
        assert_eq!(errors.len(), 2);
    }
}
