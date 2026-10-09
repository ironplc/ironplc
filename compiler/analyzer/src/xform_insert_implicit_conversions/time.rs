//! Recording the conversions of the operands of a typed time or date
//! operation.
//!
//! A typed time or date function (IEC 61131-3 Table 30: `ADD_TIME`,
//! `SUB_DT_DT`, `MUL_TIME`, ...), and an operator expression or function form
//! that resolves to one (`lt + t` is `ADD_LTIME`), computes at the width of
//! its form: 32 bits for a short form, 64 for a long one. This module records
//! the conversion of each operand to that width, as an
//! [`ExprKind::ImplicitConversion`](ironplc_dsl::textual::ExprKind):
//!
//! * an operand of a temporal parameter is converted to the parameter's type
//!   when its width differs: the `TIME` of `lt + t` to `LTIME`, and the `DATE`
//!   of `SUB_LDATE_LDATE(ld, d)` to `LDATE`;
//! * the number a long form of `MUL` or `DIV` scales by is converted to
//!   `LINT` when it is an integer of 32 bits and to `LREAL` when it is a
//!   `REAL`, since the routine scales at 64 bits;
//! * a 64-bit integer the short form scales by is converted to `LREAL`, at
//!   which the routine scales the duration, since the integer cannot be
//!   narrowed to the duration's 32 bits.
//!
//! An operand of its parameter's width is operated on as it is, whatever its
//! signedness: the `DATE` of `SUB_DATE_DATE` is unsigned and the routine
//! signed. A short form scaling by a `REAL` converts the duration to the
//! number's type, which is the routine's own computation, not a conversion
//! of either operand to the other.

use ironplc_dsl::common::{ElementaryTypeName, TypeName};
use ironplc_dsl::core::Id;
use ironplc_dsl::textual::{Expr, ExprType, Function, Operator, ParamAssignmentKind};
use ironplc_dsl::type_id::TypeId;

use super::ImplicitConversions;
use crate::intermediates::arithmetic_overload::{typed_overload, Overload};
use crate::intermediates::conversion_target::concrete;
use crate::intermediates::numeric_operation::{
    literal_default_type, operation_width_of, OperationWidth,
};
use crate::intrinsic::Intrinsic;

impl ImplicitConversions<'_> {
    /// Records the conversions of the inputs of `func` when it is a call to a
    /// typed time or date function, such as `ADD_LTIME(lt, t)`.
    pub(super) fn record_time_function_inputs(&self, func: &mut Function) {
        if !matches!(self.intrinsic_of(func), Some(Intrinsic::Time { .. })) {
            return;
        }
        let name = func.name.clone();
        // The named-argument pass made every input positional; any other
        // shape is one a rule reported.
        if let [ParamAssignmentKind::PositionalInput(in1), ParamAssignmentKind::PositionalInput(in2)] =
            func.param_assignment.as_mut_slice()
        {
            self.record_time_inputs(&name, &mut in1.expr, &mut in2.expr);
        }
    }

    /// The typed overload of `op` on `left` and `right`, the name of the
    /// typed time or date function the pair computes as, or `None` when the
    /// pair has none.
    pub(super) fn typed_overload_of(&self, op: &Operator, left: &Expr, right: &Expr) -> Option<Id> {
        let left = self.conversions.operand_name(left)?;
        let right = self.conversions.operand_name(right)?;
        match typed_overload(op, &left, &right)? {
            Overload::Typed { name, .. } => Some(Id::from(name)),
            Overload::Unchecked { .. } | Overload::Numeric { .. } => None,
        }
    }

    /// Records the conversion of `in1` and `in2`, the inputs of the typed
    /// time or date function `name`, to the width it computes at.
    pub(super) fn record_time_inputs(&self, name: &Id, in1: &mut Expr, in2: &mut Expr) {
        let Some(signature) = self.context.functions().get(name) else {
            return;
        };
        let Some(Intrinsic::Time { long, .. }) = signature.intrinsic else {
            return;
        };
        let params: Vec<TypeName> = signature
            .input_parameters()
            .take(2)
            .map(|param| param.param_type)
            .collect();
        for (input, param) in [in1, in2].into_iter().zip(&params) {
            match self.conversions.id_of(param) {
                Some(target) => self.record_temporal_input(input, target),
                // The one parameter that is not a type is the `ANY_NUM` of
                // `MUL` and `DIV`.
                None => self.record_scale(input, long),
            }
        }
    }

    /// Converts `input` to `target`, the type of its temporal parameter,
    /// when its width differs.
    fn record_temporal_input(&self, input: &mut Expr, target: TypeId) {
        let own = concrete(input).and_then(|own| self.width_of_id(own));
        if let (Some(own), Some(width)) = (own, self.width_of_id(target)) {
            if own != width {
                self.convert(input, target);
            }
        }
    }

    /// Converts `input`, the number a `MUL` or `DIV` scales a duration by, to
    /// the type its routine scales at: `LINT` or `LREAL` for a long form, and
    /// `LREAL` for a 64-bit integer of a short form.
    fn record_scale(&self, input: &mut Expr, long: bool) {
        let target = match &input.expr_type {
            Some(ExprType::Literal(generic)) if long => match literal_default_type(generic) {
                Some(ElementaryTypeName::REAL) => ElementaryTypeName::LREAL,
                Some(_) => ElementaryTypeName::LINT,
                None => return,
            },
            Some(ExprType::Concrete(own) | ExprType::Inferred(own)) => {
                match (self.width_of_id(*own), long) {
                    (Some(OperationWidth::W32), true) => ElementaryTypeName::LINT,
                    (Some(OperationWidth::F32), true) | (Some(OperationWidth::W64), false) => {
                        ElementaryTypeName::LREAL
                    }
                    _ => return,
                }
            }
            // A literal a short form scales by takes its default type.
            Some(ExprType::Literal(_) | ExprType::Null) | None => return,
        };
        if let Some(target) = self.conversions.id_of(&target.into()) {
            self.convert(input, target);
        }
    }

    /// The width a value of the type `id` is operated on at.
    fn width_of_id(&self, id: TypeId) -> Option<OperationWidth> {
        operation_width_of(&self.context.types().get_by_id(id)?.representation)
    }
}
