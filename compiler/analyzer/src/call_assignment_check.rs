//! Shared parameter-assignment validation for function-block and method
//! invocations.
//!
//! `rule_function_block_invocation` (`FbCall`) and `rule_method_call_declared`
//! (`MethodCall`) both validate a call's `ParamAssignmentKind` list against
//! a callee's declared `VAR_INPUT`/`VAR_IN_OUT`/`VAR_OUTPUT` variables --
//! same algorithm, same diagnostics, differing only in the label text and
//! context key used in the resulting `Diagnostic` (a function block calls
//! itself "invocation"; a method calls itself "method"). Shared here so the
//! two rules can't drift out of sync on the actual validation logic.
//!
//! Which parameter each argument binds to is decided once, by
//! `call_binding::bind`; the check reports why a call does not bind. Finding
//! the callee in the first place is `callee_resolution`'s job.

use ironplc_dsl::{
    core::SourceSpan,
    diagnostic::{Diagnostic, Label},
    textual::ParamAssignmentKind,
};
use ironplc_problems::Problem;

use crate::call_binding::{bind, BindProblem};
use crate::callee_resolution::CallParameters;

/// Labels used to build the diagnostics `check_assignments` returns, so
/// the same validation logic reads naturally for both a function-block
/// invocation ("Function block invocation" / "invocation" /
/// "Function block declaration") and a method call ("Method invocation" /
/// "method" / "Method declaration").
pub(crate) struct AssignmentCheckLabels<'a> {
    /// Primary label on the call-site span, e.g. "Method invocation".
    pub(crate) call_label: &'a str,
    /// Context key paired with `owner_name`, e.g. "method" or "invocation".
    pub(crate) context_key: &'a str,
    /// The callee's own name/identity shown in diagnostic context.
    pub(crate) owner_name: &'a str,
    /// Secondary label on the declaration span, e.g. "Method declaration".
    pub(crate) decl_label: &'a str,
}

/// Reports why a call's arguments do not bind to `parameters`, the callee's
/// declared `VAR_INPUT`/`VAR_IN_OUT`/`VAR_OUTPUT` parameters
/// ([`crate::call_binding::bind`]):
///
/// - named and positional inputs may not be mixed (P4001)
/// - every named input must match a declared input (P4002)
/// - a positional call must supply every declared input, and no more (P4003)
/// - every output assignment (`=>`) must match a declared output (P4004)
/// - no parameter may be named twice (P4024)
///
/// `owner_span` is the callee declaration's span, used for the secondary
/// label on a missing-input diagnostic; a standard function block has none.
///
/// Returns every problem found, not the first: a call that names three inputs
/// the callee does not declare is three diagnostics. The one exception is a
/// call that mixes named and positional inputs, which returns alone --- with
/// the call's shape itself in doubt, the per-parameter checks would report
/// noise derived from a misreading of the arguments.
pub(crate) fn check_assignments(
    parameters: &CallParameters,
    owner_span: Option<SourceSpan>,
    call_span: SourceSpan,
    params: &[ParamAssignmentKind],
    labels: &AssignmentCheckLabels,
) -> Vec<Diagnostic> {
    let Err(problems) = bind(parameters, params) else {
        return vec![];
    };
    let label = || Label::span(call_span.clone(), labels.call_label);
    let owner = labels.owner_name.to_string();
    problems
        .into_iter()
        .map(|problem| match problem {
            BindProblem::Mixed => Diagnostic::problem(Problem::FunctionCallMixedArgTypes, label())
                .with_context(labels.context_key, &owner),
            BindProblem::UnknownInput(name) => {
                let diagnostic =
                    Diagnostic::problem(Problem::FunctionInvocationMissingInput, label())
                        .with_context(labels.context_key, &owner)
                        .with_context_id("undefined input", name);
                match &owner_span {
                    Some(span) => {
                        diagnostic.with_secondary(Label::span(span.clone(), labels.decl_label))
                    }
                    None => diagnostic,
                }
            }
            BindProblem::PositionalCount { required, actual } => {
                Diagnostic::problem(Problem::FunctionInvocationRequiresFormal, label())
                    .with_context(labels.context_key, &owner)
                    .with_context("required", &format!("{required}"))
                    .with_context("actual", &format!("{actual}"))
            }
            BindProblem::UnknownOutput(output) => {
                Diagnostic::problem(Problem::FunctionInvocationUndefinedOutput, label())
                    .with_context(labels.context_key, &owner)
                    .with_context_id("source", &output.src)
                    .with_context("target", &output.tgt.to_string())
            }
            BindProblem::Duplicate(name) => Diagnostic::problem(
                Problem::FunctionCallDuplicateNamedArg,
                Label::span(name.span.clone(), "Duplicate argument"),
            )
            .with_context(labels.context_key, &owner),
        })
        .collect()
}
