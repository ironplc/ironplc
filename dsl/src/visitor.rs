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
//! use ironplc_dsl::dsl::FunctionDeclaration;
//! use ironplc_dsl::visitor::{ Visitor, visit_function_declaration };
//!
//! struct Dummy {}
//! impl Dummy {
//!   fn do_work() {}
//! }
//!
//! impl Visitor<String> for Dummy {
//!     type Value = ();
//!
//!     fn visit_function_declaration(&mut self, node: &FunctionDeclaration) -> Result<Self::Value, String> {
//!         // Do something custom before visiting the FunctionDeclaration node
//!         Dummy::do_work();
//!
//!         // Continue the recursion
//!         visit_function_declaration(self, node)
//!     }
//! }
//! ```

use crate::ast::*;
use crate::dsl::*;
use crate::sfc::Network;

/// Defines a way to recurse into an object in the AST or DSL.
pub trait Acceptor {
    fn accept<V: Visitor<E> + ?Sized, E>(&self, visitor: &mut V) -> Result<V::Value, E>;
}

/// Recurses into a vec of objects.
impl<X> Acceptor for Vec<X>
where
    X: Acceptor,
{
    fn accept<V: Visitor<E> + ?Sized, E>(&self, visitor: &mut V) -> Result<V::Value, E> {
        match self
            .into_iter()
            .map(|x| x.accept(visitor))
            .find(|r| r.is_err())
        {
            Some(err) => {
                // At least one of the items returned an error, so
                // return the first error.
                err
            }
            None => {
                // There were no errors, so return the default value
                Ok(V::Value::default())
            }
        }
    }
}

/// Recurses into an optional object. Does nothing if the option is none.
impl<X> Acceptor for Option<X>
where
    X: Acceptor,
{
    fn accept<V: Visitor<E> + ?Sized, E>(&self, visitor: &mut V) -> Result<V::Value, E> {
        match self.as_ref() {
            Some(x) => x.accept(visitor),
            None => Ok(V::Value::default()),
        }
    }
}

/// Defines a visitor for the object tree. The default visitor recursively
/// walks to visit items in the tree.
pub trait Visitor<E> {
    /// Value produced by this visitor when the result is not an error.
    ///
    /// The returned value is usually not meaningful because no guarantee
    /// is provided when returning from vectors of objects.
    type Value: Default;

    fn walk(&mut self, node: &Library) -> Result<Self::Value, E> {
        Acceptor::accept(&node.elems, self)
    }

    fn visit_enum_declaration(&mut self, node: &EnumerationDeclaration) -> Result<Self::Value, E> {
        visit_enum_declaration(self, node)
    }

    fn visit_function_block_declaration(
        &mut self,
        node: &FunctionBlockDeclaration,
    ) -> Result<Self::Value, E> {
        visit_function_block_declaration(self, node)
    }

    fn visit_function_declaration(&mut self, node: &FunctionDeclaration) -> Result<Self::Value, E> {
        visit_function_declaration(self, node)
    }

    fn visit_program_declaration(&mut self, node: &ProgramDeclaration) -> Result<Self::Value, E> {
        visit_program_declaration(self, node)
    }

    fn visit_located_var_init(&mut self, node: &LocatedVarInit) -> Result<Self::Value, E> {
        todo!()
    }

    fn visit_var_init_decl(&mut self, node: &VarInitDecl) -> Result<Self::Value, E> {
        visit_var_int_decl(self, &node)
    }

    fn visit_type_initializer(&mut self, node: &TypeInitializer) -> Result<Self::Value, E> {
        todo!()
    }

    fn visit_sfc(&mut self, node: &Sfc) -> Result<Self::Value, E> {
        Acceptor::accept(&node.networks, self)
    }

    fn visit_statements(&mut self, node: &Statements) -> Result<Self::Value, E> {
        Acceptor::accept(&node.body, self)
    }

    fn visit_assignment(&mut self, node: &Assignment) -> Result<Self::Value, E> {
        Acceptor::accept(&node.target, self)
    }

    fn visit_if(&mut self, node: &If) -> Result<Self::Value, E> {
        visit_if(self, &node)
    }

    fn visit_compare(&mut self, op: &CompareOp, terms: &Vec<ExprKind>) -> Result<Self::Value, E> {
        visit_compare(self, op, terms)
    }

    fn visit_direct_variable(&mut self, node: &DirectVariable) -> Result<Self::Value, E> {
        todo!()
    }

    fn visit_symbolic_variable(&mut self, node: &SymbolicVariable) -> Result<Self::Value, E> {
        todo!()
    }

    fn visit_fb_call(&mut self, fb_call: &FbCall) -> Result<Self::Value, E> {
        todo!()
    }
}

pub fn visit_enum_declaration<V: Visitor<E> + ?Sized, E>(
    v: &mut V,
    node: &EnumerationDeclaration,
) -> Result<V::Value, E> {
    Acceptor::accept(&node.initializer, v)
}

pub fn visit_function_block_declaration<V: Visitor<E> + ?Sized, E>(
    v: &mut V,
    node: &FunctionBlockDeclaration,
) -> Result<V::Value, E> {
    Acceptor::accept(&node.inouts, v)?;
    Acceptor::accept(&node.outputs, v)?;
    Acceptor::accept(&node.inouts, v)?;
    Acceptor::accept(&node.vars, v)?;
    Acceptor::accept(&node.externals, v)?;
    Acceptor::accept(&node.body, v)
}

pub fn visit_program_declaration<V: Visitor<E> + ?Sized, E>(
    v: &mut V,
    node: &ProgramDeclaration,
) -> Result<V::Value, E> {
    Acceptor::accept(&node.inputs, v)?;
    Acceptor::accept(&node.outputs, v)?;
    Acceptor::accept(&node.inouts, v)?;
    Acceptor::accept(&node.vars, v)?;
    Acceptor::accept(&node.body, v)
}

pub fn visit_function_declaration<V: Visitor<E> + ?Sized, E>(
    v: &mut V,
    node: &FunctionDeclaration,
) -> Result<V::Value, E> {
    Acceptor::accept(&node.inputs, v)?;
    Acceptor::accept(&node.outputs, v)?;
    Acceptor::accept(&node.inouts, v)?;
    Acceptor::accept(&node.vars, v)?;
    Acceptor::accept(&node.externals, v)?;
    Acceptor::accept(&node.body, v)
}

pub fn visit_var_int_decl<V: Visitor<E> + ?Sized, E>(
    v: &mut V,
    node: &VarInitDecl,
) -> Result<V::Value, E> {
    Acceptor::accept(&node.initializer, v)
}

pub fn visit_if<V: Visitor<E> + ?Sized, E>(v: &mut V, node: &If) -> Result<V::Value, E> {
    Acceptor::accept(&node.expr, v)?;
    Acceptor::accept(&node.body, v)?;
    Acceptor::accept(&node.else_body, v)
}

pub fn visit_compare<V: Visitor<E> + ?Sized, E>(
    v: &mut V,
    op: &CompareOp,
    terms: &Vec<ExprKind>,
) -> Result<V::Value, E> {
    Acceptor::accept(terms, v)
}

impl Acceptor for LibraryElement {
    fn accept<V: Visitor<E> + ?Sized, E>(&self, visitor: &mut V) -> Result<V::Value, E> {
        match self {
            LibraryElement::ConfigurationDeclaration(config) => {
                todo!()
            }
            LibraryElement::DataTypeDeclaration(data_type_decl) => {
                Acceptor::accept(data_type_decl, visitor)
            }
            LibraryElement::FunctionBlockDeclaration(func_block_decl) => {
                visitor.visit_function_block_declaration(func_block_decl)
            }
            LibraryElement::FunctionDeclaration(func_decl) => {
                visitor.visit_function_declaration(func_decl)
            }
            LibraryElement::ProgramDeclaration(prog_decl) => {
                visitor.visit_program_declaration(prog_decl)
            }
        }
    }
}

impl Acceptor for EnumerationDeclaration {
    fn accept<V: Visitor<E> + ?Sized, E>(&self, visitor: &mut V) -> Result<V::Value, E> {
        visitor.visit_enum_declaration(self)
    }
}

impl Acceptor for TypeInitializer {
    fn accept<V: Visitor<E> + ?Sized, E>(&self, visitor: &mut V) -> Result<V::Value, E> {
        // TODO don't yet know how to visit these
        Ok(V::Value::default())
    }
}

impl Acceptor for VarInitKind {
    fn accept<V: Visitor<E> + ?Sized, E>(&self, visitor: &mut V) -> Result<V::Value, E> {
        match self {
            VarInitKind::VarInit(init) => visitor.visit_var_init_decl(init),
            VarInitKind::LocatedVarInit(located_var) => visitor.visit_located_var_init(located_var),
        }
    }
}

impl Acceptor for ExprKind {
    fn accept<V: Visitor<E> + ?Sized, E>(&self, visitor: &mut V) -> Result<V::Value, E> {
        match self {
            ExprKind::Compare { op, terms } => visitor.visit_compare(op, terms),
            ExprKind::BinaryOp { ops, terms } => {
                todo!()
            }
            ExprKind::UnaryOp { op, term } => {
                todo!()
            }
            ExprKind::Const(_) => {
                todo!()
            }
            ExprKind::Variable(variable) => Acceptor::accept(variable, visitor),
            ExprKind::Function {
                name,
                param_assignment,
            } => {
                todo!()
            }
        }
    }
}

impl Acceptor for VarInitDecl {
    fn accept<V: Visitor<E> + ?Sized, E>(&self, visitor: &mut V) -> Result<V::Value, E> {
        visitor.visit_var_init_decl(self)
    }
}

impl Acceptor for FunctionBlockBody {
    fn accept<V: Visitor<E> + ?Sized, E>(&self, visitor: &mut V) -> Result<V::Value, E> {
        match self {
            FunctionBlockBody::Sfc(network) => visitor.visit_sfc(network),
            FunctionBlockBody::Statements(stmts) => visitor.visit_statements(stmts),
        }
    }
}

impl Acceptor for StmtKind {
    fn accept<V: Visitor<E> + ?Sized, E>(&self, visitor: &mut V) -> Result<V::Value, E> {
        match self {
            StmtKind::Assignment(node) => visitor.visit_assignment(node),
            StmtKind::If(node) => visitor.visit_if(node),
            StmtKind::FbCall(node) => visitor.visit_fb_call(node),
        }
    }
}

impl Acceptor for Network {
    fn accept<V: Visitor<E> + ?Sized, E>(&self, visitor: &mut V) -> Result<V::Value, E> {
        todo!()
    }
}

impl Acceptor for Variable {
    fn accept<V: Visitor<E> + ?Sized, E>(&self, visitor: &mut V) -> Result<V::Value, E> {
        match self {
            Variable::DirectVariable(var) => visitor.visit_direct_variable(var),
            Variable::SymbolicVariable(var) => visitor.visit_symbolic_variable(var),
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
    use std::fmt::Error;

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

    impl Visitor<Error> for Descender {
        type Value = ();

        fn visit_direct_variable(&mut self, variable: &DirectVariable) -> Result<(), Error> {
            let mut dst = &mut self.names;
            dst.push_back(variable.to_string());
            Ok(())
        }

        fn visit_symbolic_variable(&mut self, var: &SymbolicVariable) -> Result<(), Error> {
            let mut dst = &mut self.names;
            dst.push_back(var.name.clone());
            Ok(())
        }

        fn visit_fb_call(&mut self, fb_call: &FbCall) -> Result<(), Error> {
            let mut dst = &mut self.names;
            dst.push_back(fb_call.name.clone());
            Ok(())
        }
    }

    #[test]
    fn visit_walks_tree() {
        let library = Library {
            elems: vec![LibraryElement::ProgramDeclaration(ProgramDeclaration {
                type_name: String::from("plc_prg"),
                inputs: vec![VarInitDecl::simple("Reset", "BOOL")],
                outputs: vec![],
                inouts: vec![],
                vars: vec![],
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
