//! Semantic rule that rejects a variable declared with the type of an
//! `ABSTRACT` function block (the CODESYS/TwinCAT vendor extension).
//!
//! An `ABSTRACT` function block exists only to be extended via
//! `EXTENDS` -- it cannot be instantiated directly.
//!
//! Verified against a real TcXaeShell compile before implementing
//! (`C0434: Function block ... is ABSTRACT and cannot be instantiated`).
//! See `specs/plans/2026-07-20-twincat-abstract-instantiation.md`.
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

use crate::{result::SemanticResult, semantic_context::SemanticContext};
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
            LibraryElementKind::FunctionBlockDeclaration(fb) if fb.is_abstract => {
                Some(fb.name.clone())
            }
            _ => None,
        })
        .collect();

    if abstract_fbs.is_empty() {
        return Ok(());
    }

    let mut visitor = RuleAbstractNotInstantiated {
        abstract_fbs,
        diagnostics: Vec::new(),
    };
    visitor.walk(lib).map_err(|e| vec![e])?;

    if !visitor.diagnostics.is_empty() {
        return Err(visitor.diagnostics);
    }
    Ok(())
}

struct RuleAbstractNotInstantiated {
    abstract_fbs: HashSet<TypeName>,
    diagnostics: Vec<Diagnostic>,
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
    use crate::semantic_context::SemanticContextBuilder;
    use crate::test_helpers::parse_and_resolve_types_with_options;

    fn opts_with_oop_extensions() -> CompilerOptions {
        CompilerOptions {
            allow_oop_extensions: true,
            ..CompilerOptions::default()
        }
    }

    #[test]
    fn apply_when_abstract_fb_instantiated_then_error() {
        let program = "
FUNCTION_BLOCK ABSTRACT FB_Base
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_User
VAR
    inst : FB_Base;
END_VAR
END_FUNCTION_BLOCK";
        let (input, _context) =
            parse_and_resolve_types_with_options(program, &opts_with_oop_extensions());
        let context = SemanticContextBuilder::new().build().unwrap();
        let result = apply(&input, &context, &opts_with_oop_extensions());

        let errors = result.unwrap_err();
        assert_eq!(errors.len(), 1);
        assert_eq!(
            Problem::AbstractFunctionBlockInstantiated.code(),
            errors[0].code
        );
    }

    #[test]
    fn apply_when_non_abstract_fb_instantiated_then_ok() {
        let program = "
FUNCTION_BLOCK FB_Base
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_User
VAR
    inst : FB_Base;
END_VAR
END_FUNCTION_BLOCK";
        let (input, _context) =
            parse_and_resolve_types_with_options(program, &opts_with_oop_extensions());
        let context = SemanticContextBuilder::new().build().unwrap();
        let result = apply(&input, &context, &opts_with_oop_extensions());

        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_concrete_subclass_of_abstract_instantiated_then_ok() {
        // A concrete FB that EXTENDS an abstract base can itself still
        // be instantiated -- only the abstract type is flagged.
        let program = "
FUNCTION_BLOCK ABSTRACT FB_Base
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Concrete EXTENDS FB_Base
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_User
VAR
    inst : FB_Concrete;
END_VAR
END_FUNCTION_BLOCK";
        let (input, _context) =
            parse_and_resolve_types_with_options(program, &opts_with_oop_extensions());
        let context = SemanticContextBuilder::new().build().unwrap();
        let result = apply(&input, &context, &opts_with_oop_extensions());

        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_no_abstract_fb_in_library_then_ok() {
        let program = "
FUNCTION_BLOCK FB_Plain
VAR
    x : INT;
END_VAR
END_FUNCTION_BLOCK";
        let (input, _context) =
            parse_and_resolve_types_with_options(program, &opts_with_oop_extensions());
        let context = SemanticContextBuilder::new().build().unwrap();
        let result = apply(&input, &context, &opts_with_oop_extensions());

        assert!(result.is_ok());
    }
}
