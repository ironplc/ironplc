//! Semantic rule that rejects a derived function block (via the
//! `EXTENDS` extension) redeclaring a field
//! already declared on its base function block or any ancestor further
//! up the `EXTENDS` chain.
//!
//! A redeclaration is rejected as a duplicate definition even when the
//! redeclared field has a different type. See
//! `specs/plans/2026-07-20-twincat-extends-duplicate-field.md`.
//!
//! ## Passes
//!
//! ```ignore
//! FUNCTION_BLOCK FB_Base
//! VAR
//!     state : INT;
//! END_VAR
//! END_FUNCTION_BLOCK
//!
//! FUNCTION_BLOCK FB_Derived EXTENDS FB_Base
//! VAR
//!     derivedState : BOOL;
//! END_VAR
//! END_FUNCTION_BLOCK
//! ```
//!
//! ## Fails
//!
//! ```ignore
//! FUNCTION_BLOCK FB_Base
//! VAR
//!     state : INT;
//! END_VAR
//! END_FUNCTION_BLOCK
//!
//! FUNCTION_BLOCK FB_Derived EXTENDS FB_Base
//! VAR
//!     state : BOOL;
//! END_VAR
//! END_FUNCTION_BLOCK
//! ```

use ironplc_dsl::{
    common::*,
    core::Located,
    diagnostic::{Diagnostic, Label},
};
use ironplc_problems::Problem;

use crate::{
    intermediates::inherited_fields::collect_inherited_fields, result::SemanticResult,
    semantic_context::SemanticContext,
};
use ironplc_parser::options::CompilerOptions;

pub fn apply(
    lib: &Library,
    _context: &SemanticContext,
    _options: &CompilerOptions,
) -> SemanticResult {
    let inherited = collect_inherited_fields(lib);
    let mut diagnostics = vec![];

    for element in &lib.elements {
        let LibraryElementKind::FunctionBlockDeclaration(fb) = element else {
            continue;
        };
        let Some(inherited_fields) = inherited.get(&fb.name) else {
            continue;
        };

        for own_field in &fb.variables {
            let Some(own_id) = own_field.identifier.symbolic_id() else {
                continue;
            };
            if let Some(base_field) = inherited_fields
                .iter()
                .find(|f| f.identifier.symbolic_id() == Some(own_id))
            {
                let base_name = base_field
                    .identifier
                    .symbolic_id()
                    .map(|id| id.to_string())
                    .unwrap_or_default();
                diagnostics.push(Diagnostic::problem(
                    Problem::ExtendsFieldNameDuplicated,
                    Label::span(
                        own_id.span(),
                        format!(
                            "Field '{own_id}' is already declared as '{base_name}' in a base function block"
                        ),
                    ),
                ));
            }
        }
    }

    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts_with_fb_inheritance() -> CompilerOptions {
        CompilerOptions {
            allow_fb_inheritance: true,
            ..CompilerOptions::default()
        }
    }

    rule_err1_with!(
        apply_when_derived_redeclares_base_field_same_type_then_error,
        opts_with_fb_inheritance(),
        "FUNCTION_BLOCK FB_Base VAR state : INT; END_VAR END_FUNCTION_BLOCK FUNCTION_BLOCK FB_Derived EXTENDS FB_Base VAR state : INT; END_VAR END_FUNCTION_BLOCK",
        Problem::ExtendsFieldNameDuplicated
    );

    rule_err1_with!(
        apply_when_derived_redeclares_base_field_different_type_then_error,
        opts_with_fb_inheritance(),
        "FUNCTION_BLOCK FB_Base VAR state : INT; END_VAR END_FUNCTION_BLOCK FUNCTION_BLOCK FB_Derived EXTENDS FB_Base VAR state : BOOL; END_VAR END_FUNCTION_BLOCK",
        Problem::ExtendsFieldNameDuplicated
    );

    rule_ok_with!(
        apply_when_derived_has_no_field_collision_then_ok,
        opts_with_fb_inheritance(),
        "FUNCTION_BLOCK FB_Base VAR state : INT; END_VAR END_FUNCTION_BLOCK FUNCTION_BLOCK FB_Derived EXTENDS FB_Base VAR derivedState : BOOL; END_VAR END_FUNCTION_BLOCK"
    );

    rule_err1_with!(
        apply_when_grandparent_field_collision_then_error,
        opts_with_fb_inheritance(),
        "FUNCTION_BLOCK FB_A VAR a : BOOL; END_VAR END_FUNCTION_BLOCK FUNCTION_BLOCK FB_B EXTENDS FB_A VAR b : BOOL; END_VAR END_FUNCTION_BLOCK FUNCTION_BLOCK FB_C EXTENDS FB_B VAR a : INT; END_VAR END_FUNCTION_BLOCK",
        Problem::ExtendsFieldNameDuplicated
    );

    rule_ok_with!(
        apply_when_no_extends_then_ok,
        opts_with_fb_inheritance(),
        "FUNCTION_BLOCK FB_Plain VAR x : INT; END_VAR END_FUNCTION_BLOCK"
    );
}
