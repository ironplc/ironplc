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
//! `call_binding::bind`; the check reports why a call does not bind.
//! [`bind_inputs`] pairs each input of a call with its parameter for an
//! analysis that reads a call as written. Finding the callee in the first
//! place is `callee_resolution`'s job.

use ironplc_dsl::{
    core::SourceSpan,
    diagnostic::{Diagnostic, Label},
    textual::ParamAssignmentKind,
};
use ironplc_problems::Problem;

use crate::call_binding::{bind, BindProblem};
use crate::callee_resolution::{CallParameter, CallParameters};

/// Pairs every input argument of a call with the declared parameter it
/// binds to: a named input (`x := e`) by name against `VAR_INPUT` and
/// `VAR_IN_OUT`, a positional input by its position among the `VAR_INPUT`
/// declarations alone, in declaration order. The parameter is `None` when
/// the callee declares nothing for the argument -- an unknown name, or more
/// positional arguments than inputs -- which [`check_assignments`] reports.
///
/// Output assignments (`=>`) are not inputs and are not returned.
pub(crate) fn bind_inputs<'a>(
    parameters: &'a CallParameters,
    params: &'a [ParamAssignmentKind],
) -> Vec<(&'a ParamAssignmentKind, Option<&'a CallParameter>)> {
    let mut positional = parameters.inputs();
    params
        .iter()
        .filter_map(|param| match param {
            ParamAssignmentKind::NamedInput(named) => {
                Some((param, parameters.named_input(&named.name)))
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

#[cfg(test)]
mod tests {
    use super::*;
    use ironplc_dsl::common::{
        FunctionBlockBodyKind, FunctionBlockDeclaration, Library, LibraryElementKind,
    };
    use ironplc_dsl::textual::{FbCall, StmtKind};

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
        // As written: `bind_inputs` reads a call's arguments as the program
        // wrote them, before the binding pass puts them in normal form.
        ironplc_parser::parse_program(
            &program,
            &ironplc_dsl::core::FileId::default(),
            &ironplc_parser::options::CompilerOptions::default(),
        )
        .unwrap()
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
        let parameters = CallParameters::of_declaration(fb);
        bind_inputs(&parameters, &call.params)
            .iter()
            .map(|(_, declared)| match declared {
                Some(parameter) => parameter.name.to_string(),
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
        let parameters = CallParameters::of_declaration(fb);
        let bound = bind_inputs(&parameters, &call.params);
        assert_eq!(1, bound.len());
        assert!(matches!(
            bound[0],
            (ParamAssignmentKind::NamedInput(named), Some(_))
                if named.expr.to_string() == "a"
        ));
    }
}
