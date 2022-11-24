use std::{collections::HashMap, fmt::Error};

use ironplc_dsl::{
    dsl::*,
    visitor::{
        visit_function_block_declaration, visit_function_declaration, visit_program_declaration,
        Visitor,
    },
};

use crate::symbol_table::{self, NodeData, SymbolTable};

pub fn apply(lib: Library) -> Result<(), String> {
    let mut visitor: SymbolTable<DummyNode> = symbol_table::SymbolTable::new();

    visitor.walk(&lib)
}

#[derive(Clone)]
struct DummyNode {}
impl NodeData for DummyNode {}

impl Visitor<String> for SymbolTable<DummyNode> {
    type Value = ();

    fn visit_function_declaration(&mut self, func_decl: &FunctionDeclaration) -> Result<(), String> {
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

    fn visit_function_block_declaration(&mut self, func_decl: &FunctionBlockDeclaration) -> Result<(), String> {
        self.enter();
        let ret = visit_function_block_declaration(self, func_decl);
        self.exit();
        ret
    }

    fn visit_symbolic_variable(&mut self, node: &ironplc_dsl::ast::SymbolicVariable) -> Result<(), String> {
        match self.find(&node.name.as_str()) {
            Some(_) => {
                // We found the variable being referred to
                Ok(Self::Value::default())
            }
            None => {
                Err(format!("Variable {} not defined before used", node.name))
            },
        }
    }
}

struct GlobalTypeDefinitionVisitor<'a> {
    types: &'a mut HashMap<String, TypeDefinitionKind>,
}

struct UseDeclaredChecker {
    symbol_table: symbol_table::SymbolTable<DummyNode>,
}
impl UseDeclaredChecker {}

#[cfg(test)]
mod tests {
    use ironplc_dsl::ast::*;
    use ironplc_dsl::dsl::*;

    use super::*;

    use crate::test_helpers::new_library;

    #[test]
    fn test_identifies_undeclared_symbol() {
        let input = new_library::<String>(LibraryElement::FunctionBlockDeclaration(
            FunctionBlockDeclaration {
                name: String::from("LOGGER"),
                var_decls: vec![],
                body: FunctionBlockBody::stmts(vec![StmtKind::if_then(
                    ExprKind::Compare {
                        op: CompareOp::And,
                        terms: vec![
                            ExprKind::symbolic_variable("TRIG"),
                            ExprKind::UnaryOp {
                                op: UnaryOp::Not,
                                term: ExprKind::boxed_symbolic_variable("TRIG0"),
                            },
                        ],
                    },
                    vec![],
                )]),
            },
        ))
        .unwrap();

        let result = apply(input);
        assert_eq!(true, result.is_err());
        assert_eq!("Variable TRIG not defined before used", result.unwrap_err());
    }
}
