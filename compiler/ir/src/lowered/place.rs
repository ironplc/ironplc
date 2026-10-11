//! Places: where a value lives, a variable and a path into it.

use ironplc_dsl::core::SourceSpan;
use ironplc_dsl::diagnostic::Diagnostic;

use super::ids::{FieldIdx, Stamp, TypeId, VarId};
use super::invariant::broken;
use super::types::TypeTable;
use super::variables::VariableTable;

/// One step of the path from a variable to a place inside it.
#[derive(Clone, Debug)]
pub enum Projection {
    /// A field of a structure, function block or program, by position.
    Field(FieldIdx),
    /// Through a reference to what it refers to.
    Deref,
}

/// Where a value lives: a variable, a path into it, and the type of the value
/// located there, so a backend never walks the path to learn what it loads.
///
/// A place says nothing about storage. Whether a variable is a slot, a run of
/// a data region or an address in linear memory is each backend's layout.
#[derive(Clone, Debug)]
pub struct Place {
    root: VarId,
    path: Vec<Projection>,
    ty: TypeId,
}

impl Place {
    /// The whole of the variable `root`, at the type `variables` holds for
    /// it; a P9998 if `variables` did not allocate `root`.
    pub(crate) fn var(
        variables: &VariableTable,
        root: VarId,
        span: &SourceSpan,
    ) -> Result<Self, Diagnostic> {
        let ty = variables.type_of(root, span)?;
        Ok(Self {
            root,
            path: Vec::new(),
            ty,
        })
    }

    /// The field `field` of this place, whose type must be a structure,
    /// function block or program with that field (REQ-LOW-lowering-030).
    pub(crate) fn field(
        mut self,
        types: &TypeTable,
        field: FieldIdx,
        span: &SourceSpan,
    ) -> Result<Self, Diagnostic> {
        if field.stamp() != self.stamp() {
            return Err(broken(
                "Place::field",
                span,
                format!("{field} is a field of another program"),
            ));
        }
        let fields = types
            .fields(self.ty)
            .ok_or_else(|| broken("Place::field", span, format!("{} has no fields", self.ty)))?;
        let declared = fields.get(field.index() as usize).ok_or_else(|| {
            broken(
                "Place::field",
                span,
                format!("{} has no field {field}", self.ty),
            )
        })?;
        self.ty = declared.ty;
        self.path.push(Projection::Field(field));
        Ok(self)
    }

    /// What this place refers to; its type must be a reference
    /// (REQ-LOW-lowering-032).
    pub(crate) fn deref(
        mut self,
        types: &TypeTable,
        span: &SourceSpan,
    ) -> Result<Self, Diagnostic> {
        let (target, _) = types.reference(self.ty).ok_or_else(|| {
            broken(
                "Place::deref",
                span,
                format!("{} is not a reference", self.ty),
            )
        })?;
        self.ty = target;
        self.path.push(Projection::Deref);
        Ok(self)
    }

    /// The variable the place is in.
    pub fn root(&self) -> VarId {
        self.root
    }

    /// The steps from the variable to the place, outermost first.
    pub fn path(&self) -> &[Projection] {
        &self.path
    }

    /// The type of the value located at the place.
    pub fn ty(&self) -> TypeId {
        self.ty
    }

    /// The program the place belongs to.
    pub(crate) fn stamp(&self) -> Stamp {
        self.root.stamp()
    }
}
