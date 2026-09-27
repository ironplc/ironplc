//! Standard function blocks as IR function blocks.
//!
//! The bytecode VM runs them natively (ADR-0003, `compiler/vm/src/intrinsic.rs`);
//! here they are ordinary function blocks whose bodies compute what the
//! intrinsics compute (REQ-WT-wasm-025): the timers keep their start in
//! microseconds of the cycle time and measure whole milliseconds, the edge
//! detectors and counters remember the previous input.

use ironplc_dsl::core::SourceSpan;
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_wasm_ir::{
    Addr, Base, BinaryOp, Const, ConvMode, Expr, ExprKind, FuncKind, Intrinsic, Place, Scalar,
    Stmt, StmtKind,
};

use crate::body::{ex, not, stmt};
use crate::layout::Image;
use crate::lower::{FbType, Lowerer, Member, Section};
use crate::pou::instance_type;
use crate::types::{Ty, BOOL, I16, I32, I64, U32, U64};

/// Declares and defines a standard function block; `None` when the name is
/// not one.
pub(crate) fn declare(l: &mut Lowerer, name: &str) -> Result<Option<FbType>, Diagnostic> {
    let spec = match spec(name) {
        Some(s) => s,
        None => return Ok(None),
    };
    let func = l.declare_function(name, FuncKind::FunctionBlock, None);
    let mut image = Image::default();
    let mut members = vec![];
    for (n, section, ty) in &spec.members {
        let (size, align) = (
            ty.sc().map(|s| s.size()).unwrap_or(1),
            ty.sc().map(|s| s.size()).unwrap_or(1),
        );
        let offset = image.alloc(size, align);
        members.push(Member {
            name: (*n).to_string(),
            section: *section,
            offset,
            ty: ty.clone(),
            span: SourceSpan::default(),
            constant: false,
            location: None,
        });
    }
    let fb = instance_type(name.to_string(), func, members, image, None);
    let body = (spec.body)(&Fb { fb: &fb });
    l.define_function(func, body, vec![], &SourceSpan::default());
    Ok(Some(fb))
}

struct Spec {
    members: Vec<(&'static str, Section, Ty)>,
    body: fn(&Fb) -> Vec<Stmt>,
}

fn bool_ty() -> Ty {
    Ty::scalar(BOOL, "BOOL")
}

fn time_ty() -> Ty {
    Ty::scalar(I32, "TIME")
}

fn spec(name: &str) -> Option<Spec> {
    use Section::*;
    let timer = |body: fn(&Fb) -> Vec<Stmt>| Spec {
        members: vec![
            ("IN", Input, bool_ty()),
            ("PT", Input, time_ty()),
            ("Q", Output, bool_ty()),
            ("ET", Output, time_ty()),
            ("START", Hidden, Ty::scalar(I64, "LINT")),
            ("RUNNING", Hidden, bool_ty()),
        ],
        body,
    };
    let counter = |name: &str| -> Option<Ty> {
        let suffix = name.split_once('_').map(|(_, s)| s).unwrap_or("INT");
        Some(match suffix {
            "INT" => Ty::scalar(I16, "INT"),
            "DINT" => Ty::scalar(I32, "DINT"),
            "LINT" => Ty::scalar(I64, "LINT"),
            "UDINT" => Ty::scalar(U32, "UDINT"),
            "ULINT" => Ty::scalar(U64, "ULINT"),
            _ => return None,
        })
    };
    let bistable = |a: &'static str, b: &'static str, body: fn(&Fb) -> Vec<Stmt>| Spec {
        members: vec![
            (a, Input, bool_ty()),
            (b, Input, bool_ty()),
            ("Q1", Output, bool_ty()),
        ],
        body,
    };
    let edge = |body: fn(&Fb) -> Vec<Stmt>| Spec {
        members: vec![
            ("CLK", Input, bool_ty()),
            ("Q", Output, bool_ty()),
            ("M", Hidden, bool_ty()),
        ],
        body,
    };
    let kind = name.split('_').next().unwrap_or(name);
    Some(match (name, kind) {
        ("TON", _) => timer(ton),
        ("TOF", _) => timer(tof),
        ("TP", _) => timer(tp),
        ("SR", _) => bistable("S1", "R", sr),
        ("RS", _) => bistable("S", "R1", rs),
        ("R_TRIG", _) => edge(r_trig),
        ("F_TRIG", _) => edge(f_trig),
        (_, "CTU") => {
            let t = counter(name)?;
            Spec {
                members: vec![
                    ("CU", Input, bool_ty()),
                    ("R", Input, bool_ty()),
                    ("PV", Input, t.clone()),
                    ("Q", Output, bool_ty()),
                    ("CV", Output, t),
                    ("PREV", Hidden, bool_ty()),
                ],
                body: ctu,
            }
        }
        (_, "CTD") => {
            let t = counter(name)?;
            Spec {
                members: vec![
                    ("CD", Input, bool_ty()),
                    ("LD", Input, bool_ty()),
                    ("PV", Input, t.clone()),
                    ("Q", Output, bool_ty()),
                    ("CV", Output, t),
                    ("PREV", Hidden, bool_ty()),
                ],
                body: ctd,
            }
        }
        (_, "CTUD") => {
            let t = counter(name)?;
            Spec {
                members: vec![
                    ("CU", Input, bool_ty()),
                    ("CD", Input, bool_ty()),
                    ("R", Input, bool_ty()),
                    ("LD", Input, bool_ty()),
                    ("PV", Input, t.clone()),
                    ("QU", Output, bool_ty()),
                    ("QD", Output, bool_ty()),
                    ("CV", Output, t),
                    ("PREVU", Hidden, bool_ty()),
                    ("PREVD", Hidden, bool_ty()),
                ],
                body: ctud,
            }
        }
        _ => return None,
    })
}

/// Access to the members of the instance being defined.
struct Fb<'a> {
    fb: &'a FbType,
}

impl Fb<'_> {
    fn member(&self, name: &str) -> (Addr, Scalar) {
        let m = self.fb.member(name).expect("member of a standard block");
        (
            Addr {
                base: Base::SelfPart(0),
                offset: m.offset,
            },
            m.ty.sc().unwrap_or(BOOL),
        )
    }

    fn get(&self, name: &str) -> Expr {
        let (addr, sc) = self.member(name);
        ex(ExprKind::Load(Place { addr, ty: sc }), sc)
    }

    fn set(&self, name: &str, v: Expr) -> Stmt {
        let (addr, sc) = self.member(name);
        stmt(StmtKind::Assign(Place { addr, ty: sc }, v))
    }

    fn sc(&self, name: &str) -> Scalar {
        self.member(name).1
    }
}

fn b(v: bool) -> Expr {
    ex(ExprKind::Const(Const::Bool(v)), BOOL)
}

fn int(v: i128, t: Scalar) -> Expr {
    ex(ExprKind::Const(Const::Int(v)), t)
}

fn bin(op: BinaryOp, a: Expr, c: Expr, t: Scalar) -> Expr {
    ex(ExprKind::Binary(op, Box::new(a), Box::new(c), None), t)
}

fn and(a: Expr, c: Expr) -> Expr {
    bin(BinaryOp::And, a, c, BOOL)
}

fn or(a: Expr, c: Expr) -> Expr {
    bin(BinaryOp::Or, a, c, BOOL)
}

fn if_(c: Expr, a: Vec<Stmt>, e: Vec<Stmt>) -> Stmt {
    stmt(StmtKind::If(c, a, e))
}

/// The cycle time in microseconds.
fn now_us() -> Expr {
    let now = ex(
        ExprKind::Intrinsic(Intrinsic::Now, vec![]),
        Scalar::Duration { long: true },
    );
    let now = ex(ExprKind::Convert(ConvMode::Widen, Box::new(now)), I64);
    bin(BinaryOp::Div, now, int(1000, I64), I64)
}

/// `ET := min(elapsed, PT)` with the elapsed milliseconds since `START`,
/// then the statements of `done` when `ET >= PT`.
fn measure(f: &Fb, done: Vec<Stmt>) -> Vec<Stmt> {
    let elapsed = bin(
        BinaryOp::Div,
        bin(BinaryOp::Sub, now_us(), f.get("START"), I64),
        int(1000, I64),
        I64,
    );
    let elapsed = ex(ExprKind::Convert(ConvMode::Wrap, Box::new(elapsed)), I32);
    let over = bin(BinaryOp::Gt, elapsed.clone(), f.get("PT"), BOOL);
    let et = ex(
        ExprKind::Select(Box::new(over), Box::new(f.get("PT")), Box::new(elapsed)),
        I32,
    );
    vec![
        f.set("ET", et),
        if_(
            bin(BinaryOp::Ge, f.get("ET"), f.get("PT"), BOOL),
            done,
            vec![],
        ),
    ]
}

fn start(f: &Fb, q: bool) -> Vec<Stmt> {
    vec![
        f.set("START", now_us()),
        f.set("RUNNING", b(true)),
        f.set("ET", int(0, I32)),
        f.set("Q", b(q)),
    ]
}

fn ton(f: &Fb) -> Vec<Stmt> {
    vec![if_(
        f.get("IN"),
        vec![if_(
            not(f.get("RUNNING")),
            start(f, false),
            measure(f, vec![f.set("Q", b(true))]),
        )],
        vec![
            f.set("Q", b(false)),
            f.set("ET", int(0, I32)),
            f.set("RUNNING", b(false)),
        ],
    )]
}

fn tof(f: &Fb) -> Vec<Stmt> {
    vec![if_(
        f.get("IN"),
        vec![
            f.set("Q", b(true)),
            f.set("ET", int(0, I32)),
            f.set("RUNNING", b(false)),
        ],
        vec![if_(
            not(f.get("RUNNING")),
            start(f, true),
            measure(f, vec![f.set("Q", b(false))]),
        )],
    )]
}

fn tp(f: &Fb) -> Vec<Stmt> {
    vec![if_(
        f.get("RUNNING"),
        measure(f, vec![f.set("Q", b(false)), f.set("RUNNING", b(false))]),
        vec![if_(f.get("IN"), start(f, true), vec![])],
    )]
}

fn sr(f: &Fb) -> Vec<Stmt> {
    vec![f.set("Q1", or(f.get("S1"), and(not(f.get("R")), f.get("Q1"))))]
}

fn rs(f: &Fb) -> Vec<Stmt> {
    vec![f.set("Q1", and(not(f.get("R1")), or(f.get("S"), f.get("Q1"))))]
}

fn r_trig(f: &Fb) -> Vec<Stmt> {
    vec![
        f.set("Q", and(f.get("CLK"), not(f.get("M")))),
        f.set("M", f.get("CLK")),
    ]
}

fn f_trig(f: &Fb) -> Vec<Stmt> {
    vec![
        f.set("Q", and(not(f.get("CLK")), f.get("M"))),
        f.set("M", f.get("CLK")),
    ]
}

fn limits(t: Scalar) -> (i128, i128) {
    match t {
        Scalar::Int { bits, signed: true } => (-(1i128 << (bits - 1)), (1i128 << (bits - 1)) - 1),
        Scalar::Int { bits, .. } => (0, (1i128 << bits) - 1),
        _ => (0, 0),
    }
}

/// `CV := CV + 1` saturating at the largest value of its type.
fn count_up(f: &Fb) -> Stmt {
    let t = f.sc("CV");
    let (_, max) = limits(t);
    if_(
        bin(BinaryOp::Lt, f.get("CV"), int(max, t), BOOL),
        vec![f.set("CV", bin(BinaryOp::Add, f.get("CV"), int(1, t), t))],
        vec![],
    )
}

/// `CV := CV - 1` saturating at the smallest value of its type.
fn count_down(f: &Fb) -> Stmt {
    let t = f.sc("CV");
    let (min, _) = limits(t);
    if_(
        bin(BinaryOp::Gt, f.get("CV"), int(min, t), BOOL),
        vec![f.set("CV", bin(BinaryOp::Sub, f.get("CV"), int(1, t), t))],
        vec![],
    )
}

fn rising(f: &Fb, input: &str, prev: &str) -> Expr {
    and(f.get(input), not(f.get(prev)))
}

fn ctu(f: &Fb) -> Vec<Stmt> {
    let t = f.sc("CV");
    vec![
        if_(
            f.get("R"),
            vec![f.set("CV", int(0, t))],
            vec![if_(rising(f, "CU", "PREV"), vec![count_up(f)], vec![])],
        ),
        f.set("Q", bin(BinaryOp::Ge, f.get("CV"), f.get("PV"), BOOL)),
        f.set("PREV", f.get("CU")),
    ]
}

fn ctd(f: &Fb) -> Vec<Stmt> {
    let t = f.sc("CV");
    vec![
        if_(
            f.get("LD"),
            vec![f.set("CV", f.get("PV"))],
            vec![if_(rising(f, "CD", "PREV"), vec![count_down(f)], vec![])],
        ),
        f.set("Q", bin(BinaryOp::Le, f.get("CV"), int(0, t), BOOL)),
        f.set("PREV", f.get("CD")),
    ]
}

fn ctud(f: &Fb) -> Vec<Stmt> {
    let t = f.sc("CV");
    vec![
        if_(
            f.get("R"),
            vec![f.set("CV", int(0, t))],
            vec![if_(
                f.get("LD"),
                vec![f.set("CV", f.get("PV"))],
                vec![
                    if_(rising(f, "CU", "PREVU"), vec![count_up(f)], vec![]),
                    if_(rising(f, "CD", "PREVD"), vec![count_down(f)], vec![]),
                ],
            )],
        ),
        f.set("QU", bin(BinaryOp::Ge, f.get("CV"), f.get("PV"), BOOL)),
        f.set("QD", bin(BinaryOp::Le, f.get("CV"), int(0, t), BOOL)),
        f.set("PREVU", f.get("CU")),
        f.set("PREVD", f.get("CD")),
    ]
}

/// Sets the uptime globals to the cycle time in milliseconds, as the VM
/// does at the start of each round: `TIME` wraps, `LTIME` does not.
pub(crate) fn uptime(l: &Lowerer) -> Vec<Stmt> {
    let Some((time, ltime)) = &l.uptime else {
        return vec![];
    };
    let ms = bin(BinaryOp::Div, now_us(), int(1000, I64), I64);
    vec![
        stmt(StmtKind::Assign(
            Place {
                addr: time.clone(),
                ty: I32,
            },
            ex(ExprKind::Convert(ConvMode::Wrap, Box::new(ms.clone())), I32),
        )),
        stmt(StmtKind::Assign(
            Place {
                addr: ltime.clone(),
                ty: I64,
            },
            ms,
        )),
    ]
}
