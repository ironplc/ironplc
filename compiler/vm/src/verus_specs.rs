//! What Verus must know about code it does not verify.
//!
//! The type declarations only make types visible to Verus; they assume
//! nothing. Each `assume_specification` is trusted: Verus takes it on faith,
//! so a wrong one undermines every proof that relies on it. Keep each no
//! stronger than the code it describes.

use vstd::prelude::*;

use ironplc_container::builtin::str_to_num;
use ironplc_container::{ConstantIndex, FbTypeId, FunctionId, TaskId, VarIndex};

use crate::error::{StringPreview, Trap};
use crate::value::Slot;

verus! {

/// The operand stack only moves slots around, so no proof needs to see
/// inside one.
#[verifier::external_type_specification]
#[verifier::external_body]
pub struct ExSlot(Slot);

/// Verified code returns `Trap` variants, so Verus must know them. It must
/// then know every payload type too; those stay opaque.
#[verifier::external_type_specification]
pub struct ExTrap(Trap);

#[verifier::external_type_specification]
#[verifier::external_body]
pub struct ExConstantIndex(ConstantIndex);

#[verifier::external_type_specification]
#[verifier::external_body]
pub struct ExVarIndex(VarIndex);

#[verifier::external_type_specification]
#[verifier::external_body]
pub struct ExFunctionId(FunctionId);

#[verifier::external_type_specification]
#[verifier::external_body]
pub struct ExTaskId(TaskId);

#[verifier::external_type_specification]
#[verifier::external_body]
pub struct ExFbTypeId(FbTypeId);

#[verifier::external_type_specification]
#[verifier::external_body]
pub struct ExStrToNumTarget(str_to_num::Target);

#[verifier::external_type_specification]
#[verifier::external_body]
pub struct ExStringPreview(StringPreview);

/// `<[T]>::swap` panics unless both indices are in bounds, and otherwise
/// exchanges the two elements. vstd does not specify it.
pub assume_specification<T>[ <[T]>::swap ](slice: &mut [T], a: usize, b: usize)
    requires
        a < old(slice)@.len(),
        b < old(slice)@.len(),
    ensures
        final(slice)@ == old(slice)@.update(a as int, old(slice)@[b as int]).update(
            b as int,
            old(slice)@[a as int],
        ),
;

} // verus!
