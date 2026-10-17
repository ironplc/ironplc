//! The declared types of method parameters, collected from the library
//! before the pass folds it.
//!
//! A value passed to a parameter of a method is converted to the type the
//! parameter is declared with, and an untyped literal takes that type. The
//! analyzer cannot read it from the type of the block, which does not list a
//! block's methods, so the declarations are read here, once, before the
//! library is folded. A field of a block has its declared type in the type
//! environment (`TypeEnvironment::field_type`).
//!
//! A declared type is the elementary type the value is operated as: its own,
//! or its base type for a subrange. A parameter that is not a single numeric
//! value (an enumeration, a string, an aggregate, a reference) has none, and
//! nothing is converted to it.

use std::collections::HashMap;

use ironplc_dsl::common::{Library, LibraryElementKind, TypeName, VarDecl};
use ironplc_dsl::core::Id;
use ironplc_dsl::type_id::TypeId;

use crate::callee_resolution::MethodOwners;
use crate::semantic_type::SemanticType;
use crate::type_environment::TypeEnvironment;

/// The input parameters of a method, in declaration order: each parameter's
/// name and its declared type.
pub(super) type Parameters = Vec<(Id, Option<TypeId>)>;

/// What the declarations of the user-defined function blocks in a library
/// say about the types their method parameters are declared with.
#[derive(Default)]
pub(super) struct Declarations {
    /// The input parameters of each method, by the block that declares it
    /// and the method's name.
    methods: HashMap<(Id, Id), Parameters>,
    /// The block that declares each method callable on each block.
    owners: MethodOwners,
}

impl Declarations {
    /// Collects the declarations of every user-defined function block in
    /// `lib`.
    pub(super) fn collect(lib: &Library, types: &TypeEnvironment) -> Self {
        let mut declarations = Declarations {
            owners: MethodOwners::of_library(lib, types),
            ..Declarations::default()
        };
        for element in &lib.elements {
            let LibraryElementKind::FunctionBlockDeclaration(block) = element else {
                continue;
            };
            let name = &block.name.name;
            for method in &block.methods {
                let parameters = method
                    .variables
                    .iter()
                    .filter(|decl| decl.var_type.is_input_compatible())
                    .filter_map(|decl| {
                        let parameter = decl.identifier.symbolic_id()?.clone();
                        Some((parameter, declared_type(types, decl)))
                    })
                    .collect();
                declarations
                    .methods
                    .insert((name.clone(), method.name.clone()), parameters);
            }
        }
        declarations
    }

    /// The input parameters of the method `method` called on an instance
    /// of the function block `block`: the method `block` declares, or else
    /// the one the nearest block it `EXTENDS` declares.
    pub(super) fn method(&self, block: &TypeName, method: &Id) -> Option<&Parameters> {
        let owner = self.owners.declaring_block(block, method)?;
        self.methods.get(&(owner.name.clone(), method.clone()))
    }
}

/// The type `decl` declares, as the elementary type a value of it is
/// operated as.
fn declared_type(types: &TypeEnvironment, decl: &VarDecl) -> Option<TypeId> {
    let attributes = types.get_by_id(decl.type_id?)?;
    elementary_of(types, &attributes.representation)
}

/// The elementary type a value of `representation` is operated as: its own,
/// or its base type for a subrange.
pub(super) fn elementary_of(
    types: &TypeEnvironment,
    representation: &SemanticType,
) -> Option<TypeId> {
    types.id_of(&types.elementary_type_name_for(representation.operated_as())?)
}
