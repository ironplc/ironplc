//! Default values of single-slot types.
//!
//! A structure field or an array element that no initial value sets takes
//! its type's default (IEC 61131-3 2.4.3.1). This module decides that
//! default for a field or element held in one slot, in one place, so that
//! the default can later come from the analyzer instead.
//!
//! The declared default of a derived type (`TYPE T : INT := 7; END_TYPE`)
//! or of an enumeration type does not reach code generation: the
//! intermediate type keeps no default. Such a type gets its base type's
//! default here, which is 0 for an enumeration too.

use ironplc_analyzer::intermediate_type::IntermediateType;

use super::compile::{CompileContext, OpType, OpWidth};
use super::compile_array::ArraySpec;
use super::compile_setup::emit_zero_const;
use crate::emit::Emitter;

/// The value a single-slot variable, field or element starts from when no
/// initial value sets it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LeafDefault {
    /// Zero, which is also FALSE, 0.0 and a zero duration.
    Zero,
    /// NULL, the reference that designates nothing.
    Null,
    /// An integer other than zero, such as a subrange's lower bound.
    Integer(i128),
}

impl LeafDefault {
    /// The default of a value of type `ty`.
    pub(crate) fn of(ty: &IntermediateType) -> Self {
        match ty {
            IntermediateType::Reference { .. } => LeafDefault::Null,
            // The leftmost value of the subrange (IEC 61131-3 2.4.3.1).
            IntermediateType::Subrange { min_value, .. } if *min_value != 0 => {
                LeafDefault::Integer(*min_value)
            }
            _ => LeafDefault::Zero,
        }
    }

    /// The default of an element of the plain array `spec` describes. Its
    /// elements are elementary values or references; STRING elements are
    /// initialized through their headers instead.
    pub(crate) fn of_array_element(spec: &ArraySpec) -> Self {
        if spec.ref_to {
            LeafDefault::Null
        } else {
            LeafDefault::Zero
        }
    }

    /// Emits a constant load of the default, at `op_type`.
    pub(crate) fn emit(self, emitter: &mut Emitter, ctx: &mut CompileContext, op_type: OpType) {
        match (self, op_type.0) {
            (LeafDefault::Null, _) => {
                // The null sentinel (u64::MAX) that `REF_TO` slots hold.
                let pool_index = ctx.add_i64_constant(u64::MAX as i64);
                emitter.emit_load_const_i64(pool_index);
            }
            (LeafDefault::Integer(value), OpWidth::W32) => {
                let pool_index = ctx.add_i32_constant(value as i32);
                emitter.emit_load_const_i32(pool_index);
            }
            (LeafDefault::Integer(value), OpWidth::W64) => {
                let pool_index = ctx.add_i64_constant(value as i64);
                emitter.emit_load_const_i64(pool_index);
            }
            _ => emit_zero_const(emitter, ctx, op_type),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::LeafDefault;
    use ironplc_analyzer::intermediate_type::{ByteSized, IntermediateType};

    #[test]
    fn of_when_reference_then_null() {
        let ty = IntermediateType::Reference {
            target_type: Box::new(IntermediateType::Int {
                size: ByteSized::B16,
            }),
        };
        assert_eq!(LeafDefault::of(&ty), LeafDefault::Null);
    }

    #[test]
    fn of_when_subrange_then_lower_bound() {
        let ty = IntermediateType::Subrange {
            base_type: Box::new(IntermediateType::Int {
                size: ByteSized::B16,
            }),
            min_value: 5,
            max_value: 10,
        };
        assert_eq!(LeafDefault::of(&ty), LeafDefault::Integer(5));
    }

    #[test]
    fn of_when_elementary_then_zero() {
        assert_eq!(LeafDefault::of(&IntermediateType::Bool), LeafDefault::Zero);
    }
}
