//! Scalar expressions.
//!
//! Each expression is lowered at the type the analyzer recorded for it
//! (`Expr::expr_type`), and each conversion the analyzer recorded
//! (`ExprKind::ImplicitConversion`) becomes a `Convert`, or a
//! `ConvertChecked` for a checked one (REQ-LOW-lowering-163). Lowering
//! decides no literal type and no conversion itself (REQ-LOW-lowering-094).

use ironplc_analyzer::NumericFunction;
use ironplc_dsl::common::{Boolean, ConstantKind};
use ironplc_dsl::core::{Located, SourceSpan};
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_dsl::textual::{
    CompareOp as AstCompareOp, ConversionKind, Expr as AstExpr, ExprKind as AstExprKind, Operator,
    UnaryOp as AstUnaryOp, Variable,
};
use ironplc_ir::lowered::{
    BinaryOp, CompareOp, Expr, Place, ShortCircuitOp, TemporalKind, UnaryOp,
};

use crate::bits::selected_bits;
use crate::typed::{Arithmetic, Class, LogicalForm};
use crate::variables::BodyFrame;
use crate::{Lowered, Lowering, Unlowered};

impl Lowering<'_> {
    /// Lowers the scalar expression `expr` of the body of `frame`.
    pub(crate) fn expr(&mut self, frame: &BodyFrame, expr: &AstExpr) -> Lowered<Expr> {
        let span = expr.span();
        match &expr.kind {
            AstExprKind::Expression(inner) => self.expr(frame, inner),
            AstExprKind::Const(constant) => self.constant(expr, constant, span),
            AstExprKind::Variable(Variable::Symbolic(symbolic))
                if selected_bits(symbolic).is_some() =>
            {
                self.bits_read(frame, expr, symbolic)
            }
            AstExprKind::Variable(variable) => {
                let place = self.variable_place(frame, variable)?;
                self.read(place, span)
            }
            AstExprKind::BinaryOp(binary) => {
                let op = match binary.op {
                    Operator::Add => BinaryOp::Add,
                    Operator::Sub => BinaryOp::Sub,
                    Operator::Mul => BinaryOp::Mul,
                    Operator::Div => BinaryOp::Div,
                    Operator::Mod => BinaryOp::Mod,
                    // `**` is the standard function `EXPT`.
                    Operator::Pow => {
                        let args = [&binary.left, &binary.right];
                        return self.numeric_call(frame, expr, NumericFunction::Expt, &args);
                    }
                };
                let (left, right) = (&binary.left, &binary.right);
                match self.typed(expr)?.class.arithmetic()? {
                    Arithmetic::Temporal => self.time_arithmetic(frame, expr, op, left, right),
                    Arithmetic::Numeric => self.binary(frame, op, left, right, span),
                }
            }
            AstExprKind::UnaryOp(unary) => match unary.op {
                AstUnaryOp::Neg => {
                    // A number, or a duration, which is negated as the signed
                    // integer it is stored as (ADR-0025).
                    let negated = matches!(
                        self.typed(expr)?.class,
                        Class::Integer | Class::Real | Class::Temporal(TemporalKind::Time)
                    );
                    if !negated {
                        return Err(Unlowered::NotYet);
                    }
                    let value = self.expr(frame, &unary.term)?;
                    Ok(Expr::unary(UnaryOp::Neg, value, span)?)
                }
                AstUnaryOp::Not => self.not(frame, expr, &unary.term),
            },
            AstExprKind::Compare(compare) => {
                let (left, right) = (&compare.left, &compare.right);
                match compare.op {
                    AstCompareOp::And => self.logical(
                        frame,
                        expr,
                        (BinaryOp::BoolAnd, BinaryOp::BitAnd),
                        left,
                        right,
                    ),
                    AstCompareOp::Or => self.logical(
                        frame,
                        expr,
                        (BinaryOp::BoolOr, BinaryOp::BitOr),
                        left,
                        right,
                    ),
                    AstCompareOp::Xor => self.logical(
                        frame,
                        expr,
                        (BinaryOp::BoolXor, BinaryOp::BitXor),
                        left,
                        right,
                    ),
                    AstCompareOp::AndThen => self.short_circuit(
                        frame,
                        expr,
                        (ShortCircuitOp::AndThen, BinaryOp::BitAnd),
                        left,
                        right,
                    ),
                    AstCompareOp::OrElse => self.short_circuit(
                        frame,
                        expr,
                        (ShortCircuitOp::OrElse, BinaryOp::BitOr),
                        left,
                        right,
                    ),
                    AstCompareOp::Eq => self.compare(frame, CompareOp::Eq, left, right, span),
                    AstCompareOp::Ne => self.compare(frame, CompareOp::Ne, left, right, span),
                    AstCompareOp::Lt => self.compare(frame, CompareOp::Lt, left, right, span),
                    AstCompareOp::Gt => self.compare(frame, CompareOp::Gt, left, right, span),
                    AstCompareOp::LtEq => self.compare(frame, CompareOp::Le, left, right, span),
                    AstCompareOp::GtEq => self.compare(frame, CompareOp::Ge, left, right, span),
                }
            }
            AstExprKind::ImplicitConversion(conversion) => {
                let to = self.typed(expr)?.scalar()?;
                let value = self.expr(frame, &conversion.value)?;
                match conversion.kind {
                    ConversionKind::Ordinary => {
                        if value.ty() == to {
                            // The two types are operated alike, as an `INT`
                            // widened to a `DINT` is: nothing converts.
                            return Ok(value);
                        }
                        if !(value.ty().is_numeric() && to.is_numeric()) {
                            return Err(Unlowered::NotYet);
                        }
                        Ok(Expr::convert(value, to, span)?)
                    }
                    ConversionKind::Checked => {
                        if !(value.ty().is_integer() && to.is_integer()) {
                            return Err(Unlowered::NotYet);
                        }
                        Ok(Expr::convert_checked(value, to, span)?)
                    }
                }
            }
            AstExprKind::EnumeratedValue(value) => {
                let ty = self.typed(expr)?.scalar()?;
                let ordinal = self.enumerated_ordinal(expr, value)?;
                Ok(Expr::integer(ordinal.into(), ty, span)?)
            }
            AstExprKind::Deref(reference) => {
                let place = self.dereferenced_place(frame, reference, &span)?;
                self.read(place, span)
            }
            AstExprKind::Function(func) => self.function_call(frame, expr, func),
            // The analyzer resolves every late-bound name to a variable or an
            // enumerated value (`xform_resolve_late_bound_expr_kind`).
            AstExprKind::LateBound(late_bound) => Err(Diagnostic::internal_error_at(Label::span(
                late_bound.value.span(),
                format!(
                    "{} is a name the analyzer did not resolve",
                    late_bound.value
                ),
            ))
            .into()),
            // `REF(x)`: a reference to the place `x` names, which for a
            // `VAR_IN_OUT` is the place it refers to.
            AstExprKind::Ref(variable) => {
                let place = self.variable_place(frame, variable)?;
                Ok(Expr::ref_to(place, span))
            }
            AstExprKind::Null(_) => Ok(Expr::null(span)),
            // The output of the function block call whose outputs are being
            // stored (`fb_call.rs`).
            AstExprKind::CallOutput(output) => self.call_output(frame, output, span),
            AstExprKind::MethodCall(call) => self.method_call_expr(frame, expr, call),
        }
    }

    /// The place a variable names. A directly represented variable is not
    /// lowered yet.
    pub(crate) fn variable_place(
        &mut self,
        frame: &BodyFrame,
        variable: &Variable,
    ) -> Lowered<Place> {
        match variable {
            Variable::Symbolic(symbolic) => self.symbolic_place(frame, symbolic),
            Variable::Direct(_) => Err(Unlowered::NotYet),
        }
    }

    /// The place `reference^` names. `REF(x)^`, and `ADR(x)^`, which the
    /// analyzer rewrote to `REF(x)^` (`xform_resolve_adr`), is `x` itself,
    /// `p^` what the reference variable `p` refers to, and `pp^^` what the
    /// reference `pp^` refers to: a `Deref` of the place `pp^` names
    /// (REQ-LOW-lowering-202).
    pub(crate) fn dereferenced_place(
        &mut self,
        frame: &BodyFrame,
        reference: &AstExpr,
        span: &SourceSpan,
    ) -> Lowered<Place> {
        let reference = match &reference.kind {
            AstExprKind::Ref(variable) => return self.variable_place(frame, variable),
            AstExprKind::Variable(variable) => self.variable_place(frame, variable)?,
            AstExprKind::Deref(inner) => self.dereferenced_place(frame, inner, span)?,
            AstExprKind::Expression(inner) => {
                return self.dereferenced_place(frame, inner, span);
            }
            // A reference that is not in a variable, such as one a function
            // returns, is not a place to follow.
            AstExprKind::Compare(_)
            | AstExprKind::BinaryOp(_)
            | AstExprKind::UnaryOp(_)
            | AstExprKind::Const(_)
            | AstExprKind::EnumeratedValue(_)
            | AstExprKind::Function(_)
            | AstExprKind::MethodCall(_)
            | AstExprKind::LateBound(_)
            | AstExprKind::Null(_)
            | AstExprKind::ImplicitConversion(_)
            | AstExprKind::CallOutput(_) => return Err(Unlowered::NotYet),
        };
        Ok(self.program.deref(reference, span)?)
    }

    /// The value at `place`, when its type is operated on as one value.
    pub(crate) fn read(&mut self, place: Place, span: SourceSpan) -> Lowered<Expr> {
        if self.program.types().operation_type(place.ty()).is_none() {
            return Err(Unlowered::NotYet);
        }
        Ok(self.program.read(place, span)?)
    }

    fn constant(
        &mut self,
        expr: &AstExpr,
        constant: &ConstantKind,
        span: SourceSpan,
    ) -> Lowered<Expr> {
        let ty = self.typed(expr)?.scalar()?;
        let integer = match constant {
            // An integer literal at a real type is that number, as a real.
            ConstantKind::IntegerLiteral(literal) => i128::try_from(literal.value.clone()).ok(),
            ConstantKind::RealLiteral(literal) => return Ok(Expr::real(literal.value, ty, span)?),
            ConstantKind::Boolean(literal) => {
                let value = match literal.value {
                    Boolean::True => true,
                    Boolean::False => false,
                };
                return Ok(Expr::boolean(value, span));
            }
            ConstantKind::BitStringLiteral(literal) => i128::try_from(literal.value.value).ok(),
            // A time or date is the count it is stored as (ADR-0025): the
            // milliseconds of a duration or a time of day, and the seconds
            // since 1970-01-01 of a date or a date and time.
            ConstantKind::Duration(literal) => Some(literal.interval.whole_milliseconds()),
            ConstantKind::TimeOfDay(literal) => Some(literal.whole_milliseconds().into()),
            ConstantKind::Date(literal) => Some(literal.seconds_since_epoch().into()),
            ConstantKind::DateAndTime(literal) => Some(literal.seconds_since_epoch().into()),
            // A string is a `StrExpr` (`string.rs`), not a scalar.
            ConstantKind::CharacterString(_) => return Err(Unlowered::NotYet),
        };
        let integer = integer.ok_or(Unlowered::NotYet)?;
        Ok(Expr::integer(integer, ty, span)?)
    }

    fn binary(
        &mut self,
        frame: &BodyFrame,
        op: BinaryOp,
        left: &AstExpr,
        right: &AstExpr,
        span: SourceSpan,
    ) -> Lowered<Expr> {
        let lhs = self.expr(frame, left)?;
        let rhs = self.expr(frame, right)?;
        Ok(Expr::binary(op, lhs, rhs, span)?)
    }

    /// `NOT term`, the expression `expr`: the logical negation of a `BOOL`,
    /// and the complement of an integer or bit string.
    pub(crate) fn not(
        &mut self,
        frame: &BodyFrame,
        expr: &AstExpr,
        term: &AstExpr,
    ) -> Lowered<Expr> {
        let span = expr.span();
        let typed = self.typed(expr)?;
        let op = match typed.class.logical_form()? {
            LogicalForm::Logical => UnaryOp::BoolNot,
            LogicalForm::Bitwise => UnaryOp::BitNot,
        };
        let value = self.expr(frame, term)?;
        let value = Expr::unary(op, value, span.clone())?;
        if op != UnaryOp::BitNot {
            return Ok(value);
        }
        // Inverting a `BYTE` or `WORD` sets the bits above its width, so the
        // result is narrowed back to the width at once.
        match self.program.types().narrowed_storage(typed.ty) {
            Some(width) => Ok(Expr::truncate(width, value, span)?),
            None => Ok(value),
        }
    }

    /// `AND`, `OR` or `XOR`: the logical form of `ops` for a `BOOL`, and the
    /// bitwise form for an integer or bit string (REQ-LOW-lowering-048).
    fn logical(
        &mut self,
        frame: &BodyFrame,
        expr: &AstExpr,
        ops: (BinaryOp, BinaryOp),
        left: &AstExpr,
        right: &AstExpr,
    ) -> Lowered<Expr> {
        let op = self.logical_op(expr, ops)?;
        self.binary(frame, op, left, right, expr.span())
    }

    /// The form of `ops` for `expr`: the logical one for a `BOOL`, and the
    /// bitwise one for an integer or bit string.
    pub(crate) fn logical_op(
        &mut self,
        expr: &AstExpr,
        (logical, bitwise): (BinaryOp, BinaryOp),
    ) -> Lowered<BinaryOp> {
        Ok(match self.typed(expr)?.class.logical_form()? {
            LogicalForm::Logical => logical,
            LogicalForm::Bitwise => bitwise,
        })
    }

    /// The comparison `op` of `left` and `right`; of two strings, through
    /// the order of their intrinsic ([`Self::string_compare`]).
    pub(crate) fn compare(
        &mut self,
        frame: &BodyFrame,
        op: CompareOp,
        left: &AstExpr,
        right: &AstExpr,
        span: SourceSpan,
    ) -> Lowered<Expr> {
        // `NULL` has no type of its own: it is compared as the reference it
        // is.
        let class = if matches!(left.kind, AstExprKind::Null(_)) {
            Class::Reference
        } else {
            self.typed(left)?.class
        };
        if let Class::String(encoding) = class {
            return self.string_compare(frame, encoding, op, (left, right), span);
        }
        let lhs = self.expr(frame, left)?;
        let rhs = self.expr(frame, right)?;
        Ok(Expr::compare(op, lhs, rhs, span)?)
    }

    /// `AND_THEN` or `OR_ELSE`: the short circuit of `ops` for a `BOOL`, and
    /// for an integer or bit string the eager bitwise form, since every bit
    /// of both operands decides the result.
    fn short_circuit(
        &mut self,
        frame: &BodyFrame,
        expr: &AstExpr,
        (op, bitwise): (ShortCircuitOp, BinaryOp),
        left: &AstExpr,
        right: &AstExpr,
    ) -> Lowered<Expr> {
        let span = expr.span();
        if let LogicalForm::Bitwise = self.typed(expr)?.class.logical_form()? {
            return self.binary(frame, bitwise, left, right, span);
        }
        let lhs = self.expr(frame, left)?;
        let rhs = self.expr(frame, right)?;
        Ok(Expr::short_circuit(op, lhs, rhs, span)?)
    }
}
