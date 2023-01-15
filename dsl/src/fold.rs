//! A set of traits and functions for folding all nodes in a library.
//!
//! Folding the library returns a new instance with changes to the
//! library defined based on the fold_* functions. The default behavior
//! returns an copy of input.
//!
//! To fold a library, define a struct and implement the Fold trait
//! for the struct. The implement fold_* functions from the trait to
//! customize the behavior.
use crate::ast::*;
use crate::dsl::*;

// Defines an object as being able to be folded. That is, return a new
// folded version of itself.
pub(crate) trait Foldable {
    type Mapped;
    fn fold<F: Fold + ?Sized>(self, folder: &mut F) -> Self::Mapped;
}

impl<X> Foldable for Vec<X>
where
    X: Foldable,
{
    type Mapped = Vec<X::Mapped>;
    fn fold<F: Fold + ?Sized>(self, folder: &mut F) -> Self::Mapped {
        self.into_iter().map(|x| x.fold(folder)).collect()
    }
}

impl<X> Foldable for Option<X>
where
    X: Foldable,
{
    type Mapped = Option<X::Mapped>;
    fn fold<F: Fold + ?Sized>(self, folder: &mut F) -> Self::Mapped {
        self.map(|x| x.fold(folder))
    }
}

pub trait Fold {
    fn fold(&mut self, node: Library) -> Library {
        Library {
            elems: Foldable::fold(node.elems, self),
        }
    }
    fn fold_library_element_declaration(&mut self, node: LibraryElement) -> LibraryElement {
        match node {
            LibraryElement::FunctionBlockDeclaration(function_block_decl) => {
                LibraryElement::FunctionBlockDeclaration(
                    self.fold_function_block_declaration(function_block_decl),
                )
            }
            LibraryElement::FunctionDeclaration(function_decl) => {
                LibraryElement::FunctionDeclaration(self.fold_function_declaration(function_decl))
            }
            LibraryElement::ProgramDeclaration(program_decl) => {
                LibraryElement::ProgramDeclaration(self.fold_program_declaration(program_decl))
            }
            _ => node,
        }
    }

    fn fold_function_block_declaration(
        &mut self,
        node: FunctionBlockDeclaration,
    ) -> FunctionBlockDeclaration {
        FunctionBlockDeclaration {
            name: node.name,
            variables: Foldable::fold(node.variables, self),
            body: node.body,
        }
    }

    fn fold_function_declaration(&mut self, node: FunctionDeclaration) -> FunctionDeclaration {
        FunctionDeclaration {
            name: node.name.clone(),
            return_type: node.return_type,
            variables: Foldable::fold(node.variables, self),
            body: node.body,
        }
    }

    fn fold_program_declaration(&mut self, node: ProgramDeclaration) -> ProgramDeclaration {
        ProgramDeclaration {
            type_name: node.type_name,
            variables: Foldable::fold(node.variables, self),
            body: node.body,
        }
    }

    fn fold_variable_declaration(&mut self, node: VarDecl) -> VarDecl {
        VarDecl {
            name: node.name.clone(),
            var_type: node.var_type,
            qualifier: node.qualifier,
            initializer: Foldable::fold(node.initializer, self),
            position: node.position,
        }
    }

    fn fold_type_initializer(&mut self, node: TypeInitializer) -> TypeInitializer {
        node
    }

    fn fold_direct_variable(&mut self, node: DirectVariable) -> DirectVariable {
        node
    }
}

impl Foldable for LibraryElement {
    type Mapped = LibraryElement;
    fn fold<F: Fold + ?Sized>(self, folder: &mut F) -> Self::Mapped {
        folder.fold_library_element_declaration(self)
    }
}

impl Foldable for VarDecl {
    type Mapped = VarDecl;
    fn fold<F: Fold + ?Sized>(self, folder: &mut F) -> Self::Mapped {
        folder.fold_variable_declaration(self)
    }
}

impl Foldable for TypeInitializer {
    type Mapped = TypeInitializer;
    fn fold<F: Fold + ?Sized>(self, folder: &mut F) -> Self::Mapped {
        folder.fold_type_initializer(self)
    }
}

impl Foldable for DirectVariable {
    type Mapped = DirectVariable;
    fn fold<F: Fold + ?Sized>(self, folder: &mut F) -> Self::Mapped {
        folder.fold_direct_variable(self)
    }
}
