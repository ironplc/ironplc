//! Giving a type to a call whose result is an integer of its context's type.
//!
//! `TRUNC`, `BCD_TO_INT` and `SIZEOF` return `ANY_INT`, and nothing about
//! their inputs says which integer
//! ([`Intrinsic::result_is_integer_of_context`]). The resolver gives such a
//! call an untyped integer literal's category, and it takes the type of its
//! context as a literal does (ADR-0028). Unlike a literal it stays an
//! integer: in a context that is not an integer type, such as a `REAL`, it
//! takes `DINT`, the type an untyped integer literal defaults to, and is
//! converted to the context's type. The code generator compiled `TRUNC` at
//! its context's type, so `r := TRUNC(x)` with a `REAL` target did not
//! truncate at all.

use ironplc_dsl::common::GenericTypeName;
use ironplc_dsl::textual::{Expr, ExprKind, ExprType};
use ironplc_dsl::type_id::TypeId;

use super::ImplicitConversions;
use crate::semantic_type::SemanticType;

impl ImplicitConversions<'_> {
    /// Makes `operand` a value of the type `target`, as
    /// [`ConversionTarget::convert`](crate::intermediates::conversion_target::ConversionTarget::convert)
    /// does, except that a call whose result is an integer of its context's
    /// type stays an integer (see [`Self::settle_integer_result`]).
    pub(super) fn convert(&self, operand: &mut Expr, target: TypeId) {
        if self.is_integer_result_in_other_context(operand, target) {
            self.settle_integer_result(operand, target);
        } else {
            self.conversions.convert(operand, target);
        }
    }

    /// Returns `true` when `operand` is a call whose result is an integer of
    /// its context's type, not given a type yet, and `target` is not an
    /// integer type, so the call cannot simply take it.
    pub(super) fn is_integer_result_in_other_context(
        &self,
        operand: &Expr,
        target: TypeId,
    ) -> bool {
        matches!(
            operand.expr_type,
            Some(ExprType::Literal(GenericTypeName::AnyInt))
        ) && matches!(
            &operand.kind,
            ExprKind::Function(func) if self
                .intrinsic_of(func)
                .is_some_and(|intrinsic| intrinsic.result_is_integer_of_context())
        ) && !self.is_integer_type(target)
    }

    /// Gives `operand`, a call whose result is an integer of its context's
    /// type, the type `DINT` and converts it to `target`.
    pub(super) fn settle_integer_result(&self, operand: &mut Expr, target: TypeId) {
        let Some(dint) = self.dint() else {
            return;
        };
        operand.expr_type = Some(ExprType::Inferred(dint));
        self.conversions.convert(operand, target);
    }

    /// Returns `true` for a signed or unsigned integer type, or a subrange
    /// of one.
    fn is_integer_type(&self, id: TypeId) -> bool {
        let Some(attributes) = self.context.types().get_by_id(id) else {
            return false;
        };
        let representation = match &attributes.representation {
            SemanticType::Subrange { base_type, .. } => base_type.as_ref(),
            other => other,
        };
        matches!(
            representation,
            SemanticType::Int { .. } | SemanticType::UInt { .. }
        )
    }
}
