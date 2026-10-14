//! The behaviour policies a lowered intrinsic names (ADR-0049), as this
//! backend encodes them. While codegen still starts lowering, it also hands
//! lowering the policy its options select.

use ironplc_container::opcode::builtin::str_to_num::{func_id, Target};
use ironplc_container::policy::{StringToNumFailure, StringToNumNonNumeric};
use ironplc_ir::lowered::{NonNumeric, ParseFailure, ParsePolicy};

use crate::compile::StringToNumPolicies;

/// The parse policy `policies` selects, for lowering to name in each
/// `STRING_TO_*` intrinsic (REQ-LOW-lowering-073).
pub(crate) fn parse_policy(policies: &StringToNumPolicies) -> ParsePolicy {
    ParsePolicy {
        non_numeric: match policies.non_numeric {
            StringToNumNonNumeric::Reject => NonNumeric::Reject,
            StringToNumNonNumeric::IgnoreTrailing => NonNumeric::IgnoreTrailing,
            StringToNumNonNumeric::IgnoreSurrounding => NonNumeric::IgnoreSurrounding,
        },
        failure: match policies.failure {
            StringToNumFailure::Trap => ParseFailure::Trap,
            StringToNumFailure::Zero => ParseFailure::Zero,
        },
    }
}

/// The builtin that reads the string at a data region offset as a number of
/// `target`, as `policy` says.
pub(super) fn parse_builtin(target: Target, policy: ParsePolicy) -> u16 {
    let non_numeric = match policy.non_numeric {
        NonNumeric::Reject => StringToNumNonNumeric::Reject,
        NonNumeric::IgnoreTrailing => StringToNumNonNumeric::IgnoreTrailing,
        NonNumeric::IgnoreSurrounding => StringToNumNonNumeric::IgnoreSurrounding,
    };
    let failure = match policy.failure {
        ParseFailure::Trap => StringToNumFailure::Trap,
        ParseFailure::Zero => StringToNumFailure::Zero,
    };
    func_id(target, non_numeric, failure)
}
