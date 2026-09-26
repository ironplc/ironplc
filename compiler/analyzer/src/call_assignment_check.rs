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
//! The binding of each argument to the parameter it names or occupies,
//! [`bind_inputs`], is the part of that validation any other analysis of a
//! call needs too, so it is a function of its own that the check builds on.
//! Finding the callee in the first place is `callee_resolution`'s job.

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

/// Pairs every input argument of a call with the declared parameter it
/// binds to: a named input (`x := e`) by name against `VAR_INPUT` and
/// `VAR_IN_OUT`, a positional input by its position among the `VAR_INPUT`
/// declarations alone, in declaration order. The parameter is `None` when
/// the callee declares nothing for the argument -- an unknown name, or more
/// positional arguments than inputs -- which [`check_assignments`] reports.
///
/// Output assignments (`=>`) are not inputs and are not returned.
pub(crate) fn bind_inputs<'a>(
    owner: &'a dyn HasVariables,
    params: &'a [ParamAssignmentKind],
) -> Vec<(&'a ParamAssignmentKind, Option<&'a VarDecl>)> {
    let mut positional = owner
        .variables()
        .iter()
        .filter(|item| item.var_type == VariableType::Input);
    params
        .iter()
        .filter_map(|param| match param {
            ParamAssignmentKind::NamedInput(named) => {
                Some((param, find_input_type(owner, &named.name)))
            }
            ParamAssignmentKind::PositionalInput(_) => Some((param, positional.next())),
            ParamAssignmentKind::Output(_) => None,
        })
        .collect()
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
///
/// Returns every problem found, not the first: a call that names three inputs
/// the callee does not declare is three diagnostics. The one exception is a
/// call that mixes named and positional inputs, which returns alone --- with
/// the call's shape itself in doubt, the per-parameter checks below would
/// report noise derived from a misreading of the arguments.
pub(crate) fn check_assignments(
    owner: &dyn HasVariables,
    owner_span: SourceSpan,
    call_span: SourceSpan,
    params: &[ParamAssignmentKind],
    labels: &AssignmentCheckLabels,
) -> Vec<Diagnostic> {
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
        return vec![Diagnostic::problem(
            Problem::FunctionCallMixedArgTypes,
            Label::span(call_span.clone(), labels.call_label),
        )
        .with_context(labels.context_key, &labels.owner_name.to_string())];
    }

    let mut diagnostics = Vec::new();

    // Check that the names and types match. Unassigned values are
    // permitted so we use the assignments as the set to iterate
    if !formal.is_empty() {
        // TODO check the types.
        for (param, declared) in bind_inputs(owner, params) {
            if let (ParamAssignmentKind::NamedInput(name), None) = (param, declared) {
                diagnostics.push(
                    Diagnostic::problem(
                        Problem::FunctionInvocationMissingInput,
                        Label::span(call_span.clone(), labels.call_label),
                    )
                    .with_context(labels.context_key, &labels.owner_name.to_string())
                    .with_context_id("undefined input", &name.name)
                    .with_secondary(Label::span(owner_span.clone(), labels.decl_label)),
                );
            }
        }
    }

    // Check that the number of variables matches exactly the number
    // of expected inputs and the types match.
    if !non_formal.is_empty() {
        let num_required_inputs = count_input_type(owner);
        if non_formal.len() != num_required_inputs {
            diagnostics.push(
                Diagnostic::problem(
                    Problem::FunctionInvocationRequiresFormal,
                    Label::span(call_span.clone(), labels.call_label),
                )
                .with_context(labels.context_key, &labels.owner_name.to_string())
                .with_context("required", &format!("{num_required_inputs}"))
                .with_context("actual", &format!("{}", non_formal.len())),
            );
        }
    }

    // Check that the assigned output parameter names match the actual
    // output parameter names
    for output in outputs {
        if find_output_type(owner, &output.src).is_none() {
            diagnostics.push(
                Diagnostic::problem(
                    Problem::FunctionInvocationUndefinedOutput,
                    Label::span(call_span.clone(), labels.call_label),
                )
                .with_context(labels.context_key, &labels.owner_name.to_string())
                .with_context_id("source", &output.src)
                .with_context("target", &output.tgt.to_string()),
            );
        }
    }

    diagnostics
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::parse_and_resolve_types;
    use ironplc_dsl::common::{
        FunctionBlockBodyKind, FunctionBlockDeclaration, Library, LibraryElementKind,
    };
    use ironplc_dsl::textual::{ExprKind, FbCall, StmtKind};

    /// A function block with one input, one in-out and one output, and a
    /// program that calls it with `params`.
    fn parse_call(params: &str) -> Library {
        let program = format!(
            "
FUNCTION_BLOCK FB_Bump
VAR_INPUT
    step : INT;
END_VAR
VAR_IN_OUT
    total : INT;
END_VAR
VAR_OUTPUT
    done : BOOL;
END_VAR
END_FUNCTION_BLOCK
PROGRAM main
VAR
    inst : FB_Bump;
    a : INT;
    b : INT;
    q : BOOL;
END_VAR
    inst({params});
END_PROGRAM"
        );
        parse_and_resolve_types(&program)
    }

    /// The function block and the one call the program makes to it.
    fn fb_and_call(lib: &Library) -> (&FunctionBlockDeclaration, &FbCall) {
        let fb = lib
            .elements
            .iter()
            .find_map(|element| match element {
                LibraryElementKind::FunctionBlockDeclaration(fb) => Some(fb),
                _ => None,
            })
            .unwrap();
        let call = lib
            .elements
            .iter()
            .find_map(|element| match element {
                LibraryElementKind::ProgramDeclaration(program) => match &program.body {
                    FunctionBlockBodyKind::Statements(statements) => {
                        statements.body.iter().find_map(|stmt| match stmt {
                            StmtKind::FbCall(call) => Some(call),
                            _ => None,
                        })
                    }
                    _ => None,
                },
                _ => None,
            })
            .unwrap();
        (fb, call)
    }

    /// The names of the parameters each input argument binds to, in argument
    /// order, with `-` for an unbound argument.
    fn bound_names(lib: &Library) -> Vec<String> {
        let (fb, call) = fb_and_call(lib);
        bind_inputs(fb, &call.params)
            .iter()
            .map(|(_, declared)| match declared {
                Some(decl) => decl.identifier.to_string(),
                None => "-".to_owned(),
            })
            .collect()
    }

    #[test]
    fn bind_inputs_when_named_then_binds_input_and_in_out_by_name() {
        let lib = parse_call("total := b, step := a, done => q");
        assert_eq!(vec!["total", "step"], bound_names(&lib));
    }

    #[test]
    fn bind_inputs_when_named_unknown_then_unbound() {
        let lib = parse_call("nope := a");
        assert_eq!(vec!["-"], bound_names(&lib));
    }

    #[test]
    fn bind_inputs_when_positional_then_binds_inputs_in_order_and_extra_is_unbound() {
        let lib = parse_call("a, b");
        assert_eq!(vec!["step", "-"], bound_names(&lib));
    }

    #[test]
    fn bind_inputs_when_argument_bound_then_pairs_it_with_its_expression() {
        let lib = parse_call("step := a");
        let (fb, call) = fb_and_call(&lib);
        let bound = bind_inputs(fb, &call.params);
        assert_eq!(1, bound.len());
        assert!(matches!(
            bound[0],
            (ParamAssignmentKind::NamedInput(named), Some(_))
                if matches!(named.expr.kind, ExprKind::Variable(_))
        ));
    }
}
