//! The operands of the logical and bitwise operators, `AND`, `OR`, `XOR`
//! and the short-circuit `AND_THEN` and `OR_ELSE`, in the operator and the
//! function form: they must have a type every operand widens to.

use ironplc_dsl::{
    common::TypeName,
    core::Located,
    diagnostic::Label,
    textual::{CompareOp, Expr, Function},
};

use super::RuleOperatorOperandTypeCheck;
use crate::intermediates::common_operand::common_operand;
use crate::intermediates::operator_function_form::{
    form_of_operator, operator_function_form, FormOf,
};
use crate::type_compat::{are_types_compatible, is_checkable_type};

impl RuleOperatorOperandTypeCheck<'_> {
    /// Reports P4049 when the operands of the logical or bitwise operator
    /// `op` have no type both widen to, labelled at the whole expression.
    ///
    /// A narrower bit string widens to a wider one, so `w AND lw` computes
    /// at `LWORD`. A `BOOL` widens to no other type, so `b AND w` on a
    /// `BOOL` and a `WORD` has no type to compute at, rather than one where
    /// the `BOOL` is 0 or 1. An operand that is not a `BOOL` or a bit string
    /// is reported by [`Self::check_operands`] instead, and one whose type is
    /// not known is left alone.
    pub(super) fn check_bitwise_operands(
        &mut self,
        expr: &Expr,
        op: &CompareOp,
        left: &Expr,
        right: &Expr,
    ) {
        let (Some(left), Some(right)) = (self.bit_operand_name(left), self.bit_operand_name(right))
        else {
            return;
        };
        if common_operand(&left, &right, self.options).is_none() {
            self.report_arithmetic(
                Label::span(expr.span(), "Expression"),
                &op.to_string(),
                &left,
                &right,
            );
        }
    }

    /// Reports P4049 when a call to `AND`, `OR` or `XOR` has inputs with no
    /// type every input widens to, labelled at the name: the inputs are
    /// folded from the left, as [`Self::check_bitwise_operands`] checks a
    /// pair, and the step that fails names both types.
    pub(super) fn check_bitwise_call(&mut self, function: &Function) {
        let Some(form) = operator_function_form(&function.name.to_string()) else {
            return;
        };
        let FormOf::Compare(op) = &form.operator else {
            return;
        };
        if !op.is_bitwise() {
            return;
        }
        let mut widest: Option<TypeName> = None;
        for input in function
            .param_assignment
            .iter()
            .filter_map(|p| p.input_expr())
        {
            let Some(name) = self.bit_operand_name(input) else {
                return;
            };
            widest = match widest {
                None => Some(name),
                Some(widest) => match common_operand(&widest, &name, self.options) {
                    Some(common) => Some(common),
                    None => {
                        self.report_arithmetic(
                            Label::span(function.name.span(), "Function call"),
                            &function.name.original().to_string(),
                            &widest,
                            &name,
                        );
                        return;
                    }
                },
            };
        }
    }

    /// The name of `operand`'s type when it is a `BOOL` or a bit string,
    /// the types the logical and bitwise operators take; `None` for any
    /// other type, an untyped literal, or an operand whose type is not
    /// known.
    fn bit_operand_name(&self, operand: &Expr) -> Option<TypeName> {
        let name = self.operand_name(operand)?;
        let row = form_of_operator(&FormOf::Compare(CompareOp::And))?;
        (is_checkable_type(&name) && are_types_compatible(&row.operand_type(), &name, self.options))
            .then_some(name)
    }
}
