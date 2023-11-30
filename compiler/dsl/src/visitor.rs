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
//! use ironplc_dsl::common::FunctionDeclaration;
//! use ironplc_dsl::diagnostic::Diagnostic;
//! use ironplc_dsl::visitor::{ Visitor, visit_function_declaration };
//!
//! struct Dummy {}
//! impl Dummy {
//!   fn do_work() {}
//! }
//!
//! impl Visitor<Diagnostic> for Dummy {
//!     type Value = ();
//!
//!     fn visit_function_declaration(&mut self, node: &FunctionDeclaration) -> Result<Self::Value, Diagnostic> {
//!         // Do something custom before visiting the FunctionDeclaration node
//!         Dummy::do_work();
//!
//!         // Continue the recursion
//!         visit_function_declaration(self, &node)
//!     }
//! }
//! ```

use crate::common::*;
use crate::configuration::*;
use crate::core::Id;
use crate::diagnostic::Diagnostic;
use crate::sfc::*;
use crate::textual::*;
use paste::paste;

/// Defines a macro for the `Visitor` trait that dispatches visiting
/// to a function. In other words, creates a function of the form:
///
/// ```ignore
///
/// fn visit_type_name(&mut self, node: &TypeName) -> Result<Self::Value, E> {
///    visit_type_name(self, node)
/// }
/// ```
///
///  The visitor generally dispatches to a dedicated function so that
/// implementations can re-use the behavior.
macro_rules! dispatch {
    ($struct_name:ident) => {
        paste! {
            fn [<visit_ $struct_name:snake >](&mut self, node: &$struct_name) -> Result<Self::Value, E> {
                [< visit_ $struct_name:snake >](self, &node)
            }
        }
    };
}

/// Defines a macro for the `Visitor` trait that returns `Ok`.
/// In other words, creates a function of the form:
///
/// ```ignore
/// fn visit_type_name(&mut self, node: &TypeName) -> Result<Self::Value, E> {
///    Ok(Self::Value::default())
/// }
/// ```
///
///  The visitor generally dispatches to a dedicated function so that
/// implementations can re-use the behavior.
macro_rules! leaf {
    ($struct_name:ident) => {
        paste! {
            fn [<visit_ $struct_name:snake >](&mut self, node: &$struct_name) -> Result<Self::Value, E> {
                Ok(Self::Value::default())
            }
        }
    };
}

/// Defines a way to recurse into an object in the AST or DSL.
pub trait Acceptor {
    fn accept<V: Visitor<E> + ?Sized, E: From<Diagnostic>>(
        &self,
        visitor: &mut V,
    ) -> Result<V::Value, E>;
}

/// Recurses into a vec of objects.
impl<X> Acceptor for Vec<X>
where
    X: Acceptor,
{
    fn accept<V: Visitor<E> + ?Sized, E: From<Diagnostic>>(
        &self,
        visitor: &mut V,
    ) -> Result<V::Value, E> {
        match self.iter().map(|x| x.accept(visitor)).find(|r| r.is_err()) {
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
    fn accept<V: Visitor<E> + ?Sized, E: From<Diagnostic>>(
        &self,
        visitor: &mut V,
    ) -> Result<V::Value, E> {
        match self.as_ref() {
            Some(x) => x.accept(visitor),
            None => Ok(V::Value::default()),
        }
    }
}

/// Defines a macro for the `Acceptor` trait that dispatches to the visitor.
/// (The `Acceptor` trait defines a handler for lists and optionals
/// of 61131-3 elements.)
///
/// In other words, creates a train implementation of the form
///
/// ```ignore
/// impl Acceptor for TypeName {
///    fn accept<V: Visitor<E> + ?Sized, E: From<Diagnostic>>(
///       &self,
///       visitor: &mut V,
///    ) -> Result<V::Value, E> {
///       visitor.visit_type_name(self)
///    }
/// }
/// ```
///
///  The visitor generally dispatches to a dedicated function so that
/// implementations can re-use the behavior.
macro_rules! acceptor_impl {
    ($struct_name:ident) => {
        paste! {
            impl Acceptor for $struct_name {
                fn accept<V: Visitor<E> + ?Sized, E: From<Diagnostic>>(
                    &self,
                    visitor: &mut V,
                ) -> Result<V::Value, E> {
                    visitor.[<visit_ $struct_name:snake >](self)
                }
            }
        }
    };
}

/// Defines a visitor for the object tree. The default visitor recursively
/// walks to visit items in the tree.
///
/// Functions in the visitor are named based snake-case variant of the element
/// name. For example, the `Id` element's visitor function is `visit_id`.
pub trait Visitor<E: std::convert::From<Diagnostic>> {
    /// Value produced by this visitor when the result is not an error.
    ///
    /// The returned value is usually not meaningful because no guarantee
    /// is provided when returning from vectors of objects.
    type Value: Default;

    fn walk(&mut self, node: &Library) -> Result<Self::Value, E> {
        Acceptor::accept(&node.elements, self)
    }

    // Declarations from Core

    // 2.1.2.
    leaf!(Id);

    // Declarations from Common

    // TODO Constants
    leaf!(SignedInteger);

    dispatch!(ConstantKind);

    // 2.3.3.1
    dispatch!(DataTypeDeclarationKind);

    // 2.3.3.1
    dispatch!(LateBoundDeclaration);

    // 2.3.3.1
    dispatch!(EnumerationDeclaration);

    dispatch!(InitialValueAssignmentKind);
    dispatch!(StructInitialValueAssignmentKind);

    // 2.4.3.2
    leaf!(EnumeratedSpecificationInit);

    // 2.4.3.2
    leaf!(EnumeratedSpecificationValues);

    // 2.3.3.1
    leaf!(EnumeratedValue);

    // 2.3.3.1
    dispatch!(SubrangeDeclaration);

    // 2.3.3.1
    dispatch!(SubrangeSpecificationKind);

    // 2.3.3.1
    dispatch!(SubrangeSpecification);

    // 2.3.3.1
    dispatch!(SimpleDeclaration);

    // 2.3.3.1
    dispatch!(ArrayDeclaration);

    dispatch!(ArrayInitialElementKind);

    // 2.3.3.1
    dispatch!(StructureDeclaration);

    // 2.3.3.1
    dispatch!(StructureElementDeclaration);

    // 2.3.3.1
    dispatch!(StructureInitializationDeclaration);

    // 2.3.3.1
    dispatch!(StructureElementInit);

    // 2.3.3.1
    dispatch!(StringDeclaration);

    dispatch!(ArraySpecificationKind);

    // 2.4.2.1
    leaf!(Subrange);

    // 2.4.3
    dispatch!(VarDecl);

    // 2.4.3.1
    leaf!(AddressAssignment);

    // 2.4.3.2
    dispatch!(SimpleInitializer);

    // 2.4.3.1 and 2.4.3.2
    leaf!(StringInitializer);

    // 2.4.3.2
    leaf!(EnumeratedValuesInitializer);

    // 2.4.3.2
    leaf!(FunctionBlockInitialValueAssignment);

    // 2.4.3.2.
    dispatch!(ArrayInitialValueAssignment);

    // 2.4.3.2 (TODO - where?)
    dispatch!(EnumeratedSpecificationKind);

    dispatch!(EnumeratedInitialValueAssignment);

    dispatch!(VariableIdentifier);

    dispatch!(LibraryElementKind);

    // 2.5.1
    dispatch!(FunctionDeclaration);

    // 2.5.2
    dispatch!(FunctionBlockDeclaration);

    dispatch!(FunctionBlockBodyKind);

    // 2.5.3
    dispatch!(ProgramDeclaration);

    // Declarations from Sfc

    // 2.6
    dispatch!(Sfc);

    // 2.6
    dispatch!(Network);

    // 2.6.2
    dispatch!(ElementKind);

    // 2.6.2
    leaf!(Step);

    // 2.6.3
    dispatch!(Transition);

    // 2.6.4
    dispatch!(Action);

    // Declarations from Configuration

    // 2.7.1
    dispatch!(ResourceDeclaration);

    // 2.7.1
    dispatch!(ProgramConfiguration);

    // 2.7.2
    dispatch!(ConfigurationDeclaration);

    // 2.7.2
    leaf!(TaskConfiguration);

    // Declarations from Textual

    // 3
    dispatch!(Statements);

    dispatch!(Variable);

    dispatch!(SymbolicVariableKind);

    // 3.2.3
    dispatch!(FbCall);

    // 3.2.3
    dispatch!(PositionalInput);

    // 3.2.3
    dispatch!(NamedInput);

    // 3.2.3
    dispatch!(Output);

    // 3.2.3
    dispatch!(ParamAssignmentKind);

    dispatch!(StmtKind);

    // 3.3.1
    dispatch!(CompareExpr);

    // 3.3.1
    dispatch!(BinaryExpr);

    // 3.3.1
    dispatch!(UnaryExpr);

    dispatch!(Function);

    dispatch!(ExprKind);

    // 3.3.2.1
    dispatch!(Assignment);

    // 3.3.2.3
    dispatch!(If);

    // 3.3.2.3
    dispatch!(Case);

    // 3.3.2.3
    dispatch!(CaseStatementGroup);

    // 3.3.2.3
    dispatch!(CaseSelectionKind);

    dispatch!(For);

    dispatch!(While);

    dispatch!(Repeat);

    leaf!(NamedVariable);

    dispatch!(ArrayVariable);
}

pub fn visit_constant_kind<V: Visitor<E> + ?Sized, E: From<Diagnostic>>(
    v: &mut V,
    node: &ConstantKind,
) -> Result<V::Value, E> {
    match node {
        // TODO visit the values
        ConstantKind::IntegerLiteral(node) => Ok(V::Value::default()),
        ConstantKind::RealLiteral(node) => Ok(V::Value::default()),
        ConstantKind::CharacterString() => Ok(V::Value::default()),
        ConstantKind::Duration(node) => Ok(V::Value::default()),
        ConstantKind::TimeOfDay() => Ok(V::Value::default()),
        ConstantKind::Date() => Ok(V::Value::default()),
        ConstantKind::DateAndTime() => Ok(V::Value::default()),
        ConstantKind::Boolean(node) => Ok(V::Value::default()),
        ConstantKind::BitStringLiteral(node) => Ok(V::Value::default()),
    }
}

pub fn visit_data_type_declaration_kind<V: Visitor<E> + ?Sized, E: From<Diagnostic>>(
    v: &mut V,
    node: &DataTypeDeclarationKind,
) -> Result<V::Value, E> {
    match node {
        DataTypeDeclarationKind::Enumeration(e) => v.visit_enumeration_declaration(e),
        DataTypeDeclarationKind::Subrange(sr) => v.visit_subrange_declaration(sr),
        DataTypeDeclarationKind::Simple(simple) => v.visit_simple_declaration(simple),
        DataTypeDeclarationKind::Array(a) => v.visit_array_declaration(a),
        DataTypeDeclarationKind::Structure(s) => v.visit_structure_declaration(s),
        DataTypeDeclarationKind::StructureInitialization(si) => {
            v.visit_structure_initialization_declaration(si)
        }
        DataTypeDeclarationKind::String(s) => v.visit_string_declaration(s),
        DataTypeDeclarationKind::LateBound(bound) => v.visit_late_bound_declaration(bound),
    }
}

pub fn visit_late_bound_declaration<V: Visitor<E> + ?Sized, E: From<Diagnostic>>(
    v: &mut V,
    node: &LateBoundDeclaration,
) -> Result<V::Value, E> {
    v.visit_id(&node.data_type_name)?;
    v.visit_id(&node.base_type_name)
}

pub fn visit_enumeration_declaration<V: Visitor<E> + ?Sized, E: From<Diagnostic>>(
    v: &mut V,
    node: &EnumerationDeclaration,
) -> Result<V::Value, E> {
    v.visit_id(&node.type_name)?;
    v.visit_enumerated_specification_init(&node.spec_init)
}

pub fn visit_enumerated_specification_init<V: Visitor<E> + ?Sized, E: From<Diagnostic>>(
    v: &mut V,
    node: &EnumeratedSpecificationInit,
) -> Result<V::Value, E> {
    v.visit_enumerated_specification_kind(&node.spec)?;
    node.default.as_ref().map_or_else(
        || Ok(V::Value::default()),
        |val| v.visit_enumerated_value(val),
    )
}

// 2.3.3.1
pub fn visit_subrange_declaration<V: Visitor<E> + ?Sized, E: From<Diagnostic>>(
    v: &mut V,
    node: &SubrangeDeclaration,
) -> Result<V::Value, E> {
    v.visit_id(&node.type_name)?;
    v.visit_subrange_specification_kind(&node.spec)?;
    node.default.as_ref().map_or_else(
        || Ok(V::Value::default()),
        |val| v.visit_signed_integer(val),
    )
}

// 2.3.3.1
pub fn visit_subrange_specification_kind<V: Visitor<E> + ?Sized, E: From<Diagnostic>>(
    v: &mut V,
    node: &SubrangeSpecificationKind,
) -> Result<V::Value, E> {
    match node {
        SubrangeSpecificationKind::Specification(node) => v.visit_subrange_specification(node),
        SubrangeSpecificationKind::Type(node) => v.visit_id(node),
    }
}

pub fn visit_subrange_specification<V: Visitor<E> + ?Sized, E: From<Diagnostic>>(
    v: &mut V,
    node: &SubrangeSpecification,
) -> Result<V::Value, E> {
    // TODO type name
    v.visit_subrange(&node.subrange)
}

pub fn visit_simple_declaration<V: Visitor<E> + ?Sized, E: From<Diagnostic>>(
    v: &mut V,
    node: &SimpleDeclaration,
) -> Result<V::Value, E> {
    v.visit_id(&node.type_name)?;
    v.visit_initial_value_assignment_kind(&node.spec_and_init)
}

pub fn visit_array_declaration<V: Visitor<E> + ?Sized, E: From<Diagnostic>>(
    v: &mut V,
    node: &ArrayDeclaration,
) -> Result<V::Value, E> {
    v.visit_id(&node.type_name)?;
    v.visit_array_specification_kind(&node.spec)?;
    Acceptor::accept(&node.init, v)
}

pub fn visit_array_initial_element_kind<V: Visitor<E> + ?Sized, E: From<Diagnostic>>(
    v: &mut V,
    node: &ArrayInitialElementKind,
) -> Result<V::Value, E> {
    match node {
        ArrayInitialElementKind::Constant(node) => v.visit_constant_kind(node),
        ArrayInitialElementKind::EnumValue(node) => v.visit_enumerated_value(node),
        ArrayInitialElementKind::Repeated(_, init) => {
            // TODO visit the int
            Acceptor::accept(init.as_ref(), v)
        }
    }
}

pub fn visit_structure_declaration<V: Visitor<E> + ?Sized, E: From<Diagnostic>>(
    v: &mut V,
    node: &StructureDeclaration,
) -> Result<V::Value, E> {
    v.visit_id(&node.type_name)?;
    Acceptor::accept(&node.elements, v)
}

pub fn visit_structure_element_declaration<V: Visitor<E> + ?Sized, E: From<Diagnostic>>(
    v: &mut V,
    node: &StructureElementDeclaration,
) -> Result<V::Value, E> {
    v.visit_id(&node.name)?;
    v.visit_initial_value_assignment_kind(&node.init)
}

pub fn visit_structure_initialization_declaration<V: Visitor<E> + ?Sized, E: From<Diagnostic>>(
    v: &mut V,
    node: &StructureInitializationDeclaration,
) -> Result<V::Value, E> {
    Acceptor::accept(&node.elements_init, v)
}

pub fn visit_structure_element_init<V: Visitor<E> + ?Sized, E: From<Diagnostic>>(
    v: &mut V,
    node: &StructureElementInit,
) -> Result<V::Value, E> {
    v.visit_id(&node.name)?;
    v.visit_struct_initial_value_assignment_kind(&node.init)
}

pub fn visit_initial_value_assignment_kind<V: Visitor<E> + ?Sized, E: From<Diagnostic>>(
    v: &mut V,
    node: &InitialValueAssignmentKind,
) -> Result<V::Value, E> {
    match node {
        // TODO
        InitialValueAssignmentKind::None => Ok(V::Value::default()),
        InitialValueAssignmentKind::Simple(si) => v.visit_simple_initializer(si),
        InitialValueAssignmentKind::String(str) => v.visit_string_initializer(str),
        InitialValueAssignmentKind::EnumeratedValues(ev) => {
            v.visit_enumerated_values_initializer(ev)
        }
        InitialValueAssignmentKind::EnumeratedType(et) => {
            v.visit_enumerated_initial_value_assignment(et)
        }
        InitialValueAssignmentKind::FunctionBlock(fbi) => {
            v.visit_function_block_initial_value_assignment(fbi)
        }
        InitialValueAssignmentKind::Subrange(node) => v.visit_subrange_specification_kind(node),
        InitialValueAssignmentKind::Structure(si) => {
            v.visit_structure_initialization_declaration(si)
        }
        InitialValueAssignmentKind::Array(array_init) => {
            v.visit_array_initial_value_assignment(array_init)
        }
        InitialValueAssignmentKind::LateResolvedType(_) => Ok(V::Value::default()),
    }
}

pub fn visit_struct_initial_value_assignment_kind<V: Visitor<E> + ?Sized, E: From<Diagnostic>>(
    v: &mut V,
    node: &StructInitialValueAssignmentKind,
) -> Result<V::Value, E> {
    match node {
        StructInitialValueAssignmentKind::Constant(node) => v.visit_constant_kind(node),
        StructInitialValueAssignmentKind::EnumeratedValue(_) => {
            Err(Into::<E>::into(Diagnostic::todo(file!(), line!())))
        }
        StructInitialValueAssignmentKind::Array(_) => {
            Err(Into::<E>::into(Diagnostic::todo(file!(), line!())))
        }
        StructInitialValueAssignmentKind::Structure(_) => {
            Err(Into::<E>::into(Diagnostic::todo(file!(), line!())))
        }
    }
}

pub fn visit_string_declaration<V: Visitor<E> + ?Sized, E: From<Diagnostic>>(
    v: &mut V,
    node: &StringDeclaration,
) -> Result<V::Value, E> {
    v.visit_id(&node.type_name)?;
    // TODO more items here
    Ok(V::Value::default())
}

pub fn visit_array_specification_kind<V: Visitor<E> + ?Sized, E: From<Diagnostic>>(
    v: &mut V,
    node: &ArraySpecificationKind,
) -> Result<V::Value, E> {
    match node {
        ArraySpecificationKind::Type(node) => v.visit_id(node),
        ArraySpecificationKind::Subranges(subranges, node) => {
            v.visit_id(node)?;
            Acceptor::accept(subranges, v)
        }
    }
}

pub fn visit_var_decl<V: Visitor<E> + ?Sized, E: From<Diagnostic>>(
    v: &mut V,
    node: &VarDecl,
) -> Result<V::Value, E> {
    // TODO there are children here
    v.visit_variable_identifier(&node.identifier)?;
    v.visit_initial_value_assignment_kind(&node.initializer)
}

// 2.4.3.2
pub fn visit_simple_initializer<V: Visitor<E> + ?Sized, E: From<Diagnostic>>(
    v: &mut V,
    node: &SimpleInitializer,
) -> Result<V::Value, E> {
    Acceptor::accept(&node.initial_value, v)
}

pub fn visit_array_initial_value_assignment<V: Visitor<E> + ?Sized, E: From<Diagnostic>>(
    v: &mut V,
    node: &ArrayInitialValueAssignment,
) -> Result<V::Value, E> {
    v.visit_array_specification_kind(&node.spec)?;
    Acceptor::accept(&node.initial_values, v)
}

pub fn visit_enumerated_specification_kind<V: Visitor<E> + ?Sized, E: From<Diagnostic>>(
    v: &mut V,
    node: &EnumeratedSpecificationKind,
) -> Result<V::Value, E> {
    match node {
        EnumeratedSpecificationKind::TypeName(node) => v.visit_id(node),
        EnumeratedSpecificationKind::Values(node) => v.visit_enumerated_specification_values(node),
    }
}

// 2.4.3.2
pub fn visit_enumerated_initial_value_assignment<V: Visitor<E> + ?Sized, E: From<Diagnostic>>(
    v: &mut V,
    node: &EnumeratedInitialValueAssignment,
) -> Result<V::Value, E> {
    v.visit_id(&node.type_name)?;
    Acceptor::accept(&node.initial_value, v)
}

pub fn visit_variable_identifier<V: Visitor<E> + ?Sized, E: From<Diagnostic>>(
    v: &mut V,
    node: &VariableIdentifier,
) -> Result<V::Value, E> {
    match node {
        VariableIdentifier::Symbol(node) => v.visit_id(node),
        VariableIdentifier::Direct(node, assignment) => {
            Acceptor::accept(node, v)?;
            v.visit_address_assignment(assignment)
        }
    }
}

pub fn visit_library_element_kind<V: Visitor<E> + ?Sized, E: From<Diagnostic>>(
    v: &mut V,
    node: &LibraryElementKind,
) -> Result<V::Value, E> {
    match node {
        LibraryElementKind::ConfigurationDeclaration(config) => {
            v.visit_configuration_declaration(config)
        }
        LibraryElementKind::DataTypeDeclaration(data_type_decl) => {
            Acceptor::accept(data_type_decl, v)
        }
        LibraryElementKind::FunctionBlockDeclaration(func_block_decl) => {
            v.visit_function_block_declaration(func_block_decl)
        }
        LibraryElementKind::FunctionDeclaration(func_decl) => {
            v.visit_function_declaration(func_decl)
        }
        LibraryElementKind::ProgramDeclaration(prog_decl) => v.visit_program_declaration(prog_decl),
    }
}

pub fn visit_function_declaration<V: Visitor<E> + ?Sized, E: From<Diagnostic>>(
    v: &mut V,
    node: &FunctionDeclaration,
) -> Result<V::Value, E> {
    v.visit_id(&node.name)?;
    v.visit_id(&node.return_type)?;
    Acceptor::accept(&node.variables, v)?;
    Acceptor::accept(&node.body, v)
}

pub fn visit_function_block_declaration<V: Visitor<E> + ?Sized, E: From<Diagnostic>>(
    v: &mut V,
    node: &FunctionBlockDeclaration,
) -> Result<V::Value, E> {
    v.visit_id(&node.name)?;
    Acceptor::accept(&node.variables, v)?;
    v.visit_function_block_body_kind(&node.body)
}

pub fn visit_function_block_body_kind<V: Visitor<E> + ?Sized, E: From<Diagnostic>>(
    v: &mut V,
    node: &FunctionBlockBodyKind,
) -> Result<V::Value, E> {
    match node {
        FunctionBlockBodyKind::Sfc(network) => v.visit_sfc(network),
        FunctionBlockBodyKind::Statements(stmts) => v.visit_statements(stmts),
        FunctionBlockBodyKind::Empty() => Ok(V::Value::default()),
    }
}

pub fn visit_program_declaration<V: Visitor<E> + ?Sized, E: From<Diagnostic>>(
    v: &mut V,
    node: &ProgramDeclaration,
) -> Result<V::Value, E> {
    v.visit_id(&node.type_name)?;
    Acceptor::accept(&node.variables, v)?;
    v.visit_function_block_body_kind(&node.body)
}

pub fn visit_sfc<V: Visitor<E> + ?Sized, E: From<Diagnostic>>(
    v: &mut V,
    node: &Sfc,
) -> Result<V::Value, E> {
    Acceptor::accept(&node.networks, v)
}

// 2.6.2
pub fn visit_network<V: Visitor<E> + ?Sized, E: From<Diagnostic>>(
    v: &mut V,
    node: &Network,
) -> Result<V::Value, E> {
    v.visit_step(&node.initial_step)?;
    Acceptor::accept(&node.elements, v)
}

pub fn visit_element_kind<V: Visitor<E> + ?Sized, E: From<Diagnostic>>(
    v: &mut V,
    node: &ElementKind,
) -> Result<V::Value, E> {
    match node {
        ElementKind::Step(node) => v.visit_step(node),
        ElementKind::Transition(node) => v.visit_transition(node),
        ElementKind::Action(node) => v.visit_action(node),
    }
}

// 2.6.3
pub fn visit_transition<V: Visitor<E> + ?Sized, E: From<Diagnostic>>(
    v: &mut V,
    node: &Transition,
) -> Result<V::Value, E> {
    Acceptor::accept(&node.name, v)?;
    Acceptor::accept(&node.from, v)?;
    Acceptor::accept(&node.to, v)?;
    v.visit_expr_kind(&node.condition)
}

// 2.6.3
pub fn visit_action<V: Visitor<E> + ?Sized, E: From<Diagnostic>>(
    v: &mut V,
    node: &Action,
) -> Result<V::Value, E> {
    v.visit_id(&node.name)?;
    v.visit_function_block_body_kind(&node.body)
}

pub fn visit_resource_declaration<V: Visitor<E> + ?Sized, E: From<Diagnostic>>(
    v: &mut V,
    node: &ResourceDeclaration,
) -> Result<V::Value, E> {
    v.visit_id(&node.name)?;
    v.visit_id(&node.resource)?;
    Acceptor::accept(&node.global_vars, v)?;
    Acceptor::accept(&node.tasks, v)?;
    Acceptor::accept(&node.programs, v)
}

pub fn visit_program_configuration<V: Visitor<E> + ?Sized, E: From<Diagnostic>>(
    v: &mut V,
    node: &ProgramConfiguration,
) -> Result<V::Value, E> {
    v.visit_id(&node.name)?;
    Acceptor::accept(&node.task_name, v)?;
    v.visit_id(&node.type_name)
}

pub fn visit_configuration_declaration<V: Visitor<E> + ?Sized, E: From<Diagnostic>>(
    v: &mut V,
    node: &ConfigurationDeclaration,
) -> Result<V::Value, E> {
    v.visit_id(&node.name)?;
    Acceptor::accept(&node.global_var, v)?;
    Acceptor::accept(&node.resource_decl, v)
}

pub fn visit_statements<V: Visitor<E> + ?Sized, E: From<Diagnostic>>(
    v: &mut V,
    node: &Statements,
) -> Result<V::Value, E> {
    Acceptor::accept(&node.body, v)
}

pub fn visit_variable<V: Visitor<E> + ?Sized, E: From<Diagnostic>>(
    v: &mut V,
    node: &Variable,
) -> Result<V::Value, E> {
    match node {
        Variable::AddressAssignment(var) => v.visit_address_assignment(var),
        Variable::Named(var) => v.visit_named_variable(var),
        Variable::Array(var) => v.visit_array_variable(var),
        Variable::Structured(_) => Ok(V::Value::default()),
    }
}

pub fn visit_symbolic_variable_kind<V: Visitor<E> + ?Sized, E: From<Diagnostic>>(
    v: &mut V,
    node: &SymbolicVariableKind,
) -> Result<V::Value, E> {
    match node {
        SymbolicVariableKind::Named(var) => v.visit_named_variable(var),
        SymbolicVariableKind::Array(var) => v.visit_array_variable(var),
        SymbolicVariableKind::Structured(_) => {
            Err(Into::<E>::into(Diagnostic::todo(file!(), line!())))
        }
    }
}

// 3.2.3
pub fn visit_fb_call<V: Visitor<E> + ?Sized, E: From<Diagnostic>>(
    v: &mut V,
    node: &FbCall,
) -> Result<V::Value, E> {
    v.visit_id(&node.var_name)?;
    Acceptor::accept(&node.params, v)
}

// 3.2.3
pub fn visit_positional_input<V: Visitor<E> + ?Sized, E: From<Diagnostic>>(
    v: &mut V,
    node: &PositionalInput,
) -> Result<V::Value, E> {
    v.visit_expr_kind(&node.expr)
}

// 3.2.3
pub fn visit_named_input<V: Visitor<E> + ?Sized, E: From<Diagnostic>>(
    v: &mut V,
    node: &NamedInput,
) -> Result<V::Value, E> {
    v.visit_id(&node.name)?;
    v.visit_expr_kind(&node.expr)
}

// 3.2.3
pub fn visit_output<V: Visitor<E> + ?Sized, E: From<Diagnostic>>(
    v: &mut V,
    node: &Output,
) -> Result<V::Value, E> {
    v.visit_id(&node.src)?;
    v.visit_variable(&node.tgt)
}

pub fn visit_param_assignment_kind<V: Visitor<E> + ?Sized, E: From<Diagnostic>>(
    v: &mut V,
    node: &ParamAssignmentKind,
) -> Result<V::Value, E> {
    match node {
        ParamAssignmentKind::PositionalInput(node) => v.visit_positional_input(node),
        ParamAssignmentKind::NamedInput(node) => v.visit_named_input(node),
        ParamAssignmentKind::Output(node) => v.visit_output(node),
    }
}

pub fn visit_stmt_kind<V: Visitor<E> + ?Sized, E: From<Diagnostic>>(
    v: &mut V,
    node: &StmtKind,
) -> Result<V::Value, E> {
    match node {
        StmtKind::Assignment(node) => v.visit_assignment(node),
        StmtKind::FbCall(node) => v.visit_fb_call(node),
        StmtKind::If(node) => v.visit_if(node),
        StmtKind::Case(node) => v.visit_case(node),
        StmtKind::For(node) => v.visit_for(node),
        StmtKind::While(node) => v.visit_while(node),
        StmtKind::Repeat(node) => v.visit_repeat(node),
        StmtKind::Return => Ok(V::Value::default()),
        StmtKind::Exit => Ok(V::Value::default()),
    }
}

pub fn visit_compare_expr<V: Visitor<E> + ?Sized, E: From<Diagnostic>>(
    v: &mut V,
    node: &CompareExpr,
) -> Result<V::Value, E> {
    v.visit_expr_kind(&node.left)?;
    v.visit_expr_kind(&node.right)
}

pub fn visit_binary_expr<V: Visitor<E> + ?Sized, E: From<Diagnostic>>(
    v: &mut V,
    node: &BinaryExpr,
) -> Result<V::Value, E> {
    v.visit_expr_kind(&node.left)?;
    v.visit_expr_kind(&node.right)
}

pub fn visit_unary_expr<V: Visitor<E> + ?Sized, E: From<Diagnostic>>(
    v: &mut V,
    node: &UnaryExpr,
) -> Result<V::Value, E> {
    v.visit_expr_kind(&node.term)
}

pub fn visit_function<V: Visitor<E> + ?Sized, E: From<Diagnostic>>(
    v: &mut V,
    node: &Function,
) -> Result<V::Value, E> {
    v.visit_id(&node.name)?;
    Acceptor::accept(&node.param_assignment, v)
}

pub fn visit_expr_kind<V: Visitor<E> + ?Sized, E: From<Diagnostic>>(
    v: &mut V,
    node: &ExprKind,
) -> Result<V::Value, E> {
    match node {
        ExprKind::Compare(node) => v.visit_compare_expr(node.as_ref()),
        ExprKind::BinaryOp(node) => v.visit_binary_expr(node.as_ref()),
        ExprKind::UnaryOp(node) => v.visit_unary_expr(node.as_ref()),
        ExprKind::Const(node) => v.visit_constant_kind(node),
        ExprKind::Expression(node) => v.visit_expr_kind(node.as_ref()),
        ExprKind::Variable(node) => v.visit_variable(node),
        ExprKind::Function(node) => v.visit_function(node),
    }
}

pub fn visit_assignment<V: Visitor<E> + ?Sized, E: From<Diagnostic>>(
    v: &mut V,
    node: &Assignment,
) -> Result<V::Value, E> {
    v.visit_variable(&node.target)?;
    v.visit_expr_kind(&node.value)
}

pub fn visit_if<V: Visitor<E> + ?Sized, E: From<Diagnostic>>(
    v: &mut V,
    node: &If,
) -> Result<V::Value, E> {
    v.visit_expr_kind(&node.expr)?;
    Acceptor::accept(&node.body, v)?;
    // TODO else ifs
    Acceptor::accept(&node.else_body, v)
}

pub fn visit_case<V: Visitor<E> + ?Sized, E: From<Diagnostic>>(
    v: &mut V,
    node: &Case,
) -> Result<V::Value, E> {
    v.visit_expr_kind(&node.selector)?;
    Acceptor::accept(&node.statement_groups, v)?;
    Acceptor::accept(&node.else_body, v)
}

pub fn visit_case_statement_group<V: Visitor<E> + ?Sized, E: From<Diagnostic>>(
    v: &mut V,
    node: &CaseStatementGroup,
) -> Result<V::Value, E> {
    Acceptor::accept(&node.selectors, v)?;
    Acceptor::accept(&node.statements, v)
}

pub fn visit_case_selection_kind<V: Visitor<E> + ?Sized, E: From<Diagnostic>>(
    v: &mut V,
    node: &CaseSelectionKind,
) -> Result<V::Value, E> {
    match node {
        CaseSelectionKind::Subrange(sr) => v.visit_subrange(sr),
        CaseSelectionKind::SignedInteger(si) => v.visit_signed_integer(si),
        CaseSelectionKind::EnumeratedValue(ev) => v.visit_enumerated_value(ev),
    }
}

pub fn visit_for<V: Visitor<E> + ?Sized, E: From<Diagnostic>>(
    v: &mut V,
    node: &For,
) -> Result<V::Value, E> {
    v.visit_id(&node.control)?;
    v.visit_expr_kind(&node.from)?;
    v.visit_expr_kind(&node.to)?;
    Acceptor::accept(&node.step, v)?;
    Acceptor::accept(&node.body, v)
}

pub fn visit_while<V: Visitor<E> + ?Sized, E: From<Diagnostic>>(
    v: &mut V,
    node: &While,
) -> Result<V::Value, E> {
    v.visit_expr_kind(&node.condition)?;
    Acceptor::accept(&node.body, v)
}

pub fn visit_repeat<V: Visitor<E> + ?Sized, E: From<Diagnostic>>(
    v: &mut V,
    node: &Repeat,
) -> Result<V::Value, E> {
    v.visit_expr_kind(&node.until)?;
    Acceptor::accept(&node.body, v)
}

pub fn visit_array_variable<V: Visitor<E> + ?Sized, E: From<Diagnostic>>(
    v: &mut V,
    node: &ArrayVariable,
) -> Result<V::Value, E> {
    v.visit_symbolic_variable_kind(node.variable.as_ref())?;
    Acceptor::accept(&node.subscripts, v)
}

acceptor_impl!(Id);
acceptor_impl!(ConstantKind);

acceptor_impl!(LibraryElementKind);
acceptor_impl!(DataTypeDeclarationKind);
acceptor_impl!(EnumeratedValue);
acceptor_impl!(StructureElementDeclaration);
acceptor_impl!(StructureElementInit);
acceptor_impl!(Subrange);
acceptor_impl!(VarDecl);
acceptor_impl!(Network);
acceptor_impl!(ElementKind);
acceptor_impl!(ResourceDeclaration);

acceptor_impl!(ArrayInitialElementKind);
acceptor_impl!(ExprKind);
acceptor_impl!(CaseStatementGroup);
acceptor_impl!(CaseSelectionKind);
acceptor_impl!(ParamAssignmentKind);
acceptor_impl!(StmtKind);

acceptor_impl!(TaskConfiguration);
acceptor_impl!(ProgramConfiguration);

mod test {
    use super::*;
    use crate::common::*;
    use crate::core::{Id, SourceLoc};
    use crate::textual::*;
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

    impl Visitor<()> for Descender {
        type Value = ();

        fn visit_address_assignment(&mut self, variable: &AddressAssignment) -> Result<(), ()> {
            let mut dst = &mut self.names;
            dst.push_back(variable.to_string());
            Ok(())
        }

        fn visit_named_variable(&mut self, var: &NamedVariable) -> Result<(), ()> {
            let mut dst = &mut self.names;
            dst.push_back(var.name.to_string());
            Ok(())
        }

        fn visit_fb_call(&mut self, fb_call: &FbCall) -> Result<(), ()> {
            let mut dst = &mut self.names;
            dst.push_back(fb_call.var_name.to_string());
            Ok(())
        }
    }

    #[test]
    fn walk_when_has_symbolic_variable_then_visits_variable() {
        let library = Library {
            elements: vec![LibraryElementKind::ProgramDeclaration(ProgramDeclaration {
                type_name: Id::from("plc_prg"),
                variables: vec![VarDecl::simple("Reset", "BOOL").with_type(VariableType::Input)],
                body: FunctionBlockBodyKind::stmts(vec![StmtKind::fb_assign(
                    "AverageVal",
                    vec!["Cnt1", "Cnt2"],
                    "_TMP_AverageVal17_OUT",
                )]),
            })],
        };

        let mut descender = Descender::new();

        descender.walk(&library);

        assert_eq!(3, descender.names.len())
    }
}
