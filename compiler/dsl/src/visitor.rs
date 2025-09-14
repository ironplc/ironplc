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
//! use ironplc_dsl::visitor::{ Visitor };
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
//!         node.recurse_visit(self)
//!     }
//! }
//! ```

use crate::common::*;
use crate::configuration::*;
use crate::core::{Id, SourceSpan};
use crate::diagnostic::Diagnostic;
use crate::sfc::*;
use crate::textual::*;
use crate::time::*;
use paste::paste;

/// Defines a macro for the `Visitor` trait that dispatches visiting
/// to a function. In other words, creates a function of the form:
///
/// ```ignore
///
/// fn visit_type_name(&mut self, node: &TypeName) -> Result<Self::Value, E> {
///    TypeName::recurse_visit(self, node)
/// }
/// ```
///
///  The visitor generally dispatches to a dedicated function so that
/// implementations can re-use the behavior.
macro_rules! dispatch {
    ($struct_name:ident) => {
        paste! {
            fn [<visit_ $struct_name:snake >](&mut self, node: &$struct_name) -> Result<Self::Value, E> {
                $struct_name::recurse_visit(&node, self)
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

/// Defines a visitor for the object tree. The default visitor recursively
/// walks to visit items in the tree.
///
/// Functions in the visitor are named based snake-case variant of the element
/// name. For example, the `Id` element's visitor function is `visit_id`.
pub trait Visitor<E> {
    /// Value produced by this visitor when the result is not an error.
    ///
    /// The returned value is usually not meaningful because no guarantee
    /// is provided when returning from vectors of objects.
    type Value: Default;

    fn walk(&mut self, node: &Library) -> Result<Self::Value, E> {
        node.recurse_visit(self)
    }

    // Declarations from Core

    leaf!(SourceSpan);

    // 2.1.2.
    dispatch!(Id);

    // Declarations from Common

    dispatch!(Integer);

    dispatch!(SignedInteger);

    // 2.2.1
    dispatch!(IntegerLiteral);

    // 2.2.1
    leaf!(RealLiteral);

    // 2.2.2
    leaf!(BooleanLiteral);

    // 2.2.2
    leaf!(CharacterStringLiteral);

    // 2.2.3
    leaf!(DurationLiteral);

    // 2.2.3.2
    leaf!(TimeOfDayLiteral);

    // 2.2.3.2
    leaf!(DateLiteral);

    // 2.2.3.2
    leaf!(DateAndTimeLiteral);

    // TODO where is this?
    leaf!(BitStringLiteral);

    dispatch!(TypeName);

    // 2.2
    dispatch!(ConstantKind);

    // 2.3.3.1
    dispatch!(DataTypeDeclarationKind);

    // 2.3.3.1
    dispatch!(LateBoundDeclaration);

    // 2.3.3.1
    dispatch!(EnumerationDeclaration);

    dispatch!(InitialValueAssignmentKind);

    // 2.4.3.2
    dispatch!(EnumeratedSpecificationInit);

    // 2.4.3.2
    dispatch!(EnumeratedSpecificationValues);

    // 2.4.3.2
    dispatch!(StructInitialValueAssignmentKind);

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

    dispatch!(ProgramAccessDecl);

    // 2.4.3
    dispatch!(VarDecl);

    dispatch!(EdgeVarDecl);

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

    // 2.4.3.2
    dispatch!(ArrayInitialValueAssignment);

    dispatch!(VariableSpecificationKind);

    dispatch!(StringSpecification);

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

    dispatch!(ActionQualifier);

    dispatch!(ActionTimeKind);

    dispatch!(ActionAssociation);

    // Declarations from Configuration

    // 2.7.1
    dispatch!(ResourceDeclaration);

    // 2.7.1
    dispatch!(ProgramConfiguration);

    // 2.7.2
    dispatch!(ConfigurationDeclaration);

    // 2.7.2
    dispatch!(AccessDeclaration);

    // 2.7.2
    dispatch!(AccessPathKind);

    // 2.7.2
    dispatch!(DirectAccessPath);

    // 2.7.2
    dispatch!(SymbolicAccessPath);

    // 2.7.2
    dispatch!(TaskConfiguration);

    dispatch!(FunctionBlockTask);

    dispatch!(ProgramConnectionSource);

    dispatch!(ProgramConnectionSourceKind);

    dispatch!(ProgramConnectionSink);

    dispatch!(ProgramConnectionSinkKind);

    dispatch!(GlobalVarReference);

    dispatch!(FunctionBlockInit);

    dispatch!(LocatedVarInit);

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

    dispatch!(LateBound);

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

mod test {
    use super::*;
    use crate::common::*;
    use crate::core::{Id, SourceSpan};
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

        fn visit_late_bound(&mut self, node: &LateBound) -> Result<(), ()> {
            let mut dst = &mut self.names;
            dst.push_back(node.value.to_string());
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
                name: Id::from("plc_prg"),
                variables: vec![VarDecl::simple("Reset", "BOOL").with_type(VariableType::Input)],
                access_variables: vec![],
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
