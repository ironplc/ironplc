//! IR validator: types of every expression, labels in scope,
//! places inside their objects, temporaries and calls consistent. A failure
//! is an internal error of the compiler (`I001`).

use crate::ir::*;

/// Validates a module; returns a description of the first problem.
pub fn validate(m: &Module) -> Result<(), String> {
    for (i, f) in m.functions.iter().enumerate() {
        let v = Validator {
            m,
            f,
            labels: vec![],
        };
        let mut v = v;
        v.stmts(&f.body)
            .map_err(|e| format!("function {i} ({}): {e}", f.name))?;
    }
    for (i, l) in m.leaves.iter().enumerate() {
        let o = m
            .objects
            .get(l.object as usize)
            .ok_or_else(|| format!("leaf {i}: no object {}", l.object))?;
        if l.offset + l.size > o.init.len() as u32 {
            return Err(format!("leaf {} outside its object", l.path));
        }
        if l.declared as usize >= m.sites.len() {
            return Err(format!("leaf {}: no site {}", l.path, l.declared));
        }
    }
    for t in &m.tasks {
        for (f, _) in &t.programs {
            match m.functions.get(*f as usize) {
                Some(func) if func.kind == FuncKind::Program => {}
                _ => return Err(format!("task {}: function {f} is not a program", t.name)),
            }
        }
    }
    Ok(())
}

struct Validator<'m> {
    m: &'m Module,
    f: &'m Function,
    /// Enclosing labels; `true` for loops.
    labels: Vec<(Label, bool)>,
}

const U32: Scalar = Scalar::Int {
    bits: 32,
    signed: false,
};

fn same(a: Scalar, b: Scalar, what: &str) -> Result<(), String> {
    if a == b {
        Ok(())
    } else {
        Err(format!("{what}: {} and {}", a.name(), b.name()))
    }
}

impl Validator<'_> {
    fn addr(&self, a: &Addr, size: u32) -> Result<(), String> {
        match &a.base {
            Base::Object(o) => {
                let obj = self
                    .m
                    .objects
                    .get(*o as usize)
                    .ok_or_else(|| format!("no object {o}"))?;
                if a.offset + size > obj.init.len() as u32 {
                    return Err(format!(
                        "address o{o}+{} outside the object ({} bytes)",
                        a.offset,
                        obj.init.len()
                    ));
                }
                Ok(())
            }
            Base::SelfPart(k) => {
                if self.f.kind != FuncKind::FunctionBlock || *k > 2 {
                    return Err(format!("instance part {k} outside a function block"));
                }
                Ok(())
            }
            Base::Deref(inner) => self.addr(inner, 4),
            Base::Index(inner, e) => {
                self.addr(inner, 0)?;
                self.expr(e)?;
                same(e.ty, U32, "element offset")
            }
        }
    }

    fn place(&self, p: &Place) -> Result<(), String> {
        self.addr(&p.addr, p.ty.size())
    }

    fn expr(&self, e: &Expr) -> Result<(), String> {
        match &e.kind {
            ExprKind::Const(_) => Ok(()),
            ExprKind::Load(p) => {
                self.place(p)?;
                same(p.ty, e.ty, "load")
            }
            ExprKind::Temp(t) => {
                let ty = self
                    .f
                    .temps
                    .get(*t as usize)
                    .ok_or_else(|| format!("no temporary {t}"))?;
                same(*ty, e.ty, "temporary")
            }
            ExprKind::AddrOf(a) => self.addr(a, 0),
            ExprKind::Unary(op, a) => {
                self.expr(a)?;
                same(a.ty, e.ty, "unary operand")?;
                if *op == UnOp::Not && e.ty.is_real() {
                    return Err("NOT of a real".into());
                }
                Ok(())
            }
            ExprKind::Binary(BinaryOp::Pow, ..) => Err("** is Intrinsic::Pow".into()),
            ExprKind::Binary(op, a, b, site) => {
                self.expr(a)?;
                self.expr(b)?;
                same(a.ty, b.ty, "binary operands")?;
                let comparison = matches!(
                    op,
                    BinaryOp::Eq
                        | BinaryOp::Ne
                        | BinaryOp::Lt
                        | BinaryOp::Gt
                        | BinaryOp::Le
                        | BinaryOp::Ge
                );
                if comparison {
                    same(e.ty, Scalar::Bool, "comparison result")?;
                } else {
                    same(a.ty, e.ty, "binary result")?;
                }
                if let Some(s) = site {
                    if *s as usize >= self.m.sites.len() {
                        return Err(format!("no site {s}"));
                    }
                }
                Ok(())
            }
            ExprKind::Convert(_, a) => self.expr(a),
            ExprKind::Select(c, a, b) => {
                self.expr(c)?;
                self.expr(a)?;
                self.expr(b)?;
                same(c.ty, Scalar::Bool, "select condition")?;
                same(a.ty, e.ty, "select")?;
                same(b.ty, e.ty, "select")
            }
            ExprKind::Intrinsic(i, args) => {
                for a in args {
                    self.expr(a)?;
                }
                match i {
                    Intrinsic::Pow => {
                        if args.len() != 2 || !e.ty.is_real() {
                            return Err("malformed power".into());
                        }
                        same(args[0].ty, e.ty, "power base")
                    }
                    Intrinsic::Shl | Intrinsic::Shr | Intrinsic::Rol | Intrinsic::Ror => {
                        if args.len() != 2 || args[1].ty.is_real() || args[1].ty == Scalar::Bool {
                            return Err(format!("malformed {i:?}"));
                        }
                        same(args[0].ty, e.ty, "shifted value")
                    }
                    Intrinsic::Mux => {
                        if args.len() < 2 || args[0].ty.is_real() {
                            return Err("malformed MUX".into());
                        }
                        args[1..]
                            .iter()
                            .try_for_each(|a| same(a.ty, e.ty, "MUX input"))
                    }
                    Intrinsic::Compare(_) => {
                        same(e.ty, Scalar::Bool, "comparison result")?;
                        if args.len() < 2 {
                            return Err("comparison with fewer than two operands".into());
                        }
                        args.iter()
                            .try_for_each(|a| same(a.ty, args[0].ty, "compared operand"))
                    }
                    Intrinsic::Sqrt | Intrinsic::Ln | Intrinsic::Log | Intrinsic::Exp => {
                        if args.len() != 1 || !e.ty.is_real() {
                            return Err(format!("malformed {i:?}"));
                        }
                        same(args[0].ty, e.ty, "real argument")
                    }
                    Intrinsic::Now => {
                        if !args.is_empty() || e.ty != (Scalar::Duration { long: true }) {
                            return Err("malformed time source".into());
                        }
                        Ok(())
                    }
                    _ => {
                        let n = match i {
                            Intrinsic::Abs => Some(1),
                            Intrinsic::Limit => Some(3),
                            _ => None,
                        };
                        if n.is_some_and(|n| n != args.len()) || args.is_empty() {
                            return Err(format!("{i:?} with {} arguments", args.len()));
                        }
                        args.iter()
                            .try_for_each(|a| same(a.ty, e.ty, "intrinsic argument"))
                    }
                }
            }
            ExprKind::Call(c) => {
                self.call(c)?;
                let r = c
                    .result
                    .as_ref()
                    .ok_or("call without result used as a value")?;
                same(r.ty, e.ty, "call result")
            }
            ExprKind::Index {
                index, site, count, ..
            } => {
                self.expr(index)?;
                if index.ty.is_real() || index.ty == Scalar::Bool {
                    return Err("index of a non-integer type".into());
                }
                if *count == 0 {
                    return Err("dimension without elements".into());
                }
                self.site(*site)?;
                same(e.ty, U32, "element offset")
            }
            ExprKind::StrChar(a, _) => self.addr(a, 1),
            ExprKind::Seq(pre, e) => {
                let mut v = Validator {
                    m: self.m,
                    f: self.f,
                    labels: self.labels.clone(),
                };
                v.stmts(pre)?;
                self.expr(e)
            }
            ExprKind::StrLen(a, _) | ExprKind::StrFind(a, _, _) | ExprKind::StrCmp(a, _, _) => {
                self.addr(a, 0)?;
                if let ExprKind::StrFind(_, b, _) | ExprKind::StrCmp(_, b, _) = &e.kind {
                    self.addr(b, 0)?;
                }
                match e.ty {
                    Scalar::Int { bits: 16 | 32, .. } => Ok(()),
                    t => Err(format!("string function of type {}", t.name())),
                }
            }
            ExprKind::Checked(a, low, high, site) => {
                self.expr(a)?;
                if low > high {
                    return Err("empty subrange".into());
                }
                self.site(Some(*site))?;
                same(a.ty, e.ty, "checked value")
            }
        }
    }

    fn call(&self, c: &Call) -> Result<(), String> {
        let callee = self
            .m
            .functions
            .get(c.func as usize)
            .ok_or_else(|| format!("no function {}", c.func))?;
        match callee.kind {
            FuncKind::FunctionBlock => {
                let parts = c
                    .instance
                    .as_ref()
                    .ok_or("function block called without instance")?;
                for p in parts {
                    self.addr(p, 0)?;
                }
            }
            FuncKind::Function => {
                if c.frame != callee.frame {
                    return Err(format!("call of {} with a wrong frame", callee.name));
                }
            }
            FuncKind::Program => return Err("call of a program".into()),
        }
        for (p, e) in c.inputs.iter().chain(&c.outputs) {
            self.place(p)?;
            self.expr(e)?;
            same(p.ty, e.ty, "argument")?;
        }
        for (slot, a) in &c.in_outs {
            self.addr(slot, 4)?;
            self.addr(a, 0)?;
        }
        for c in c.copies_in.iter().chain(&c.copies_out) {
            self.copy(c)?;
        }
        for (d, _, s, _) in c.strings_in.iter().chain(&c.strings_out) {
            self.addr(d, 1)?;
            self.addr(s, 1)?;
        }
        Ok(())
    }

    fn copy(&self, c: &Copy) -> Result<(), String> {
        if c.size == 0 {
            return Err("empty copy".into());
        }
        self.addr(&c.dst, c.size)?;
        self.addr(&c.src, c.size)
    }

    fn site(&self, site: Option<SiteId>) -> Result<(), String> {
        match site {
            Some(s) if s as usize >= self.m.sites.len() => Err(format!("no site {s}")),
            _ => Ok(()),
        }
    }

    fn label(&self, l: Label, need_loop: bool) -> Result<(), String> {
        if self
            .labels
            .iter()
            .any(|(x, is_loop)| *x == l && (*is_loop || !need_loop))
        {
            Ok(())
        } else {
            Err(format!("label L{l} not in scope"))
        }
    }

    fn stmts(&mut self, list: &[Stmt]) -> Result<(), String> {
        for s in list {
            self.stmt(s)?;
        }
        Ok(())
    }

    fn stmt(&mut self, s: &Stmt) -> Result<(), String> {
        match &s.kind {
            StmtKind::Assign(p, e) => {
                self.place(p)?;
                self.expr(e)?;
                same(p.ty, e.ty, "assignment")
            }
            StmtKind::Copy(c) => self.copy(c),
            StmtKind::Str(op, _) => {
                let i32t = Scalar::Int {
                    bits: 32,
                    signed: true,
                };
                match &**op {
                    StrOp::Assign { dst, src, .. } => {
                        self.addr(dst, 1)?;
                        self.addr(src, 1)
                    }
                    StrOp::Clear(d) => self.addr(d, 1),
                    StrOp::Convert { dst, src, .. } => {
                        self.addr(dst, 1)?;
                        self.addr(src, 1)
                    }
                    StrOp::SetChar { dst, value } => {
                        self.addr(dst, 2)?;
                        self.expr(value)
                    }
                    StrOp::Append {
                        dst,
                        src,
                        start,
                        count,
                        ..
                    } => {
                        self.addr(dst, 1)?;
                        self.addr(src, 1)?;
                        for e in [start, count] {
                            self.expr(e)?;
                            same(e.ty, i32t, "string position")?;
                        }
                        Ok(())
                    }
                    StrOp::Splice {
                        dst,
                        src,
                        pos,
                        len,
                        ins,
                        ..
                    } => {
                        self.addr(dst, 1)?;
                        self.addr(src, 1)?;
                        if let Some(i) = ins {
                            self.addr(i, 1)?;
                        }
                        for e in [pos, len] {
                            self.expr(e)?;
                            same(e.ty, i32t, "string position")?;
                        }
                        Ok(())
                    }
                }
            }
            StmtKind::SetTemp(t, e) => {
                self.expr(e)?;
                let ty = self
                    .f
                    .temps
                    .get(*t as usize)
                    .ok_or_else(|| format!("no temporary {t}"))?;
                same(*ty, e.ty, "temporary assignment")
            }
            StmtKind::Call(c) => self.call(c),
            StmtKind::Eval(e) => self.expr(e),
            StmtKind::If(c, a, b) => {
                self.expr(c)?;
                same(c.ty, Scalar::Bool, "condition")?;
                self.stmts(a)?;
                self.stmts(b)
            }
            StmtKind::Switch(sel, arms, default) => {
                self.expr(sel)?;
                if sel.ty.is_real() || sel.ty == Scalar::Bool {
                    return Err("switch on a non-integer".into());
                }
                for (_, body) in arms {
                    self.stmts(body)?;
                }
                self.stmts(default)
            }
            StmtKind::Block(l, body) | StmtKind::Loop(l, body) => {
                self.labels.push((*l, matches!(s.kind, StmtKind::Loop(..))));
                let r = self.stmts(body);
                self.labels.pop();
                r
            }
            StmtKind::Break(l) => self.label(*l, false),
            StmtKind::Continue(l) => self.label(*l, true),
            StmtKind::Return => Ok(()),
            StmtKind::Check(c, _, site) => {
                self.expr(c)?;
                if *site as usize >= self.m.sites.len() {
                    return Err(format!("no site {site}"));
                }
                Ok(())
            }
            StmtKind::DebugSite(site) => {
                if *site as usize >= self.m.sites.len() {
                    return Err(format!("no site {site}"));
                }
                Ok(())
            }
            StmtKind::Reset(o) => {
                if *o as usize >= self.m.objects.len() {
                    return Err(format!("no object {o}"));
                }
                Ok(())
            }
        }
    }
}
