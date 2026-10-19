//! Scalar expressions.

use ironplc_dsl::core::SourceSpan;
use ironplc_dsl::diagnostic::Diagnostic;

use super::call::{program_of, Arg, Callee, Intrinsic, Param};
use super::ids::{PouId, Stamp};
use super::in_place::InPlaceCall;
use super::invariant::broken;
use super::place::Place;
use super::scalar::{ScalarType, Signedness};
use super::stmt::Stmt;
use super::types::{NarrowWidth, TypeTable};
use super::variables::{PouTable, VariableTable};

/// The value of a constant: a `BOOL`, or a value at one of the four operation
/// widths, so a value always fits its type and a backend emits it as it is.
///
/// An integer's signedness is the type of the expression that holds it: an
/// unsigned value is stored as its bit pattern, so a `UDINT` of 4,000,000,000
/// is `I32(-294967296)` in an expression of type `i32u`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Const {
    Bool(bool),
    I32(i32),
    I64(i64),
    F32(f32),
    F64(f64),
}

impl Const {
    /// The integer the constant stands for, read at the integer type `ty`;
    /// `None` for a real constant or a type of another width.
    pub fn integer_value(self, ty: ScalarType) -> Option<i128> {
        match ty {
            ScalarType::I32(signedness) => match self {
                Const::I32(value) => Some(match signedness {
                    Signedness::Signed => value.into(),
                    Signedness::Unsigned => (value as u32).into(),
                }),
                Const::Bool(_) | Const::I64(_) | Const::F32(_) | Const::F64(_) => None,
            },
            ScalarType::I64(signedness) => match self {
                Const::I64(value) => Some(match signedness {
                    Signedness::Signed => value.into(),
                    Signedness::Unsigned => (value as u64).into(),
                }),
                Const::Bool(_) | Const::I32(_) | Const::F32(_) | Const::F64(_) => None,
            },
            ScalarType::Bool
            | ScalarType::F32
            | ScalarType::F64
            | ScalarType::Ref
            | ScalarType::Interface => None,
        }
    }

    /// The integer `value` as a constant of the integer type `ty`; `None` when
    /// `ty` is not an integer type or does not hold the value. An unsigned
    /// value is held as its bit pattern.
    pub fn integer(value: i128, ty: ScalarType) -> Option<Const> {
        match ty {
            ScalarType::I32(Signedness::Signed) => i32::try_from(value).ok().map(Const::I32),
            ScalarType::I32(Signedness::Unsigned) => {
                u32::try_from(value).ok().map(|v| Const::I32(v as i32))
            }
            ScalarType::I64(Signedness::Signed) => i64::try_from(value).ok().map(Const::I64),
            ScalarType::I64(Signedness::Unsigned) => {
                u64::try_from(value).ok().map(|v| Const::I64(v as i64))
            }
            ScalarType::Bool
            | ScalarType::F32
            | ScalarType::F64
            | ScalarType::Ref
            | ScalarType::Interface => None,
        }
    }

    /// Whether the constant is of the width of `ty`.
    pub(crate) fn has_width_of(self, ty: ScalarType) -> bool {
        match ty {
            ScalarType::Bool => matches!(self, Const::Bool(_)),
            ScalarType::I32(_) => matches!(self, Const::I32(_)),
            ScalarType::I64(_) => matches!(self, Const::I64(_)),
            ScalarType::F32 => matches!(self, Const::F32(_)),
            ScalarType::F64 => matches!(self, Const::F64(_)),
            // No constant is a reference or an interface value.
            ScalarType::Ref | ScalarType::Interface => false,
        }
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
    /// The integer operand converted to the integer type of the expression,
    /// as `Convert` converts it, when that type holds the operand's value.
    /// Any other value traps (REQ-LOW-codegen-164).
    ConvertChecked(Box<Expr>),
    /// The low `bits` bits of the operand, sign-extended for a signed type and
    /// zero-extended for an unsigned one.
    Truncate {
        width: NarrowWidth,
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
    /// The result of calling `callee` with `args`, evaluated in order.
    Call {
        callee: Callee,
        args: Vec<Arg>,
    },
    /// The result of a call of a function or method with outputs, run where
    /// the expression is evaluated: the call, then the stores of its
    /// outputs (REQ-LOW-ir-182).
    InPlaceCall(InPlaceCall),
    /// A reference to a place.
    RefTo(Place),
    /// The null reference.
    Null,
}

/// A scalar expression: what it computes, the type it is operated at, and the
/// span of the source it stands for. Every scalar expression has a type
/// (REQ-LOW-lowering-040).
#[derive(Clone, Debug)]
pub struct Expr {
    kind: ExprKind,
    ty: ScalarType,
    span: SourceSpan,
    /// The program the places the expression reads belong to; `None` for an
    /// expression that reads no place.
    program: Option<Stamp>,
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
            ScalarType::I32(_) | ScalarType::I64(_) => Const::integer(value, ty),
            ScalarType::F32 => Some(Const::F32(value as f32)),
            ScalarType::F64 => Some(Const::F64(value as f64)),
            ScalarType::Bool | ScalarType::Ref | ScalarType::Interface => None,
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

    /// The `BOOL` constant `value`.
    pub fn boolean(value: bool, span: SourceSpan) -> Self {
        Self::new(ExprKind::Const(Const::Bool(value)), ScalarType::Bool, span)
    }

    /// The real `value` as a constant of the real type `ty`, rounded to it.
    pub fn real(value: f64, ty: ScalarType, span: SourceSpan) -> Result<Self, Diagnostic> {
        let constant = match ty {
            ScalarType::F32 => Const::F32(value as f32),
            ScalarType::F64 => Const::F64(value),
            ScalarType::Bool
            | ScalarType::I32(_)
            | ScalarType::I64(_)
            | ScalarType::Ref
            | ScalarType::Interface => {
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
    pub(crate) fn read(
        types: &TypeTable,
        place: Place,
        span: SourceSpan,
    ) -> Result<Self, Diagnostic> {
        let ty = types.operation_type(place.ty()).ok_or_else(|| {
            broken(
                "Expr::read",
                &span,
                format!("{} is not operated on as one value", place.ty()),
            )
        })?;
        Ok(Self::new(ExprKind::Read(place), ty, span))
    }

    /// A reference to `place`.
    pub fn ref_to(place: Place, span: SourceSpan) -> Self {
        Self::new(ExprKind::RefTo(place), ScalarType::Ref, span)
    }

    /// The null reference.
    pub fn null(span: SourceSpan) -> Self {
        Self::new(ExprKind::Null, ScalarType::Ref, span)
    }

    /// `value` converted to `ty`, which must differ from its type
    /// (REQ-LOW-lowering-044). Both must be numeric, or `value` a `BOOL` and
    /// `ty` an integer, as in `BOOL_TO_INT`.
    pub fn convert(value: Expr, ty: ScalarType, span: SourceSpan) -> Result<Self, Diagnostic> {
        let converts = (value.ty.is_numeric() && ty.is_numeric())
            || (value.ty == ScalarType::Bool && ty.is_integer());
        if value.ty == ty || !converts {
            return Err(broken(
                "Expr::convert",
                &span,
                format!("cannot convert {} to {ty}", value.ty),
            ));
        }
        Ok(Self::new(ExprKind::Convert(Box::new(value)), ty, span))
    }

    /// The integer `value` converted to the integer type `ty`, trapping on a
    /// value `ty` does not hold (REQ-LOW-lowering-163). `value`'s type must
    /// hold values `ty` does not: a conversion that cannot trap is a
    /// [`Expr::convert`].
    pub fn convert_checked(
        value: Expr,
        ty: ScalarType,
        span: SourceSpan,
    ) -> Result<Self, Diagnostic> {
        let may_not_fit = value
            .ty
            .integer_range()
            .zip(ty.integer_range())
            .is_some_and(|((low, high), (min, max))| low < min || max < high);
        if !may_not_fit {
            return Err(broken(
                "Expr::convert_checked",
                &span,
                format!(
                    "cannot convert {} to {ty} by a checked conversion",
                    value.ty
                ),
            ));
        }
        Ok(Self::new(
            ExprKind::ConvertChecked(Box::new(value)),
            ty,
            span,
        ))
    }

    /// The low bits of the integer `value`, `width` of them, extended back to
    /// its type. `width` must be narrower than the type.
    pub fn truncate(width: NarrowWidth, value: Expr, span: SourceSpan) -> Result<Self, Diagnostic> {
        let narrower = value.ty.is_integer() && value.ty.bits().is_some_and(|w| width.bits() < w);
        if !narrower {
            return Err(broken(
                "Expr::truncate",
                &span,
                format!("cannot truncate {} to {} bits", value.ty, width.bits()),
            ));
        }
        let ty = value.ty;
        Ok(Self::new(
            ExprKind::Truncate {
                width,
                value: Box::new(value),
            },
            ty,
            span,
        ))
    }

    /// `op` applied to `value`, at its type. `Neg` needs a number, `BitNot`
    /// an integer, and `BoolNot` a `BOOL`.
    pub fn unary(op: UnaryOp, value: Expr, span: SourceSpan) -> Result<Self, Diagnostic> {
        let applies = match op {
            UnaryOp::Neg => value.ty.is_numeric(),
            UnaryOp::BitNot => value.ty.is_integer(),
            UnaryOp::BoolNot => value.ty == ScalarType::Bool,
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
        Self::of_one_program("Expr::binary", &lhs, &rhs, &span)?;
        let ty = lhs.ty;
        let applies = match op {
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div => ty.is_numeric(),
            BinaryOp::Mod | BinaryOp::BitAnd | BinaryOp::BitOr | BinaryOp::BitXor => {
                ty.is_integer()
            }
            BinaryOp::BoolAnd | BinaryOp::BoolOr | BinaryOp::BoolXor => ty == ScalarType::Bool,
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
    /// a `BOOL` (REQ-LOW-lowering-043).
    pub fn compare(
        op: CompareOp,
        lhs: Expr,
        rhs: Expr,
        span: SourceSpan,
    ) -> Result<Self, Diagnostic> {
        Self::of_one_program("Expr::compare", &lhs, &rhs, &span)?;
        // References are compared only for equality: an order of addresses
        // means something different on every target.
        let ordered = match op {
            CompareOp::Eq | CompareOp::Ne => false,
            CompareOp::Lt | CompareOp::Le | CompareOp::Gt | CompareOp::Ge => true,
        };
        let comparable = match lhs.ty {
            ScalarType::Interface => false,
            ScalarType::Ref => !ordered,
            ScalarType::Bool
            | ScalarType::I32(_)
            | ScalarType::I64(_)
            | ScalarType::F32
            | ScalarType::F64 => true,
        };
        if lhs.ty != rhs.ty || !comparable {
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
            ScalarType::Bool,
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
        Self::of_one_program("Expr::short_circuit", &lhs, &rhs, &span)?;
        if lhs.ty != ScalarType::Bool || rhs.ty != ScalarType::Bool {
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
            ScalarType::Bool,
            span,
        ))
    }

    /// The value `intrinsic` yields from `args`. There is one argument for
    /// each of its parameters, of what the parameter takes, and one or more
    /// for a last parameter that repeats (REQ-LOW-ir-145). The arguments read
    /// places of one program. An intrinsic that yields a string is a
    /// [`StrExpr`](super::StrExpr) call.
    pub fn call(
        intrinsic: Intrinsic,
        args: Vec<Arg>,
        span: SourceSpan,
    ) -> Result<Self, Diagnostic> {
        const CONSTRUCTOR: &str = "Expr::call";
        let Param::Value(result) = intrinsic.signature().result() else {
            return Err(broken(
                CONSTRUCTOR,
                &span,
                format!("{} yields a string, not a value", intrinsic.name()),
            ));
        };
        intrinsic.check_args(CONSTRUCTOR, &args, &span)?;
        Self::of_call(
            CONSTRUCTOR,
            Callee::Intrinsic(intrinsic),
            args,
            result,
            span,
        )
    }

    /// The result of calling the function or method `pou` that `pous`
    /// declares with `args`, which [`check_user_call`] describes. A method
    /// without a result is only called by a statement.
    pub(crate) fn call_function(
        types: &TypeTable,
        variables: &VariableTable,
        pous: &PouTable,
        pou: PouId,
        args: Vec<Arg>,
        span: SourceSpan,
    ) -> Result<Self, Diagnostic> {
        const CONSTRUCTOR: &str = "ProgramBuilder::call";
        let Some(result) = check_user_call(CONSTRUCTOR, types, variables, pous, pou, &args, &span)?
        else {
            return Err(broken(CONSTRUCTOR, &span, format!("{pou} has no result")));
        };
        Self::of_call(CONSTRUCTOR, Callee::User(pou), args, result, span)
    }

    /// The result of calling the function or method `pou` that `pous`
    /// declares with `args`, which [`check_user_call`] describes, in place:
    /// the call, then `stores`, which [`InPlaceCall`] describes. The callee
    /// has a value: a function that returns its result through a parameter
    /// is called in place by a [`StrExpr`](super::StrExpr).
    pub(crate) fn call_in_place(
        types: &TypeTable,
        variables: &VariableTable,
        pous: &PouTable,
        pou: PouId,
        args: Vec<Arg>,
        stores: Vec<Stmt>,
        span: SourceSpan,
    ) -> Result<Self, Diagnostic> {
        const CONSTRUCTOR: &str = "ProgramBuilder::call_in_place";
        let Some(result) = check_user_call(CONSTRUCTOR, types, variables, pous, pou, &args, &span)?
        else {
            return Err(broken(CONSTRUCTOR, &span, format!("{pou} has no value")));
        };
        let call = InPlaceCall::new(CONSTRUCTOR, pou, args, stores, &span)?;
        Ok(Self::new(ExprKind::InPlaceCall(call), result, span))
    }

    /// The `ty` result of calling `callee` with `args`, whose types the
    /// caller checked, when the arguments read places of one program.
    fn of_call(
        constructor: &str,
        callee: Callee,
        args: Vec<Arg>,
        ty: ScalarType,
        span: SourceSpan,
    ) -> Result<Self, Diagnostic> {
        program_of(constructor, &args, &span)?;
        Ok(Self::new(ExprKind::Call { callee, args }, ty, span))
    }

    fn new(kind: ExprKind, ty: ScalarType, span: SourceSpan) -> Self {
        let program = match &kind {
            ExprKind::Const(_) | ExprKind::Null => None,
            ExprKind::Read(place) | ExprKind::RefTo(place) => Some(place.stamp()),
            ExprKind::Convert(value)
            | ExprKind::ConvertChecked(value)
            | ExprKind::Truncate { value, .. }
            | ExprKind::Unary { value, .. } => value.program,
            ExprKind::Binary { lhs, rhs, .. }
            | ExprKind::Compare { lhs, rhs, .. }
            | ExprKind::ShortCircuit { lhs, rhs, .. } => lhs.program.or(rhs.program),
            ExprKind::Call { args, .. } => args.iter().find_map(Arg::program),
            ExprKind::InPlaceCall(call) => call.program(),
        };
        Self {
            kind,
            ty,
            span,
            program,
        }
    }

    /// The program the places the expression reads belong to.
    pub(crate) fn program(&self) -> Option<Stamp> {
        self.program
    }

    /// Refuses operands that read places of two different programs.
    fn of_one_program(
        constructor: &str,
        lhs: &Expr,
        rhs: &Expr,
        span: &SourceSpan,
    ) -> Result<(), Diagnostic> {
        match (lhs.program, rhs.program) {
            (Some(left), Some(right)) if left != right => Err(broken(
                constructor,
                span,
                "the operands belong to two different programs",
            )),
            _ => Ok(()),
        }
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

/// Refuses `args` unless they are one argument for each parameter of the
/// function or method `pou` that `pous` declares, in order, at the
/// parameter's operation type, or a place of the type a `VAR_IN_OUT`,
/// `VAR_OUTPUT` or method's instance parameter refers to, or of a function
/// block type that extends it (REQ-LOW-lowering-071, REQ-LOW-lowering-072,
/// REQ-LOW-lowering-076, REQ-LOW-ir-173), that read places of the program
/// the tables belong to. Returns the operation
/// type of the result; `None` for a method without one.
pub(crate) fn check_user_call(
    constructor: &str,
    types: &TypeTable,
    variables: &VariableTable,
    pous: &PouTable,
    pou: PouId,
    args: &[Arg],
    span: &SourceSpan,
) -> Result<Option<ScalarType>, Diagnostic> {
    let Some(signature) = pous.function(pou) else {
        return Err(broken(
            constructor,
            span,
            format!("{pou} is not a function or method of this program"),
        ));
    };
    // `None` for a result that is not a scalar.
    let result = match signature.result() {
        Some(result) => variables
            .get(result)
            .and_then(|variable| types.operation_type(variable.ty()))
            .map(Some),
        None => Some(None),
    };
    let (Some(params), Some(result)) = (signature.params(types, variables), result) else {
        return Err(broken(
            constructor,
            span,
            format!("{pou} has a parameter or result that is not a scalar"),
        ));
    };
    let passed: Vec<Param> = args.iter().map(Arg::param).collect();
    let passes = passed.len() == params.len()
        && passed
            .iter()
            .zip(&params)
            .all(|(passed, param)| passed.passes_for(*param, types));
    if !passes {
        return Err(broken(
            constructor,
            span,
            format!("{pou} takes {params:?}, not {passed:?}"),
        ));
    }
    if let Some(other) = args.iter().find_map(Arg::program) {
        if other != pou.stamp() {
            return Err(broken(
                constructor,
                span,
                "the arguments belong to another program",
            ));
        }
    }
    Ok(result)
}
