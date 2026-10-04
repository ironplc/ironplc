//! Making an operand a value of the type its operation computes at.
//!
//! Every construct whose operands are converted without the program saying so
//! (ADR-0056) asks the same two questions of an operand once the type it
//! converts to is settled: is it an untyped literal, which takes the type, or
//! a scalar of another type, which is wrapped in an
//! [`ExprKind::ImplicitConversion`]? This is where they are answered, so that
//! each construct the pass records settles only its own target type.

use ironplc_dsl::common::{GenericTypeName, TypeName};
use ironplc_dsl::textual::{Expr, ExprKind, ExprType};
use ironplc_dsl::type_id::TypeId;

use crate::semantic_type::SemanticType;
use crate::type_environment::TypeEnvironment;
use crate::value_type::operand_type_name;

/// Converts operands to a target type, using `types` to tell a scalar from
/// anything else and one type from another name for it.
pub(crate) struct ConversionTarget<'a> {
    types: &'a TypeEnvironment,
}

impl<'a> ConversionTarget<'a> {
    pub(crate) fn new(types: &'a TypeEnvironment) -> Self {
        Self { types }
    }

    /// Makes `operand` a value of the type `target`: an untyped literal is
    /// given the type, and a scalar operand of another type is wrapped
    /// in a conversion to it.
    pub(crate) fn convert(&self, operand: &mut Expr, target: TypeId) {
        match operand.expr_type {
            Some(ExprType::Literal(_)) => operand.expr_type = Some(ExprType::Concrete(target)),
            Some(ExprType::Concrete(own)) if self.needs_conversion(own, target) => {
                let placeholder = Expr::new(ExprKind::Null(operand.span.clone()));
                let inner = std::mem::replace(operand, placeholder);
                *operand = Expr::implicit_conversion(inner, target);
            }
            Some(ExprType::Concrete(_) | ExprType::Null) | None => {}
        }
    }

    /// Returns `true` when a value of type `own` is converted to be operated
    /// on at `target`: both are scalars, and they are different types rather
    /// than one type under two names (an alias and the type it aliases, or
    /// an anonymous subrange and its base type).
    ///
    /// An enumeration or a reference is operated on as the type it is.
    pub(crate) fn needs_conversion(&self, own: TypeId, target: TypeId) -> bool {
        own != target
            && self.is_scalar(own)
            && self.is_scalar(target)
            && self.name_of(own) != self.name_of(target)
    }

    /// Returns `true` for an elementary type, or a subrange of one.
    pub(crate) fn is_scalar(&self, id: TypeId) -> bool {
        self.representation(id).is_some_and(|representation| {
            representation.is_primitive() || representation.is_subrange()
        })
    }

    /// Returns `true` when `expr` is a string, or an untyped string literal.
    pub(crate) fn is_string(&self, expr: &Expr) -> bool {
        match &expr.expr_type {
            Some(ExprType::Concrete(id)) => {
                matches!(self.representation(*id), Some(SemanticType::String { .. }))
            }
            Some(ExprType::Literal(generic)) => *generic == GenericTypeName::AnyString,
            Some(ExprType::Null) | None => false,
        }
    }

    /// The name of the type of `expr`, when it has one.
    pub(crate) fn operand_name(&self, expr: &Expr) -> Option<TypeName> {
        operand_type_name(self.types, expr.expr_type.as_ref()?)
    }

    /// The id of the type named `name`.
    pub(crate) fn id_of(&self, name: &TypeName) -> Option<TypeId> {
        self.types.id_of(name)
    }

    fn representation(&self, id: TypeId) -> Option<&SemanticType> {
        self.types
            .get_by_id(id)
            .map(|attributes| &attributes.representation)
    }

    fn name_of(&self, id: TypeId) -> Option<TypeName> {
        operand_type_name(self.types, &ExprType::Concrete(id))
    }
}

/// The type of `expr` when it is a value of one concrete type.
pub(crate) fn concrete(expr: &Expr) -> Option<TypeId> {
    match expr.expr_type {
        Some(ExprType::Concrete(id)) => Some(id),
        Some(ExprType::Literal(_) | ExprType::Null) | None => None,
    }
}
