//! Lowers an analyzed library to the whole lowered program a backend
//! compiles (`specs/design/lowered-program.md`, "Position in the Pipeline").
//!
//! [`lower`] completes every initializer on a copy of the library, so the
//! analyzed library stays as written (`specs/design/initial-values.md`),
//! lowers the copy in one walk, and gives the whole program: or one P9999 at
//! each construct lowering does not express yet, so that `check` reports
//! what the compiler cannot generate yet (REQ-LOW-project-004).

use ironplc_analyzer::{xform_resolve_initial_values, CleanAnalysis, CompilerOptions};
use ironplc_container::policy::{StringToNumFailure, StringToNumNonNumeric};
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_ir::lowered::{NonNumeric, ParseFailure, ParsePolicy, Program};

use crate::interface::reject_interface_variables;
use crate::Lowering;

/// The whole lowered program of `analysis`; otherwise a P9999 at each
/// construct lowering does not express yet, or a P9998 when the analyzed
/// library breaks an invariant lowering relies on.
pub fn lower(analysis: &CleanAnalysis<'_>) -> Result<Program, Vec<Diagnostic>> {
    reject_interface_variables(analysis.library()).map_err(|diagnostic| vec![diagnostic])?;
    let context = analysis.context();
    let completed = xform_resolve_initial_values::apply(analysis.library().clone(), context);
    let completed =
        CleanAnalysis::new(&completed, context).map_err(|_| vec![Diagnostic::internal_error()])?;
    let parse = parse_policy(context.compiler_options());
    Lowering::new(&completed, parse)
        .lower_program()?
        .into_program()
}

/// The parse policy `options` select, for lowering to name in each
/// `STRING_TO_*` intrinsic (REQ-LOW-lowering-073).
pub fn parse_policy(options: &CompilerOptions) -> ParsePolicy {
    ParsePolicy {
        non_numeric: match options.policy_string_to_num_non_numeric {
            StringToNumNonNumeric::Reject => NonNumeric::Reject,
            StringToNumNonNumeric::IgnoreTrailing => NonNumeric::IgnoreTrailing,
            StringToNumNonNumeric::IgnoreSurrounding => NonNumeric::IgnoreSurrounding,
        },
        failure: match options.policy_string_to_num_failure {
            StringToNumFailure::Trap => ParseFailure::Trap,
            StringToNumFailure::Zero => ParseFailure::Zero,
        },
    }
}
