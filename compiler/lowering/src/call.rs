//! Calls of standard functions. A function form of an operator, and `MOVE`,
//! lower to what the operator, or the operand, lowers to; any other function
//! is the `Intrinsic` for its function at its operand types
//! (REQ-LOW-lowering-147). The match over the analyzer's `BuiltinFunction`
//! names every function, so one the analyzer adds is not lowered until it has
//! an arm here.

use ironplc_analyzer::{BitShift, BuiltinFunction, FormOf, NumericFunction};
use ironplc_dsl::core::{Located, SourceSpan};
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_dsl::textual::{CompareOp, Expr as AstExpr, Function, Operator};
use ironplc_ir::lowered::{
    Arg, BinaryOp, Callee, CompareOp as LoweredCompareOp, Expr, Intrinsic, ScalarType, Signedness,
};

use crate::expr::Lowered;
use crate::variables::BodyFrame;
use crate::Lowering;

impl Lowering<'_> {
    /// The call `func`, the expression `expr`.
    pub(crate) fn function_call(
        &mut self,
        frame: &BodyFrame,
        expr: &AstExpr,
        func: &Function,
    ) -> Lowered {
        // A call of a user-defined function is not lowered yet.
        let Some(builtin) = self
            .analysis
            .context()
            .functions()
            .get(&func.name)
            .and_then(|signature| signature.builtin_function.clone())
        else {
            return Ok(None);
        };
        // The analyzer wrote the call in normal form: its inputs in the order
        // of the function's parameters.
        let args: Vec<&AstExpr> = func.inputs().collect();
        match builtin {
            BuiltinFunction::Operator(form) => self.operator_form(frame, expr, &form, &args),
            // `MOVE(x)` is `x`.
            BuiltinFunction::Move => match args.as_slice() {
                [value] => {
                    let value = self.expr(frame, value)?;
                    self.of_own_type(expr, value)
                }
                _ => Ok(None),
            },
            BuiltinFunction::BitShift(shift) => {
                // A shift computes at the type of the value it shifts.
                let Some((ty, _)) = self.operated_type(expr)? else {
                    return Ok(None);
                };
                let types = self.program.types();
                let Some(intrinsic) = types
                    .operation_type(ty)
                    .zip(types.bit_width(ty))
                    .and_then(|(operated, width)| shift_intrinsic(shift, operated, width))
                else {
                    return Ok(None);
                };
                self.intrinsic_call(frame, expr, intrinsic, &args)
            }
            BuiltinFunction::Numeric(function) => self.numeric_call(frame, expr, function, &args),
            BuiltinFunction::Mux
            | BuiltinFunction::Trunc
            | BuiltinFunction::BcdToInt
            | BuiltinFunction::IntToBcd
            | BuiltinFunction::Sizeof
            | BuiltinFunction::String(_)
            | BuiltinFunction::Conversion { .. }
            | BuiltinFunction::Time { .. }
            | BuiltinFunction::DtToDate
            | BuiltinFunction::DtToTod => Ok(None),
        }
    }

    /// The call `expr` of the numeric function `function` with `args`. The
    /// operator `a ** b` is a call of `EXPT`.
    pub(crate) fn numeric_call(
        &mut self,
        frame: &BodyFrame,
        expr: &AstExpr,
        function: NumericFunction,
        args: &[&AstExpr],
    ) -> Lowered {
        // A numeric function computes at the type the analyzer converted its
        // inputs to.
        let Some(intrinsic) = self
            .operated_type(expr)?
            .and_then(|(_, operated)| numeric_intrinsic(function, operated))
        else {
            return Ok(None);
        };
        self.intrinsic_call(frame, expr, intrinsic, args)
    }

    /// The call `expr` of `intrinsic` with `args`.
    fn intrinsic_call(
        &mut self,
        frame: &BodyFrame,
        expr: &AstExpr,
        intrinsic: Intrinsic,
        args: &[&AstExpr],
    ) -> Lowered {
        let Some(args) = self.exprs(frame, args)? else {
            return Ok(None);
        };
        // An argument of another type than its parameter's is one the
        // analyzer did not convert to it, and is not lowered.
        let (params, _) = intrinsic.signature();
        if !args.iter().map(Expr::ty).eq(params.iter().copied()) {
            return Ok(None);
        }
        Expr::call(
            Callee::Intrinsic(intrinsic),
            args.into_iter().map(Arg::Value).collect(),
            expr.span(),
        )
        .map(Some)
    }

    /// The function form of an operator, `expr`: the expression the
    /// operator itself lowers to (REQ-LOW-lowering-101). `ADD`, `MUL`, `AND`,
    /// `OR` and `XOR` take two or more inputs and fold from the left.
    fn operator_form(
        &mut self,
        frame: &BodyFrame,
        expr: &AstExpr,
        form: &FormOf,
        args: &[&AstExpr],
    ) -> Lowered {
        let span = expr.span();
        let folded = match form {
            FormOf::Arithmetic(op) => {
                // A typed overload on times and dates (`ADD_TIME`) follows
                // rules of its own.
                if !self.is_arithmetic(expr)? {
                    return Ok(None);
                }
                let Some(op) = arithmetic_op(op) else {
                    return Ok(None);
                };
                self.fold(frame, op, args, span)?
            }
            FormOf::Compare(op) => match logical_ops(op) {
                Some(ops) => {
                    let Some(op) = self.logical_op(expr, ops)? else {
                        return Ok(None);
                    };
                    self.fold(frame, op, args, span)?
                }
                None => match (compare_op(op), args) {
                    (Some(op), [left, right]) => self.compare(frame, op, left, right, span)?,
                    _ => None,
                },
            },
            FormOf::Not => match args {
                [term] => self.not(frame, expr, term)?,
                _ => None,
            },
        };
        self.of_own_type(expr, folded)
    }

    /// `args`, two or more, folded from the left with `op`.
    fn fold(
        &mut self,
        frame: &BodyFrame,
        op: BinaryOp,
        args: &[&AstExpr],
        span: SourceSpan,
    ) -> Lowered {
        let Some(values) = self.exprs(frame, args)? else {
            return Ok(None);
        };
        let mut values = values.into_iter();
        let (Some(mut folded), Some(_)) = (values.next(), args.get(1)) else {
            return Ok(None);
        };
        for value in values {
            // The analyzer converted every input to the type the call
            // computes at; one it did not convert is not lowered yet.
            if value.ty() != folded.ty() {
                return Ok(None);
            }
            folded = Expr::binary(op, folded, value, span.clone())?;
        }
        Ok(Some(folded))
    }

    /// Each of `args` lowered, or `None` when one of them is not supported.
    fn exprs(
        &mut self,
        frame: &BodyFrame,
        args: &[&AstExpr],
    ) -> Result<Option<Vec<Expr>>, Diagnostic> {
        let mut values = Vec::with_capacity(args.len());
        for arg in args {
            let Some(value) = self.expr(frame, arg)? else {
                return Ok(None);
            };
            values.push(value);
        }
        Ok(Some(values))
    }

    /// `value`, when it has the type the analyzer recorded for `expr`.
    fn of_own_type(&mut self, expr: &AstExpr, value: Option<Expr>) -> Lowered {
        let Some(value) = value else {
            return Ok(None);
        };
        let Some((_, operated)) = self.operated_type(expr)? else {
            return Ok(None);
        };
        Ok((value.ty() == operated).then_some(value))
    }
}

/// The binary operation of an arithmetic operator; `None` for `**`, which is
/// the standard function `EXPT`.
fn arithmetic_op(op: &Operator) -> Option<BinaryOp> {
    match op {
        Operator::Add => Some(BinaryOp::Add),
        Operator::Sub => Some(BinaryOp::Sub),
        Operator::Mul => Some(BinaryOp::Mul),
        Operator::Div => Some(BinaryOp::Div),
        Operator::Mod => Some(BinaryOp::Mod),
        Operator::Pow => None,
    }
}

/// The logical and the bitwise operation of `AND`, `OR` or `XOR`; `None` for
/// any other operator.
fn logical_ops(op: &CompareOp) -> Option<(BinaryOp, BinaryOp)> {
    match op {
        CompareOp::And => Some((BinaryOp::BoolAnd, BinaryOp::BitAnd)),
        CompareOp::Or => Some((BinaryOp::BoolOr, BinaryOp::BitOr)),
        CompareOp::Xor => Some((BinaryOp::BoolXor, BinaryOp::BitXor)),
        CompareOp::Eq
        | CompareOp::Ne
        | CompareOp::Lt
        | CompareOp::Gt
        | CompareOp::LtEq
        | CompareOp::GtEq
        | CompareOp::AndThen
        | CompareOp::OrElse => None,
    }
}

/// The comparison of a comparison operator; `None` for any other operator.
fn compare_op(op: &CompareOp) -> Option<LoweredCompareOp> {
    match op {
        CompareOp::Eq => Some(LoweredCompareOp::Eq),
        CompareOp::Ne => Some(LoweredCompareOp::Ne),
        CompareOp::Lt => Some(LoweredCompareOp::Lt),
        CompareOp::Gt => Some(LoweredCompareOp::Gt),
        CompareOp::LtEq => Some(LoweredCompareOp::Le),
        CompareOp::GtEq => Some(LoweredCompareOp::Ge),
        CompareOp::And
        | CompareOp::Or
        | CompareOp::Xor
        | CompareOp::AndThen
        | CompareOp::OrElse => None,
    }
}

/// The shift or rotate `shift` of a bit string operated at `operated` and
/// `width` bits wide.
fn shift_intrinsic(shift: BitShift, operated: ScalarType, width: u8) -> Option<Intrinsic> {
    // A bit string operates as an unsigned integer. A shift of a `BOOL`, or
    // of an integer the analyzer let through, is not lowered.
    let wide = match operated {
        ScalarType::I32(Signedness::Unsigned) => false,
        ScalarType::I64(Signedness::Unsigned) => true,
        ScalarType::Bool
        | ScalarType::I32(Signedness::Signed)
        | ScalarType::I64(Signedness::Signed)
        | ScalarType::F32
        | ScalarType::F64
        | ScalarType::Ref
        | ScalarType::Interface => return None,
    };
    match shift {
        BitShift::Shl if wide => Some(Intrinsic::ShlU64),
        BitShift::Shl => Some(Intrinsic::ShlU32),
        BitShift::Shr if wide => Some(Intrinsic::ShrU64),
        BitShift::Shr => Some(Intrinsic::ShrU32),
        BitShift::Rol => rotate(
            wide,
            width,
            [
                Intrinsic::RolU8,
                Intrinsic::RolU16,
                Intrinsic::RolU32,
                Intrinsic::RolU64,
            ],
        ),
        BitShift::Ror => rotate(
            wide,
            width,
            [
                Intrinsic::RorU8,
                Intrinsic::RorU16,
                Intrinsic::RorU32,
                Intrinsic::RorU64,
            ],
        ),
    }
}

/// The rotate of a bit string `width` bits wide, from the rotates of 8, 16,
/// 32 and 64 bits. A rotate keeps its bits within the width of its type, so
/// there is one per width.
fn rotate(
    wide: bool,
    width: u8,
    [bits_8, bits_16, bits_32, bits_64]: [Intrinsic; 4],
) -> Option<Intrinsic> {
    match (wide, width) {
        (false, 8) => Some(bits_8),
        (false, 16) => Some(bits_16),
        (false, 32) => Some(bits_32),
        (true, 64) => Some(bits_64),
        // No bit string has any other width.
        _ => None,
    }
}

/// The numeric function `function` computed at `operated`.
fn numeric_intrinsic(function: NumericFunction, operated: ScalarType) -> Option<Intrinsic> {
    let at = match function {
        NumericFunction::Abs => AtTypes {
            signed: Some([Intrinsic::AbsI32, Intrinsic::AbsI64]),
            unsigned: None,
            reals: [Intrinsic::AbsF32, Intrinsic::AbsF64],
        },
        NumericFunction::Expt => AtTypes {
            signed: Some([Intrinsic::ExptI32, Intrinsic::ExptI64]),
            unsigned: None,
            reals: [Intrinsic::ExptF32, Intrinsic::ExptF64],
        },
        NumericFunction::Sqrt => AtTypes::reals(Intrinsic::SqrtF32, Intrinsic::SqrtF64),
        NumericFunction::Ln => AtTypes::reals(Intrinsic::LnF32, Intrinsic::LnF64),
        NumericFunction::Log => AtTypes::reals(Intrinsic::LogF32, Intrinsic::LogF64),
        NumericFunction::Exp => AtTypes::reals(Intrinsic::ExpF32, Intrinsic::ExpF64),
        NumericFunction::Sin => AtTypes::reals(Intrinsic::SinF32, Intrinsic::SinF64),
        NumericFunction::Cos => AtTypes::reals(Intrinsic::CosF32, Intrinsic::CosF64),
        NumericFunction::Tan => AtTypes::reals(Intrinsic::TanF32, Intrinsic::TanF64),
        NumericFunction::Asin => AtTypes::reals(Intrinsic::AsinF32, Intrinsic::AsinF64),
        NumericFunction::Acos => AtTypes::reals(Intrinsic::AcosF32, Intrinsic::AcosF64),
        NumericFunction::Atan => AtTypes::reals(Intrinsic::AtanF32, Intrinsic::AtanF64),
        NumericFunction::Atan2 => AtTypes::reals(Intrinsic::Atan2F32, Intrinsic::Atan2F64),
        NumericFunction::TruncReal => AtTypes::reals(Intrinsic::TruncF32, Intrinsic::TruncF64),
        NumericFunction::ModReal => AtTypes::reals(Intrinsic::ModF32, Intrinsic::ModF64),
        NumericFunction::Min => AtTypes::every([
            Intrinsic::MinI32,
            Intrinsic::MinI64,
            Intrinsic::MinU32,
            Intrinsic::MinU64,
            Intrinsic::MinF32,
            Intrinsic::MinF64,
        ]),
        NumericFunction::Max => AtTypes::every([
            Intrinsic::MaxI32,
            Intrinsic::MaxI64,
            Intrinsic::MaxU32,
            Intrinsic::MaxU64,
            Intrinsic::MaxF32,
            Intrinsic::MaxF64,
        ]),
        NumericFunction::Limit => AtTypes::every([
            Intrinsic::LimitI32,
            Intrinsic::LimitI64,
            Intrinsic::LimitU32,
            Intrinsic::LimitU64,
            Intrinsic::LimitF32,
            Intrinsic::LimitF64,
        ]),
        NumericFunction::Sel => AtTypes::every([
            Intrinsic::SelI32,
            Intrinsic::SelI64,
            Intrinsic::SelU32,
            Intrinsic::SelU64,
            Intrinsic::SelF32,
            Intrinsic::SelF64,
        ]),
    };
    let [real, long_real] = at.reals;
    match operated {
        ScalarType::I32(Signedness::Signed) => at.signed.map(|[integer, _]| integer),
        ScalarType::I64(Signedness::Signed) => at.signed.map(|[_, long]| long),
        // `EXPT` of an unsigned integer is not lowered yet. The analyzer
        // removes `ABS` of one.
        ScalarType::I32(Signedness::Unsigned) => at.unsigned.map(|[integer, _]| integer),
        ScalarType::I64(Signedness::Unsigned) => at.unsigned.map(|[_, long]| long),
        ScalarType::F32 => Some(real),
        ScalarType::F64 => Some(long_real),
        ScalarType::Bool | ScalarType::Ref | ScalarType::Interface => None,
    }
}

/// The intrinsics of a numeric function: at a 32-bit and a 64-bit signed
/// integer, and unsigned integer, where it takes them, and at a `REAL` and an
/// `LREAL`.
struct AtTypes {
    signed: Option<[Intrinsic; 2]>,
    unsigned: Option<[Intrinsic; 2]>,
    reals: [Intrinsic; 2],
}

impl AtTypes {
    /// A function of reals only.
    fn reals(real: Intrinsic, long_real: Intrinsic) -> Self {
        AtTypes {
            signed: None,
            unsigned: None,
            reals: [real, long_real],
        }
    }

    /// A function of every integer and real, in the order `I32`, `I64`,
    /// `U32`, `U64`, `F32`, `F64`.
    fn every([i32, i64, u32, u64, f32, f64]: [Intrinsic; 6]) -> Self {
        AtTypes {
            signed: Some([i32, i64]),
            unsigned: Some([u32, u64]),
            reals: [f32, f64],
        }
    }
}
