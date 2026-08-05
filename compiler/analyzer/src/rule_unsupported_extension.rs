//! Semantic rule that flags non-standard language extensions that
//! are parsed and represented in the AST but not yet semantically analyzed.
//!
//! See `ironplc_dsl::extension::LanguageExtension`,
//! `specs/plans/2026-07-18-twincat-extends-implements-interface.md`, and
//! `specs/plans/2026-07-20-twincat-extends-field-inheritance.md` (plain
//! `EXTENDS` with no `IMPLEMENTS`/`ABSTRACT` no longer flags, since field
//! inheritance is fully resolved).
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

use crate::{result::SemanticResult, semantic_context::SemanticContext};
use ironplc_parser::options::CompilerOptions;

pub fn apply(
    lib: &Library,
    _context: &SemanticContext,
    _options: &CompilerOptions,
) -> SemanticResult {
    let mut visitor = RuleUnsupportedExtension {
        diagnostics: Vec::new(),
    };
    visitor.walk(lib).map_err(|e| vec![e])?;

    if !visitor.diagnostics.is_empty() {
        return Err(visitor.diagnostics);
    }
    Ok(())
}

struct RuleUnsupportedExtension {
    diagnostics: Vec<Diagnostic>,
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

impl Visitor<Diagnostic> for RuleUnsupportedExtension {
    type Value = ();

    fn visit_function_block_declaration(
        &mut self,
        node: &FunctionBlockDeclaration,
    ) -> Result<Self::Value, Diagnostic> {
        // Most function blocks are standard IEC 61131-3 — only flag when
        // something genuinely unsupported is present. Plain EXTENDS (no
        // IMPLEMENTS, not ABSTRACT) is no longer flagged: field
        // inheritance through the EXTENDS chain is fully resolved (see
        // specs/plans/2026-07-20-twincat-extends-field-inheritance.md),
        // so there's nothing left unsupported for that shape. IMPLEMENTS
        // (interface dispatch) and ABSTRACT (instantiation-legality
        // enforcement) remain unimplemented and still flag.
        if let Some(oop) = &node.oop {
            if !oop.implements.is_empty() || oop.is_abstract {
                self.flag(oop);
            }
        }
        node.recurse_visit(self)
    }

    fn visit_interface_declaration(
        &mut self,
        node: &InterfaceDeclaration,
    ) -> Result<Self::Value, Diagnostic> {
        // An InterfaceDeclaration only exists when INTERFACE syntax was
        // used, so it is always an extension.
        self.flag(node);
        node.recurse_visit(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::semantic_context::SemanticContextBuilder;
    use crate::test_helpers::parse_and_resolve_types_with_options;

    fn opts_with_fb_inheritance() -> CompilerOptions {
        CompilerOptions {
            allow_fb_inheritance: true,
            ..CompilerOptions::default()
        }
    }

    rule_ok!(
        apply_when_plain_function_block_then_ok,
        "FUNCTION_BLOCK FB_Motor VAR bRunning : BOOL; END_VAR END_FUNCTION_BLOCK"
    );

    // Plain EXTENDS (no IMPLEMENTS, not ABSTRACT) no longer flags --
    // field inheritance through the EXTENDS chain is fully resolved.
    // See specs/plans/2026-07-20-twincat-extends-field-inheritance.md.
    rule_ok_with!(
        apply_when_plain_extends_then_ok,
        opts_with_fb_inheritance(),
        "FUNCTION_BLOCK FB_Motor VAR bRunning : BOOL; END_VAR END_FUNCTION_BLOCK FUNCTION_BLOCK FB_AdvancedMotor EXTENDS FB_Motor VAR bTurbo : BOOL; END_VAR END_FUNCTION_BLOCK"
    );

    #[test]
    fn apply_when_implements_then_p9999() {
        let program = "
FUNCTION_BLOCK FB_AdvancedMotor IMPLEMENTS I_Drivable
VAR
    bRunning : BOOL;
END_VAR
END_FUNCTION_BLOCK";

        let (input, _context) =
            parse_and_resolve_types_with_options(program, &opts_with_fb_inheritance());
        let context = SemanticContextBuilder::new().build().unwrap();
        let result = apply(&input, &context, &opts_with_fb_inheritance());

        let errors = result.unwrap_err();
        assert_eq!(errors.len(), 1);
        // P9999 == Problem::NotImplemented; the enum variant is #[deprecated]
        // (must be constructed via Diagnostic::not_implemented), so assert on
        // the stable code string rather than referencing the variant.
        assert_eq!("P9999", errors[0].code);
    }

    #[test]
    fn apply_when_abstract_then_p9999() {
        let program = "
FUNCTION_BLOCK ABSTRACT FB_BaseAxis
VAR
    bEnabled : BOOL;
END_VAR
END_FUNCTION_BLOCK";

        let (input, _context) =
            parse_and_resolve_types_with_options(program, &opts_with_fb_inheritance());
        let context = SemanticContextBuilder::new().build().unwrap();
        let result = apply(&input, &context, &opts_with_fb_inheritance());

        let errors = result.unwrap_err();
        assert_eq!(errors.len(), 1);
        // P9999 == Problem::NotImplemented; the enum variant is #[deprecated]
        // (must be constructed via Diagnostic::not_implemented), so assert on
        // the stable code string rather than referencing the variant.
        assert_eq!("P9999", errors[0].code);
    }

    #[test]
    fn apply_when_abstract_and_implements_then_only_one_p9999() {
        let program = "
FUNCTION_BLOCK ABSTRACT FB_BaseAxis IMPLEMENTS I_BaseAxis
VAR
    bEnabled : BOOL;
END_VAR
END_FUNCTION_BLOCK";

        let (input, _context) =
            parse_and_resolve_types_with_options(program, &opts_with_fb_inheritance());
        let context = SemanticContextBuilder::new().build().unwrap();
        let result = apply(&input, &context, &opts_with_fb_inheritance());

        let errors = result.unwrap_err();
        // One diagnostic for the whole FB, not one per clause.
        assert_eq!(errors.len(), 1);
        // P9999 == Problem::NotImplemented; the enum variant is #[deprecated]
        // (must be constructed via Diagnostic::not_implemented), so assert on
        // the stable code string rather than referencing the variant.
        assert_eq!("P9999", errors[0].code);
    }

    #[test]
    fn apply_when_interface_declaration_then_p9999() {
        let program = "
INTERFACE I_Drivable
END_INTERFACE";

        let (input, _context) =
            parse_and_resolve_types_with_options(program, &opts_with_fb_inheritance());
        let context = SemanticContextBuilder::new().build().unwrap();
        let result = apply(&input, &context, &opts_with_fb_inheritance());

        let errors = result.unwrap_err();
        assert_eq!(errors.len(), 1);
        // P9999 == Problem::NotImplemented; the enum variant is #[deprecated]
        // (must be constructed via Diagnostic::not_implemented), so assert on
        // the stable code string rather than referencing the variant.
        assert_eq!("P9999", errors[0].code);
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

        let (input, _context) =
            parse_and_resolve_types_with_options(program, &opts_with_fb_inheritance());
        let context = SemanticContextBuilder::new().build().unwrap();
        let result = apply(&input, &context, &opts_with_fb_inheritance());

        let errors = result.unwrap_err();
        // One for the INTERFACE declaration, one for the FB's IMPLEMENTS
        // clause (EXTENDS alone wouldn't flag, but IMPLEMENTS still does).
        assert_eq!(errors.len(), 2);
    }
}
