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
    BinaryOp, CompareOp, Expr, Place, ScalarType, ShortCircuitOp, TypeId, TypeKind, UnaryOp,
};

use crate::bits::selected_bits;
use crate::variables::BodyFrame;
use crate::Lowering;

/// A lowered expression, or `None` when lowering does not support it yet.
pub(crate) type Lowered = Result<Option<Expr>, Diagnostic>;

impl Lowering<'_> {
    /// Lowers the scalar expression `expr` of the body of `frame`.
    pub(crate) fn expr(&mut self, frame: &BodyFrame, expr: &AstExpr) -> Lowered {
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
                let Some(place) = self.variable_place(frame, variable)? else {
                    return Ok(None);
                };
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
                if self.is_temporal(expr)? {
                    return self.time_arithmetic(frame, expr, op, &binary.left, &binary.right);
                }
                if !self.is_arithmetic(expr)? {
                    return Ok(None);
                }
                self.binary(frame, op, &binary.left, &binary.right, span)
            }
            AstExprKind::UnaryOp(unary) => match unary.op {
                AstUnaryOp::Neg => {
                    // A duration is negated as the signed integer it is
                    // stored as (ADR-0025).
                    if !(self.is_arithmetic(expr)? || self.is_duration(expr)?) {
                        return Ok(None);
                    }
                    let Some(value) = self.expr(frame, &unary.term)? else {
                        return Ok(None);
                    };
                    Expr::unary(UnaryOp::Neg, value, span).map(Some)
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
                let Some((_, to)) = self.operated_type(expr)? else {
                    return Ok(None);
                };
                let Some(value) = self.expr(frame, &conversion.value)? else {
                    return Ok(None);
                };
                match conversion.kind {
                    ConversionKind::Ordinary => {
                        if value.ty() == to {
                            // The two types are operated alike, as an `INT`
                            // widened to a `DINT` is: nothing converts.
                            return Ok(Some(value));
                        }
                        if !(value.ty().is_numeric() && to.is_numeric()) {
                            return Ok(None);
                        }
                        Expr::convert(value, to, span).map(Some)
                    }
                    ConversionKind::Checked => {
                        if !(value.ty().is_integer() && to.is_integer()) {
                            return Ok(None);
                        }
                        Expr::convert_checked(value, to, span).map(Some)
                    }
                }
            }
            AstExprKind::EnumeratedValue(value) => {
                let Some((_, ty)) = self.operated_type(expr)? else {
                    return Ok(None);
                };
                let ordinal = self.enumerated_ordinal(expr, value)?;
                Expr::integer(ordinal.into(), ty, span).map(Some)
            }
            AstExprKind::Deref(reference) => {
                // A reference that is not in a variable, such as one a
                // function returns, is not a place to follow.
                let AstExprKind::Variable(variable) = &reference.kind else {
                    return Ok(None);
                };
                let Some(reference) = self.variable_place(frame, variable)? else {
                    return Ok(None);
                };
                let place = self.program.deref(reference, &span)?;
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
            ))),
            // `REF(x)`: a reference to the place `x` names, which for a
            // `VAR_IN_OUT` is the place it refers to.
            AstExprKind::Ref(variable) => {
                let Some(place) = self.variable_place(frame, variable)? else {
                    return Ok(None);
                };
                Ok(Some(Expr::ref_to(place, span)))
            }
            AstExprKind::Null(_) => Ok(Some(Expr::null(span))),
            // The output of the function block call whose outputs are being
            // stored (`fb_call.rs`).
            AstExprKind::CallOutput(output) => self.call_output(output, span),
            AstExprKind::MethodCall(call) => self.method_call_expr(frame, expr, call),
        }
    }

    /// The place a variable names, or `None` when lowering does not reach it
    /// yet.
    pub(crate) fn variable_place(
        &mut self,
        frame: &BodyFrame,
        variable: &Variable,
    ) -> Result<Option<Place>, Diagnostic> {
        match variable {
            Variable::Symbolic(symbolic) => self.symbolic_place(frame, symbolic),
            Variable::Direct(_) => Ok(None),
        }
    }

    /// The value at `place`, when its type is operated on as one value.
    pub(crate) fn read(&mut self, place: Place, span: SourceSpan) -> Lowered {
        if self.program.types().operation_type(place.ty()).is_none() {
            return Ok(None);
        }
        self.program.read(place, span).map(Some)
    }

    fn constant(&mut self, expr: &AstExpr, constant: &ConstantKind, span: SourceSpan) -> Lowered {
        let Some((_, ty)) = self.operated_type(expr)? else {
            return Ok(None);
        };
        let integer = match constant {
            // An integer literal at a real type is that number, as a real.
            ConstantKind::IntegerLiteral(literal) => i128::try_from(literal.value.clone()).ok(),
            ConstantKind::RealLiteral(literal) => {
                return Expr::real(literal.value, ty, span).map(Some)
            }
            ConstantKind::Boolean(literal) => {
                let value = match literal.value {
                    Boolean::True => true,
                    Boolean::False => false,
                };
                return Ok(Some(Expr::boolean(value, span)));
            }
            ConstantKind::BitStringLiteral(literal) => i128::try_from(literal.value.value).ok(),
            // A time or date is the count it is stored as (ADR-0025): the
            // milliseconds of a duration or a time of day, and the seconds
            // since 1970-01-01 of a date or a date and time.
            ConstantKind::Duration(literal) => Some(literal.interval.whole_milliseconds()),
            ConstantKind::TimeOfDay(literal) => Some(literal.whole_milliseconds().into()),
            ConstantKind::Date(literal) => Some(literal.seconds_since_epoch().into()),
            ConstantKind::DateAndTime(literal) => Some(literal.seconds_since_epoch().into()),
            ConstantKind::CharacterString(_) => return Ok(None),
        };
        let Some(integer) = integer else {
            return Ok(None);
        };
        Expr::integer(integer, ty, span).map(Some)
    }

    fn binary(
        &mut self,
        frame: &BodyFrame,
        op: BinaryOp,
        left: &AstExpr,
        right: &AstExpr,
        span: SourceSpan,
    ) -> Lowered {
        let Some(lhs) = self.expr(frame, left)? else {
            return Ok(None);
        };
        let Some(rhs) = self.expr(frame, right)? else {
            return Ok(None);
        };
        Expr::binary(op, lhs, rhs, span).map(Some)
    }

    /// `NOT term`, the expression `expr`: the logical negation of a `BOOL`,
    /// and the complement of an integer or bit string.
    pub(crate) fn not(&mut self, frame: &BodyFrame, expr: &AstExpr, term: &AstExpr) -> Lowered {
        let span = expr.span();
        let op = match self.is_bool(expr)? {
            Some(true) => UnaryOp::BoolNot,
            Some(false) => UnaryOp::BitNot,
            None => return Ok(None),
        };
        let Some(value) = self.expr(frame, term)? else {
            return Ok(None);
        };
        let value = Expr::unary(op, value, span.clone())?;
        if op != UnaryOp::BitNot {
            return Ok(Some(value));
        }
        // Inverting a `BYTE` or `WORD` sets the bits above its width, so the
        // result is narrowed back to the width at once.
        let Some((ty, _)) = self.operated_type(expr)? else {
            return Ok(None);
        };
        match self.program.types().narrowed_storage(ty) {
            Some(width) => Expr::truncate(width, value, span).map(Some),
            None => Ok(Some(value)),
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
    ) -> Lowered {
        let Some(op) = self.logical_op(expr, ops)? else {
            return Ok(None);
        };
        self.binary(frame, op, left, right, expr.span())
    }

    /// The form of `ops` for `expr`: the logical one for a `BOOL`, the bitwise
    /// one for an integer or bit string, and `None` for any other type.
    pub(crate) fn logical_op(
        &mut self,
        expr: &AstExpr,
        (logical, bitwise): (BinaryOp, BinaryOp),
    ) -> Result<Option<BinaryOp>, Diagnostic> {
        Ok(self
            .is_bool(expr)?
            .map(|is_bool| if is_bool { logical } else { bitwise }))
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
    ) -> Lowered {
        if let Some(encoding) = self.string_encoding(left)? {
            return self.string_compare(frame, encoding, op, (left, right), span);
        }
        let Some(lhs) = self.expr(frame, left)? else {
            return Ok(None);
        };
        let Some(rhs) = self.expr(frame, right)? else {
            return Ok(None);
        };
        Expr::compare(op, lhs, rhs, span).map(Some)
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
    ) -> Lowered {
        let span = expr.span();
        match self.is_bool(expr)? {
            Some(true) => {}
            Some(false) => return self.binary(frame, bitwise, left, right, span),
            None => return Ok(None),
        }
        let Some(lhs) = self.expr(frame, left)? else {
            return Ok(None);
        };
        let Some(rhs) = self.expr(frame, right)? else {
            return Ok(None);
        };
        Expr::short_circuit(op, lhs, rhs, span).map(Some)
    }

    /// The lowered type the analyzer recorded for `expr`; `None` when it
    /// recorded none, or one lowering cannot express yet.
    pub(crate) fn recorded_type(&mut self, expr: &AstExpr) -> Result<Option<TypeId>, Diagnostic> {
        let Some(recorded) = expr.expr_type.as_ref().and_then(|ty| ty.type_id()) else {
            return Ok(None);
        };
        self.lowered_type(recorded)
    }

    /// The lowered type the analyzer recorded for `expr`, and the type it is
    /// operated at; `None` when it recorded none, or one lowering cannot
    /// operate on as one value.
    pub(crate) fn operated_type(
        &mut self,
        expr: &AstExpr,
    ) -> Result<Option<(TypeId, ScalarType)>, Diagnostic> {
        let Some(ty) = self.recorded_type(expr)? else {
            return Ok(None);
        };
        Ok(self
            .program
            .types()
            .operation_type(ty)
            .map(|operated| (ty, operated)))
    }

    /// Whether `expr` is computed by arithmetic: a number or a bit string,
    /// not a time, a date or an enumeration, whose operations follow rules of
    /// their own.
    pub(crate) fn is_arithmetic(&mut self, expr: &AstExpr) -> Result<bool, Diagnostic> {
        let Some((ty, _)) = self.operated_type(expr)? else {
            return Ok(false);
        };
        let Some(kind) = self.program.types().kind(ty) else {
            return Ok(false);
        };
        Ok(match kind {
            TypeKind::Integer { .. } | TypeKind::BitString { .. } | TypeKind::Real { .. } => true,
            TypeKind::Subrange { base, .. } => {
                matches!(
                    self.program.types().kind(*base),
                    Some(TypeKind::Integer { .. })
                )
            }
            TypeKind::Bool
            | TypeKind::Temporal { .. }
            | TypeKind::Enumeration { .. }
            | TypeKind::String(_)
            | TypeKind::Array { .. }
            | TypeKind::Structure { .. }
            | TypeKind::FunctionBlock { .. }
            | TypeKind::Program { .. }
            | TypeKind::Reference { .. } => false,
        })
    }

    /// Whether `expr` is a `BOOL` (`Some(true)`) or an integer or bit string
    /// (`Some(false)`), which decides between the logical and bitwise form of
    /// `NOT`, `AND`, `OR` and `XOR` (REQ-LOW-lowering-048). `None` for any
    /// other type.
    fn is_bool(&mut self, expr: &AstExpr) -> Result<Option<bool>, Diagnostic> {
        let Some((ty, _)) = self.operated_type(expr)? else {
            return Ok(None);
        };
        let Some(kind) = self.program.types().kind(ty) else {
            return Ok(None);
        };
        Ok(match kind {
            TypeKind::Bool => Some(true),
            TypeKind::Integer { .. } | TypeKind::BitString { .. } => Some(false),
            TypeKind::Real { .. }
            | TypeKind::Temporal { .. }
            | TypeKind::Enumeration { .. }
            | TypeKind::Subrange { .. }
            | TypeKind::String(_)
            | TypeKind::Array { .. }
            | TypeKind::Structure { .. }
            | TypeKind::FunctionBlock { .. }
            | TypeKind::Program { .. }
            | TypeKind::Reference { .. } => None,
        })
    }
}
