//! A place as this backend's layout addresses it: a variable the layout
//! keeps by its lowered id ([`Stored`]), then the fields, subscripts and
//! dereferences that reach the place within it.
//!
//! The lowered program names a place by ids and projections
//! (`ironplc_ir::lowered::Place`), and the layout keeps each variable by
//! the id that names it (`specs/design/lowered-program.md`,
//! REQ-LOW-codegen-123). So a lowered place is addressed as the [`Access`]
//! that names it ([`crate::body`]), with its own subscripts, and the
//! layout's addressing ([`crate::compile_place`],
//! [`crate::compile_string_place`]) resolves that.

use std::fmt;

use ironplc_ir::lowered::{FieldIdx, TypeId, VarId};
use ironplc_ir::{Id, Located, SourceSpan};

/// A variable the layout gives storage to, by its lowered id: a variable,
/// or a field of a function block's or the program's instance, which the
/// body that runs on the instance keeps in a slot of its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Stored {
    /// The variable `VarId`.
    Var(VarId),
    /// The field of an instance of the function block or program `TypeId`.
    Field(TypeId, FieldIdx),
}

/// A variable the layout gives storage to, with the name it is declared by,
/// for a diagnostic about it.
#[derive(Clone, Debug)]
pub(crate) struct Named {
    pub stored: Stored,
    pub name: Id,
}

impl Named {
    /// Where the variable is named.
    pub(crate) fn span(&self) -> SourceSpan {
        self.name.span()
    }
}

impl fmt::Display for Named {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.name.fmt(f)
    }
}

/// A place: a variable, or a field, element or dereference of another
/// place. `S` is a subscript.
#[derive(Clone, Debug)]
pub(crate) enum Access<S> {
    /// The variable the layout keeps.
    Named(Named),
    /// The field `field` of the structure or instance `record` is.
    Field(Field<S>),
    /// The element of the array `array` is at `subscripts`.
    Index(Index<S>),
    /// What the reference `reference` holds refers to.
    Deref(Box<Access<S>>),
}

/// The field `idx` of the record `record`, a structure or a function block
/// instance, declared as `field`, for a diagnostic about it.
#[derive(Clone, Debug)]
pub(crate) struct Field<S> {
    pub record: Box<Access<S>>,
    pub idx: FieldIdx,
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

    /// The variable when the place is the whole of a variable.
    pub(crate) fn name(&self) -> Option<&Named> {
        match self {
            Access::Named(name) => Some(name),
            Access::Field(_) | Access::Index(_) | Access::Deref(_) => None,
        }
    }
}

/// A variable to key the layout by in a unit test: one of a program
/// lowering made, named `name`.
#[cfg(test)]
pub(crate) fn test_variable(name: &str) -> Named {
    let source = format!("PROGRAM main VAR {name} : INT; END_VAR END_PROGRAM");
    let options = ironplc_parser::options::CompilerOptions::default();
    let library =
        ironplc_parser::parse_program(&source, &ironplc_ir::FileId::default(), &options).unwrap();
    let (library, context) = ironplc_analyzer::stages::analyze(&[&library], &options).unwrap();
    let completed = ironplc_analyzer::xform_resolve_initial_values::apply(library, &context);
    let analysis = ironplc_analyzer::CleanAnalysis::new(&completed, &context).unwrap();
    let lowering = ironplc_lowering::Lowering::new(
        &analysis,
        ironplc_lowering::parse_policy(&Default::default()),
    )
    .lower_program()
    .unwrap();
    let types = lowering.types();
    let (program, fields) = types
        .iter()
        .find_map(|(ty, entry)| {
            let ironplc_ir::lowered::TypeKind::Program { fields, .. } = &entry.kind else {
                return None;
            };
            Some((ty, fields))
        })
        .unwrap();
    let position = fields
        .iter()
        .position(|field| field.name.to_string() == name)
        .unwrap();
    let idx = types.field_idx(program, position).unwrap();
    Named {
        stored: Stored::Field(program, idx),
        name: Id::from(name),
    }
}
