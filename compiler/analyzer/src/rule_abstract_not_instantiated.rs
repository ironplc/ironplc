//! Semantic rule that rejects a variable declared with the type of an
//! `ABSTRACT` function block.
//!
//! An `ABSTRACT` function block exists only to be extended via
//! `EXTENDS` -- it cannot be instantiated directly.
//!
//! Deliberately works directly off the AST rather than threading
//! `is_abstract` through `IntermediateType::FunctionBlock` -- by the
//! time semantic rules run, a `VAR`'s initializer has already been
//! resolved from `LateResolvedType` into the concrete `FunctionBlock`
//! variant, so no additional type resolution is needed here.
//!
//! ## Passes
//!
//! ```ignore
//! FUNCTION_BLOCK ABSTRACT FB_Base
//! END_FUNCTION_BLOCK
//!
//! FUNCTION_BLOCK FB_Concrete EXTENDS FB_Base
//! END_FUNCTION_BLOCK
//!
//! FUNCTION_BLOCK FB_User
//! VAR
//!     inst : FB_Concrete;
//! END_VAR
//! END_FUNCTION_BLOCK
//! ```
//!
//! ## Fails
//!
//! ```ignore
//! FUNCTION_BLOCK ABSTRACT FB_Base
//! END_FUNCTION_BLOCK
//!
//! FUNCTION_BLOCK FB_User
//! VAR
//!     inst : FB_Base;
//! END_VAR
//! END_FUNCTION_BLOCK
//! ```

use std::collections::HashSet;

use ironplc_dsl::{
    common::*,
    core::Located,
    diagnostic::{Diagnostic, Label},
    visitor::Visitor,
};
use ironplc_problems::Problem;

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
    let abstract_fbs: HashSet<TypeName> = lib
        .elements
        .iter()
        .filter_map(|e| match e {
            LibraryElementKind::FunctionBlockDeclaration(fb)
                if fb.oop.as_ref().is_some_and(|oop| oop.is_abstract) =>
            {
                Some(fb.name.clone())
            }
            _ => None,
        })
        .collect();

    if abstract_fbs.is_empty() {
        return Ok(());
    }

    run_rule(
        RuleAbstractNotInstantiated {
            abstract_fbs,
            diagnostics: Vec::new(),
        },
        lib,
    )
}

struct RuleAbstractNotInstantiated {
    abstract_fbs: HashSet<TypeName>,
    diagnostics: Vec<Diagnostic>,
}

impl DiagnosticVisitor for RuleAbstractNotInstantiated {
    fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

impl Visitor<Diagnostic> for RuleAbstractNotInstantiated {
    type Value = ();

    fn visit_var_decl(&mut self, node: &VarDecl) -> Result<Self::Value, Diagnostic> {
        if let InitialValueAssignmentKind::FunctionBlock(fb_init) = &node.initializer {
            if self.abstract_fbs.contains(&fb_init.type_name) {
                self.diagnostics.push(Diagnostic::problem(
                    Problem::AbstractFunctionBlockInstantiated,
                    Label::span(
                        fb_init.type_name.span(),
                        format!(
                            "Function block '{}' is ABSTRACT and cannot be instantiated",
                            fb_init.type_name
                        ),
                    ),
                ));
            }
        }
        node.recurse_visit(self)
    }
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
        apply_when_abstract_fb_instantiated_then_error,
        opts_with_fb_inheritance(),
        "
FUNCTION_BLOCK ABSTRACT FB_Base
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_User
VAR
    inst : FB_Base;
END_VAR
END_FUNCTION_BLOCK",
        Problem::AbstractFunctionBlockInstantiated
    );

    rule_ok_with!(
        apply_when_non_abstract_fb_instantiated_then_ok,
        opts_with_fb_inheritance(),
        "
FUNCTION_BLOCK FB_Base
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_User
VAR
    inst : FB_Base;
END_VAR
END_FUNCTION_BLOCK"
    );

    rule_ok_with!(
        apply_when_concrete_subclass_of_abstract_instantiated_then_ok,
        opts_with_fb_inheritance(),
        "
FUNCTION_BLOCK ABSTRACT FB_Base
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Concrete EXTENDS FB_Base
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_User
VAR
    inst : FB_Concrete;
END_VAR
END_FUNCTION_BLOCK"
    );

    rule_ok_with!(
        apply_when_no_abstract_fb_in_library_then_ok,
        opts_with_fb_inheritance(),
        "
FUNCTION_BLOCK FB_Plain
VAR
    x : INT;
END_VAR
END_FUNCTION_BLOCK"
    );
}
