//! String helpers, written in WebAssembly: length,
//! assignment with truncation, append of a clamped slice, splice (for
//! `INSERT`, `DELETE`, `REPLACE`), comparison by code and search. One set
//! for `STRING` (bytes) and one for `WSTRING` (16-bit code units), each
//! only when the module uses it. Strings end with a zero character.

use ironplc_wasm_ir::{walk, Call, Expr, ExprKind, Module, Stmt, StmtKind, Visitor};
use wasm_encoder::{BlockType, Function, Instruction as I, MemArg, TypeSection, ValType};

/// Which kinds of strings a module uses.
#[derive(Debug, Clone, Copy, Default)]
pub struct Used {
    narrow: bool,
    wide: bool,
}

impl Used {
    fn mark(&mut self, wide: bool) {
        if wide {
            self.wide = true;
        } else {
            self.narrow = true;
        }
    }
}

impl Visitor for Used {
    fn expr(&mut self, e: &Expr) {
        if let ExprKind::StrLen(_, w)
        | ExprKind::StrFind(_, _, w)
        | ExprKind::StrCmp(_, _, w)
        | ExprKind::StrChar(_, w) = &e.kind
        {
            self.mark(*w);
        }
    }

    fn stmt(&mut self, s: &Stmt) {
        if let StmtKind::Str(_, w) = &s.kind {
            self.mark(*w);
        }
    }

    fn call(&mut self, c: &Call) {
        for (_, _, _, w) in c.strings_in.iter().chain(&c.strings_out) {
            self.mark(*w);
        }
    }
}

/// Kinds of strings used by a module.
pub fn used(m: &Module) -> Used {
    let mut u = Used::default();
    for f in &m.functions {
        walk(&f.body, &mut u);
    }
    u
}

/// Function indexes of the helpers of one kind of string.
#[derive(Debug, Clone, Copy)]
pub struct Kind {
    /// `len(s) -> n`
    pub len: u32,
    /// `assign(dst, cap, src)`
    pub assign: u32,
    /// `append(dst, cap, src, start, count)`
    pub append: u32,
    /// `splice(dst, cap, src, pos, len, ins)`, `ins` 0 for none
    pub splice: u32,
    /// `cmp(a, b) -> -1 | 0 | 1`
    pub cmp: u32,
    /// `find(a, b) -> position | 0`
    pub find: u32,
    /// `convert(dst, cap, src)` from a string of the other kind
    pub convert: u32,
}

/// Function indexes of the string helpers.
#[derive(Debug, Clone, Copy, Default)]
pub struct StrIndexes {
    narrow: Option<Kind>,
    wide: Option<Kind>,
    /// Number of helper functions.
    pub count: u32,
}

impl StrIndexes {
    /// Indexes from `first`, `STRING` helpers first.
    pub fn assign(first: u32, u: Used) -> StrIndexes {
        let mut next = first;
        let mut take = |flag: bool| {
            flag.then(|| {
                let k = Kind {
                    len: next,
                    assign: next + 1,
                    append: next + 2,
                    splice: next + 3,
                    cmp: next + 4,
                    find: next + 5,
                    convert: next + 6,
                };
                next += 7;
                k
            })
        };
        let narrow = take(u.narrow);
        let wide = take(u.wide);
        StrIndexes {
            narrow,
            wide,
            count: next - first,
        }
    }

    /// Helpers of a kind of string.
    pub fn get(&self, wide: bool) -> Result<Kind, String> {
        if wide { self.wide } else { self.narrow }.ok_or_else(|| "string helpers missing".into())
    }
}

/// Type indexes of the helpers.
pub struct StrTypes {
    i_i: u32,
    iii: u32,
    i5: u32,
    i6: u32,
    ii_i: u32,
}

/// Adds the types of the helpers.
pub fn add_types(types: &mut TypeSection) -> StrTypes {
    let base = types.len();
    let i = ValType::I32;
    types.ty().function([i], [i]);
    types.ty().function([i, i, i], []);
    types.ty().function([i, i, i, i, i], []);
    types.ty().function([i, i, i, i, i, i], []);
    types.ty().function([i, i], [i]);
    StrTypes {
        i_i: base,
        iii: base + 1,
        i5: base + 2,
        i6: base + 3,
        ii_i: base + 4,
    }
}

/// The helper functions, in the order of their indexes.
pub fn functions(h: &StrIndexes, t: &StrTypes) -> Vec<(u32, Function)> {
    let mut out = vec![];
    for (wide, k) in [(false, h.narrow), (true, h.wide)] {
        let Some(k) = k else { continue };
        let u = Unit { wide };
        out.push((t.i_i, u.len()));
        out.push((t.iii, u.assign(k)));
        out.push((t.i5, u.append(k)));
        out.push((t.i6, u.splice(k)));
        out.push((t.ii_i, u.cmp()));
        out.push((t.ii_i, u.find(k)));
        out.push((t.iii, u.convert()));
    }
    out
}

/// Code of the helpers for one size of character.
struct Unit {
    wide: bool,
}

fn body(locals: Vec<(u32, ValType)>, ins: Vec<I<'static>>) -> Function {
    let mut f = Function::new(locals);
    for i in &ins {
        f.instruction(i);
    }
    f.instruction(&I::End);
    f
}

impl Unit {
    fn mem(&self) -> MemArg {
        MemArg {
            offset: 0,
            align: self.wide as u32,
            memory_index: 0,
        }
    }

    fn load(&self) -> I<'static> {
        if self.wide {
            I::I32Load16U(self.mem())
        } else {
            I::I32Load8U(self.mem())
        }
    }

    fn store(&self) -> I<'static> {
        if self.wide {
            I::I32Store16(self.mem())
        } else {
            I::I32Store8(self.mem())
        }
    }

    /// Scales the index on the stack to bytes.
    fn scale(&self, out: &mut Vec<I<'static>>) {
        if self.wide {
            out.push(I::I32Const(1));
            out.push(I::I32Shl);
        }
    }

    /// Pushes `base + index * unit` from two locals.
    fn at(&self, out: &mut Vec<I<'static>>, base: u32, index: u32) {
        out.push(I::LocalGet(base));
        out.push(I::LocalGet(index));
        self.scale(out);
        out.push(I::I32Add);
    }

    /// `local := min(local, bound)` for signed values.
    fn min(out: &mut Vec<I<'static>>, local: u32, bound: Vec<I<'static>>) {
        out.push(I::LocalGet(local));
        out.extend(bound.iter().cloned());
        out.push(I::I32GtS);
        out.push(I::If(BlockType::Empty));
        out.extend(bound);
        out.push(I::LocalSet(local));
        out.push(I::End);
    }

    /// `local := max(local, bound)` for signed values.
    fn max(out: &mut Vec<I<'static>>, local: u32, bound: Vec<I<'static>>) {
        out.push(I::LocalGet(local));
        out.extend(bound.iter().cloned());
        out.push(I::I32LtS);
        out.push(I::If(BlockType::Empty));
        out.extend(bound);
        out.push(I::LocalSet(local));
        out.push(I::End);
    }

    /// `len(p)`: params p (0); local n (1).
    fn len(&self) -> Function {
        let mut c = vec![I::I32Const(0), I::LocalSet(1)];
        c.push(I::Block(BlockType::Empty));
        c.push(I::Loop(BlockType::Empty));
        self.at(&mut c, 0, 1);
        c.extend([
            self.load(),
            I::I32Eqz,
            I::BrIf(1),
            I::LocalGet(1),
            I::I32Const(1),
            I::I32Add,
            I::LocalSet(1),
            I::Br(0),
            I::End,
            I::End,
            I::LocalGet(1),
        ]);
        body(vec![(1, ValType::I32)], c)
    }

    /// `assign(dst, cap, src)`: params 0 to 2; local n (3). The copy moves
    /// overlapping strings correctly.
    fn assign(&self, k: Kind) -> Function {
        let mut c = vec![I::LocalGet(2), I::Call(k.len), I::LocalSet(3)];
        Self::min(&mut c, 3, vec![I::LocalGet(1)]);
        c.extend([I::LocalGet(0), I::LocalGet(2), I::LocalGet(3)]);
        self.scale(&mut c);
        c.push(I::MemoryCopy {
            src_mem: 0,
            dst_mem: 0,
        });
        self.at(&mut c, 0, 3);
        c.extend([I::I32Const(0), self.store()]);
        body(vec![(1, ValType::I32)], c)
    }

    /// `append(dst, cap, src, start, count)`: params 0 to 4; locals dl (5),
    /// sl (6).
    fn append(&self, k: Kind) -> Function {
        let mut c = vec![
            I::LocalGet(0),
            I::Call(k.len),
            I::LocalSet(5),
            I::LocalGet(2),
            I::Call(k.len),
            I::LocalSet(6),
        ];
        // start in 0..=sl, count in 0..=sl - start, then within the room.
        Self::max(&mut c, 3, vec![I::I32Const(0)]);
        Self::min(&mut c, 3, vec![I::LocalGet(6)]);
        Self::max(&mut c, 4, vec![I::I32Const(0)]);
        Self::min(&mut c, 4, vec![I::LocalGet(6), I::LocalGet(3), I::I32Sub]);
        Self::min(&mut c, 4, vec![I::LocalGet(1), I::LocalGet(5), I::I32Sub]);
        self.at(&mut c, 0, 5);
        self.at(&mut c, 2, 3);
        c.push(I::LocalGet(4));
        self.scale(&mut c);
        c.push(I::MemoryCopy {
            src_mem: 0,
            dst_mem: 0,
        });
        c.extend([I::LocalGet(0), I::LocalGet(5), I::LocalGet(4), I::I32Add]);
        self.scale(&mut c);
        c.extend([I::I32Add, I::I32Const(0), self.store()]);
        body(vec![(2, ValType::I32)], c)
    }

    /// `splice(dst, cap, src, pos, len, ins)`: params 0 to 5; local sl (6).
    fn splice(&self, k: Kind) -> Function {
        let mut c = vec![I::LocalGet(2), I::Call(k.len), I::LocalSet(6)];
        Self::max(&mut c, 3, vec![I::I32Const(1)]);
        Self::min(&mut c, 3, vec![I::LocalGet(6), I::I32Const(1), I::I32Add]);
        Self::max(&mut c, 4, vec![I::I32Const(0)]);
        Self::min(
            &mut c,
            4,
            vec![
                I::LocalGet(6),
                I::LocalGet(3),
                I::I32Sub,
                I::I32Const(1),
                I::I32Add,
            ],
        );
        // dst := '' ; the part before pos ; ins ; the part after pos + len.
        c.extend([I::LocalGet(0), I::I32Const(0), self.store()]);
        c.extend([
            I::LocalGet(0),
            I::LocalGet(1),
            I::LocalGet(2),
            I::I32Const(0),
            I::LocalGet(3),
            I::I32Const(1),
            I::I32Sub,
            I::Call(k.append),
            I::LocalGet(5),
            I::If(BlockType::Empty),
            I::LocalGet(0),
            I::LocalGet(1),
            I::LocalGet(5),
            I::I32Const(0),
            I::I32Const(65536),
            I::Call(k.append),
            I::End,
            I::LocalGet(0),
            I::LocalGet(1),
            I::LocalGet(2),
            I::LocalGet(3),
            I::I32Const(1),
            I::I32Sub,
            I::LocalGet(4),
            I::I32Add,
            I::I32Const(65536),
            I::Call(k.append),
        ]);
        body(vec![(1, ValType::I32)], c)
    }

    /// `convert(dst, cap, src)` with `src` of the other kind: params 0 to 2;
    /// locals i (3), c (4). A character above 255 becomes `?` in a
    /// `STRING`.
    fn convert(&self) -> Function {
        let other = Unit { wide: !self.wide };
        let mut c = vec![
            I::I32Const(0),
            I::LocalSet(3),
            I::Block(BlockType::Empty),
            I::Loop(BlockType::Empty),
            I::LocalGet(3),
            I::LocalGet(1),
            I::I32GeU,
            I::BrIf(1),
        ];
        other.at(&mut c, 2, 3);
        c.extend([other.load(), I::LocalTee(4), I::I32Eqz, I::BrIf(1)]);
        if !self.wide {
            c.extend([
                I::LocalGet(4),
                I::I32Const(255),
                I::I32GtU,
                I::If(BlockType::Empty),
                I::I32Const('?' as i32),
                I::LocalSet(4),
                I::End,
            ]);
        }
        self.at(&mut c, 0, 3);
        c.extend([
            I::LocalGet(4),
            self.store(),
            I::LocalGet(3),
            I::I32Const(1),
            I::I32Add,
            I::LocalSet(3),
            I::Br(0),
            I::End,
            I::End,
        ]);
        self.at(&mut c, 0, 3);
        c.extend([I::I32Const(0), self.store()]);
        body(vec![(2, ValType::I32)], c)
    }

    /// `cmp(a, b)`: params 0, 1; locals i (2), ca (3), cb (4).
    fn cmp(&self) -> Function {
        let mut c = vec![I::I32Const(0), I::LocalSet(2), I::Loop(BlockType::Empty)];
        self.at(&mut c, 0, 2);
        c.extend([self.load(), I::LocalSet(3)]);
        self.at(&mut c, 1, 2);
        c.extend([
            self.load(),
            I::LocalSet(4),
            I::LocalGet(3),
            I::LocalGet(4),
            I::I32Ne,
            I::If(BlockType::Empty),
            // 1 when a > b, else -1, without `select`.
            I::LocalGet(3),
            I::LocalGet(4),
            I::I32GtU,
            I::If(BlockType::Result(ValType::I32)),
            I::I32Const(1),
            I::Else,
            I::I32Const(-1),
            I::End,
            I::Return,
            I::End,
            I::LocalGet(3),
            I::I32Eqz,
            I::If(BlockType::Empty),
            I::I32Const(0),
            I::Return,
            I::End,
            I::LocalGet(2),
            I::I32Const(1),
            I::I32Add,
            I::LocalSet(2),
            I::Br(0),
            I::End,
            I::Unreachable,
        ]);
        body(vec![(3, ValType::I32)], c)
    }

    /// `find(a, b)`: params 0, 1; locals la (2), lb (3), i (4), j (5).
    fn find(&self, k: Kind) -> Function {
        let mut c = vec![
            I::LocalGet(1),
            I::Call(k.len),
            I::LocalTee(3),
            I::I32Eqz,
            I::If(BlockType::Empty),
            I::I32Const(0),
            I::Return,
            I::End,
            I::LocalGet(0),
            I::Call(k.len),
            I::LocalSet(2),
            I::I32Const(0),
            I::LocalSet(4),
            I::Block(BlockType::Empty),
            I::Loop(BlockType::Empty),
            // i + lb > la: not found.
            I::LocalGet(4),
            I::LocalGet(3),
            I::I32Add,
            I::LocalGet(2),
            I::I32GtS,
            I::BrIf(1),
            I::I32Const(0),
            I::LocalSet(5),
            I::Block(BlockType::Empty),
            I::Loop(BlockType::Empty),
            I::LocalGet(5),
            I::LocalGet(3),
            I::I32GeS,
            I::If(BlockType::Empty),
            I::LocalGet(4),
            I::I32Const(1),
            I::I32Add,
            I::Return,
            I::End,
        ];
        // a[i + j] != b[j]: next position.
        c.extend([I::LocalGet(0), I::LocalGet(4), I::LocalGet(5), I::I32Add]);
        self.scale(&mut c);
        c.extend([I::I32Add, self.load()]);
        self.at(&mut c, 1, 5);
        c.extend([
            self.load(),
            I::I32Ne,
            I::BrIf(1),
            I::LocalGet(5),
            I::I32Const(1),
            I::I32Add,
            I::LocalSet(5),
            I::Br(0),
            I::End,
            I::End,
            I::LocalGet(4),
            I::I32Const(1),
            I::I32Add,
            I::LocalSet(4),
            I::Br(0),
            I::End,
            I::End,
            I::I32Const(0),
        ]);
        body(vec![(4, ValType::I32)], c)
    }
}
