//! Semantic rule that reference in a function block, function or program to
//! a symbolic variable must be to a symbolic variable that is
//! declared in that scope.
//!
//! ## Passes
//! 
//! FUNCTION_BLOCK LOGGER
//!    VAR
//!       TRIG : BOOL;
//!       TRIG0 : BOOL;
//!    END_VAR
//! 
//!    TRIG := TRIG0;
//! END_FUNCTION_BLOCK
//!   
//! ## Fails
//! 
//! FUNCTION_BLOCK LOGGER
//!    VAR
//!       TRIG0 : BOOL;
//!    END_VAR
//! 
//!    TRIG := TRIG0;
//! END_FUNCTION_BLOCK
use ironplc_dsl::{
    ast::Id,
    dsl::*,
    visitor::{
        visit_function_block_declaration, visit_function_declaration, visit_program_declaration,
        visit_var_init_decl, Visitor,
    },
};

use crate::symbol_table::{self, Key, NodeData, SymbolTable};

pub fn apply(lib: &Library) -> Result<(), String> {
    let mut visitor: SymbolTable<Id, DummyNode> = symbol_table::SymbolTable::new();

    visitor.walk(&lib)
}

#[derive(Clone)]
struct DummyNode {}
impl NodeData for DummyNode {}
impl Key for Id {}

impl Visitor<String> for SymbolTable<Id, DummyNode> {
    type Value = ();

    fn visit_function_declaration(
        &mut self,
        func_decl: &FunctionDeclaration,
    ) -> Result<(), String> {
        self.enter();
        let ret = visit_function_declaration(self, func_decl);
        self.exit();
        ret
    }

    fn visit_program_declaration(&mut self, prog_decl: &ProgramDeclaration) -> Result<(), String> {
        self.enter();
        let ret = visit_program_declaration(self, prog_decl);
        self.exit();
        ret
    }

    fn visit_function_block_declaration(
        &mut self,
        func_decl: &FunctionBlockDeclaration,
    ) -> Result<(), String> {
        self.enter();
        let ret = visit_function_block_declaration(self, func_decl);
        self.exit();
        ret
    }

    fn visit_var_init_decl(&mut self, node: &VarInitDecl) -> Result<Self::Value, String> {
        self.add(&node.name, DummyNode {});
        visit_var_init_decl(self, node)
    }

    fn visit_symbolic_variable(
        &mut self,
        node: &ironplc_dsl::ast::SymbolicVariable,
    ) -> Result<(), String> {
        match self.find(&node.name) {
            Some(_) => {
                // We found the variable being referred to
                Ok(Self::Value::default())
            }
            None => Err(format!("Variable {} not defined before used", node.name)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stages::parse;

    #[test]
    fn apply_when_undeclared_symbol_then_error() {
        let program = "
FUNCTION_BLOCK LOGGER
VAR
TRIG0 : BOOL;
END_VAR
         
TRIG := TRIG0;
END_FUNCTION_BLOCK";
        
        let library = parse(program).unwrap();
        let result = apply(&library);

        assert_eq!(true, result.is_err());
        assert_eq!("Variable TRIG not defined before used", result.unwrap_err());
    }

    #[test]
    fn apply_when_all_symbol_declared_then_ok() {
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

        assert_eq!(true, result.is_ok());
    }
}
