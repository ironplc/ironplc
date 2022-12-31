//! A set of traits and functions for visiting all nodes in a library.
//!
//! To use the visitor, define a struct and implement the Visitor trait
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

    fn visit_configuration_declaration(
        &mut self,
        node: &ConfigurationDeclaration,
    ) -> Result<Self::Value, E> {
        visit_configuration_declaration(self, node)
    }

    fn visit_resource_declaration(&mut self, node: &ResourceDeclaration) -> Result<Self::Value, E> {
        visit_resource_declaration(self, node)
    }

    fn visit_declaration(&mut self, node: &Declaration) -> Result<Self::Value, E> {
        visit_declaration(self, node)
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
        visit_var_init_decl(self, &node)
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

    fn visit_binary_op(
        &mut self,
        op: &Vec<Operator>,
        terms: &Vec<ExprKind>,
    ) -> Result<Self::Value, E> {
        // TODO this doesn't really go through binary operators - maybe this should be split
        // into smaller pieces.
        visit_binary_op(self, op, terms)
    }

    fn visit_unary_op(&mut self, op: &UnaryOp, term: &ExprKind) -> Result<Self::Value, E> {
        visit_unary_op(self, op, term)
    }

    fn visit_direct_variable(&mut self, node: &DirectVariable) -> Result<Self::Value, E> {
        todo!()
    }

    fn visit_symbolic_variable(&mut self, node: &SymbolicVariable) -> Result<Self::Value, E> {
        // leaf node - no children
        Ok(Self::Value::default())
    }

    fn visit_fb_call(&mut self, fb_call: &FbCall) -> Result<Self::Value, E> {
        // TODO
        Ok(Self::Value::default())
    }

    fn visit_enumerated_type_initializer(
        &mut self,
        init: &EnumeratedTypeInitializer,
    ) -> Result<Self::Value, E> {
        // leaf node - no children
        Ok(Self::Value::default())
    }
}

pub fn visit_configuration_declaration<V: Visitor<E> + ?Sized, E>(
    v: &mut V,
    node: &ConfigurationDeclaration,
) -> Result<V::Value, E> {
    Acceptor::accept(&node.global_var, v)?;
    Acceptor::accept(&node.resource_decl, v)
}

pub fn visit_resource_declaration<V: Visitor<E> + ?Sized, E>(
    v: &mut V,
    node: &ResourceDeclaration,
) -> Result<V::Value, E> {
    Acceptor::accept(&node.global_vars, v)
    // TODO there are more child elements here
}

pub fn visit_declaration<V: Visitor<E> + ?Sized, E>(
    v: &mut V,
    node: &Declaration,
) -> Result<V::Value, E> {
    Acceptor::accept(&node.initializer, v)
}

pub fn visit_enum_declaration<V: Visitor<E> + ?Sized, E>(
    v: &mut V,
    node: &EnumerationDeclaration,
) -> Result<V::Value, E> {
    Acceptor::accept(&node.spec, v)
}

pub fn visit_function_block_declaration<V: Visitor<E> + ?Sized, E>(
    v: &mut V,
    node: &FunctionBlockDeclaration,
) -> Result<V::Value, E> {
    Acceptor::accept(&node.inputs, v)?;
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

pub fn visit_var_init_decl<V: Visitor<E> + ?Sized, E>(
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

pub fn visit_binary_op<V: Visitor<E> + ?Sized, E>(
    v: &mut V,
    op: &Vec<Operator>,
    terms: &Vec<ExprKind>,
) -> Result<V::Value, E> {
    // TODO maybe something with the operator?
    Acceptor::accept(terms, v)
}

pub fn visit_unary_op<V: Visitor<E> + ?Sized, E>(
    v: &mut V,
    op: &UnaryOp,
    term: &ExprKind,
) -> Result<V::Value, E> {
    // TODO maybe something with the operator?
    Acceptor::accept(term, v)
}

impl Acceptor for LibraryElement {
    fn accept<V: Visitor<E> + ?Sized, E>(&self, visitor: &mut V) -> Result<V::Value, E> {
        match self {
            LibraryElement::ConfigurationDeclaration(config) => {
                visitor.visit_configuration_declaration(config)
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

impl Acceptor for ResourceDeclaration {
    fn accept<V: Visitor<E> + ?Sized, E>(&self, visitor: &mut V) -> Result<V::Value, E> {
        visitor.visit_resource_declaration(self)
    }
}

impl Acceptor for Declaration {
    fn accept<V: Visitor<E> + ?Sized, E>(&self, visitor: &mut V) -> Result<V::Value, E> {
        visitor.visit_declaration(self)
    }
}

impl Acceptor for EnumerationDeclaration {
    fn accept<V: Visitor<E> + ?Sized, E>(&self, visitor: &mut V) -> Result<V::Value, E> {
        visitor.visit_enum_declaration(self)
    }
}

impl Acceptor for EnumeratedSpecificationKind {
    fn accept<V: Visitor<E> + ?Sized, E>(&self, visitor: &mut V) -> Result<V::Value, E> {
        // TODO I don't know if we need to visit these items
        Ok(V::Value::default())
    }
}

impl Acceptor for TypeInitializer {
    fn accept<V: Visitor<E> + ?Sized, E>(&self, visitor: &mut V) -> Result<V::Value, E> {
        match self {
            TypeInitializer::Simple {
                type_name,
                initial_value,
            } => Ok(V::Value::default()),
            TypeInitializer::EnumeratedValues { values, default } => Ok(V::Value::default()),
            TypeInitializer::EnumeratedType(et) => visitor.visit_enumerated_type_initializer(et),
            TypeInitializer::FunctionBlock { type_name } => Ok(V::Value::default()),
            TypeInitializer::Structure { type_name } => Ok(V::Value::default()),
            TypeInitializer::LateResolvedType(_) => Ok(V::Value::default()),
        }
        // TODO don't yet know how to visit these
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
            ExprKind::BinaryOp { ops, terms } => visitor.visit_binary_op(ops, terms),
            ExprKind::UnaryOp { op, term } => visitor.visit_unary_op(op, term.as_ref()),
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
            // TODO it isn't clear if visiting this is necessary
            FunctionBlockBody::Empty() => Ok(V::Value::default()),
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
            dst.push_back(var.name.to_string());
            Ok(())
        }

        fn visit_fb_call(&mut self, fb_call: &FbCall) -> Result<(), Error> {
            let mut dst = &mut self.names;
            dst.push_back(fb_call.var_name.to_string());
            Ok(())
        }
    }

    #[test]
    fn walk_when_has_symbolic_variable_then_visits_variable() {
        let library = Library {
            elems: vec![LibraryElement::ProgramDeclaration(ProgramDeclaration {
                type_name: Id::from("plc_prg"),
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
