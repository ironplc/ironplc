//! A set of traits and functions for folding all nodes in a library.
//!
//! Folding the library returns a new instance with changes to the
//! library defined based on the fold_* functions. The default behavior
//! returns an copy of input.
//!
//! To fold a library, define a struct and implement the Fold trait
//! for the struct. The implement fold_* functions from the trait to
//! customize the behavior.
use crate::common::*;
use crate::configuration::*;
use crate::core::*;
use crate::sfc::*;
use crate::textual::*;
use paste::paste;

/// Defines a macro for the Fold struct that dispatches folding
/// to a function. In other words, creates a function of the form:
///
/// ```ignore
/// fn visit_type_name<E>(&mut self, node: TypeName) -> Result<Fold::Value, E> {
///    visit_type_name(self, node)
/// }
/// ```
macro_rules! dispatch
{
    ($struct_name:ident) => {
        paste! {
            fn [<fold_ $struct_name:snake >](&mut self, node: $struct_name) -> Result<$struct_name, E> {
                $struct_name::recurse_fold(node, self)
            }
        }
    };
}

macro_rules! leaf
{
    ($struct_name:ident) => {
        paste! {
            fn [<fold_ $struct_name:snake >](&mut self, node: $struct_name) -> Result<$struct_name, E> {
                Ok(node)
            }
        }
    };
}

pub trait Fold<E> {
    fn fold_library(&mut self, node: Library) -> Result<Library, E> {
        node.recurse_fold(self)
    }

    // Declarations from Core

    leaf!(SourceLoc);

    // 2.1.2.
    dispatch!(Id);

    // Declarations from Common

    // TODO Constants
    dispatch!(Integer);

    dispatch!(SignedInteger);

    // TODO should probably recurse to find source locations
    leaf!(ConstantKind);

    // 2.3.3.1
    dispatch!(DataTypeDeclarationKind);

    // 2.3.3.1
    dispatch!(LateBoundDeclaration);

    // 2.3.3.1
    dispatch!(EnumerationDeclaration);

    dispatch!(InitialValueAssignmentKind);
    dispatch!(StructInitialValueAssignmentKind);

    // 2.4.3.2
    dispatch!(EnumeratedSpecificationInit);

    // 2.4.3.2
    dispatch!(EnumeratedSpecificationValues);

    // 2.3.3.1
    dispatch!(EnumeratedValue);

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

    dispatch!(Repeated);

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

    dispatch!(ArraySubranges);

    // 2.4.2.1
    dispatch!(Subrange);

    // 2.4.3
    dispatch!(VarDecl);

    // 2.4.3.1
    dispatch!(AddressAssignment);

    // 2.4.3.2
    dispatch!(SimpleInitializer);

    // 2.4.3.1 and 2.4.3.2
    dispatch!(StringInitializer);

    // 2.4.3.2
    dispatch!(EnumeratedValuesInitializer);

    // 2.4.3.2
    dispatch!(FunctionBlockInitialValueAssignment);

    // 2.4.3.2.
    dispatch!(ArrayInitialValueAssignment);

    // 2.4.3.2 (TODO - where?)
    dispatch!(EnumeratedSpecificationKind);

    dispatch!(EnumeratedInitialValueAssignment);

    dispatch!(VariableIdentifier);

    dispatch!(DirectVariableIdentifier);

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
    dispatch!(Step);

    // 2.6.3
    dispatch!(Transition);

    // 2.6.4
    dispatch!(Action);

    dispatch!(ActionAssociation);

    // Declarations from Configuration

    // 2.7.1
    dispatch!(ResourceDeclaration);

    // 2.7.1
    dispatch!(ProgramConfiguration);

    // 2.7.2
    dispatch!(ConfigurationDeclaration);

    // 2.7.2
    dispatch!(TaskConfiguration);

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
    dispatch!(ElseIf);

    // 3.3.2.3
    dispatch!(Case);

    // 3.3.2.3
    dispatch!(CaseStatementGroup);

    // 3.3.2.3
    dispatch!(CaseSelectionKind);

    dispatch!(For);

    dispatch!(While);

    dispatch!(Repeat);

    dispatch!(NamedVariable);

    dispatch!(ArrayVariable);

    dispatch!(StructuredVariable);
}
