//! Spec conformance tests for behavior policies (container-owned
//! requirements): the policy enums' encoding contract and the
//! `STRING_TO_<numeric>` func_id block.
//!
//! Each test is annotated with `#[spec_test(REQ_BP_container_NNN)]`, which
//! adds `#[test]` and references a build-script-generated constant so the
//! test fails to compile if the requirement is removed from the spec. The
//! `all_spec_requirements_have_tests` meta-test in `spec_conformance` asserts
//! every container-owned requirement has a test.
//!
//! See `specs/design/behavior-policies.md`.

use spec_test_macro::spec_test;

use crate::builtin::str_to_num::{decode, func_id, Target, BASE, END};
use crate::builtin::{self, name};
use crate::policy::{BehaviorPolicy, StringToNumFailure, StringToNumNonNumeric};

/// REQ-BP-container-001: an alternative's encoding offset is its position in
/// `ALL`, and the first alternative is the default.
#[spec_test(REQ_BP_container_001)]
fn container_spec_req_bp_001_alternative_offset_is_position_and_first_is_default() {
    for (position, alt) in StringToNumNonNumeric::ALL.iter().enumerate() {
        assert_eq!(alt.index() as usize, position);
        assert_eq!(
            StringToNumNonNumeric::from_index(position as u16),
            Some(*alt)
        );
    }
    for (position, alt) in StringToNumFailure::ALL.iter().enumerate() {
        assert_eq!(alt.index() as usize, position);
        assert_eq!(StringToNumFailure::from_index(position as u16), Some(*alt));
    }
    assert_eq!(
        StringToNumNonNumeric::default(),
        StringToNumNonNumeric::ALL[0]
    );
    assert_eq!(StringToNumFailure::default(), StringToNumFailure::ALL[0]);
}

/// REQ-BP-container-002: the block arithmetic, and the six pinned U32 rows.
#[spec_test(REQ_BP_container_002)]
fn container_spec_req_bp_002_func_id_is_base_plus_target_stride_plus_policies() {
    use StringToNumFailure::{Trap, Zero};
    use StringToNumNonNumeric::{IgnoreSurrounding, IgnoreTrailing, Reject};

    assert_eq!(BASE, 0x0480);
    for target in Target::ALL {
        for non_numeric in StringToNumNonNumeric::ALL {
            for failure in StringToNumFailure::ALL {
                assert_eq!(
                    func_id(*target, *non_numeric, *failure),
                    0x0480 + (*target as u16) * 8 + non_numeric.index() * 2 + failure.index()
                );
            }
        }
    }
    assert_eq!(
        func_id(Target::U32, Reject, Trap),
        builtin::CONV_STR_TO_U32_REJECT_TRAP
    );
    assert_eq!(
        func_id(Target::U32, Reject, Zero),
        builtin::CONV_STR_TO_U32_REJECT_ZERO
    );
    assert_eq!(
        func_id(Target::U32, IgnoreTrailing, Trap),
        builtin::CONV_STR_TO_U32_IGNORE_TRAILING_TRAP
    );
    assert_eq!(
        func_id(Target::U32, IgnoreTrailing, Zero),
        builtin::CONV_STR_TO_U32_IGNORE_TRAILING_ZERO
    );
    assert_eq!(
        func_id(Target::U32, IgnoreSurrounding, Trap),
        builtin::CONV_STR_TO_U32_IGNORE_SURROUNDING_TRAP
    );
    assert_eq!(
        func_id(Target::U32, IgnoreSurrounding, Zero),
        builtin::CONV_STR_TO_U32_IGNORE_SURROUNDING_ZERO
    );
    // The other integer targets, at the positions the block reserves: even
    // positions unsigned, the odd position after each its signed counterpart.
    assert_eq!(
        func_id(Target::I32, Reject, Trap),
        builtin::CONV_STR_TO_I32_REJECT_TRAP
    );
    assert_eq!(builtin::CONV_STR_TO_I32_REJECT_TRAP, 0x0488);
    assert_eq!(
        func_id(Target::U8, IgnoreSurrounding, Zero),
        builtin::CONV_STR_TO_U8_IGNORE_SURROUNDING_ZERO
    );
    assert_eq!(builtin::CONV_STR_TO_U8_IGNORE_SURROUNDING_ZERO, 0x0495);
    assert_eq!(
        func_id(Target::I8, IgnoreTrailing, Trap),
        builtin::CONV_STR_TO_I8_IGNORE_TRAILING_TRAP
    );
    assert_eq!(builtin::CONV_STR_TO_I8_IGNORE_TRAILING_TRAP, 0x049A);
    assert_eq!(
        func_id(Target::U16, Reject, Zero),
        builtin::CONV_STR_TO_U16_REJECT_ZERO
    );
    assert_eq!(builtin::CONV_STR_TO_U16_REJECT_ZERO, 0x04A1);
    assert_eq!(
        func_id(Target::I16, IgnoreSurrounding, Trap),
        builtin::CONV_STR_TO_I16_IGNORE_SURROUNDING_TRAP
    );
    assert_eq!(builtin::CONV_STR_TO_I16_IGNORE_SURROUNDING_TRAP, 0x04AC);
    assert_eq!(
        func_id(Target::U64, Reject, Trap),
        builtin::CONV_STR_TO_U64_REJECT_TRAP
    );
    assert_eq!(builtin::CONV_STR_TO_U64_REJECT_TRAP, 0x04B0);
    assert_eq!(
        func_id(Target::I64, IgnoreTrailing, Zero),
        builtin::CONV_STR_TO_I64_IGNORE_TRAILING_ZERO
    );
    assert_eq!(builtin::CONV_STR_TO_I64_IGNORE_TRAILING_ZERO, 0x04BB);
}

/// REQ-BP-container-003: `decode` inverts `func_id` for every encoded ID and
/// is `None` for every other ID in the block -- exactly the IDs that are
/// named rows, so the arithmetic and the table agree.
#[spec_test(REQ_BP_container_003)]
fn container_spec_req_bp_003_decode_inverts_func_id_and_rejects_the_rest_of_the_block() {
    for target in Target::ALL {
        for non_numeric in StringToNumNonNumeric::ALL {
            for failure in StringToNumFailure::ALL {
                let id = func_id(*target, *non_numeric, *failure);
                let encoding = decode(id).unwrap();
                assert_eq!(encoding.target, *target);
                assert_eq!(encoding.non_numeric, *non_numeric);
                assert_eq!(encoding.failure, *failure);
            }
        }
    }
    assert_eq!(END, 0x04FF);
    for id in BASE..=END {
        assert_eq!(decode(id).is_some(), name(id).is_some(), "0x{id:04X}");
    }
    assert_eq!(decode(BASE - 1), None);
    assert_eq!(decode(END + 1), None);
}
