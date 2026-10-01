//! Semantic rule that rejects a variable declared with the type of an
//! `ABSTRACT` function block.
//!
//! An `ABSTRACT` function block exists only to be extended via
//! `EXTENDS` -- it cannot be instantiated directly.
//!
//! Whether a function block is `ABSTRACT` is recorded on its symbol in
//! the symbol environment. By the time semantic rules run, a `VAR`'s
//! initializer has already been resolved from `LateResolvedType` into the
//! concrete `FunctionBlock` variant, so the instance's type name is the
//! function block's name.
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

use std::convert::Infallible;

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
    symbol_environment::{ScopeKind, SymbolEnvironment, SymbolKind},
};
use ironplc_parser::options::CompilerOptions;

pub fn apply(
    lib: &Library,
    context: &SemanticContext,
    _options: &CompilerOptions,
) -> SemanticResult {
    run_rule(
        RuleAbstractNotInstantiated {
            symbols: context.symbols(),
            diagnostics: Vec::new(),
        },
        lib,
    )
}

struct RuleAbstractNotInstantiated<'a> {
    symbols: &'a SymbolEnvironment,
    diagnostics: Vec<Diagnostic>,
}

impl RuleAbstractNotInstantiated<'_> {
    /// Whether `type_name` names a function block declared `ABSTRACT`.
    fn is_abstract_function_block(&self, type_name: &TypeName) -> bool {
        self.symbols
            .find(&type_name.name, &ScopeKind::Global)
            .is_some_and(|info| info.kind == SymbolKind::FunctionBlock && info.is_abstract)
    }
}

impl DiagnosticVisitor for RuleAbstractNotInstantiated<'_> {
    fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

impl Visitor<Infallible> for RuleAbstractNotInstantiated<'_> {
    type Value = ();

    fn visit_var_decl(&mut self, node: &VarDecl) -> Result<Self::Value, Infallible> {
        if let InitialValueAssignmentKind::FunctionBlock(fb_init) = &node.initializer {
            if self.is_abstract_function_block(&fb_init.type_name) {
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

    rule_ctx_err1_with!(
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

    rule_ctx_ok_with!(
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

    rule_ctx_ok_with!(
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

    rule_ctx_ok_with!(
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
