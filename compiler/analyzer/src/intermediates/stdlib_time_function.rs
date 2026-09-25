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
    vec![
        // Time duration arithmetic
        FunctionSignature::stdlib(
            "ADD_TIME",
            TypeName::from("TIME"),
            vec![input_param("IN1", "TIME"), input_param("IN2", "TIME")],
        ),
        FunctionSignature::stdlib(
            "SUB_TIME",
            TypeName::from("TIME"),
            vec![input_param("IN1", "TIME"), input_param("IN2", "TIME")],
        ),
        FunctionSignature::stdlib(
            "MUL_TIME",
            TypeName::from("TIME"),
            vec![input_param("IN1", "TIME"), input_param("IN2", "ANY_NUM")],
        ),
        FunctionSignature::stdlib(
            "DIV_TIME",
            TypeName::from("TIME"),
            vec![input_param("IN1", "TIME"), input_param("IN2", "ANY_NUM")],
        ),
        // Date/time + duration
        FunctionSignature::stdlib(
            "ADD_DT_TIME",
            TypeName::from("DATE_AND_TIME"),
            vec![
                input_param("IN1", "DATE_AND_TIME"),
                input_param("IN2", "TIME"),
            ],
        ),
        FunctionSignature::stdlib(
            "ADD_TOD_TIME",
            TypeName::from("TIME_OF_DAY"),
            vec![
                input_param("IN1", "TIME_OF_DAY"),
                input_param("IN2", "TIME"),
            ],
        ),
        FunctionSignature::stdlib(
            "SUB_DT_TIME",
            TypeName::from("DATE_AND_TIME"),
            vec![
                input_param("IN1", "DATE_AND_TIME"),
                input_param("IN2", "TIME"),
            ],
        ),
        FunctionSignature::stdlib(
            "SUB_TOD_TIME",
            TypeName::from("TIME_OF_DAY"),
            vec![
                input_param("IN1", "TIME_OF_DAY"),
                input_param("IN2", "TIME"),
            ],
        ),
        // Date/time differences
        FunctionSignature::stdlib(
            "SUB_DT_DT",
            TypeName::from("TIME"),
            vec![
                input_param("IN1", "DATE_AND_TIME"),
                input_param("IN2", "DATE_AND_TIME"),
            ],
        ),
        FunctionSignature::stdlib(
            "SUB_DATE_DATE",
            TypeName::from("TIME"),
            vec![input_param("IN1", "DATE"), input_param("IN2", "DATE")],
        ),
        FunctionSignature::stdlib(
            "SUB_TOD_TOD",
            TypeName::from("TIME"),
            vec![
                input_param("IN1", "TIME_OF_DAY"),
                input_param("IN2", "TIME_OF_DAY"),
            ],
        ),
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
