//! Standard functions as IR operators, conversions and intrinsics.

use ironplc_dsl::core::Located;
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_dsl::textual::{Expr as AstExpr, Function, ParamAssignmentKind};
use ironplc_wasm_ir::{BinaryOp, Expr, ExprKind, Intrinsic, Scalar};

use crate::body::{ex, nyi, Body};
use crate::expr::{convert, widen};
use crate::types::{elementary, is_int, op_scalar, BOOL, F64, I32};

fn args(f: &Function) -> Vec<&AstExpr> {
    f.param_assignment
        .iter()
        .filter_map(ParamAssignmentKind::input_expr)
        .collect()
}

fn fold(op: BinaryOp, mut values: Vec<Expr>, ty: Scalar) -> Expr {
    let first = values.remove(0);
    values.into_iter().fold(first, |a, b| {
        ex(ExprKind::Binary(op, Box::new(a), Box::new(b), None), ty)
    })
}

/// The storage scalar of an elementary type name.
fn named_scalar(b: &Body, name: &str) -> Option<Scalar> {
    let tn = ironplc_dsl::common::TypeName::from(name);
    let it = b.l.ctx.types().get(&tn)?;
    elementary(&it.representation)?.sc()
}

impl Body<'_, '_> {
    pub fn std_function(
        &mut self,
        f: &Function,
        want: Option<Scalar>,
        e: &AstExpr,
    ) -> Result<Expr, Diagnostic> {
        let name = f.name.to_string().to_uppercase();
        let a = args(f);
        let span = e.span();
        if let Some(r) = self.string_function(&name, &a, e)? {
            return Ok(r);
        }
        if let [x, y] = a.as_slice() {
            if let Some(r) = self.typed_function(&name, x, y, &span)? {
                return Ok(r);
            }
        }
        if let Some((from, to)) = name.split_once("_TO_") {
            return self.conversion(from, to, &a, &span);
        }
        let t = want
            .filter(|w| *w != BOOL)
            .or_else(|| self.concrete(e))
            .unwrap_or_else(|| self.natural(e));
        let n = a.len();
        let values =
            |b: &mut Self, list: &[&AstExpr], t: Scalar| -> Result<Vec<Expr>, Diagnostic> {
                list.iter().map(|x| b.expr(x, Some(t))).collect()
            };
        let intrinsic = |i, v| ex(ExprKind::Intrinsic(i, v), t);
        let r = match (name.as_str(), n) {
            ("ABS", 1) => intrinsic(Intrinsic::Abs, values(self, &a, t)?),
            ("MAX", 2..) => intrinsic(Intrinsic::Max, values(self, &a, t)?),
            ("MIN", 2..) => intrinsic(Intrinsic::Min, values(self, &a, t)?),
            ("LIMIT", 3) => intrinsic(Intrinsic::Limit, values(self, &a, t)?),
            ("MOVE", 1) => self.expr(a[0], Some(t))?,
            ("SIZEOF", 1) => {
                // Bytes of the value: its storage in this target, which
                // for elementary values and their arrays is the size the
                // bytecode reports.
                let ty = match &a[0].kind {
                    ironplc_dsl::textual::ExprKind::Variable(v) => Some(self.place(v)?.1),
                    _ => self.resolved(a[0]),
                };
                let ty = ty.ok_or_else(|| nyi(&span, "SIZEOF of this value"))?;
                let (size, _) = self.l.size_align(&ty);
                convert(
                    ex(
                        ExprKind::Const(ironplc_wasm_ir::Const::Int(size as i128)),
                        I32,
                    ),
                    t,
                )
            }
            ("NOT", 1) => {
                let t = self.concrete(a[0]).unwrap_or(t);
                let v = self.expr(a[0], Some(t))?;
                ex(ExprKind::Unary(ironplc_wasm_ir::UnOp::Not, Box::new(v)), t)
            }
            ("SEL", 3) => {
                let g = self.expr(a[0], Some(BOOL))?;
                let in0 = self.expr(a[1], Some(t))?;
                let in1 = self.expr(a[2], Some(t))?;
                ex(
                    ExprKind::Select(Box::new(g), Box::new(in1), Box::new(in0)),
                    t,
                )
            }
            ("MUX", 2..) => {
                let kt = self.natural(a[0]);
                let kt = if is_int(kt) { kt } else { I32 };
                let mut v = vec![self.expr(a[0], Some(kt))?];
                v.extend(values(self, &a[1..], t)?);
                intrinsic(Intrinsic::Mux, v)
            }
            ("ADD", 2..) => fold(BinaryOp::Add, values(self, &a, t)?, t),
            ("MUL", 2..) => fold(BinaryOp::Mul, values(self, &a, t)?, t),
            ("SUB", 2) => fold(BinaryOp::Sub, values(self, &a, t)?, t),
            ("DIV" | "MOD", 2) => {
                let op = if name == "DIV" {
                    BinaryOp::Div
                } else {
                    BinaryOp::Mod
                };
                let [x, y] = <[Expr; 2]>::try_from(values(self, &a, t)?)
                    .map_err(|_| nyi(&span, "This call"))?;
                let site = is_int(t).then(|| self.l.site(&span));
                ex(ExprKind::Binary(op, Box::new(x), Box::new(y), site), t)
            }
            // An integer EXPT is an integer power that wraps, as the VM's
            // EXPT_I32/EXPT_I64; a real one is a real power.
            ("EXPT", 2) => self.power(a[0], a[1], t, &span)?,
            ("SQRT" | "LN" | "LOG" | "EXP", 1) => {
                let t = if t.is_real() { t } else { F64 };
                let i = match name.as_str() {
                    "SQRT" => Intrinsic::Sqrt,
                    "LN" => Intrinsic::Ln,
                    "LOG" => Intrinsic::Log,
                    _ => Intrinsic::Exp,
                };
                ex(ExprKind::Intrinsic(i, values(self, &a, t)?), t)
            }
            ("SHL" | "SHR" | "ROL" | "ROR", 2) => return self.shift(&name, a[0], a[1], t),
            ("AND" | "OR" | "XOR", 2..) => {
                let op = match name.as_str() {
                    "AND" => BinaryOp::And,
                    "OR" => BinaryOp::Or,
                    _ => BinaryOp::Xor,
                };
                let t = self.concrete(a[0]).unwrap_or(t);
                fold(op, values(self, &a, t)?, t)
            }
            ("GT" | "GE" | "EQ" | "LE" | "LT" | "NE", 2..) => {
                let op = match name.as_str() {
                    "GT" => BinaryOp::Gt,
                    "GE" => BinaryOp::Ge,
                    "EQ" => BinaryOp::Eq,
                    "LE" => BinaryOp::Le,
                    "LT" => BinaryOp::Lt,
                    _ => BinaryOp::Ne,
                };
                let ot = a
                    .iter()
                    .find_map(|x| self.concrete(x))
                    .unwrap_or_else(|| self.natural(a[0]));
                let v = values(self, &a, ot)?;
                match <[Expr; 2]>::try_from(v) {
                    Ok([x, y]) => ex(ExprKind::Binary(op, Box::new(x), Box::new(y), None), BOOL),
                    Err(v) => ex(ExprKind::Intrinsic(Intrinsic::Compare(op), v), BOOL),
                }
            }
            ("TRUNC", 1) => {
                let src = self.natural(a[0]);
                let v = self.expr(a[0], Some(src))?;
                let to = if is_int(t) { t } else { crate::types::I32 };
                convert(v, to)
            }
            _ => return Err(nyi(&span, &format!("The function {name}"))),
        };
        Ok(r)
    }

    /// `SHL`, `SHR`, `ROL`, `ROR`. A rotation of a narrow value turns within
    /// the width of its type, as the bytecode's narrow rotations do.
    fn shift(
        &mut self,
        name: &str,
        value: &AstExpr,
        n: &AstExpr,
        t: Scalar,
    ) -> Result<Expr, Diagnostic> {
        let storage = self.resolved(value).and_then(|ty| ty.sc()).unwrap_or(t);
        let rotate = matches!(name, "ROL" | "ROR");
        let work = if rotate && storage.size() < 4 {
            Scalar::Int {
                bits: (storage.size() * 8) as u8,
                signed: false,
            }
        } else {
            op_scalar(storage)
        };
        let v = self.expr(value, Some(op_scalar(storage)))?;
        let v = convert(v, work);
        let nt = self.natural(n);
        let nt = if is_int(nt) { nt } else { I32 };
        let count = self.expr(n, Some(nt))?;
        let i = match name {
            "SHL" => Intrinsic::Shl,
            "SHR" => Intrinsic::Shr,
            "ROL" => Intrinsic::Rol,
            _ => Intrinsic::Ror,
        };
        let r = ex(ExprKind::Intrinsic(i, vec![v, count]), work);
        Ok(widen(r, t))
    }

    /// `A_TO_B`: the value stored in `B`, then computed at its operation
    /// width, as the bytecode converts and truncates.
    fn conversion(
        &mut self,
        from: &str,
        to: &str,
        a: &[&AstExpr],
        span: &ironplc_dsl::core::SourceSpan,
    ) -> Result<Expr, Diagnostic> {
        if a.len() != 1 {
            return Err(nyi(span, "This conversion"));
        }
        let (Some(src), Some(dst)) = (named_scalar(self, from), named_scalar(self, to)) else {
            return Err(nyi(span, &format!("The conversion {from}_TO_{to}")));
        };
        let special = match (from, to) {
            ("DT" | "DATE_AND_TIME", "DATE") => Some(true),
            ("DT" | "DATE_AND_TIME", "TOD" | "TIME_OF_DAY") => Some(false),
            _ => None,
        };
        let v = self.expr(a[0], Some(op_scalar(src)))?;
        if let Some(date) = special {
            return Ok(self.date_part(v, date));
        }
        Ok(widen(convert(v, dst), op_scalar(dst)))
    }

    /// The date (seconds at midnight) or the time of day (milliseconds) of a
    /// date and time in seconds.
    fn date_part(&mut self, v: Expr, date: bool) -> Expr {
        let t = v.ty;
        let day = ex(ExprKind::Const(ironplc_wasm_ir::Const::Int(86_400)), t);
        let site = None;
        let rem = ex(
            ExprKind::Binary(BinaryOp::Mod, Box::new(v.clone()), Box::new(day), site),
            t,
        );
        if date {
            ex(
                ExprKind::Binary(BinaryOp::Sub, Box::new(v), Box::new(rem), None),
                t,
            )
        } else {
            let ms = ex(ExprKind::Const(ironplc_wasm_ir::Const::Int(1000)), t);
            ex(
                ExprKind::Binary(BinaryOp::Mul, Box::new(rem), Box::new(ms), None),
                t,
            )
        }
    }
}
