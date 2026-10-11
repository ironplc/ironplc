//! Scalar expressions.

use ironplc_dsl::core::SourceSpan;
use ironplc_dsl::diagnostic::Diagnostic;

use super::invariant::broken;
use super::place::Place;
use super::scalar::ScalarType;
use super::types::TypeTable;

/// The value of a constant. Its [`ScalarType`] is the type of the expression
/// that holds it, which says how the value is operated.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Const {
    /// A value of an integer operation type.
    Int(i128),
    /// A value of a real operation type.
    Real(f64),
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
    /// The constant `value` of type `ty`, which must hold it
    /// (REQ-LOW-lowering-041).
    pub fn constant(value: Const, ty: ScalarType, span: SourceSpan) -> Result<Self, Diagnostic> {
        let holds = match value {
            Const::Int(int) => ty
                .integer_range()
                .is_some_and(|(min, max)| (min..=max).contains(&int)),
            Const::Real(_) => ty.is_real(),
        };
        if !holds {
            return Err(broken(
                "Expr::constant",
                &span,
                format!("{ty} does not hold {value:?}"),
            ));
        }
        Ok(Self::new(ExprKind::Const(value), ty, span))
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
