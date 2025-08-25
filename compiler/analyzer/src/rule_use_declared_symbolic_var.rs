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
    symbol_environment::SymbolEnvironment,
    type_environment::TypeEnvironment,
};

pub fn apply(
    lib: &Library,
    _type_environment: &TypeEnvironment,
    _symbol_environment: &SymbolEnvironment,
) -> SemanticResult {
    let mut visitor: ScopedTable<Id, DummyNode> = scoped_table::ScopedTable::new();

    visitor.walk(lib).map_err(|e| vec![e])
}

#[derive(Debug)]
struct DummyNode {}
impl Value for DummyNode {}

impl Key for Id {}
impl Key for Type {}

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
            None => Err(Diagnostic::problem(
                Problem::VariableUndefined,
                Label::span(node.name.span(), "Undefined variable"),
            )
            .with_context_id("variable", &node.name)),
        }
    }
}

#[cfg(test)]
mod tests {
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
        let type_env = TypeEnvironment::new();
        let symbol_env = SymbolEnvironment::new();
        let result = apply(&library, &type_env, &symbol_env);

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
        let type_env = TypeEnvironment::new();
        let symbol_env = SymbolEnvironment::new();
        let result = apply(&library, &type_env, &symbol_env);

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
        let type_env = TypeEnvironment::new();
        let symbol_env = SymbolEnvironment::new();
        let result = apply(&library, &type_env, &symbol_env);

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
        let type_env = TypeEnvironment::new();
        let symbol_env = SymbolEnvironment::new();
        let result = apply(&library, &type_env, &symbol_env);

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
        let type_env = TypeEnvironment::new();
        let symbol_env = SymbolEnvironment::new();
        let result = apply(&library, &type_env, &symbol_env);

        assert!(result.is_ok());
    }
}
