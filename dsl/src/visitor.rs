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
//! impl Dummy {
//!   fn do_work() {}
//! }
//!
//! impl Visitor for Dummy {
//!     fn visit_function_declaration(&mut self, func_decl: &FunctionDeclaration) {
//!         // Do something custom before visiting the FunctionDeclaration node
//!         self.do_work();
//!
//!         // Continue the recursion
//!         visit_function_declaration(self, node);
//!     }
//! }
//! ```

use crate::ast::*;
use crate::dsl::*;
use crate::sfc::Network;

/// Defines a way to recurse into an object in the AST or DSL.
pub trait Acceptor {
    fn accept<V: Visitor + ?Sized>(&self, visitor: &mut V);
}

/// Recurses into a vec of objects.
impl<X> Acceptor for Vec<X>
where
    X: Acceptor,
{
    fn accept<V: Visitor + ?Sized>(&self, visitor: &mut V) {
        self.into_iter().for_each(|x| x.accept(visitor));
    }
}

/// Recurses into an optional object. Does nothing if the option is none.
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

/// Defines a visitor for the object tree. The default visitor recursively
/// walks to visit items in the tree.
pub trait Visitor {
    fn walk(&mut self, node: &Library) {
        Acceptor::accept(&node.elems, self)
    }

    fn visit_enum_declaration(&mut self, node: &EnumerationDeclaration) {
        visit_enum_declaration(self, node);
    }

    fn visit_function_block_declaration(&mut self, node: &FunctionBlockDeclaration) {
        visit_function_block_declaration(self, node)
    }

    fn visit_function_declaration(&mut self, node: &FunctionDeclaration) {
        visit_function_declaration(self, node);
    }

    fn visit_program_declaration(&mut self, node: &ProgramDeclaration) {
        visit_program_declaration(self, node);
    }

    fn visit_located_var_init(&mut self, node: &LocatedVarInit) {}

    fn visit_var_init_decl(&mut self, node: &VarInitDecl) {}

    fn visit_type_initializer(&mut self, node: &TypeInitializer) {}

    fn visit_sfc(&mut self, node: &Sfc) {
        Acceptor::accept(&node.networks, self);
    }

    fn visit_statements(&mut self, node: &Statements) {
        Acceptor::accept(&node.body, self);
    }

    fn visit_assignment(&mut self, node: &Assignment) {
        Acceptor::accept(&node.target, self);
    }

    fn visit_if(&mut self, node: &If) {
        visit_if(self, &node);
    }

    fn visit_compare(&mut self, op: &CompareOp, terms: &Vec<ExprKind>) {
        visit_compare(self, op, terms);
    }

    fn visit_direct_variable(&mut self, node: &DirectVariable) {}

    fn visit_symbolic_variable(&mut self, node: &SymbolicVariable) {}

    fn visit_fb_call(&mut self, fb_call: &FbCall) {}
}

pub fn visit_enum_declaration<V: Visitor + ?Sized>(v: &mut V, node: &EnumerationDeclaration) {
    Acceptor::accept(&node.initializer, v);
}

pub fn visit_function_block_declaration<V: Visitor + ?Sized>(
    v: &mut V,
    node: &FunctionBlockDeclaration,
) {
    Acceptor::accept(&node.var_decls, v);
    Acceptor::accept(&node.body, v);
}

pub fn visit_program_declaration<V: Visitor + ?Sized>(v: &mut V, node: &ProgramDeclaration) {
    Acceptor::accept(&node.var_declarations, v);
    Acceptor::accept(&node.body, v);
}

pub fn visit_function_declaration<V: Visitor + ?Sized>(v: &mut V, node: &FunctionDeclaration) {
    Acceptor::accept(&node.var_decls, v);
    Acceptor::accept(&node.body, v);
}

pub fn visit_if<V: Visitor + ?Sized>(v: &mut V, node: &If) {
    Acceptor::accept(&node.expr, v);
    Acceptor::accept(&node.body, v);
    Acceptor::accept(&node.else_body, v);
}

pub fn visit_compare<V: Visitor + ?Sized>(v: &mut V, op: &CompareOp, terms: &Vec<ExprKind>) {
    Acceptor::accept(terms, v);
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

impl Acceptor for TypeInitializer {
    fn accept<V: Visitor + ?Sized>(&self, visitor: &mut V) {
        visitor.visit_type_initializer(self);
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

impl Acceptor for ExprKind {
    fn accept<V: Visitor + ?Sized>(&self, visitor: &mut V) {
        match self {
            ExprKind::Compare { op, terms } =>  {
                visitor.visit_compare(op, terms);
            },
            ExprKind::BinaryOp { ops, terms } => {todo!()},
            ExprKind::UnaryOp { op, term } => {todo!()},
            ExprKind::Const(_) => {todo!()},
            ExprKind::Variable(variable) => {
                Acceptor::accept(variable, visitor);
            },
            ExprKind::Function { name, param_assignment } => {todo!()},
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
            StmtKind::Assignment(node) => {
                visitor.visit_assignment(node);
            }
            StmtKind::If(node) => {
                visitor.visit_if(node);
            }
            StmtKind::FbCall(node) => {
                visitor.visit_fb_call(node);
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
