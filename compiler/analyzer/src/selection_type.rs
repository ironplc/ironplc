//! The type a variable selection names.
//!
//! A selection starts at a variable and walks outwards through subscripts,
//! fields and dereferences: `rows[1][2]`, `recs[i].values[j]`, `p^[k]`. This
//! module walks it and answers the type each step selects.
//!
//! A subscript takes one index per dimension of the array it is in, and a
//! bracket may give some of them: `m[1][2]` on an `ARRAY[1..2, 1..3]` selects
//! the same element as `m[1, 2]`. Once every dimension has an index the
//! walk reaches the element type, so on `rows : ARRAY[1..2] OF Row`,
//! `rows[1]` is a `Row` and `rows[1][2]` is an element of a `Row`. A bracket
//! does not reach past the array it starts in: `rows[1, 2]` selects nothing.
//!
//! The walk keeps a type's **name** for as long as it is known, since the
//! name is the type's identity: an element of an `ARRAY OF Color` is a
//! `Color`. The type environment keeps only the element *shape* of a named
//! array type, so the element names of array type declarations are read
//! from the library ([`ArrayDeclarations`]). A field's type is known only by
//! its shape, which names an elementary type but nothing else.
//!
//! A reference to an array is subscripted as the array it references, and a
//! reference to a structure selects that structure's fields, as the
//! resolution of the reference's declared type always did.
use std::collections::HashMap;

use ironplc_dsl::common::*;
use ironplc_dsl::textual::*;

use crate::intermediate_type::IntermediateType;
use crate::type_environment::TypeEnvironment;
use crate::variable_type::{Declarations, Declared};

/// How many named types a walk expands in a row before giving up. A chain
/// of array type aliases longer than this is not resolved; a cyclic one is
/// reported by the type declaration rules.
const MAX_EXPANSIONS: usize = 32;

/// The array specification of every array type declared in a library, by
/// type name.
#[derive(Default)]
pub(crate) struct ArrayDeclarations(HashMap<TypeName, ArraySpecificationKind>);

impl ArrayDeclarations {
    /// Collects the array type declarations of `lib`.
    pub(crate) fn from_library(lib: &Library) -> Self {
        Self(
            lib.elements
                .iter()
                .filter_map(|element| match element {
                    LibraryElementKind::DataTypeDeclaration(DataTypeDeclarationKind::Array(
                        decl,
                    )) => Some((decl.type_name.clone(), decl.spec.clone())),
                    _ => None,
                })
                .collect(),
        )
    }
}

/// What a walk knows of the type of a selection so far.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum View {
    /// A type known by its name.
    Named(TypeName),
    /// An array, of which the subscripts given so far select part.
    Array {
        /// The number of dimensions, or 0 when the analyzer does not know
        /// it; a bracket then selects an element.
        dimensions: usize,
        /// The number of subscripts given so far, fewer than `dimensions`.
        given: usize,
        element: Box<View>,
    },
    /// A reference to a type.
    Reference(Box<View>),
    /// A type known only by its representation.
    Shape(IntermediateType),
}

impl View {
    fn array(subranges: &ArraySubranges) -> Self {
        let element = View::Named(subranges.type_name.to_type_name());
        View::Array {
            dimensions: subranges.ranges.len(),
            given: 0,
            element: Box::new(match subranges.ref_to {
                Some(_) => View::Reference(Box::new(element)),
                None => element,
            }),
        }
    }

    fn of_shape(shape: &IntermediateType) -> Self {
        match shape {
            IntermediateType::Array {
                element_type,
                dimensions,
            } => View::Array {
                dimensions: dimensions.len(),
                given: 0,
                element: Box::new(View::Shape(element_type.as_ref().clone())),
            },
            IntermediateType::Reference { target_type } => {
                View::Reference(Box::new(View::Shape(target_type.as_ref().clone())))
            }
            other => View::Shape(other.clone()),
        }
    }
}

/// Walks selections against the declarations in scope.
pub(crate) struct Selections<'a, 'd> {
    pub declarations: &'a Declarations<'d>,
    pub types: &'a TypeEnvironment,
    pub arrays: &'a ArrayDeclarations,
}

impl Selections<'_, '_> {
    /// What `kind` selects, or `None` when the walk cannot tell: an
    /// undeclared variable, a subscript on something that is not an array,
    /// a field that does not exist, or more subscripts than dimensions.
    pub(crate) fn view_of(&self, kind: &SymbolicVariableKind) -> Option<View> {
        match kind {
            SymbolicVariableKind::Named(named) => {
                self.declared_view(self.declarations.find(&named.name)?)
            }
            SymbolicVariableKind::Array(array) => {
                let base = self.view_of(&array.subscripted_variable)?;
                self.subscript(base, array.subscripts.len())
            }
            SymbolicVariableKind::Structured(structured) => {
                let record = self.view_of(&structured.record)?;
                self.field(record, &structured.field)
            }
            SymbolicVariableKind::Deref(deref) => {
                match self.expand(self.view_of(&deref.variable)?)? {
                    View::Reference(target) => Some(*target),
                    _ => None,
                }
            }
            SymbolicVariableKind::BitAccess(_) => Some(View::Named(TypeName::from("BOOL"))),
            SymbolicVariableKind::PartialAccess(partial) => {
                Some(View::Named(TypeName::from(match partial.size {
                    PartialAccessSize::Byte => "BYTE",
                    PartialAccessSize::Word => "WORD",
                    PartialAccessSize::DWord => "DWORD",
                    PartialAccessSize::LWord => "LWORD",
                })))
            }
            // Typing a member of THIS^/SUPER^ needs function-block member
            // resolution, which does not exist yet. See issue #1406.
            SymbolicVariableKind::SelfRef(_) => None,
        }
    }

    /// The name of the type `view` is, when it has one: an elementary type
    /// by its canonical name, any other type by the name it was declared
    /// with. An array or reference spelled out in place has no name.
    pub(crate) fn type_name(&self, view: &View) -> Option<TypeName> {
        match view {
            View::Named(name) => Some(
                self.types
                    .resolve_elementary_type_name(name)
                    .unwrap_or_else(|| name.clone()),
            ),
            View::Shape(shape) => elementary_type_name(self.types, shape),
            View::Array { .. } | View::Reference(_) => None,
        }
    }

    /// The view of a declared variable.
    fn declared_view(&self, declared: &Declared) -> Option<View> {
        let init = match declared {
            Declared::Typed(type_name) => return Some(View::Named(type_name.clone())),
            Declared::Variable { init, .. } => init.as_ref(),
        };
        match init {
            InitialValueAssignmentKind::Array(array) => match &array.spec {
                SpecificationKind::Inline(subranges) => Some(View::array(subranges)),
                SpecificationKind::Named(name) => Some(View::Named(name.clone())),
            },
            InitialValueAssignmentKind::Reference(reference) => {
                Some(View::Reference(Box::new(match &reference.target {
                    ReferenceTarget::Array(subranges) => View::array(subranges),
                    ReferenceTarget::Named(name) => View::Named(name.clone()),
                })))
            }
            other => match other.type_reference() {
                TypeReference::Named(name) => Some(View::Named(name)),
                TypeReference::Inline | TypeReference::Unspecified => None,
            },
        }
    }

    /// Replaces a named type or a shape by the array or reference it is,
    /// so a subscript or dereference can see through it. Any other view is
    /// returned as it is.
    fn expand(&self, mut view: View) -> Option<View> {
        for _ in 0..MAX_EXPANSIONS {
            view = match view {
                View::Named(name) => match self.arrays.0.get(&name) {
                    Some(SpecificationKind::Inline(subranges)) => {
                        return Some(View::array(subranges))
                    }
                    Some(SpecificationKind::Named(alias)) => View::Named(alias.clone()),
                    None => {
                        let shape = &self.types.get(&name)?.representation;
                        if !matches!(
                            shape,
                            IntermediateType::Array { .. } | IntermediateType::Reference { .. }
                        ) {
                            return Some(View::Named(name));
                        }
                        return Some(View::of_shape(shape));
                    }
                },
                View::Shape(shape) => return Some(View::of_shape(&shape)),
                other => return Some(other),
            };
        }
        None
    }

    /// The view `count` subscripts in one bracket select from `view`.
    fn subscript(&self, view: View, count: usize) -> Option<View> {
        match self.expand(view)? {
            View::Array {
                dimensions,
                given,
                element,
            } => {
                let given = given + count;
                if dimensions == 0 || given == dimensions {
                    Some(*element)
                } else if given < dimensions {
                    Some(View::Array {
                        dimensions,
                        given,
                        element,
                    })
                } else {
                    None
                }
            }
            // `pa[i]` subscripts the array `pa` references.
            View::Reference(target) => match self.expand(*target)? {
                array @ View::Array { given: 0, .. } => self.subscript(array, count),
                _ => None,
            },
            View::Named(_) | View::Shape(_) => None,
        }
    }

    /// The view of the field `field` of `record`.
    fn field(&self, record: View, field: &ironplc_dsl::core::Id) -> Option<View> {
        let shape = match self.expand(record)? {
            View::Named(name) => self.types.resolve_member_access_type(&name)?.clone(),
            View::Shape(shape) => shape,
            // A reference selects the fields of the structure it references.
            View::Reference(target) => return self.field(*target, field),
            View::Array { .. } => return None,
        };
        let member = shape.member_fields()?.iter().find(|f| f.name == *field)?;
        Some(View::Shape(member.field_type.clone()))
    }
}

/// Maps an [`IntermediateType`] to its canonical elementary [`TypeName`].
///
/// Delegates to [`TypeEnvironment::elementary_type_name_for`] for the simple
/// cases. That helper does a strict equality lookup against the elementary
/// types table, which only contains `String { max_len: None }`. A struct
/// field declared `STRING[n]` resolves to `String { max_len: Some(n) }` and
/// would otherwise return `None`, so strings are handled explicitly.
fn elementary_type_name(types: &TypeEnvironment, shape: &IntermediateType) -> Option<TypeName> {
    if let Some(name) = types.elementary_type_name_for(shape) {
        return Some(name);
    }
    match shape {
        IntermediateType::String { .. } => Some(TypeName::from("STRING")),
        _ => None,
    }
}
