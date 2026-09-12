//! Transformation pass that converts named (formal) function call arguments
//! to positional arguments.
//!
//! This pass runs after `xform_resolve_symbol_and_function_environment` (which
//! builds the `FunctionEnvironment`) and before `xform_resolve_expr_types`.
//! It looks up each function's signature to determine the declared parameter
//! order, then rewrites `NamedInput` arguments into `PositionalInput` arguments
//! in the correct positions. This allows codegen to treat all function call
//! arguments uniformly as positional.
//!
//! An extensible function (MUX, ADD, AND, ...) accepts inputs beyond its
//! declared parameters, named by counting on from the last declared one
//! (`IN3` after `IN2`); those bind in number order after the declared ones.

use std::collections::HashMap;

use ironplc_dsl::common::Library;
use ironplc_dsl::core::Id;
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_dsl::fold::Fold;
use ironplc_dsl::textual::*;
use ironplc_problems::Problem;

use crate::function_environment::{FunctionEnvironment, FunctionSignature};

/// Rewrites every named call argument this pass can place into a positional
/// one, and diagnoses the rest.
///
/// Best effort: the rewritten library rides back alongside the diagnostics
/// rather than being discarded. A diagnosed call keeps its `NamedInput`
/// entries, which is exactly the state reverting would leave *every* call in
/// -- including the valid ones in unrelated POUs -- so reverting cannot be the
/// safer option. Downstream passes already have `NamedInput` arms
/// (`xform_resolve_expr_types`, `xform_mark_unwritten_constants`,
/// `call_assignment_check`, `codegen::compile_stmt`), and codegen is never
/// reached because `ironplc_project::compile` gates it on an empty diagnostic
/// list.
///
/// `Err` is reserved for a fold that could not produce a library at all.
pub fn apply(
    lib: Library,
    function_environment: &FunctionEnvironment,
) -> Result<(Library, Vec<Diagnostic>), Vec<Diagnostic>> {
    let mut resolver = NamedToPositionalResolver {
        function_environment,
        errors: vec![],
    };
    match resolver.fold_library(lib) {
        Ok(result) => Ok((result, resolver.errors)),
        Err(e) => {
            let mut errors = resolver.errors;
            errors.push(e);
            Err(errors)
        }
    }
}

struct NamedToPositionalResolver<'a> {
    function_environment: &'a FunctionEnvironment,
    errors: Vec<Diagnostic>,
}

impl NamedToPositionalResolver<'_> {
    /// Works out which declared parameter each named argument of a call binds
    /// to, returning the parameter names in the order the rewritten call must
    /// list them.
    ///
    /// Returns `None` when the call cannot be rewritten: a duplicated name
    /// makes the intended order ambiguous, and an undeclared name has no
    /// position to take. Either way the problem is recorded and the caller
    /// leaves *that call's* arguments named. The granularity matters — the
    /// alternative, dropping the offending argument or discarding the whole
    /// library's rewrite, hands later passes either a call missing an argument
    /// it was written with or a library of valid calls that were never
    /// rewritten.
    fn plan_positional_order(
        &mut self,
        args: &[ParamAssignmentKind],
        signature: &FunctionSignature,
    ) -> Option<Vec<Id>> {
        let mut placeable = true;

        // Argument order, so a name is diagnosed where it was written.
        let mut names: Vec<&Id> = vec![];
        for arg in args {
            let ParamAssignmentKind::NamedInput(ni) = arg else {
                continue;
            };
            if names.contains(&&ni.name) {
                self.errors.push(Diagnostic::problem(
                    Problem::FunctionCallDuplicateNamedArg,
                    Label::span(ni.name.span.clone(), "Duplicate argument"),
                ));
                placeable = false;
            } else {
                names.push(&ni.name);
            }
        }

        // Declared input parameters in declaration order.
        let mut order: Vec<Id> = vec![];
        for param in &signature.parameters {
            if !param.is_input_compatible() {
                continue;
            }
            if let Some(at) = names.iter().position(|name| **name == param.name) {
                order.push(names.remove(at).clone());
            }
        }

        // An extensible function's further inputs continue the numbering of
        // its last declared one. Only as many are tried as there are names
        // left to place: the parameter list may be unbounded, and an input
        // numbered past a gap is undeclared like any other.
        if signature.is_extensible {
            let further = signature
                .input_parameters()
                .skip(signature.input_parameter_count())
                .take(names.len());
            for param in further {
                if let Some(at) = names.iter().position(|name| **name == param.name) {
                    order.push(names.remove(at).clone());
                }
            }
        }

        // Anything still unplaced names no parameter of this function.
        for name in names {
            self.errors.push(Diagnostic::problem(
                Problem::FunctionCallNamedArgUndeclared,
                Label::span(name.span.clone(), "Undeclared parameter"),
            ));
            placeable = false;
        }

        placeable.then_some(order)
    }
}

impl Fold<Diagnostic> for NamedToPositionalResolver<'_> {
    fn fold_function(&mut self, node: Function) -> Result<Function, Diagnostic> {
        // 1. Look up the function signature; if not found, pass through
        let Some(signature) = self.function_environment.get(&node.name) else {
            return Function::recurse_fold(node, self);
        };

        // 2. If there are ANY positional arguments, pass through
        let has_positional = node
            .param_assignment
            .iter()
            .any(|p| matches!(p, ParamAssignmentKind::PositionalInput(_)));
        if has_positional {
            return Function::recurse_fold(node, self);
        }

        // 3. Decide where each named argument goes before taking the call
        //    apart, so a call that cannot be rewritten keeps every argument it
        //    was written with.
        let Some(order) = self.plan_positional_order(&node.param_assignment, signature) else {
            return Function::recurse_fold(node, self);
        };

        // 4. Take the call apart. Output assignments keep their place at the
        //    end and bind no input.
        let mut named: HashMap<Id, NamedInput> = HashMap::new();
        let mut outputs: Vec<ParamAssignmentKind> = vec![];
        for param in node.param_assignment {
            match param {
                ParamAssignmentKind::NamedInput(ni) => {
                    named.insert(ni.name.clone(), ni);
                }
                ParamAssignmentKind::Output(_) => outputs.push(param),
                ParamAssignmentKind::PositionalInput(_) => {
                    unreachable!("positional inputs already handled above")
                }
            }
        }

        // 5. Emit the inputs positionally, in the planned order.
        let mut param_assignment: Vec<ParamAssignmentKind> =
            Vec::with_capacity(order.len() + outputs.len());
        for name in order {
            let ni = named
                .remove(&name)
                .expect("planned name came from these arguments");
            param_assignment.push(ParamAssignmentKind::PositionalInput(PositionalInput {
                expr: self.fold_expr(ni.expr)?,
            }));
        }
        debug_assert!(
            named.is_empty(),
            "every named argument is either placed or diagnosed"
        );
        param_assignment.extend(outputs);

        Ok(Function {
            name: node.name,
            param_assignment,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::function_environment::{
        FunctionEnvironment, FunctionEnvironmentBuilder, FunctionSignature,
    };
    use crate::intermediate_type::IntermediateFunctionParameter;
    use crate::test_helpers::{parse_and_resolve_types, parse_only};
    use ironplc_dsl::common::{FunctionReturnType, TypeName};
    use ironplc_dsl::core::Id;

    /// Applies the pass and asserts it rewrote everything cleanly, returning
    /// the rewritten library.
    fn rewritten(lib: Library, env: &FunctionEnvironment) -> Library {
        let (lib, diagnostics) = apply(lib, env).expect("fold produced a library");
        assert!(
            diagnostics.is_empty(),
            "expected no diagnostics, got {diagnostics:?}"
        );
        lib
    }

    /// Applies the pass and returns the diagnostics it reported. The rewritten
    /// library still comes back -- the pass is best effort -- so this asserts
    /// the `Ok` arm rather than an `Err`.
    fn diagnostics_of(lib: Library, env: &FunctionEnvironment) -> Vec<Diagnostic> {
        apply(lib, env).expect("fold produced a library").1
    }

    /// Signature of a user-defined `INT` function taking the given inputs.
    fn signature_of(name: &str, params: Vec<(&str, &str)>) -> FunctionSignature {
        let parameters = params
            .into_iter()
            .map(|(pname, ptype)| IntermediateFunctionParameter {
                name: Id::from(pname),
                param_type: TypeName::from(ptype),
                is_input: true,
                is_output: false,
                is_inout: false,
                is_reference: false,
            })
            .collect();

        FunctionSignature::new(
            Id::from(name),
            Some(FunctionReturnType::Named(TypeName::from("INT"))),
            parameters,
            ironplc_dsl::core::SourceSpan::default(),
        )
    }

    /// Helper to build a FunctionEnvironment with a single user-defined function.
    fn env_with_function(name: &str, params: Vec<(&str, &str)>) -> FunctionEnvironment {
        env_with_functions(vec![(name, params)])
    }

    /// Helper to build a FunctionEnvironment with several user-defined functions.
    fn env_with_functions(functions: Vec<(&str, Vec<(&str, &str)>)>) -> FunctionEnvironment {
        let mut env = FunctionEnvironment::new();
        for (name, params) in functions {
            env.insert(signature_of(name, params)).unwrap();
        }
        env
    }

    #[test]
    fn apply_when_positional_args_then_unchanged() {
        let program = "
FUNCTION MY_FUNC : INT
VAR_INPUT
  A : INT;
  B : INT;
END_VAR
  MY_FUNC := A + B;
END_FUNCTION

PROGRAM main
VAR
  x : INT;
END_VAR
  x := MY_FUNC(1, 2);
END_PROGRAM
";
        let library = parse_and_resolve_types(program);
        let env = env_with_function("MY_FUNC", vec![("A", "INT"), ("B", "INT")]);
        let diagnostics = diagnostics_of(library, &env);
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn apply_when_no_args_then_unchanged() {
        let program = "
FUNCTION MY_FUNC : INT
  MY_FUNC := 42;
END_FUNCTION

PROGRAM main
VAR
  x : INT;
END_VAR
  x := MY_FUNC();
END_PROGRAM
";
        let library = parse_and_resolve_types(program);
        let env = env_with_function("MY_FUNC", vec![]);
        let diagnostics = diagnostics_of(library, &env);
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn apply_when_named_args_then_reordered_to_positional() {
        let program = "
FUNCTION MY_FUNC : INT
VAR_INPUT
  A : INT;
  B : INT;
END_VAR
  MY_FUNC := A + B;
END_FUNCTION

PROGRAM main
VAR
  x : INT;
END_VAR
  x := MY_FUNC(A := 1, B := 2);
END_PROGRAM
";
        let library = parse_and_resolve_types(program);
        let env = env_with_function("MY_FUNC", vec![("A", "INT"), ("B", "INT")]);
        let result = rewritten(library, &env);

        // Verify the function call now has positional args
        let func_call = find_function_call(&result, "MY_FUNC");
        assert!(func_call.is_some(), "Should find MY_FUNC call");
        let func = func_call.unwrap();
        assert_eq!(func.param_assignment.len(), 2);
        assert!(
            matches!(
                &func.param_assignment[0],
                ParamAssignmentKind::PositionalInput(_)
            ),
            "First arg should be positional"
        );
        assert!(
            matches!(
                &func.param_assignment[1],
                ParamAssignmentKind::PositionalInput(_)
            ),
            "Second arg should be positional"
        );
    }

    #[test]
    fn apply_when_named_args_reversed_order_then_reordered() {
        let program = "
FUNCTION MY_FUNC : INT
VAR_INPUT
  A : INT;
  B : INT;
END_VAR
  MY_FUNC := A + B;
END_FUNCTION

PROGRAM main
VAR
  x : INT;
END_VAR
  x := MY_FUNC(B := 2, A := 1);
END_PROGRAM
";
        let library = parse_and_resolve_types(program);
        let env = env_with_function("MY_FUNC", vec![("A", "INT"), ("B", "INT")]);
        let result = rewritten(library, &env);

        // Verify the function call has positional args in declaration order (A, B)
        let func_call = find_function_call(&result, "MY_FUNC");
        assert!(func_call.is_some(), "Should find MY_FUNC call");
        let func = func_call.unwrap();
        assert_eq!(func.param_assignment.len(), 2);
        // Both should be positional now
        assert!(matches!(
            &func.param_assignment[0],
            ParamAssignmentKind::PositionalInput(_)
        ));
        assert!(matches!(
            &func.param_assignment[1],
            ParamAssignmentKind::PositionalInput(_)
        ));
    }

    /// The integer literal each positional argument of `func` is, in order.
    fn positional_literals(func: &Function) -> Vec<u128> {
        func.param_assignment
            .iter()
            .map(|p| match p {
                ParamAssignmentKind::PositionalInput(pos) => match &pos.expr.kind {
                    ExprKind::Const(ironplc_dsl::common::ConstantKind::IntegerLiteral(lit)) => {
                        lit.value.value.value
                    }
                    _ => u128::MAX,
                },
                _ => u128::MAX,
            })
            .collect()
    }

    /// An extensible function's inputs beyond the declared ones bind by
    /// number, so `IN3` takes the third position however it is written.
    #[test]
    fn apply_when_named_args_on_extensible_function_then_positional_in_number_order() {
        let program = "
PROGRAM main
VAR
  x : INT;
END_VAR
  x := ADD(IN3 := 3, IN1 := 1, IN2 := 2);
END_PROGRAM
";
        let library = parse_and_resolve_types(program);
        let env = FunctionEnvironmentBuilder::new()
            .with_stdlib_functions()
            .build();
        let result = rewritten(library, &env);

        let func = find_function_call(&result, "ADD").unwrap();
        assert_eq!(positional_literals(&func), vec![1, 2, 3]);
    }

    #[test]
    fn apply_when_named_args_on_mux_then_positional_after_declared() {
        let program = "
PROGRAM main
VAR
  x : INT;
END_VAR
  x := MUX(IN2 := 3, K := 0, IN0 := 1, IN1 := 2);
END_PROGRAM
";
        let library = parse_and_resolve_types(program);
        let env = FunctionEnvironmentBuilder::new()
            .with_stdlib_functions()
            .build();
        let result = rewritten(library, &env);

        let func = find_function_call(&result, "MUX").unwrap();
        assert_eq!(positional_literals(&func), vec![0, 1, 2, 3]);
    }

    /// The numbering is consecutive: `IN5` with no `IN3` and `IN4` is not a
    /// parameter of the call.
    #[test]
    fn apply_when_named_arg_on_extensible_function_skips_a_number_then_error() {
        let program = "
PROGRAM main
VAR
  x : INT;
END_VAR
  x := ADD(IN1 := 1, IN2 := 2, IN5 := 3);
END_PROGRAM
";
        let library = parse_and_resolve_types(program);
        let env = FunctionEnvironmentBuilder::new()
            .with_stdlib_functions()
            .build();
        let errs = diagnostics_of(library, &env);
        assert!(errs
            .iter()
            .any(|d| d.code == Problem::FunctionCallNamedArgUndeclared.code()));
    }

    #[test]
    fn apply_when_function_not_found_then_unchanged() {
        let program = "
FUNCTION MY_FUNC : INT
VAR_INPUT
  A : INT;
END_VAR
  MY_FUNC := A;
END_FUNCTION

PROGRAM main
VAR
  x : INT;
END_VAR
  x := MY_FUNC(A := 1);
END_PROGRAM
";
        let library = parse_and_resolve_types(program);
        // Empty environment — function not found
        let env = FunctionEnvironment::new();
        let diagnostics = diagnostics_of(library, &env);
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn apply_when_mixed_positional_and_named_then_unchanged() {
        let program = "
FUNCTION MY_FUNC : INT
VAR_INPUT
  A : INT;
  B : INT;
END_VAR
  MY_FUNC := A + B;
END_FUNCTION

PROGRAM main
VAR
  x : INT;
END_VAR
  x := MY_FUNC(1, B := 2);
END_PROGRAM
";
        let library = parse_and_resolve_types(program);
        let env = env_with_function("MY_FUNC", vec![("A", "INT"), ("B", "INT")]);
        // Mixed args are passed through unchanged — later validation catches it
        let diagnostics = diagnostics_of(library, &env);
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn apply_when_named_arg_wrong_name_then_error() {
        let program = "
FUNCTION MY_FUNC : INT
VAR_INPUT
  A : INT;
END_VAR
  MY_FUNC := A;
END_FUNCTION

PROGRAM main
VAR
  x : INT;
END_VAR
  x := MY_FUNC(WRONG := 1);
END_PROGRAM
";
        let library = parse_and_resolve_types(program);
        let env = env_with_function("MY_FUNC", vec![("A", "INT")]);
        let errs = diagnostics_of(library, &env);
        assert!(errs
            .iter()
            .any(|d| d.code == Problem::FunctionCallNamedArgUndeclared.code()));
    }

    #[test]
    fn apply_when_duplicate_named_arg_then_error() {
        let program = "
FUNCTION MY_FUNC : INT
VAR_INPUT
  A : INT;
END_VAR
  MY_FUNC := A;
END_FUNCTION

PROGRAM main
VAR
  x : INT;
END_VAR
  x := MY_FUNC(A := 1, A := 2);
END_PROGRAM
";
        let library = parse_and_resolve_types(program);
        let env = env_with_function("MY_FUNC", vec![("A", "INT")]);
        let errs = diagnostics_of(library, &env);
        assert!(errs
            .iter()
            .any(|d| d.code == Problem::FunctionCallDuplicateNamedArg.code()));
    }

    /// One call that cannot be rewritten must not leave valid calls elsewhere
    /// in the library named. Reverting the whole library on a per-call problem
    /// is what made a source that analyzed cleanly alone fail once merged with
    /// unrelated code.
    ///
    /// Parses without running the pipeline, so this pass is the only thing
    /// that has touched the calls being asserted on.
    #[test]
    fn apply_when_one_call_undeclared_then_other_calls_still_positional() {
        let program = "
FUNCTION GOOD_FUNC : INT
VAR_INPUT
  A : INT;
END_VAR
  GOOD_FUNC := A;
END_FUNCTION

FUNCTION BAD_FUNC : INT
VAR_INPUT
  B : INT;
END_VAR
  BAD_FUNC := B;
END_FUNCTION

PROGRAM main
VAR
  x : INT;
  y : INT;
END_VAR
  x := GOOD_FUNC(A := 1);
  y := BAD_FUNC(WRONG := 2);
END_PROGRAM
";
        let library = parse_only(program);
        let env = env_with_functions(vec![
            ("GOOD_FUNC", vec![("A", "INT")]),
            ("BAD_FUNC", vec![("B", "INT")]),
        ]);

        let (library, diagnostics) = apply(library, &env).expect("fold produced a library");

        assert_eq!(
            diagnostics
                .iter()
                .map(|d| d.code.as_str())
                .collect::<Vec<_>>(),
            vec![Problem::FunctionCallNamedArgUndeclared.code()]
        );
        let good = find_function_call(&library, "GOOD_FUNC").expect("GOOD_FUNC call");
        assert_eq!(positional_literals(&good), vec![1]);
    }

    /// The diagnosed argument stays on the call. Dropping it would hand later
    /// passes a call missing an argument the author wrote, so an arity or type
    /// problem would be reported against a call that does not exist in the
    /// source.
    #[test]
    fn apply_when_named_arg_undeclared_then_call_keeps_its_arguments() {
        let program = "
FUNCTION MY_FUNC : INT
VAR_INPUT
  A : INT;
END_VAR
  MY_FUNC := A;
END_FUNCTION

PROGRAM main
VAR
  x : INT;
END_VAR
  x := MY_FUNC(A := 1, WRONG := 2);
END_PROGRAM
";
        let library = parse_only(program);
        let env = env_with_function("MY_FUNC", vec![("A", "INT")]);

        let (library, _diagnostics) = apply(library, &env).expect("fold produced a library");

        let call = find_function_call(&library, "MY_FUNC").expect("MY_FUNC call");
        let names: Vec<String> = call
            .param_assignment
            .iter()
            .map(|p| match p {
                ParamAssignmentKind::NamedInput(ni) => ni.name.to_string(),
                other => panic!("expected the call to stay named, got {other:?}"),
            })
            .collect();
        assert_eq!(names, vec!["A".to_string(), "WRONG".to_string()]);
    }

    /// Helper to build a FunctionEnvironment with input and inout parameters.
    fn env_with_inout_function(name: &str, params: Vec<(&str, &str, bool)>) -> FunctionEnvironment {
        let parameters = params
            .into_iter()
            .map(|(pname, ptype, is_inout)| IntermediateFunctionParameter {
                name: Id::from(pname),
                param_type: TypeName::from(ptype),
                is_input: !is_inout,
                is_output: false,
                is_inout,
                is_reference: false,
            })
            .collect();

        let sig = FunctionSignature::new(
            Id::from(name),
            Some(FunctionReturnType::Named(TypeName::from("BOOL"))),
            parameters,
            ironplc_dsl::core::SourceSpan::default(),
        );

        let mut env = FunctionEnvironment::new();
        env.insert(sig).unwrap();
        env
    }

    #[test]
    fn apply_when_named_inout_arg_then_converted_to_positional() {
        let program = "
FUNCTION MY_FUNC : BOOL
VAR_IN_OUT
  data : DINT;
END_VAR
  MY_FUNC := TRUE;
END_FUNCTION

PROGRAM main
VAR
  x : DINT;
END_VAR
  x := MY_FUNC(data := x);
END_PROGRAM
";
        let library = parse_and_resolve_types(program);
        let env = env_with_inout_function("MY_FUNC", vec![("data", "DINT", true)]);
        let result = rewritten(library, &env);

        let func_call = find_function_call(&result, "MY_FUNC");
        assert!(func_call.is_some(), "Should find MY_FUNC call");
        let func = func_call.unwrap();
        assert_eq!(func.param_assignment.len(), 1);
        assert!(
            matches!(
                &func.param_assignment[0],
                ParamAssignmentKind::PositionalInput(_)
            ),
            "InOut arg should be converted to positional"
        );
    }

    #[test]
    fn apply_when_mixed_input_and_inout_named_args_then_positional_in_declaration_order() {
        let program = "
FUNCTION MY_FUNC : BOOL
VAR_INPUT
  count : INT;
END_VAR
VAR_IN_OUT
  data : DINT;
END_VAR
  MY_FUNC := TRUE;
END_FUNCTION

PROGRAM main
VAR
  x : DINT;
END_VAR
  x := MY_FUNC(data := x, count := 5);
END_PROGRAM
";
        let library = parse_and_resolve_types(program);
        let env = env_with_inout_function(
            "MY_FUNC",
            vec![("count", "INT", false), ("data", "DINT", true)],
        );
        let result = rewritten(library, &env);

        let func_call = find_function_call(&result, "MY_FUNC");
        assert!(func_call.is_some(), "Should find MY_FUNC call");
        let func = func_call.unwrap();
        assert_eq!(func.param_assignment.len(), 2);
        assert!(matches!(
            &func.param_assignment[0],
            ParamAssignmentKind::PositionalInput(_)
        ));
        assert!(matches!(
            &func.param_assignment[1],
            ParamAssignmentKind::PositionalInput(_)
        ));
    }

    /// Helper to find a Function call node by name in the library.
    fn find_function_call(library: &Library, name: &str) -> Option<Function> {
        use ironplc_dsl::visitor::Visitor;

        struct FunctionFinder {
            target: Id,
            found: Option<Function>,
        }

        impl Visitor<()> for FunctionFinder {
            type Value = ();
            fn visit_function(&mut self, node: &Function) -> Result<Self::Value, ()> {
                if node.name == self.target {
                    self.found = Some(node.clone());
                }
                Ok(())
            }
        }

        let mut finder = FunctionFinder {
            target: Id::from(name),
            found: None,
        };
        let _ = finder.walk(library);
        finder.found
    }
}
