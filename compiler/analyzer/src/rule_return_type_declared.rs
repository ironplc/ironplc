//! Semantic rule that the return type of a function or method is a declared
//! type.
//!
//! A variable of an undeclared type is reported while its initializer is
//! resolved (P2008), but nothing resolves a return type, so without this rule
//! an undeclared one is accepted and compiled.
//!
//! ## Passes
//!
//! ```ignore
//! TYPE
//!     E_Mode : (Idle, Running);
//! END_TYPE
//!
//! FUNCTION GetMode : E_Mode
//!     GetMode := Idle;
//! END_FUNCTION
//! ```
//!
//! ## Fails
//!
//! ```ignore
//! FUNCTION GetMode : E_Mode
//!     GetMode := 0;
//! END_FUNCTION
//! ```
use ironplc_dsl::{
    common::*,
    core::Located,
    diagnostic::{Diagnostic, Label},
    visitor::Visitor,
};
use ironplc_problems::Problem;
use std::convert::Infallible;

use crate::{
    result::SemanticResult,
    rule_support::{run_rule, DiagnosticVisitor},
    semantic_context::SemanticContext,
    type_environment::TypeEnvironment,
};
use ironplc_parser::options::CompilerOptions;

pub fn apply(
    lib: &Library,
    context: &SemanticContext,
    _options: &CompilerOptions,
) -> SemanticResult {
    run_rule(
        RuleReturnTypeDeclared {
            types: context.types(),
            diagnostics: Vec::new(),
        },
        lib,
    )
}

struct RuleReturnTypeDeclared<'a> {
    types: &'a TypeEnvironment,
    diagnostics: Vec<Diagnostic>,
}

impl RuleReturnTypeDeclared<'_> {
    /// Reports `return_type` when it names a type the environment does not
    /// hold. `STRING`/`WSTRING` return types are always declared.
    fn check(&mut self, return_type: &FunctionReturnType) {
        let FunctionReturnType::Named(type_name) = return_type else {
            return;
        };
        if self.types.get(type_name).is_none() {
            self.diagnostics.push(Diagnostic::problem(
                Problem::ReturnTypeNotDeclared,
                Label::span(type_name.span(), "Return type"),
            ));
        }
    }
}

impl DiagnosticVisitor for RuleReturnTypeDeclared<'_> {
    fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

impl Visitor<Infallible> for RuleReturnTypeDeclared<'_> {
    type Value = ();

    fn visit_function_declaration(
        &mut self,
        node: &FunctionDeclaration,
    ) -> Result<Self::Value, Infallible> {
        self.check(&node.return_type);
        node.recurse_visit(self)
    }

    fn visit_method_declaration(
        &mut self,
        node: &MethodDeclaration,
    ) -> Result<Self::Value, Infallible> {
        if let Some(return_type) = &node.return_type {
            self.check(return_type);
        }
        node.recurse_visit(self)
    }
}

#[cfg(test)]
mod tests {
    use super::apply;
    use crate::test_helpers::parse_and_resolve_types_with_options;
    use ironplc_parser::options::CompilerOptions;
    use ironplc_problems::Problem;

    /// The codes this rule reports for `program`, checked against the
    /// resolved type environment (a fresh context would hold no types).
    fn codes(program: &str) -> Vec<String> {
        let options = CompilerOptions {
            allow_fb_inheritance: true,
            ..CompilerOptions::default()
        };
        let (library, context) = parse_and_resolve_types_with_options(program, &options);
        match apply(&library, &context, &options) {
            Ok(()) => vec![],
            Err(diagnostics) => diagnostics.into_iter().map(|d| d.code).collect(),
        }
    }

    #[test]
    fn apply_when_function_return_type_undeclared_then_p2042() {
        let codes = codes(
            "
FUNCTION F : E_Missing
F := 0;
END_FUNCTION",
        );
        assert_eq!(codes, vec![Problem::ReturnTypeNotDeclared.code()]);
    }

    #[test]
    fn apply_when_method_return_type_undeclared_then_p2042() {
        let codes = codes(
            "
FUNCTION_BLOCK FB_A
METHOD Move : E_Missing
Move := 0;
END_METHOD
END_FUNCTION_BLOCK",
        );
        assert_eq!(codes, vec![Problem::ReturnTypeNotDeclared.code()]);
    }

    #[test]
    fn apply_when_return_types_declared_then_ok() {
        let codes = codes(
            "
TYPE
    E_Mode : (Idle, Running);
END_TYPE

FUNCTION F_Elementary : LREAL
F_Elementary := 1.0;
END_FUNCTION

FUNCTION F_Enum : E_Mode
F_Enum := Idle;
END_FUNCTION

FUNCTION F_String : STRING[20]
F_String := 'ok';
END_FUNCTION

FUNCTION F_AliasDeclaredLater : T_Count
F_AliasDeclaredLater := 1;
END_FUNCTION

TYPE
    T_Count : DINT;
END_TYPE

FUNCTION_BLOCK FB_Result
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_A
VAR
    n : INT;
END_VAR
METHOD Advance
n := n + 1;
END_METHOD
METHOD Make : FB_Result
;
END_METHOD
END_FUNCTION_BLOCK",
        );
        assert!(codes.is_empty(), "{codes:?}");
    }
}
