//! Calls of functions and function blocks.

use ironplc_dsl::core::{Located, SourceSpan};
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_dsl::textual::{Expr as AstExpr, FbCall, Function, ParamAssignmentKind};
use ironplc_wasm_ir::{Addr, Call, Copy, Expr, ExprKind, Place, Scalar, Stmt, StmtKind};

use crate::body::{ex, nyi, stmt, Body};
use crate::expr::{convert, widen};
use crate::lower::{Member, Section};
use crate::types::{op_scalar, Ty};

/// The arguments of a call, bound to the members of the callee.
struct Bound<'e> {
    inputs: Vec<(&'e Member, &'e AstExpr)>,
    outputs: Vec<(&'e Member, &'e ironplc_dsl::textual::Output)>,
}

fn bind<'e>(
    members: &'e [Member],
    params: &'e [ParamAssignmentKind],
    span: &SourceSpan,
) -> Result<Bound<'e>, Diagnostic> {
    let positional: Vec<&Member> = members
        .iter()
        .filter(|m| matches!(m.section, Section::Input | Section::InOut))
        .collect();
    let mut bound = Bound {
        inputs: vec![],
        outputs: vec![],
    };
    let mut next = 0;
    for p in params {
        match p {
            ParamAssignmentKind::PositionalInput(i) => {
                let m = positional
                    .get(next)
                    .ok_or_else(|| nyi(span, "Too many arguments"))?;
                next += 1;
                bound.inputs.push((m, &i.expr));
            }
            ParamAssignmentKind::NamedInput(i) => {
                let m = find(members, &i.name.to_string())
                    .ok_or_else(|| nyi(&i.name.span(), "This parameter"))?;
                bound.inputs.push((m, &i.expr));
            }
            ParamAssignmentKind::Output(o) => {
                if o.not {
                    return Err(nyi(&o.src.span(), "A negated output"));
                }
                let m = find(members, &o.src.to_string())
                    .ok_or_else(|| nyi(&o.src.span(), "This output"))?;
                bound.outputs.push((m, o));
            }
        }
    }
    Ok(bound)
}

fn find<'m>(members: &'m [Member], name: &str) -> Option<&'m Member> {
    members.iter().find(|m| m.name.eq_ignore_ascii_case(name))
}

fn empty_call(func: u32) -> Call {
    Call {
        func,
        instance: None,
        frame: None,
        inputs: vec![],
        in_outs: vec![],
        copies_in: vec![],
        strings_in: vec![],
        outputs: vec![],
        copies_out: vec![],
        strings_out: vec![],
        result: None,
    }
}

impl Body<'_, '_> {
    /// Fills the inputs and outputs of a call; `at` is the address of a
    /// member in the callee. Statements that compute string arguments are
    /// appended to `pre`.
    fn arguments(
        &mut self,
        call: &mut Call,
        bound: &Bound,
        at: &dyn Fn(u32) -> Addr,
        pre: &mut Vec<Stmt>,
    ) -> Result<(), Diagnostic> {
        for (m, e) in &bound.inputs {
            let dst = at(m.offset);
            if m.section == Section::InOut {
                let ironplc_dsl::textual::ExprKind::Variable(v) = &e.kind else {
                    return Err(nyi(&e.span(), "An in-out argument that is not a variable"));
                };
                let (addr, _) = self.place(v)?;
                call.in_outs.push((dst, addr));
                continue;
            }
            match &m.ty {
                Ty::Scalar { .. } | Ty::Ref(_) => {
                    let sc = m.ty.sc().unwrap_or(crate::types::U32);
                    let v = self.expr(e, Some(op_scalar(sc)))?;
                    call.inputs
                        .push((Place { addr: dst, ty: sc }, convert(v, sc)));
                }
                Ty::Str { cap, wide } => {
                    let src = self.string_operand(e, *wide, pre)?;
                    call.strings_in.push((dst, *cap, src, *wide));
                }
                ty @ (Ty::Array(_) | Ty::Struct(_)) => {
                    let (size, _) = self.l.size_align(ty);
                    let src = self.aggregate(e, ty, pre)?;
                    call.copies_in.push(Copy { dst, src, size });
                }
                Ty::Fb(_) => return Err(nyi(&e.span(), "A function block as argument")),
            }
        }
        for (m, o) in &bound.outputs {
            let src = at(m.offset);
            let (addr, ty) = self.place(&o.tgt)?;
            match (&m.ty, &ty) {
                (from, to) if from.sc().is_some() && to.sc().is_some() => {
                    let (from, to) = (
                        &from.sc().unwrap_or(crate::types::U32),
                        &to.sc().unwrap_or(crate::types::U32),
                    );
                    let v = widen(
                        ex(
                            ExprKind::Load(Place {
                                addr: src,
                                ty: *from,
                            }),
                            *from,
                        ),
                        op_scalar(*from),
                    );
                    call.outputs
                        .push((Place { addr, ty: *to }, convert(v, *to)));
                }
                (Ty::Str { wide, .. }, Ty::Str { cap, .. }) => {
                    call.strings_out.push((addr, *cap, src, *wide));
                }
                (from, to) if self.l.size_align(from) == self.l.size_align(to) => {
                    let (size, _) = self.l.size_align(from);
                    call.copies_out.push(Copy {
                        dst: addr,
                        src,
                        size,
                    });
                }
                _ => return Err(nyi(&o.src.span(), "This output")),
            }
        }
        Ok(())
    }

    /// `instance(...)`.
    pub fn fb_call(&mut self, c: &FbCall, out: &mut Vec<Stmt>) -> Result<(), Diagnostic> {
        let (inst, ty) = self.place(&ironplc_dsl::textual::Variable::named(
            &c.var_name.to_string(),
        ))?;
        let Ty::Fb(name) = ty else {
            return Err(nyi(&c.position, "A call of this variable"));
        };
        let fb = self.l.fb_type(&name, &c.position)?;
        let bound = bind(&fb.members, &c.params, &c.position)?;
        let mut call = empty_call(fb.func);
        call.instance = Some([inst.clone(), inst.clone(), inst.clone()]);
        let base = inst.clone();
        self.arguments(&mut call, &bound, &move |off| base.shifted(off), out)?;
        out.push(stmt(StmtKind::Call(Box::new(call))));
        Ok(())
    }

    /// A function call in an expression.
    pub fn function(
        &mut self,
        f: &Function,
        want: Option<Scalar>,
        e: &AstExpr,
    ) -> Result<Expr, Diagnostic> {
        match self.l.func_type(&f.name.to_string())? {
            Some(ft) => {
                let mut pre = vec![];
                let (call, result) =
                    self.user_call(&ft, &f.param_assignment, &e.span(), &mut pre)?;
                let (off, ty) =
                    result.ok_or_else(|| nyi(&e.span(), "A function without result"))?;
                let sc = ty
                    .sc()
                    .ok_or_else(|| nyi(&e.span(), "A function result of this type"))?;
                let mut call = call;
                call.result = Some(Place {
                    addr: Addr::object(ft.frame, off),
                    ty: sc,
                });
                let v = ex(ExprKind::Call(Box::new(call)), sc);
                let v = if pre.is_empty() {
                    v
                } else {
                    ex(ExprKind::Seq(pre, Box::new(v)), sc)
                };
                Ok(widen(v, op_scalar(sc)))
            }
            None => self.std_function(f, want, e),
        }
    }

    /// Builds the call of a function declared in the library; returns the
    /// call and the offset and type of its result.
    pub fn user_call(
        &mut self,
        ft: &crate::lower::FuncType,
        params: &[ParamAssignmentKind],
        span: &SourceSpan,
        pre: &mut Vec<Stmt>,
    ) -> Result<(Call, Option<(u32, Ty)>), Diagnostic> {
        let bound = bind(&ft.members, params, span)?;
        let mut call = empty_call(ft.func);
        call.frame = Some(ft.frame);
        let frame = ft.frame;
        self.arguments(&mut call, &bound, &move |off| Addr::object(frame, off), pre)?;
        Ok((call, ft.result.clone()))
    }
}
