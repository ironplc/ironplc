//! A set of traits and functions for visiting all nodes in a library.
//! To use the visitor, define a struct and implement the Visitor train
//! for the struct.
//!
//! Visitor trait functions call functions that implement walking through
//! the library. Selectively call this functions to selectively descend
//! into the library.
//!
//! # Example
//!
//! ```
//! struct Dummy {}
//!
//! impl Visitor for Dummy {
//!     fn visit_function_declaration(&mut self, func_decl: &FunctionDeclaration) {
//!         // Do something custom before visiting the FunctionDeclaration node
//!         visit_function_declaration(self, node);
//!     }
//! }
//! ```

use crate::ast::*;
use crate::dsl::*;
use crate::sfc::Network;

pub trait Acceptor {
    fn accept<V: Visitor + ?Sized>(&self, visitor: &mut V);
}

impl<X> Acceptor for Vec<X>
where
    X: Acceptor,
{
    fn accept<V: Visitor + ?Sized>(&self, visitor: &mut V) {
        self.into_iter().for_each(|x| x.accept(visitor));
    }
}

impl<X> Acceptor for Option<X>
where
    X: Acceptor,
{
    fn accept<V: Visitor + ?Sized>(&self, visitor: &mut V) {
        match self.as_ref() {
            Some(x) => x.accept(visitor),
            None => {}
        };
    }
}

pub trait Visitor {
    fn walk(&mut self, node: &Library) {
        Acceptor::accept(&node.elems, self)
    }

    fn visit_enum_declaration(&mut self, enum_decl: &EnumerationDeclaration) {
        visit_enum_declaration(self, enum_decl);
    }

    fn visit_function_block_declaration(&mut self, func_block_decl: &FunctionBlockDeclaration) {
        Acceptor::accept(&func_block_decl.var_decls, self);
        Acceptor::accept(&func_block_decl.body, self);
    }

    fn visit_function_declaration(&mut self, func_decl: &FunctionDeclaration) {
        visit_function_declaration(self, func_decl);
    }

    fn visit_program_declaration(&mut self, prog_decl: &ProgramDeclaration) {
        Acceptor::accept(&prog_decl.var_declarations, self);
        Acceptor::accept(&prog_decl.body, self);
    }

    fn visit_located_var_init(&mut self, var_init: &LocatedVarInit) {}

    fn visit_var_init_decl(&mut self, var_init: &VarInitDecl) {}

    fn visit_sfc(&mut self, sfc: &Sfc) {
        Acceptor::accept(&sfc.networks, self);
    }

    fn visit_statements(&mut self, statements: &Statements) {
        Acceptor::accept(&statements.body, self);
    }

    fn visit_assignment(&mut self, assignment: &Assignment) {
        Acceptor::accept(&assignment.target, self);
    }

    fn visit_direct_variable(&mut self, variable: &DirectVariable) {}

    fn visit_symbolic_variable(&mut self, variable: &SymbolicVariable) {}

    fn visit_fb_call(&mut self, fb_call: &FbCall) {}
}

pub fn visit_enum_declaration<V: Visitor + ?Sized>(v: &mut V, node: &EnumerationDeclaration) {}

pub fn visit_function_block_declaration<V: Visitor + ?Sized>(
    v: &mut V,
    node: &FunctionBlockDeclaration,
) {
    Acceptor::accept(&node.var_decls, v);
    Acceptor::accept(&node.body, v);
}

pub fn visit_function_declaration<V: Visitor + ?Sized>(v: &mut V, func_decl: &FunctionDeclaration) {
    Acceptor::accept(&func_decl.var_decls, v);
    Acceptor::accept(&func_decl.body, v);
}

pub fn visit_program_declaration<V: Visitor + ?Sized>(v: &mut V, prog_decl: &ProgramDeclaration) {
    Acceptor::accept(&prog_decl.var_declarations, v);
    Acceptor::accept(&prog_decl.body, v);
}

impl Acceptor for LibraryElement {
    fn accept<V: Visitor + ?Sized>(&self, visitor: &mut V) {
        match self {
            LibraryElement::ConfigurationDeclaration(config) => {}
            LibraryElement::DataTypeDeclaration(data_type_decl) => {
                Acceptor::accept(data_type_decl, visitor);
            }
            LibraryElement::FunctionBlockDeclaration(func_block_decl) => {
                visitor.visit_function_block_declaration(func_block_decl);
            }
            LibraryElement::FunctionDeclaration(func_decl) => {
                visitor.visit_function_declaration(func_decl);
            }
            LibraryElement::ProgramDeclaration(prog_decl) => {
                visitor.visit_program_declaration(prog_decl);
            }
        }
    }
}

impl Acceptor for EnumerationDeclaration {
    fn accept<V: Visitor + ?Sized>(&self, visitor: &mut V) {
        visitor.visit_enum_declaration(self);
    }
}

impl Acceptor for VarInitKind {
    fn accept<V: Visitor + ?Sized>(&self, visitor: &mut V) {
        match self {
            VarInitKind::VarInit(init) => {
                visitor.visit_var_init_decl(init);
            }
            VarInitKind::LocatedVarInit(located_var) => {
                visitor.visit_located_var_init(located_var);
            }
        }
    }
}

impl Acceptor for VarInitDecl {
    fn accept<V: Visitor + ?Sized>(&self, visitor: &mut V) {
        visitor.visit_var_init_decl(self);
    }
}

impl Acceptor for FunctionBlockBody {
    fn accept<V: Visitor + ?Sized>(&self, visitor: &mut V) {
        match self {
            FunctionBlockBody::Sfc(network) => {
                visitor.visit_sfc(network);
            }
            FunctionBlockBody::Statements(stmts) => {
                visitor.visit_statements(stmts);
            }
        }
    }
}

impl Acceptor for StmtKind {
    fn accept<V: Visitor + ?Sized>(&self, visitor: &mut V) {
        match self {
            StmtKind::Assignment(assignment) => {
                visitor.visit_assignment(assignment);
            }
            StmtKind::If {
                expr,
                body,
                else_body,
            } => {}
            StmtKind::FbCall(fb_call) => {
                visitor.visit_fb_call(fb_call);
            }
        }
    }
}

impl Acceptor for Network {
    fn accept<V: Visitor + ?Sized>(&self, visitor: &mut V) {
        // TODO
    }
}

impl Acceptor for Variable {
    fn accept<V: Visitor + ?Sized>(&self, visitor: &mut V) {
        match self {
            Variable::DirectVariable(var) => {
                visitor.visit_direct_variable(var);
            }
            Variable::SymbolicVariable(var) => {
                visitor.visit_symbolic_variable(var);
            }
            Variable::MultiElementVariable(_) => {
                todo!()
            }
        }
    }
}

mod test {
    use super::*;
    use crate::ast::*;
    use crate::dsl::*;
    use std::collections::LinkedList;

    struct Descender {
        names: LinkedList<String>,
    }
    impl Descender {
        fn new() -> Descender {
            Descender {
                names: LinkedList::new(),
            }
        }
    }

    impl Visitor for Descender {
        fn visit_direct_variable(&mut self, variable: &DirectVariable) {
            let mut dst = &mut self.names;
            dst.push_back(variable.to_string());
        }

        fn visit_symbolic_variable(&mut self, var: &SymbolicVariable) {
            let mut dst = &mut self.names;
            dst.push_back(var.name.clone());
        }

        fn visit_fb_call(&mut self, fb_call: &FbCall) {
            let mut dst = &mut self.names;
            dst.push_back(fb_call.name.clone());
        }
    }

    #[test]
    fn visit_walks_tree() {
        let library = Library {
            elems: vec![LibraryElement::ProgramDeclaration(ProgramDeclaration {
                type_name: String::from("plc_prg"),
                var_declarations: vec![VarInitKind::VarInit(VarInitDecl::simple("Reset", "BOOL"))],
                body: FunctionBlockBody::stmts(vec![StmtKind::fb_assign(
                    "AverageVal",
                    vec!["Cnt1", "Cnt2"],
                    "_TMP_AverageVal17_OUT",
                )]),
            })],
        };

        let mut descender = Descender::new();

        descender.walk(&library);

        assert_eq!(1, descender.names.len())
    }
}
