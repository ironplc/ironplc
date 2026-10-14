//! Strings: a string literal, the string at a place, the assignment of a
//! string to a string place, and calls of the standard string functions.
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

use ironplc_analyzer::{BuiltinFunction, StringFunction};
use ironplc_dsl::common::{ConstantKind, StringType};
use ironplc_dsl::core::{Located, SourceSpan};
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_dsl::textual::{Expr as AstExpr, ExprKind as AstExprKind, Function};
use ironplc_ir::lowered::{Arg, Encoding, Intrinsic, Param, Place, Stmt, StrExpr, TypeKind};

use crate::expr::Lowered;
use crate::variables::BodyFrame;
use crate::Lowering;

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

    /// The string the call `func`, the expression `expr`, yields, when it is
    /// a call of a standard string function that yields a string. Its
    /// capacity is the sum of its strings' (REQ-LOW-lowering-167): room for
    /// every code unit `CONCAT`, `INSERT` and `REPLACE` join, and for all
    /// `LEFT`, `RIGHT`, `MID` and `DELETE` can keep of their one string.
    fn string_call(
        &mut self,
        frame: &BodyFrame,
        expr: &AstExpr,
        func: &Function,
    ) -> Result<Option<StrExpr>, Diagnostic> {
        let builtin = self
            .analysis
            .context()
            .functions()
            .get(&func.name)
            .and_then(|signature| signature.builtin_function.as_ref());
        let Some(BuiltinFunction::String(function)) = builtin else {
            return Ok(None);
        };
        let function = *function;
        // The analyzer wrote the call in normal form: its inputs in the order
        // of the function's parameters.
        let args: Vec<&AstExpr> = func.inputs().collect();
        let Some(intrinsic) = self.string_intrinsic(function, &args)? else {
            return Ok(None);
        };
        let Param::Str(_) = intrinsic.signature().result() else {
            return Ok(None);
        };
        let Some(args) = self.intrinsic_args(frame, intrinsic, &args)? else {
            return Ok(None);
        };
        let capacity = args.iter().fold(0u32, |sum, arg| match arg {
            Arg::Str(value) => sum.saturating_add(value.shape().capacity),
            Arg::Value(_) => sum,
        });
        StrExpr::call(intrinsic, args, capacity, expr.span()).map(Some)
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
        let Some(ty) = self.recorded_type(first)? else {
            return Ok(None);
        };
        let Some(TypeKind::String(shape)) = self.program.types().kind(ty) else {
            return Ok(None);
        };
        Ok(Some(match shape.encoding {
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
