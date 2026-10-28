//! Strings: a string literal, the string at a place, the assignment of a
//! string to a string place, calls of the standard string functions,
//! conversions to and from a `STRING`, and comparisons of strings.
//!
//! A literal is in the encoding its delimiter spells: `'abc'` is a `STRING`
//! and `"abc"` a `WSTRING` (ADR-0016). The analyzer reports a literal stored
//! at a string of the other encoding (P4035), so the assignment refuses one
//! as a broken invariant (REQ-LOW-lowering-050).
//!
//! A string function's strings share an encoding, which the analyzer checks
//! (P4034), so the encoding of its first string chooses its `Intrinsic`.
//! `LEN` and `FIND` yield a number, an `Expr`; the others yield a string, a
//! `StrExpr` with room for every code unit of their strings
//! (REQ-LOW-lowering-167).
//!
//! A conversion from a `STRING` reads a number of the type the analyzer
//! recorded for the call, as the program's parse policy says. A conversion
//! to a `STRING` writes its number into a string of the default capacity
//! (REQ-LOW-lowering-168).
//!
//! A comparison of two strings, which share an encoding as a string
//! function's do, compares the order its `Intrinsic` gives them with zero.

use ironplc_analyzer::{BuiltinFunction, StringFunction};
use ironplc_container::DEFAULT_STRING_MAX_LENGTH;
use ironplc_dsl::common::{ConstantKind, ElementaryTypeName, StringType};
use ironplc_dsl::core::{Located, SourceSpan};
use ironplc_dsl::textual::{Expr as AstExpr, ExprKind as AstExprKind, Function};
use ironplc_ir::lowered::{
    Arg, CompareOp, Encoding, Expr, Intrinsic, NarrowWidth, Param, Place, ScalarType, Signedness,
    Stmt, StrExpr,
};

use crate::typed::Class;
use crate::variables::BodyFrame;
use crate::{Body, Lowered, Lowering, Unlowered};

impl Lowering<'_> {
    /// `place := value`, where `place` is a string.
    pub(crate) fn string_assignment(
        &mut self,
        frame: &BodyFrame,
        place: Place,
        value: &AstExpr,
        span: SourceSpan,
    ) -> Lowered<Stmt> {
        let value = self.str_expr(frame, value)?;
        Ok(self.program.assign_str(place, value, span)?)
    }

    /// The call `expr` of the string function `function` with `args`, where
    /// it yields a number: `LEN` and `FIND`. A string function that yields
    /// a string is a `StrExpr` ([`Self::string_call`]).
    pub(crate) fn string_function_call(
        &mut self,
        frame: &BodyFrame,
        expr: &AstExpr,
        function: StringFunction,
        args: &[&AstExpr],
    ) -> Lowered<Expr> {
        let intrinsic = self.string_intrinsic(function, args)?;
        let Param::Value(_) = intrinsic.signature().result() else {
            return Err(Unlowered::NotYet);
        };
        self.intrinsic_call(frame, expr, intrinsic, args)
    }

    /// The string the call `func`, the expression `expr`, yields: a call of
    /// a standard string function that yields a string, a conversion to a
    /// `STRING`, or a call of a function whose result is a string, moved
    /// out of its statement (`string_result.rs`). A call of any other
    /// function yields a number, an `Expr` (`call.rs`), not a string.
    fn string_call(
        &mut self,
        frame: &BodyFrame,
        expr: &AstExpr,
        func: &Function,
    ) -> Lowered<StrExpr> {
        let signature = self
            .analysis
            .context()
            .functions()
            .get(&func.name)
            .ok_or(Unlowered::NotYet)?;
        let Some(builtin) = signature.builtin_function.clone() else {
            return match self.body_named(&func.name) {
                Some(Body::Function(function)) => {
                    self.string_result_call(frame, expr, func, function)
                }
                Some(Body::Program(_) | Body::FunctionBlock(_) | Body::Method { .. }) | None => {
                    Err(Unlowered::NotYet)
                }
            };
        };
        // The analyzer wrote the call in normal form: its inputs in the order
        // of the function's parameters.
        let args: Vec<&AstExpr> = func.inputs().collect();
        // `MOVE(s)` is `s` (REQ-LOW-lowering-203).
        if let (BuiltinFunction::Move, [value]) = (&builtin, args.as_slice()) {
            return self.str_expr(frame, value);
        }
        if let BuiltinFunction::String(function) = builtin {
            return self.string_function_string(frame, expr, function, &args);
        }
        if let BuiltinFunction::Conversion {
            target: ElementaryTypeName::STRING,
            ..
        } = builtin
        {
            return self.number_to_string(frame, expr, &args);
        }
        Err(Unlowered::NotYet)
    }

    /// The string the call `expr` of the string function `function` with
    /// `args` yields, where it yields one. Its capacity is the sum of its
    /// strings' (REQ-LOW-lowering-167): room for every code unit `CONCAT`,
    /// `INSERT` and `REPLACE` join, and for all `LEFT`, `RIGHT`, `MID` and
    /// `DELETE` can keep of their one string.
    fn string_function_string(
        &mut self,
        frame: &BodyFrame,
        expr: &AstExpr,
        function: StringFunction,
        args: &[&AstExpr],
    ) -> Lowered<StrExpr> {
        let intrinsic = self.string_intrinsic(function, args)?;
        let Param::Str(_) = intrinsic.signature().result() else {
            return Err(Unlowered::NotYet);
        };
        let args = self.intrinsic_args(frame, intrinsic, args)?;
        let capacity = args.iter().fold(0u32, |sum, arg| match arg {
            Arg::Str(value) => sum.saturating_add(value.shape().capacity),
            Arg::Value(_) | Arg::Ref(_) | Arg::Copy(_) => sum,
        });
        Ok(StrExpr::call(intrinsic, args, capacity, expr.span())?)
    }

    /// The conversion of `args`, one number, to a `STRING`, the expression
    /// `expr`: the intrinsic that writes a number of its operation type, into
    /// a string of the default capacity (REQ-LOW-lowering-168).
    fn number_to_string(
        &mut self,
        frame: &BodyFrame,
        expr: &AstExpr,
        args: &[&AstExpr],
    ) -> Lowered<StrExpr> {
        let [value] = args else {
            return Err(Unlowered::NotYet);
        };
        let operated = self.typed(value)?.scalar()?;
        let intrinsic = match operated {
            ScalarType::I32(Signedness::Signed) => Intrinsic::I32ToString,
            ScalarType::I32(Signedness::Unsigned) => Intrinsic::U32ToString,
            ScalarType::F32 => Intrinsic::F32ToString,
            ScalarType::I64(_)
            | ScalarType::F64
            | ScalarType::Bool
            | ScalarType::Ref
            | ScalarType::Interface => return Err(Unlowered::NotYet),
        };
        let args = self.intrinsic_args(frame, intrinsic, args)?;
        let capacity = u32::from(DEFAULT_STRING_MAX_LENGTH);
        Ok(StrExpr::call(intrinsic, args, capacity, expr.span())?)
    }

    /// `STRING_TO_*(value)`, the expression `expr`: the intrinsic that reads
    /// a number of the type the analyzer recorded for the call, as the
    /// program's parse policy says. A bit string reads as the unsigned
    /// integer of its width.
    pub(crate) fn string_to_number(
        &mut self,
        frame: &BodyFrame,
        expr: &AstExpr,
        value: &AstExpr,
    ) -> Lowered<Expr> {
        let target = self.typed(expr)?;
        let operated = target.scalar()?;
        let parse = self.parse;
        let intrinsic = match operated {
            ScalarType::I32(signedness) => {
                let [bits_8, bits_16, bits_32] = match signedness {
                    Signedness::Signed => [
                        Intrinsic::StringToI8(parse),
                        Intrinsic::StringToI16(parse),
                        Intrinsic::StringToI32(parse),
                    ],
                    Signedness::Unsigned => [
                        Intrinsic::StringToU8(parse),
                        Intrinsic::StringToU16(parse),
                        Intrinsic::StringToU32(parse),
                    ],
                };
                match self.program.types().narrowed_storage(target.ty) {
                    Some(NarrowWidth::B8) => bits_8,
                    Some(NarrowWidth::B16) => bits_16,
                    None => bits_32,
                    Some(NarrowWidth::B32) => return Err(Unlowered::NotYet),
                }
            }
            ScalarType::I64(Signedness::Signed) => Intrinsic::StringToI64(parse),
            ScalarType::I64(Signedness::Unsigned) => Intrinsic::StringToU64(parse),
            ScalarType::F32 => Intrinsic::StringToF32(parse),
            ScalarType::F64 => Intrinsic::StringToF64(parse),
            ScalarType::Bool | ScalarType::Ref | ScalarType::Interface => {
                return Err(Unlowered::NotYet)
            }
        };
        self.intrinsic_call(frame, expr, intrinsic, &[value])
    }

    /// The comparison `op` of `left` and `right`, strings of `encoding`: the
    /// order the intrinsic of `encoding` gives them, compared with zero.
    pub(crate) fn string_compare(
        &mut self,
        frame: &BodyFrame,
        encoding: Encoding,
        op: CompareOp,
        (left, right): (&AstExpr, &AstExpr),
        span: SourceSpan,
    ) -> Lowered<Expr> {
        let intrinsic = match encoding {
            Encoding::Narrow => Intrinsic::CompareString,
            Encoding::Wide => Intrinsic::CompareWString,
        };
        let args = self.intrinsic_args(frame, intrinsic, &[left, right])?;
        let order = Expr::call(intrinsic, args, span.clone())?;
        let zero = Expr::integer(0, order.ty(), span.clone())?;
        Ok(Expr::compare(op, order, zero, span)?)
    }

    /// The intrinsic of the string function `function` for the encoding the
    /// analyzer recorded for its first argument.
    fn string_intrinsic(
        &mut self,
        function: StringFunction,
        args: &[&AstExpr],
    ) -> Lowered<Intrinsic> {
        let [narrow, wide] = match function {
            StringFunction::Len => [Intrinsic::LenString, Intrinsic::LenWString],
            StringFunction::Find => [Intrinsic::FindString, Intrinsic::FindWString],
            StringFunction::Concat => [Intrinsic::ConcatString, Intrinsic::ConcatWString],
            StringFunction::Insert => [Intrinsic::InsertString, Intrinsic::InsertWString],
            StringFunction::Replace => [Intrinsic::ReplaceString, Intrinsic::ReplaceWString],
            StringFunction::Delete => [Intrinsic::DeleteString, Intrinsic::DeleteWString],
            StringFunction::Left => [Intrinsic::LeftString, Intrinsic::LeftWString],
            StringFunction::Right => [Intrinsic::RightString, Intrinsic::RightWString],
            StringFunction::Mid => [Intrinsic::MidString, Intrinsic::MidWString],
        };
        let first = args.first().ok_or(Unlowered::NotYet)?;
        let Class::String(encoding) = self.typed(first)?.class else {
            return Err(Unlowered::NotYet);
        };
        Ok(match encoding {
            Encoding::Narrow => narrow,
            Encoding::Wide => wide,
        })
    }

    /// The string `expr` makes. A method's string result is not lowered
    /// yet, and an expression that yields a number, a `BOOL`, a time or a
    /// date is a scalar (`expr.rs`), not a string.
    pub(crate) fn str_expr(&mut self, frame: &BodyFrame, expr: &AstExpr) -> Lowered<StrExpr> {
        let span = expr.span();
        match &expr.kind {
            AstExprKind::Expression(inner) => self.str_expr(frame, inner),
            AstExprKind::Const(constant) => match constant {
                ConstantKind::CharacterString(literal) => {
                    let encoding = match literal.width {
                        StringType::String => Encoding::Narrow,
                        StringType::WString => Encoding::Wide,
                    };
                    Ok(StrExpr::literal(literal.value.clone(), encoding, span)?)
                }
                ConstantKind::IntegerLiteral(_)
                | ConstantKind::RealLiteral(_)
                | ConstantKind::Boolean(_)
                | ConstantKind::Duration(_)
                | ConstantKind::TimeOfDay(_)
                | ConstantKind::Date(_)
                | ConstantKind::DateAndTime(_)
                | ConstantKind::BitStringLiteral(_) => Err(Unlowered::NotYet),
            },
            AstExprKind::Variable(variable) => {
                let place = self.variable_place(frame, variable)?;
                Ok(self.program.read_str(place, span)?)
            }
            AstExprKind::Function(func) => self.string_call(frame, expr, func),
            // The string output of the call whose outputs are being stored.
            AstExprKind::CallOutput(output) => {
                let place = self.call_output_place(frame, output)?;
                Ok(self.program.read_str(place, span)?)
            }
            // The string a reference variable refers to, and `REF(s)^`, a
            // read of `s` (REQ-LOW-lowering-188).
            AstExprKind::Deref(reference) => {
                let place = self.dereferenced_place(frame, reference, &span)?;
                Ok(self.program.read_str(place, span)?)
            }
            AstExprKind::MethodCall(_)
            | AstExprKind::Compare(_)
            | AstExprKind::BinaryOp(_)
            | AstExprKind::UnaryOp(_)
            | AstExprKind::EnumeratedValue(_)
            | AstExprKind::LateBound(_)
            | AstExprKind::Ref(_)
            | AstExprKind::ImplicitConversion(_)
            | AstExprKind::Null(_) => Err(Unlowered::NotYet),
        }
    }
}
