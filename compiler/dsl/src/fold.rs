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
use crate::textual::*;

// Defines an object as being able to be folded. That is, return a new
// folded version of itself.
pub(crate) trait Foldable {
    type Mapped;
    fn fold<F: Fold<E> + ?Sized, E>(self, folder: &mut F) -> Result<Self::Mapped, E>;
}

impl<X> Foldable for Vec<X>
where
    X: Foldable,
{
    type Mapped = Vec<X::Mapped>;
    fn fold<F: Fold<E> + ?Sized, E>(self, folder: &mut F) -> Result<Self::Mapped, E> {
        self.into_iter().map(|x| x.fold(folder)).collect()
    }
}

pub trait Fold<E> {
    fn fold(&mut self, node: Library) -> Result<Library, E> {
        Ok(Library {
            elements: Foldable::fold(node.elements, self)?,
        })
    }
    fn fold_library_element_declaration(
        &mut self,
        node: LibraryElement,
    ) -> Result<LibraryElement, E> {
        match node {
            LibraryElement::DataTypeDeclaration(data_type) => {
                Ok(LibraryElement::DataTypeDeclaration(
                    self.fold_data_type_declaration_kind(data_type)?,
                ))
            }
            LibraryElement::FunctionBlockDeclaration(function_block_decl) => {
                Ok(LibraryElement::FunctionBlockDeclaration(
                    self.fold_function_block_declaration(function_block_decl)?,
                ))
            }
            LibraryElement::FunctionDeclaration(function_decl) => Ok(
                LibraryElement::FunctionDeclaration(self.fold_function_declaration(function_decl)?),
            ),
            LibraryElement::ProgramDeclaration(program_decl) => Ok(
                LibraryElement::ProgramDeclaration(self.fold_program_declaration(program_decl)?),
            ),
            _ => Ok(node),
        }
    }

    /// Fold data type declarations.
    ///
    /// See section 2.4.3.
    fn fold_data_type_declaration_kind(
        &mut self,
        node: DataTypeDeclarationKind,
    ) -> Result<DataTypeDeclarationKind, E> {
        Ok(node)
    }

    /// Fold variable declaration.
    ///
    /// See section 2.4.3.
    fn fold_variable_declaration(&mut self, node: VarDecl) -> Result<VarDecl, E> {
        Ok(VarDecl {
            name: node.name.clone(),
            var_type: node.var_type,
            qualifier: node.qualifier,
            initializer: Foldable::fold(node.initializer, self)?,
            position: node.position,
        })
    }

    /// Fold an address assignment.
    ///
    /// See section 2.4.3.1.
    fn fold_address_assignment(&mut self, node: AddressAssignment) -> Result<AddressAssignment, E> {
        Ok(node)
    }

    /// Fold initial value assignments.
    ///
    /// See section 2.4.3.2.
    fn fold_initial_value_assignment(
        &mut self,
        node: InitialValueAssignmentKind,
    ) -> Result<InitialValueAssignmentKind, E> {
        Ok(node)
    }

    /// Fold function declarations.
    ///
    /// See section 2.5.1.
    fn fold_function_declaration(
        &mut self,
        node: FunctionDeclaration,
    ) -> Result<FunctionDeclaration, E> {
        Ok(FunctionDeclaration {
            name: node.name.clone(),
            return_type: node.return_type,
            variables: Foldable::fold(node.variables, self)?,
            body: node.body,
        })
    }

    /// Fold function block declarations.
    ///
    /// See section 2.5.2.
    fn fold_function_block_declaration(
        &mut self,
        node: FunctionBlockDeclaration,
    ) -> Result<FunctionBlockDeclaration, E> {
        Ok(FunctionBlockDeclaration {
            name: node.name,
            variables: Foldable::fold(node.variables, self)?,
            body: node.body,
            position: node.position,
        })
    }

    /// Fold program declarations.
    ///
    /// See section 2.5.3.
    fn fold_program_declaration(
        &mut self,
        node: ProgramDeclaration,
    ) -> Result<ProgramDeclaration, E> {
        Ok(ProgramDeclaration {
            type_name: node.type_name,
            variables: Foldable::fold(node.variables, self)?,
            body: node.body,
        })
    }
}

impl Foldable for LibraryElement {
    type Mapped = LibraryElement;
    fn fold<F: Fold<E> + ?Sized, E>(self, folder: &mut F) -> Result<Self::Mapped, E> {
        folder.fold_library_element_declaration(self)
    }
}

impl Foldable for VarDecl {
    type Mapped = VarDecl;
    fn fold<F: Fold<E> + ?Sized, E>(self, folder: &mut F) -> Result<Self::Mapped, E> {
        folder.fold_variable_declaration(self)
    }
}

impl Foldable for InitialValueAssignmentKind {
    type Mapped = InitialValueAssignmentKind;
    fn fold<F: Fold<E> + ?Sized, E>(self, folder: &mut F) -> Result<Self::Mapped, E> {
        folder.fold_initial_value_assignment(self)
    }
}

impl Foldable for AddressAssignment {
    type Mapped = AddressAssignment;
    fn fold<F: Fold<E> + ?Sized, E>(self, folder: &mut F) -> Result<Self::Mapped, E> {
        folder.fold_address_assignment(self)
    }
}
