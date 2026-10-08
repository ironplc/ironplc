//! Recording the conversions of the inputs of a function of several inputs of
//! one type.
//!
//! `MIN`, `MAX`, `LIMIT`, `SEL`, `MUX`, `EXPT` and `ATAN2` compute at their
//! result type, which the resolver gave the call: the type every input of the
//! function's type widens to, or `EXPT`'s first input's
//! ([`Intrinsic::inputs_of_one_type`]). This module records, on each input of
//! that type:
//!
//! * an [`ExprKind::ImplicitConversion`] to the result type when its operation
//!   width differs from the result's: `MAX(i, l)` on an `INT` and an `LINT`
//!   compares `LINT(i)` with `l`;
//! * the result type on an untyped literal: the `5` of `MAX(d, 5)` is a
//!   `DINT`.
//!
//! The operator forms of `AND`, `OR` and `XOR` compute at their result type
//! too, the type both operands widen to, and their operands are recorded the
//! same way: `w OR lw` on a `WORD` and an `LWORD` is `LWORD(w) OR lw`.
//!
//! `MUX`'s selector `K` is converted to `DINT` when its width differs, and
//! `SEL`'s `G` stays a `BOOL`. A call whose inputs are all untyped literals
//! has a literal's category until the literal pass gives it its context's
//! type, which records its inputs then. The conversion of the result to the
//! type of its context is recorded where that context is.

use ironplc_dsl::common::{ElementaryTypeName, TypeName};
use ironplc_dsl::textual::{Expr, ExprKind, ExprType, Function, ParamAssignmentKind};
use ironplc_dsl::type_id::TypeId;

use super::ImplicitConversions;
use crate::intermediates::numeric_operation::{operation_width_of, OperationWidth};
use crate::intrinsic::{is_bitwise, Intrinsic};

impl ImplicitConversions<'_> {
    /// Records the conversions of the inputs of `expr` when it is a call to a
    /// function of several inputs of one type whose own type is settled.
    pub(super) fn record_one_type_inputs(&self, expr: &mut Expr) {
        let Some(result) = expr.expr_type.as_ref().and_then(ExprType::type_id) else {
            return;
        };
        if let ExprKind::Function(func) = &mut expr.kind {
            self.record_one_type_call(func, result);
        }
    }

    /// Records the conversions of the operands of `expr` when it is an `AND`,
    /// `OR` or `XOR` whose own type is settled, to that type.
    pub(super) fn record_bitwise_operands(&self, expr: &mut Expr) {
        let Some(result) = expr.expr_type.as_ref().and_then(ExprType::type_id) else {
            return;
        };
        let ExprKind::Compare(compare) = &mut expr.kind else {
            return;
        };
        if !is_bitwise(&compare.op) {
            return;
        }
        let Some(width) = self.operation_width(result) else {
            return;
        };
        self.record_one_type_input(&mut compare.left, (result, width));
        self.record_one_type_input(&mut compare.right, (result, width));
    }

    /// Records the conversions of the inputs of `func`, when it is a call to
    /// a function of several inputs of one type, to `result`, the type the
    /// call computes at.
    pub(super) fn record_one_type_call(&self, func: &mut Function, result: TypeId) {
        let Some(intrinsic) = self.intrinsic_of(func) else {
            return;
        };
        let Some(shape) = intrinsic.inputs_of_one_type() else {
            return;
        };
        let Some(width) = self.operation_width(result) else {
            return;
        };
        let dint = self.dint_step();
        // The named-argument pass made every input positional; any other
        // shape is one a rule reported.
        let inputs = func
            .param_assignment
            .iter_mut()
            .filter_map(|input| match input {
                ParamAssignmentKind::PositionalInput(input) => Some(&mut input.expr),
                ParamAssignmentKind::NamedInput(_) | ParamAssignmentKind::Output(_) => None,
            });
        for (index, input) in inputs.enumerate() {
            if index >= shape.first {
                self.record_one_type_input(input, (result, width));
            } else if intrinsic == Intrinsic::Mux {
                // `K` is an index the multiplexer reads as a `DINT`.
                if let Some(dint) = dint {
                    self.record_one_type_input(input, dint);
                }
            }
        }
    }

    /// Makes `input` a value of the type `target`, of operation width
    /// `width`: an untyped literal takes the type, and an input of another
    /// width is converted to it. An input of the same width is computed with
    /// as it is.
    fn record_one_type_input(&self, input: &mut Expr, (target, width): (TypeId, OperationWidth)) {
        let differs = match &input.expr_type {
            Some(ExprType::Literal(_)) => true,
            Some(ExprType::Concrete(own) | ExprType::Inferred(own)) => {
                self.operation_width(*own).is_some_and(|own| own != width)
            }
            Some(ExprType::Null) | None => false,
        };
        if differs {
            self.conversions.convert(input, target);
        }
    }

    /// The width a value of the type `id` is operated on at, when it is one
    /// value: an elementary type, a subrange by its base type, or an
    /// enumeration.
    fn operation_width(&self, id: TypeId) -> Option<OperationWidth> {
        operation_width_of(&self.context.types().get_by_id(id)?.representation)
    }

    /// `DINT`, the type `MUX` reads its selector as, and its width.
    fn dint_step(&self) -> Option<(TypeId, OperationWidth)> {
        let dint: TypeName = ElementaryTypeName::DINT.into();
        Some((self.context.types().id_of(&dint)?, OperationWidth::W32))
    }
}
