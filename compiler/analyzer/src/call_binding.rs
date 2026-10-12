//! Binds the arguments of a function, function block or method call to the
//! callee's parameters, and writes the call in normal form.
//!
//! How each argument binds is a language rule, so the analyzer decides it
//! once, here, and every later stage reads the answer
//! (`specs/design/call-binding.md`):
//!
//! - A **named input** (`x := e`) binds to the `VAR_INPUT` or `VAR_IN_OUT` of
//!   that name, and an **output assignment** (`q => v`) to the `VAR_OUTPUT`.
//! - **Positional inputs** bind in declaration order to the `VAR_INPUT`
//!   parameters of a function block or method, and to the `VAR_INPUT` and
//!   `VAR_IN_OUT` parameters of a function
//!   ([`CallParameters::positional`]), and there must be one for each.
//! - Named and positional inputs may not be mixed, and no parameter may be
//!   named twice.
//!
//! A call that binds is written in **normal form**: one named input for each
//! input or `VAR_IN_OUT` it supplies, in the order the callee declares them,
//! then its output assignments in the order the callee declares those. That
//! order is the order the arguments are evaluated in. A parameter the call
//! does not supply is absent: an omitted function block input keeps its value
//! from the previous call, and a function or method call must supply every
//! input ([`Binding::omitted`]).

use ironplc_dsl::core::Id;
use ironplc_dsl::textual::{NamedInput, Output, ParamAssignmentKind};

use crate::callee_resolution::{CallParameter, CallParameters, ParameterSection};

/// Why the arguments of a call do not bind to its callee's parameters.
#[derive(Debug, PartialEq)]
pub(crate) enum BindProblem<'a> {
    /// The call mixes named and positional inputs. When it does, it is the
    /// only problem reported: with the call's shape in doubt, any other would
    /// be noise derived from a misreading of the arguments.
    Mixed,
    /// A named input names no `VAR_INPUT` or `VAR_IN_OUT`.
    UnknownInput(&'a Id),
    /// A positional call does not supply exactly one argument for each
    /// `VAR_INPUT`.
    PositionalCount { required: usize, actual: usize },
    /// An output assignment names no `VAR_OUTPUT`.
    UnknownOutput(&'a Output),
    /// A parameter is named twice, here the second time.
    Duplicate(&'a Id),
}

/// Which argument binds to which parameter, in normal order: each argument's
/// index in the call, inputs and `VAR_IN_OUT` in declaration order, then
/// outputs in declaration order.
pub(crate) struct Binding {
    arguments: Vec<usize>,
    /// The position of each input and `VAR_IN_OUT` no argument binds to.
    omitted: Vec<usize>,
}

impl Binding {
    /// The inputs and `VAR_IN_OUT` parameters of `parameters`, the parameters
    /// the call was bound to, that no argument binds to, in declaration
    /// order.
    pub(crate) fn omitted<'p>(&self, parameters: &'p CallParameters) -> Vec<&'p CallParameter> {
        let declared: Vec<_> = parameters.iter().collect();
        self.omitted
            .iter()
            .map(|&position| declared[position])
            .collect()
    }
}

/// Binds `args` to `parameters`, or returns every problem found, in argument
/// order except that a positional count comes after the named inputs and
/// before the outputs.
pub(crate) fn bind<'a>(
    parameters: &CallParameters,
    args: &'a [ParamAssignmentKind],
) -> Result<Binding, Vec<BindProblem<'a>>> {
    let has_named = args
        .iter()
        .any(|arg| matches!(arg, ParamAssignmentKind::NamedInput(_)));
    let has_positional = args
        .iter()
        .any(|arg| matches!(arg, ParamAssignmentKind::PositionalInput(_)));
    if has_named && has_positional {
        return Err(vec![BindProblem::Mixed]);
    }

    let declared: Vec<_> = parameters.iter().collect();
    // The argument bound to each parameter, by the parameter's position.
    let mut bound: Vec<Option<usize>> = vec![None; declared.len()];
    let mut problems = Vec::new();

    for (at, arg) in args.iter().enumerate() {
        if let ParamAssignmentKind::NamedInput(named) = arg {
            let position = declared.iter().position(|parameter| {
                parameter.name == named.name && parameter.section != ParameterSection::Output
            });
            match position {
                None => problems.push(BindProblem::UnknownInput(&named.name)),
                Some(position) if bound[position].is_some() => {
                    problems.push(BindProblem::Duplicate(&named.name))
                }
                Some(position) => bound[position] = Some(at),
            }
        }
    }

    if has_positional {
        let binds_positional: Vec<&CallParameter> = parameters.positional().collect();
        let inputs: Vec<usize> = declared
            .iter()
            .enumerate()
            .filter(|(_, parameter)| binds_positional.contains(parameter))
            .map(|(position, _)| position)
            .collect();
        let positional: Vec<usize> = args
            .iter()
            .enumerate()
            .filter(|(_, arg)| matches!(arg, ParamAssignmentKind::PositionalInput(_)))
            .map(|(at, _)| at)
            .collect();
        if inputs.len() == positional.len() {
            for (position, at) in inputs.into_iter().zip(positional) {
                bound[position] = Some(at);
            }
        } else {
            problems.push(BindProblem::PositionalCount {
                required: inputs.len(),
                actual: positional.len(),
            });
        }
    }

    for (at, arg) in args.iter().enumerate() {
        if let ParamAssignmentKind::Output(output) = arg {
            let position = declared.iter().position(|parameter| {
                parameter.name == output.src && parameter.section == ParameterSection::Output
            });
            match position {
                None => problems.push(BindProblem::UnknownOutput(output)),
                Some(position) if bound[position].is_some() => {
                    problems.push(BindProblem::Duplicate(&output.src))
                }
                Some(position) => bound[position] = Some(at),
            }
        }
    }

    if !problems.is_empty() {
        return Err(problems);
    }

    let in_order = |outputs: bool| {
        declared
            .iter()
            .zip(&bound)
            .filter(move |(parameter, _)| {
                (parameter.section == ParameterSection::Output) == outputs
            })
            .filter_map(|(_, at)| *at)
            .collect::<Vec<_>>()
    };
    let mut arguments = in_order(false);
    arguments.extend(in_order(true));
    let omitted = declared
        .iter()
        .zip(&bound)
        .enumerate()
        .filter(|(_, (parameter, at))| {
            parameter.section != ParameterSection::Output && at.is_none()
        })
        .map(|(position, _)| position)
        .collect();
    Ok(Binding { arguments, omitted })
}

/// `args`, the arguments of a call to a callee with `parameters`, in normal
/// form; or `args` unchanged when they do not bind, for the rules to report.
pub(crate) fn normal_form(
    parameters: &CallParameters,
    args: Vec<ParamAssignmentKind>,
) -> Result<Vec<ParamAssignmentKind>, Vec<ParamAssignmentKind>> {
    let Ok(binding) = bind(parameters, &args) else {
        return Err(args);
    };
    let inputs: Vec<&Id> = parameters
        .positional()
        .map(|parameter| &parameter.name)
        .collect();
    // A positional argument binds to the input at its position among the
    // positional arguments; name it after that input.
    let mut positional = 0;
    let mut named_args: Vec<Option<ParamAssignmentKind>> = Vec::with_capacity(args.len());
    for arg in args {
        named_args.push(Some(match arg {
            ParamAssignmentKind::PositionalInput(input) => {
                let mut name = inputs[positional].clone();
                positional += 1;
                name.span = input.expr.span.clone();
                ParamAssignmentKind::NamedInput(NamedInput {
                    name,
                    expr: input.expr,
                })
            }
            other => other,
        }));
    }
    Ok(binding
        .arguments
        .into_iter()
        .filter_map(|at| named_args[at].take())
        .collect())
}

#[cfg(test)]
mod tests {
    use ironplc_dsl::common::{FunctionBlockDeclaration, Library, LibraryElementKind};
    use ironplc_dsl::core::FileId;
    use ironplc_dsl::textual::{FbCall, StmtKind};
    use ironplc_dsl::visitor::Visitor;
    use ironplc_parser::{options::CompilerOptions, parse_program};
    use spec_test_macro::spec_test;

    use super::*;

    /// A block whose parameters are declared out of section order: an
    /// output, an input, an in-out, another input, another output.
    const BLOCK: &str = "
FUNCTION_BLOCK FB
VAR_OUTPUT done : BOOL; END_VAR
VAR_INPUT a : INT; END_VAR
VAR_IN_OUT io : INT; END_VAR
VAR_INPUT b : INT; END_VAR
VAR_OUTPUT q : INT; END_VAR
END_FUNCTION_BLOCK";

    /// The parameters of `FB`, and the arguments of the call `call`.
    fn parse_call(call: &str) -> (CallParameters, Vec<ParamAssignmentKind>) {
        let source = format!(
            "{BLOCK}
PROGRAM main
VAR inst : FB; x : INT; y : INT; z : INT; d : BOOL; r : INT; END_VAR
{call}
END_PROGRAM"
        );
        let lib = parse_program(&source, &FileId::default(), &CompilerOptions::default()).unwrap();
        let block = lib
            .elements
            .iter()
            .find_map(|element| match element {
                LibraryElementKind::FunctionBlockDeclaration(block) => Some(block),
                _ => None,
            })
            .unwrap();
        let parameters = CallParameters::of_declaration(block as &FunctionBlockDeclaration);
        (parameters, fb_call(&lib).params)
    }

    fn fb_call(lib: &Library) -> FbCall {
        struct Find(Option<FbCall>);
        impl Visitor<()> for Find {
            type Value = ();
            fn visit_stmt_kind(&mut self, node: &StmtKind) -> Result<(), ()> {
                if let StmtKind::FbCall(call) = node {
                    self.0 = Some(call.clone());
                }
                node.recurse_visit(self)
            }
        }
        let mut find = Find(None);
        let _ = find.walk(lib);
        find.0.unwrap()
    }

    /// The normal form of `call`, as `name := value` and `name => target`.
    fn normalized(call: &str) -> Vec<String> {
        let (parameters, args) = parse_call(call);
        normal_form(&parameters, args)
            .unwrap()
            .iter()
            .map(ToString::to_string)
            .collect()
    }

    /// The problems binding `call` finds, as debug text.
    fn problems(call: &str) -> Vec<String> {
        let (parameters, args) = parse_call(call);
        match bind(&parameters, &args) {
            Ok(_) => vec![],
            Err(problems) => problems.iter().map(|p| format!("{p:?}")).collect(),
        }
    }

    #[test]
    fn normal_form_when_named_out_of_order_then_inputs_then_outputs_in_declaration_order() {
        assert_eq!(
            vec!["a := x", "io := z", "b := y", "done => d", "q => r"],
            normalized("inst(q => r, b := y, done => d, io := z, a := x);")
        );
    }

    #[test]
    fn normal_form_when_positional_then_named_after_inputs_in_declaration_order() {
        assert_eq!(vec!["a := x", "b := y"], normalized("inst(x, y);"));
    }

    #[test]
    fn normal_form_when_positional_then_names_take_spans_of_arguments() {
        let (parameters, args) = parse_call("inst(x, y);");
        let spans: Vec<_> = args
            .iter()
            .filter_map(|arg| arg.input_expr().map(|expr| expr.span.clone()))
            .collect();
        let normal = normal_form(&parameters, args).unwrap();
        let names: Vec<_> = normal
            .iter()
            .map(|arg| match arg {
                ParamAssignmentKind::NamedInput(named) => named.name.span.clone(),
                _ => panic!("expected a named input"),
            })
            .collect();
        assert_eq!(spans, names);
    }

    #[test]
    fn normal_form_when_positional_with_outputs_then_outputs_after_inputs() {
        assert_eq!(
            vec!["a := x", "b := y", "q => r"],
            normalized("inst(x, y, q => r);")
        );
    }

    #[spec_test(REQ_CB_analyzer_050)]
    fn normal_form_when_input_omitted_then_absent() {
        assert_eq!(vec!["b := y"], normalized("inst(b := y);"));
    }

    #[spec_test(REQ_CB_analyzer_027)]
    fn normal_form_when_negated_output_then_keeps_negation() {
        assert_eq!(vec!["NOT done => d"], normalized("inst(NOT done => d);"));
    }

    #[test]
    fn normal_form_when_no_arguments_then_empty() {
        assert_eq!(Vec::<String>::new(), normalized("inst();"));
    }

    #[test]
    fn normal_form_when_arguments_do_not_bind_then_returns_them_unchanged() {
        let (parameters, args) = parse_call("inst(a := x, nope := y);");
        assert_eq!(Err(args.clone()), normal_form(&parameters, args));
    }

    /// The names of the inputs and in-outs `call` omits.
    fn omitted(call: &str) -> Vec<String> {
        let (parameters, args) = parse_call(call);
        let binding = bind(&parameters, &args).unwrap();
        binding
            .omitted(&parameters)
            .iter()
            .map(|parameter| parameter.name.to_string())
            .collect()
    }

    #[test]
    fn omitted_when_inputs_and_in_out_omitted_then_each_in_declaration_order() {
        assert_eq!(vec!["a", "io"], omitted("inst(b := y, q => r);"));
    }

    #[test]
    fn omitted_when_positional_then_in_out_omitted() {
        assert_eq!(vec!["io"], omitted("inst(x, y);"));
    }

    #[test]
    fn omitted_when_every_input_supplied_then_none() {
        assert!(omitted("inst(io := z, b := y, a := x);").is_empty());
    }

    #[spec_test(REQ_CB_analyzer_023)]
    fn bind_when_mixed_then_only_mixed() {
        assert_eq!(vec!["Mixed"], problems("inst(x, b := y, nope => d);"));
    }

    #[spec_test(REQ_CB_analyzer_022)]
    fn bind_when_named_input_unknown_then_unknown_input() {
        assert_eq!(1, problems("inst(nope := x);").len());
        assert!(problems("inst(nope := x);")[0].starts_with("UnknownInput"));
    }

    #[test]
    fn bind_when_named_input_is_output_then_unknown_input() {
        assert!(problems("inst(done := d);")[0].starts_with("UnknownInput"));
    }

    #[test]
    fn bind_when_positional_count_differs_then_positional_count() {
        assert_eq!(
            vec!["PositionalCount { required: 2, actual: 3 }"],
            problems("inst(x, y, z);")
        );
    }

    #[spec_test(REQ_CB_analyzer_022)]
    fn bind_when_output_unknown_then_unknown_output() {
        assert!(problems("inst(a := x, a2 => d);")[0].starts_with("UnknownOutput"));
    }

    #[test]
    fn bind_when_in_out_assigned_as_output_then_unknown_output() {
        assert!(problems("inst(io => r);")[0].starts_with("UnknownOutput"));
    }

    #[test]
    fn bind_when_input_named_twice_then_duplicate() {
        let found = problems("inst(a := x, A := y);");
        assert_eq!(1, found.len());
        assert!(found[0].starts_with("Duplicate"));
    }

    #[test]
    fn bind_when_output_named_twice_then_duplicate() {
        let found = problems("inst(q => r, q => z);");
        assert_eq!(1, found.len());
        assert!(found[0].starts_with("Duplicate"));
    }

    #[test]
    fn bind_when_several_problems_then_named_then_count_then_outputs() {
        let found = problems("inst(nope := x, a := y, a := z, gone => d);");
        assert_eq!(3, found.len());
        assert!(found[0].starts_with("UnknownInput"));
        assert!(found[1].starts_with("Duplicate"));
        assert!(found[2].starts_with("UnknownOutput"));
    }
}
