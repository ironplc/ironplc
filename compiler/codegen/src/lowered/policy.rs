//! The behaviour policies a lowered intrinsic names (ADR-0049), as this
//! backend encodes them.

use ironplc_container::opcode::builtin::str_to_num::{func_id, Target};
use ironplc_container::policy::{StringToNumFailure, StringToNumNonNumeric};
use ironplc_ir::lowered::{NonNumeric, ParseFailure, ParsePolicy};

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
