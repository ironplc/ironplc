//! The standard time and date functions (IEC 61131-3 Section 2.5.1.5.8,
//! Table 35): arithmetic on durations, date/time offsets and differences,
//! date and time-of-day concatenation, and date-and-time decomposition.
//!
//! The arithmetic functions are the typed overloads of `ADD`, `SUB`, `MUL`
//! and `DIV` (Table 30). Each has a short form over `TIME`, `DATE`,
//! `TIME_OF_DAY` and `DATE_AND_TIME` and a long form of the same shape over
//! `LTIME`, `LDATE`, `LTIME_OF_DAY` and `LDATE_AND_TIME` (third edition).
//! [`OVERLOADS`] states the short forms; the long forms are derived from
//! them. Both are registered in every dialect: without
//! `--allow-long-time-types` no program can declare an operand of a long
//! type, so the long forms are unreachable rather than harmful.

use ironplc_dsl::common::TypeName;

use super::stdlib_function::input_param;
use crate::function_environment::FunctionSignature;

/// Returns standard time and date function definitions.
///
/// These functions provide arithmetic on time durations, date/time offsets,
/// date/time differences, and date+time concatenation.
pub(super) fn get_time_functions() -> Vec<FunctionSignature> {
    let short = OVERLOADS.iter().map(TimeOverload::signature);
    let long = OVERLOADS
        .iter()
        .filter_map(|o| o.long())
        .map(|o| o.signature());
    short.chain(long).chain(other_time_functions()).collect()
}

/// One typed overload of an arithmetic function on the time and date types
/// (IEC 61131-3 Table 30): a fixed signature with its own name, such as
/// `ADD_TIME` for `ADD` on two `TIME` operands.
#[derive(Clone, Copy)]
pub(crate) struct TimeOverload {
    pub(crate) name: &'static str,
    pub(crate) in1: &'static str,
    pub(crate) in2: &'static str,
    pub(crate) result: &'static str,
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

    /// The long form of this overload: the same shape with each temporal
    /// type replaced by its long-width type, under the long name. `None`
    /// only for a row [`long_form`] does not name, which the tests rule out
    /// for every row of [`OVERLOADS`].
    pub(crate) fn long(&self) -> Option<TimeOverload> {
        Some(TimeOverload {
            name: long_form(self.name)?,
            in1: long_type(self.in1),
            in2: long_type(self.in2),
            result: long_type(self.result),
        })
    }
}

/// Returns the short-form overload named `name`, or `None` when `name` is
/// not one. The name is matched exactly, as the operator-form table and
/// [`OVERLOADS`] spell it.
pub(crate) fn short_overload(name: &str) -> Option<&'static TimeOverload> {
    OVERLOADS.iter().find(|o| o.name == name)
}

/// Returns the name of the long form of the typed overload `short`, or
/// `None` when `short` is not the name of a short-form overload.
///
/// IEC 61131-3 (third edition) names each long form by replacing every
/// temporal type in the short name with its long counterpart.
pub(crate) fn long_form(short: &str) -> Option<&'static str> {
    match short {
        "ADD_TIME" => Some("ADD_LTIME"),
        "SUB_TIME" => Some("SUB_LTIME"),
        "MUL_TIME" => Some("MUL_LTIME"),
        "DIV_TIME" => Some("DIV_LTIME"),
        "ADD_DT_TIME" => Some("ADD_LDT_LTIME"),
        "ADD_TOD_TIME" => Some("ADD_LTOD_LTIME"),
        "SUB_DT_TIME" => Some("SUB_LDT_LTIME"),
        "SUB_TOD_TIME" => Some("SUB_LTOD_LTIME"),
        "SUB_DT_DT" => Some("SUB_LDT_LDT"),
        "SUB_DATE_DATE" => Some("SUB_LDATE_LDATE"),
        "SUB_TOD_TOD" => Some("SUB_LTOD_LTOD"),
        _ => None,
    }
}

/// Returns the long-width type of the temporal type `short`, or `short`
/// itself for a type that has no long form (the `ANY_NUM` of `MUL_TIME`).
fn long_type(short: &'static str) -> &'static str {
    match short {
        "TIME" => "LTIME",
        "DATE" => "LDATE",
        "TIME_OF_DAY" => "LTIME_OF_DAY",
        "DATE_AND_TIME" => "LDATE_AND_TIME",
        other => other,
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

#[cfg(test)]
mod tests {
    use super::*;
    use ironplc_dsl::core::Id;
    use rstest::rstest;

    /// Returns the registered time function named `name`, if there is one.
    fn registered(name: &str) -> Option<FunctionSignature> {
        get_time_functions()
            .into_iter()
            .find(|sig| sig.name == Id::from(name))
    }

    /// Every typed overload, short and long, pinned: the long form is the
    /// short form's shape over the long-width types.
    #[rstest]
    #[case::add_time("ADD_TIME", "TIME", "TIME", "TIME")]
    #[case::add_ltime("ADD_LTIME", "LTIME", "LTIME", "LTIME")]
    #[case::sub_time("SUB_TIME", "TIME", "TIME", "TIME")]
    #[case::sub_ltime("SUB_LTIME", "LTIME", "LTIME", "LTIME")]
    #[case::mul_time("MUL_TIME", "TIME", "ANY_NUM", "TIME")]
    #[case::mul_ltime("MUL_LTIME", "LTIME", "ANY_NUM", "LTIME")]
    #[case::div_time("DIV_TIME", "TIME", "ANY_NUM", "TIME")]
    #[case::div_ltime("DIV_LTIME", "LTIME", "ANY_NUM", "LTIME")]
    #[case::add_dt_time("ADD_DT_TIME", "DATE_AND_TIME", "TIME", "DATE_AND_TIME")]
    #[case::add_ldt_ltime("ADD_LDT_LTIME", "LDATE_AND_TIME", "LTIME", "LDATE_AND_TIME")]
    #[case::add_tod_time("ADD_TOD_TIME", "TIME_OF_DAY", "TIME", "TIME_OF_DAY")]
    #[case::add_ltod_ltime("ADD_LTOD_LTIME", "LTIME_OF_DAY", "LTIME", "LTIME_OF_DAY")]
    #[case::sub_dt_time("SUB_DT_TIME", "DATE_AND_TIME", "TIME", "DATE_AND_TIME")]
    #[case::sub_ldt_ltime("SUB_LDT_LTIME", "LDATE_AND_TIME", "LTIME", "LDATE_AND_TIME")]
    #[case::sub_tod_time("SUB_TOD_TIME", "TIME_OF_DAY", "TIME", "TIME_OF_DAY")]
    #[case::sub_ltod_ltime("SUB_LTOD_LTIME", "LTIME_OF_DAY", "LTIME", "LTIME_OF_DAY")]
    #[case::sub_dt_dt("SUB_DT_DT", "DATE_AND_TIME", "DATE_AND_TIME", "TIME")]
    #[case::sub_ldt_ldt("SUB_LDT_LDT", "LDATE_AND_TIME", "LDATE_AND_TIME", "LTIME")]
    #[case::sub_date_date("SUB_DATE_DATE", "DATE", "DATE", "TIME")]
    #[case::sub_ldate_ldate("SUB_LDATE_LDATE", "LDATE", "LDATE", "LTIME")]
    #[case::sub_tod_tod("SUB_TOD_TOD", "TIME_OF_DAY", "TIME_OF_DAY", "TIME")]
    #[case::sub_ltod_ltod("SUB_LTOD_LTOD", "LTIME_OF_DAY", "LTIME_OF_DAY", "LTIME")]
    fn get_time_functions_when_typed_overload_then_registered_with_its_shape(
        #[case] name: &str,
        #[case] in1: &str,
        #[case] in2: &str,
        #[case] result: &str,
    ) {
        let sig = registered(name);
        assert!(sig.is_some(), "{name} is not registered");
        let sig = sig.unwrap();
        let params: Vec<(Id, TypeName)> = sig
            .parameters
            .iter()
            .map(|p| (p.name.clone(), p.param_type.clone()))
            .collect();
        assert_eq!(
            params,
            vec![
                (Id::from("IN1"), TypeName::from(in1)),
                (Id::from("IN2"), TypeName::from(in2)),
            ]
        );
        assert!(sig.parameters.iter().all(|p| p.is_input));
        assert_eq!(
            sig.return_type.unwrap().to_type_name(),
            TypeName::from(result)
        );
    }

    #[test]
    fn long_form_when_every_overload_then_names_a_distinct_long_form() {
        let mut long: Vec<&str> = OVERLOADS
            .iter()
            .map(|o| long_form(o.name).unwrap())
            .collect();
        long.sort_unstable();
        long.dedup();
        assert_eq!(long.len(), OVERLOADS.len());
    }

    /// Every long form is callable through the full analysis pipeline with
    /// long operands, and a short operand of the same family is accepted
    /// where a long one is expected.
    #[test]
    fn analyze_when_long_typed_time_functions_called_then_clean() {
        use crate::stages::analyze;
        use ironplc_dsl::core::FileId;
        use ironplc_parser::options::{CompilerOptions, Dialect};

        let program = "
PROGRAM main
  VAR
    lt : LTIME;
    tod_long : LTIME_OF_DAY;
    date_long : LDATE;
    dt_long : LDATE_AND_TIME;
    t : TIME;
    r : REAL;
  END_VAR
  lt := ADD_LTIME(lt, lt);
  lt := ADD_LTIME(lt, t);
  lt := SUB_LTIME(lt, lt);
  lt := MUL_LTIME(lt, r);
  lt := DIV_LTIME(lt, 2);
  tod_long := ADD_LTOD_LTIME(tod_long, lt);
  tod_long := SUB_LTOD_LTIME(tod_long, lt);
  lt := SUB_LTOD_LTOD(tod_long, tod_long);
  dt_long := ADD_LDT_LTIME(dt_long, lt);
  dt_long := SUB_LDT_LTIME(dt_long, lt);
  lt := SUB_LDT_LDT(dt_long, dt_long);
  lt := SUB_LDATE_LDATE(date_long, date_long);
END_PROGRAM";
        let options = CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3);
        let library = ironplc_parser::parse_program(program, &FileId::default(), &options).unwrap();
        let (_library, context) = analyze(&[&library], &options).unwrap();
        assert!(
            context.diagnostics().is_empty(),
            "{:?}",
            context.diagnostics()
        );
    }

    #[test]
    fn long_form_when_not_a_short_overload_then_none() {
        assert_eq!(long_form("ADD_LTIME"), None);
        assert_eq!(long_form("CONCAT_DATE_TOD"), None);
        assert_eq!(long_form("ADD"), None);
    }
}
