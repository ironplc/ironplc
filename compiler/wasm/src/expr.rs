//! Expressions, conversions and the places of variables.
//!
//! An expression is computed in an operation scalar: 32 bits for integers
//! narrower than 32 bits (ADR-0001), the width of the destination when the
//! context gives one (REQ-WT-wasm-021), as the bytecode code generator does.

use ironplc_dsl::common::{ConstantKind, TypeName};
use ironplc_dsl::core::Located;
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_dsl::textual::{
    CompareOp, Expr as AstExpr, ExprKind as Ast, ExprType, Operator, SymbolicVariableKind, UnaryOp,
    Variable,
};
use ironplc_wasm_ir::{
    Addr, Base, BinaryOp, Const, ConvMode, Expr, ExprKind, Intrinsic, Place, Scalar, Stmt,
    StmtKind, UnOp,
};

use crate::body::{ex, nyi, stmt, Body};
use crate::types::{elementary, is_int, op_scalar, Ty, BOOL, F64, I32, U32};

/// Converts a value to the scalar `to`, keeping the value where the target
/// holds it: integers wrap, reals truncate toward zero at the operation
/// width of the target and then wrap (the bytecode's conversion followed by
/// its truncation), anything to `BOOL` tests for zero.
pub(crate) fn convert(e: Expr, to: Scalar) -> Expr {
    let from = e.ty;
    if from == to {
        return e;
    }
    let mode = match (from, to) {
        (_, Scalar::Bool) => ConvMode::NotZero,
        (Scalar::Real { .. }, Scalar::Real { .. }) => ConvMode::Float,
        (_, Scalar::Real { .. }) => ConvMode::Float,
        (Scalar::Real { .. }, _) => {
            let op = op_scalar(to);
            if op != to {
                return convert(convert(e, op), to);
            }
            ConvMode::Truncate
        }
        _ if to.size() < from.size() => ConvMode::Wrap,
        _ => ConvMode::Widen,
    };
    ex(ExprKind::Convert(mode, Box::new(e)), to)
}

/// Converts a value loaded from storage to its operation scalar.
pub(crate) fn widen(e: Expr, to: Scalar) -> Expr {
    convert(e, to)
}

/// Whether a constant expression is negative; `None` when it is not a
/// constant.
pub(crate) fn constant_sign(e: &AstExpr) -> Option<bool> {
    match &e.kind {
        Ast::Const(ConstantKind::IntegerLiteral(i)) => {
            Some(i.value.is_neg && i.value.value.value != 0)
        }
        Ast::Const(ConstantKind::RealLiteral(r)) => Some(r.value < 0.0),
        Ast::Expression(inner) => constant_sign(inner),
        Ast::UnaryOp(u) if u.op == UnaryOp::Neg => constant_sign(&u.term).map(|s| !s),
        _ => None,
    }
}

pub(crate) fn bin(op: BinaryOp, a: Expr, b: Expr, ty: Scalar) -> Expr {
    ex(ExprKind::Binary(op, Box::new(a), Box::new(b), None), ty)
}

pub(crate) fn int(v: i128, ty: Scalar) -> Expr {
    ex(ExprKind::Const(Const::Int(v)), ty)
}

impl Body<'_, '_> {
    /// The storage type of an expression, from the type the analyzer
    /// resolved; `None` for a generic or unknown type.
    pub fn resolved(&self, e: &AstExpr) -> Option<Ty> {
        elementary(self.l.ctx.types().representation_of_expr(e)?)
    }

    /// The type name of an operand, as the operator overloads take it: a
    /// concrete type's name, or the generic category of an untyped literal.
    pub fn operand_name(&self, e: &AstExpr) -> Option<TypeName> {
        match e.expr_type.as_ref()? {
            ExprType::Concrete(id) => self.l.ctx.types().name_of(*id).cloned(),
            ExprType::Literal(generic) => Some(generic.clone().into()),
            ExprType::Null => None,
        }
    }

    /// The operation scalar of an expression when its type is concrete.
    pub fn concrete(&self, e: &AstExpr) -> Option<Scalar> {
        self.resolved(e).and_then(|t| t.sc()).map(op_scalar)
    }

    /// The operation scalar of an expression, `DINT` when its type is
    /// generic (the bytecode's default for constant expressions).
    pub fn natural(&self, e: &AstExpr) -> Scalar {
        if let Some(sc) = self.concrete(e) {
            return sc;
        }
        match &e.kind {
            Ast::Const(ConstantKind::RealLiteral(_)) => F64,
            Ast::Const(ConstantKind::Boolean(_)) => BOOL,
            Ast::Expression(inner) => self.natural(inner),
            _ => I32,
        }
    }

    /// Whether an expression is a string.
    pub fn is_string(&mut self, e: &AstExpr) -> bool {
        match &e.kind {
            Ast::Const(ConstantKind::CharacterString(_)) => true,
            Ast::Variable(v) if crate::bits::slice(v).is_none() => {
                matches!(self.place(v), Ok((_, Ty::Str { .. })))
            }
            Ast::Expression(inner) => self.is_string(inner),
            _ => matches!(self.resolved(e), Some(Ty::Str { .. })),
        }
    }

    /// An expression computed in `want`, or in its natural scalar.
    pub fn expr(&mut self, e: &AstExpr, want: Option<Scalar>) -> Result<Expr, Diagnostic> {
        let v = self.value(e, want)?;
        Ok(match want {
            Some(w) => convert(v, w),
            None => v,
        })
    }

    fn value(&mut self, e: &AstExpr, want: Option<Scalar>) -> Result<Expr, Diagnostic> {
        let t = want.unwrap_or_else(|| self.natural(e));
        match &e.kind {
            // Object-oriented function blocks are not supported by this
            // target (see `specs/design/wasm-target.md`).
            Ast::MethodCall(_) => Err(nyi(&e.span(), "A method call")),
            Ast::Const(c) => {
                if let ConstantKind::CharacterString(_) = c {
                    return Err(nyi(&e.span(), "A string in this context"));
                }
                let target = if t == BOOL && !matches!(c, ConstantKind::Boolean(_)) {
                    self.natural(e)
                } else {
                    t
                };
                let c = crate::constant::constant(c, target, &e.span())?;
                Ok(ex(ExprKind::Const(c), target))
            }
            Ast::EnumeratedValue(v) => {
                let o = self
                    .l
                    .enums
                    .ordinal(v)
                    .ok_or_else(|| nyi(&v.span(), "This enumerated value"))?;
                Ok(int(o as i128, I32))
            }
            Ast::Variable(v) => self.load(v),
            Ast::LateBound(lb) => {
                let key = lb.value.to_string().to_uppercase();
                if self.vars.contains_key(&key) {
                    return self.load(&Variable::named(&lb.value.to_string()));
                }
                let v = ironplc_dsl::common::EnumeratedValue::new(&lb.value.to_string());
                let o = self
                    .l
                    .enums
                    .ordinal(&v)
                    .ok_or_else(|| nyi(&lb.value.span(), "This name"))?;
                Ok(int(o as i128, I32))
            }
            Ast::Expression(inner) => self.value(inner, want),
            Ast::UnaryOp(u) => match u.op {
                UnaryOp::Neg => {
                    let a = self.expr(&u.term, Some(t))?;
                    Ok(ex(ExprKind::Unary(UnOp::Neg, Box::new(a)), t))
                }
                UnaryOp::Not => self.not(&u.term, want),
            },
            Ast::BinaryOp(b) => {
                if let Some(r) = self.typed_operator(&b.op, &b.left, &b.right, &e.span())? {
                    return Ok(r);
                }
                // An operator computes at its own result type, then `expr`
                // converts to what the context wants: `DINT * DINT` wraps at
                // 32 bits before it is stored in a LINT, as on the VM.
                let t = match self.concrete(e) {
                    Some(own) => own,
                    None if t == BOOL => self.natural(e),
                    None => t,
                };
                let a = self.expr(&b.left, Some(t))?;
                let c = self.expr(&b.right, Some(t))?;
                let op = match b.op {
                    Operator::Add => BinaryOp::Add,
                    Operator::Sub => BinaryOp::Sub,
                    Operator::Mul => BinaryOp::Mul,
                    Operator::Div => BinaryOp::Div,
                    Operator::Mod => BinaryOp::Mod,
                    Operator::Pow => return self.power(&b.left, &b.right, t, &e.span()),
                };
                let site = (matches!(op, BinaryOp::Div | BinaryOp::Mod) && is_int(t))
                    .then(|| self.l.site(&e.span()));
                Ok(ex(ExprKind::Binary(op, Box::new(a), Box::new(c), site), t))
            }
            Ast::Compare(c) => self.compare(&c.op, &c.left, &c.right, want),
            Ast::Function(f) => self.function(f, want, e),
            Ast::Ref(v) => {
                let (addr, _) = self.place(v)?;
                Ok(ex(ExprKind::AddrOf(addr), U32))
            }
            Ast::Null(_) => Ok(int(0, U32)),
            Ast::Deref(inner) => {
                let (addr, ty) = self.deref(inner)?;
                let sc = ty
                    .sc()
                    .ok_or_else(|| nyi(&e.span(), "A dereference of this type"))?;
                Ok(widen(
                    ex(ExprKind::Load(Place { addr, ty: sc }), sc),
                    op_scalar(sc),
                ))
            }
        }
    }

    /// `NOT`: bitwise for unsigned integers (masked to the width of a
    /// narrow operand), logical otherwise, as the bytecode computes it.
    fn not(&mut self, term: &AstExpr, want: Option<Scalar>) -> Result<Expr, Diagnostic> {
        let t = want
            .filter(|w| *w != BOOL)
            .or_else(|| self.concrete(term))
            .unwrap_or(BOOL);
        match t {
            Scalar::Int { signed: false, .. } => {
                let a = self.expr(term, Some(t))?;
                let n = ex(ExprKind::Unary(UnOp::Not, Box::new(a)), t);
                let bits = self
                    .resolved(term)
                    .and_then(|ty| ty.sc())
                    .map(|sc| sc.size() * 8)
                    .unwrap_or(32);
                if bits < 32 && t == U32 {
                    Ok(bin(BinaryOp::And, n, int((1i128 << bits) - 1, U32), U32))
                } else {
                    Ok(n)
                }
            }
            _ => {
                let a = self.expr(term, Some(BOOL))?;
                Ok(ex(ExprKind::Unary(UnOp::Not, Box::new(a)), BOOL))
            }
        }
    }

    fn compare(
        &mut self,
        op: &CompareOp,
        left: &AstExpr,
        right: &AstExpr,
        want: Option<Scalar>,
    ) -> Result<Expr, Diagnostic> {
        if self.is_string(left) || self.is_string(right) {
            return self.string_compare(op, left, right);
        }
        let operand = self
            .concrete(left)
            .or_else(|| self.concrete(right))
            .or(want)
            .unwrap_or_else(|| self.natural(left));
        let (bop, logical) = match op {
            CompareOp::And => (BinaryOp::And, true),
            CompareOp::Or => (BinaryOp::Or, true),
            CompareOp::Xor => (BinaryOp::Xor, true),
            CompareOp::AndThen | CompareOp::OrElse => {
                return self.short_circuit(*op == CompareOp::AndThen, left, right)
            }
            CompareOp::Eq => (BinaryOp::Eq, false),
            CompareOp::Ne => (BinaryOp::Ne, false),
            CompareOp::Lt => (BinaryOp::Lt, false),
            CompareOp::Gt => (BinaryOp::Gt, false),
            CompareOp::LtEq => (BinaryOp::Le, false),
            CompareOp::GtEq => (BinaryOp::Ge, false),
        };
        let a = self.expr(left, Some(operand))?;
        let b = self.expr(right, Some(operand))?;
        let ty = if logical { operand } else { BOOL };
        Ok(bin(bop, a, b, ty))
    }

    /// `AND_THEN`, `OR_ELSE`: the right operand only when the left one does
    /// not decide.
    fn short_circuit(
        &mut self,
        and: bool,
        left: &AstExpr,
        right: &AstExpr,
    ) -> Result<Expr, Diagnostic> {
        let t = self.temp(BOOL);
        let a = self.expr(left, Some(BOOL))?;
        let b = self.expr(right, Some(BOOL))?;
        let get = || ex(ExprKind::Temp(t), BOOL);
        let cond = if and { get() } else { crate::body::not(get()) };
        let pre = vec![
            stmt(StmtKind::SetTemp(t, a)),
            stmt(StmtKind::If(
                cond,
                vec![stmt(StmtKind::SetTemp(t, b))],
                vec![],
            )),
        ];
        Ok(ex(ExprKind::Seq(pre, Box::new(get())), BOOL))
    }

    /// `**` and `EXPT`: a real power for a real type, an integer power that
    /// wraps for an integer type.
    pub fn power(
        &mut self,
        base: &AstExpr,
        exp: &AstExpr,
        t: Scalar,
        span: &ironplc_dsl::core::SourceSpan,
    ) -> Result<Expr, Diagnostic> {
        if !t.is_real() {
            return self.int_power(base, exp, t, span);
        }
        let b = self.expr(base, Some(t))?;
        let en = self.natural(exp);
        let e = self.expr(exp, Some(en))?;
        Ok(ex(ExprKind::Intrinsic(Intrinsic::Pow, vec![b, e]), t))
    }

    /// An integer power, wrapping as the bytecode's `wrapping_pow`, by
    /// squaring; a negative exponent traps, with the code of a value out of
    /// its range since the ABI has none of its own.
    fn int_power(
        &mut self,
        base: &AstExpr,
        exp: &AstExpr,
        t: Scalar,
        span: &ironplc_dsl::core::SourceSpan,
    ) -> Result<Expr, Diagnostic> {
        let b = self.expr(base, Some(t))?;
        let e = self.expr(exp, Some(t))?;
        let site = self.l.site(span);
        let (r, x, n) = (self.temp(t), self.temp(t), self.temp(t));
        let get = |i| ex(ExprKind::Temp(i), t);
        let set = |i, v| stmt(StmtKind::SetTemp(i, v));
        let l = self.l.label();
        let odd = bin(
            BinaryOp::Ne,
            bin(BinaryOp::And, get(n), int(1, t), t),
            int(0, t),
            BOOL,
        );
        let body = vec![
            stmt(StmtKind::If(
                bin(BinaryOp::Le, get(n), int(0, t), BOOL),
                vec![stmt(StmtKind::Break(l))],
                vec![],
            )),
            stmt(StmtKind::If(
                odd,
                vec![set(r, bin(BinaryOp::Mul, get(r), get(x), t))],
                vec![],
            )),
            set(x, bin(BinaryOp::Mul, get(x), get(x), t)),
            set(
                n,
                ex(
                    ExprKind::Intrinsic(Intrinsic::Shr, vec![get(n), int(1, t)]),
                    t,
                ),
            ),
            stmt(StmtKind::Continue(l)),
        ];
        let pre = vec![
            set(n, e),
            stmt(StmtKind::Check(
                bin(BinaryOp::Ge, get(n), int(0, t), BOOL),
                4,
                site,
            )),
            set(x, b),
            set(r, int(1, t)),
            stmt(StmtKind::Loop(l, body)),
        ];
        Ok(ex(ExprKind::Seq(pre, Box::new(get(r))), t))
    }

    // ----- places

    /// The address and the type of a variable.
    pub fn place(&mut self, v: &Variable) -> Result<(Addr, Ty), Diagnostic> {
        match v {
            Variable::Symbolic(s) => self.symbolic(s),
            Variable::Direct(a) => Err(nyi(&a.position, "A direct address in an expression")),
        }
    }

    fn symbolic(&mut self, s: &SymbolicVariableKind) -> Result<(Addr, Ty), Diagnostic> {
        match s {
            SymbolicVariableKind::Named(n) => {
                let var = self
                    .vars
                    .get(&n.name.to_string().to_uppercase())
                    .ok_or_else(|| nyi(&n.name.span(), "This variable"))?;
                Ok((var.place(), var.ty.clone()))
            }
            SymbolicVariableKind::Array(a) => {
                let (base, ty) = self.symbolic(&a.subscripted_variable)?;
                let Ty::Array(at) = ty else {
                    return Err(nyi(&a.span(), "A subscript of this variable"));
                };
                if a.subscripts.len() != at.dims.len() {
                    return Err(nyi(&a.span(), "A partial subscript"));
                }
                let mut stride = at.stride as u64 * at.count();
                let mut offset: Option<Expr> = None;
                for (sub, (lo, hi)) in a.subscripts.iter().zip(&at.dims) {
                    let count = (hi - lo + 1).max(1) as u64;
                    stride /= count;
                    let it = self.natural(sub);
                    let it = if is_int(it) { it } else { I32 };
                    let index = self.expr(sub, Some(it))?;
                    let site = self.l.bounds_checks.then(|| self.l.site(&sub.span()));
                    let term = ex(
                        ExprKind::Index {
                            index: Box::new(index),
                            low: *lo,
                            count,
                            stride: stride as u32,
                            site,
                        },
                        U32,
                    );
                    offset = Some(match offset {
                        None => term,
                        Some(o) => bin(BinaryOp::Add, o, term, U32),
                    });
                }
                let offset = offset.unwrap_or_else(|| int(0, U32));
                Ok((
                    Addr {
                        base: Base::Index(Box::new(base), Box::new(offset)),
                        offset: 0,
                    },
                    at.elem.clone(),
                ))
            }
            SymbolicVariableKind::Structured(st) => {
                let (base, ty) = self.symbolic(&st.record)?;
                let name = st.field.to_string();
                let (off, fty) = match &ty {
                    Ty::Struct(s) => s
                        .field(&name)
                        .map(|f| (f.offset, f.ty.clone()))
                        .ok_or_else(|| nyi(&st.field.span(), "This member"))?,
                    Ty::Fb(fb) => {
                        let fb = self.l.fbs.get(fb).cloned();
                        let m = fb
                            .as_ref()
                            .and_then(|fb| fb.member(&name))
                            .ok_or_else(|| nyi(&st.field.span(), "This member"))?;
                        if m.section == crate::lower::Section::InOut {
                            return Err(nyi(&st.field.span(), "An in-out member from outside"));
                        }
                        (m.offset, m.ty.clone())
                    }
                    _ => return Err(nyi(&st.field.span(), "A member of this variable")),
                };
                Ok((base.shifted(off), fty))
            }
            SymbolicVariableKind::BitAccess(b) => Err(nyi(&b.span(), "A bit as a place")),
            SymbolicVariableKind::PartialAccess(p) => {
                Err(nyi(&p.span(), "A partial access as a place"))
            }
            SymbolicVariableKind::Deref(d) => {
                let (slot, ty) = self.symbolic(&d.variable)?;
                self.through(slot, ty, &d.span())
            }
            SymbolicVariableKind::SelfRef(s) => Err(nyi(&s.position, "THIS^ and SUPER^")),
        }
    }

    /// The target of the reference an expression names.
    fn deref(&mut self, inner: &AstExpr) -> Result<(Addr, Ty), Diagnostic> {
        match &inner.kind {
            Ast::Variable(v) => {
                let (slot, ty) = self.place(v)?;
                self.through(slot, ty, &inner.span())
            }
            Ast::Expression(e) => self.deref(e),
            _ => Err(nyi(&inner.span(), "A dereference of this expression")),
        }
    }

    /// The target of the reference stored at `slot`. A `NULL` reference
    /// traps before the access, with the code of an access out of bounds:
    /// the ABI has no code of its own for it.
    pub fn through(
        &mut self,
        slot: Addr,
        ty: Ty,
        span: &ironplc_dsl::core::SourceSpan,
    ) -> Result<(Addr, Ty), Diagnostic> {
        let Ty::Ref(target) = ty else {
            return Err(nyi(
                span,
                "A dereference of a value that is not a reference",
            ));
        };
        let site = self.l.site(span);
        let ptr = ex(
            ExprKind::Load(Place {
                addr: slot.clone(),
                ty: U32,
            }),
            U32,
        );
        let not_null = bin(BinaryOp::Ne, ptr, int(0, U32), BOOL);
        let check = stmt(StmtKind::Check(not_null, 3, site));
        let zero = ex(ExprKind::Seq(vec![check], Box::new(int(0, U32))), U32);
        let at = Addr {
            base: Base::Deref(Box::new(slot)),
            offset: 0,
        };
        Ok((
            Addr {
                base: Base::Index(Box::new(at), Box::new(zero)),
                offset: 0,
            },
            *target,
        ))
    }

    /// The value of a variable in its operation scalar.
    pub fn load(&mut self, v: &Variable) -> Result<Expr, Diagnostic> {
        if let Some(s) = crate::bits::slice(v) {
            return self.read_slice(&s);
        }
        let (addr, ty) = self.place(v)?;
        let sc = ty
            .sc()
            .ok_or_else(|| nyi(&v.span(), "A value of this type in an expression"))?;
        Ok(widen(
            ex(ExprKind::Load(Place { addr, ty: sc }), sc),
            op_scalar(sc),
        ))
    }

    /// The address of an array or a structure used as a whole.
    /// A function result is read from the frame after the call, which is
    /// appended to `pre`.
    pub fn aggregate(
        &mut self,
        value: &AstExpr,
        ty: &Ty,
        pre: &mut Vec<Stmt>,
    ) -> Result<Addr, Diagnostic> {
        let (addr, vty) = match &value.kind {
            Ast::Variable(v) => self.place(v)?,
            Ast::Expression(inner) => return self.aggregate(inner, ty, pre),
            Ast::Function(f) => {
                let ft = self
                    .l
                    .func_type(&f.name.to_string())?
                    .ok_or_else(|| nyi(&value.span(), "This aggregate value"))?;
                let (call, result) =
                    self.user_call(&ft, &f.param_assignment, &value.span(), pre)?;
                let (off, rty) =
                    result.ok_or_else(|| nyi(&value.span(), "A function without result"))?;
                pre.push(stmt(StmtKind::Call(Box::new(call))));
                (Addr::object(ft.frame, off), rty)
            }
            _ => return Err(nyi(&value.span(), "This aggregate value")),
        };
        if self.l.size_align(&vty) != self.l.size_align(ty) {
            return Err(nyi(&value.span(), "A copy between different types"));
        }
        Ok(addr)
    }
}
