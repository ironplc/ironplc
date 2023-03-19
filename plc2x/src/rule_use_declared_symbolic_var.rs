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
use std::path::PathBuf;

use ironplc_dsl::{
    common::*,
    core::{Id, SourcePosition},
    diagnostic::{Diagnostic, Label},
    visitor::{
        visit_function_block_declaration, visit_function_declaration, visit_program_declaration,
        visit_variable_declaration, Visitor,
    },
};

use crate::symbol_table::{self, Key, SymbolTable};

pub fn apply(lib: &Library) -> Result<(), Diagnostic> {
    let mut visitor: SymbolTable<Id, DummyNode> = symbol_table::SymbolTable::new();

    visitor.walk(lib)
}

struct DummyNode {}
impl Key for Id {}

impl Visitor<Diagnostic> for SymbolTable<'_, Id, DummyNode> {
    type Value = ();

    fn visit_function_declaration(&mut self, node: &FunctionDeclaration) -> Result<(), Diagnostic> {
        self.enter();

        self.add(&node.name, DummyNode {});
        let ret = visit_function_declaration(self, node);
        self.exit();
        ret
    }

    fn visit_program_declaration(&mut self, node: &ProgramDeclaration) -> Result<(), Diagnostic> {
        self.enter();
        self.add(&node.type_name, DummyNode {});
        let ret = visit_program_declaration(self, node);
        self.exit();
        ret
    }

    fn visit_function_block_declaration(
        &mut self,
        node: &FunctionBlockDeclaration,
    ) -> Result<(), Diagnostic> {
        self.enter();
        self.add(&node.name, DummyNode {});
        let ret = visit_function_block_declaration(self, node);
        self.exit();
        ret
    }

    fn visit_variable_declaration(&mut self, node: &VarDecl) -> Result<Self::Value, Diagnostic> {
        self.add(&node.name, DummyNode {});
        visit_variable_declaration(self, node)
    }

    fn visit_symbolic_variable(
        &mut self,
        node: &ironplc_dsl::textual::SymbolicVariable,
    ) -> Result<(), Diagnostic> {
        match self.find(&node.name) {
            Some(_) => {
                // We found the variable being referred to
                Ok(())
            }
            None => Err(Diagnostic::new(
                "S0001",
                format!("Variable {} not defined before used", node.name),
                Label::source_loc(
                    PathBuf::default(),
                    node.name.position(),
                    "Undefined variable",
                ),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

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

        let library = parse(program, &PathBuf::default()).unwrap();
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

        let library = parse(program, &PathBuf::default()).unwrap();
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

        let library = parse(program, &PathBuf::default()).unwrap();
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

        let library = parse(program, &PathBuf::default()).unwrap();
        let result = apply(&library);

        assert!(result.is_ok());
    }
}
