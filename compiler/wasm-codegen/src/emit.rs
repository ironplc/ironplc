//! Emission of function bodies: expressions, structured statements, calls,
//! division checks (ABI-080), debug hooks (ABI-032) and fuel (ABI-070).
//!
//! Values narrower than 32 bits live in `i32` locals and stack slots in
//! normalised form: sign-extended for signed types, zero-extended for
//! unsigned types and bit strings; every operation that can leave the range
//! of its type is followed by a normalisation, which makes integer
//! arithmetic wrap modulo 2^n.

use std::borrow::Cow;

use ironplc_wasm_ir::{
    walk, Addr, Base, BinaryOp, Call, Const, ConvMode, Copy, Expr, ExprKind, FuncKind,
    Function as IrFunction, Intrinsic, Label, Module, Place, Scalar, Span, Stmt, StmtKind, StrOp,
    SwitchArm, UnOp, Visitor,
};
use ironplc_wasm_symbols::Site;
use wasm_encoder::{BlockType, Function, Instruction as I, MemArg, ValType};

use crate::{Indexes, Layout};

/// Objects reset during execution: frames of functions and `VAR_TEMP`.
pub fn resettable(m: &Module, flags: &mut [bool]) {
    struct Reset<'a>(&'a mut [bool]);
    impl Visitor for Reset<'_> {
        fn stmt(&mut self, s: &Stmt) {
            if let StmtKind::Reset(o) = s.kind {
                self.0[o as usize] = true;
            }
        }
        fn call(&mut self, c: &Call) {
            if let Some(f) = c.frame {
                self.0[f as usize] = true;
            }
        }
    }
    let mut v = Reset(flags);
    for f in &m.functions {
        walk(&f.body, &mut v);
    }
}

/// A function made of a fixed instruction sequence.
pub fn simple(ins: &[I<'static>]) -> Function {
    let mut f = Function::new([]);
    for i in ins {
        f.instruction(i);
    }
    f.instruction(&I::End);
    f
}

/// `plc_init`: resets the trap code and every region (ABI-011, ABI-081).
pub fn init(lay: &Layout, segments: &[Option<u32>; 5], ix: &Indexes) -> Function {
    let mut f = Function::new([]);
    f.instruction(&I::I32Const(0));
    f.instruction(&I::GlobalSet(ix.trap_code));
    for (ri, (base, size)) in lay.regions.iter().enumerate() {
        if *size == 0 {
            continue;
        }
        f.instruction(&I::I32Const(*base as i32));
        f.instruction(&I::I32Const(0));
        f.instruction(&I::I32Const(*size as i32));
        match segments[ri] {
            Some(seg) => f.instruction(&I::MemoryInit {
                mem: 0,
                data_index: seg,
            }),
            None => f.instruction(&I::MemoryFill(0)),
        };
    }
    f.instruction(&I::I32Const(0));
    f.instruction(&I::End);
    f
}

/// `plc_task_run(task)`: runs the programs of a task (ABI-012).
pub fn task_run(m: &Module, ix: &Indexes) -> Function {
    let mut f = Function::new([]);
    f.instruction(&I::I32Const(0));
    f.instruction(&I::GlobalSet(ix.trap_code));
    for (i, t) in m.tasks.iter().enumerate() {
        f.instruction(&I::LocalGet(0));
        f.instruction(&I::I32Const(i as i32));
        f.instruction(&I::I32Eq);
        f.instruction(&I::If(BlockType::Empty));
        for (func, _) in &t.programs {
            f.instruction(&I::Call(ix.first_ir_func + func));
        }
        f.instruction(&I::I32Const(0));
        f.instruction(&I::Return);
        f.instruction(&I::End);
    }
    f.instruction(&I::I32Const(-1));
    f.instruction(&I::End);
    f
}

pub(crate) fn valtype(t: Scalar) -> ValType {
    match t {
        Scalar::Real { bits: 32 } => ValType::F32,
        Scalar::Real { .. } => ValType::F64,
        t if t.is_wide() => ValType::I64,
        _ => ValType::I32,
    }
}

fn is_signed_int(t: Scalar) -> bool {
    matches!(
        t,
        Scalar::Int { signed: true, .. } | Scalar::Duration { .. }
    )
}

enum Ins {
    I(I<'static>),
    /// Fuel decrement for a region, with the site of the trap.
    Charge {
        region: usize,
        site: u32,
    },
}

struct Em<'a> {
    lay: &'a Layout,
    ix: &'a Indexes,
    m: &'a Module,
    sites: &'a mut Vec<Site>,
    code: Vec<Ins>,
    nparams: u32,
    locals: Vec<ValType>,
    free: Vec<(ValType, u32)>,
    temps: Vec<u32>,
    /// Control stack: IR label and whether the entry is the start of a loop.
    ctrl: Vec<Option<(Label, bool)>>,
    regions: Vec<usize>,
    counts: Vec<u32>,
    loop_regions: Vec<(Label, usize, u32)>,
}

/// Emits the body of an IR function.
pub fn function(
    m: &Module,
    f: &IrFunction,
    lay: &Layout,
    ix: &Indexes,
    sites: &mut Vec<Site>,
) -> Result<Function, String> {
    let nparams = if f.kind == FuncKind::FunctionBlock {
        3
    } else {
        0
    };
    let mut em = Em {
        lay,
        ix,
        m,
        sites,
        code: vec![],
        nparams,
        locals: vec![],
        free: vec![],
        temps: vec![],
        ctrl: vec![],
        regions: vec![0],
        counts: vec![0],
        loop_regions: vec![],
    };
    for t in &f.temps {
        let l = em.new_local(valtype(*t));
        em.temps.push(l);
    }
    if ix.fuel.is_some() {
        let site = em.site(f.span);
        em.code.push(Ins::Charge { region: 0, site });
    }
    em.stmts(&f.body)?;
    Ok(em.finish())
}

fn mem(offset: u64, size: u32) -> MemArg {
    MemArg {
        offset,
        align: size.trailing_zeros(),
        memory_index: 0,
    }
}

impl Em<'_> {
    fn emit(&mut self, i: I<'static>) {
        self.code.push(Ins::I(i));
        let r = *self.regions.last().expect("a fuel region");
        self.counts[r] += 1;
    }

    fn site(&mut self, s: Span) -> u32 {
        let site = Site {
            file: s.file,
            start: s.start,
            end: s.end,
        };
        if let Some(i) = self.sites.iter().position(|x| *x == site) {
            return i as u32;
        }
        self.sites.push(site);
        (self.sites.len() - 1) as u32
    }

    fn new_local(&mut self, t: ValType) -> u32 {
        self.locals.push(t);
        self.nparams + self.locals.len() as u32 - 1
    }

    fn scratch(&mut self, t: ValType) -> u32 {
        if let Some(i) = self.free.iter().position(|(ty, _)| *ty == t) {
            return self.free.swap_remove(i).1;
        }
        self.new_local(t)
    }

    fn release(&mut self, t: ValType, l: u32) {
        self.free.push((t, l));
    }

    fn finish(self) -> Function {
        let mut groups: Vec<(u32, ValType)> = vec![];
        for t in &self.locals {
            match groups.last_mut() {
                Some((n, ty)) if ty == t => *n += 1,
                _ => groups.push((1, *t)),
            }
        }
        let mut f = Function::new(groups);
        for ins in &self.code {
            match ins {
                Ins::I(i) => {
                    f.instruction(i);
                }
                Ins::Charge { region, site } => {
                    let fuel = self.ix.fuel.expect("fuel global");
                    f.instruction(&I::GlobalGet(fuel));
                    f.instruction(&I::I64Const(self.counts[*region] as i64));
                    f.instruction(&I::I64Sub);
                    f.instruction(&I::GlobalSet(fuel));
                    f.instruction(&I::GlobalGet(fuel));
                    f.instruction(&I::I64Const(0));
                    f.instruction(&I::I64LtS);
                    f.instruction(&I::If(BlockType::Empty));
                    f.instruction(&I::I32Const(1));
                    f.instruction(&I::GlobalSet(self.ix.trap_code));
                    f.instruction(&I::I32Const(*site as i32));
                    f.instruction(&I::GlobalSet(self.ix.trap_site));
                    f.instruction(&I::Unreachable);
                    f.instruction(&I::End);
                }
            }
        }
        f.instruction(&I::End);
        f
    }

    // ----- memory

    /// Pushes the dynamic part of an address; returns the static offset.
    fn addr(&mut self, a: &Addr) -> Result<u64, String> {
        Ok(match &a.base {
            Base::Object(o) => {
                self.emit(I::I32Const((self.lay.addr[*o as usize] + a.offset) as i32));
                0
            }
            Base::SelfPart(k) => {
                self.emit(I::LocalGet(*k as u32));
                a.offset as u64
            }
            Base::Deref(inner) => {
                let off = self.addr(inner)?;
                self.emit(I::I32Load(mem(off, 4)));
                a.offset as u64
            }
            Base::Index(inner, e) => {
                self.addr_value(inner)?;
                self.expr(e)?;
                self.emit(I::I32Add);
                a.offset as u64
            }
        })
    }

    fn addr_value(&mut self, a: &Addr) -> Result<(), String> {
        let off = self.addr(a)?;
        if off != 0 {
            self.emit(I::I32Const(off as i32));
            self.emit(I::I32Add);
        }
        Ok(())
    }

    /// `dst := src` for strings, truncated to `cap` characters.
    fn str_assign(&mut self, dst: &Addr, cap: u32, src: &Addr, wide: bool) -> Result<(), String> {
        let k = self.ix.strings.get(wide)?;
        self.addr_value(dst)?;
        self.emit(I::I32Const(cap as i32));
        self.addr_value(src)?;
        self.emit(I::Call(k.assign));
        Ok(())
    }

    /// String operations by the helpers of `strings.rs`.
    fn str_op(&mut self, op: &StrOp, wide: bool) -> Result<(), String> {
        let k = self.ix.strings.get(wide)?;
        match op {
            StrOp::Assign { dst, cap, src } => self.str_assign(dst, *cap, src, wide)?,
            StrOp::Convert { dst, cap, src } => {
                self.addr_value(dst)?;
                self.emit(I::I32Const(*cap as i32));
                self.addr_value(src)?;
                self.emit(I::Call(k.convert));
            }
            StrOp::SetChar { dst, value } => {
                let t = if wide {
                    Scalar::Bits { bits: 16 }
                } else {
                    Scalar::Bits { bits: 8 }
                };
                let a = self.scratch(ValType::I32);
                self.addr_value(dst)?;
                self.emit(I::LocalTee(a));
                self.expr(value)?;
                self.store(t, 0);
                self.emit(I::LocalGet(a));
                self.emit(I::I32Const(0));
                self.store(t, t.size() as u64);
                self.release(ValType::I32, a);
            }
            StrOp::Clear(d) => {
                let off = self.addr(d)?;
                self.emit(I::I32Const(0));
                let t = if wide {
                    Scalar::Bits { bits: 16 }
                } else {
                    Scalar::Bits { bits: 8 }
                };
                self.store(t, off);
            }
            StrOp::Append {
                dst,
                cap,
                src,
                start,
                count,
            } => {
                self.addr_value(dst)?;
                self.emit(I::I32Const(*cap as i32));
                self.addr_value(src)?;
                self.expr(start)?;
                self.expr(count)?;
                self.emit(I::Call(k.append));
            }
            StrOp::Splice {
                dst,
                cap,
                src,
                pos,
                len,
                ins,
            } => {
                self.addr_value(dst)?;
                self.emit(I::I32Const(*cap as i32));
                self.addr_value(src)?;
                self.expr(pos)?;
                self.expr(len)?;
                match ins {
                    Some(i) => self.addr_value(i)?,
                    None => self.emit(I::I32Const(0)),
                }
                self.emit(I::Call(k.splice));
            }
        }
        Ok(())
    }

    /// `memory.copy` of `size` bytes (bulk memory, ABI-001).
    fn copy(&mut self, c: &Copy) -> Result<(), String> {
        self.addr_value(&c.dst)?;
        self.addr_value(&c.src)?;
        self.emit(I::I32Const(c.size as i32));
        self.emit(I::MemoryCopy {
            src_mem: 0,
            dst_mem: 0,
        });
        Ok(())
    }

    /// Byte offset of an array element in one dimension: the
    /// index minus the lower bound, in 64 bits, checked against the count
    /// as an unsigned number (code 3), times the stride.
    fn index(
        &mut self,
        index: &Expr,
        low: i64,
        count: u64,
        stride: u32,
        site: Option<u32>,
    ) -> Result<(), String> {
        self.expr(index)?;
        let t = index.ty;
        if !t.is_wide() {
            self.emit(if is_signed_int(t) {
                I::I64ExtendI32S
            } else {
                I::I64ExtendI32U
            });
        }
        if low != 0 {
            self.emit(I::I64Const(low));
            self.emit(I::I64Sub);
        }
        if let Some(site) = site {
            let l = self.scratch(ValType::I64);
            self.emit(I::LocalTee(l));
            self.emit(I::I64Const(count as i64));
            self.emit(I::I64GeU);
            self.emit(I::If(BlockType::Empty));
            self.trap(3, site);
            self.emit(I::End);
            self.emit(I::LocalGet(l));
            self.release(ValType::I64, l);
        }
        self.emit(I::I32WrapI64);
        if stride != 1 {
            self.emit(I::I32Const(stride as i32));
            self.emit(I::I32Mul);
        }
        Ok(())
    }

    /// A value checked to be in a subrange (code 4).
    fn checked(&mut self, a: &Expr, low: i128, high: i128, site: u32) -> Result<(), String> {
        self.expr(a)?;
        let t = a.ty;
        let vt = valtype(t);
        let l = self.scratch(vt);
        self.emit(I::LocalTee(l));
        let wide = t.is_wide();
        let signed = is_signed_int(t);
        self.int_const(t, low as i64);
        self.emit(match (wide, signed) {
            (true, true) => I::I64LtS,
            (true, false) => I::I64LtU,
            (false, true) => I::I32LtS,
            (false, false) => I::I32LtU,
        });
        self.emit(I::LocalGet(l));
        self.int_const(t, high as i64);
        self.emit(match (wide, signed) {
            (true, true) => I::I64GtS,
            (true, false) => I::I64GtU,
            (false, true) => I::I32GtS,
            (false, false) => I::I32GtU,
        });
        self.emit(I::I32Or);
        self.emit(I::If(BlockType::Empty));
        self.trap(4, site);
        self.emit(I::End);
        self.emit(I::LocalGet(l));
        self.release(vt, l);
        Ok(())
    }

    fn load(&mut self, t: Scalar, off: u64) {
        let m = mem(off, t.size());
        let i = match t {
            Scalar::Bool
            | Scalar::Int {
                bits: 8,
                signed: false,
            }
            | Scalar::Bits { bits: 8 } => I::I32Load8U(m),
            Scalar::Int {
                bits: 8,
                signed: true,
            } => I::I32Load8S(m),
            Scalar::Int {
                bits: 16,
                signed: true,
            } => I::I32Load16S(m),
            Scalar::Int { bits: 16, .. } | Scalar::Bits { bits: 16 } => I::I32Load16U(m),
            Scalar::Real { bits: 32 } => I::F32Load(m),
            Scalar::Real { .. } => I::F64Load(m),
            t if t.is_wide() => I::I64Load(m),
            _ => I::I32Load(m),
        };
        self.emit(i);
    }

    fn store(&mut self, t: Scalar, off: u64) {
        let m = mem(off, t.size());
        let i = match t {
            Scalar::Real { bits: 32 } => I::F32Store(m),
            Scalar::Real { .. } => I::F64Store(m),
            t if t.is_wide() => I::I64Store(m),
            t if t.size() == 1 => I::I32Store8(m),
            t if t.size() == 2 => I::I32Store16(m),
            _ => I::I32Store(m),
        };
        self.emit(i);
    }

    fn reset(&mut self, o: u32) {
        let size = self.m.objects[o as usize].init.len() as i32;
        if size == 0 {
            return;
        }
        self.emit(I::I32Const(self.lay.addr[o as usize] as i32));
        self.emit(I::I32Const(0));
        self.emit(I::I32Const(size));
        match self.ix.segments[o as usize] {
            Some(seg) => self.emit(I::MemoryInit {
                mem: 0,
                data_index: seg,
            }),
            None => self.emit(I::MemoryFill(0)),
        }
    }

    // ----- values

    fn normalize(&mut self, t: Scalar) {
        match t {
            Scalar::Int {
                bits: 8,
                signed: true,
            } => self.emit(I::I32Extend8S),
            Scalar::Int {
                bits: 16,
                signed: true,
            } => self.emit(I::I32Extend16S),
            Scalar::Int {
                bits: 8,
                signed: false,
            }
            | Scalar::Bits { bits: 8 } => {
                self.emit(I::I32Const(0xFF));
                self.emit(I::I32And);
            }
            Scalar::Int {
                bits: 16,
                signed: false,
            }
            | Scalar::Bits { bits: 16 } => {
                self.emit(I::I32Const(0xFFFF));
                self.emit(I::I32And);
            }
            _ => {}
        }
    }

    fn konst(&mut self, c: Const, t: Scalar) {
        let i = match (c, t) {
            (_, Scalar::Real { bits: 32 }) => I::F32Const(
                (match c {
                    Const::Real(r) => r as f32,
                    Const::Int(i) => i as f32,
                    _ => 0.0,
                })
                .into(),
            ),
            (_, Scalar::Real { .. }) => I::F64Const(
                (match c {
                    Const::Real(r) => r,
                    Const::Int(i) => i as f64,
                    _ => 0.0,
                })
                .into(),
            ),
            (c, t) => {
                let v: i128 = match c {
                    Const::Bool(b) => b as i128,
                    Const::Int(i) => i,
                    Const::Duration(d) => d as i128,
                    Const::Real(r) => r as i128,
                };
                if t.is_wide() {
                    I::I64Const(v as i64)
                } else {
                    I::I32Const(v as i32)
                }
            }
        };
        self.emit(i);
    }

    /// Integer constant in the machine type of `t`.
    fn int_const(&mut self, t: Scalar, v: i64) {
        if t.is_wide() {
            self.emit(I::I64Const(v));
        } else {
            self.emit(I::I32Const(v as i32));
        }
    }

    /// Chooses between two values by a condition, all three on the stack
    /// (first value, second value, condition), without the `select`
    /// instruction, which wasmi 2.0.0 miscompiles when the condition comes
    /// from `i32.eqz`: the values go to locals and an `if` with a
    /// result picks one.
    fn choose(&mut self, vt: ValType) {
        let c = self.scratch(ValType::I32);
        let second = self.scratch(vt);
        let first = self.scratch(vt);
        self.emit(I::LocalSet(c));
        self.emit(I::LocalSet(second));
        self.emit(I::LocalSet(first));
        self.emit(I::LocalGet(c));
        self.emit(I::If(BlockType::Result(vt)));
        self.emit(I::LocalGet(first));
        self.emit(I::Else);
        self.emit(I::LocalGet(second));
        self.emit(I::End);
        self.release(ValType::I32, c);
        self.release(vt, second);
        self.release(vt, first);
    }

    fn trap(&mut self, code: i32, site: u32) {
        self.emit(I::I32Const(code));
        self.emit(I::GlobalSet(self.ix.trap_code));
        self.emit(I::I32Const(site as i32));
        self.emit(I::GlobalSet(self.ix.trap_site));
        self.emit(I::Unreachable);
    }

    fn expr(&mut self, e: &Expr) -> Result<(), String> {
        let t = e.ty;
        match &e.kind {
            ExprKind::Const(c) => self.konst(*c, t),
            ExprKind::Load(p) => {
                let off = self.addr(&p.addr)?;
                self.load(p.ty, off);
            }
            ExprKind::Temp(i) => self.emit(I::LocalGet(self.temps[*i as usize])),
            ExprKind::AddrOf(a) => self.addr_value(a)?,
            ExprKind::Index {
                index,
                low,
                count,
                stride,
                site,
            } => self.index(index, *low, *count, *stride, *site)?,
            ExprKind::Checked(a, low, high, site) => self.checked(a, *low, *high, *site)?,
            ExprKind::Seq(pre, e) => {
                self.stmts(pre)?;
                self.expr(e)?;
            }
            ExprKind::StrChar(a, w) => {
                let off = self.addr(a)?;
                let t = if *w {
                    Scalar::Bits { bits: 16 }
                } else {
                    Scalar::Bits { bits: 8 }
                };
                self.load(t, off);
            }
            ExprKind::StrLen(a, w) => {
                let k = self.ix.strings.get(*w)?;
                self.addr_value(a)?;
                self.emit(I::Call(k.len));
            }
            ExprKind::StrFind(a, b, w) | ExprKind::StrCmp(a, b, w) => {
                let k = self.ix.strings.get(*w)?;
                self.addr_value(a)?;
                self.addr_value(b)?;
                let f = if matches!(e.kind, ExprKind::StrFind(..)) {
                    k.find
                } else {
                    k.cmp
                };
                self.emit(I::Call(f));
            }
            ExprKind::Unary(op, a) => {
                self.expr(a)?;
                self.unary(*op, t);
            }
            ExprKind::Binary(op, a, b, site) => self.binary(*op, a, b, *site)?,
            ExprKind::Convert(mode, a) => {
                self.expr(a)?;
                self.convert(a.ty, t, *mode);
            }
            ExprKind::Select(c, a, b) => {
                self.expr(a)?;
                self.expr(b)?;
                self.expr(c)?;
                self.choose(valtype(t));
            }
            ExprKind::Intrinsic(i, args) => self.intrinsic(*i, t, args)?,
            ExprKind::Call(c) => {
                self.call(c)?;
                let r = c.result.as_ref().ok_or("call without result")?;
                let off = self.addr(&r.addr)?;
                self.load(r.ty, off);
            }
        }
        Ok(())
    }

    fn unary(&mut self, op: UnOp, t: Scalar) {
        match (op, t) {
            (UnOp::Neg, Scalar::Real { bits: 32 }) => self.emit(I::F32Neg),
            (UnOp::Neg, Scalar::Real { .. }) => self.emit(I::F64Neg),
            (UnOp::Neg, t) => {
                self.int_const(t, -1);
                self.emit(if t.is_wide() { I::I64Mul } else { I::I32Mul });
                self.normalize(t);
            }
            (UnOp::Not, Scalar::Bool) => self.emit(I::I32Eqz),
            (UnOp::Not, t) if t.is_wide() => {
                self.emit(I::I64Const(-1));
                self.emit(I::I64Xor);
            }
            (UnOp::Not, t) => {
                let mask = match t.size() {
                    1 => 0xFF,
                    2 => 0xFFFF,
                    _ => -1,
                };
                self.emit(I::I32Const(mask));
                self.emit(I::I32Xor);
            }
        }
    }

    fn compare(&mut self, op: BinaryOp, t: Scalar) {
        use BinaryOp::*;
        let signed = is_signed_int(t);
        let i = match t {
            Scalar::Real { bits: 32 } => match op {
                Eq => I::F32Eq,
                Ne => I::F32Ne,
                Lt => I::F32Lt,
                Gt => I::F32Gt,
                Le => I::F32Le,
                _ => I::F32Ge,
            },
            Scalar::Real { .. } => match op {
                Eq => I::F64Eq,
                Ne => I::F64Ne,
                Lt => I::F64Lt,
                Gt => I::F64Gt,
                Le => I::F64Le,
                _ => I::F64Ge,
            },
            t if t.is_wide() => match (op, signed) {
                (Eq, _) => I::I64Eq,
                (Ne, _) => I::I64Ne,
                (Lt, true) => I::I64LtS,
                (Lt, false) => I::I64LtU,
                (Gt, true) => I::I64GtS,
                (Gt, false) => I::I64GtU,
                (Le, true) => I::I64LeS,
                (Le, false) => I::I64LeU,
                (_, true) => I::I64GeS,
                (_, false) => I::I64GeU,
            },
            _ => match (op, signed) {
                (Eq, _) => I::I32Eq,
                (Ne, _) => I::I32Ne,
                (Lt, true) => I::I32LtS,
                (Lt, false) => I::I32LtU,
                (Gt, true) => I::I32GtS,
                (Gt, false) => I::I32GtU,
                (Le, true) => I::I32LeS,
                (Le, false) => I::I32LeU,
                (_, true) => I::I32GeS,
                (_, false) => I::I32GeU,
            },
        };
        self.emit(i);
    }

    fn binary(
        &mut self,
        op: BinaryOp,
        a: &Expr,
        b: &Expr,
        site: Option<u32>,
    ) -> Result<(), String> {
        use BinaryOp::*;
        let t = a.ty;
        if matches!(op, Eq | Ne | Lt | Gt | Le | Ge) {
            self.expr(a)?;
            self.expr(b)?;
            self.compare(op, t);
            return Ok(());
        }
        if let (Div | Mod, Some(site), false) = (op, site, t.is_real()) {
            return self.checked_division(op, a, b, site);
        }
        self.expr(a)?;
        self.expr(b)?;
        let wide = t.is_wide();
        let signed = is_signed_int(t);
        let i = match (t, op) {
            (Scalar::Real { bits: 32 }, Add) => I::F32Add,
            (Scalar::Real { bits: 32 }, Sub) => I::F32Sub,
            (Scalar::Real { bits: 32 }, Mul) => I::F32Mul,
            (Scalar::Real { bits: 32 }, Div) => I::F32Div,
            (Scalar::Real { .. }, Add) => I::F64Add,
            (Scalar::Real { .. }, Sub) => I::F64Sub,
            (Scalar::Real { .. }, Mul) => I::F64Mul,
            (Scalar::Real { .. }, Div) => I::F64Div,
            (_, Add) if wide => I::I64Add,
            (_, Sub) if wide => I::I64Sub,
            (_, Mul) if wide => I::I64Mul,
            (_, Div) if wide => {
                if signed {
                    I::I64DivS
                } else {
                    I::I64DivU
                }
            }
            (_, Mod) if wide => {
                if signed {
                    I::I64RemS
                } else {
                    I::I64RemU
                }
            }
            (_, And) if wide => I::I64And,
            (_, Or) if wide => I::I64Or,
            (_, Xor) if wide => I::I64Xor,
            (_, Add) => I::I32Add,
            (_, Sub) => I::I32Sub,
            (_, Mul) => I::I32Mul,
            (_, Div) => {
                if signed {
                    I::I32DivS
                } else {
                    I::I32DivU
                }
            }
            (_, Mod) => {
                if signed {
                    I::I32RemS
                } else {
                    I::I32RemU
                }
            }
            (_, And) => I::I32And,
            (_, Or) => I::I32Or,
            (_, Xor) => I::I32Xor,
            (_, op) => return Err(format!("operator {op:?} on {}", t.name())),
        };
        self.emit(i);
        if matches!(op, Add | Sub | Mul | Div | Mod) {
            self.normalize(t);
        }
        Ok(())
    }

    /// Integer `/` and `MOD` with the checks of ABI section 10 (codes 2
    /// and 5).
    fn checked_division(
        &mut self,
        op: BinaryOp,
        a: &Expr,
        b: &Expr,
        site: u32,
    ) -> Result<(), String> {
        let t = a.ty;
        let vt = valtype(t);
        let wide = t.is_wide();
        let signed = is_signed_int(t);
        self.expr(a)?;
        let la = self.scratch(vt);
        self.emit(I::LocalSet(la));
        self.expr(b)?;
        let lb = self.scratch(vt);
        self.emit(I::LocalSet(lb));
        self.emit(I::LocalGet(lb));
        self.emit(if wide { I::I64Eqz } else { I::I32Eqz });
        self.emit(I::If(BlockType::Empty));
        self.trap(2, site);
        self.emit(I::End);
        if signed && op == BinaryOp::Div {
            let min: i64 = match t {
                Scalar::Int { bits, .. } if bits < 64 => -(1i64 << (bits - 1)),
                _ => i64::MIN,
            };
            self.emit(I::LocalGet(la));
            self.int_const(t, min);
            self.emit(if wide { I::I64Eq } else { I::I32Eq });
            self.emit(I::LocalGet(lb));
            self.int_const(t, -1);
            self.emit(if wide { I::I64Eq } else { I::I32Eq });
            self.emit(I::I32And);
            self.emit(I::If(BlockType::Empty));
            self.trap(5, site);
            self.emit(I::End);
        }
        self.emit(I::LocalGet(la));
        self.emit(I::LocalGet(lb));
        let i = match (op, wide, signed) {
            (BinaryOp::Div, true, true) => I::I64DivS,
            (BinaryOp::Div, true, false) => I::I64DivU,
            (BinaryOp::Div, false, true) => I::I32DivS,
            (BinaryOp::Div, false, false) => I::I32DivU,
            (_, true, true) => I::I64RemS,
            (_, true, false) => I::I64RemU,
            (_, false, true) => I::I32RemS,
            (_, false, false) => I::I32RemU,
        };
        self.emit(i);
        self.normalize(t);
        self.release(vt, la);
        self.release(vt, lb);
        Ok(())
    }

    /// Conversions: the operand is on the stack.
    fn convert(&mut self, from: Scalar, to: Scalar, mode: ConvMode) {
        let (fr, tr) = (from.is_real(), to.is_real());
        match mode {
            // Nanoseconds since midnight, modulo one day, and the day
            //; dates are 64-bit.
            ConvMode::TimePart | ConvMode::DatePart => {
                const DAY: i64 = 86_400_000_000_000;
                let x = self.scratch(ValType::I64);
                let r = self.scratch(ValType::I64);
                self.emit(I::LocalTee(x));
                self.emit(I::I64Const(DAY));
                self.emit(I::I64RemS);
                self.emit(I::LocalTee(r));
                self.emit(I::I64Const(0));
                self.emit(I::I64LtS);
                self.emit(I::If(BlockType::Result(ValType::I64)));
                self.emit(I::LocalGet(r));
                self.emit(I::I64Const(DAY));
                self.emit(I::I64Add);
                self.emit(I::Else);
                self.emit(I::LocalGet(r));
                self.emit(I::End);
                if mode == ConvMode::DatePart {
                    self.emit(I::LocalSet(r));
                    self.emit(I::LocalGet(x));
                    self.emit(I::LocalGet(r));
                    self.emit(I::I64Sub);
                }
                self.release(ValType::I64, x);
                self.release(ValType::I64, r);
            }
            ConvMode::NotZero => {
                self.emit(if from.is_wide() { I::I64Eqz } else { I::I32Eqz });
                self.emit(I::I32Eqz);
            }
            _ if !fr && !tr => self.int_to_int(from, to),
            _ if !fr && tr => self.int_to_real(from, to),
            _ if fr && tr => match (from, to) {
                (Scalar::Real { bits: 32 }, Scalar::Real { bits: 64 }) => {
                    self.emit(I::F64PromoteF32)
                }
                (Scalar::Real { bits: 64 }, Scalar::Real { bits: 32 }) => {
                    self.emit(I::F32DemoteF64)
                }
                _ => {}
            },
            _ => self.real_to_int(from, to, mode == ConvMode::Round),
        }
    }

    fn int_to_int(&mut self, from: Scalar, to: Scalar) {
        match (from.is_wide(), to.is_wide()) {
            (false, false) => self.normalize(to),
            (false, true) => self.emit(if is_signed_int(from) {
                I::I64ExtendI32S
            } else {
                I::I64ExtendI32U
            }),
            (true, false) => {
                self.emit(I::I32WrapI64);
                self.normalize(to);
            }
            (true, true) => {}
        }
    }

    fn int_to_real(&mut self, from: Scalar, to: Scalar) {
        let s = is_signed_int(from);
        let i = match (from.is_wide(), to, s) {
            (true, Scalar::Real { bits: 32 }, true) => I::F32ConvertI64S,
            (true, Scalar::Real { bits: 32 }, false) => I::F32ConvertI64U,
            (true, _, true) => I::F64ConvertI64S,
            (true, _, false) => I::F64ConvertI64U,
            (false, Scalar::Real { bits: 32 }, true) => I::F32ConvertI32S,
            (false, Scalar::Real { bits: 32 }, false) => I::F32ConvertI32U,
            (false, _, true) => I::F64ConvertI32S,
            (false, _, false) => I::F64ConvertI32U,
        };
        self.emit(i);
    }

    /// Real to integer: round (ties to even) or truncate, saturate, NaN
    /// gives 0.
    fn real_to_int(&mut self, from: Scalar, to: Scalar, round: bool) {
        let f32 = from == Scalar::Real { bits: 32 };
        if round {
            self.emit(if f32 { I::F32Nearest } else { I::F64Nearest });
        }
        if to
            == (Scalar::Int {
                bits: 64,
                signed: false,
            })
            || to == (Scalar::Bits { bits: 64 })
        {
            self.emit(if f32 {
                I::I64TruncSatF32U
            } else {
                I::I64TruncSatF64U
            });
            return;
        }
        self.emit(if f32 {
            I::I64TruncSatF32S
        } else {
            I::I64TruncSatF64S
        });
        if to.is_wide() {
            return;
        }
        let (lo, hi): (i64, i64) = match to {
            Scalar::Bool => (0, 1),
            Scalar::Int { bits, signed: true } => (-(1i64 << (bits - 1)), (1i64 << (bits - 1)) - 1),
            Scalar::Int { bits, .. } | Scalar::Bits { bits } => (0, (1i64 << bits) - 1),
            _ => (i64::MIN, i64::MAX),
        };
        let t = self.scratch(ValType::I64);
        self.emit(I::LocalSet(t));
        // max(t, lo)
        self.emit(I::I64Const(lo));
        self.emit(I::LocalGet(t));
        self.emit(I::LocalGet(t));
        self.emit(I::I64Const(lo));
        self.emit(I::I64LtS);
        self.choose(ValType::I64);
        self.emit(I::LocalSet(t));
        // min(t, hi)
        self.emit(I::I64Const(hi));
        self.emit(I::LocalGet(t));
        self.emit(I::LocalGet(t));
        self.emit(I::I64Const(hi));
        self.emit(I::I64GtS);
        self.choose(ValType::I64);
        self.emit(I::I32WrapI64);
        self.release(ValType::I64, t);
    }

    fn intrinsic(&mut self, i: Intrinsic, t: Scalar, args: &[Expr]) -> Result<(), String> {
        let vt = valtype(t);
        match i {
            Intrinsic::Abs => {
                self.expr(&args[0])?;
                match t {
                    Scalar::Real { bits: 32 } => self.emit(I::F32Abs),
                    Scalar::Real { .. } => self.emit(I::F64Abs),
                    t if is_signed_int(t) => {
                        let x = self.scratch(vt);
                        self.emit(I::LocalSet(x));
                        self.emit(I::LocalGet(x));
                        self.unary(UnOp::Neg, t);
                        self.emit(I::LocalGet(x));
                        self.emit(I::LocalGet(x));
                        self.int_const(t, 0);
                        self.emit(if t.is_wide() { I::I64LtS } else { I::I32LtS });
                        self.choose(vt);
                        self.release(vt, x);
                    }
                    _ => {}
                }
            }
            Intrinsic::Max | Intrinsic::Min => {
                let op = if i == Intrinsic::Max {
                    BinaryOp::Gt
                } else {
                    BinaryOp::Lt
                };
                self.expr(&args[0])?;
                let m = self.scratch(vt);
                self.emit(I::LocalSet(m));
                for a in &args[1..] {
                    self.expr(a)?;
                    let x = self.scratch(vt);
                    self.emit(I::LocalSet(x));
                    self.emit(I::LocalGet(x));
                    self.emit(I::LocalGet(m));
                    self.emit(I::LocalGet(x));
                    self.emit(I::LocalGet(m));
                    self.compare(op, t);
                    self.choose(vt);
                    self.emit(I::LocalSet(m));
                    self.release(vt, x);
                }
                self.emit(I::LocalGet(m));
                self.release(vt, m);
            }
            Intrinsic::Limit => {
                let mut ls = vec![];
                for a in args {
                    self.expr(a)?;
                    let l = self.scratch(vt);
                    self.emit(I::LocalSet(l));
                    ls.push(l);
                }
                let (mn, x, mx) = (ls[0], ls[1], ls[2]);
                // r := MAX(IN, MN)
                self.emit(I::LocalGet(x));
                self.emit(I::LocalGet(mn));
                self.emit(I::LocalGet(x));
                self.emit(I::LocalGet(mn));
                self.compare(BinaryOp::Gt, t);
                self.choose(vt);
                self.emit(I::LocalSet(x));
                // MIN(r, MX)
                self.emit(I::LocalGet(x));
                self.emit(I::LocalGet(mx));
                self.emit(I::LocalGet(x));
                self.emit(I::LocalGet(mx));
                self.compare(BinaryOp::Lt, t);
                self.choose(vt);
                for l in ls {
                    self.release(vt, l);
                }
            }
            Intrinsic::Shl | Intrinsic::Shr | Intrinsic::Rol | Intrinsic::Ror => {
                self.shift(i, t, &args[0], &args[1])?
            }
            Intrinsic::Mux => {
                self.expr(&args[0])?;
                self.int_to_int(
                    args[0].ty,
                    Scalar::Int {
                        bits: 64,
                        signed: true,
                    },
                );
                let k = self.scratch(ValType::I64);
                self.emit(I::LocalSet(k));
                let mut ls = vec![];
                for a in &args[1..] {
                    self.expr(a)?;
                    let l = self.scratch(vt);
                    self.emit(I::LocalSet(l));
                    ls.push(l);
                }
                // r := IN(n-1); for i from n-2 down: r := K <= i ? IN(i) : r
                let r = *ls.last().expect("MUX inputs");
                for (i, l) in ls.iter().enumerate().rev().skip(1) {
                    self.emit(I::LocalGet(*l));
                    self.emit(I::LocalGet(r));
                    self.emit(I::LocalGet(k));
                    self.emit(I::I64Const(i as i64));
                    self.emit(I::I64LeS);
                    self.choose(vt);
                    self.emit(I::LocalSet(r));
                }
                self.emit(I::LocalGet(r));
                for l in ls {
                    self.release(vt, l);
                }
                self.release(ValType::I64, k);
            }
            Intrinsic::Compare(op) => {
                let at = args[0].ty;
                let avt = valtype(at);
                let mut ls = vec![];
                for a in args {
                    self.expr(a)?;
                    let l = self.scratch(avt);
                    self.emit(I::LocalSet(l));
                    ls.push(l);
                }
                for (j, pair) in ls.windows(2).enumerate() {
                    self.emit(I::LocalGet(pair[0]));
                    self.emit(I::LocalGet(pair[1]));
                    self.compare(op, at);
                    if j > 0 {
                        self.emit(I::I32And);
                    }
                }
                for l in ls {
                    self.release(avt, l);
                }
            }
            Intrinsic::Now => {
                let f = self.ix.now.ok_or("time source not imported")?;
                self.emit(I::Call(f));
            }
            Intrinsic::Sqrt => {
                self.expr(&args[0])?;
                self.emit(if t == (Scalar::Real { bits: 32 }) {
                    I::F32Sqrt
                } else {
                    I::F64Sqrt
                });
            }
            Intrinsic::Ln | Intrinsic::Log | Intrinsic::Exp => {
                let f32 = t == Scalar::Real { bits: 32 };
                let h = &self.ix.helpers;
                let f = if i == Intrinsic::Exp { h.exp } else { h.ln }
                    .ok_or("ln/exp helper missing")?;
                self.expr(&args[0])?;
                if f32 {
                    self.emit(I::F64PromoteF32);
                }
                self.emit(I::Call(f));
                if i == Intrinsic::Log {
                    self.emit(I::F64Const(std::f64::consts::LN_10.into()));
                    self.emit(I::F64Div);
                }
                if f32 {
                    self.emit(I::F32DemoteF64);
                }
            }
            Intrinsic::Pow => {
                let (base, exp) = (&args[0], &args[1]);
                let f32 = t == Scalar::Real { bits: 32 };
                let h = &self.ix.helpers;
                if exp.ty.is_real() {
                    let f = h.pow_real.ok_or("pow helper missing")?;
                    self.expr(base)?;
                    if f32 {
                        self.emit(I::F64PromoteF32);
                    }
                    self.expr(exp)?;
                    if exp.ty == (Scalar::Real { bits: 32 }) {
                        self.emit(I::F64PromoteF32);
                    }
                    self.emit(I::Call(f));
                    if f32 {
                        self.emit(I::F32DemoteF64);
                    }
                } else {
                    let f =
                        if f32 { h.powi_f32 } else { h.powi_f64 }.ok_or("powi helper missing")?;
                    self.expr(base)?;
                    self.expr(exp)?;
                    if !exp.ty.is_wide() {
                        self.int_to_int(
                            exp.ty,
                            Scalar::Int {
                                bits: 64,
                                signed: true,
                            },
                        );
                    }
                    self.emit(I::Call(f));
                }
            }
        }
        Ok(())
    }

    /// `SHL`, `SHR`, `ROL`, `ROR` on a bit string of width `w`:
    /// shifts by `w` or more give 0, a negative count shifts by 0, rotations
    /// take the count modulo `w`.
    fn shift(&mut self, i: Intrinsic, t: Scalar, x: &Expr, n: &Expr) -> Result<(), String> {
        let w = t.size() as i64 * 8;
        let wide = t.is_wide();
        let vt = valtype(t);
        self.expr(x)?;
        let lx = self.scratch(vt);
        self.emit(I::LocalSet(lx));
        self.expr(n)?;
        self.int_to_int(
            n.ty,
            Scalar::Int {
                bits: 64,
                signed: true,
            },
        );
        let ln = self.scratch(ValType::I64);
        self.emit(I::LocalSet(ln));
        let lr = self.scratch(vt);
        match i {
            Intrinsic::Shl | Intrinsic::Shr => {
                // Shifted by the count in the machine type; wasm takes the
                // count modulo 32 or 64, the cases outside [1, w - 1] are
                // replaced below.
                self.emit(I::LocalGet(lx));
                self.emit(I::LocalGet(ln));
                if !wide {
                    self.emit(I::I32WrapI64);
                }
                self.emit(match (i, wide) {
                    (Intrinsic::Shl, true) => I::I64Shl,
                    (Intrinsic::Shl, false) => I::I32Shl,
                    (_, true) => I::I64ShrU,
                    (_, false) => I::I32ShrU,
                });
                self.normalize(t);
                self.emit(I::LocalSet(lr));
                // N >= w: 0
                self.int_const(t, 0);
                self.emit(I::LocalGet(lr));
                self.emit(I::LocalGet(ln));
                self.emit(I::I64Const(w));
                self.emit(I::I64GeS);
                self.choose(vt);
                self.emit(I::LocalSet(lr));
                // N <= 0: IN
                self.emit(I::LocalGet(lx));
                self.emit(I::LocalGet(lr));
                self.emit(I::LocalGet(ln));
                self.emit(I::I64Const(0));
                self.emit(I::I64LeS);
                self.choose(vt);
            }
            _ => {
                let rotl = i == Intrinsic::Rol;
                // k := ((N % w) + w) % w
                self.emit(I::LocalGet(ln));
                self.emit(I::I64Const(w));
                self.emit(I::I64RemS);
                self.emit(I::I64Const(w));
                self.emit(I::I64Add);
                self.emit(I::I64Const(w));
                self.emit(I::I64RemS);
                if w >= 32 {
                    if !wide {
                        self.emit(I::I32WrapI64);
                    }
                    let lk = self.scratch(vt);
                    self.emit(I::LocalSet(lk));
                    self.emit(I::LocalGet(lx));
                    self.emit(I::LocalGet(lk));
                    self.emit(match (rotl, wide) {
                        (true, true) => I::I64Rotl,
                        (true, false) => I::I32Rotl,
                        (false, true) => I::I64Rotr,
                        (false, false) => I::I32Rotr,
                    });
                    self.release(vt, lk);
                } else {
                    // Narrow widths: (x << k) | (x >> (w - k)), masked.
                    self.emit(I::I32WrapI64);
                    let lk = self.scratch(ValType::I32);
                    self.emit(I::LocalSet(lk));
                    let (first, second) = if rotl {
                        (I::I32Shl, I::I32ShrU)
                    } else {
                        (I::I32ShrU, I::I32Shl)
                    };
                    self.emit(I::LocalGet(lx));
                    self.emit(I::LocalGet(lk));
                    self.emit(first);
                    self.emit(I::LocalGet(lx));
                    self.emit(I::I32Const(w as i32));
                    self.emit(I::LocalGet(lk));
                    self.emit(I::I32Sub);
                    self.emit(second);
                    self.emit(I::I32Or);
                    self.normalize(t);
                    self.release(ValType::I32, lk);
                }
            }
        }
        self.release(vt, lr);
        self.release(vt, lx);
        self.release(ValType::I64, ln);
        Ok(())
    }

    /// A call (section 3.3): arguments evaluated left to right into locals,
    /// then the frame reset, the arguments written, the call, the outputs
    /// copied.
    fn call(&mut self, c: &Call) -> Result<(), String> {
        let mut vals = vec![];
        for (p, e) in &c.inputs {
            self.expr(e)?;
            let vt = valtype(p.ty);
            let l = self.scratch(vt);
            self.emit(I::LocalSet(l));
            vals.push((vt, l));
        }
        let mut ptrs = vec![];
        for (_, a) in &c.in_outs {
            self.addr_value(a)?;
            let l = self.scratch(ValType::I32);
            self.emit(I::LocalSet(l));
            ptrs.push(l);
        }
        if let Some(frame) = c.frame {
            self.reset(frame);
        }
        for ((p, _), (_, l)) in c.inputs.iter().zip(&vals) {
            let off = self.addr(&p.addr)?;
            self.emit(I::LocalGet(*l));
            self.store(p.ty, off);
        }
        for ((slot, _), l) in c.in_outs.iter().zip(&ptrs) {
            let off = self.addr(slot)?;
            self.emit(I::LocalGet(*l));
            self.emit(I::I32Store(mem(off, 4)));
        }
        for copy in &c.copies_in {
            self.copy(copy)?;
        }
        for (dst, cap, src, w) in &c.strings_in {
            self.str_assign(dst, *cap, src, *w)?;
        }
        if let Some(parts) = &c.instance {
            for p in parts {
                self.addr_value(p)?;
            }
        }
        self.emit(I::Call(self.ix.first_ir_func + c.func));
        for (dst, value) in &c.outputs {
            let off = self.addr(&dst.addr)?;
            self.expr(value)?;
            self.store(dst.ty, off);
        }
        for copy in &c.copies_out {
            self.copy(copy)?;
        }
        for (dst, cap, src, w) in &c.strings_out {
            self.str_assign(dst, *cap, src, *w)?;
        }
        for (vt, l) in vals {
            self.release(vt, l);
        }
        for l in ptrs {
            self.release(ValType::I32, l);
        }
        Ok(())
    }

    // ----- statements

    fn depth(&self, label: Label, loop_start: bool) -> Result<u32, String> {
        let pos = self
            .ctrl
            .iter()
            .rposition(|c| *c == Some((label, loop_start)))
            .ok_or_else(|| format!("label L{label} not found"))?;
        Ok((self.ctrl.len() - 1 - pos) as u32)
    }

    fn stmts(&mut self, list: &[Stmt]) -> Result<(), String> {
        for s in list {
            self.stmt(s)?;
        }
        Ok(())
    }

    fn assign(&mut self, p: &Place, e: &Expr) -> Result<(), String> {
        let off = self.addr(&p.addr)?;
        self.expr(e)?;
        self.store(p.ty, off);
        Ok(())
    }

    fn stmt(&mut self, s: &Stmt) -> Result<(), String> {
        match &s.kind {
            StmtKind::Assign(p, e) => self.assign(p, e)?,
            StmtKind::Copy(c) => self.copy(c)?,
            StmtKind::Str(op, w) => self.str_op(op, *w)?,
            StmtKind::SetTemp(t, e) => {
                self.expr(e)?;
                self.emit(I::LocalSet(self.temps[*t as usize]));
            }
            StmtKind::Call(c) => self.call(c)?,
            StmtKind::Eval(e) => {
                self.expr(e)?;
                self.emit(I::Drop);
            }
            StmtKind::If(c, a, b) => {
                self.expr(c)?;
                self.emit(I::If(BlockType::Empty));
                self.ctrl.push(None);
                self.stmts(a)?;
                if !b.is_empty() {
                    self.emit(I::Else);
                    self.stmts(b)?;
                }
                self.emit(I::End);
                self.ctrl.pop();
            }
            StmtKind::Switch(sel, arms, default) => self.switch(sel, arms, default)?,
            StmtKind::Block(l, body) => {
                self.emit(I::Block(BlockType::Empty));
                self.ctrl.push(Some((*l, false)));
                self.stmts(body)?;
                self.emit(I::End);
                self.ctrl.pop();
            }
            StmtKind::Loop(l, body) => {
                self.emit(I::Block(BlockType::Empty));
                self.ctrl.push(Some((*l, false)));
                self.emit(I::Loop(BlockType::Empty));
                self.ctrl.push(Some((*l, true)));
                let region = self.counts.len();
                self.counts.push(0);
                self.regions.push(region);
                let site = if self.ix.fuel.is_some() {
                    self.site(s.span)
                } else {
                    0
                };
                self.loop_regions.push((*l, region, site));
                self.stmts(body)?;
                self.regions.pop();
                self.emit(I::End);
                self.ctrl.pop();
                self.emit(I::End);
                self.ctrl.pop();
            }
            StmtKind::Break(l) => {
                let d = self.depth(*l, false)?;
                self.emit(I::Br(d));
            }
            StmtKind::Continue(l) => {
                if self.ix.fuel.is_some() {
                    if let Some(&(_, region, site)) =
                        self.loop_regions.iter().rev().find(|(x, _, _)| x == l)
                    {
                        self.code.push(Ins::Charge { region, site });
                    }
                }
                let d = self.depth(*l, true)?;
                self.emit(I::Br(d));
            }
            StmtKind::Return => self.emit(I::Return),
            StmtKind::Check(c, code, site) => {
                self.expr(c)?;
                self.emit(I::I32Eqz);
                self.emit(I::If(BlockType::Empty));
                self.trap(*code as i32, *site);
                self.emit(I::End);
            }
            StmtKind::DebugSite(site) => {
                if let Some(h) = self.ix.debug_hook {
                    self.emit(I::I32Const(*site as i32));
                    self.emit(I::Call(h));
                }
            }
            StmtKind::Reset(o) => self.reset(*o),
        }
        Ok(())
    }

    /// CASE: a `br_table` when the labels are dense (their range
    /// is at most four times their number), comparisons otherwise.
    fn switch(&mut self, sel: &Expr, arms: &[SwitchArm], default: &[Stmt]) -> Result<(), String> {
        let t = sel.ty;
        let vt = valtype(t);
        self.expr(sel)?;
        let s = self.scratch(vt);
        self.emit(I::LocalSet(s));
        let values: i128 = arms
            .iter()
            .flat_map(|(ls, _)| ls.iter())
            .map(|(lo, hi)| (hi - lo + 1).max(0))
            .sum();
        let min = arms.iter().flat_map(|(ls, _)| ls.iter()).map(|l| l.0).min();
        let max = arms.iter().flat_map(|(ls, _)| ls.iter()).map(|l| l.1).max();
        let dense = match (min, max) {
            (Some(lo), Some(hi)) => {
                let span = hi - lo + 1;
                span > 0 && span <= 4 * values && span <= 4096
            }
            _ => false,
        };
        if dense {
            let (lo, hi) = (min.unwrap_or(0), max.unwrap_or(0));
            self.dense_switch(s, t, lo, hi, arms, default)?;
        } else {
            self.chain(s, t, arms, default)?;
        }
        self.release(vt, s);
        Ok(())
    }

    fn dense_switch(
        &mut self,
        s: u32,
        t: Scalar,
        lo: i128,
        hi: i128,
        arms: &[SwitchArm],
        default: &[Stmt],
    ) -> Result<(), String> {
        let n = arms.len() as u32;
        let span = (hi - lo + 1) as usize;
        let mut targets = vec![n; span];
        for (i, (labels, _)) in arms.iter().enumerate().rev() {
            for (a, b) in labels {
                for v in *a..=*b {
                    targets[(v - lo) as usize] = i as u32;
                }
            }
        }
        // block $end, block $default, blocks of the arms (innermost first).
        let end_pos = self.ctrl.len();
        for _ in 0..n + 2 {
            self.emit(I::Block(BlockType::Empty));
            self.ctrl.push(None);
        }
        self.emit(I::LocalGet(s));
        if t.is_wide() {
            self.emit(I::I64Const(lo as i64));
            self.emit(I::I64Sub);
            let d = self.scratch(ValType::I64);
            self.emit(I::LocalTee(d));
            self.emit(I::I32WrapI64);
            self.emit(I::I32Const(-1));
            self.emit(I::LocalGet(d));
            self.emit(I::I64Const(span as i64));
            self.emit(I::I64LtU);
            self.choose(ValType::I32);
            self.release(ValType::I64, d);
        } else {
            self.emit(I::I32Const(lo as i32));
            self.emit(I::I32Sub);
        }
        self.emit(I::BrTable(Cow::Owned(targets), n));
        for (_, body) in arms {
            self.emit(I::End);
            self.ctrl.pop();
            self.stmts(body)?;
            let d = (self.ctrl.len() - 1 - end_pos) as u32;
            self.emit(I::Br(d));
        }
        self.emit(I::End);
        self.ctrl.pop();
        self.stmts(default)?;
        self.emit(I::End);
        self.ctrl.pop();
        Ok(())
    }

    fn chain(
        &mut self,
        s: u32,
        t: Scalar,
        arms: &[SwitchArm],
        default: &[Stmt],
    ) -> Result<(), String> {
        let Some(((labels, body), rest)) = arms.split_first() else {
            return self.stmts(default);
        };
        let mut first = true;
        for (a, b) in labels {
            if a == b {
                self.emit(I::LocalGet(s));
                self.int_const(t, *a as i64);
                self.compare(BinaryOp::Eq, t);
            } else {
                self.emit(I::LocalGet(s));
                self.int_const(t, *a as i64);
                self.compare(BinaryOp::Ge, t);
                self.emit(I::LocalGet(s));
                self.int_const(t, *b as i64);
                self.compare(BinaryOp::Le, t);
                self.emit(I::I32And);
            }
            if !first {
                self.emit(I::I32Or);
            }
            first = false;
        }
        if first {
            self.emit(I::I32Const(0));
        }
        self.emit(I::If(BlockType::Empty));
        self.ctrl.push(None);
        self.stmts(body)?;
        self.emit(I::Else);
        self.chain(s, t, rest, default)?;
        self.emit(I::End);
        self.ctrl.pop();
        Ok(())
    }
}
