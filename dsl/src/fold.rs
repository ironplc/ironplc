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

    fn fold_function_block_declaration(
        &mut self,
        node: FunctionBlockDeclaration,
    ) -> Result<FunctionBlockDeclaration, E> {
        Ok(FunctionBlockDeclaration {
            name: node.name,
            variables: Foldable::fold(node.variables, self)?,
            body: node.body,
        })
    }

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

    fn fold_variable_declaration(&mut self, node: VarDecl) -> Result<VarDecl, E> {
        Ok(VarDecl {
            name: node.name.clone(),
            var_type: node.var_type,
            qualifier: node.qualifier,
            initializer: Foldable::fold(node.initializer, self)?,
            position: node.position,
        })
    }

    fn fold_type_initializer(
        &mut self,
        node: InitialValueAssignmentKind,
    ) -> Result<InitialValueAssignmentKind, E> {
        Ok(node)
    }

    fn fold_direct_variable(&mut self, node: AddressAssignment) -> Result<AddressAssignment, E> {
        Ok(node)
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
        folder.fold_type_initializer(self)
    }
}

impl Foldable for AddressAssignment {
    type Mapped = AddressAssignment;
    fn fold<F: Fold<E> + ?Sized, E>(self, folder: &mut F) -> Result<Self::Mapped, E> {
        folder.fold_direct_variable(self)
    }
}
