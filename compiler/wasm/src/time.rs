//! The typed time and date functions (`ADD_TIME`, `SUB_DT_DT`, `MUL_TIME`,
//! ..., IEC 61131-3 Table 30) and the operators the analyzer resolves to
//! them, in the units of the bytecode target: milliseconds for `TIME` and
//! `TIME_OF_DAY`, seconds for `DATE` and `DATE_AND_TIME` (ADR-0021,
//! ADR-0025).

use ironplc_analyzer::{typed_overload, Overload};
use ironplc_dsl::core::SourceSpan;
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_dsl::textual::{Expr as AstExpr, Operator};
use ironplc_wasm_ir::{BinaryOp, Const, Expr, ExprKind, Scalar};

use crate::body::{ex, Body};
use crate::expr::convert;
use crate::types::{is_int, F32, F64, I32, I64};

/// How a typed function computes.
#[derive(Clone, Copy)]
enum Arith {
    /// Both operands in the same unit.
    SameUnit(BinaryOp),
    /// Seconds and milliseconds: the milliseconds divided by 1000.
    SecondsAndMillis(BinaryOp),
    /// The difference of two values in seconds, as milliseconds.
    SecondsDifference,
    /// A duration scaled by a number.
    Scale(BinaryOp),
}

/// The computation of a typed function and its operation scalar.
fn arith(name: &str) -> Option<(Arith, Scalar)> {
    use Arith::*;
    use BinaryOp::{Add, Div, Mul, Sub};
    Some(match name {
        "ADD_TIME" | "ADD_TOD_TIME" => (SameUnit(Add), I32),
        "ADD_LTIME" | "ADD_LTOD_LTIME" => (SameUnit(Add), I64),
        "SUB_TIME" | "SUB_TOD_TIME" | "SUB_TOD_TOD" => (SameUnit(Sub), I32),
        "SUB_LTIME" | "SUB_LTOD_LTIME" | "SUB_LTOD_LTOD" => (SameUnit(Sub), I64),
        "ADD_DT_TIME" | "CONCAT_DATE_TOD" => (SecondsAndMillis(Add), I32),
        "ADD_LDT_LTIME" => (SecondsAndMillis(Add), I64),
        "SUB_DT_TIME" => (SecondsAndMillis(Sub), I32),
        "SUB_LDT_LTIME" => (SecondsAndMillis(Sub), I64),
        "SUB_DT_DT" | "SUB_DATE_DATE" => (SecondsDifference, I32),
        "SUB_LDT_LDT" | "SUB_LDATE_LDATE" => (SecondsDifference, I64),
        "MUL_TIME" => (Scale(Mul), I32),
        "MUL_LTIME" => (Scale(Mul), I64),
        "DIV_TIME" => (Scale(Div), I32),
        "DIV_LTIME" => (Scale(Div), I64),
        _ => return None,
    })
}

fn bin(op: BinaryOp, a: Expr, b: Expr, t: Scalar, site: Option<u32>) -> Expr {
    ex(ExprKind::Binary(op, Box::new(a), Box::new(b), site), t)
}

fn int(v: i128, t: Scalar) -> Expr {
    ex(ExprKind::Const(Const::Int(v)), t)
}

impl Body<'_, '_> {
    /// The typed function an operator resolves to, when its operands are
    /// times or dates.
    pub fn typed_operator(
        &mut self,
        op: &Operator,
        left: &AstExpr,
        right: &AstExpr,
        span: &SourceSpan,
    ) -> Result<Option<Expr>, Diagnostic> {
        let (Some(l), Some(r)) = (self.operand_name(left), self.operand_name(right)) else {
            return Ok(None);
        };
        match typed_overload(op, &l, &r) {
            Some(Overload::Typed { name, .. }) => self.typed_function(name, left, right, span),
            _ => Ok(None),
        }
    }

    /// A typed time or date function of that upper-case name.
    pub fn typed_function(
        &mut self,
        name: &str,
        left: &AstExpr,
        right: &AstExpr,
        span: &SourceSpan,
    ) -> Result<Option<Expr>, Diagnostic> {
        let Some((arith, w)) = arith(name) else {
            return Ok(None);
        };
        let r = match arith {
            Arith::SameUnit(op) => {
                let a = self.expr(left, Some(w))?;
                let b = self.expr(right, Some(w))?;
                bin(op, a, b, w, None)
            }
            Arith::SecondsAndMillis(op) => {
                let a = self.expr(left, Some(w))?;
                let b = self.expr(right, Some(w))?;
                let s = bin(BinaryOp::Div, b, int(1000, w), w, None);
                bin(op, a, s, w, None)
            }
            Arith::SecondsDifference => {
                let a = self.expr(left, Some(w))?;
                let b = self.expr(right, Some(w))?;
                let d = bin(BinaryOp::Sub, a, b, w, None);
                bin(BinaryOp::Mul, d, int(1000, w), w, None)
            }
            Arith::Scale(op) => {
                let n = self.natural(right);
                if is_int(n) && (w == I64 || n.size() <= 4) {
                    let a = self.expr(left, Some(w))?;
                    let b = self.expr(right, Some(w))?;
                    let site = (op == BinaryOp::Div).then(|| self.l.site(span));
                    bin(op, a, b, w, site)
                } else {
                    // A real factor, or a 64-bit one for a TIME: computed
                    // in a real and truncated back, as the bytecode does.
                    let real = if n == F32 && w == I32 { F32 } else { F64 };
                    let a = convert(self.expr(left, Some(w))?, real);
                    let b = self.expr(right, Some(real))?;
                    convert(bin(op, a, b, real, None), w)
                }
            }
        };
        Ok(Some(r))
    }
}
