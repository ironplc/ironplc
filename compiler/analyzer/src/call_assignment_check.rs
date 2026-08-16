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

use ironplc_dsl::{
    common::{HasVariables, VarDecl, VariableType},
    core::{Id, SourceSpan},
    diagnostic::{Diagnostic, Label},
    textual::ParamAssignmentKind,
};
use ironplc_problems::Problem;

/// Returns the first variable matching the specified name and one of the
/// variable types, or `None` if the owner does not contain a matching
/// variable.
fn find<'a>(
    owner: &'a dyn HasVariables,
    name: &'a Id,
    types: &[VariableType],
) -> Option<&'a VarDecl> {
    owner
        .variables()
        .iter()
        .find(|item| match item.identifier.symbolic_id() {
            Some(n) => n.eq(name) && types.contains(&item.var_type),
            None => false,
        })
}

fn count_input_type(owner: &dyn HasVariables) -> usize {
    owner
        .variables()
        .iter()
        .filter(|item| item.var_type == VariableType::Input)
        .count()
}

/// Returns the first VAR_INPUT or VAR_INOUT variable matching the name
/// or `None` if the owner does not contain a matching variable.
fn find_input_type<'a>(owner: &'a dyn HasVariables, name: &'a Id) -> Option<&'a VarDecl> {
    find(owner, name, &[VariableType::Input, VariableType::InOut])
}

/// Returns the first VAR_OUTPUT variable matching the name
/// or `None` if the owner does not contain a matching variable.
///
/// VAR_IN_OUT are output variables, but they are only assigned
/// through the input `:=` syntax so not included for this rule.
fn find_output_type<'a>(owner: &'a dyn HasVariables, name: &'a Id) -> Option<&'a VarDecl> {
    find(owner, name, &[VariableType::Output])
}

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

/// Validates a call's parameter assignments against `owner`'s declared
/// `VAR_INPUT`/`VAR_IN_OUT`/`VAR_OUTPUT` variables:
///
/// - named and positional inputs may not be mixed
/// - every named input must match a declared input
/// - a positional-args count must match the declared input count exactly
/// - every output assignment (`=>`) must match a declared output
///
/// `owner_span` is the callee declaration's span, used for the secondary
/// label on a missing-input diagnostic.
pub(crate) fn check_assignments(
    owner: &dyn HasVariables,
    owner_span: SourceSpan,
    call_span: SourceSpan,
    params: &[ParamAssignmentKind],
    labels: &AssignmentCheckLabels,
) -> Result<(), Diagnostic> {
    // Sort the inputs as either named, positional, and outputs
    let mut formal = Vec::new();
    let mut non_formal = Vec::new();
    let mut outputs = Vec::new();
    for param in params {
        match param {
            ParamAssignmentKind::NamedInput(n) => formal.push(n),
            ParamAssignmentKind::PositionalInput(p) => non_formal.push(p),
            // Don't care outputs here
            ParamAssignmentKind::Output(o) => outputs.push(o),
        }
    }

    // Don't allow a mixture so assert that either named is empty or
    // positional is empty
    if !formal.is_empty() && !non_formal.is_empty() {
        return Err(Diagnostic::problem(
            Problem::FunctionCallMixedArgTypes,
            Label::span(call_span.clone(), labels.call_label),
        )
        .with_context(labels.context_key, &labels.owner_name.to_string()));
    }

    // Check that the names and types match. Unassigned values are
    // permitted so we use the assignments as the set to iterate
    if !formal.is_empty() {
        // TODO check the types.
        for name in &formal {
            if find_input_type(owner, &name.name).is_none() {
                return Err(Diagnostic::problem(
                    Problem::FunctionInvocationMissingInput,
                    Label::span(call_span.clone(), labels.call_label),
                )
                .with_context(labels.context_key, &labels.owner_name.to_string())
                .with_context_id("undefined input", &name.name)
                .with_secondary(Label::span(owner_span.clone(), labels.decl_label)));
            }
        }
    }

    // Check that the number of variables matches exactly the number
    // of expected inputs and the types match.
    if !non_formal.is_empty() {
        let num_required_inputs = count_input_type(owner);
        if non_formal.len() != num_required_inputs {
            return Err(Diagnostic::problem(
                Problem::FunctionInvocationRequiresFormal,
                Label::span(call_span.clone(), labels.call_label),
            )
            .with_context(labels.context_key, &labels.owner_name.to_string())
            .with_context("required", &format!("{num_required_inputs}"))
            .with_context("actual", &format!("{}", non_formal.len())));
        }
    }

    // Check that the assigned output parameter names match the actual
    // output parameter names
    for output in outputs {
        if find_output_type(owner, &output.src).is_none() {
            return Err(Diagnostic::problem(
                Problem::FunctionInvocationUndefinedOutput,
                Label::span(call_span.clone(), labels.call_label),
            )
            .with_context(labels.context_key, &labels.owner_name.to_string())
            .with_context_id("source", &output.src)
            .with_context("target", &output.tgt.to_string()));
        }
    }

    Ok(())
}
