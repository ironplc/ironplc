//! Semantic rule that reference in a function block, function or program to
//! a symbolic variable must be to a symbolic variable that is
//! declared in that scope.
//!
//! ## Passes
//!
//! ```ignore
//! FUNCTION_BLOCK LOGGER
//!    VAR
//!       TRIG : BOOL;
//!       TRIG0 : BOOL;
//!    END_VAR
//!
//!    TRIG := TRIG0;
//! END_FUNCTION_BLOCK
//! ```
//!
//! ```ignore
//! TYPE
//!     MyColors: (Red, Green);
//! END_TYPE
//! FUNCTION_BLOCK
//!     VAR
//!         Color: MyColors := Red;
//!     END_VAR
//!     Color := Green;
//! END_FUNCTION_BLOCK
//! ```
//!   
//! ## Fails
//!
//! ```ignore
//! FUNCTION_BLOCK LOGGER
//!    VAR
//!       TRIG0 : BOOL;
//!    END_VAR
//!
//!    TRIG := TRIG0;
//! END_FUNCTION_BLOCK
//! ```
use ironplc_dsl::{
    common::*,
    core::{Id, Located},
    diagnostic::{Diagnostic, Label},
    visitor::Visitor,
};
use ironplc_problems::Problem;

use crate::{
    result::SemanticResult,
    scoped_table::{self, Key, ScopedTable, Value},
    semantic_context::SemanticContext,
    string_similarity::find_closest_match,
};

pub fn apply(lib: &Library, _context: &SemanticContext) -> SemanticResult {
    let mut visitor: ScopedTable<Id, DummyNode> = scoped_table::ScopedTable::new();

    visitor.walk(lib).map_err(|e| vec![e])
}

#[derive(Debug)]
struct DummyNode {}
impl Value for DummyNode {}

impl Key for Id {}
impl Key for TypeName {}

impl Visitor<Diagnostic> for ScopedTable<'_, Id, DummyNode> {
    type Value = ();

    fn visit_function_declaration(&mut self, node: &FunctionDeclaration) -> Result<(), Diagnostic> {
        self.enter();

        self.add(&node.name, DummyNode {});
        let ret = node.recurse_visit(self);
        self.exit();
        ret
    }

    fn visit_program_declaration(&mut self, node: &ProgramDeclaration) -> Result<(), Diagnostic> {
        self.enter();
        self.add(&node.name, DummyNode {});
        let ret = node.recurse_visit(self);
        self.exit();
        ret
    }

    fn visit_function_block_declaration(
        &mut self,
        node: &FunctionBlockDeclaration,
    ) -> Result<(), Diagnostic> {
        self.enter();
        self.add(&node.name.name, DummyNode {});
        let ret = node.recurse_visit(self);
        self.exit();
        ret
    }

    fn visit_var_decl(&mut self, node: &VarDecl) -> Result<Self::Value, Diagnostic> {
        self.add_if(node.identifier.symbolic_id(), DummyNode {});
        node.recurse_visit(self)
    }

    fn visit_named_variable(
        &mut self,
        node: &ironplc_dsl::textual::NamedVariable,
    ) -> Result<(), Diagnostic> {
        match self.find(&node.name) {
            Some(_) => {
                // We found the variable being referred to
                Ok(())
            }
            None => {
                let suggestion = find_closest_match(
                    node.name.original(),
                    self.keys().iter().map(|k| k.original().as_str()),
                );
                let mut diagnostic = Diagnostic::problem(
                    Problem::VariableUndefined,
                    Label::span(node.name.span(), "Undefined variable"),
                )
                .with_context_id("variable", &node.name);
                if let Some(suggestion) = suggestion {
                    diagnostic = diagnostic.with_context("did you mean", &suggestion);
                }
                Err(diagnostic)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::semantic_context::SemanticContextBuilder;
    use crate::test_helpers::parse_and_resolve_types;

    use super::*;

    #[test]
    fn apply_when_function_block_undeclared_symbol_then_error() {
        let program = "
FUNCTION_BLOCK LOGGER
VAR
TRIG0 : BOOL;
END_VAR
         
TRIG := TRIG0.A;
END_FUNCTION_BLOCK";

        let library = parse_and_resolve_types(program);
        let context = SemanticContextBuilder::new().build().unwrap();
        let result = apply(&library, &context);

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .first()
            .unwrap()
            .described
            .contains(&"variable=TRIG".to_owned()))
    }

    #[test]
    fn apply_when_function_block_all_symbol_declared_then_ok() {
        let program = "
FUNCTION_BLOCK LOGGER
VAR
TRIG : BOOL;
TRIG0 : BOOL;
END_VAR
         
TRIG := TRIG0;
END_FUNCTION_BLOCK";

        let library = parse_and_resolve_types(program);
        let context = SemanticContextBuilder::new().build().unwrap();
        let result = apply(&library, &context);

        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_function_all_symbol_declared_then_ok() {
        let program = "
FUNCTION LOGGER : REAL
VAR_INPUT
TRIG : BOOL;
TRIG0 : BOOL;
END_VAR
         
TRIG := TRIG0;
END_FUNCTION";

        let library = parse_and_resolve_types(program);
        let context = SemanticContextBuilder::new().build().unwrap();
        let result = apply(&library, &context);

        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_program_all_symbol_declared_then_ok() {
        let program = "
PROGRAM LOGGER
VAR
TRIG : BOOL;
TRIG0 : BOOL;
END_VAR
         
TRIG := TRIG0;
END_PROGRAM";

        let library = parse_and_resolve_types(program);
        let context = SemanticContextBuilder::new().build().unwrap();
        let result = apply(&library, &context);

        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_assign_enum_variant_then_ok() {
        let program = "
TYPE
    MyColors: (Red, Green);
END_TYPE

FUNCTION_BLOCK FB_EXAMPLE
    VAR
        Color: MyColors := Red;
    END_VAR
    Color := Green;
END_FUNCTION_BLOCK";

        let library = parse_and_resolve_types(program);
        let context = SemanticContextBuilder::new().build().unwrap();
        let result = apply(&library, &context);

        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_typo_in_variable_name_then_suggests_closest_match() {
        let program = "
FUNCTION_BLOCK LOGGER
VAR
counter : INT;
END_VAR

conter := 1;
END_FUNCTION_BLOCK";

        let library = parse_and_resolve_types(program);
        let context = SemanticContextBuilder::new().build().unwrap();
        let result = apply(&library, &context);

        assert!(result.is_err());
        let errors = result.unwrap_err();
        let error = errors.first().unwrap();
        assert!(error.described.contains(&"variable=conter".to_owned()));
        assert!(error.described.contains(&"did you mean=counter".to_owned()));
    }

    #[test]
    fn apply_when_no_similar_variable_then_no_suggestion() {
        let program = "
FUNCTION_BLOCK LOGGER
VAR
x : INT;
END_VAR

completely_different := 1;
END_FUNCTION_BLOCK";

        let library = parse_and_resolve_types(program);
        let context = SemanticContextBuilder::new().build().unwrap();
        let result = apply(&library, &context);

        assert!(result.is_err());
        let errors = result.unwrap_err();
        let error = errors.first().unwrap();
        assert!(error
            .described
            .contains(&"variable=completely_different".to_owned()));
        assert!(!error
            .described
            .iter()
            .any(|d| d.starts_with("did you mean")));
    }
}
