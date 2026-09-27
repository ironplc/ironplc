//! Helper functions of `**` (LANG-073), written in WebAssembly so that every
//! engine computes the same result (ABI-005):
//!
//! - integer exponents: square and multiply in the type of the base, the
//!   algorithm used for constant folding;
//! - real exponents: `exp(y * ln(x))`, with `ln` by the series of
//!   `2 atanh((m - 1) / (m + 1))` after reduction of the mantissa to
//!   `[sqrt(1/2), sqrt(2))`, and `exp` by a Taylor polynomial after reduction
//!   by multiples of `ln 2`; integral real exponents use the integer
//!   algorithm.

use ironplc_wasm_ir::{walk, ExprKind, Intrinsic, Module, Visitor};
use wasm_encoder::{BlockType, Function, Instruction as I, TypeSection, ValType};

/// Which helpers a module needs.
#[derive(Debug, Clone, Copy, Default)]
pub struct Used {
    powi_f32: bool,
    powi_f64: bool,
    pow_real: bool,
    /// The module reads the cycle time (`plc_rt.now_ns`, ABI-030).
    pub now: bool,
    /// `LN`, `LOG` or `EXP` is used.
    ln_exp: bool,
}

/// Function indexes of the helpers.
#[derive(Debug, Clone, Copy, Default)]
pub struct HelperIndexes {
    pub powi_f32: Option<u32>,
    pub powi_f64: Option<u32>,
    pub pow_real: Option<u32>,
    pub ln: Option<u32>,
    pub exp: Option<u32>,
    pub count: u32,
}

/// Type indexes of the helpers.
pub struct HelperTypes {
    powi_f32: u32,
    powi_f64: u32,
    f64_f64_f64: u32,
    f64_f64: u32,
}

pub fn add_types(types: &mut TypeSection) -> HelperTypes {
    let base = types.len();
    types
        .ty()
        .function([ValType::F32, ValType::I64], [ValType::F32]);
    types
        .ty()
        .function([ValType::F64, ValType::I64], [ValType::F64]);
    types
        .ty()
        .function([ValType::F64, ValType::F64], [ValType::F64]);
    types.ty().function([ValType::F64], [ValType::F64]);
    HelperTypes {
        powi_f32: base,
        powi_f64: base + 1,
        f64_f64_f64: base + 2,
        f64_f64: base + 3,
    }
}

impl Visitor for Used {
    fn expr(&mut self, e: &ironplc_wasm_ir::Expr) {
        match &e.kind {
            ExprKind::Intrinsic(Intrinsic::Pow, args) => {
                if args[1].ty.is_real() {
                    self.pow_real = true;
                    self.powi_f64 = true;
                } else if e.ty == (ironplc_wasm_ir::Scalar::Real { bits: 32 }) {
                    self.powi_f32 = true;
                } else {
                    self.powi_f64 = true;
                }
            }
            ExprKind::Intrinsic(Intrinsic::Now, _) => self.now = true,
            ExprKind::Intrinsic(Intrinsic::Ln | Intrinsic::Log | Intrinsic::Exp, _) => {
                self.ln_exp = true
            }
            _ => {}
        }
    }
}
pub fn used(m: &Module) -> Used {
    let mut u = Used::default();
    for f in &m.functions {
        walk(&f.body, &mut u);
    }
    u
}

impl HelperIndexes {
    pub fn assign(first: u32, u: Used) -> HelperIndexes {
        let mut h = HelperIndexes::default();
        let mut next = first;
        let mut take = |flag: bool| {
            flag.then(|| {
                next += 1;
                next - 1
            })
        };
        h.powi_f32 = take(u.powi_f32);
        h.powi_f64 = take(u.powi_f64);
        h.pow_real = take(u.pow_real);
        h.ln = take(u.pow_real || u.ln_exp);
        h.exp = take(u.pow_real || u.ln_exp);
        h.count = next - first;
        h
    }
}

/// The helper functions, in the order of their indexes.
pub fn functions(h: &HelperIndexes, t: &HelperTypes) -> Vec<(u32, Function)> {
    let mut out = vec![];
    if h.powi_f32.is_some() {
        out.push((t.powi_f32, powi(true)));
    }
    if h.powi_f64.is_some() {
        out.push((t.powi_f64, powi(false)));
    }
    if let (Some(_), Some(ln), Some(exp), Some(powi64)) = (h.pow_real, h.ln, h.exp, h.powi_f64) {
        out.push((t.f64_f64_f64, pow_real(ln, exp, powi64)));
    }
    if h.ln.is_some() {
        out.push((t.f64_f64, ln_f64()));
        out.push((t.f64_f64, exp_f64()));
    }
    out
}

fn body(locals: Vec<(u32, ValType)>, ins: &[I<'static>]) -> Function {
    let mut f = Function::new(locals);
    for i in ins {
        f.instruction(i);
    }
    f.instruction(&I::End);
    f
}

fn f64c(v: f64) -> I<'static> {
    I::F64Const(v.into())
}

/// `powi(x, n)`: params x (0), n (1); locals result (2), base (3), k (4).
fn powi(single: bool) -> Function {
    let (ft, one, mul, div) = if single {
        (
            ValType::F32,
            I::F32Const(1.0f32.into()),
            I::F32Mul,
            I::F32Div,
        )
    } else {
        (ValType::F64, f64c(1.0), I::F64Mul, I::F64Div)
    };
    let ins = vec![
        one.clone(),
        I::LocalSet(2),
        I::LocalGet(0),
        I::LocalSet(3),
        // k := |n| (the minimum stays 2^63 as unsigned); no `select`
        I::LocalGet(1),
        I::I64Const(0),
        I::I64GeS,
        I::If(BlockType::Result(ValType::I64)),
        I::LocalGet(1),
        I::Else,
        I::I64Const(0),
        I::LocalGet(1),
        I::I64Sub,
        I::End,
        I::LocalSet(4),
        I::Block(BlockType::Empty),
        I::Loop(BlockType::Empty),
        I::LocalGet(4),
        I::I64Eqz,
        I::BrIf(1),
        I::LocalGet(4),
        I::I64Const(1),
        I::I64And,
        I::I32WrapI64,
        I::If(BlockType::Empty),
        I::LocalGet(2),
        I::LocalGet(3),
        mul.clone(),
        I::LocalSet(2),
        I::End,
        I::LocalGet(3),
        I::LocalGet(3),
        mul,
        I::LocalSet(3),
        I::LocalGet(4),
        I::I64Const(1),
        I::I64ShrU,
        I::LocalSet(4),
        I::Br(0),
        I::End,
        I::End,
        I::LocalGet(1),
        I::I64Const(0),
        I::I64LtS,
        I::If(BlockType::Empty),
        one,
        I::LocalGet(2),
        div,
        I::LocalSet(2),
        I::End,
        I::LocalGet(2),
    ];
    body(vec![(2, ft), (1, ValType::I64)], &ins)
}

/// `pow(x, y)` for a real exponent: params x (0), y (1).
fn pow_real(ln: u32, exp: u32, powi64: u32) -> Function {
    let ins = vec![
        // NaN in, NaN out.
        I::LocalGet(0),
        I::LocalGet(0),
        I::F64Ne,
        I::LocalGet(1),
        I::LocalGet(1),
        I::F64Ne,
        I::I32Or,
        I::If(BlockType::Empty),
        I::LocalGet(0),
        I::LocalGet(1),
        I::F64Add,
        I::Return,
        I::End,
        // Integral exponent: square and multiply.
        I::LocalGet(1),
        I::F64Trunc,
        I::LocalGet(1),
        I::F64Eq,
        I::LocalGet(1),
        I::F64Abs,
        f64c(9.0e18),
        I::F64Lt,
        I::I32And,
        I::If(BlockType::Empty),
        I::LocalGet(0),
        I::LocalGet(1),
        I::I64TruncSatF64S,
        I::Call(powi64),
        I::Return,
        I::End,
        // Negative base with a fractional exponent: NaN.
        I::LocalGet(0),
        f64c(0.0),
        I::F64Lt,
        I::If(BlockType::Empty),
        f64c(f64::NAN),
        I::Return,
        I::End,
        // Zero base: 0 for a positive exponent, +inf otherwise.
        I::LocalGet(0),
        f64c(0.0),
        I::F64Eq,
        I::If(BlockType::Empty),
        I::LocalGet(1),
        f64c(0.0),
        I::F64Gt,
        I::If(BlockType::Result(ValType::F64)),
        f64c(0.0),
        I::Else,
        f64c(f64::INFINITY),
        I::End,
        I::Return,
        I::End,
        I::LocalGet(1),
        I::LocalGet(0),
        I::Call(ln),
        I::F64Mul,
        I::Call(exp),
    ];
    body(vec![], &ins)
}

/// High part of ln 2 (its low mantissa bits are zero, so that `k * LN2_HI`
/// is exact).
const LN2_HI: f64 = f64::from_bits(0x3FE6_2E42_FEE0_0000);
/// ln 2 - LN2_HI.
const LN2_LO: f64 = f64::from_bits(0x3DEA_39EF_3579_3C76);

/// `ln(x)`: NaN for NaN and negative values, -inf for 0; param x (0); locals
/// bits (1: i64), e (2: f64), m (3), s (4), z (5), p (6).
fn ln_f64() -> Function {
    let mut ins = vec![
        I::LocalGet(0),
        I::LocalGet(0),
        I::F64Ne,
        I::If(BlockType::Empty),
        I::LocalGet(0),
        I::Return,
        I::End,
        I::LocalGet(0),
        f64c(0.0),
        I::F64Lt,
        I::If(BlockType::Empty),
        f64c(f64::NAN),
        I::Return,
        I::End,
        I::LocalGet(0),
        f64c(0.0),
        I::F64Eq,
        I::If(BlockType::Empty),
        f64c(f64::NEG_INFINITY),
        I::Return,
        I::End,
        I::LocalGet(0),
        f64c(f64::INFINITY),
        I::F64Eq,
        I::If(BlockType::Empty),
        I::LocalGet(0),
        I::Return,
        I::End,
        f64c(0.0),
        I::LocalSet(2),
        // Subnormals: scale by 2^54.
        I::LocalGet(0),
        f64c(f64::MIN_POSITIVE),
        I::F64Lt,
        I::If(BlockType::Empty),
        I::LocalGet(0),
        f64c(18_014_398_509_481_984.0),
        I::F64Mul,
        I::LocalSet(0),
        f64c(-54.0),
        I::LocalSet(2),
        I::End,
        I::LocalGet(0),
        I::I64ReinterpretF64,
        I::LocalSet(1),
        // e += exponent - 1023
        I::LocalGet(2),
        I::LocalGet(1),
        I::I64Const(52),
        I::I64ShrU,
        I::I64Const(0x7FF),
        I::I64And,
        I::I64Const(1023),
        I::I64Sub,
        I::F64ConvertI64S,
        I::F64Add,
        I::LocalSet(2),
        // m in [1, 2)
        I::LocalGet(1),
        I::I64Const(0x000F_FFFF_FFFF_FFFF),
        I::I64And,
        I::I64Const(0x3FF0_0000_0000_0000),
        I::I64Or,
        I::F64ReinterpretI64,
        I::LocalSet(3),
        // m in [sqrt(1/2), sqrt(2))
        I::LocalGet(3),
        f64c(std::f64::consts::SQRT_2),
        I::F64Ge,
        I::If(BlockType::Empty),
        I::LocalGet(3),
        f64c(0.5),
        I::F64Mul,
        I::LocalSet(3),
        I::LocalGet(2),
        f64c(1.0),
        I::F64Add,
        I::LocalSet(2),
        I::End,
        // s = (m - 1) / (m + 1), z = s * s
        I::LocalGet(3),
        f64c(1.0),
        I::F64Sub,
        I::LocalGet(3),
        f64c(1.0),
        I::F64Add,
        I::F64Div,
        I::LocalTee(4),
        I::LocalGet(4),
        I::F64Mul,
        I::LocalSet(5),
        f64c(2.0 / 23.0),
        I::LocalSet(6),
    ];
    for k in (1..=21).rev().step_by(2) {
        ins.extend([
            I::LocalGet(6),
            I::LocalGet(5),
            I::F64Mul,
            f64c(2.0 / k as f64),
            I::F64Add,
            I::LocalSet(6),
        ]);
    }
    ins.extend([
        // e * ln2_hi + (e * ln2_lo + s * p)
        I::LocalGet(2),
        f64c(LN2_HI),
        I::F64Mul,
        I::LocalGet(2),
        f64c(LN2_LO),
        I::F64Mul,
        I::LocalGet(4),
        I::LocalGet(6),
        I::F64Mul,
        I::F64Add,
        I::F64Add,
    ]);
    body(vec![(1, ValType::I64), (5, ValType::F64)], &ins)
}

/// `exp(x)`: param x (0); locals k (1), r (2), p (3), k1 (4: i64), k2 (5: i64).
fn exp_f64() -> Function {
    let mut ins = vec![
        I::LocalGet(0),
        I::LocalGet(0),
        I::F64Ne,
        I::If(BlockType::Empty),
        I::LocalGet(0),
        I::Return,
        I::End,
        I::LocalGet(0),
        f64c(709.782_712_893_384),
        I::F64Gt,
        I::If(BlockType::Empty),
        f64c(f64::INFINITY),
        I::Return,
        I::End,
        I::LocalGet(0),
        f64c(-745.133_219_101_941_1),
        I::F64Lt,
        I::If(BlockType::Empty),
        f64c(0.0),
        I::Return,
        I::End,
        // k = nearest(x / ln 2), r = x - k ln2_hi - k ln2_lo
        I::LocalGet(0),
        f64c(std::f64::consts::LOG2_E),
        I::F64Mul,
        I::F64Nearest,
        I::LocalSet(1),
        I::LocalGet(0),
        I::LocalGet(1),
        f64c(LN2_HI),
        I::F64Mul,
        I::F64Sub,
        I::LocalGet(1),
        f64c(LN2_LO),
        I::F64Mul,
        I::F64Sub,
        I::LocalSet(2),
    ];
    // Taylor polynomial of degree 13, Horner form.
    let mut fact = [1.0f64; 14];
    for i in 1..14 {
        fact[i] = fact[i - 1] * i as f64;
    }
    ins.extend([f64c(1.0 / fact[13]), I::LocalSet(3)]);
    for n in (0..13).rev() {
        ins.extend([
            I::LocalGet(3),
            I::LocalGet(2),
            I::F64Mul,
            f64c(1.0 / fact[n]),
            I::F64Add,
            I::LocalSet(3),
        ]);
    }
    ins.extend([
        // 2^k as 2^k1 * 2^k2 with k1 = k / 2, k2 = k - k1, both normal.
        I::LocalGet(1),
        I::I64TruncSatF64S,
        I::LocalTee(5),
        I::I64Const(2),
        I::I64DivS,
        I::LocalSet(4),
        I::LocalGet(5),
        I::LocalGet(4),
        I::I64Sub,
        I::LocalSet(5),
        I::LocalGet(3),
        I::LocalGet(4),
        I::I64Const(1023),
        I::I64Add,
        I::I64Const(52),
        I::I64Shl,
        I::F64ReinterpretI64,
        I::F64Mul,
        I::LocalGet(5),
        I::I64Const(1023),
        I::I64Add,
        I::I64Const(52),
        I::I64Shl,
        I::F64ReinterpretI64,
        I::F64Mul,
    ]);
    body(vec![(3, ValType::F64), (2, ValType::I64)], &ins)
}
