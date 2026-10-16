//! Writes the arguments of every function, function block and method call in
//! normal form.
//!
//! Which parameter each argument of a call binds to, and the order the
//! arguments are evaluated in, are decisions of the language, so the analyzer
//! makes them and records them in the call itself (ADR-0058). This pass
//! rewrites each call whose arguments bind into the normal form
//! `call_binding::normal_form` defines: one named input for each input the
//! call supplies, in the order the callee declares them, then its output
//! assignments in declaration order. Every later stage, the rules, the
//! recording pass and code generation, reads that form and binds nothing
//! itself. A method call is written in normal form against the method its
//! receiver's block declares or inherits, and a function call against the
//! function's signature. See `specs/design/call-binding.md`.
//!
//! Each output assignment of a call that binds also records the value the
//! call stores in its target
//! ([`ironplc_dsl::textual::Output::read`]): the output the call leaves,
//! `CallOutput(q)`, inverted for a negated output. The resolver types it as
//! the output's declared type, the rules check it against the target as they
//! check an assigned value, and the recording pass converts it to the
//! target's type, so an output assignment is checked and compiled as an
//! assignment of the output's value is.
//!
//! A call whose arguments do not bind keeps them as written. For a function
//! block or method call, `rule_function_block_invocation` or
//! `rule_method_call_declared` reports why. For a function call, this pass
//! reports a parameter named twice (P4024) and a name that is no parameter
//! (P4023), and `rule_function_call_declared` reports mixed arguments and the
//! argument count.

use ironplc_dsl::common::Library;
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_dsl::fold::Fold;
use ironplc_dsl::scope::ScopeNode;
use ironplc_dsl::textual::{FbCall, Function, MethodCall, ParamAssignmentKind};
use ironplc_problems::Problem;
use std::convert::Infallible;

use crate::call_binding::{bind, normal_form, BindProblem};
use crate::callee_resolution::{instance_type, receiver_block, CallParameters, MethodParameters};
use crate::function_environment::FunctionEnvironment;
use crate::symbol_environment::{ScopeTracker, SymbolEnvironment};
use crate::type_environment::TypeEnvironment;

/// Writes every function, function block and method call of `lib` whose
/// arguments bind in normal form, and returns the problems of the function
/// calls whose arguments do not.
pub fn apply(
    lib: Library,
    symbols: &SymbolEnvironment,
    types: &TypeEnvironment,
    functions: &FunctionEnvironment,
) -> (Library, Vec<Diagnostic>) {
    let mut binder = CallArgumentBinder {
        parameters: MethodParameters::of_library(&lib, types),
        symbols,
        types,
        functions,
        scope: ScopeTracker::default(),
        diagnostics: vec![],
    };
    let Ok(lib) = binder.fold_library(lib);
    (lib, binder.diagnostics)
}

struct CallArgumentBinder<'a> {
    /// The parameters of every method a call can name.
    parameters: MethodParameters,
    symbols: &'a SymbolEnvironment,
    types: &'a TypeEnvironment,
    functions: &'a FunctionEnvironment,
    /// Where the fold is, to find the instance a call names.
    scope: ScopeTracker,
    diagnostics: Vec<Diagnostic>,
}

impl Fold<Infallible> for CallArgumentBinder<'_> {
    fn enter_scope(&mut self, node: ScopeNode<'_>) -> Result<(), Infallible> {
        self.scope.enter(&node);
        Ok(())
    }

    fn exit_scope(&mut self) {
        self.scope.exit();
    }

    fn fold_function(&mut self, node: Function) -> Result<Function, Infallible> {
        let mut node = Function::recurse_fold(node, self)?;
        let Some(signature) = self.functions.get(&node.name) else {
            return Ok(node);
        };
        let inputs = node
            .param_assignment
            .iter()
            .filter(|arg| arg.input_expr().is_some())
            .count();
        let parameters = CallParameters::of_function(signature, inputs);
        if let Err(problems) = bind(&parameters, &node.param_assignment) {
            self.diagnostics
                .extend(problems.iter().filter_map(function_call_problem));
        }
        node.param_assignment = match normal_form(&parameters, node.param_assignment) {
            Ok(mut normal) => {
                read_outputs(&mut normal);
                normal
            }
            Err(args) => args,
        };
        Ok(node)
    }

    fn fold_fb_call(&mut self, node: FbCall) -> Result<FbCall, Infallible> {
        let mut node = FbCall::recurse_fold(node, self)?;
        let parameters = instance_type(
            &node.var_name,
            &self.scope.current(),
            self.symbols,
            self.types,
        )
        .and_then(|block| CallParameters::of_block(&block, self.types));
        if let Some(parameters) = parameters {
            node.params = match normal_form(&parameters, node.params) {
                Ok(mut normal) => {
                    read_outputs(&mut normal);
                    normal
                }
                Err(args) => args,
            };
        }
        Ok(node)
    }

    fn fold_method_call(&mut self, node: MethodCall) -> Result<MethodCall, Infallible> {
        let mut node = MethodCall::recurse_fold(node, self)?;
        let block = receiver_block(
            &node.receiver,
            &self.scope.current(),
            self.symbols,
            self.types,
        );
        let parameters = block.and_then(|block| self.parameters.method(&block, &node.method));
        if let Some(parameters) = parameters {
            node.params = match normal_form(parameters, node.params) {
                Ok(mut normal) => {
                    read_outputs(&mut normal);
                    normal
                }
                Err(args) => args,
            };
        }
        Ok(node)
    }
}

/// Records, on each output assignment of `args`, the arguments of a call
/// that bind, the value the call stores in its target.
fn read_outputs(args: &mut [ParamAssignmentKind]) {
    for arg in args {
        if let ParamAssignmentKind::Output(output) = arg {
            output.value = Some(output.read());
        }
    }
}

/// The diagnostic this pass reports for `problem` in a function call, if it
/// reports one: a parameter named twice (P4024), a name that is no input
/// (P4023) and an output assignment that names no output (P4004). Mixed
/// arguments and the count are `rule_function_call_declared`'s.
fn function_call_problem(problem: &BindProblem) -> Option<Diagnostic> {
    match problem {
        BindProblem::Duplicate(name) => Some(Diagnostic::problem(
            Problem::FunctionCallDuplicateNamedArg,
            Label::span(name.span.clone(), "Duplicate argument"),
        )),
        BindProblem::UnknownInput(name) => Some(Diagnostic::problem(
            Problem::FunctionCallNamedArgUndeclared,
            Label::span(name.span.clone(), "Undeclared parameter"),
        )),
        BindProblem::UnknownOutput(output) => Some(
            Diagnostic::problem(
                Problem::FunctionInvocationUndefinedOutput,
                Label::span(output.src.span.clone(), "Undeclared output"),
            )
            .with_context_id("source", &output.src),
        ),
        BindProblem::Mixed | BindProblem::PositionalCount { .. } => None,
    }
}

#[cfg(test)]
mod tests {
    use ironplc_dsl::core::{FileId, Located, SourceSpan};
    use ironplc_dsl::textual::{
        Expr, ExprKind, ExprType, Function, MethodCall, Output, ParamAssignmentKind, StmtKind,
    };
    use ironplc_dsl::visitor::Visitor;
    use ironplc_parser::{options::CompilerOptions, parse_program};
    use spec_test_macro::spec_test;

    use crate::stages::resolve_types;

    /// The arguments of every function block call in `program`, after type
    /// resolution, one string per call.
    fn calls(program: &str) -> Vec<String> {
        let options = CompilerOptions {
            allow_fb_inheritance: true,
            ..CompilerOptions::default()
        };
        let lib = parse_program(program, &FileId::default(), &options).unwrap();
        let (lib, _) = resolve_types(&[&lib], &options).unwrap();
        struct Calls(Vec<String>);
        impl Visitor<()> for Calls {
            type Value = ();
            fn visit_stmt_kind(&mut self, node: &StmtKind) -> Result<(), ()> {
                if let StmtKind::FbCall(call) = node {
                    let args: Vec<String> = call.params.iter().map(ToString::to_string).collect();
                    self.0.push(args.join(", "));
                }
                node.recurse_visit(self)
            }
        }
        let mut calls = Calls(vec![]);
        let _ = calls.walk(&lib);
        calls.0
    }

    /// Every method call in `program`, after type resolution, one string per
    /// call: the method's name, then its arguments.
    fn method_calls(program: &str) -> Vec<String> {
        let options = CompilerOptions {
            allow_fb_inheritance: true,
            ..CompilerOptions::default()
        };
        let lib = parse_program(program, &FileId::default(), &options).unwrap();
        let (lib, _) = resolve_types(&[&lib], &options).unwrap();
        struct Calls(Vec<String>);
        impl Visitor<()> for Calls {
            type Value = ();
            fn visit_method_call(&mut self, node: &MethodCall) -> Result<(), ()> {
                let args: Vec<String> = node.params.iter().map(ToString::to_string).collect();
                self.0.push(format!("{}: {}", node.method, args.join(", ")));
                node.recurse_visit(self)
            }
        }
        let mut calls = Calls(vec![]);
        let _ = calls.walk(&lib);
        calls.0
    }

    /// A block whose method `M` declares an in-out between two inputs, and
    /// whose derived block inherits `M` and overrides `Get`.
    const METHODS: &str = "
FUNCTION_BLOCK Base
METHOD M
VAR_INPUT a : DINT; END_VAR
VAR_IN_OUT io : DINT; END_VAR
VAR_INPUT b : DINT; END_VAR
END_METHOD
METHOD Get : DINT
VAR_INPUT x : DINT; y : DINT; END_VAR
END_METHOD
END_FUNCTION_BLOCK
FUNCTION_BLOCK Derived EXTENDS Base
METHOD Get : DINT
VAR_INPUT y : DINT; x : DINT; END_VAR
END_METHOD
END_FUNCTION_BLOCK";

    #[spec_test(REQ_CB_analyzer_028)]
    fn apply_when_method_named_out_of_declaration_order_then_declaration_order() {
        let program = format!(
            "{METHODS}
PROGRAM main VAR inst : Base; t : DINT; END_VAR
inst.M(b := 2, io := t, a := 1);
END_PROGRAM"
        );
        assert_eq!(vec!["M: a := 1, io := t, b := 2"], method_calls(&program));
    }

    #[spec_test(REQ_CB_analyzer_029)]
    fn apply_when_method_positional_then_named_after_var_inputs() {
        let program = format!(
            "{METHODS}
PROGRAM main VAR inst : Base; r : DINT; END_VAR
r := inst.Get(1, 2);
END_PROGRAM"
        );
        assert_eq!(vec!["Get: x := 1, y := 2"], method_calls(&program));
    }

    #[spec_test(REQ_CB_analyzer_028)]
    fn apply_when_method_inherited_then_declaration_order_of_base_method() {
        let program = format!(
            "{METHODS}
PROGRAM main VAR d : Derived; t : DINT; END_VAR
d.M(b := 2, io := t, a := 1);
END_PROGRAM"
        );
        assert_eq!(vec!["M: a := 1, io := t, b := 2"], method_calls(&program));
    }

    #[spec_test(REQ_CB_analyzer_028)]
    fn apply_when_method_overridden_then_declaration_order_of_own_method() {
        let program = format!(
            "{METHODS}
PROGRAM main VAR d : Derived; b : Base; r : DINT; END_VAR
r := d.Get(x := 1, y := 2);
r := b.Get(y := 2, x := 1);
END_PROGRAM"
        );
        assert_eq!(
            vec!["Get: y := 2, x := 1", "Get: x := 1, y := 2"],
            method_calls(&program)
        );
    }

    #[spec_test(REQ_CB_analyzer_030)]
    fn apply_when_this_and_super_then_declaration_order_of_method_they_reach() {
        let program = "
FUNCTION_BLOCK Base
METHOD Get : DINT
VAR_INPUT x : DINT; y : DINT; END_VAR
END_METHOD
END_FUNCTION_BLOCK
FUNCTION_BLOCK Derived EXTENDS Base
METHOD Get : DINT
VAR_INPUT y : DINT; x : DINT; END_VAR
END_METHOD
METHOD Run
VAR r : DINT; END_VAR
r := THIS^.Get(x := 1, y := 2);
r := SUPER^.Get(y := 2, x := 1);
END_METHOD
END_FUNCTION_BLOCK
PROGRAM main VAR d : Derived; END_VAR
END_PROGRAM";
        assert_eq!(
            vec!["Get: y := 2, x := 1", "Get: x := 1, y := 2"],
            method_calls(program)
        );
    }

    #[test]
    fn apply_when_method_arguments_do_not_bind_then_kept_as_written() {
        let program = format!(
            "{METHODS}
PROGRAM main VAR inst : Base; r : DINT; END_VAR
r := inst.Get(y := 2, nope := 1);
END_PROGRAM"
        );
        assert_eq!(vec!["Get: y := 2, nope := 1"], method_calls(&program));
    }

    #[test]
    fn apply_when_method_not_declared_then_kept_as_written() {
        let program = format!(
            "{METHODS}
PROGRAM main VAR inst : Base; r : DINT; END_VAR
r := inst.Missing(y := 2, x := 1);
END_PROGRAM"
        );
        assert_eq!(vec!["Missing: y := 2, x := 1"], method_calls(&program));
    }

    const BLOCK: &str = "
FUNCTION_BLOCK FB
VAR_INPUT a : DINT; b : DINT; END_VAR
VAR_OUTPUT q : DINT; done : BOOL; END_VAR
END_FUNCTION_BLOCK";

    #[spec_test(REQ_CB_analyzer_020)]
    fn apply_when_named_out_of_declaration_order_then_declaration_order() {
        let program = format!(
            "{BLOCK}
PROGRAM main VAR inst : FB; x : DINT; r : DINT; d : BOOL; END_VAR
inst(done => d, b := x, q => r, a := 1);
END_PROGRAM"
        );
        assert_eq!(vec!["a := 1, b := x, q => r, done => d"], calls(&program));
    }

    #[spec_test(REQ_CB_analyzer_021)]
    fn apply_when_positional_then_named_inputs() {
        let program = format!(
            "{BLOCK}
PROGRAM main VAR inst : FB; END_VAR
inst(1, 2);
END_PROGRAM"
        );
        assert_eq!(vec!["a := 1, b := 2"], calls(&program));
    }

    /// A call to a block that extends another binds the inputs and outputs
    /// it inherits, and writes them ahead of the block's own.
    #[spec_test(REQ_CB_analyzer_053)]
    fn apply_when_block_extends_then_inherited_arguments_first() {
        let blocks = "
FUNCTION_BLOCK Base
VAR_INPUT count : DINT; END_VAR
VAR_OUTPUT seen : DINT; END_VAR
END_FUNCTION_BLOCK
FUNCTION_BLOCK Derived EXTENDS Base
VAR_INPUT speed : DINT; END_VAR
VAR_OUTPUT twice : DINT; END_VAR
END_FUNCTION_BLOCK";
        let program = format!(
            "{blocks}
PROGRAM main VAR d : Derived; x : DINT; r : DINT; s : DINT; END_VAR
d(twice => r, speed := x, seen => s, count := 3);
d(1, 2);
END_PROGRAM"
        );
        assert_eq!(
            vec![
                "count := 3, speed := x, seen => s, twice => r",
                "count := 1, speed := 2"
            ],
            calls(&program)
        );
    }

    #[test]
    fn apply_when_standard_block_positional_then_named_inputs() {
        let program = "
PROGRAM main VAR t : TON; preset : TIME; END_VAR
t(TRUE, preset);
END_PROGRAM";
        assert_eq!(vec!["IN := TRUE, PT := preset"], calls(program));
    }

    #[test]
    fn apply_when_call_in_method_then_normalized_from_its_scope() {
        let program = format!(
            "{BLOCK}
FUNCTION_BLOCK Outer
VAR inner : FB; END_VAR
METHOD Run
inner(b := 2, a := 1);
END_METHOD
END_FUNCTION_BLOCK
PROGRAM main VAR o : Outer; END_VAR
END_PROGRAM"
        );
        assert_eq!(vec!["a := 1, b := 2"], calls(&program));
    }

    #[test]
    fn apply_when_arguments_do_not_bind_then_kept_as_written() {
        let program = format!(
            "{BLOCK}
PROGRAM main VAR inst : FB; END_VAR
inst(b := 2, nope := 1);
END_PROGRAM"
        );
        assert_eq!(vec!["b := 2, nope := 1"], calls(&program));
    }

    /// The value each output assignment of every call in `program` stores,
    /// after type resolution, with its type: `q => r: CallOutput(q) (DINT)`,
    /// or `q => r: none` when the call records none.
    fn output_values(program: &str) -> Vec<String> {
        let options = CompilerOptions {
            allow_fb_inheritance: true,
            ..CompilerOptions::default()
        };
        let lib = parse_program(program, &FileId::default(), &options).unwrap();
        let (lib, context) = resolve_types(&[&lib], &options).unwrap();
        struct Outputs(Vec<Output>);
        impl Visitor<()> for Outputs {
            type Value = ();
            fn visit_output(&mut self, node: &Output) -> Result<(), ()> {
                self.0.push(node.clone());
                node.recurse_visit(self)
            }
        }
        let mut outputs = Outputs(vec![]);
        let _ = outputs.walk(&lib);
        outputs
            .0
            .iter()
            .map(|output| {
                let value = match &output.value {
                    None => "none".to_string(),
                    Some(value) => {
                        let ty = value
                            .expr_type
                            .as_ref()
                            .and_then(ExprType::type_id)
                            .and_then(|id| context.types().name_of(id))
                            .map(|name| name.to_string().to_uppercase())
                            .unwrap_or_else(|| "untyped".to_string());
                        format!("{} ({ty})", read(value))
                    }
                };
                let output = ParamAssignmentKind::Output(output.clone());
                format!("{output}: {value}")
            })
            .collect()
    }

    /// `value`, with a call output written as `CallOutput(q)`.
    fn read(value: &Expr) -> String {
        match &value.kind {
            ExprKind::UnaryOp(unary) => format!("NOT {}", read(&unary.term)),
            ExprKind::CallOutput(output) => format!("CallOutput({output})"),
            _ => value.to_string(),
        }
    }

    #[spec_test(REQ_CB_analyzer_036)]
    fn apply_when_output_assigned_then_value_reads_call_output() {
        let program = format!(
            "{BLOCK}
PROGRAM main VAR inst : FB; r : LINT; d : BOOL; END_VAR
inst(NOT done => d, q => r);
END_PROGRAM"
        );
        assert_eq!(
            vec![
                "q => r: CallOutput(q) (DINT)",
                "NOT done => d: NOT CallOutput(done) (BOOL)"
            ],
            output_values(&program)
        );
    }

    #[spec_test(REQ_CB_analyzer_036)]
    fn apply_when_standard_block_output_assigned_then_value_reads_call_output() {
        let program = "
PROGRAM main VAR t : TON; e : LTIME; END_VAR
t(IN := TRUE, ET => e);
END_PROGRAM";
        assert_eq!(
            vec!["ET => e: CallOutput(ET) (TIME)"],
            output_values(program)
        );
    }

    #[spec_test(REQ_CB_analyzer_036)]
    fn apply_when_output_assigned_then_value_written_where_output_named() {
        let program = format!(
            "{BLOCK}
PROGRAM main VAR inst : FB; r : DINT; END_VAR
inst(q => r);
END_PROGRAM"
        );
        let options = CompilerOptions::default();
        let lib = parse_program(&program, &FileId::default(), &options).unwrap();
        let (lib, _) = resolve_types(&[&lib], &options).unwrap();
        struct Spans(Vec<(SourceSpan, SourceSpan)>);
        impl Visitor<()> for Spans {
            type Value = ();
            fn visit_output(&mut self, node: &Output) -> Result<(), ()> {
                let value = node.value.as_ref().unwrap().span.clone();
                self.0.push((node.src.span(), value));
                Ok(())
            }
        }
        let mut spans = Spans(vec![]);
        let _ = spans.walk(&lib);
        let [(src, value)] = spans.0.as_slice() else {
            panic!("expected one output, got {:?}", spans.0);
        };
        assert_eq!(src, value);
    }

    #[spec_test(REQ_CB_analyzer_037)]
    fn apply_when_function_and_method_outputs_assigned_then_values_read_call_outputs() {
        let program = "
FUNCTION_BLOCK K
METHOD M
VAR_OUTPUT mq : REAL; END_VAR
END_METHOD
END_FUNCTION_BLOCK
FUNCTION F : DINT
VAR_INPUT x : DINT; END_VAR
VAR_OUTPUT q : LINT; done : BOOL; END_VAR
F := x;
END_FUNCTION
PROGRAM main VAR k : K; r : DINT; l : LINT; d : BOOL; lr : LREAL; END_VAR
r := F(NOT done => d, x := 1, q => l);
k.M(mq => lr);
END_PROGRAM";
        assert_eq!(
            vec![
                "q => l: CallOutput(q) (LINT)",
                "NOT done => d: NOT CallOutput(done) (BOOL)",
                "mq => lr: CallOutput(mq) (REAL)"
            ],
            output_values(program)
        );
    }

    #[test]
    fn apply_when_function_arguments_do_not_bind_then_output_has_no_value() {
        let program = "
FUNCTION F : DINT
VAR_INPUT x : DINT; END_VAR
VAR_OUTPUT q : LINT; END_VAR
F := x;
END_FUNCTION
PROGRAM main VAR r : DINT; l : LINT; END_VAR
r := F(x := 1, nope := 2, q => l);
END_PROGRAM";
        assert_eq!(vec!["q => l: none"], output_values(program));
    }

    #[test]
    fn apply_when_arguments_do_not_bind_then_output_has_no_value() {
        let program = format!(
            "{BLOCK}
PROGRAM main VAR inst : FB; r : DINT; END_VAR
inst(nope := 1, q => r);
END_PROGRAM"
        );
        assert_eq!(vec!["q => r: none"], output_values(&program));
    }

    #[test]
    fn apply_when_instance_not_declared_then_kept_as_written() {
        let program = "
PROGRAM main VAR x : DINT; END_VAR
missing(b := 2, a := 1);
END_PROGRAM";
        assert_eq!(vec!["b := 2, a := 1"], calls(program));
    }

    fn function_options() -> CompilerOptions {
        CompilerOptions::default()
    }

    /// Every function call in `program`, after type resolution, one string
    /// per call: the function's name, then its arguments.
    fn function_calls(program: &str) -> Vec<String> {
        let options = function_options();
        let lib = parse_program(program, &FileId::default(), &options).unwrap();
        let (lib, _) = resolve_types(&[&lib], &options).unwrap();
        struct Calls(Vec<String>);
        impl Visitor<()> for Calls {
            type Value = ();
            fn visit_function(&mut self, node: &Function) -> Result<(), ()> {
                let args: Vec<String> = node
                    .param_assignment
                    .iter()
                    .map(ToString::to_string)
                    .collect();
                self.0.push(format!("{}: {}", node.name, args.join(", ")));
                node.recurse_visit(self)
            }
        }
        let mut calls = Calls(vec![]);
        let _ = calls.walk(&lib);
        calls.0
    }

    /// The problem codes type resolution reports for `program`.
    fn binding_problems(program: &str) -> Vec<String> {
        let options = function_options();
        let lib = parse_program(program, &FileId::default(), &options).unwrap();
        let diagnostics = match resolve_types(&[&lib], &options) {
            Ok((_, context)) => context.diagnostics().to_vec(),
            Err(diagnostics) => diagnostics,
        };
        diagnostics.into_iter().map(|d| d.code).collect()
    }

    /// A function whose in-out is declared between two inputs, and a program
    /// that calls it with `body`.
    fn function_program(body: &str) -> String {
        format!(
            "
FUNCTION f : DINT
VAR_INPUT a : DINT; END_VAR
VAR_IN_OUT io : DINT; END_VAR
VAR_INPUT b : DINT; END_VAR
f := a * 10 + b;
END_FUNCTION
PROGRAM main
VAR t : DINT; r : DINT; END_VAR
{body}
END_PROGRAM"
        )
    }

    #[spec_test(REQ_CB_analyzer_031)]
    fn apply_when_function_named_out_of_declaration_order_then_declaration_order() {
        assert_eq!(
            vec!["f: a := 1, io := t, b := 2"],
            function_calls(&function_program("r := f(b := 2, io := t, a := 1);"))
        );
    }

    #[spec_test(REQ_CB_analyzer_032)]
    fn apply_when_function_positional_then_named_after_inputs_and_in_outs() {
        assert_eq!(
            vec!["f: a := 1, io := t, b := 2"],
            function_calls(&function_program("r := f(1, t, 2);"))
        );
    }

    #[spec_test(REQ_CB_analyzer_032)]
    fn apply_when_standard_function_positional_then_named_inputs() {
        assert_eq!(
            vec!["ADD: IN1 := 1, IN2 := 2, IN3 := 3"],
            function_calls(&function_program("r := ADD(1, 2, 3);"))
        );
    }

    #[spec_test(REQ_CB_analyzer_033)]
    fn apply_when_extensible_function_named_then_further_inputs_in_number_order() {
        assert_eq!(
            vec!["ADD: IN1 := 1, IN2 := 2, IN3 := 3"],
            function_calls(&function_program("r := ADD(IN3 := 3, IN1 := 1, IN2 := 2);"))
        );
    }

    #[spec_test(REQ_CB_analyzer_033)]
    fn apply_when_mux_named_then_further_inputs_after_declared() {
        assert_eq!(
            vec!["MUX: K := 0, IN0 := 1, IN1 := 2, IN2 := 3"],
            function_calls(&function_program(
                "r := MUX(IN2 := 3, K := 0, IN0 := 1, IN1 := 2);"
            ))
        );
    }

    #[spec_test(REQ_CB_analyzer_033)]
    fn apply_when_extensible_input_numbered_past_a_gap_then_undeclared() {
        assert_eq!(
            vec!["P4023"],
            binding_problems(&function_program("r := ADD(IN1 := 1, IN3 := 3);"))
        );
    }

    #[spec_test(REQ_CB_analyzer_034)]
    fn apply_when_function_input_named_wrong_then_undeclared() {
        assert_eq!(
            vec!["P4023"],
            binding_problems(&function_program("r := f(a := 1, io := t, nope := 2);"))
        );
    }

    #[spec_test(REQ_CB_analyzer_022)]
    fn apply_when_function_output_names_no_output_then_undefined_output() {
        assert_eq!(
            vec!["P4004"],
            binding_problems(&function_program(
                "r := f(a := 1, io := t, b := 2, nope => r);"
            ))
        );
    }

    #[spec_test(REQ_CB_analyzer_024)]
    fn apply_when_function_input_named_twice_then_duplicate() {
        assert_eq!(
            vec!["P4024"],
            binding_problems(&function_program(
                "r := f(a := 1, io := t, b := 2, A := 3);"
            ))
        );
    }

    #[test]
    fn apply_when_function_mixed_then_kept_as_written() {
        assert_eq!(
            vec!["f: 1, io := t, b := 2"],
            function_calls(&function_program("r := f(1, io := t, b := 2);"))
        );
    }

    #[test]
    fn apply_when_function_positional_count_differs_then_kept_as_written() {
        assert_eq!(
            vec!["f: 1, t"],
            function_calls(&function_program("r := f(1, t);"))
        );
    }

    #[test]
    fn apply_when_function_call_is_argument_then_both_normalized() {
        assert_eq!(
            vec![
                "f: a := f(a := 1, io := t, b := 2), io := t, b := 3",
                "f: a := 1, io := t, b := 2"
            ],
            function_calls(&function_program(
                "r := f(b := 3, io := t, a := f(1, t, 2));"
            ))
        );
    }

    #[test]
    fn apply_when_function_not_declared_then_kept_as_written() {
        assert_eq!(
            vec!["missing: b := 2, a := 1"],
            function_calls(&function_program("r := missing(b := 2, a := 1);"))
        );
    }
}
