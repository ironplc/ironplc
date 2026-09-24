//! The standard time and date functions (IEC 61131-3 Section 2.5.1.5.8,
//! Table 35): arithmetic on durations, date/time offsets and differences,
//! date and time-of-day concatenation, and date-and-time decomposition.

use ironplc_dsl::common::TypeName;

use super::stdlib_function::input_param;
use crate::function_environment::FunctionSignature;

/// Returns standard time and date function definitions.
///
/// These functions provide arithmetic on time durations, date/time offsets,
/// date/time differences, and date+time concatenation.
pub(super) fn get_time_functions() -> Vec<FunctionSignature> {
    let overloads = OVERLOADS.iter().map(TimeOverload::signature);
    overloads.chain(other_time_functions()).collect()
}

/// One typed overload of an arithmetic function on the time and date types
/// (IEC 61131-3 Table 30): a fixed signature with its own name, such as
/// `ADD_TIME` for `ADD` on two `TIME` operands.
struct TimeOverload {
    name: &'static str,
    in1: &'static str,
    in2: &'static str,
    result: &'static str,
}

impl TimeOverload {
    /// The registered signature: `name(IN1 : in1, IN2 : in2) : result`.
    fn signature(&self) -> FunctionSignature {
        FunctionSignature::stdlib(
            self.name,
            TypeName::from(self.result),
            vec![input_param("IN1", self.in1), input_param("IN2", self.in2)],
        )
    }
}

/// Builds one row of [`OVERLOADS`].
const fn overload(
    name: &'static str,
    in1: &'static str,
    in2: &'static str,
    result: &'static str,
) -> TimeOverload {
    TimeOverload {
        name,
        in1,
        in2,
        result,
    }
}

/// The typed overloads of `ADD`, `SUB`, `MUL` and `DIV` on the time and date
/// types (IEC 61131-3 Table 30).
const OVERLOADS: &[TimeOverload] = &[
    // Duration arithmetic
    overload("ADD_TIME", "TIME", "TIME", "TIME"),
    overload("SUB_TIME", "TIME", "TIME", "TIME"),
    overload("MUL_TIME", "TIME", "ANY_NUM", "TIME"),
    overload("DIV_TIME", "TIME", "ANY_NUM", "TIME"),
    // Date/time plus or minus a duration
    overload("ADD_DT_TIME", "DATE_AND_TIME", "TIME", "DATE_AND_TIME"),
    overload("ADD_TOD_TIME", "TIME_OF_DAY", "TIME", "TIME_OF_DAY"),
    overload("SUB_DT_TIME", "DATE_AND_TIME", "TIME", "DATE_AND_TIME"),
    overload("SUB_TOD_TIME", "TIME_OF_DAY", "TIME", "TIME_OF_DAY"),
    // Date/time differences
    overload("SUB_DT_DT", "DATE_AND_TIME", "DATE_AND_TIME", "TIME"),
    overload("SUB_DATE_DATE", "DATE", "DATE", "TIME"),
    overload("SUB_TOD_TOD", "TIME_OF_DAY", "TIME_OF_DAY", "TIME"),
];

/// The time and date functions that are not overloads of an arithmetic
/// function: concatenation and decomposition.
fn other_time_functions() -> Vec<FunctionSignature> {
    vec![
        // Concatenation
        FunctionSignature::stdlib(
            "CONCAT_DATE_TOD",
            TypeName::from("DATE_AND_TIME"),
            vec![
                input_param("IN1", "DATE"),
                input_param("IN2", "TIME_OF_DAY"),
            ],
        ),
        // Decomposition: extract DATE or TIME_OF_DAY from DATE_AND_TIME
        FunctionSignature::stdlib(
            "DT_TO_DATE",
            TypeName::from("DATE"),
            vec![input_param("IN", "DATE_AND_TIME")],
        ),
        FunctionSignature::stdlib(
            "DATE_AND_TIME_TO_DATE",
            TypeName::from("DATE"),
            vec![input_param("IN", "DATE_AND_TIME")],
        ),
        FunctionSignature::stdlib(
            "DT_TO_TOD",
            TypeName::from("TIME_OF_DAY"),
            vec![input_param("IN", "DATE_AND_TIME")],
        ),
        FunctionSignature::stdlib(
            "DATE_AND_TIME_TO_TIME_OF_DAY",
            TypeName::from("TIME_OF_DAY"),
            vec![input_param("IN", "DATE_AND_TIME")],
        ),
    ]
}
