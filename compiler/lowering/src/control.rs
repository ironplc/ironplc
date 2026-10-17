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
    is_loop_control, CaseArm, CaseLabel, Const, Expr, ExprKind, ForParts, LoopScope, ScalarType,
    Stmt, UnaryOp,
};

use crate::variables::BodyFrame;
use crate::Lowering;

/// A lowered node, `None` when lowering does not support it, or something
/// inside it, yet.
type Made<T> = Result<Option<T>, Diagnostic>;

impl Lowering<'_> {
    /// Lowers every statement of `stmts`, each to the statements it runs.
    pub(crate) fn statements(
        &mut self,
        frame: &BodyFrame,
        stmts: &[AstStmtKind],
    ) -> Made<Vec<Stmt>> {
        let mut lowered = Vec::with_capacity(stmts.len());
        for stmt in stmts {
            let Some(stmts) = self.statement(frame, stmt)? else {
                return Ok(None);
            };
            lowered.extend(stmts);
        }
        Ok(Some(lowered))
    }

    /// `IF`, with each `ELSIF` an `If` in the `ELSE` of the one before.
    pub(crate) fn if_statement(&mut self, frame: &BodyFrame, stmt: &If) -> Made<Stmt> {
        let Some(mut otherwise) = self.statements(frame, &stmt.else_body)? else {
            return Ok(None);
        };
        for elsif in stmt.else_ifs.iter().rev() {
            let Some(cond) = self.expr(frame, &elsif.expr)? else {
                return Ok(None);
            };
            let Some(then) = self.statements(frame, &elsif.body)? else {
                return Ok(None);
            };
            otherwise = vec![Stmt::if_(cond, then, otherwise, elsif.expr.span())?];
        }
        let Some(cond) = self.expr(frame, &stmt.expr)? else {
            return Ok(None);
        };
        let Some(then) = self.statements(frame, &stmt.body)? else {
            return Ok(None);
        };
        Stmt::if_(cond, then, otherwise, stmt.span()).map(Some)
    }

    /// `CASE`, whose selector the lowered program evaluates once
    /// (REQ-LOW-codegen-124).
    pub(crate) fn case_statement(&mut self, frame: &BodyFrame, stmt: &Case) -> Made<Stmt> {
        let Some(selector) = self.expr(frame, &stmt.selector)? else {
            return Ok(None);
        };
        let mut arms = Vec::with_capacity(stmt.statement_groups.len());
        for group in &stmt.statement_groups {
            let mut labels = Vec::with_capacity(group.selectors.len());
            for selection in &group.selectors {
                let Some(label) = self.case_label(&stmt.selector, selector.ty(), selection)? else {
                    return Ok(None);
                };
                labels.push(label);
            }
            let Some(body) = self.statements(frame, &group.statements)? else {
                return Ok(None);
            };
            arms.push(CaseArm::new(labels, body));
        }
        let Some(otherwise) = self.statements(frame, &stmt.else_body)? else {
            return Ok(None);
        };
        Stmt::case(selector, arms, otherwise, stmt.span()).map(Some)
    }

    /// A label of a `CASE` whose selector is `selector`, of type `ty`
    /// (REQ-LOW-lowering-064).
    fn case_label(
        &mut self,
        selector: &AstExpr,
        ty: ScalarType,
        selection: &CaseSelectionKind,
    ) -> Made<CaseLabel> {
        Ok(Some(match selection {
            CaseSelectionKind::SignedInteger(value) => {
                CaseLabel::Value(label_value(value.is_neg, &value.value, ty)?)
            }
            CaseSelectionKind::BitStringLiteral(literal) => {
                CaseLabel::Value(label_value(false, &literal.value, ty)?)
            }
            CaseSelectionKind::Subrange(range) => {
                let (SignedIntegerRef::Literal(low), SignedIntegerRef::Literal(high)) =
                    (&range.start, &range.end)
                else {
                    // A bound that names a constant is not compiled by either
                    // route yet.
                    return Ok(None);
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
        }))
    }

    /// The ordinal of `value`, a value of the enumeration that `typed`'s type
    /// is, as the analyzer decided it.
    pub(crate) fn enumerated_ordinal(
        &mut self,
        typed: &AstExpr,
        value: &EnumeratedValue,
    ) -> Result<i32, Diagnostic> {
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
        let lowered = self.lowered_type(analyzer_type)?.ok_or_else(not_a_member)?;
        self.program
            .types()
            .enumeration_ordinal(lowered, position)
            .ok_or_else(not_a_member)
    }

    /// `FOR`, whose step is a nonzero constant (REQ-LOW-lowering-067).
    pub(crate) fn for_statement(&mut self, frame: &BodyFrame, stmt: &For) -> Made<Stmt> {
        let span = stmt.span();
        let Some(control) = self.named_place(frame, &stmt.control, &stmt.control.span())? else {
            return Ok(None);
        };
        // What a `VAR_IN_OUT` refers to is not a control variable yet, on
        // either route; what a `VAR_OUTPUT` parameter refers to is
        // (REQ-LOW-lowering-067).
        if !is_loop_control(&control) || self.is_in_out(frame, &stmt.control) {
            return Ok(None);
        }
        let Some(ty) = self.program.types().operation_type(control.ty()) else {
            return Ok(None);
        };
        let Some(mut from) = self.expr(frame, &stmt.from)? else {
            return Ok(None);
        };
        let Some(to) = self.expr(frame, &stmt.to)? else {
            return Ok(None);
        };
        let step = match &stmt.step {
            None => Const::integer(1, ty).ok_or_else(|| {
                Diagnostic::internal_error_at(Label::span(
                    stmt.control.span(),
                    format!("The control variable is a {ty}, not an integer"),
                ))
            })?,
            Some(step) => {
                let Some(step) = self.expr(frame, step)? else {
                    return Ok(None);
                };
                match step_constant(&step) {
                    Some(step) => step,
                    // A step that is not a constant is not compiled by either
                    // route.
                    None => return Ok(None),
                }
            }
        };
        if let Some(width) = self.program.store_narrowing(control.ty(), &from) {
            let span = from.span().clone();
            from = Expr::truncate(width, from, span)?;
        }
        let Some((scope, body)) = self.in_loop(|this| this.statements(frame, &stmt.body))? else {
            return Ok(None);
        };
        let parts = ForParts {
            control,
            from,
            to,
            step,
            body,
        };
        self.program.for_(scope, parts, span).map(Some)
    }

    /// `WHILE c DO b`: a `Loop` whose body leaves when `c` is false, then
    /// runs `b`.
    pub(crate) fn while_statement(&mut self, frame: &BodyFrame, stmt: &While) -> Made<Stmt> {
        let span = stmt.span();
        let Some(cond) = self.expr(frame, &stmt.condition)? else {
            return Ok(None);
        };
        let lowered = self.in_loop(|this| {
            let exit = this.current_loop(&span)?.exit(span.clone());
            let Some(body) = this.statements(frame, &stmt.body)? else {
                return Ok(None);
            };
            let head = Stmt::if_(cond, vec![], vec![exit], span.clone())?;
            Ok(Some(std::iter::once(head).chain(body).collect()))
        })?;
        let Some((scope, body)) = lowered else {
            return Ok(None);
        };
        self.program.loop_(scope, body, vec![], span).map(Some)
    }

    /// `REPEAT b UNTIL c`: a `Loop` with body `b` whose `continuing` leaves
    /// when `c` is true.
    pub(crate) fn repeat_statement(&mut self, frame: &BodyFrame, stmt: &Repeat) -> Made<Stmt> {
        let span = stmt.span();
        let lowered = self.in_loop(|this| {
            let Some(body) = this.statements(frame, &stmt.body)? else {
                return Ok(None);
            };
            let Some(until) = this.expr(frame, &stmt.until)? else {
                return Ok(None);
            };
            let exit = this.current_loop(&span)?.exit(span.clone());
            let tail = Stmt::if_(until, vec![exit], vec![], span.clone())?;
            Ok(Some((body, tail)))
        })?;
        let Some((scope, (body, tail))) = lowered else {
            return Ok(None);
        };
        self.program.loop_(scope, body, vec![tail], span).map(Some)
    }

    /// `EXIT`, leaving the innermost loop. An `EXIT` outside every loop being
    /// lowered is in a loop compiled from the AST, and is not lowered either.
    pub(crate) fn exit_statement(&self, span: &SourceSpan) -> Made<Stmt> {
        Ok(self.loops.last().map(|scope| scope.exit(span.clone())))
    }

    /// `CONTINUE`, going on with the innermost loop. A `CONTINUE` outside
    /// every loop being lowered is in a loop compiled from the AST, and is
    /// not lowered either.
    pub(crate) fn continue_statement(&self, span: &SourceSpan) -> Made<Stmt> {
        Ok(self.loops.last().map(|scope| scope.continue_(span.clone())))
    }

    /// Lowers the body of a new loop with `lower`, returning the loop's scope
    /// with it for the caller to close.
    fn in_loop<T>(
        &mut self,
        lower: impl FnOnce(&mut Self) -> Made<T>,
    ) -> Result<Option<(LoopScope, T)>, Diagnostic> {
        let scope = self.program.open_loop();
        self.loops.push(scope);
        let lowered = lower(self);
        let scope = self.loops.pop().ok_or_else(Diagnostic::internal_error)?;
        Ok(lowered?.map(|lowered| (scope, lowered)))
    }

    /// The innermost loop being lowered.
    fn current_loop(&self, span: &SourceSpan) -> Result<&LoopScope, Diagnostic> {
        self.loops.last().ok_or_else(|| {
            Diagnostic::internal_error_at(Label::span(span.clone(), "No loop is being lowered"))
        })
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
