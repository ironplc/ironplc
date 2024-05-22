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
    common::*,
    core::{Id, SourcePosition},
    diagnostic::{Diagnostic, Label},
    visitor::Visitor,
};
use ironplc_problems::Problem;

use crate::{
    result::SemanticResult,
    symbol_table::{self, Key, SymbolTable, Value},
};

pub fn apply(lib: &Library) -> SemanticResult {
    let mut visitor: SymbolTable<Id, DummyNode> = symbol_table::SymbolTable::new();

    visitor.walk(lib).map_err(|e| vec![e])
}

#[derive(Debug)]
struct DummyNode {}
impl Value for DummyNode {}

impl Key for Id {}

impl Visitor<Diagnostic> for SymbolTable<'_, Id, DummyNode> {
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
        self.add(&node.type_name, DummyNode {});
        let ret = node.recurse_visit(self);
        self.exit();
        ret
    }

    fn visit_function_block_declaration(
        &mut self,
        node: &FunctionBlockDeclaration,
    ) -> Result<(), Diagnostic> {
        self.enter();
        self.add(&node.name, DummyNode {});
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
                Label::span(node.name.position(), "Undefined variable"),
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
         
TRIG := TRIG0;
END_FUNCTION_BLOCK";

        let library = parse_and_resolve_types(program);
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

        let library = parse_and_resolve_types(program);
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

        let library = parse_and_resolve_types(program);
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

        let library = parse_and_resolve_types(program);
        let result = apply(&library);

        assert!(result.is_ok());
    }
}
