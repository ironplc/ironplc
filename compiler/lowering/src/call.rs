//! Calls of functions. A call of a user-defined function is a `Call` of its
//! POU (`function.rs`). Of the standard functions, a function form of an
//! operator, and `MOVE`, lower to what the operator, or the operand, lowers
//! to; a type conversion between numbers and `TRUNC` to a `Convert`;
//! `SIZEOF` to a constant;
//! a time or date function to arithmetic (`time.rs`); any other function is
//! the `Intrinsic` for its function at its operand types
//! (REQ-LOW-lowering-147), a string function at the encoding of its strings
//! and a conversion to or from a `STRING` at its number's type (`string.rs`). The match over the analyzer's `BuiltinFunction`
//! names every function, so one the analyzer adds is not lowered until it
//! has an arm here.

use ironplc_analyzer::{BitShift, BuiltinFunction, FormOf, NumericFunction};
use ironplc_dsl::common::ElementaryTypeName;
use ironplc_dsl::core::{Located, SourceSpan};
use ironplc_dsl::textual::{
    CompareOp, Expr as AstExpr, ExprKind as AstExprKind, Function, Operator,
};
use ironplc_ir::lowered::{
    Arg, BinaryOp, CompareOp as LoweredCompareOp, Encoding, Expr, Intrinsic, Param, ScalarType,
    Signedness, StringShape, TypeId, TypeKind, TypeTable,
};

use crate::typed::Arithmetic;
use crate::variables::BodyFrame;
use crate::{Body, Lowered, Lowering, Unlowered};

impl Lowering<'_> {
    /// The call `func`, the expression `expr`.
    pub(crate) fn function_call(
        &mut self,
        frame: &BodyFrame,
        expr: &AstExpr,
        func: &Function,
    ) -> Lowered<Expr> {
        let signature = self
            .analysis
            .context()
            .functions()
            .get(&func.name)
            .ok_or(Unlowered::NotYet)?;
        let Some(builtin) = signature.builtin_function.clone() else {
            return match self.body_named(&func.name) {
                Some(Body::Function(function)) => {
                    self.user_function_call(frame, expr, func, function)
                }
                Some(Body::Program(_) | Body::FunctionBlock(_) | Body::Method { .. }) | None => {
                    Err(Unlowered::NotYet)
                }
            };
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
                _ => Err(Unlowered::NotYet),
            },
            BuiltinFunction::BitShift(shift) => {
                // A shift computes at the type of the value it shifts.
                let typed = self.typed(expr)?;
                let operated = typed.scalar()?;
                let intrinsic = self
                    .program
                    .types()
                    .bit_width(typed.ty)
                    .and_then(|width| shift_intrinsic(shift, operated, width))
                    .ok_or(Unlowered::NotYet)?;
                self.intrinsic_call(frame, expr, intrinsic, &args)
            }
            BuiltinFunction::Numeric(function) => self.numeric_call(frame, expr, function, &args),
            BuiltinFunction::Mux => {
                // `MUX` computes at the type the analyzer converted its
                // inputs to.
                let operated = self.typed(expr)?.scalar()?;
                if operated == ScalarType::Bool {
                    return self.bool_selection(frame, expr, Intrinsic::MuxI32, &args);
                }
                let mux = AtTypes::every([
                    Intrinsic::MuxI32,
                    Intrinsic::MuxI64,
                    Intrinsic::MuxU32,
                    Intrinsic::MuxU64,
                    Intrinsic::MuxF32,
                    Intrinsic::MuxF64,
                ]);
                let intrinsic = mux.at(operated).ok_or(Unlowered::NotYet)?;
                self.intrinsic_call(frame, expr, intrinsic, &args)
            }
            BuiltinFunction::Trunc => match args.as_slice() {
                [value] => self.trunc(frame, expr, value),
                _ => Err(Unlowered::NotYet),
            },
            BuiltinFunction::Conversion { source, target } => match args.as_slice() {
                // A conversion to a `STRING` yields a string, a `StrExpr`
                // (`string.rs`), not a scalar.
                [_] if target == ElementaryTypeName::STRING => Err(Unlowered::NotYet),
                [value] if source == ElementaryTypeName::STRING => {
                    self.string_to_number(frame, expr, value)
                }
                [value] => self.conversion(frame, expr, value),
                _ => Err(Unlowered::NotYet),
            },
            BuiltinFunction::BcdToInt => match args.as_slice() {
                [value] => self.bcd_to_int(frame, expr, value),
                _ => Err(Unlowered::NotYet),
            },
            BuiltinFunction::IntToBcd => match args.as_slice() {
                [value] => self.int_to_bcd(frame, expr, value),
                _ => Err(Unlowered::NotYet),
            },
            BuiltinFunction::Sizeof => match args.as_slice() {
                [value] => self.sizeof(expr, value),
                _ => Err(Unlowered::NotYet),
            },
            // A typed time function computes at the width of its result.
            BuiltinFunction::Time { function, .. } => {
                self.time_function(frame, expr, function, &args)
            }
            BuiltinFunction::DtToDate => match args.as_slice() {
                [value] => self.dt_to_date(frame, expr, value),
                _ => Err(Unlowered::NotYet),
            },
            BuiltinFunction::DtToTod => match args.as_slice() {
                [value] => self.dt_to_tod(frame, expr, value),
                _ => Err(Unlowered::NotYet),
            },
            BuiltinFunction::String(function) => {
                self.string_function_call(frame, expr, function, &args)
            }
        }
    }

    /// `TRUNC(value)`, the expression `expr`: the real `value` converted to
    /// the integer the analyzer recorded for the call, the integer of its
    /// context. Like an arithmetic result, it is not narrowed until it is
    /// stored.
    fn trunc(&mut self, frame: &BodyFrame, expr: &AstExpr, value: &AstExpr) -> Lowered<Expr> {
        let to = self.typed(expr)?.scalar()?;
        let value = self.expr(frame, value)?;
        if !(value.ty().is_real() && to.is_integer()) {
            return Err(Unlowered::NotYet);
        }
        Ok(Expr::convert(value, to, expr.span())?)
    }

    /// The type conversion `<SOURCE>_TO_<TARGET>(value)`, the expression
    /// `expr`: a `Convert` where the operation type changes, then a
    /// `Truncate` where the target's storage narrows. A conversion to `BOOL`
    /// is a comparison with zero.
    fn conversion(&mut self, frame: &BodyFrame, expr: &AstExpr, value: &AstExpr) -> Lowered<Expr> {
        let span = expr.span();
        // The analyzer recorded the target as the type of the call.
        let target = self.typed(expr)?;
        let to = target.scalar()?;
        let value = self.expr(frame, value)?;
        if to == ScalarType::Bool {
            let zero = Expr::integer(0, value.ty(), span.clone())?;
            return Ok(Expr::compare(LoweredCompareOp::Ne, value, zero, span)?);
        }
        let value = if value.ty() == to {
            value
        } else {
            Expr::convert(value, to, span.clone())?
        };
        match self.program.types().narrowed_storage(target.ty) {
            Some(width) => Ok(Expr::truncate(width, value, span)?),
            None => Ok(value),
        }
    }

    /// `BCD_TO_INT(value)`, the expression `expr`: the bit string `value`
    /// decoded at its width, then converted to the integer the analyzer
    /// recorded for the call, the integer of its context.
    fn bcd_to_int(&mut self, frame: &BodyFrame, expr: &AstExpr, value: &AstExpr) -> Lowered<Expr> {
        let to = self.typed(expr)?.scalar()?;
        if !to.is_integer() {
            return Err(Unlowered::NotYet);
        }
        let ty = self.typed(value)?.ty;
        let intrinsic = self
            .program
            .types()
            .bit_width(ty)
            .and_then(|width| {
                at_width(
                    width,
                    [
                        Intrinsic::BcdToIntU8,
                        Intrinsic::BcdToIntU16,
                        Intrinsic::BcdToIntU32,
                        Intrinsic::BcdToIntU64,
                    ],
                )
            })
            .ok_or(Unlowered::NotYet)?;
        let decoded = self.intrinsic_call(frame, expr, intrinsic, &[value])?;
        if decoded.ty() == to {
            return Ok(decoded);
        }
        Ok(Expr::convert(decoded, to, expr.span())?)
    }

    /// `INT_TO_BCD(value)`, the expression `expr`: the integer `value`
    /// encoded at its width and signedness, as the bit string as wide as it.
    fn int_to_bcd(&mut self, frame: &BodyFrame, expr: &AstExpr, value: &AstExpr) -> Lowered<Expr> {
        let typed = self.typed(value)?;
        let operated = typed.scalar()?;
        let at_signedness = match operated.signedness() {
            Some(Signedness::Signed) => [
                Intrinsic::IntToBcdI8,
                Intrinsic::IntToBcdI16,
                Intrinsic::IntToBcdI32,
                Intrinsic::IntToBcdI64,
            ],
            Some(Signedness::Unsigned) => [
                Intrinsic::IntToBcdU8,
                Intrinsic::IntToBcdU16,
                Intrinsic::IntToBcdU32,
                Intrinsic::IntToBcdU64,
            ],
            None => return Err(Unlowered::NotYet),
        };
        let intrinsic = self
            .program
            .types()
            .bit_width(typed.ty)
            .and_then(|width| at_width(width, at_signedness))
            .ok_or(Unlowered::NotYet)?;
        let encoded = self.intrinsic_call(frame, expr, intrinsic, &[value])?;
        self.of_own_type(expr, encoded)
    }

    /// `SIZEOF(value)`, the expression `expr`: the size in bytes of the type
    /// of `value`, which is not evaluated, as a constant of the integer the
    /// analyzer recorded for the call (REQ-LOW-lowering-103). A type whose
    /// size only a layout gives, such as a structure, is not lowered.
    fn sizeof(&mut self, expr: &AstExpr, value: &AstExpr) -> Lowered<Expr> {
        let ty = self.typed(expr)?.scalar()?;
        let of = self.typed(value)?.ty;
        let size = byte_size(self.program.types(), of).ok_or(Unlowered::NotYet)?;
        if !ty
            .integer_range()
            .is_some_and(|(min, max)| (min..=max).contains(&size))
        {
            return Err(Unlowered::NotYet);
        }
        Ok(Expr::integer(size, ty, expr.span())?)
    }

    /// The call `expr` of the numeric function `function` with `args`. The
    /// operator `a ** b` is a call of `EXPT`.
    pub(crate) fn numeric_call(
        &mut self,
        frame: &BodyFrame,
        expr: &AstExpr,
        function: NumericFunction,
        args: &[&AstExpr],
    ) -> Lowered<Expr> {
        // A numeric function computes at the type the analyzer converted its
        // inputs to.
        let operated = self.typed(expr)?.scalar()?;
        if function == NumericFunction::Sel && operated == ScalarType::Bool {
            return self.bool_selection(frame, expr, Intrinsic::SelI32, args);
        }
        let intrinsic = numeric_intrinsic(function, operated).ok_or(Unlowered::NotYet)?;
        self.intrinsic_call(frame, expr, intrinsic, args)
    }

    /// `SEL` or `MUX` of `BOOL`s, the call `expr`: the selection `intrinsic`
    /// at the signed 32-bit integer, of each `BOOL` it selects between
    /// converted to that integer as `BOOL_TO_DINT` converts it, compared with
    /// zero as `DINT_TO_BOOL` converts the integer back (REQ-LOW-lowering-202).
    /// No `Intrinsic` selects between `BOOL`s, and the integers of `TRUE` and
    /// `FALSE` select as the `BOOL`s do. The selector is lowered as it is.
    fn bool_selection(
        &mut self,
        frame: &BodyFrame,
        expr: &AstExpr,
        intrinsic: Intrinsic,
        args: &[&AstExpr],
    ) -> Lowered<Expr> {
        let span = expr.span();
        let integer = ScalarType::I32(Signedness::Signed);
        let (selector, inputs) = args.split_first().ok_or(Unlowered::NotYet)?;
        let mut lowered = Vec::with_capacity(args.len());
        lowered.push(Arg::Value(self.expr(frame, selector)?));
        for input in inputs {
            let value = self.expr(frame, input)?;
            if value.ty() != ScalarType::Bool {
                return Err(Unlowered::NotYet);
            }
            lowered.push(Arg::Value(Expr::convert(value, integer, input.span())?));
        }
        let passed: Vec<Param> = lowered.iter().map(Arg::param).collect();
        if !intrinsic.signature().takes(&passed) {
            return Err(Unlowered::NotYet);
        }
        let selected = Expr::call(intrinsic, lowered, span.clone())?;
        let zero = Expr::integer(0, integer, span.clone())?;
        Ok(Expr::compare(LoweredCompareOp::Ne, selected, zero, span)?)
    }

    /// The call `expr` of `intrinsic` with `args`.
    pub(crate) fn intrinsic_call(
        &mut self,
        frame: &BodyFrame,
        expr: &AstExpr,
        intrinsic: Intrinsic,
        args: &[&AstExpr],
    ) -> Lowered<Expr> {
        let args = self.intrinsic_args(frame, intrinsic, args)?;
        Ok(Expr::call(intrinsic, args, expr.span())?)
    }

    /// `args` of a call of `intrinsic`, each lowered as what its parameter
    /// takes ([`Self::arg`]). An argument of another type than its
    /// parameter's, which the analyzer did not convert to it, is not
    /// lowered.
    pub(crate) fn intrinsic_args(
        &mut self,
        frame: &BodyFrame,
        intrinsic: Intrinsic,
        args: &[&AstExpr],
    ) -> Lowered<Vec<Arg>> {
        let signature = intrinsic.signature();
        let mut lowered = Vec::with_capacity(args.len());
        for (index, arg) in args.iter().enumerate() {
            let param = signature.param(index).ok_or(Unlowered::NotYet)?;
            lowered.push(self.arg(frame, param, arg)?);
        }
        let passed: Vec<Param> = lowered.iter().map(Arg::param).collect();
        if !signature.takes(&passed) {
            return Err(Unlowered::NotYet);
        }
        Ok(lowered)
    }

    /// `expr` lowered as what `param` takes: a value, a string, or the place
    /// a variable names, passed by reference or, for an array or a
    /// structure, copied.
    pub(crate) fn arg(&mut self, frame: &BodyFrame, param: Param, expr: &AstExpr) -> Lowered<Arg> {
        match param {
            Param::Value(_) => Ok(Arg::Value(self.expr(frame, expr)?)),
            Param::Str(_) => Ok(Arg::Str(self.str_expr(frame, expr)?)),
            Param::Ref(_) => {
                let AstExprKind::Variable(variable) = &expr.kind else {
                    return Err(Unlowered::NotYet);
                };
                Ok(Arg::Ref(self.variable_place(frame, variable)?))
            }
            Param::Copy(_) => {
                let AstExprKind::Variable(variable) = &self.unwrapped(expr).kind else {
                    return Err(Unlowered::NotYet);
                };
                Ok(Arg::Copy(self.variable_place(frame, variable)?))
            }
        }
    }

    /// The value `MOVE(x)` and `(x)` are: `x`, through any number of `MOVE`s
    /// and parentheses, and `expr` itself when it is neither
    /// (REQ-LOW-lowering-203). An array, a structure or a function block
    /// instance is copied from a place rather than lowered as an expression,
    /// so where one is copied lowering reads through them to the place it
    /// copies.
    pub(crate) fn unwrapped<'e>(&self, mut expr: &'e AstExpr) -> &'e AstExpr {
        loop {
            if let AstExprKind::Expression(inner) = &expr.kind {
                expr = inner;
                continue;
            }
            if let AstExprKind::Function(func) = &expr.kind {
                let mut inputs = func.inputs();
                if let (true, Some(value), None) =
                    (self.is_move(func), inputs.next(), inputs.next())
                {
                    expr = value;
                    continue;
                }
            }
            return expr;
        }
    }

    /// Whether `func` is a call of `MOVE`.
    fn is_move(&self, func: &Function) -> bool {
        self.analysis
            .context()
            .functions()
            .get(&func.name)
            .is_some_and(|signature| {
                matches!(signature.builtin_function, Some(BuiltinFunction::Move))
            })
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
    ) -> Lowered<Expr> {
        let span = expr.span();
        let folded = match form {
            FormOf::Arithmetic(op) => {
                let op = arithmetic_op(op).ok_or(Unlowered::NotYet)?;
                match self.typed(expr)?.class.arithmetic()? {
                    Arithmetic::Temporal => {
                        // The analyzer wrote a fold of three or more times
                        // as the two-input calls it folds to.
                        let [in1, in2] = args else {
                            return Err(Unlowered::NotYet);
                        };
                        return self.time_arithmetic(frame, expr, op, in1, in2);
                    }
                    Arithmetic::Numeric => self.fold(frame, op, args, span)?,
                }
            }
            FormOf::Compare(op) => match logical_ops(op) {
                Some(ops) => {
                    let op = self.logical_op(expr, ops)?;
                    self.fold(frame, op, args, span)?
                }
                None => match (compare_op(op), args) {
                    (Some(op), [left, right]) => self.compare(frame, op, left, right, span)?,
                    _ => return Err(Unlowered::NotYet),
                },
            },
            FormOf::Not => match args {
                [term] => self.not(frame, expr, term)?,
                _ => return Err(Unlowered::NotYet),
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
    ) -> Lowered<Expr> {
        let mut values = self.exprs(frame, args)?.into_iter();
        let (Some(mut folded), Some(_)) = (values.next(), args.get(1)) else {
            return Err(Unlowered::NotYet);
        };
        for value in values {
            // The analyzer converted every input to the type the call
            // computes at; one it did not convert is not lowered yet.
            if value.ty() != folded.ty() {
                return Err(Unlowered::NotYet);
            }
            folded = Expr::binary(op, folded, value, span.clone())?;
        }
        Ok(folded)
    }

    /// Each of `args` lowered.
    fn exprs(&mut self, frame: &BodyFrame, args: &[&AstExpr]) -> Lowered<Vec<Expr>> {
        args.iter().map(|arg| self.expr(frame, arg)).collect()
    }

    /// `value`, when it has the type the analyzer recorded for `expr`; one
    /// of another type is not lowered.
    fn of_own_type(&mut self, expr: &AstExpr, value: Expr) -> Lowered<Expr> {
        let operated = self.typed(expr)?.scalar()?;
        if value.ty() != operated {
            return Err(Unlowered::NotYet);
        }
        Ok(value)
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
    // A bit string operates as an unsigned integer. The analyzer rejects a
    // shift of a `BOOL` (P4087); one of an integer it let through is not
    // lowered.
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
        BitShift::Rol => at_width(
            width,
            [
                Intrinsic::RolU8,
                Intrinsic::RolU16,
                Intrinsic::RolU32,
                Intrinsic::RolU64,
            ],
        ),
        BitShift::Ror => at_width(
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

/// The operation on a value `width` bits wide, from its variants for 8, 16,
/// 32 and 64 bits: a rotate keeps its bits within the width of its type, and
/// a BCD conversion has as many digits as the width holds.
fn at_width(width: u8, [bits_8, bits_16, bits_32, bits_64]: [Intrinsic; 4]) -> Option<Intrinsic> {
    match width {
        8 => Some(bits_8),
        16 => Some(bits_16),
        32 => Some(bits_32),
        64 => Some(bits_64),
        // No integer or bit string has any other width.
        _ => None,
    }
}

/// The size in bytes of a value of `ty`, from the language's storage widths
/// (REQ-LOW-lowering-103): a `BOOL` takes a byte, a string one code unit
/// more than its capacity, as CODESYS, TwinCAT and RuSTy store its
/// terminator, an enumeration its underlying type's size, and an array its
/// element's size for each element. `None` for a type whose size is a
/// target's layout, such as a structure.
fn byte_size(types: &TypeTable, ty: TypeId) -> Option<i128> {
    match types.kind(ty)? {
        TypeKind::Bool => Some(1),
        TypeKind::Integer { width, .. } | TypeKind::BitString { width } => {
            Some(i128::from(width.bits() / 8))
        }
        TypeKind::Real { width } | TypeKind::Temporal { width, .. } => {
            Some(i128::from(width.bits() / 8))
        }
        TypeKind::Subrange { base, .. } => byte_size(types, *base),
        TypeKind::Array {
            element,
            dimensions,
        } => dimensions
            .iter()
            .try_fold(byte_size(types, *element)?, |size, (lower, upper)| {
                size.checked_mul(i128::from(*upper) - i128::from(*lower) + 1)
            }),
        TypeKind::Enumeration { underlying, .. } => byte_size(types, *underlying),
        TypeKind::String(StringShape { encoding, capacity }) => {
            let code_unit = match encoding {
                Encoding::Narrow => 1,
                Encoding::Wide => 2,
            };
            Some(code_unit * (i128::from(*capacity) + 1))
        }
        TypeKind::Structure { .. }
        | TypeKind::FunctionBlock { .. }
        | TypeKind::Program { .. }
        | TypeKind::Reference { .. } => None,
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
        NumericFunction::Expt => AtTypes::every([
            Intrinsic::ExptI32,
            Intrinsic::ExptI64,
            Intrinsic::ExptU32,
            Intrinsic::ExptU64,
            Intrinsic::ExptF32,
            Intrinsic::ExptF64,
        ]),
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
    // `EXPT` of an unsigned integer is not lowered yet. The analyzer removes
    // `ABS` of one.
    at.at(operated)
}

/// The intrinsics of a numeric or selection function: at a 32-bit and a 64-bit signed
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

    /// The function's intrinsic at `operated`.
    fn at(&self, operated: ScalarType) -> Option<Intrinsic> {
        let [real, long_real] = self.reals;
        match operated {
            ScalarType::I32(Signedness::Signed) => self.signed.map(|[integer, _]| integer),
            ScalarType::I64(Signedness::Signed) => self.signed.map(|[_, long]| long),
            ScalarType::I32(Signedness::Unsigned) => self.unsigned.map(|[integer, _]| integer),
            ScalarType::I64(Signedness::Unsigned) => self.unsigned.map(|[_, long]| long),
            ScalarType::F32 => Some(real),
            ScalarType::F64 => Some(long_real),
            ScalarType::Bool | ScalarType::Ref | ScalarType::Interface => None,
        }
    }
}
