//! Semantic rule that `THIS^` and `SUPER^` have something to refer to
//! (OOP extension): `THIS^` only inside a function block (its body, its
//! methods, its property accessors), and `SUPER^` only in a function block
//! that `EXTENDS` another.
//!
//! ## Passes
//!
//! ```ignore
//! FUNCTION_BLOCK FB_Base
//! VAR x : INT; END_VAR
//! END_FUNCTION_BLOCK
//!
//! FUNCTION_BLOCK FB_A EXTENDS FB_Base
//! METHOD M
//!     THIS^.x := SUPER^.x;
//! END_METHOD
//! END_FUNCTION_BLOCK
//! ```
//!
//! ## Fails
//!
//! ```ignore
//! PROGRAM main
//! VAR x : INT; END_VAR
//!     THIS^.x := 1;
//! END_PROGRAM
//! ```
use ironplc_dsl::{
    core::Located,
    diagnostic::{Diagnostic, Label},
    scope::ScopeNode,
    textual::{SelfRefKind, SelfRefVariable},
    visitor::Visitor,
};
use ironplc_parser::options::CompilerOptions;
use ironplc_problems::Problem;
use std::convert::Infallible;

use crate::{
    result::SemanticResult,
    rule_support::{run_rule, DiagnosticVisitor},
    semantic_context::SemanticContext,
    symbol_environment::{ScopeTracker, SymbolEnvironment},
};

pub fn apply(
    lib: &ironplc_dsl::common::Library,
    context: &SemanticContext,
    _options: &CompilerOptions,
) -> SemanticResult {
    run_rule(
        RuleSelfReferenceContext {
            symbols: context.symbols(),
            scope: ScopeTracker::default(),
            diagnostics: Vec::new(),
        },
        lib,
    )
}

struct RuleSelfReferenceContext<'a> {
    symbols: &'a SymbolEnvironment,
    scope: ScopeTracker,
    diagnostics: Vec<Diagnostic>,
}

impl DiagnosticVisitor for RuleSelfReferenceContext<'_> {
    fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

impl Visitor<Infallible> for RuleSelfReferenceContext<'_> {
    type Value = ();

    fn enter_scope(&mut self, node: ScopeNode<'_>) -> Result<(), Infallible> {
        self.scope.enter(&node);
        Ok(())
    }

    fn exit_scope(&mut self) {
        self.scope.exit();
    }

    fn visit_self_ref_variable(&mut self, node: &SelfRefVariable) -> Result<(), Infallible> {
        let scope = self.scope.current();
        if self.symbols.self_type(&scope, node.kind).is_none() {
            // `SUPER^` in a function block that extends nothing differs from
            // a self reference outside any function block.
            let in_block = self.symbols.self_type(&scope, SelfRefKind::This).is_some();
            let reason = if node.kind == SelfRefKind::Super && in_block {
                "SUPER^ needs a function block that EXTENDS another".to_string()
            } else {
                format!(
                    "{} is only valid inside a function block",
                    node.kind.spelling()
                )
            };
            self.diagnostics.push(
                Diagnostic::problem(
                    Problem::SelfReferenceWithoutTarget,
                    Label::span(node.span(), reason),
                )
                .with_context("reference", &node.kind.spelling().to_string()),
            );
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    fn options() -> CompilerOptions {
        CompilerOptions {
            allow_fb_inheritance: true,
            ..CompilerOptions::default()
        }
    }

    fn codes(program: &str) -> Vec<String> {
        let (library, context) =
            crate::test_helpers::parse_and_resolve_types_with_options(program, &options());
        match apply(&library, &context, &options()) {
            Ok(()) => vec![],
            Err(errors) => errors.iter().map(|e| e.code.clone()).collect(),
        }
    }

    const BASE: &str = "
FUNCTION_BLOCK FB_Base
VAR
    x : INT;
END_VAR
METHOD M
END_METHOD
END_FUNCTION_BLOCK
";

    #[rstest]
    #[case::this_in_body(
        "FUNCTION_BLOCK FB_A\nVAR y : INT; END_VAR\n    THIS^.y := 1;\nEND_FUNCTION_BLOCK"
    )]
    #[case::this_in_method(
        "FUNCTION_BLOCK FB_A\nVAR y : INT; END_VAR\nMETHOD N\n    THIS^.y := 1;\nEND_METHOD\nEND_FUNCTION_BLOCK"
    )]
    #[case::super_in_derived_method(
        "FUNCTION_BLOCK FB_A EXTENDS FB_Base\nMETHOD N\n    SUPER^.x := 1;\n    SUPER^.M();\nEND_METHOD\nEND_FUNCTION_BLOCK"
    )]
    #[case::this_in_property_accessor(
        "FUNCTION_BLOCK FB_A\nVAR y : INT; END_VAR\nPROPERTY P : INT\nGET\n    P := THIS^.y;\nEND_GET\nEND_PROPERTY\nEND_FUNCTION_BLOCK"
    )]
    fn apply_when_self_reference_has_target_then_ok(#[case] source: &str) {
        assert!(codes(&format!("{BASE}\n{source}")).is_empty());
    }

    #[rstest]
    #[case::this_in_program("PROGRAM main\nVAR y : INT; END_VAR\n    THIS^.y := 1;\nEND_PROGRAM")]
    #[case::this_in_function("FUNCTION F : INT\n    F := THIS^.x;\nEND_FUNCTION")]
    #[case::super_without_base(
        "FUNCTION_BLOCK FB_A\nVAR y : INT; END_VAR\nMETHOD N\n    SUPER^.y := 1;\nEND_METHOD\nEND_FUNCTION_BLOCK"
    )]
    #[case::super_call_without_base(
        "FUNCTION_BLOCK FB_A\nMETHOD N\n    SUPER^.M();\nEND_METHOD\nEND_FUNCTION_BLOCK"
    )]
    fn apply_when_self_reference_has_no_target_then_error(#[case] source: &str) {
        assert_eq!(
            vec![Problem::SelfReferenceWithoutTarget.code().to_string()],
            codes(&format!("{BASE}\n{source}"))
        );
    }
}
