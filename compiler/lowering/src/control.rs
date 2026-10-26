//! Control flow: `IF`, `CASE`, the loops, `EXIT`, `CONTINUE` and `RETURN`.
//!
//! `ELSIF` becomes an `If` nested in the `ELSE` of the one before, and `WHILE`
//! and `REPEAT` become a `Loop` that leaves when its condition says to
//! (`specs/design/lowered-program.md`, "Desugaring"). `FOR` stays a `For`.

use ironplc_dsl::common::{EnumeratedValue, Integer, SignedIntegerRef};
use ironplc_dsl::core::{Located, SourceSpan};
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_dsl::textual::{
    Case, CaseSelectionKind, Expr as AstExpr, For, If, Repeat, StmtKind as AstStmtKind, While,
};
use ironplc_ir::lowered::{
    is_loop_control, CaseArm, CaseLabel, Const, Expr, ExprKind, ForParts, ScalarType, Stmt, UnaryOp,
};

use crate::variables::BodyFrame;
use crate::{Lowered, Lowering, Unlowered};

impl Lowering<'_> {
    /// Lowers every statement of `stmts`, each to the statements it runs.
    pub(crate) fn statements(
        &mut self,
        frame: &BodyFrame,
        stmts: &[AstStmtKind],
    ) -> Lowered<Vec<Stmt>> {
        let mut lowered = Vec::with_capacity(stmts.len());
        for stmt in stmts {
            lowered.extend(self.statement(frame, stmt)?);
        }
        Ok(lowered)
    }

    /// `IF`, with each `ELSIF` an `If` in the `ELSE` of the one before.
    pub(crate) fn if_statement(&mut self, frame: &BodyFrame, stmt: &If) -> Lowered<Stmt> {
        let mut otherwise = self.statements(frame, &stmt.else_body)?;
        for elsif in stmt.else_ifs.iter().rev() {
            let cond = self.expr(frame, &elsif.expr)?;
            let then = self.statements(frame, &elsif.body)?;
            otherwise = vec![Stmt::if_(cond, then, otherwise, elsif.expr.span())?];
        }
        let cond = self.expr(frame, &stmt.expr)?;
        let then = self.statements(frame, &stmt.body)?;
        Ok(Stmt::if_(cond, then, otherwise, stmt.span())?)
    }

    /// `CASE`, whose selector the lowered program evaluates once
    /// (REQ-LOW-codegen-124).
    pub(crate) fn case_statement(&mut self, frame: &BodyFrame, stmt: &Case) -> Lowered<Stmt> {
        let selector = self.expr(frame, &stmt.selector)?;
        let mut arms = Vec::with_capacity(stmt.statement_groups.len());
        for group in &stmt.statement_groups {
            let mut labels = Vec::with_capacity(group.selectors.len());
            for selection in &group.selectors {
                labels.push(self.case_label(&stmt.selector, selector.ty(), selection)?);
            }
            let body = self.statements(frame, &group.statements)?;
            arms.push(CaseArm::new(labels, body));
        }
        let otherwise = self.statements(frame, &stmt.else_body)?;
        Ok(Stmt::case(selector, arms, otherwise, stmt.span())?)
    }

    /// A label of a `CASE` whose selector is `selector`, of type `ty`
    /// (REQ-LOW-lowering-064).
    fn case_label(
        &mut self,
        selector: &AstExpr,
        ty: ScalarType,
        selection: &CaseSelectionKind,
    ) -> Lowered<CaseLabel> {
        Ok(match selection {
            CaseSelectionKind::SignedInteger(value) => {
                CaseLabel::Value(label_value(value.is_neg, &value.value, ty)?)
            }
            CaseSelectionKind::BitStringLiteral(literal) => {
                CaseLabel::Value(label_value(false, &literal.value, ty)?)
            }
            CaseSelectionKind::Subrange(range) => {
                // A bound that names a constant is not lowered yet.
                let (SignedIntegerRef::Literal(low), SignedIntegerRef::Literal(high)) =
                    (&range.start, &range.end)
                else {
                    return Err(Unlowered::NotYet);
                };
                CaseLabel::Range(
                    label_value(low.is_neg, &low.value, ty)?,
                    label_value(high.is_neg, &high.value, ty)?,
                )
            }
            CaseSelectionKind::EnumeratedValue(value) => {
                let ordinal = self.enumerated_ordinal(selector, value)?;
                CaseLabel::Value(
                    Const::integer(ordinal.into(), ty)
                        .ok_or_else(|| not_of_selector(&value.value.span(), ordinal.into(), ty))?,
                )
            }
        })
    }

    /// The ordinal of `value`, a value of the enumeration that `typed`'s type
    /// is, as the analyzer decided it.
    pub(crate) fn enumerated_ordinal(
        &mut self,
        typed: &AstExpr,
        value: &EnumeratedValue,
    ) -> Lowered<i32> {
        let not_a_member = || {
            Diagnostic::internal_error_at(Label::span(
                value.span(),
                format!("{} is not a value of its enumeration", value.value),
            ))
        };
        let analyzer_type = typed
            .expr_type
            .as_ref()
            .and_then(|ty| ty.type_id())
            .ok_or_else(not_a_member)?;
        let position = self
            .analysis
            .context()
            .types()
            .get_by_id(analyzer_type)
            .and_then(|attributes| attributes.representation.enumeration_members())
            .and_then(|members| members.iter().position(|member| member.name == value.value))
            .ok_or_else(not_a_member)?;
        let lowered = self.lowered_type(analyzer_type)?;
        Ok(self
            .program
            .types()
            .enumeration_ordinal(lowered, position)
            .ok_or_else(not_a_member)?)
    }

    /// `FOR`, whose step is a nonzero constant (REQ-LOW-lowering-067).
    pub(crate) fn for_statement(&mut self, frame: &BodyFrame, stmt: &For) -> Lowered<Stmt> {
        let span = stmt.span();
        let control = self.named_place(frame, &stmt.control, &stmt.control.span())?;
        // What a `VAR_IN_OUT` refers to is not a control variable yet; what
        // a `VAR_OUTPUT` parameter refers to is (REQ-LOW-lowering-067).
        if !is_loop_control(&control) || self.is_in_out(frame, &stmt.control) {
            return Err(Unlowered::NotYet);
        }
        let ty = self
            .program
            .types()
            .operation_type(control.ty())
            .ok_or(Unlowered::NotYet)?;
        let mut from = self.expr(frame, &stmt.from)?;
        let to = self.expr(frame, &stmt.to)?;
        let step = match &stmt.step {
            None => Const::integer(1, ty).ok_or_else(|| {
                Diagnostic::internal_error_at(Label::span(
                    stmt.control.span(),
                    format!("The control variable is a {ty}, not an integer"),
                ))
            })?,
            Some(step) => {
                let step = self.expr(frame, step)?;
                // A step that is not a constant is not lowered yet.
                step_constant(&step).ok_or(Unlowered::NotYet)?
            }
        };
        if let Some(width) = self.program.store_narrowing(control.ty(), &from) {
            let span = from.span().clone();
            from = Expr::truncate(width, from, span)?;
        }
        let scope = self.program.open_loop();
        let body = self.statements(&frame.in_loop(&scope), &stmt.body)?;
        let parts = ForParts {
            control,
            from,
            to,
            step,
            body,
        };
        Ok(self.program.for_(scope, parts, span)?)
    }

    /// `WHILE c DO b`: a `Loop` whose body leaves when `c` is false, then
    /// runs `b`.
    pub(crate) fn while_statement(&mut self, frame: &BodyFrame, stmt: &While) -> Lowered<Stmt> {
        let span = stmt.span();
        let cond = self.expr(frame, &stmt.condition)?;
        let scope = self.program.open_loop();
        let exit = scope.exit(span.clone());
        let body = self.statements(&frame.in_loop(&scope), &stmt.body)?;
        let head = Stmt::if_(cond, vec![], vec![exit], span.clone())?;
        let body = std::iter::once(head).chain(body).collect();
        Ok(self.program.loop_(scope, body, vec![], span)?)
    }

    /// `REPEAT b UNTIL c`: a `Loop` with body `b` whose `continuing` leaves
    /// when `c` is true.
    pub(crate) fn repeat_statement(&mut self, frame: &BodyFrame, stmt: &Repeat) -> Lowered<Stmt> {
        let span = stmt.span();
        let scope = self.program.open_loop();
        let body = self.statements(&frame.in_loop(&scope), &stmt.body)?;
        let until = self.expr(frame, &stmt.until)?;
        let exit = scope.exit(span.clone());
        let tail = Stmt::if_(until, vec![exit], vec![], span.clone())?;
        Ok(self.program.loop_(scope, body, vec![tail], span)?)
    }

    /// `EXIT`, leaving the innermost loop. One outside every loop being
    /// lowered is not lowered.
    pub(crate) fn exit_statement(&self, frame: &BodyFrame, span: &SourceSpan) -> Lowered<Stmt> {
        let scope = frame.innermost_loop().ok_or(Unlowered::NotYet)?;
        Ok(scope.exit(span.clone()))
    }

    /// `CONTINUE`, going on with the innermost loop. One outside every loop
    /// being lowered is not lowered.
    pub(crate) fn continue_statement(&self, frame: &BodyFrame, span: &SourceSpan) -> Lowered<Stmt> {
        let scope = frame.innermost_loop().ok_or(Unlowered::NotYet)?;
        Ok(scope.continue_(span.clone()))
    }
}

/// A `CASE` label written as `magnitude`, negated when `is_neg`, as a
/// constant of the selector's type `ty`. Analysis reports a label outside
/// the selector's type (P2026), so one that does not fit is a defect.
fn label_value(is_neg: bool, magnitude: &Integer, ty: ScalarType) -> Result<Const, Diagnostic> {
    let value = i128::try_from(magnitude.value)
        .ok()
        .map(|value| if is_neg { -value } else { value });
    value
        .and_then(|value| Const::integer(value, ty))
        .ok_or_else(|| not_of_selector(&magnitude.span, value.unwrap_or(i128::MAX), ty))
}

fn not_of_selector(span: &SourceSpan, value: i128, ty: ScalarType) -> Diagnostic {
    Diagnostic::internal_error_at(Label::span(
        span.clone(),
        format!("The CASE label {value} is not a {ty}"),
    ))
}

/// The constant a `FOR` step is: a literal, a negated literal, or a literal
/// the analyzer converted to the control variable's type. `None` for any
/// other expression.
fn step_constant(step: &Expr) -> Option<Const> {
    Const::integer(integer_constant(step)?, step.ty())
}

/// The integer a literal, a negated literal or a converted literal states.
fn integer_constant(expr: &Expr) -> Option<i128> {
    match expr.kind() {
        ExprKind::Const(constant) => constant.integer_value(expr.ty()),
        ExprKind::Unary {
            op: UnaryOp::Neg,
            value,
        } => integer_constant(value).map(|value| -value),
        ExprKind::Convert(value) => integer_constant(value),
        ExprKind::Read(_)
        | ExprKind::ConvertChecked(_)
        | ExprKind::Truncate { .. }
        | ExprKind::Unary { .. }
        | ExprKind::Binary { .. }
        | ExprKind::Compare { .. }
        | ExprKind::ShortCircuit { .. }
        | ExprKind::Call { .. }
        | ExprKind::InPlaceCall(_)
        | ExprKind::RefTo(_)
        | ExprKind::Null => None,
    }
}
