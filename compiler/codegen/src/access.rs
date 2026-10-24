//! A place as this backend's layout addresses it: a variable named as the
//! layout knows it, then the fields, subscripts and dereferences that reach
//! the place within it.
//!
//! The lowered program names a place by ids and projections
//! (`ironplc_ir::lowered::Place`); the layout is still found by name
//! (`specs/design/lowered-program.md`, REQ-LOW-codegen-123). So a lowered
//! place is addressed as the [`Access`] that names it ([`crate::lowered`]),
//! with its own subscripts, and the layout's addressing ([`crate::compile_place`],
//! [`crate::compile_string_place`]) resolves that.

use ironplc_dsl::core::{Id, Located, SourceSpan};

/// A place: a variable, or a field, element or dereference of another
/// place. `S` is a subscript.
#[derive(Clone, Debug)]
pub(crate) enum Access<S> {
    /// The variable the layout knows by this name.
    Named(Id),
    /// The field `field` of the structure or instance `record` is.
    Field(Field<S>),
    /// The element of the array `array` is at `subscripts`.
    Index(Index<S>),
    /// What the reference `reference` holds refers to.
    Deref(Box<Access<S>>),
}

/// The field `field` of the record `record`.
#[derive(Clone, Debug)]
pub(crate) struct Field<S> {
    pub record: Box<Access<S>>,
    pub field: Id,
}

/// The element of `array` at `subscripts`, one for each dimension.
#[derive(Clone, Debug)]
pub(crate) struct Index<S> {
    pub array: Box<Access<S>>,
    pub subscripts: Vec<S>,
}

impl<S> Access<S> {
    /// Where the place is named, for a diagnostic about it.
    pub(crate) fn span(&self) -> SourceSpan {
        match self {
            Access::Named(name) => name.span(),
            Access::Field(field) => field.field.span(),
            Access::Index(index) => index.array.span(),
            Access::Deref(reference) => reference.span(),
        }
    }

    /// The variable's name when the place is the whole of a variable.
    pub(crate) fn name(&self) -> Option<&Id> {
        match self {
            Access::Named(name) => Some(name),
            Access::Field(_) | Access::Index(_) | Access::Deref(_) => None,
        }
    }
}
