//! Semantic rule that a temporal literal names a value its type can hold.
//!
//! A temporal value is an integer count in a fixed unit: milliseconds for a
//! duration or a time of day, seconds since 1970-01-01 for a date or a
//! date-and-time. The literal's type decides how wide that count is and
//! whether it is signed, so the same question answers all four families —
//! *does this count fit the storage this type gives it?*
//!
//! | Type | Storage | Range |
//! |---|---|---|
//! | `TIME` | signed 32-bit ms | ±24.8 days |
//! | `LTIME` | signed 64-bit ms | ±292 million years |
//! | `DATE`, `DATE_AND_TIME` | unsigned 32-bit s | 1970-01-01 to 2106-02-07 |
//! | `LDATE`, `LDATE_AND_TIME` | unsigned 64-bit s | 1970-01-01 onwards |
//! | `TIME_OF_DAY`, `LTIME_OF_DAY` | unsigned ms | bounded by construction |
//!
//! The count and its storage come from the literal
//! ([`StoredCount`](ironplc_dsl::time::StoredCount)) and the comparison from
//! [`value_range::fits`](crate::value_range::fits) — the same comparison
//! codegen makes when it emits the value, so a program that analyzes cleanly
//! cannot then fail to compile for the reason this rule exists to report.
//!
//! ## The range is the literal's own
//!
//! A literal states its type in its prefix, so it is held to that type's range
//! wherever it is written — the way `rule_constant_range` holds `INT#40000` to
//! `INT` whatever it is stored into. `DATE#2200-01-01` is out of range even
//! when assigned to an `LDATE`, and the fix is to write `LDATE#2200-01-01`,
//! not to widen the variable.
//!
//! Before the prefix survived parsing, every temporal literal was the 32-bit
//! member of its family, so an `LDATE` reached no further than a `DATE`
//! (issue #1560) and an out-of-range count wrapped rather than being
//! reported: `LDATE#2200-01-01` became 2063-11-24, and `T#30d` became a
//! *negative* 19.7 days.
//!
//! ## Passes
//!
//! ```ignore
//! PROGRAM main
//!    VAR
//!       last32 : DATE := DATE#2106-02-07;
//!       far : LDATE := LDATE#2200-01-01;   (* 64 bits reach further *)
//!       wait : TIME := T#24d;
//!       long_wait : LTIME := LTIME#30d;
//!    END_VAR
//! END_PROGRAM
//! ```
//!
//! ## Fails
//!
//! ```ignore
//! PROGRAM main
//!    VAR
//!       past32 : DATE := DATE#2106-02-08;   (* past 2106-02-07 *)
//!       historic : DATE := DATE#1969-12-31; (* before the epoch *)
//!       too_long : TIME := T#30d;           (* past 24.8 days *)
//!    END_VAR
//! END_PROGRAM
//! ```
use ironplc_dsl::{
    common::{ElementaryTypeName, Library},
    core::SourceSpan,
    diagnostic::{Diagnostic, Label},
    time::{DateAndTimeLiteral, DateLiteral, DurationLiteral, StoredCount, TimeOfDayLiteral},
    visitor::Visitor,
};
use ironplc_parser::options::CompilerOptions;
use ironplc_problems::Problem;
use std::convert::Infallible;

use crate::{
    result::SemanticResult,
    rule_support::{run_rule, DiagnosticVisitor},
    semantic_context::SemanticContext,
    value_range,
};

pub fn apply(
    lib: &Library,
    _context: &SemanticContext,
    _options: &CompilerOptions,
) -> SemanticResult {
    run_rule(
        RuleTemporalLiteralRange {
            diagnostics: Vec::new(),
        },
        lib,
    )
}

struct RuleTemporalLiteralRange {
    diagnostics: Vec<Diagnostic>,
}

impl DiagnosticVisitor for RuleTemporalLiteralRange {
    fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

impl RuleTemporalLiteralRange {
    /// Reports `literal` when its type cannot hold the count it names.
    ///
    /// Every family is checked the same way and differs only in what it is
    /// called and how it spells its value, because one count and one storage
    /// decide the outcome for all of them.
    fn check(
        &mut self,
        stored: StoredCount,
        type_name: ElementaryTypeName,
        value: String,
        problem: Problem,
        span: &SourceSpan,
    ) {
        if value_range::fits(stored.count, stored.bits, stored.signed) {
            return;
        }

        let type_name = type_name.to_string();
        self.diagnostics.push(
            Diagnostic::problem(
                problem,
                Label::span(span.clone(), format!("{type_name} cannot hold this value")),
            )
            .with_context("value", &value)
            .with_context("type", &type_name),
        );
    }
}

impl Visitor<Infallible> for RuleTemporalLiteralRange {
    type Value = ();

    fn visit_duration_literal(&mut self, node: &DurationLiteral) -> Result<(), Infallible> {
        let stored = node.stored_count();
        self.check(
            stored,
            node.type_name(),
            format!("{}ms", stored.count),
            Problem::DurationLiteralOutOfRange,
            &node.span,
        );
        Ok(())
    }

    fn visit_time_of_day_literal(&mut self, node: &TimeOfDayLiteral) -> Result<(), Infallible> {
        let stored = node.stored_count();
        let (hour, minute, second, _micro) = node.hmsm();
        self.check(
            stored,
            node.type_name(),
            format!("{hour:02}:{minute:02}:{second:02}"),
            Problem::DurationLiteralOutOfRange,
            &node.span,
        );
        Ok(())
    }

    fn visit_date_literal(&mut self, node: &DateLiteral) -> Result<(), Infallible> {
        let (year, month, day) = node.ymd();
        self.check(
            node.stored_count(),
            node.type_name(),
            format!("{year}-{month:02}-{day:02}"),
            Problem::DateLiteralOutOfRange,
            &node.span,
        );
        Ok(())
    }

    fn visit_date_and_time_literal(&mut self, node: &DateAndTimeLiteral) -> Result<(), Infallible> {
        let (year, month, day) = node.ymd();
        let (hour, minute, second, _micro) = node.hmsm();
        self.check(
            node.stored_count(),
            node.type_name(),
            format!("{year}-{month:02}-{day:02}-{hour:02}:{minute:02}:{second:02}"),
            Problem::DateLiteralOutOfRange,
            &node.span,
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use ironplc_parser::options::{CompilerOptions, Dialect};
    use ironplc_problems::Problem;

    /// The options the 64-bit members need: `LTIME`, `LDATE`, `LTOD` and `LDT`
    /// are Edition 3 keywords, and are identifiers without them.
    fn edition3() -> CompilerOptions {
        CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3)
    }

    // --- The 32-bit boundary, from both sides ---
    //
    // The ranges this rule reports are only as true as these: each pair pins
    // the last value the storage holds and the first it does not.

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

    // --- A duration is signed, so both ends are 32-bit two's complement ---

    rule_ok!(
        apply_when_duration_is_last_representable_then_ok,
        "
PROGRAM main
VAR
    t : TIME := T#2147483647ms;
END_VAR
END_PROGRAM"
    );

    rule_err1_at!(
        apply_when_duration_is_past_last_representable_then_error,
        "
PROGRAM main
VAR
    t : TIME := T#30d;
END_VAR
END_PROGRAM",
        Problem::DurationLiteralOutOfRange,
        "T#30d"
    );

    rule_err1_at!(
        apply_when_duration_is_below_first_representable_then_error,
        "
PROGRAM main
VAR
    t : TIME := T#-30d;
END_VAR
END_PROGRAM",
        Problem::DurationLiteralOutOfRange,
        "T#-30d"
    );

    // --- The 64-bit members reach further, which is issue #1560's second
    //     problem: each of these was reported before the literal carried the
    //     type its prefix named.

    rule_ok_with!(
        apply_when_ldate_is_past_the_32_bit_ceiling_then_ok,
        edition3(),
        "
PROGRAM main
VAR
    d : LDATE := LDATE#2200-01-01;
END_VAR
END_PROGRAM"
    );

    rule_ok_with!(
        apply_when_ldt_is_past_the_32_bit_ceiling_then_ok,
        edition3(),
        "
PROGRAM main
VAR
    d : LDT := LDT#2200-01-01-00:00:00;
END_VAR
END_PROGRAM"
    );

    rule_ok_with!(
        apply_when_ltime_is_past_the_32_bit_ceiling_then_ok,
        edition3(),
        "
PROGRAM main
VAR
    t : LTIME := LTIME#30d;
END_VAR
END_PROGRAM"
    );

    // A 64-bit date is still bounded below by the epoch: the count is
    // unsigned at both widths.
    rule_err1_with!(
        apply_when_ldate_is_before_epoch_then_error,
        edition3(),
        "
PROGRAM main
VAR
    d : LDATE := LDATE#1969-12-31;
END_VAR
END_PROGRAM",
        Problem::DateLiteralOutOfRange
    );

    // --- The literal's own type decides, not the variable's ---

    /// `DATE#` names a `DATE` whatever it is stored into, the way `INT#40000`
    /// names an `INT`. Widening the variable does not widen the literal.
    rule_err1_at!(
        apply_when_short_literal_is_out_of_range_in_a_long_variable_then_error,
        "
PROGRAM main
VAR
    d : LDATE := DATE#2200-01-01;
END_VAR
END_PROGRAM",
        Problem::DateLiteralOutOfRange,
        "DATE#2200-01-01"
    );

    // --- Every position a literal can be written in ---

    rule_err1_at!(
        apply_when_out_of_range_literal_is_assigned_then_error,
        "
PROGRAM main
VAR
    t : TIME;
END_VAR
    t := T#30d;
END_PROGRAM",
        Problem::DurationLiteralOutOfRange,
        "T#30d"
    );

    rule_err1_at!(
        apply_when_out_of_range_literal_is_compared_then_error,
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

    // --- Every violation is reported, not just the first ---

    rule_errn!(
        apply_when_several_literals_are_out_of_range_then_reports_every_one,
        "
PROGRAM main
VAR
    a : DATE := DATE#2200-01-01;
    b : DATE := DATE#1969-12-31;
END_VAR
END_PROGRAM",
        2,
        Problem::DateLiteralOutOfRange
    );

    // --- What the rule does not report ---

    rule_ok!(
        apply_when_literals_are_in_range_then_ok,
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

    /// A time of day is bounded by construction at either width, so neither
    /// can be out of range.
    rule_ok_with!(
        apply_when_time_of_day_is_end_of_day_then_ok,
        edition3(),
        "
PROGRAM main
VAR
    t : TIME_OF_DAY := TOD#23:59:59;
    l : LTIME_OF_DAY := LTOD#23:59:59;
END_VAR
END_PROGRAM"
    );
}
