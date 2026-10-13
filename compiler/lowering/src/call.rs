//! Calls of standard functions. A function form of an operator, and `MOVE`,
//! lower to what the operator, or the operand, lowers to; any other function
//! is the `Intrinsic` for its function at its operand types
//! (REQ-LOW-lowering-147). The match over the analyzer's `BuiltinFunction`
//! names every function, so one the analyzer adds is not lowered until it has
//! an arm here.

use ironplc_analyzer::{BitShift, BuiltinFunction, FormOf};
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
            BuiltinFunction::Numeric(_)
            | BuiltinFunction::Mux
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
    const U32: ScalarType = ScalarType::I32(Signedness::Unsigned);
    const U64: ScalarType = ScalarType::I64(Signedness::Unsigned);
    let intrinsic = match (shift, operated, width) {
        (BitShift::Shl, U32, _) => Intrinsic::ShlU32,
        (BitShift::Shl, U64, _) => Intrinsic::ShlU64,
        (BitShift::Shr, U32, _) => Intrinsic::ShrU32,
        (BitShift::Shr, U64, _) => Intrinsic::ShrU64,
        (BitShift::Rol, U32, 8) => Intrinsic::RolU8,
        (BitShift::Rol, U32, 16) => Intrinsic::RolU16,
        (BitShift::Rol, U32, 32) => Intrinsic::RolU32,
        (BitShift::Rol, U64, 64) => Intrinsic::RolU64,
        (BitShift::Ror, U32, 8) => Intrinsic::RorU8,
        (BitShift::Ror, U32, 16) => Intrinsic::RorU16,
        (BitShift::Ror, U32, 32) => Intrinsic::RorU32,
        (BitShift::Ror, U64, 64) => Intrinsic::RorU64,
        // A shift of a `BOOL`, or of an integer the analyzer let through.
        _ => return None,
    };
    Some(intrinsic)
}
