//! Strings: zero-terminated in memory (ABI-044), Latin-1 for `STRING` and
//! UTF-16 code units for `WSTRING` (ADR-0016). A string computed by a
//! function lives in a static object of its call site.

use ironplc_container::DEFAULT_STRING_MAX_LENGTH;
use ironplc_dsl::common::{ConstantKind, StringType};
use ironplc_dsl::core::Located;
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_dsl::textual::{CompareOp, Expr as AstExpr, ExprKind as Ast};
use ironplc_wasm_ir::{
    Addr, BinaryOp, Const, Expr, ExprKind, Intrinsic, Region, Stmt, StmtKind, StrOp,
};

use crate::body::{ex, nyi, stmt, Body};
use crate::expr::{convert, widen};
use crate::types::{is_int, Ty, BOOL, I16, I32};

/// Code units of a literal: one byte per character for `STRING`, two
/// little-endian bytes for `WSTRING`.
pub(crate) fn encode(chars: &[char], wide: bool) -> Vec<u8> {
    if wide {
        chars
            .iter()
            .flat_map(|c| (*c as u32 as u16).to_le_bytes())
            .collect()
    } else {
        chars.iter().map(|c| *c as u32 as u8).collect()
    }
}

fn i32c(v: i128) -> Expr {
    ex(ExprKind::Const(Const::Int(v)), I32)
}

impl Body<'_, '_> {
    /// The object holding a string literal.
    fn literal(&mut self, chars: &[char], wide: bool) -> Addr {
        let mut bytes = encode(chars, wide);
        bytes.extend(if wide { vec![0, 0] } else { vec![0] });
        let key = (bytes.clone(), wide);
        if let Some(o) = self.l.literals.get(&key) {
            return Addr::object(*o, 0);
        }
        let o = self
            .l
            .object("LITERAL", Region::Static, bytes, if wide { 2 } else { 1 });
        self.l.literals.insert(key, o);
        Addr::object(o, 0)
    }

    /// A static object for a string of at most `cap` characters.
    fn scratch(&mut self, cap: u32, wide: bool) -> Addr {
        let unit = if wide { 2 } else { 1 };
        let o = self.l.object(
            "STRING",
            Region::Static,
            vec![0; ((cap + 1) * unit) as usize],
            unit,
        );
        Addr::object(o, 0)
    }

    /// The address of a string value of kind `wide`, converting between
    /// the kinds; statements that compute it are appended to `pre`.
    pub fn string_operand(
        &mut self,
        e: &AstExpr,
        wide: bool,
        pre: &mut Vec<Stmt>,
    ) -> Result<Addr, Diagnostic> {
        let (addr, w, cap) = self.string_value(e, pre)?;
        if w == wide {
            return Ok(addr);
        }
        let dst = self.scratch(cap, wide);
        pre.push(stmt(StmtKind::Str(
            Box::new(StrOp::Convert {
                dst: dst.clone(),
                cap,
                src: addr,
            }),
            wide,
        )));
        Ok(dst)
    }

    /// Address, kind and capacity of a string expression.
    fn string_value(
        &mut self,
        e: &AstExpr,
        pre: &mut Vec<Stmt>,
    ) -> Result<(Addr, bool, u32), Diagnostic> {
        match &e.kind {
            Ast::Const(ConstantKind::CharacterString(s)) => {
                let wide = s.width == StringType::WString;
                Ok((self.literal(&s.value, wide), wide, s.value.len() as u32))
            }
            Ast::Variable(v) => {
                let (addr, ty) = self.place(v)?;
                match ty {
                    Ty::Str { cap, wide } => Ok((addr, wide, cap)),
                    _ => Err(nyi(&e.span(), "A string of this variable")),
                }
            }
            Ast::Expression(inner) => self.string_value(inner, pre),
            Ast::Function(f) => {
                if let Some(ft) = self.l.func_type(&f.name.to_string())? {
                    let (call, result) =
                        self.user_call(&ft, &f.param_assignment, &e.span(), pre)?;
                    let Some((off, Ty::Str { cap, wide })) = result else {
                        return Err(nyi(&e.span(), "This function in a string context"));
                    };
                    pre.push(stmt(StmtKind::Call(Box::new(call))));
                    // Copied out of the frame, so that a second call of the
                    // same function in the expression does not overwrite it.
                    let dst = self.scratch(cap, wide);
                    pre.push(stmt(StmtKind::Str(
                        Box::new(StrOp::Assign {
                            dst: dst.clone(),
                            cap,
                            src: Addr::object(ft.frame, off),
                        }),
                        wide,
                    )));
                    return Ok((dst, wide, cap));
                }
                self.string_call(&f.name.to_string().to_uppercase(), f, e, pre)
            }
            _ => Err(nyi(&e.span(), "This string expression")),
        }
    }

    /// `dst := value` for a string destination.
    pub fn assign_string(
        &mut self,
        dst: Addr,
        cap: u32,
        wide: bool,
        value: &AstExpr,
        out: &mut Vec<Stmt>,
    ) -> Result<(), Diagnostic> {
        let src = self.string_operand(value, wide, out)?;
        out.push(stmt(StmtKind::Str(
            Box::new(StrOp::Assign { dst, cap, src }),
            wide,
        )));
        Ok(())
    }

    /// A comparison of two strings by code.
    pub fn string_compare(
        &mut self,
        op: &CompareOp,
        left: &AstExpr,
        right: &AstExpr,
    ) -> Result<Expr, Diagnostic> {
        let bop = match op {
            CompareOp::Eq => BinaryOp::Eq,
            CompareOp::Ne => BinaryOp::Ne,
            CompareOp::Lt => BinaryOp::Lt,
            CompareOp::Gt => BinaryOp::Gt,
            CompareOp::LtEq => BinaryOp::Le,
            CompareOp::GtEq => BinaryOp::Ge,
            _ => return Err(nyi(&left.span(), "This operator on strings")),
        };
        let mut pre = vec![];
        let (a, wide, _) = self.string_value(left, &mut pre)?;
        let b = self.string_operand(right, wide, &mut pre)?;
        let cmp = ex(ExprKind::StrCmp(a, b, wide), I32);
        let r = ex(
            ExprKind::Binary(bop, Box::new(cmp), Box::new(i32c(0)), None),
            BOOL,
        );
        Ok(seq(pre, r))
    }

    /// The string functions whose result is not a string.
    pub fn string_function(
        &mut self,
        name: &str,
        args: &[&AstExpr],
        e: &AstExpr,
    ) -> Result<Option<Expr>, Diagnostic> {
        let mut pre = vec![];
        let r = match (name, args) {
            ("LEN", [s]) if self.is_string(s) => {
                let (a, wide, _) = self.string_value(s, &mut pre)?;
                widen(ex(ExprKind::StrLen(a, wide), I16), I32)
            }
            ("FIND", [s, t]) if self.is_string(s) => {
                let (a, wide, _) = self.string_value(s, &mut pre)?;
                let b = self.string_operand(t, wide, &mut pre)?;
                widen(ex(ExprKind::StrFind(a, b, wide), I16), I32)
            }
            _ => {
                if !self.is_string(e) {
                    return Ok(None);
                }
                return Err(nyi(&e.span(), "A string function in this context"));
            }
        };
        Ok(Some(seq(pre, r)))
    }

    /// A string function with a string result, computed into a static
    /// object of the call site.
    fn string_call(
        &mut self,
        name: &str,
        f: &ironplc_dsl::textual::Function,
        e: &AstExpr,
        pre: &mut Vec<Stmt>,
    ) -> Result<(Addr, bool, u32), Diagnostic> {
        let args: Vec<&AstExpr> = f
            .param_assignment
            .iter()
            .filter_map(|p| p.input_expr())
            .collect();
        let wide = match self.resolved(e) {
            Some(Ty::Str { wide, .. }) => wide,
            _ => false,
        };
        let cap = match self.resolved(e) {
            Some(Ty::Str { cap, .. }) => cap,
            _ => DEFAULT_STRING_MAX_LENGTH as u32,
        };
        let dst = self.scratch(cap, wide);
        let all = || i32c(i32::MAX as i128);
        let op = |b: &mut Self, arg: &AstExpr, pre: &mut Vec<Stmt>| -> Result<Expr, Diagnostic> {
            let t = b.natural(arg);
            let t = if is_int(t) { t } else { I32 };
            let v = b.expr(arg, Some(t))?;
            let _ = pre;
            Ok(convert(v, I32))
        };
        let push = |pre: &mut Vec<Stmt>, s: StrOp| pre.push(stmt(StmtKind::Str(Box::new(s), wide)));
        match (name, args.as_slice()) {
            ("CONCAT", parts) if !parts.is_empty() => {
                let mut srcs = vec![];
                for p in parts {
                    srcs.push(self.string_operand(p, wide, pre)?);
                }
                push(pre, StrOp::Clear(dst.clone()));
                for src in srcs {
                    push(
                        pre,
                        StrOp::Append {
                            dst: dst.clone(),
                            cap,
                            src,
                            start: i32c(0),
                            count: all(),
                        },
                    );
                }
            }
            ("LEFT" | "RIGHT" | "MID", [s, rest @ ..]) => {
                let src = self.string_operand(s, wide, pre)?;
                let (start, count) = match (name, rest) {
                    ("LEFT", [n]) => (i32c(0), op(self, n, pre)?),
                    ("RIGHT", [n]) => {
                        let n = op(self, n, pre)?;
                        let len = widen(ex(ExprKind::StrLen(src.clone(), wide), I16), I32);
                        let start = ex(
                            ExprKind::Binary(
                                BinaryOp::Sub,
                                Box::new(len),
                                Box::new(n.clone()),
                                None,
                            ),
                            I32,
                        );
                        let start = ex(
                            ExprKind::Intrinsic(Intrinsic::Max, vec![start, i32c(0)]),
                            I32,
                        );
                        (start, n)
                    }
                    ("MID", [l, p]) => {
                        let l = op(self, l, pre)?;
                        let p = op(self, p, pre)?;
                        let start = ex(
                            ExprKind::Binary(BinaryOp::Sub, Box::new(p), Box::new(i32c(1)), None),
                            I32,
                        );
                        (start, l)
                    }
                    _ => return Err(nyi(&e.span(), "This call")),
                };
                push(pre, StrOp::Clear(dst.clone()));
                push(
                    pre,
                    StrOp::Append {
                        dst: dst.clone(),
                        cap,
                        src,
                        start,
                        count,
                    },
                );
            }
            ("INSERT", [a, b, p]) => {
                let src = self.string_operand(a, wide, pre)?;
                let ins = self.string_operand(b, wide, pre)?;
                let p = op(self, p, pre)?;
                let pos = ex(
                    ExprKind::Binary(BinaryOp::Add, Box::new(p), Box::new(i32c(1)), None),
                    I32,
                );
                push(
                    pre,
                    StrOp::Splice {
                        dst: dst.clone(),
                        cap,
                        src,
                        pos,
                        len: i32c(0),
                        ins: Some(ins),
                    },
                );
            }
            ("DELETE", [a, l, p]) => {
                let src = self.string_operand(a, wide, pre)?;
                let len = op(self, l, pre)?;
                let pos = op(self, p, pre)?;
                push(
                    pre,
                    StrOp::Splice {
                        dst: dst.clone(),
                        cap,
                        src,
                        pos,
                        len,
                        ins: None,
                    },
                );
            }
            ("REPLACE", [a, b, l, p]) => {
                let src = self.string_operand(a, wide, pre)?;
                let ins = self.string_operand(b, wide, pre)?;
                let len = op(self, l, pre)?;
                let pos = op(self, p, pre)?;
                push(
                    pre,
                    StrOp::Splice {
                        dst: dst.clone(),
                        cap,
                        src,
                        pos,
                        len,
                        ins: Some(ins),
                    },
                );
            }
            _ => return Err(nyi(&e.span(), &format!("The function {name}"))),
        }
        Ok((dst, wide, cap))
    }
}

/// `pre` then `e`, or `e` alone.
pub(crate) fn seq(pre: Vec<Stmt>, e: Expr) -> Expr {
    if pre.is_empty() {
        e
    } else {
        let t = e.ty;
        ex(ExprKind::Seq(pre, Box::new(e)), t)
    }
}
