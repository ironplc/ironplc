//! Recording the conversion of an assigned value to the type of its target.
//!
//! `l := d` on an `LINT` and a `DINT` stores the `DINT` widened to `LINT`.
//! This module records that conversion on the value, as an
//! [`ExprKind::ImplicitConversion`] to the type the target is stored as: its
//! own elementary type, or its base type for a subrange.
//!
//! It records the conversions the code generator makes today, and only
//! those. A value converts to its context when it is a variable, an
//! operation that computes at its own result type, or a parenthesized one of
//! those, and its operation width differs from the target's. An operation
//! computes at its own type when it is arithmetic, a negation or `NOT`, or a
//! standard function on one value ([`Intrinsic::computes_at_operand_type`]).
//! Any other value -- a literal, a call to `MAX` or to a user-defined
//! function -- is compiled at the target's width rather than converted to it,
//! so there is no conversion to record. The target of a dereference, of a
//! function block field, or a directly represented variable is not recorded
//! yet.

use ironplc_dsl::textual::{
    Assignment, Expr, ExprKind, Function, ParamAssignmentKind, PartialAccessSize,
    SymbolicVariableKind, Variable,
};
use ironplc_dsl::type_id::TypeId;

use super::arithmetic::arithmetic_operator;
use super::ImplicitConversions;
use crate::intermediates::numeric_operation::{numeric_operation_width, OperationWidth};
use crate::intrinsic::Intrinsic;
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
            SymbolicVariableKind::Named(_)
            | SymbolicVariableKind::Array(_)
            | SymbolicVariableKind::Structured(_)
            | SymbolicVariableKind::Deref(_)
            | SymbolicVariableKind::SelfRef(_) => self.type_of(kind)?,
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
    pub(super) fn width_of(&self, expr: &Expr) -> Option<OperationWidth> {
        numeric_operation_width(&self.conversions.operand_name(expr)?)
    }

    /// Returns `true` when `expr` is computed at its own type and converted
    /// to the type of its context, rather than computed at the context's.
    ///
    /// The match names every kind of expression, so that a kind added to
    /// [`ExprKind`] is decided here rather than falling through unconverted.
    fn converts_to_its_context(&self, expr: &Expr) -> bool {
        match &expr.kind {
            // A variable is read at its own type, and an arithmetic
            // operation, a negation or `NOT` computes at its own.
            ExprKind::Variable(_) | ExprKind::BinaryOp(_) | ExprKind::UnaryOp(_) => true,
            // A recorded conversion compiles to the type it records, then to
            // its context's.
            ExprKind::ImplicitConversion(_) => true,
            ExprKind::Expression(inner) => self.converts_to_its_context(inner),
            // The function form of an arithmetic operator and an operation on
            // one value compute at their own type. Any other standard
            // function computes at its context's, and a user-defined
            // function's result is not converted (#2126).
            ExprKind::Function(func) => {
                self.is_numeric_pair(func, expr)
                    || self
                        .intrinsic_of(func)
                        .is_some_and(|intrinsic| intrinsic.computes_at_operand_type())
            }
            // Computed or read at its own type and not converted (#2126).
            ExprKind::Compare(_) | ExprKind::MethodCall(_) | ExprKind::Deref(_) => false,
            // A literal compiles at the type the literal pass gives it, and a
            // late-bound name is read at its context's type.
            ExprKind::Const(_) | ExprKind::LateBound(_) => false,
            // Not a number: an enumeration's ordinal, or a reference.
            ExprKind::EnumeratedValue(_) | ExprKind::Ref(_) | ExprKind::Null(_) => false,
        }
    }

    /// The operation the standard function `func` calls, or `None` for a
    /// user-defined function.
    pub(super) fn intrinsic_of(&self, func: &Function) -> Option<Intrinsic> {
        self.context.functions().get(&func.name)?.intrinsic.clone()
    }

    /// Returns `true` when `func`, the call `expr`, is the function form of
    /// an arithmetic operator on two inputs at the width of its result: the
    /// shape the arithmetic pass records a numeric operation in.
    pub(super) fn is_numeric_pair(&self, func: &Function, expr: &Expr) -> bool {
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
            ParamAssignmentKind::NamedInput(_) | ParamAssignmentKind::Output(_) => false,
        };
        matches!(func.param_assignment.as_slice(), [left, right] if at_natural(left) && at_natural(right))
    }
}
