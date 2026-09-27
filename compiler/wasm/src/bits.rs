//! Bits and partial accesses of integers: `x.3`, `x.%X3`, `x.%B1`,
//! `x.%W0`, `x.%D1`, `x.%L0` (IEC 61131-3 section 6.5.5.3).

use ironplc_dsl::core::{Located, SourceSpan};
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_dsl::textual::{SymbolicVariableKind, Variable};
use ironplc_wasm_ir::{Addr, BinaryOp, Expr, ExprKind, Intrinsic, Place, Scalar, Stmt, UnOp};

use crate::body::{ex, nyi, Body};
use crate::expr::{bin, convert, int, widen};
use crate::types::{is_int, op_scalar, BOOL, U32, U64};

/// A slice of the bits of an integer variable.
pub(crate) struct Slice<'v> {
    variable: &'v SymbolicVariableKind,
    /// Position of the lowest bit.
    shift: u32,
    /// Number of bits: 1 for a bit.
    width: u32,
    span: SourceSpan,
}

/// The slice an access names, if it is a bit or a partial access.
pub(crate) fn slice(v: &Variable) -> Option<Slice<'_>> {
    match v {
        Variable::Symbolic(SymbolicVariableKind::BitAccess(b)) => Some(Slice {
            variable: &b.variable,
            shift: b.index.value as u32,
            width: 1,
            span: b.span(),
        }),
        Variable::Symbolic(SymbolicVariableKind::PartialAccess(p)) => {
            let width = p.size.bit_width();
            Some(Slice {
                variable: &p.variable,
                shift: p.index.value as u32 * width,
                width,
                span: p.span(),
            })
        }
        _ => None,
    }
}

impl Slice<'_> {
    /// The operation scalar of the value of the slice: `BOOL` for a bit, an
    /// unsigned integer otherwise.
    pub fn value_scalar(&self) -> Scalar {
        match self.width {
            1 => BOOL,
            64 => U64,
            _ => U32,
        }
    }

    fn mask(&self) -> i128 {
        (1i128 << self.width) - 1
    }
}

impl Body<'_, '_> {
    fn slice_base(&mut self, s: &Slice) -> Result<(Addr, Scalar), Diagnostic> {
        let (addr, ty) = self.place(&Variable::Symbolic(s.variable.clone()))?;
        let sc = ty
            .sc()
            .filter(|sc| is_int(*sc) && s.shift + s.width <= sc.size() * 8)
            .ok_or_else(|| nyi(&s.span, "This bit or partial access"))?;
        Ok((addr, sc))
    }

    /// The value of a slice in its operation scalar.
    pub fn read_slice(&mut self, s: &Slice) -> Result<Expr, Diagnostic> {
        let (addr, sc) = self.slice_base(s)?;
        let op = op_scalar(sc);
        let v = widen(ex(ExprKind::Load(Place { addr, ty: sc }), sc), op);
        let shifted = ex(
            ExprKind::Intrinsic(Intrinsic::Shr, vec![v, int(s.shift as i128, op)]),
            op,
        );
        let bits = bin(BinaryOp::And, shifted, int(s.mask(), op), op);
        Ok(convert(bits, s.value_scalar()))
    }

    /// `x.slice := v`, `v` in the value scalar of the slice.
    pub fn write_slice(
        &mut self,
        s: &Slice,
        v: Expr,
        out: &mut Vec<Stmt>,
    ) -> Result<(), Diagnostic> {
        let (addr, sc) = self.slice_base(s)?;
        let op = op_scalar(sc);
        let old = widen(
            ex(
                ExprKind::Load(Place {
                    addr: addr.clone(),
                    ty: sc,
                }),
                sc,
            ),
            op,
        );
        let hole = ex(
            ExprKind::Unary(UnOp::Not, Box::new(int(s.mask() << s.shift, op))),
            op,
        );
        let cleared = bin(BinaryOp::And, old, hole, op);
        let v = bin(BinaryOp::And, convert(v, op), int(s.mask(), op), op);
        let placed = ex(
            ExprKind::Intrinsic(Intrinsic::Shl, vec![v, int(s.shift as i128, op)]),
            op,
        );
        out.push(self.store(addr, sc, bin(BinaryOp::Or, cleared, placed, op)));
        Ok(())
    }
}
