//! Lowering pass recording, in the AST, how each operand of a comparison is
//! converted before it is compared.
//!
//! A comparison (`=`, `<>`, `<`, `<=`, `>`, `>=`) compares its operands at
//! one operand type: the type of the concrete left operand, else of the
//! concrete right one. This pass settles that type and makes it visible on
//! the operands:
//!
//! * a variable of a numeric or bit-string type other than the operand type
//!   is wrapped in an [`ExprKind::ImplicitConversion`] to it, so `l > d` on
//!   an `LINT` and a `DINT` becomes `l > LINT(d)`;
//! * an untyped literal operand is given the operand type, so the `1` of
//!   `1 < l` is an `LINT`.
//!
//! A backend compiles the comparison at the type of its left operand, and
//! compiles a conversion where the node says, without choosing either
//! itself. See ADR-0056.
//!
//! The pass records what codegen did before it, and no more: only a variable
//! of a numeric or bit-string type was read at its own type and converted.
//! Any other operand of another type -- a call, an expression, a temporal
//! variable -- is compiled at the operand type, so a wider one is computed
//! narrow (#1920).
//!
//! The function forms (`EQ`, `NE`, `LT`, `LE`, `GT`, `GE`) are not covered:
//! codegen compiles them at the type of the enclosing expression, which is
//! not a conversion this pass can record.
//!
//! The pass runs in `stages::analyze` after the semantic rules, so a rule
//! checks the operands the program wrote. It reports nothing: a comparison
//! it cannot settle is left as it is.

use std::convert::Infallible;

use ironplc_dsl::common::Library;
use ironplc_dsl::fold::Fold;
use ironplc_dsl::textual::{CompareExpr, Expr, ExprKind, ExprType};
use ironplc_dsl::type_id::TypeId;

use crate::intermediate_type::IntermediateType;
use crate::type_environment::TypeEnvironment;
use crate::value_type::operand_type_name;

pub fn apply(lib: Library, types: &TypeEnvironment) -> Library {
    let mut inserter = ImplicitConversions { types };
    let Ok(lib) = inserter.fold_library(lib);
    lib
}

struct ImplicitConversions<'a> {
    types: &'a TypeEnvironment,
}

impl ImplicitConversions<'_> {
    /// Converts the operands of one comparison to its operand type.
    fn convert_operands(&self, left: &mut Expr, right: &mut Expr) {
        // A string is compared through the data region, in its own encoding;
        // there is nothing to convert it to.
        if self.is_string(left) || self.is_string(right) {
            return;
        }
        let Some(target) = self.operand_type(left, right) else {
            return;
        };
        self.convert(left, target);
        self.convert(right, target);
    }

    /// The type a comparison of `left` and `right` compares at: the concrete
    /// left operand's type, else the concrete right one's, the rule codegen
    /// applied before the analyzer recorded it. A wider right operand is
    /// therefore narrowed (#1920).
    fn operand_type(&self, left: &Expr, right: &Expr) -> Option<TypeId> {
        concrete(left).or_else(|| concrete(right))
    }

    /// Makes `operand` a value of the type `target`: an untyped literal is
    /// given the type, and a numeric or bit-string variable of another type
    /// is wrapped in a conversion to it.
    fn convert(&self, operand: &mut Expr, target: TypeId) {
        match operand.expr_type {
            Some(ExprType::Literal(_)) => operand.expr_type = Some(ExprType::Concrete(target)),
            Some(ExprType::Concrete(own))
                if matches!(operand.kind, ExprKind::Variable(_))
                    && self.needs_conversion(own, target) =>
            {
                let placeholder = Expr::new(ExprKind::Null(operand.span.clone()));
                let inner = std::mem::replace(operand, placeholder);
                *operand = Expr::implicit_conversion(inner, target);
            }
            Some(ExprType::Concrete(_) | ExprType::Null) | None => {}
        }
    }

    /// Returns `true` when a variable of type `own` is converted to be
    /// compared at `target`: `own` is numeric or a bit string, `target` is a
    /// scalar, and they are different types rather than one type under two
    /// names (an alias and the type it aliases).
    fn needs_conversion(&self, own: TypeId, target: TypeId) -> bool {
        own != target
            && self.is_numeric_or_bit_string(own)
            && self.is_scalar(target)
            && self.name_of(own) != self.name_of(target)
    }

    fn is_numeric_or_bit_string(&self, id: TypeId) -> bool {
        matches!(
            self.representation(id),
            Some(
                IntermediateType::Int { .. }
                    | IntermediateType::UInt { .. }
                    | IntermediateType::Real { .. }
                    | IntermediateType::Bytes { .. }
            )
        )
    }

    /// Returns `true` for an elementary type, or a subrange of one.
    fn is_scalar(&self, id: TypeId) -> bool {
        self.representation(id).is_some_and(|representation| {
            representation.is_primitive() || representation.is_subrange()
        })
    }

    fn is_string(&self, expr: &Expr) -> bool {
        match &expr.expr_type {
            Some(ExprType::Concrete(id)) => matches!(
                self.representation(*id),
                Some(IntermediateType::String { .. })
            ),
            Some(ExprType::Literal(generic)) => {
                *generic == ironplc_dsl::common::GenericTypeName::AnyString
            }
            Some(ExprType::Null) | None => false,
        }
    }

    fn representation(&self, id: TypeId) -> Option<&IntermediateType> {
        self.types
            .get_by_id(id)
            .map(|attributes| &attributes.representation)
    }

    fn name_of(&self, id: TypeId) -> Option<ironplc_dsl::common::TypeName> {
        operand_type_name(self.types, &ExprType::Concrete(id))
    }
}

/// The type of `expr` when it is a value of one concrete type.
fn concrete(expr: &Expr) -> Option<TypeId> {
    match expr.expr_type {
        Some(ExprType::Concrete(id)) => Some(id),
        Some(ExprType::Literal(_) | ExprType::Null) | None => None,
    }
}

impl Fold<Infallible> for ImplicitConversions<'_> {
    fn fold_compare_expr(&mut self, node: CompareExpr) -> Result<CompareExpr, Infallible> {
        let mut node = node.recurse_fold(self)?;
        if node.op.is_comparison() {
            self.convert_operands(&mut node.left, &mut node.right);
        }
        Ok(node)
    }
}

#[cfg(test)]
mod tests;
