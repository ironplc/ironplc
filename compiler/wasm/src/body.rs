//! The body of a POU: statements.

use std::collections::HashMap;

use ironplc_dsl::common::SignedInteger;
use ironplc_dsl::core::{Located, SourceSpan};
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_dsl::textual::{
    Assignment, Case, CaseSelectionKind, For, If, Repeat, StmtKind as Ast, Variable, While,
};
use ironplc_wasm_ir::{
    BinaryOp, Const, Expr, ExprKind, Label as IrLabel, Scalar, Span, Stmt, StmtKind, UnOp,
};

use crate::lower::{Lowerer, Var};
use crate::types::{op_scalar, Ty, BOOL};

/// The state of the lowering of one body.
pub(crate) struct Body<'l, 'a> {
    pub l: &'l mut Lowerer<'a>,
    pub vars: HashMap<String, Var>,
    /// IR temporaries of the function.
    pub temps: Vec<Scalar>,
    /// Exit labels of the enclosing loops, innermost last.
    loops: Vec<IrLabel>,
}

pub(crate) fn nyi(span: &SourceSpan, what: &str) -> Diagnostic {
    Diagnostic::not_implemented(Label::span(
        span.clone(),
        format!("{what} in the WebAssembly target"),
    ))
}

pub(crate) fn stmt(kind: StmtKind) -> Stmt {
    Stmt {
        kind,
        span: Span::default(),
    }
}

pub(crate) fn ex(kind: ExprKind, ty: Scalar) -> Expr {
    Expr {
        kind,
        ty,
        span: Span::default(),
    }
}

pub(crate) fn not(e: Expr) -> Expr {
    ex(ExprKind::Unary(UnOp::Not, Box::new(e)), BOOL)
}

impl<'l, 'a> Body<'l, 'a> {
    pub fn new(l: &'l mut Lowerer<'a>, vars: HashMap<String, Var>) -> Self {
        Body {
            l,
            vars,
            temps: vec![],
            loops: vec![],
        }
    }

    pub fn temp(&mut self, sc: Scalar) -> u32 {
        self.temps.push(sc);
        (self.temps.len() - 1) as u32
    }

    pub fn stmts(&mut self, list: &[Ast]) -> Result<Vec<Stmt>, Diagnostic> {
        let mut out = vec![];
        for s in list {
            self.stmt(s, &mut out)?;
        }
        Ok(out)
    }

    fn stmt(&mut self, s: &Ast, out: &mut Vec<Stmt>) -> Result<(), Diagnostic> {
        if self.l.debug_hooks {
            // ABI-032: the site of the statement, before it runs.
            let span = match s {
                // An assignment is located at its `:=`; the statement runs
                // from its target to its value.
                Ast::Assignment(a) => SourceSpan {
                    end: a.value.span().end,
                    ..a.target.span()
                },
                other => other.span(),
            };
            let site = self.l.site(&span);
            out.push(stmt(StmtKind::DebugSite(site)));
        }
        match s {
            Ast::Assignment(a) => self.assignment(a, out),
            Ast::FbCall(c) => self.fb_call(c, out),
            Ast::If(i) => {
                let s = self.if_stmt(i)?;
                out.push(s);
                Ok(())
            }
            Ast::Case(c) => {
                let s = self.case(c)?;
                out.push(s);
                Ok(())
            }
            Ast::For(f) => self.for_loop(f, out),
            Ast::While(w) => self.while_loop(w, out),
            Ast::Repeat(r) => self.repeat(r, out),
            Ast::Return => {
                out.push(stmt(StmtKind::Return));
                Ok(())
            }
            Ast::Exit(span) => {
                let l = *self
                    .loops
                    .last()
                    .ok_or_else(|| nyi(span, "EXIT outside a loop"))?;
                out.push(stmt(StmtKind::Break(l)));
                Ok(())
            }
            Ast::MethodCall(m) => Err(nyi(&m.position, "A method call")),
        }
    }

    fn assignment(&mut self, a: &Assignment, out: &mut Vec<Stmt>) -> Result<(), Diagnostic> {
        use ironplc_dsl::textual::ExprKind as E;
        if a.ref_bind {
            // `r REF= x` is `r := REF(x)`.
            let target = match &a.value.kind {
                E::Variable(v) => v.clone(),
                E::Ref(v) => (**v).clone(),
                E::LateBound(lb) => Variable::named(&lb.value.to_string()),
                _ => return Err(nyi(&a.span, "REF= of this value")),
            };
            let (addr, _) = self.place(&a.target)?;
            let (target, _) = self.place(&target)?;
            let value = ex(ExprKind::AddrOf(target), crate::types::U32);
            out.push(self.store(addr, crate::types::U32, value));
            return Ok(());
        }
        if a.set_bind || a.reset_bind {
            // `x S= c` sets `x` when `c` is true, `x R= c` resets it.
            let c = self.expr(&a.value, Some(BOOL))?;
            let (addr, ty) = self.place(&a.target)?;
            let sc = ty.sc().unwrap_or(BOOL);
            let v = ex(ExprKind::Const(Const::Bool(a.set_bind)), BOOL);
            let set = self.store(addr, sc, v);
            out.push(stmt(StmtKind::If(c, vec![set], vec![])));
            return Ok(());
        }
        if a.deref {
            // `p^ := v`: the target of the reference `p`.
            let (slot, ty) = self.place(&a.target)?;
            let (addr, ty) = self.through(slot, ty, &a.span)?;
            let sc = ty
                .sc()
                .ok_or_else(|| nyi(&a.span, "A dereference of this type"))?;
            let v = self.expr(&a.value, Some(op_scalar(sc)))?;
            out.push(self.store(addr, sc, v));
            return Ok(());
        }
        self.assign(&a.target, &a.value, out)
    }

    /// `target := value`.
    pub fn assign(
        &mut self,
        target: &Variable,
        value: &ironplc_dsl::textual::Expr,
        out: &mut Vec<Stmt>,
    ) -> Result<(), Diagnostic> {
        if let Some(s) = crate::bits::slice(target) {
            let v = self.expr(value, Some(s.value_scalar()))?;
            return self.write_slice(&s, v, out);
        }
        let (addr, ty) = self.place(target)?;
        match &ty {
            Ty::Scalar { .. } | Ty::Ref(_) => {
                let sc = ty.sc().unwrap_or(crate::types::U32);
                let v = self.expr(value, Some(op_scalar(sc)))?;
                out.push(self.store(addr, sc, v));
                Ok(())
            }
            Ty::Str { cap, wide } => self.assign_string(addr, *cap, *wide, value, out),
            Ty::Array(_) | Ty::Struct(_) => {
                let (size, _) = self.l.size_align(&ty);
                let src = self.aggregate(value, &ty, out)?;
                out.push(stmt(StmtKind::Copy(ironplc_wasm_ir::Copy {
                    dst: addr,
                    src,
                    size,
                })));
                Ok(())
            }
            Ty::Fb(_) => Err(nyi(&target.span(), "An assignment of a function block")),
        }
    }

    /// Stores a value computed in the operation scalar of `sc`.
    pub fn store(&self, addr: ironplc_wasm_ir::Addr, sc: Scalar, v: Expr) -> Stmt {
        stmt(StmtKind::Assign(
            ironplc_wasm_ir::Place { addr, ty: sc },
            crate::expr::convert(v, sc),
        ))
    }

    fn if_stmt(&mut self, i: &If) -> Result<Stmt, Diagnostic> {
        let mut otherwise = self.stmts(&i.else_body)?;
        for e in i.else_ifs.iter().rev() {
            let c = self.expr(&e.expr, Some(BOOL))?;
            let body = self.stmts(&e.body)?;
            otherwise = vec![stmt(StmtKind::If(c, body, otherwise))];
        }
        let c = self.expr(&i.expr, Some(BOOL))?;
        let body = self.stmts(&i.body)?;
        Ok(stmt(StmtKind::If(c, body, otherwise)))
    }

    fn case(&mut self, c: &Case) -> Result<Stmt, Diagnostic> {
        let natural = self.natural(&c.selector);
        let sel_ty = if crate::types::is_int(natural) {
            natural
        } else {
            crate::types::I32
        };
        let sel = self.expr(&c.selector, Some(sel_ty))?;
        let mut arms = vec![];
        for g in &c.statement_groups {
            let mut labels = vec![];
            for s in &g.selectors {
                labels.push(self.case_label(s)?);
            }
            arms.push((labels, self.stmts(&g.statements)?));
        }
        let default = self.stmts(&c.else_body)?;
        Ok(stmt(StmtKind::Switch(sel, arms, default)))
    }

    fn case_label(&mut self, s: &CaseSelectionKind) -> Result<(i128, i128), Diagnostic> {
        let int = |i: &SignedInteger| {
            let v = i.value.value as i128;
            if i.is_neg {
                -v
            } else {
                v
            }
        };
        let signed_ref = |r: &ironplc_dsl::common::SignedIntegerRef| match r {
            ironplc_dsl::common::SignedIntegerRef::Literal(i) => Ok(int(i)),
            ironplc_dsl::common::SignedIntegerRef::Constant(id) => {
                Err(nyi(&id.span(), "A CASE label given by a constant"))
            }
        };
        match s {
            CaseSelectionKind::SignedInteger(i) => Ok((int(i), int(i))),
            CaseSelectionKind::Subrange(r) => Ok((signed_ref(&r.start)?, signed_ref(&r.end)?)),
            CaseSelectionKind::BitStringLiteral(b) => {
                Ok((b.value.value as i128, b.value.value as i128))
            }
            CaseSelectionKind::EnumeratedValue(v) => {
                let o = self
                    .l
                    .enums
                    .ordinal(v)
                    .ok_or_else(|| nyi(&v.span(), "This enumerated value"))?
                    as i128;
                Ok((o, o))
            }
        }
    }

    /// A loop: `Loop(l)` with `l` as the exit of `EXIT`.
    fn in_loop<T>(
        &mut self,
        f: impl FnOnce(&mut Self, IrLabel) -> Result<T, Diagnostic>,
    ) -> Result<T, Diagnostic> {
        let l = self.l.label();
        self.loops.push(l);
        let r = f(self, l);
        self.loops.pop();
        r
    }

    /// `FOR` as the bytecode runs it (REQ-WT-wasm-024): the end is evaluated
    /// before each iteration, the step is added with the wrap-around of the
    /// control variable.
    fn for_loop(&mut self, f: &For, out: &mut Vec<Stmt>) -> Result<(), Diagnostic> {
        let control = Variable::named(&f.control.to_string());
        let (addr, ty) = self.place(&control)?;
        let sc = ty
            .sc()
            .ok_or_else(|| nyi(&f.control.span(), "A FOR control variable of this type"))?;
        let op = op_scalar(sc);
        let negative = match &f.step {
            None => false,
            Some(s) => crate::expr::constant_sign(s)
                .ok_or_else(|| nyi(&f.control.span(), "A FOR step that is not a constant"))?,
        };
        let from = self.expr(&f.from, Some(op))?;
        out.push(self.store(addr.clone(), sc, from));
        let load = || {
            crate::expr::widen(
                ex(
                    ExprKind::Load(ironplc_wasm_ir::Place {
                        addr: addr.clone(),
                        ty: sc,
                    }),
                    sc,
                ),
                op,
            )
        };
        let body = self.in_loop(|b, l| {
            let end = b.expr(&f.to, Some(op))?;
            let cmp = if negative { BinaryOp::Ge } else { BinaryOp::Le };
            let test = ex(
                ExprKind::Binary(cmp, Box::new(load()), Box::new(end), None),
                BOOL,
            );
            let mut body = vec![stmt(StmtKind::If(
                not(test),
                vec![stmt(StmtKind::Break(l))],
                vec![],
            ))];
            // `CONTINUE` is not a statement of IronPLC, so the body falls
            // through to the step.
            body.extend(b.stmts(&f.body)?);
            let step = match &f.step {
                Some(s) => b.expr(s, Some(op))?,
                None => ex(ExprKind::Const(one(op)), op),
            };
            let next = ex(
                ExprKind::Binary(BinaryOp::Add, Box::new(load()), Box::new(step), None),
                op,
            );
            body.push(b.store(addr.clone(), sc, next));
            body.push(stmt(StmtKind::Continue(l)));
            Ok(stmt(StmtKind::Loop(l, body)))
        })?;
        out.push(body);
        Ok(())
    }

    fn while_loop(&mut self, w: &While, out: &mut Vec<Stmt>) -> Result<(), Diagnostic> {
        let s = self.in_loop(|b, l| {
            let c = b.expr(&w.condition, Some(BOOL))?;
            let mut body = vec![stmt(StmtKind::If(
                not(c),
                vec![stmt(StmtKind::Break(l))],
                vec![],
            ))];
            body.extend(b.stmts(&w.body)?);
            body.push(stmt(StmtKind::Continue(l)));
            Ok(stmt(StmtKind::Loop(l, body)))
        })?;
        out.push(s);
        Ok(())
    }

    fn repeat(&mut self, r: &Repeat, out: &mut Vec<Stmt>) -> Result<(), Diagnostic> {
        let s = self.in_loop(|b, l| {
            let mut body = b.stmts(&r.body)?;
            let c = b.expr(&r.until, Some(BOOL))?;
            body.push(stmt(StmtKind::If(
                c,
                vec![stmt(StmtKind::Break(l))],
                vec![stmt(StmtKind::Continue(l))],
            )));
            Ok(stmt(StmtKind::Loop(l, body)))
        })?;
        out.push(s);
        Ok(())
    }
}

fn one(sc: Scalar) -> Const {
    match sc {
        Scalar::Real { .. } => Const::Real(1.0),
        _ => Const::Int(1),
    }
}
