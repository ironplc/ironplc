//! Semantic rule that rejects a derived function block (declared with
//! the IEC 61131-3 Edition 3 `EXTENDS` clause) redeclaring a field
//! already declared on its base function block or any ancestor further
//! up the `EXTENDS` chain, or declaring a field with the name of a
//! property it inherits.
//!
//! A redeclaration is rejected as a duplicate definition even when the
//! redeclared field has a different type. Edition 3 inheritance gives a
//! derived function block one set of fields and no way to hold a second
//! field of an inherited name, and CODESYS and TwinCAT reject the same
//! code. `EXTENDS` is Edition 3 syntax rather than a vendor extension,
//! so this rule applies on every dialect (ADR-0051).
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
    result::SemanticResult, semantic_context::SemanticContext, symbol_environment::SymbolKind,
};
use ironplc_parser::options::CompilerOptions;

pub fn apply(
    lib: &Library,
    context: &SemanticContext,
    _options: &CompilerOptions,
) -> SemanticResult {
    let mut diagnostics = vec![];

    for element in &lib.elements {
        let LibraryElementKind::FunctionBlockDeclaration(fb) = element else {
            continue;
        };

        for own_field in &fb.variables {
            let Some(own_id) = own_field.identifier.symbolic_id() else {
                continue;
            };
            let Some((base_name, base)) = context.symbols().inherited_member(own_id, &fb.name.id)
            else {
                continue;
            };
            let what = if base.kind == SymbolKind::Property {
                "property"
            } else {
                "field"
            };
            diagnostics.push(Diagnostic::problem(
                Problem::ExtendsFieldNameDuplicated,
                Label::span(
                    own_id.span(),
                    format!(
                        "Field '{own_id}' is already declared as {what} '{base_name}' in a base function block"
                    ),
                ),
            ));
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
    use crate::test_helpers::fb_inheritance_options;

    rule_err!(
        apply_when_derived_redeclares_base_field_same_type_then_error,
        "
FUNCTION_BLOCK FB_Base
VAR
    state : INT;
END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Derived EXTENDS FB_Base
VAR
    state : INT;
END_VAR
END_FUNCTION_BLOCK",
        [Problem::ExtendsFieldNameDuplicated],
        fb_inheritance_options()
    );

    rule_err!(
        apply_when_derived_redeclares_base_field_different_type_then_error,
        "
FUNCTION_BLOCK FB_Base
VAR
    state : INT;
END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Derived EXTENDS FB_Base
VAR
    state : BOOL;
END_VAR
END_FUNCTION_BLOCK",
        [Problem::ExtendsFieldNameDuplicated],
        fb_inheritance_options()
    );

    rule_ok!(
        apply_when_derived_has_no_field_collision_then_ok,
        "
FUNCTION_BLOCK FB_Base
VAR
    state : INT;
END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Derived EXTENDS FB_Base
VAR
    derivedState : BOOL;
END_VAR
END_FUNCTION_BLOCK",
        fb_inheritance_options()
    );

    rule_err!(
        apply_when_grandparent_field_collision_then_error,
        "
FUNCTION_BLOCK FB_A
VAR
    a : BOOL;
END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_B EXTENDS FB_A
VAR
    b : BOOL;
END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_C EXTENDS FB_B
VAR
    a : INT;
END_VAR
END_FUNCTION_BLOCK",
        [Problem::ExtendsFieldNameDuplicated],
        fb_inheritance_options()
    );

    rule_ok!(
        apply_when_no_extends_then_ok,
        "
FUNCTION_BLOCK FB_Plain
VAR
    x : INT;
END_VAR
END_FUNCTION_BLOCK",
        fb_inheritance_options()
    );

    rule_err_at!(
        apply_when_derived_redeclares_base_field_then_error_at_derived_field,
        "
FUNCTION_BLOCK FB_Derived EXTENDS FB_Base
VAR
    state : INT;
END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Base
VAR
    state : INT;
END_VAR
END_FUNCTION_BLOCK",
        Problem::ExtendsFieldNameDuplicated,
        "state",
        fb_inheritance_options()
    );

    rule_err!(
        apply_when_derived_declares_field_named_as_inherited_property_then_error,
        "
FUNCTION_BLOCK FB_PosBase
VAR
    _pos : INT;
END_VAR
PROPERTY Position : INT
GET
    Position := _pos;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_PosDerived EXTENDS FB_PosBase
VAR
    Position : INT;
END_VAR
END_FUNCTION_BLOCK",
        [Problem::ExtendsFieldNameDuplicated],
        fb_inheritance_options()
    );

    rule_ok!(
        apply_when_derived_declares_property_of_base_property_name_then_ok,
        "
FUNCTION_BLOCK FB_PosBase
PROPERTY Position : INT
GET
    Position := 1;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_PosDerived EXTENDS FB_PosBase
PROPERTY Position : INT
GET
    Position := 2;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK",
        fb_inheritance_options()
    );
}
