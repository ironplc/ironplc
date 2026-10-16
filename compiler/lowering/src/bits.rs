//! Bit access (`x.3`) and partial access (`x.%B1`): `bits` bits of an
//! integer or bit string, starting at bit `shift`.
//!
//! A read is the bits shifted down to bit 0 and masked, over a read of the
//! base (REQ-LOW-lowering-100). A write is one `AssignBits` that names the
//! base's place once (REQ-LOW-lowering-102).

use ironplc_dsl::core::{Located, SourceSpan};
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_dsl::textual::{Expr as AstExpr, SymbolicVariableKind};
use ironplc_ir::lowered::{
    Arg, BinaryOp, CompareOp, Expr, Intrinsic, ScalarType, Signedness, Stmt,
};

use crate::expr::Lowered;
use crate::variables::BodyFrame;
use crate::Lowering;

impl Lowering<'_> {
    /// `x.3 := v` or `x.%B1 := v`, the bits `target` selects set to
    /// `value`: one `AssignBits` that names the base place once
    /// (REQ-LOW-lowering-102). Bits of the whole of what a reference refers
    /// to, `p^.3`, are not lowered yet, nor is a dereference of the bits
    /// themselves, `x.3^ := v` (`deref`).
    pub(crate) fn bits_assignment(
        &mut self,
        frame: &BodyFrame,
        target: &SymbolicVariableKind,
        deref: bool,
        value: &AstExpr,
        span: SourceSpan,
    ) -> Result<Option<Stmt>, Diagnostic> {
        let Some((base, shift, bits)) = selected_bits(target) else {
            return Ok(None);
        };
        if deref || is_dereference(base) {
            return Ok(None);
        }
        // The base of `d.%B1.3` is itself a bit selection, not a place.
        let Some(place) = self.symbolic_place(frame, base)? else {
            return Ok(None);
        };
        let Some(value) = self.expr(frame, value)? else {
            return Ok(None);
        };
        self.program
            .assign_bits(place, shift, bits, value, span)
            .map(Some)
    }

    /// A read of `x.3` or `x.%B1`, the expression `expr`: the bits shifted
    /// down to bit 0 and masked, a `BOOL` for one bit and a bit string of
    /// their width for more (REQ-LOW-lowering-100).
    pub(crate) fn bits_read(
        &mut self,
        frame: &BodyFrame,
        expr: &AstExpr,
        symbolic: &SymbolicVariableKind,
    ) -> Lowered {
        let Some((_, recorded)) = self.operated_type(expr)? else {
            return Ok(None);
        };
        let Some(value) = self.selected_value(frame, symbolic)? else {
            return Ok(None);
        };
        // The analyzer gave the access the type its selection has.
        if value.ty() != recorded {
            return Ok(None);
        }
        Ok(Some(value))
    }

    /// The bits `symbolic`, a bit or partial access, selects, read.
    fn selected_value(&mut self, frame: &BodyFrame, symbolic: &SymbolicVariableKind) -> Lowered {
        let Some((base, shift, bits)) = selected_bits(symbolic) else {
            return Ok(None);
        };
        if is_dereference(base) {
            return Ok(None);
        }
        let span = symbolic.span();
        // The base of `d.%B1.3` is itself a bit selection, read as a value.
        let value = match selected_bits(base) {
            Some(_) => self.selected_value(frame, base)?,
            None => match self.symbolic_place(frame, base)? {
                Some(place) => self.read(place, span.clone())?,
                None => None,
            },
        };
        let Some(value) = value else {
            return Ok(None);
        };
        select(value, shift, bits, span)
    }
}

const U32: ScalarType = ScalarType::I32(Signedness::Unsigned);
const U64: ScalarType = ScalarType::I64(Signedness::Unsigned);
const I32: ScalarType = ScalarType::I32(Signedness::Signed);
const I64: ScalarType = ScalarType::I64(Signedness::Signed);

/// The `bits` bits of the integer `value` that start at bit `shift`, shifted
/// down to bit 0: a `BOOL` for one bit, and otherwise an unsigned integer of
/// 32 bits for up to 32 bits and of 64 for more.
fn select(value: Expr, shift: u8, bits: u8, span: SourceSpan) -> Lowered {
    let (unsigned, count, shr, width) = match value.ty() {
        ScalarType::I32(_) => (U32, I32, Intrinsic::ShrU32, 32),
        ScalarType::I64(_) => (U64, I64, Intrinsic::ShrU64, 64),
        ScalarType::Bool
        | ScalarType::F32
        | ScalarType::F64
        | ScalarType::Ref
        | ScalarType::Interface => return Ok(None),
    };
    // A signed base is read as its bit pattern.
    let mut selected = if value.ty() == unsigned {
        value
    } else {
        Expr::convert(value, unsigned, span.clone())?
    };
    if shift > 0 {
        let count = Expr::integer(shift.into(), count, span.clone())?;
        selected = Expr::call(
            shr,
            vec![Arg::Value(selected), Arg::Value(count)],
            span.clone(),
        )?;
    }
    if bits < width {
        let mask = Expr::integer((1i128 << bits) - 1, unsigned, span.clone())?;
        selected = Expr::binary(BinaryOp::BitAnd, selected, mask, span.clone())?;
    }
    if bits == 1 {
        let zero = Expr::integer(0, unsigned, span.clone())?;
        return Expr::compare(CompareOp::Ne, selected, zero, span).map(Some);
    }
    // A selection of up to 32 bits is a bit string of 32 bits.
    if bits <= 32 && selected.ty() != U32 {
        return Expr::convert(selected, U32, span).map(Some);
    }
    Ok(Some(selected))
}

/// The base, first bit and number of bits a bit access (`x.3`) or a partial
/// access (`x.%B1`) selects; `None` for any other variable. The analyzer
/// reports a selection that does not lie within its base (P4025), so the
/// first bit fits a `u8`; one that did not would be refused when the
/// `AssignBits` is built.
pub(crate) fn selected_bits(
    symbolic: &SymbolicVariableKind,
) -> Option<(&SymbolicVariableKind, u8, u8)> {
    let (base, index, bits) = match symbolic {
        SymbolicVariableKind::BitAccess(access) => (&access.variable, access.index.value, 1),
        SymbolicVariableKind::PartialAccess(access) => (
            &access.variable,
            access.index.value,
            access.size.bit_width(),
        ),
        SymbolicVariableKind::Named(_)
        | SymbolicVariableKind::Array(_)
        | SymbolicVariableKind::Structured(_)
        | SymbolicVariableKind::Deref(_)
        | SymbolicVariableKind::SelfRef(_) => return None,
    };
    let bits = u8::try_from(bits).unwrap_or(u8::MAX);
    let shift = index
        .checked_mul(u128::from(bits))
        .and_then(|shift| u8::try_from(shift).ok())
        .unwrap_or(u8::MAX);
    Some((base.as_ref(), shift, bits))
}

/// Whether `base`, the base of a bit or partial access, is the whole of what
/// a reference refers to, as in `p^.3`.
///
/// Codegen's AST route addresses bits of an element reached through a
/// reference (`p^[1].3`), but reports bits of the whole of what a reference
/// refers to as not implemented. So while both routes exist, such bits are
/// not lowered either.
fn is_dereference(base: &SymbolicVariableKind) -> bool {
    match base {
        SymbolicVariableKind::Deref(_) => true,
        SymbolicVariableKind::Named(_)
        | SymbolicVariableKind::SelfRef(_)
        | SymbolicVariableKind::Array(_)
        | SymbolicVariableKind::Structured(_)
        | SymbolicVariableKind::BitAccess(_)
        | SymbolicVariableKind::PartialAccess(_) => false,
    }
}
