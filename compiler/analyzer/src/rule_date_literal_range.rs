//! Semantic rule that a date literal names a date the compiler can represent.
//!
//! A date is stored as an unsigned count of seconds since 1970-01-01
//! ([ADR-0025](../../../specs/adrs/0025-datetime-unsigned-representation.md)),
//! so the dates a program can write are the ones that count reaches: from the
//! epoch itself to 2106-02-07, the last day whose midnight fits 32 bits.
//! `DATE#2200-01-01` is not a date the compiler can emit, and neither is
//! `DATE#1969-12-31` -- an unsigned count has nowhere to put a date before
//! the epoch.
//!
//! Without this rule the second count wrapped instead: `LDATE#2200-01-01` is
//! 7,258,118,400 seconds, which keeps its low 32 bits and becomes
//! 2063-11-24 -- a date 136 years earlier than the one the program wrote,
//! with nothing said about it (issue #1560).
//!
//! ## The range is the literal's, not the destination's
//!
//! `DATE#` and `LDATE#` parse to the same literal, as `DT#` and `LDT#` do,
//! and codegen lowers all four through one accessor. The width of the
//! variable a literal is stored into never reaches that accessor, so the
//! range a literal is held to is the same wherever it is written: an
//! initializer, an assignment, a comparison, a function argument.
//!
//! The 64-bit types are wider in storage only -- the amendment to ADR-0025
//! established that they hold the same second count rather than a
//! finer-grained one -- so today an `LDATE` reaches no further than a `DATE`.
//! Issue #1560 records that as a second problem, to be fixed by giving the
//! 64-bit types a lowering path of their own; when that lands, the ceiling
//! below is the one place that widens.
//!
//! ## Passes
//!
//! ```ignore
//! PROGRAM main
//!    VAR
//!       epoch : DATE := DATE#1970-01-01;
//!       last : DATE := DATE#2106-02-07;
//!       stamp : DT := DT#2024-06-15-12:30:00;
//!    END_VAR
//! END_PROGRAM
//! ```
//!
//! ## Fails
//!
//! ```ignore
//! PROGRAM main
//!    VAR
//!       distant : DATE := DATE#2200-01-01;   (* past 2106-02-07 *)
//!       historic : DATE := DATE#1969-12-31;  (* before the epoch *)
//!    END_VAR
//! END_PROGRAM
//! ```
use ironplc_dsl::{
    common::Library,
    core::SourceSpan,
    diagnostic::{Diagnostic, Label},
    time::{DateAndTimeLiteral, DateLiteral},
    visitor::Visitor,
};
use ironplc_parser::options::CompilerOptions;
use ironplc_problems::Problem;
use std::convert::Infallible;

use crate::{
    result::SemanticResult,
    rule_support::{run_rule, DiagnosticVisitor},
    semantic_context::SemanticContext,
};

// The bounds are written without a type prefix, because one range bounds
// four spellings: a `DATE#` literal and an `LDATE#` one are the same literal
// to the compiler, as a `DT#` and an `LDT#` one are. Naming one of them in
// the message would report a program that wrote the other against a prefix it
// did not use.

/// The first date a second count reaches.
const EARLIEST_DATE: &str = "1970-01-01";

/// The last date a second count reaches.
///
/// `u32::MAX / 86_400` is 49,710 whole days, and 49,710 days after the epoch
/// is 2106-02-07.
const LATEST_DATE: &str = "2106-02-07";

/// The first date and time a second count reaches.
const EARLIEST_DATE_AND_TIME: &str = "1970-01-01-00:00:00";

/// The last date and time a second count reaches.
///
/// The 23,295 seconds `u32::MAX` has left over after 49,710 whole days are
/// 06:28:15.
const LATEST_DATE_AND_TIME: &str = "2106-02-07-06:28:15";

pub fn apply(
    lib: &Library,
    _context: &SemanticContext,
    _options: &CompilerOptions,
) -> SemanticResult {
    run_rule(
        RuleDateLiteralRange {
            diagnostics: Vec::new(),
        },
        lib,
    )
}

struct RuleDateLiteralRange {
    diagnostics: Vec<Diagnostic>,
}

impl DiagnosticVisitor for RuleDateLiteralRange {
    fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

/// Whether `seconds` is a count the unsigned 32-bit storage holds.
fn is_representable(seconds: i64) -> bool {
    u32::try_from(seconds).is_ok()
}

impl RuleDateLiteralRange {
    /// Reports a literal that names a date outside `earliest` to `latest`.
    ///
    /// The two literal types are reported the same way and differ only in how
    /// they spell the range they fell outside, because a date and a
    /// date-and-time are bounded by one second count.
    fn report(&mut self, span: &SourceSpan, literal: String, earliest: &str, latest: &str) {
        self.diagnostics.push(
            Diagnostic::problem(
                Problem::DateLiteralOutOfRange,
                Label::span(
                    span.clone(),
                    format!("Date must be in the range {earliest} to {latest}"),
                ),
            )
            .with_context("value", &literal)
            .with_context("earliest", &earliest.to_string())
            .with_context("latest", &latest.to_string()),
        );
    }
}

impl Visitor<Infallible> for RuleDateLiteralRange {
    type Value = ();

    fn visit_date_literal(&mut self, node: &DateLiteral) -> Result<(), Infallible> {
        if !is_representable(node.seconds_since_epoch()) {
            let (year, month, day) = node.ymd();
            self.report(
                &node.span,
                format!("{year}-{month:02}-{day:02}"),
                EARLIEST_DATE,
                LATEST_DATE,
            );
        }
        Ok(())
    }

    fn visit_date_and_time_literal(&mut self, node: &DateAndTimeLiteral) -> Result<(), Infallible> {
        if !is_representable(node.seconds_since_epoch()) {
            let (year, month, day) = node.ymd();
            let (hour, minute, second, _micro) = node.hmsm();
            self.report(
                &node.span,
                format!("{year}-{month:02}-{day:02}-{hour:02}:{minute:02}:{second:02}"),
                EARLIEST_DATE_AND_TIME,
                LATEST_DATE_AND_TIME,
            );
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use ironplc_parser::options::CompilerOptions;
    use ironplc_problems::Problem;

    /// The options a 64-bit date literal needs: `LDATE` and `LDT` are
    /// Edition 3 keywords, and are identifiers without this.
    fn edition3() -> CompilerOptions {
        CompilerOptions {
            allow_long_time_types: true,
            ..CompilerOptions::default()
        }
    }

    // --- The boundary, from both sides, for both literal types ---
    //
    // The constants the rule reports are only as true as these: each pair
    // pins the last value the storage holds and the first it does not.

    rule_ok!(
        apply_when_date_is_last_representable_then_ok,
        "
PROGRAM main
VAR
    d : DATE := DATE#2106-02-07;
END_VAR
END_PROGRAM"
    );

    rule_err1_at!(
        apply_when_date_is_past_last_representable_then_error,
        "
PROGRAM main
VAR
    d : DATE := DATE#2106-02-08;
END_VAR
END_PROGRAM",
        Problem::DateLiteralOutOfRange,
        "DATE#2106-02-08"
    );

    rule_ok!(
        apply_when_date_is_epoch_then_ok,
        "
PROGRAM main
VAR
    d : DATE := DATE#1970-01-01;
END_VAR
END_PROGRAM"
    );

    rule_err1_at!(
        apply_when_date_is_before_epoch_then_error,
        "
PROGRAM main
VAR
    d : DATE := DATE#1969-12-31;
END_VAR
END_PROGRAM",
        Problem::DateLiteralOutOfRange,
        "DATE#1969-12-31"
    );

    rule_ok!(
        apply_when_date_and_time_is_last_representable_then_ok,
        "
PROGRAM main
VAR
    d : DT := DT#2106-02-07-06:28:15;
END_VAR
END_PROGRAM"
    );

    rule_err1_at!(
        apply_when_date_and_time_is_past_last_representable_then_error,
        "
PROGRAM main
VAR
    d : DT := DT#2106-02-07-06:28:16;
END_VAR
END_PROGRAM",
        Problem::DateLiteralOutOfRange,
        "DT#2106-02-07-06:28:16"
    );

    rule_ok!(
        apply_when_date_and_time_is_epoch_then_ok,
        "
PROGRAM main
VAR
    d : DT := DT#1970-01-01-00:00:00;
END_VAR
END_PROGRAM"
    );

    rule_err1!(
        apply_when_date_and_time_is_before_epoch_then_error,
        "
PROGRAM main
VAR
    d : DT := DT#1969-12-31-23:59:59;
END_VAR
END_PROGRAM",
        Problem::DateLiteralOutOfRange
    );

    // --- The 64-bit types are held to the same range (issue #1560) ---

    rule_err1_with!(
        apply_when_ldate_is_past_last_representable_then_error,
        edition3(),
        "
PROGRAM main
VAR
    d : LDATE := LDATE#2200-01-01;
END_VAR
END_PROGRAM",
        Problem::DateLiteralOutOfRange
    );

    rule_err1_with!(
        apply_when_ldt_is_past_last_representable_then_error,
        edition3(),
        "
PROGRAM main
VAR
    d : LDT := LDT#2200-01-01-00:00:00;
END_VAR
END_PROGRAM",
        Problem::DateLiteralOutOfRange
    );

    // --- Every position a date literal can be written in ---

    rule_err1_at!(
        apply_when_out_of_range_date_is_assigned_then_error,
        "
PROGRAM main
VAR
    d : DATE;
END_VAR
    d := DATE#2200-01-01;
END_PROGRAM",
        Problem::DateLiteralOutOfRange,
        "DATE#2200-01-01"
    );

    rule_err1_at!(
        apply_when_out_of_range_date_is_compared_then_error,
        "
PROGRAM main
VAR
    d : DATE;
    late : BOOL;
END_VAR
    late := d > DATE#2200-01-01;
END_PROGRAM",
        Problem::DateLiteralOutOfRange,
        "DATE#2200-01-01"
    );

    rule_err1_at!(
        apply_when_out_of_range_date_is_a_global_constant_then_error,
        "
VAR_GLOBAL CONSTANT
    LIMIT : DATE := DATE#2200-01-01;
END_VAR

PROGRAM main
VAR
    d : DATE;
END_VAR
    d := LIMIT;
END_PROGRAM",
        Problem::DateLiteralOutOfRange,
        "DATE#2200-01-01"
    );

    // --- Every violation is reported, not just the first ---

    rule_errn!(
        apply_when_several_dates_are_out_of_range_then_reports_every_one,
        "
PROGRAM main
VAR
    a : DATE := DATE#2200-01-01;
    b : DATE := DATE#1969-12-31;
    c : DT := DT#2200-01-01-00:00:00;
END_VAR
END_PROGRAM",
        3,
        Problem::DateLiteralOutOfRange
    );

    // --- What the rule does not report ---

    rule_ok!(
        apply_when_dates_are_in_range_then_ok,
        "
PROGRAM main
VAR
    d : DATE := DATE#2024-06-15;
    t : TIME_OF_DAY := TOD#23:59:59;
    s : DT := DT#2024-06-15-12:30:00;
    i : TIME := T#1d;
END_VAR
END_PROGRAM"
    );
}
