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
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_dsl::textual::{Expr as AstExpr, ExprKind as AstExprKind, Function};
use ironplc_ir::lowered::{
    Arg, CompareOp, Encoding, Expr, Intrinsic, NarrowWidth, Param, Place, ScalarType, Signedness,
    Stmt, StrExpr, TypeKind,
};

use crate::expr::Lowered;
use crate::variables::BodyFrame;
use crate::{Body, Lowering};

impl Lowering<'_> {
    /// `place := value`, where `place` is a string.
    pub(crate) fn string_assignment(
        &mut self,
        frame: &BodyFrame,
        place: Place,
        value: &AstExpr,
        span: SourceSpan,
    ) -> Result<Option<Stmt>, Diagnostic> {
        let Some(value) = self.str_expr(frame, value)? else {
            return Ok(None);
        };
        self.program.assign_str(place, value, span).map(Some)
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
    ) -> Lowered {
        let Some(intrinsic) = self.string_intrinsic(function, args)? else {
            return Ok(None);
        };
        let Param::Value(_) = intrinsic.signature().result() else {
            return Ok(None);
        };
        self.intrinsic_call(frame, expr, intrinsic, args)
    }

    /// The string the call `func`, the expression `expr`, yields: a call of
    /// a standard string function that yields a string, a conversion to a
    /// `STRING`, or a call of a function whose result is a string, moved
    /// out of its statement (`string_result.rs`). `None` for a call of any
    /// other function.
    fn string_call(
        &mut self,
        frame: &BodyFrame,
        expr: &AstExpr,
        func: &Function,
    ) -> Result<Option<StrExpr>, Diagnostic> {
        let Some(signature) = self.analysis.context().functions().get(&func.name) else {
            return Ok(None);
        };
        let Some(builtin) = signature.builtin_function.clone() else {
            return match self.body_named(&func.name) {
                Some(Body::Function(function)) => {
                    self.string_result_call(frame, expr, func, function)
                }
                Some(Body::Program(_) | Body::FunctionBlock(_) | Body::Method { .. }) | None => {
                    Ok(None)
                }
            };
        };
        // The analyzer wrote the call in normal form: its inputs in the order
        // of the function's parameters.
        let args: Vec<&AstExpr> = func.inputs().collect();
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
        Ok(None)
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
    ) -> Result<Option<StrExpr>, Diagnostic> {
        let Some(intrinsic) = self.string_intrinsic(function, args)? else {
            return Ok(None);
        };
        let Param::Str(_) = intrinsic.signature().result() else {
            return Ok(None);
        };
        let Some(args) = self.intrinsic_args(frame, intrinsic, args)? else {
            return Ok(None);
        };
        let capacity = args.iter().fold(0u32, |sum, arg| match arg {
            Arg::Str(value) => sum.saturating_add(value.shape().capacity),
            Arg::Value(_) | Arg::Ref(_) => sum,
        });
        StrExpr::call(intrinsic, args, capacity, expr.span()).map(Some)
    }

    /// The conversion of `args`, one number, to a `STRING`, the expression
    /// `expr`: the intrinsic that writes a number of its operation type, into
    /// a string of the default capacity (REQ-LOW-lowering-168).
    fn number_to_string(
        &mut self,
        frame: &BodyFrame,
        expr: &AstExpr,
        args: &[&AstExpr],
    ) -> Result<Option<StrExpr>, Diagnostic> {
        let [value] = args else {
            return Ok(None);
        };
        let Some((_, operated)) = self.operated_type(value)? else {
            return Ok(None);
        };
        let intrinsic = match operated {
            ScalarType::I32(Signedness::Signed) => Intrinsic::I32ToString,
            ScalarType::I32(Signedness::Unsigned) => Intrinsic::U32ToString,
            ScalarType::F32 => Intrinsic::F32ToString,
            ScalarType::I64(_)
            | ScalarType::F64
            | ScalarType::Bool
            | ScalarType::Ref
            | ScalarType::Interface => return Ok(None),
        };
        let Some(args) = self.intrinsic_args(frame, intrinsic, args)? else {
            return Ok(None);
        };
        let capacity = u32::from(DEFAULT_STRING_MAX_LENGTH);
        StrExpr::call(intrinsic, args, capacity, expr.span()).map(Some)
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
    ) -> Lowered {
        let Some((target, operated)) = self.operated_type(expr)? else {
            return Ok(None);
        };
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
                match self.program.types().narrowed_storage(target) {
                    Some(NarrowWidth::B8) => bits_8,
                    Some(NarrowWidth::B16) => bits_16,
                    None => bits_32,
                    Some(NarrowWidth::B32) => return Ok(None),
                }
            }
            ScalarType::I64(Signedness::Signed) => Intrinsic::StringToI64(parse),
            ScalarType::I64(Signedness::Unsigned) => Intrinsic::StringToU64(parse),
            ScalarType::F32 => Intrinsic::StringToF32(parse),
            ScalarType::F64 => Intrinsic::StringToF64(parse),
            ScalarType::Bool | ScalarType::Ref | ScalarType::Interface => return Ok(None),
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
    ) -> Lowered {
        let intrinsic = match encoding {
            Encoding::Narrow => Intrinsic::CompareString,
            Encoding::Wide => Intrinsic::CompareWString,
        };
        let Some(args) = self.intrinsic_args(frame, intrinsic, &[left, right])? else {
            return Ok(None);
        };
        let order = Expr::call(intrinsic, args, span.clone())?;
        let zero = Expr::integer(0, order.ty(), span.clone())?;
        Expr::compare(op, order, zero, span).map(Some)
    }

    /// The encoding of the string type the analyzer recorded for `expr`;
    /// `None` when it recorded no string type.
    pub(crate) fn string_encoding(
        &mut self,
        expr: &AstExpr,
    ) -> Result<Option<Encoding>, Diagnostic> {
        let Some(ty) = self.recorded_type(expr)? else {
            return Ok(None);
        };
        let Some(TypeKind::String(shape)) = self.program.types().kind(ty) else {
            return Ok(None);
        };
        Ok(Some(shape.encoding))
    }

    /// The intrinsic of the string function `function` for the encoding the
    /// analyzer recorded for its first argument; `None` when it recorded no
    /// string type.
    fn string_intrinsic(
        &mut self,
        function: StringFunction,
        args: &[&AstExpr],
    ) -> Result<Option<Intrinsic>, Diagnostic> {
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
        let Some(first) = args.first() else {
            return Ok(None);
        };
        let Some(encoding) = self.string_encoding(first)? else {
            return Ok(None);
        };
        Ok(Some(match encoding {
            Encoding::Narrow => narrow,
            Encoding::Wide => wide,
        }))
    }

    /// The string `expr` makes; `None` for one lowering does not support
    /// yet, such as a string conversion.
    pub(crate) fn str_expr(
        &mut self,
        frame: &BodyFrame,
        expr: &AstExpr,
    ) -> Result<Option<StrExpr>, Diagnostic> {
        let span = expr.span();
        match &expr.kind {
            AstExprKind::Expression(inner) => self.str_expr(frame, inner),
            AstExprKind::Const(constant) => match constant {
                ConstantKind::CharacterString(literal) => {
                    let encoding = match literal.width {
                        StringType::String => Encoding::Narrow,
                        StringType::WString => Encoding::Wide,
                    };
                    StrExpr::literal(literal.value.clone(), encoding, span).map(Some)
                }
                ConstantKind::IntegerLiteral(_)
                | ConstantKind::RealLiteral(_)
                | ConstantKind::Boolean(_)
                | ConstantKind::Duration(_)
                | ConstantKind::TimeOfDay(_)
                | ConstantKind::Date(_)
                | ConstantKind::DateAndTime(_)
                | ConstantKind::BitStringLiteral(_) => Ok(None),
            },
            AstExprKind::Variable(variable) => {
                let Some(place) = self.variable_place(frame, variable)? else {
                    return Ok(None);
                };
                self.program.read_str(place, span).map(Some)
            }
            AstExprKind::Function(func) => self.string_call(frame, expr, func),
            // The string output of the call whose outputs are being stored.
            AstExprKind::CallOutput(output) => {
                let Some(place) = self.call_output_place(output)? else {
                    return Ok(None);
                };
                self.program.read_str(place, span).map(Some)
            }
            // A string a reference refers to and a method's string result
            // are not lowered yet.
            AstExprKind::Deref(_)
            | AstExprKind::MethodCall(_)
            | AstExprKind::Compare(_)
            | AstExprKind::BinaryOp(_)
            | AstExprKind::UnaryOp(_)
            | AstExprKind::EnumeratedValue(_)
            | AstExprKind::LateBound(_)
            | AstExprKind::Ref(_)
            | AstExprKind::ImplicitConversion(_)
            | AstExprKind::Null(_) => Ok(None),
        }
    }
}
