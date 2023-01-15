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
    core::Id,
    dsl::*,
    visitor::{
        visit_function_block_declaration, visit_function_declaration, visit_program_declaration,
        visit_variable_declaration, Visitor,
    },
};

use crate::{
    error::SemanticDiagnostic,
    symbol_table::{self, Key, NodeData, SymbolTable},
};

pub fn apply(lib: &Library) -> Result<(), SemanticDiagnostic> {
    let mut visitor: SymbolTable<Id, DummyNode> = symbol_table::SymbolTable::new();

    visitor.walk(lib)
}

#[derive(Clone)]
struct DummyNode {}
impl NodeData for DummyNode {}
impl Key for Id {}

impl Visitor<SemanticDiagnostic> for SymbolTable<Id, DummyNode> {
    type Value = ();

    fn visit_function_declaration(
        &mut self,
        node: &FunctionDeclaration,
    ) -> Result<(), SemanticDiagnostic> {
        self.enter();

        self.add(&node.name, DummyNode {});
        let ret = visit_function_declaration(self, node);
        self.exit();
        ret
    }

    fn visit_program_declaration(
        &mut self,
        node: &ProgramDeclaration,
    ) -> Result<(), SemanticDiagnostic> {
        self.enter();
        self.add(&node.type_name, DummyNode {});
        let ret = visit_program_declaration(self, node);
        self.exit();
        ret
    }

    fn visit_function_block_declaration(
        &mut self,
        node: &FunctionBlockDeclaration,
    ) -> Result<(), SemanticDiagnostic> {
        self.enter();
        self.add(&node.name, DummyNode {});
        let ret = visit_function_block_declaration(self, node);
        self.exit();
        ret
    }

    fn visit_variable_declaration(
        &mut self,
        node: &VarDecl,
    ) -> Result<Self::Value, SemanticDiagnostic> {
        self.add(&node.name, DummyNode {});
        visit_variable_declaration(self, node)
    }

    fn visit_symbolic_variable(
        &mut self,
        node: &ironplc_dsl::ast::SymbolicVariable,
    ) -> Result<(), SemanticDiagnostic> {
        match self.find(&node.name) {
            Some(_) => {
                // We found the variable being referred to
                Ok(())
            }
            None => Err(SemanticDiagnostic::error(
                "S0001",
                format!("Variable {} not defined before used", node.name),
            )
            .with_label(node.name.location(), "Undefined variable")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stages::parse;

    #[test]
    fn apply_when_function_block_undeclared_symbol_then_error() {
        let program = "
FUNCTION_BLOCK LOGGER
VAR
TRIG0 : BOOL;
END_VAR
         
TRIG := TRIG0;
END_FUNCTION_BLOCK";

        let library = parse(program).unwrap();
        let result = apply(&library);

        assert!(result.is_err());
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

        let library = parse(program).unwrap();
        let result = apply(&library);

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

        let library = parse(program).unwrap();
        let result = apply(&library);

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

        let library = parse(program).unwrap();
        let result = apply(&library);

        assert!(result.is_ok());
    }
}
