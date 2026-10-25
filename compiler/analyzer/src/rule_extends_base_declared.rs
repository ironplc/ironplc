//! Semantic rule that rejects a function block whose `EXTENDS` clause names
//! anything but a function block the library declares (OOP extension).
//!
//! The base is resolved once, by the type environment, which records it only
//! when it is a function block the library declares
//! ([`TypeEnvironment::base_of`]). A block whose source names a base the
//! environment did not record is reported here, at the base name:
//!
//! - a name that is not declared, or that names another kind of type, such
//!   as a structure or an interface, is P4088;
//! - a standard function block, such as `TON`, is not implemented (P9999):
//!   nothing inherits a standard block's fields or methods, so accepting it
//!   would let a program pass analysis and lose its base.
//!
//! A repeated declaration of a block's name is reported elsewhere (P4013),
//! and the environment keeps the first one; only that one is checked here,
//! so a repeat does not report a base the environment never meant to record.
//! A library block named like a standard block is a redeclaration of that
//! block and is skipped for the same reason.
//!
//! ## Passes
//!
//! ```ignore
//! FUNCTION_BLOCK FB_Base
//! END_FUNCTION_BLOCK
//!
//! FUNCTION_BLOCK FB_Derived EXTENDS FB_Base
//! END_FUNCTION_BLOCK
//! ```
//!
//! ## Fails
//!
//! ```ignore
//! FUNCTION_BLOCK FB_Derived EXTENDS FB_Missing
//! END_FUNCTION_BLOCK
//! ```

use std::collections::HashSet;

use ironplc_dsl::{
    common::*,
    core::Located,
    diagnostic::{Diagnostic, Label},
};
use ironplc_parser::options::CompilerOptions;
use ironplc_problems::Problem;

use crate::{result::SemanticResult, semantic_context::SemanticContext};

pub fn apply(
    lib: &Library,
    context: &SemanticContext,
    _options: &CompilerOptions,
) -> SemanticResult {
    let types = context.types();
    let mut checked: HashSet<&TypeName> = HashSet::new();
    let mut diagnostics = vec![];

    for element in &lib.elements {
        let LibraryElementKind::FunctionBlockDeclaration(fb) = element else {
            continue;
        };
        if !checked.insert(&fb.name) {
            continue;
        }
        let Some(base) = fb.oop.as_ref().and_then(|oop| oop.base.as_ref()) else {
            continue;
        };
        let Some(block) = types.id_of(&fb.name) else {
            continue;
        };
        let is_standard = |name: &TypeName| {
            types
                .get(name)
                .is_some_and(|attributes| attributes.standard_block.is_some())
        };
        if is_standard(&fb.name) || types.base_of(block).is_some() {
            continue;
        }

        let diagnostic = if is_standard(base) {
            Diagnostic::not_implemented(Label::span(
                base.span(),
                format!("Extending the standard function block '{base}' is not supported"),
            ))
        } else {
            let label = match types.id_of(base) {
                None => "Not declared",
                Some(_) => "Not a function block",
            };
            Diagnostic::problem(
                Problem::ExtendsNotFunctionBlock,
                Label::span(base.span(), label),
            )
        };
        diagnostics.push(
            diagnostic
                .with_context_type("function block", &fb.name)
                .with_context_type("base", base),
        );
    }

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{
        diagnostic_codes, fb_inheritance_options, rule_diagnostics, NOT_IMPLEMENTED_CODE,
    };

    rule_ok!(
        apply_when_base_declared_then_ok,
        "
FUNCTION_BLOCK FB_Base
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Derived EXTENDS FB_Base
END_FUNCTION_BLOCK",
        fb_inheritance_options()
    );

    rule_ok!(
        apply_when_no_extends_then_ok,
        "
FUNCTION_BLOCK FB_Plain
END_FUNCTION_BLOCK",
        fb_inheritance_options()
    );

    rule_err_at!(
        apply_when_base_not_declared_then_error_at_base,
        "
FUNCTION_BLOCK FB_A EXTENDS FB_Missing
END_FUNCTION_BLOCK",
        Problem::ExtendsNotFunctionBlock,
        "FB_Missing",
        fb_inheritance_options()
    );

    rule_err_at!(
        apply_when_base_is_structure_then_error_at_base,
        "
FUNCTION_BLOCK FB_A EXTENDS Pair
END_FUNCTION_BLOCK

TYPE Pair : STRUCT f : INT; END_STRUCT; END_TYPE",
        Problem::ExtendsNotFunctionBlock,
        "Pair",
        fb_inheritance_options()
    );

    rule_err!(
        apply_when_base_is_interface_then_error,
        "
INTERFACE I_Drive
END_INTERFACE

FUNCTION_BLOCK FB_A EXTENDS I_Drive
END_FUNCTION_BLOCK",
        [Problem::ExtendsNotFunctionBlock],
        fb_inheritance_options()
    );

    rule_err!(
        apply_when_two_blocks_extend_undeclared_then_error_for_each,
        "
FUNCTION_BLOCK FB_A EXTENDS FB_Missing
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_B EXTENDS FB_Other
END_FUNCTION_BLOCK",
        [Problem::ExtendsNotFunctionBlock; 2],
        fb_inheritance_options()
    );

    // The second declaration of `FB_A` is a repeat (P4013), whose base the
    // environment does not record: it is not reported as undeclared.
    rule_ok!(
        apply_when_repeated_declaration_extends_then_ok,
        "
FUNCTION_BLOCK FB_Base
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_A
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_A EXTENDS FB_Base
END_FUNCTION_BLOCK",
        fb_inheritance_options()
    );

    #[test]
    fn apply_when_base_is_standard_block_then_not_implemented() {
        let program = "
FUNCTION_BLOCK FB_A EXTENDS TON
END_FUNCTION_BLOCK";

        let diagnostics = rule_diagnostics(apply, program, &fb_inheritance_options());

        assert_eq!(diagnostic_codes(&diagnostics), [NOT_IMPLEMENTED_CODE]);
    }
}
