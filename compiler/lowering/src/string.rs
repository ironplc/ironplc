//! Strings: a string literal, the string at a place, and the assignment of a
//! string to a string place.
//!
//! A literal is in the encoding its delimiter spells: `'abc'` is a `STRING`
//! and `"abc"` a `WSTRING` (ADR-0016). The analyzer reports a literal stored
//! at a string of the other encoding (P4035), so the assignment refuses one
//! as a broken invariant (REQ-LOW-lowering-050).

use ironplc_dsl::common::{ConstantKind, StringType};
use ironplc_dsl::core::{Located, SourceSpan};
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_dsl::textual::{Expr as AstExpr, ExprKind as AstExprKind};
use ironplc_ir::lowered::{Encoding, Place, Stmt, StrExpr};

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

    /// The string `expr` makes; `None` for one lowering does not support
    /// yet, such as a call of a string function.
    fn str_expr(
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
