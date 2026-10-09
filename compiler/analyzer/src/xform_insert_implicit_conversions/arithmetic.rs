//! Recording the conversions of arithmetic operands.
//!
//! An arithmetic operation computes at the type of its result (ADR-0001):
//! `INT + REAL` converts the `INT` to `REAL` and adds as `REAL`. This module
//! records that, for the operator `a + b` and for the function form
//! `ADD(a, b, ...)`:
//!
//! * an operand whose operation width differs from the result's is wrapped in
//!   an [`ExprKind::ImplicitConversion`] to the result type;
//! * an untyped literal operand is given the result type;
//! * a function form of three or more inputs is written as the calls it folds
//!   to, `ADD(ADD(a, b), c)`, because each step computes at its own result
//!   type and the accumulated value is converted between steps.
//!
//! A pair with a typed overload on the time and date types compiles through
//! the routine that knows the units of each type, and its operands are
//! converted to the width the routine computes at (see `time.rs`); a function
//! form of three or more inputs whose steps are typed overloads is written as
//! the calls it folds to the same way. An operation whose result is not an
//! elementary numeric type is left alone.
//!
//! The conversion of the result to the type of its context is recorded where
//! that context is: the assignment, the argument or the comparison.

use ironplc_dsl::common::TypeName;
use ironplc_dsl::core::Id;
use ironplc_dsl::textual::{
    BinaryExpr, Expr, ExprKind, ExprType, Function, Operator, ParamAssignmentKind, PositionalInput,
};
use ironplc_dsl::type_id::TypeId;

use super::ImplicitConversions;
use crate::intermediates::arithmetic_overload::{
    resolve_arithmetic_overload, typed_overload, Overload,
};
use crate::intermediates::numeric_operation::{numeric_operation_width, OperationWidth};
use crate::intermediates::operator_function_form::{operator_function_form, FormOf};

/// The type an arithmetic step computes at, and the width it is operated on.
type Step = (TypeId, OperationWidth);

impl ImplicitConversions<'_> {
    /// Records the conversions of the operands of the operator expression
    /// `binary`, whose own type is `result`.
    pub(super) fn record_binary_operands(
        &self,
        binary: &mut BinaryExpr,
        result: Option<&ExprType>,
    ) {
        if let Some(name) = self.typed_overload_of(&binary.op, &binary.left, &binary.right) {
            self.record_time_inputs(&name, &mut binary.left, &mut binary.right);
            return;
        }
        let Some(step) = self.step_of(result) else {
            return;
        };
        self.record_operand(&mut binary.left, step);
        self.record_operand(&mut binary.right, step);
    }

    /// Records the conversions of the inputs of `expr`, a call to the function
    /// form of an arithmetic operator, when every step of the fold is the
    /// numeric overload or every step a typed one.
    pub(super) fn record_fold_operands(&self, expr: &mut Expr) {
        let ExprKind::Function(func) = &expr.kind else {
            return;
        };
        let Some(op) = arithmetic_operator(func) else {
            return;
        };
        let Some(inputs) = positional_inputs(func) else {
            return;
        };
        if let Some(steps) = self.numeric_steps(&op, &inputs, expr.expr_type.as_ref()) {
            let types = steps.iter().map(|step| Some(ExprType::Concrete(step.0)));
            self.fold_into_calls(expr, types.collect(), |accumulated, input, index| {
                self.record_operand(accumulated, steps[index]);
                self.record_operand(input, steps[index]);
            });
        } else if let Some(steps) = self.typed_steps(&op, &inputs) {
            let types = steps
                .iter()
                .map(|(_, result)| self.conversions.id_of(result).map(ExprType::Concrete));
            self.fold_into_calls(expr, types.collect(), |accumulated, input, index| {
                self.record_time_inputs(&steps[index].0, accumulated, input);
            });
        }
    }

    /// Writes `expr`, a call of two or more inputs, as the calls its inputs
    /// fold to from the left, each step's call of the type in `types` and the
    /// last of the type of `expr` itself, recording each step's operands by
    /// `record` (the accumulated value, the input, the step's index).
    fn fold_into_calls(
        &self,
        expr: &mut Expr,
        types: Vec<Option<ExprType>>,
        record: impl Fn(&mut Expr, &mut Expr, usize),
    ) {
        let ExprKind::Function(func) = &expr.kind else {
            return;
        };
        let name = func.name.clone();
        let span = expr.span.clone();
        let result = expr.expr_type.clone();
        let ExprKind::Function(func) = &mut expr.kind else {
            return;
        };
        let mut inputs = std::mem::take(&mut func.param_assignment)
            .into_iter()
            .filter_map(|input| match input {
                ParamAssignmentKind::PositionalInput(input) => Some(input.expr),
                ParamAssignmentKind::NamedInput(_) | ParamAssignmentKind::Output(_) => None,
            });
        let Some(mut accumulated) = inputs.next() else {
            return;
        };
        let last = types.len() - 1;
        for (index, (input, step_type)) in inputs.zip(types).enumerate() {
            let mut input = input;
            record(&mut accumulated, &mut input, index);
            let call = Function {
                name: name.clone(),
                param_assignment: vec![positional(accumulated), positional(input)],
            };
            let step_result = if index == last {
                result.clone()
            } else {
                step_type
            };
            accumulated = Expr {
                kind: ExprKind::Function(call),
                expr_type: step_result,
                span: span.clone(),
            };
        }
        *expr = accumulated;
    }

    /// The type and width of each step of the fold of `inputs`, or `None`
    /// unless there are two or more inputs and every step is the numeric
    /// overload with a result this pass can place.
    ///
    /// The last step computes at the type of the call itself, which must be
    /// the overload's result. Each earlier step computes at its overload's
    /// result.
    fn numeric_steps(
        &self,
        op: &Operator,
        inputs: &[&Expr],
        result: Option<&ExprType>,
    ) -> Option<Vec<Step>> {
        let (first, rest) = inputs.split_first()?;
        if rest.is_empty() {
            return None;
        }
        if self.has_typed_overload(op, first, rest[0]) {
            return None;
        }
        let mut accumulated = self.conversions.operand_name(first);
        let mut results: Vec<TypeName> = Vec::with_capacity(rest.len());
        for input in rest {
            let overload = resolve_arithmetic_overload(
                op,
                accumulated.as_ref(),
                self.conversions.operand_name(input).as_ref(),
                self.options,
            )?;
            let Overload::Numeric { result } = overload else {
                return None;
            };
            accumulated = Some(result.clone());
            results.push(result);
        }
        let mut steps = Vec::with_capacity(results.len());
        for (index, step_result) in results.iter().enumerate() {
            let step = if index + 1 == results.len() {
                // The call's own type is what a context sees and what the
                // last step computes at, so they must agree.
                let step = self.step_of(result)?;
                let own = self.conversions.name_of(step.0)?;
                (own == *step_result).then_some(step)?
            } else {
                let id = self.conversions.id_of(step_result)?;
                (id, numeric_operation_width(step_result)?)
            };
            steps.push(step);
        }
        Some(steps)
    }

    /// The typed time or date function each step of the fold of `inputs`
    /// computes as, with the step's result type, or `None` unless there are
    /// two or more inputs and every step has a typed overload.
    fn typed_steps(&self, op: &Operator, inputs: &[&Expr]) -> Option<Vec<(Id, TypeName)>> {
        let (first, rest) = inputs.split_first()?;
        if rest.is_empty() {
            return None;
        }
        let mut accumulated = self.conversions.operand_name(first)?;
        rest.iter()
            .map(|input| {
                let right = self.conversions.operand_name(input)?;
                let Overload::Typed { name, result } = typed_overload(op, &accumulated, &right)?
                else {
                    return None;
                };
                accumulated = result.clone();
                Some((Id::from(name), result))
            })
            .collect()
    }

    /// The type an operation of result type `result` computes at, and its
    /// width, when it is an elementary numeric type.
    fn step_of(&self, result: Option<&ExprType>) -> Option<Step> {
        let (ExprType::Concrete(id) | ExprType::Inferred(id)) = result? else {
            return None;
        };
        let name = self.conversions.name_of(*id)?;
        Some((*id, numeric_operation_width(&name)?))
    }

    /// Returns `true` when the operands of `op` have a typed overload on the
    /// time and date types.
    pub(super) fn has_typed_overload(&self, op: &Operator, left: &Expr, right: &Expr) -> bool {
        let (Some(left), Some(right)) = (
            self.conversions.operand_name(left),
            self.conversions.operand_name(right),
        ) else {
            return false;
        };
        matches!(
            typed_overload(op, &left, &right),
            Some(Overload::Typed { .. })
        )
    }

    /// Makes `operand` an operand of the step: an untyped literal takes the
    /// step's type, and a numeric operand of another width is converted to
    /// it. An operand of the same width is operated on as it is.
    fn record_operand(&self, operand: &mut Expr, (target, width): Step) {
        let differs = match operand.expr_type {
            Some(ExprType::Literal(_)) => true,
            Some(ExprType::Concrete(_) | ExprType::Inferred(_)) => self
                .conversions
                .operand_name(operand)
                .as_ref()
                .and_then(numeric_operation_width)
                .is_some_and(|own| own != width),
            Some(ExprType::Null) | None => false,
        };
        if differs {
            self.convert(operand, target);
        }
    }
}

/// The arithmetic operator `func` is the function form of.
pub(super) fn arithmetic_operator(func: &Function) -> Option<Operator> {
    match &operator_function_form(func.name.original())?.operator {
        FormOf::Arithmetic(op) => Some(op.clone()),
        FormOf::Compare(_) | FormOf::Not => None,
    }
}

/// The inputs of `func` when every argument is a positional input; the
/// named-argument pass made every input positional, and any other shape is
/// one a rule reported.
fn positional_inputs(func: &Function) -> Option<Vec<&Expr>> {
    func.param_assignment
        .iter()
        .map(|input| match input {
            ParamAssignmentKind::PositionalInput(input) => Some(&input.expr),
            ParamAssignmentKind::NamedInput(_) | ParamAssignmentKind::Output(_) => None,
        })
        .collect()
}

fn positional(expr: Expr) -> ParamAssignmentKind {
    ParamAssignmentKind::PositionalInput(PositionalInput { expr })
}
