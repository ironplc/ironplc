//! Calls of standard functions, each the `Intrinsic` for its function at its
//! operand types (REQ-LOW-lowering-147).

use ironplc_analyzer::{BitShift, BuiltinFunction};
use ironplc_dsl::core::Located;
use ironplc_dsl::textual::{Expr as AstExpr, Function};
use ironplc_ir::lowered::{
    Arg, Callee, Expr, Intrinsic, ScalarType, Signedness, TypeId, TypeTable,
};

use crate::expr::Lowered;
use crate::variables::BodyFrame;
use crate::Lowering;

impl Lowering<'_> {
    /// The call `func`, the expression `expr`.
    pub(crate) fn function_call(
        &mut self,
        frame: &BodyFrame,
        expr: &AstExpr,
        func: &Function,
    ) -> Lowered {
        // A call of a user-defined function is not lowered yet.
        let Some(builtin) = self
            .analysis
            .context()
            .functions()
            .get(&func.name)
            .and_then(|signature| signature.builtin_function.clone())
        else {
            return Ok(None);
        };
        // The analyzer recorded the type the call computes at: for a shift,
        // the type of the value shifted.
        let Some((ty, _)) = self.operated_type(expr)? else {
            return Ok(None);
        };
        let Some(intrinsic) = intrinsic(self.program.types(), &builtin, ty) else {
            return Ok(None);
        };
        // The analyzer wrote the call in normal form: its inputs in the order
        // of the function's parameters.
        let mut args = Vec::new();
        for input in func.inputs() {
            let Some(value) = self.expr(frame, input)? else {
                return Ok(None);
            };
            args.push(value);
        }
        // An argument the analyzer did not convert to its parameter's type is
        // not lowered until the analyzer records its conversion.
        let (params, _) = intrinsic.signature();
        if !args.iter().map(Expr::ty).eq(params.iter().copied()) {
            return Ok(None);
        }
        Expr::call(
            Callee::Intrinsic(intrinsic),
            args.into_iter().map(Arg::Value).collect(),
            expr.span(),
        )
        .map(Some)
    }
}

/// The intrinsic for the standard function `builtin` computed at the type
/// `ty`; `None` for a function, or a type, lowering does not map yet. The
/// match names every function, so one the analyzer adds is not lowered until
/// it has an arm here (REQ-LOW-lowering-147).
fn intrinsic(types: &TypeTable, builtin: &BuiltinFunction, ty: TypeId) -> Option<Intrinsic> {
    match builtin {
        BuiltinFunction::BitShift(shift) => {
            shift_intrinsic(*shift, types.operation_type(ty)?, types.bit_width(ty)?)
        }
        BuiltinFunction::Operator(_)
        | BuiltinFunction::Numeric(_)
        | BuiltinFunction::Mux
        | BuiltinFunction::Move
        | BuiltinFunction::Trunc
        | BuiltinFunction::BcdToInt
        | BuiltinFunction::IntToBcd
        | BuiltinFunction::Sizeof
        | BuiltinFunction::String(_)
        | BuiltinFunction::Conversion { .. }
        | BuiltinFunction::Time { .. }
        | BuiltinFunction::DtToDate
        | BuiltinFunction::DtToTod => None,
    }
}

/// The shift or rotate `shift` of a bit string operated at `operated` and
/// `width` bits wide.
fn shift_intrinsic(shift: BitShift, operated: ScalarType, width: u8) -> Option<Intrinsic> {
    const U32: ScalarType = ScalarType::I32(Signedness::Unsigned);
    const U64: ScalarType = ScalarType::I64(Signedness::Unsigned);
    let intrinsic = match (shift, operated, width) {
        (BitShift::Shl, U32, _) => Intrinsic::ShlU32,
        (BitShift::Shl, U64, _) => Intrinsic::ShlU64,
        (BitShift::Shr, U32, _) => Intrinsic::ShrU32,
        (BitShift::Shr, U64, _) => Intrinsic::ShrU64,
        (BitShift::Rol, U32, 8) => Intrinsic::RolU8,
        (BitShift::Rol, U32, 16) => Intrinsic::RolU16,
        (BitShift::Rol, U32, 32) => Intrinsic::RolU32,
        (BitShift::Rol, U64, 64) => Intrinsic::RolU64,
        (BitShift::Ror, U32, 8) => Intrinsic::RorU8,
        (BitShift::Ror, U32, 16) => Intrinsic::RorU16,
        (BitShift::Ror, U32, 32) => Intrinsic::RorU32,
        (BitShift::Ror, U64, 64) => Intrinsic::RorU64,
        // A shift of a `BOOL`, or of an integer the analyzer let through.
        _ => return None,
    };
    Some(intrinsic)
}
