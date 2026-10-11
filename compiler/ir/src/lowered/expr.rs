//! Scalar expressions.

use ironplc_dsl::core::SourceSpan;
use ironplc_dsl::diagnostic::Diagnostic;

use super::invariant::broken;
use super::place::Place;
use super::scalar::{ScalarType, Signedness};
use super::types::TypeTable;

/// The value of a constant, at one of the four operation widths, so a value
/// always fits its width and a backend emits it as it is.
///
/// An integer's signedness is the type of the expression that holds it: an
/// unsigned value is stored as its bit pattern, so a `UDINT` of 4,000,000,000
/// is `I32(-294967296)` in an expression of type `i32u`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Const {
    I32(i32),
    I64(i64),
    F32(f32),
    F64(f64),
}

impl Const {
    /// The integer the constant stands for, read at the integer type `ty`;
    /// `None` for a real constant or a type of another width.
    pub fn integer_value(self, ty: ScalarType) -> Option<i128> {
        match (self, ty) {
            (Const::I32(value), ScalarType::I32(Signedness::Signed)) => Some(value.into()),
            (Const::I32(value), ScalarType::I32(Signedness::Unsigned)) => {
                Some((value as u32).into())
            }
            (Const::I64(value), ScalarType::I64(Signedness::Signed)) => Some(value.into()),
            (Const::I64(value), ScalarType::I64(Signedness::Unsigned)) => {
                Some((value as u64).into())
            }
            (Const::I32(_) | Const::I64(_) | Const::F32(_) | Const::F64(_), _) => None,
        }
    }

    /// Whether the constant is of the width of `ty`.
    fn has_width_of(self, ty: ScalarType) -> bool {
        matches!(
            (self, ty),
            (Const::I32(_), ScalarType::I32(_))
                | (Const::I64(_), ScalarType::I64(_))
                | (Const::F32(_), ScalarType::F32)
                | (Const::F64(_), ScalarType::F64)
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnaryOp {
    /// Arithmetic negation.
    Neg,
    /// The complement of every bit of an integer or bit string.
    BitNot,
    /// The negation of a `BOOL`.
    BoolNot,
}

/// A binary operation. The logical forms of `AND`, `OR` and `XOR` are
/// distinct from the bitwise forms (REQ-LOW-lowering-048).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    BitAnd,
    BitOr,
    BitXor,
    BoolAnd,
    BoolOr,
    BoolXor,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompareOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

/// A logical operator that evaluates its right operand only when the left
/// does not decide the result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShortCircuitOp {
    AndThen,
    OrElse,
}

/// What a scalar expression computes.
#[derive(Clone, Debug)]
pub enum ExprKind {
    Const(Const),
    /// The value currently at a place.
    Read(Place),
    /// The operand converted to the expression's type.
    Convert(Box<Expr>),
    /// The low `bits` bits of the operand, sign-extended for a signed type and
    /// zero-extended for an unsigned one.
    Truncate {
        bits: u8,
        value: Box<Expr>,
    },
    Unary {
        op: UnaryOp,
        value: Box<Expr>,
    },
    Binary {
        op: BinaryOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
    Compare {
        op: CompareOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
    ShortCircuit {
        op: ShortCircuitOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
}

/// A scalar expression: what it computes, the type it is operated at, and the
/// span of the source it stands for. Every scalar expression has a type
/// (REQ-LOW-lowering-040).
#[derive(Clone, Debug)]
pub struct Expr {
    kind: ExprKind,
    ty: ScalarType,
    span: SourceSpan,
}

impl Expr {
    /// The constant `value` of type `ty`, which is of the width of `ty`, so
    /// that `ty` holds it (REQ-LOW-lowering-041).
    pub fn constant(value: Const, ty: ScalarType, span: SourceSpan) -> Result<Self, Diagnostic> {
        if !value.has_width_of(ty) {
            return Err(broken(
                "Expr::constant",
                &span,
                format!("{value:?} is not of the width of {ty}"),
            ));
        }
        Ok(Self::new(ExprKind::Const(value), ty, span))
    }

    /// The integer `value` as a constant of type `ty`. At an integer type,
    /// `ty` must hold the value. At a real type the constant is the nearest
    /// real, rounded once from the integer.
    pub fn integer(value: i128, ty: ScalarType, span: SourceSpan) -> Result<Self, Diagnostic> {
        let constant = match ty {
            ScalarType::I32(Signedness::Signed) => i32::try_from(value).ok().map(Const::I32),
            ScalarType::I32(Signedness::Unsigned) => {
                u32::try_from(value).ok().map(|v| Const::I32(v as i32))
            }
            ScalarType::I64(Signedness::Signed) => i64::try_from(value).ok().map(Const::I64),
            ScalarType::I64(Signedness::Unsigned) => {
                u64::try_from(value).ok().map(|v| Const::I64(v as i64))
            }
            ScalarType::F32 => Some(Const::F32(value as f32)),
            ScalarType::F64 => Some(Const::F64(value as f64)),
            ScalarType::Ref | ScalarType::Interface => None,
        };
        let constant = constant.ok_or_else(|| {
            broken(
                "Expr::integer",
                &span,
                format!("{ty} does not hold {value}"),
            )
        })?;
        Ok(Self::new(ExprKind::Const(constant), ty, span))
    }

    /// The real `value` as a constant of the real type `ty`, rounded to it.
    pub fn real(value: f64, ty: ScalarType, span: SourceSpan) -> Result<Self, Diagnostic> {
        let constant = match ty {
            ScalarType::F32 => Const::F32(value as f32),
            ScalarType::F64 => Const::F64(value),
            ScalarType::I32(_) | ScalarType::I64(_) | ScalarType::Ref | ScalarType::Interface => {
                return Err(broken(
                    "Expr::real",
                    &span,
                    format!("{ty} is not a real type"),
                ))
            }
        };
        Ok(Self::new(ExprKind::Const(constant), ty, span))
    }

    /// The value at `place`, at the operation type of the place's type
    /// (REQ-LOW-lowering-047).
    pub fn read(types: &TypeTable, place: Place, span: SourceSpan) -> Result<Self, Diagnostic> {
        let ty = types.operation_type(place.ty()).ok_or_else(|| {
            broken(
                "Expr::read",
                &span,
                format!("{} is not operated on as one value", place.ty()),
            )
        })?;
        Ok(Self::new(ExprKind::Read(place), ty, span))
    }

    /// `value` converted to `ty`, which must differ from its type
    /// (REQ-LOW-lowering-044). Both must be numeric.
    pub fn convert(value: Expr, ty: ScalarType, span: SourceSpan) -> Result<Self, Diagnostic> {
        if value.ty == ty || !value.ty.is_numeric() || !ty.is_numeric() {
            return Err(broken(
                "Expr::convert",
                &span,
                format!("cannot convert {} to {ty}", value.ty),
            ));
        }
        Ok(Self::new(ExprKind::Convert(Box::new(value)), ty, span))
    }

    /// The low `bits` bits of the integer `value`, extended back to its type.
    /// `bits` must be narrower than the type.
    pub fn truncate(bits: u8, value: Expr, span: SourceSpan) -> Result<Self, Diagnostic> {
        let narrower =
            value.ty.is_integer() && value.ty.bits().is_some_and(|w| bits > 0 && bits < w);
        if !narrower {
            return Err(broken(
                "Expr::truncate",
                &span,
                format!("cannot truncate {} to {bits} bits", value.ty),
            ));
        }
        let ty = value.ty;
        Ok(Self::new(
            ExprKind::Truncate {
                bits,
                value: Box::new(value),
            },
            ty,
            span,
        ))
    }

    /// `op` applied to `value`, at its type. `Neg` needs a number, `BitNot`
    /// an integer, and `BoolNot` the operation type of a `BOOL`.
    pub fn unary(op: UnaryOp, value: Expr, span: SourceSpan) -> Result<Self, Diagnostic> {
        let applies = match op {
            UnaryOp::Neg => value.ty.is_numeric(),
            UnaryOp::BitNot => value.ty.is_integer(),
            UnaryOp::BoolNot => value.ty == ScalarType::BOOL,
        };
        if !applies {
            return Err(broken(
                "Expr::unary",
                &span,
                format!("{op:?} does not apply to {}", value.ty),
            ));
        }
        let ty = value.ty;
        Ok(Self::new(
            ExprKind::Unary {
                op,
                value: Box::new(value),
            },
            ty,
            span,
        ))
    }

    /// `op` applied to `lhs` and `rhs`, which have the type of the result
    /// (REQ-LOW-lowering-042).
    pub fn binary(
        op: BinaryOp,
        lhs: Expr,
        rhs: Expr,
        span: SourceSpan,
    ) -> Result<Self, Diagnostic> {
        let ty = lhs.ty;
        let applies = match op {
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div => ty.is_numeric(),
            BinaryOp::Mod | BinaryOp::BitAnd | BinaryOp::BitOr | BinaryOp::BitXor => {
                ty.is_integer()
            }
            BinaryOp::BoolAnd | BinaryOp::BoolOr | BinaryOp::BoolXor => ty == ScalarType::BOOL,
        };
        if rhs.ty != ty || !applies {
            return Err(broken(
                "Expr::binary",
                &span,
                format!("{op:?} does not apply to {} and {}", lhs.ty, rhs.ty),
            ));
        }
        Ok(Self::new(
            ExprKind::Binary {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            },
            ty,
            span,
        ))
    }

    /// `lhs` compared with `rhs`, which have the same type; the comparison has
    /// the operation type of a `BOOL` (REQ-LOW-lowering-043).
    pub fn compare(
        op: CompareOp,
        lhs: Expr,
        rhs: Expr,
        span: SourceSpan,
    ) -> Result<Self, Diagnostic> {
        if lhs.ty != rhs.ty || matches!(lhs.ty, ScalarType::Interface) {
            return Err(broken(
                "Expr::compare",
                &span,
                format!("cannot compare {} with {}", lhs.ty, rhs.ty),
            ));
        }
        Ok(Self::new(
            ExprKind::Compare {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            },
            ScalarType::BOOL,
            span,
        ))
    }

    /// `lhs` and then, if it does not decide the result, `rhs`; both have the
    /// operation type of a `BOOL`.
    pub fn short_circuit(
        op: ShortCircuitOp,
        lhs: Expr,
        rhs: Expr,
        span: SourceSpan,
    ) -> Result<Self, Diagnostic> {
        if lhs.ty != ScalarType::BOOL || rhs.ty != ScalarType::BOOL {
            return Err(broken(
                "Expr::short_circuit",
                &span,
                format!("{op:?} does not apply to {} and {}", lhs.ty, rhs.ty),
            ));
        }
        Ok(Self::new(
            ExprKind::ShortCircuit {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            },
            ScalarType::BOOL,
            span,
        ))
    }

    fn new(kind: ExprKind, ty: ScalarType, span: SourceSpan) -> Self {
        Self { kind, ty, span }
    }

    pub fn kind(&self) -> &ExprKind {
        &self.kind
    }

    pub fn ty(&self) -> ScalarType {
        self.ty
    }

    pub fn span(&self) -> &SourceSpan {
        &self.span
    }
}
