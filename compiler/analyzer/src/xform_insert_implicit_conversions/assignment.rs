//! Recording the conversion of an assigned value to the type of its target.
//!
//! `l := d` on an `LINT` and a `DINT` stores the `DINT` widened to `LINT`.
//! This module records that conversion on the value, as an
//! [`ExprKind::ImplicitConversion`] to the type the target is stored as: its
//! own elementary type, or its base type for a subrange.
//!
//! It records the conversions the code generator makes today, and only
//! those. A value converts to its context when it is a variable, an
//! arithmetic operation that computes at its own result type, or a
//! parenthesized one of those, and its operation width differs from the
//! target's. Any other value -- a literal, a negation, a call -- is compiled
//! at the target's width rather than converted to it, so there is no
//! conversion to record. The target of a dereference, of a function block
//! field, or a directly represented variable is not recorded yet.

use ironplc_dsl::textual::{
    Assignment, Expr, ExprKind, Function, ParamAssignmentKind, PartialAccessSize,
    SymbolicVariableKind, Variable,
};
use ironplc_dsl::type_id::TypeId;

use super::arithmetic::arithmetic_operator;
use super::ImplicitConversions;
use crate::intermediates::numeric_operation::{numeric_operation_width, OperationWidth};
use crate::semantic_type::{ByteSized, SemanticType};
use crate::variable_type;

impl ImplicitConversions<'_> {
    /// Records the conversion of the value of `node` to the type of its
    /// target.
    pub(super) fn record_assignment_value(&self, node: &mut Assignment) {
        // A dereference stores at a width only the referenced variable
        // knows, and the bind operators are not compiled yet.
        if node.deref || node.ref_bind || node.set_bind || node.reset_bind {
            return;
        }
        let Some((target, width)) = self.stored_as(&node.target) else {
            return;
        };
        if !self.converts_to_its_context(&node.value) {
            return;
        }
        if self.width_of(&node.value).is_some_and(|own| own != width) {
            self.conversions.convert(&mut node.value, target);
        }
    }

    /// The elementary type the target `target` is stored as, and its
    /// operation width, when it is numeric.
    pub(super) fn stored_as(&self, target: &Variable) -> Option<(TypeId, OperationWidth)> {
        let Variable::Symbolic(kind) = target else {
            return None;
        };
        let representation = match kind {
            SymbolicVariableKind::BitAccess(_) => SemanticType::Bool,
            SymbolicVariableKind::PartialAccess(partial) => SemanticType::Bytes {
                size: match partial.size {
                    PartialAccessSize::Byte => ByteSized::B8,
                    PartialAccessSize::Word => ByteSized::B16,
                    PartialAccessSize::DWord => ByteSized::B32,
                    PartialAccessSize::LWord => ByteSized::B64,
                },
            },
            SymbolicVariableKind::Structured(structured)
                if matches!(
                    self.type_of(&structured.record),
                    Some(SemanticType::FunctionBlock { .. })
                ) =>
            {
                return None
            }
            _ => self.type_of(kind)?,
        };
        let representation = match representation {
            SemanticType::Subrange { base_type, .. } => *base_type,
            other => other,
        };
        let types = self.context.types();
        let name = types.elementary_type_name_for(&representation)?;
        Some((types.id_of(&name)?, numeric_operation_width(&name)?))
    }

    fn type_of(&self, kind: &SymbolicVariableKind) -> Option<SemanticType> {
        variable_type::of(kind, self.context, &self.scope.current())
    }

    /// The operation width of the numeric type of `expr`.
    fn width_of(&self, expr: &Expr) -> Option<OperationWidth> {
        numeric_operation_width(&self.conversions.operand_name(expr)?)
    }

    /// Returns `true` when `expr` is computed at its own type and converted
    /// to the type of its context, rather than computed at the context's.
    fn converts_to_its_context(&self, expr: &Expr) -> bool {
        match &expr.kind {
            ExprKind::Variable(_) | ExprKind::BinaryOp(_) => true,
            ExprKind::Expression(inner) => self.converts_to_its_context(inner),
            ExprKind::Function(func) => self.is_numeric_pair(func, expr),
            _ => false,
        }
    }

    /// Returns `true` when `func`, the call `expr`, is the function form of
    /// an arithmetic operator on two inputs at the width of its result: the
    /// shape the arithmetic pass records a numeric operation in.
    fn is_numeric_pair(&self, func: &Function, expr: &Expr) -> bool {
        if arithmetic_operator(func).is_none() {
            return false;
        }
        let Some(natural) = self.width_of(expr) else {
            return false;
        };
        let at_natural = |input: &ParamAssignmentKind| match input {
            ParamAssignmentKind::PositionalInput(input) => {
                self.width_of(&input.expr) == Some(natural)
            }
            _ => false,
        };
        matches!(func.param_assignment.as_slice(), [left, right] if at_natural(left) && at_natural(right))
    }
}
