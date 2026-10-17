//! Semantic rule that flags non-standard language extensions that
//! are parsed and represented in the AST but not yet semantically analyzed.
//!
//! See `ironplc_dsl::extension::LanguageExtension` and
//! `specs/design/beckhoff-twincat-dialect.md` §1.4. `EXTENDS`, `IMPLEMENTS`
//! and `INTERFACE` declarations no longer flag: field inheritance is
//! resolved, and an interface is a type. Calling a method through an
//! interface is reported by `rule_method_call_declared`.
//!
//! ## Fails
//!
//! ```ignore
//! FUNCTION_BLOCK ABSTRACT FB_BaseAxis
//! END_FUNCTION_BLOCK
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
        // something genuinely unsupported is present. EXTENDS is not
        // flagged: field inheritance through the EXTENDS chain is fully
        // resolved. IMPLEMENTS is not flagged either: it makes the function
        // block convert to the interface, and a call through the interface
        // is flagged where it is made. ABSTRACT still flags because it is
        // not executed: instantiation legality is enforced by
        // `rule_abstract_not_instantiated` (P4045) for a direct variable
        // declaration, but indirect instantiation (as an array element
        // type) is still unchecked.
        if let Some(oop) = &node.oop {
            if oop.qualifiers.is_abstract() {
                self.flag(oop);
            }
        }
        node.recurse_visit(self)
    }

    fn visit_self_ref_variable(
        &mut self,
        node: &ironplc_dsl::textual::SelfRefVariable,
    ) -> Result<Self::Value, Infallible> {
        // A SelfRefVariable only exists when THIS^/SUPER^ was written, so
        // it is always an extension. Parsed and rendered, but neither
        // analyzed nor executed.
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

    rule_ok!(
        apply_when_implements_then_ok,
        "
INTERFACE I_Drivable
END_INTERFACE

FUNCTION_BLOCK FB_AdvancedMotor IMPLEMENTS I_Drivable
VAR
    bRunning : BOOL;
END_VAR
END_FUNCTION_BLOCK",
        fb_inheritance_options()
    );

    #[rstest::rstest]
    #[case::this("    THIS^.count := 1;")]
    #[case::super_("    count := SUPER^.count;")]
    #[case::this_method_call("    THIS^.Start();")]
    fn apply_when_self_ref_then_p9999(#[case] body: &str) {
        let program = format!(
            "
FUNCTION_BLOCK FB_Motor
VAR
    count : INT;
END_VAR
METHOD Run
{body}
END_METHOD
END_FUNCTION_BLOCK"
        );

        let errors = rule_diagnostics(apply, &program, &fb_inheritance_options());
        assert_eq!(diagnostic_codes(&errors), [NOT_IMPLEMENTED_CODE]);
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
    fn apply_when_abstract_and_implements_then_one_p9999() {
        let program = "
FUNCTION_BLOCK ABSTRACT FB_BaseAxis IMPLEMENTS I_BaseAxis
VAR
    bEnabled : BOOL;
END_VAR
END_FUNCTION_BLOCK";

        let errors = rule_diagnostics(apply, program, &fb_inheritance_options());
        // Only ABSTRACT is still flagged.
        assert_eq!(diagnostic_codes(&errors), [NOT_IMPLEMENTED_CODE]);
    }

    rule_ok!(
        apply_when_interface_declaration_then_ok,
        "
INTERFACE I_Drivable
END_INTERFACE",
        fb_inheritance_options()
    );

    rule_ok!(
        apply_when_extends_and_implements_then_ok,
        "
INTERFACE I_Drivable
END_INTERFACE

FUNCTION_BLOCK FB_Motor
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_AdvancedMotor EXTENDS FB_Motor IMPLEMENTS I_Drivable
VAR
    bRunning : BOOL;
END_VAR
END_FUNCTION_BLOCK",
        fb_inheritance_options()
    );
}
