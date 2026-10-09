//! Recording the conversion of an assigned value to the type of its target.
//!
//! `l := d` on an `LINT` and a `DINT` stores the `DINT` widened to `LINT`.
//! This module records that conversion on the value, as an
//! [`ExprKind::ImplicitConversion`] to the declared type of the target, as
//! the elementary type it is operated as: its own, or its base type for a
//! subrange. The target of a dereference is the variable the reference
//! refers to. The bounds and step of a `FOR` loop are converted to the type
//! of its control variable the same way.
//!
//! It records a conversion where the value has a type of its own, which a
//! literal does not. A value converts to its context when it is a variable, an
//! operation that computes at its own result type, or a parenthesized one of
//! those, and its operation width differs from the target's. An operation
//! computes at its own type when it is arithmetic, a negation or `NOT`, an
//! `AND`, `OR` or `XOR`, or a standard function on one value or of several
//! inputs of one type ([`Intrinsic::computes_at_own_type`]). A call to a
//! user-defined function or a method, and a dereference, have the type the
//! function or method returns or the referenced variable has, and convert to
//! their context too. Any other value -- a literal, a call to `TRUNC` -- is
//! compiled at the target's width rather than converted to it, so there is
//! no conversion to record.
//!
//! A directly represented target is not recorded yet.

use ironplc_dsl::textual::{
    Assignment, Expr, ExprKind, For, Function, ParamAssignmentKind, PartialAccessSize,
    SymbolicVariableKind, Variable,
};
use ironplc_dsl::type_id::TypeId;

use super::arithmetic::arithmetic_operator;
use super::ImplicitConversions;
use crate::intermediates::conversion_target::concrete;
use crate::intermediates::numeric_operation::{
    numeric_operation_width, operation_width_of, OperationWidth,
};
use crate::intrinsic::{is_bitwise, Intrinsic};
use crate::semantic_type::{ByteSized, SemanticType};
use crate::variable_type;

impl ImplicitConversions<'_> {
    /// Records the conversion of the value of `node` to the type of its
    /// target.
    pub(super) fn record_assignment_value(&self, node: &mut Assignment) {
        // The bind operators are not compiled yet.
        if node.ref_bind || node.set_bind || node.reset_bind {
            return;
        }
        if let Some(target) = self.assigned_at(&node.target, node.deref) {
            self.record_conversion_to(&mut node.value, target);
        }
    }

    /// Records the conversion of the bounds and step of `node` to the type
    /// of its control variable.
    pub(super) fn record_for_bounds(&self, node: &mut For) {
        let Some(control) = self.control_at(node) else {
            return;
        };
        self.record_conversion_to(&mut node.from, control);
        self.record_conversion_to(&mut node.to, control);
        if let Some(step) = &mut node.step {
            self.record_conversion_to(step, control);
        }
    }

    /// Records the conversion of `value` to `target`, the type its context
    /// stores it at, where the code generator converts it: `value` converts
    /// to its context and its operation width differs from the target's.
    pub(super) fn record_conversion_to(&self, value: &mut Expr, target: TypeId) {
        let Some(width) = self.stored_width(target) else {
            return;
        };
        let own = concrete(value).and_then(|own| self.stored_width(own));
        if self.converts_to_its_context(value) && own.is_some_and(|own| own != width) {
            self.convert(value, target);
        }
    }

    /// The operation width of a value of type `id` when it is a number, a bit
    /// string or a time or date, the types a value is converted between when
    /// stored, and of a subrange's base type.
    ///
    /// A time or date is converted as a number is: `ld := d` on an `LDATE`
    /// and a `DATE` widens the unsigned seconds of the `DATE`. The code
    /// generator used to read the `DATE` at the target's 64 bits, which
    /// sign-extended a date after 2038.
    fn stored_width(&self, id: TypeId) -> Option<OperationWidth> {
        let representation = self
            .context
            .types()
            .get_by_id(id)?
            .representation
            .operated_as();
        match representation {
            SemanticType::Int { .. }
            | SemanticType::UInt { .. }
            | SemanticType::Real { .. }
            | SemanticType::Bytes { .. }
            | SemanticType::Time { .. }
            | SemanticType::Date { .. }
            | SemanticType::TimeOfDay { .. }
            | SemanticType::DateAndTime { .. } => operation_width_of(representation),
            // A `BOOL` or an enumeration is one width, a reference is stored as
            // the reference it is, and anything else is not a single value.
            SemanticType::Bool
            | SemanticType::Enumeration { .. }
            | SemanticType::Reference { .. }
            | SemanticType::Subrange { .. }
            | SemanticType::String { .. }
            | SemanticType::Structure { .. }
            | SemanticType::Array { .. }
            | SemanticType::FunctionBlock { .. }
            | SemanticType::Function { .. } => None,
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
        let types = self.context.types();
        let id = types.id_of(&types.elementary_type_name_for(representation.operated_as())?)?;
        Some((id, self.stored_width(id)?))
    }

    pub(super) fn type_of(&self, kind: &SymbolicVariableKind) -> Option<SemanticType> {
        variable_type::of(kind, self.context, &self.scope.current())
    }

    /// The operation width of the numeric type of `expr`.
    pub(super) fn width_of(&self, expr: &Expr) -> Option<OperationWidth> {
        numeric_operation_width(&self.conversions.operand_name(expr)?)
    }

    /// The operation width of the numeric type `id`.
    pub(super) fn width_of_type(&self, id: TypeId) -> Option<OperationWidth> {
        numeric_operation_width(&self.conversions.name_of(id)?)
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
            // A user-defined function returns its declared type. The function
            // form of an arithmetic operator, an operation on one value and a
            // function of several inputs of one type compute at their own
            // type, and any other standard function at its context's.
            ExprKind::Function(func) => match self.intrinsic_of(func) {
                None => true,
                Some(intrinsic) => {
                    self.is_numeric_pair(func, expr) || intrinsic.computes_at_own_type()
                }
            },
            // A method returns its declared type, and a dereference reads the
            // referenced variable at its own.
            ExprKind::MethodCall(_) | ExprKind::Deref(_) => true,
            // `AND`, `OR` and `XOR` compute at their own type, as an
            // arithmetic operation does; a comparison is a `BOOL`.
            ExprKind::Compare(compare) => is_bitwise(&compare.op),
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
