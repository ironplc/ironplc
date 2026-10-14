//! Strings: a string literal, the string at a place, the assignment of a
//! string to a string place, and calls of the string functions that yield a
//! number.
//!
//! A literal is in the encoding its delimiter spells: `'abc'` is a `STRING`
//! and `"abc"` a `WSTRING` (ADR-0016). The analyzer reports a literal stored
//! at a string of the other encoding (P4035), so the assignment refuses one
//! as a broken invariant (REQ-LOW-lowering-050).
//!
//! A string function's strings share an encoding, which the analyzer checks
//! (P4034), so the encoding of its first string chooses its `Intrinsic`.

use ironplc_analyzer::StringFunction;
use ironplc_dsl::common::{ConstantKind, StringType};
use ironplc_dsl::core::{Located, SourceSpan};
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_dsl::textual::{Expr as AstExpr, ExprKind as AstExprKind};
use ironplc_ir::lowered::{Encoding, Intrinsic, Place, Stmt, StrExpr, TypeKind};

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

    /// The call `expr` of the string function `function` with `args`. `LEN`
    /// and `FIND` yield an `INT`; the functions that yield a string are not
    /// lowered yet.
    pub(crate) fn string_function_call(
        &mut self,
        frame: &BodyFrame,
        expr: &AstExpr,
        function: StringFunction,
        args: &[&AstExpr],
    ) -> Lowered {
        let [narrow, wide] = match function {
            StringFunction::Len => [Intrinsic::LenString, Intrinsic::LenWString],
            StringFunction::Find => [Intrinsic::FindString, Intrinsic::FindWString],
            StringFunction::Replace
            | StringFunction::Insert
            | StringFunction::Delete
            | StringFunction::Left
            | StringFunction::Right
            | StringFunction::Mid
            | StringFunction::Concat => return Ok(None),
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
        let intrinsic = match shape.encoding {
            Encoding::Narrow => narrow,
            Encoding::Wide => wide,
        };
        self.intrinsic_call(frame, expr, intrinsic, args)
    }

    /// The string `expr` makes; `None` for one lowering does not support
    /// yet, such as a call of a string function.
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
            // A call of a string function, a string a reference refers to,
            // and a method's string result are not lowered yet.
            AstExprKind::Function(_)
            | AstExprKind::Deref(_)
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
