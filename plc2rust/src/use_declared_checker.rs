use std::collections::HashMap;

use ironplc_dsl::{dsl::*, visitor::{Visitor, visit_function_block_declaration, visit_function_declaration, visit_program_declaration}};

use crate::symbol_table::{self, NodeData, SymbolTable};

pub fn apply(lib: Library) {
    let mut visitor: SymbolTable<DummyNode> = symbol_table::SymbolTable::new();

    visitor.walk(&lib);
}

#[derive(Clone)]
struct DummyNode {}
impl NodeData for DummyNode {}

impl Visitor for SymbolTable<DummyNode> {
    fn visit_function_declaration(&mut self, func_decl: &FunctionDeclaration) {
        self.enter();

        visit_function_declaration(self, func_decl);

        self.exit();
    }

    fn visit_program_declaration(&mut self, prog_decl: &ProgramDeclaration) {
        self.enter();
        
        visit_program_declaration(self, prog_decl);
    
        self.exit();
    }

    fn visit_function_block_declaration(&mut self, func_decl: &FunctionBlockDeclaration) {
        self.enter();
    
        visit_function_block_declaration(self, func_decl);

        self.exit();
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
        todo!("need to move to functions so that don't need to duplicate code when changing the implementation");
        let input = new_library::<String>(LibraryElement::FunctionBlockDeclaration(
            FunctionBlockDeclaration {
                name: String::from("LOGGER"),
                var_decls: vec![],
                body: FunctionBlockBody::stmts(vec![StmtKind::If {
                    expr: ExprKind::Compare {
                        op: CompareOp::And,
                        terms: vec![
                            ExprKind::symbolic_variable("TRIG"),
                            ExprKind::UnaryOp {
                                op: UnaryOp::Not,
                                term: ExprKind::boxed_symbolic_variable("TRIG0"),
                            },
                        ],
                    },
                    body: vec![],
                    else_body: vec![],
                },]),
            },
        ))
        .unwrap();

        apply(input);

        assert_eq!(false, true)
    }
}
