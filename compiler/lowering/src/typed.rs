//! The type the analyzer recorded for an expression, as lowering reads it:
//! the lowered type of the expression's value, and how a value of that type
//! is operated on, which chooses the lowered form of each operator.
//!
//! Every question the walk has about an expression's type it asks
//! [`Lowering::typed`], once, and answers from the [`Class`] it gets back;
//! nothing else reads the type table to classify an expression.

use ironplc_dsl::textual::Expr as AstExpr;
use ironplc_ir::lowered::{Encoding, ScalarType, TemporalKind, TypeId, TypeKind, TypeTable};

use crate::{Lowered, Lowering, Unlowered};

/// The type the analyzer recorded for an expression, as lowering reads it.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Typed {
    /// The lowered type of the expression's value.
    pub(crate) ty: TypeId,
    /// How a value of the type is operated on.
    pub(crate) class: Class,
    /// The type a value is operated at; `None` for a string or an
    /// aggregate, which is not operated on as one value.
    operated: Option<ScalarType>,
}

impl Typed {
    /// The type the expression is operated at. A string or an aggregate is
    /// not operated on as one value, so is not lowered as a scalar.
    pub(crate) fn scalar(self) -> Lowered<ScalarType> {
        self.operated.ok_or(Unlowered::NotYet)
    }
}

/// How a value of a type is operated on, which chooses the lowered form of
/// each operator on it.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Class {
    /// A `BOOL`: the logical form of the logical operators.
    Bool,
    /// An integer or a bit string, or a subrange of one: arithmetic at its
    /// operation type, and the bitwise form of the logical operators.
    Integer,
    /// A real: arithmetic at its operation type.
    Real,
    /// A time or a date, stored as a count in the unit its kind counts in
    /// (ADR-0025): the arithmetic of `time.rs`.
    Temporal(TemporalKind),
    /// An enumeration: its ordinal is compared and stored, and nothing
    /// computes on it.
    Enumeration,
    /// A reference: compared with `NULL`, and dereferenced.
    Reference,
    /// A string of the encoding: a `StrExpr` (`string.rs`), not a scalar.
    String(Encoding),
    /// An array, a structure, or an instance of a function block or
    /// program: a place of elements or fields, not a value.
    Aggregate,
}

/// Which arithmetic `+`, `-`, `*`, `/` and `MOD` compute on a type.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Arithmetic {
    /// On a number or a bit string, at its operation type.
    Numeric,
    /// On the count a time or a date is stored as (`time.rs`).
    Temporal,
}

/// Which form `NOT`, `AND`, `OR` and `XOR`, and the short circuits
/// `AND_THEN` and `OR_ELSE`, take on a type (REQ-LOW-lowering-048).
#[derive(Clone, Copy, Debug)]
pub(crate) enum LogicalForm {
    /// On a `BOOL`: of the truth value.
    Logical,
    /// On an integer or a bit string: of each bit.
    Bitwise,
}

impl Class {
    /// Which arithmetic computes on the type. None computes on a `BOOL`, an
    /// enumeration, a reference, a string or an aggregate, so an expression
    /// of one is not lowered as arithmetic.
    pub(crate) fn arithmetic(self) -> Lowered<Arithmetic> {
        match self {
            Class::Integer | Class::Real => Ok(Arithmetic::Numeric),
            Class::Temporal(_) => Ok(Arithmetic::Temporal),
            Class::Bool
            | Class::Enumeration
            | Class::Reference
            | Class::String(_)
            | Class::Aggregate => Err(Unlowered::NotYet),
        }
    }

    /// Which form the logical operators take on the type. They apply to no
    /// type but a `BOOL`, an integer and a bit string, so an expression of
    /// another is not lowered.
    pub(crate) fn logical_form(self) -> Lowered<LogicalForm> {
        match self {
            Class::Bool => Ok(LogicalForm::Logical),
            Class::Integer => Ok(LogicalForm::Bitwise),
            Class::Real
            | Class::Temporal(_)
            | Class::Enumeration
            | Class::Reference
            | Class::String(_)
            | Class::Aggregate => Err(Unlowered::NotYet),
        }
    }

    /// Which time or date the type counts, when it is one.
    pub(crate) fn temporal(self) -> Option<TemporalKind> {
        match self {
            Class::Temporal(kind) => Some(kind),
            Class::Bool
            | Class::Integer
            | Class::Real
            | Class::Enumeration
            | Class::Reference
            | Class::String(_)
            | Class::Aggregate => None,
        }
    }
}

impl Lowering<'_> {
    /// The type the analyzer recorded for `expr`, as lowering reads it. An
    /// expression it recorded no type for, `NULL` or a literal it left in
    /// its category, has none to lower by.
    pub(crate) fn typed(&mut self, expr: &AstExpr) -> Lowered<Typed> {
        let recorded = expr
            .expr_type
            .as_ref()
            .and_then(|ty| ty.type_id())
            .ok_or(Unlowered::NotYet)?;
        let ty = self.lowered_type(recorded)?;
        let types = self.program.types();
        let class = class_of(types, ty).ok_or(Unlowered::NotYet)?;
        Ok(Typed {
            ty,
            class,
            operated: types.operation_type(ty),
        })
    }
}

/// How a value of `ty` is operated on; `None` for a type `types` does not
/// hold, as one declared and not yet defined.
fn class_of(types: &TypeTable, ty: TypeId) -> Option<Class> {
    Some(match types.kind(ty)? {
        TypeKind::Bool => Class::Bool,
        TypeKind::Integer { .. } | TypeKind::BitString { .. } => Class::Integer,
        TypeKind::Subrange { base, .. } => class_of(types, *base)?,
        TypeKind::Real { .. } => Class::Real,
        TypeKind::Temporal { kind, .. } => Class::Temporal(*kind),
        TypeKind::Enumeration { .. } => Class::Enumeration,
        TypeKind::Reference { .. } => Class::Reference,
        TypeKind::String(shape) => Class::String(shape.encoding),
        TypeKind::Array { .. }
        | TypeKind::Structure { .. }
        | TypeKind::FunctionBlock { .. }
        | TypeKind::Program { .. } => Class::Aggregate,
    })
}
