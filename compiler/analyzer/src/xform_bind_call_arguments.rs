//! Writes the arguments of every function block and method call in normal
//! form.
//!
//! Which parameter each argument of a call binds to, and the order the
//! arguments are evaluated in, are decisions of the language, so the analyzer
//! makes them and records them in the call itself (ADR-0058). This pass
//! rewrites each function block call whose arguments bind into the normal form
//! `call_binding::normal_form` defines: one named input for each input the
//! call supplies, in the order the block declares them, then its output
//! assignments in declaration order. Every later stage, the rules, the
//! recording pass and code generation, reads that form and binds nothing
//! itself. A method call is written in normal form against the method its
//! receiver's block declares or inherits. See `specs/design/call-binding.md`.
//!
//! A call whose arguments do not bind keeps them as written, and
//! `rule_function_block_invocation` or `rule_method_call_declared` reports
//! why. The pass reports nothing
//! itself, so it cannot fail; it runs before the rules, where the named
//! argument rewrite for function calls runs.

use std::convert::Infallible;

use ironplc_dsl::common::Library;
use ironplc_dsl::fold::Fold;
use ironplc_dsl::scope::ScopeNode;
use ironplc_dsl::textual::{FbCall, MethodCall};

use crate::call_binding::normal_form;
use crate::callee_resolution::{instance_type, receiver_block, BlockParameters};
use crate::symbol_environment::{ScopeTracker, SymbolEnvironment};
use crate::type_environment::TypeEnvironment;

/// Writes every function block and method call of `lib` whose arguments bind
/// in normal form.
pub fn apply(lib: Library, symbols: &SymbolEnvironment, types: &TypeEnvironment) -> Library {
    let mut binder = CallArgumentBinder {
        parameters: BlockParameters::of_library(&lib),
        symbols,
        types,
        scope: ScopeTracker::default(),
    };
    let Ok(lib) = binder.fold_library(lib);
    lib
}

struct CallArgumentBinder<'a> {
    /// The parameters of every block and method a call can name.
    parameters: BlockParameters,
    symbols: &'a SymbolEnvironment,
    types: &'a TypeEnvironment,
    /// Where the fold is, to find the instance a call names.
    scope: ScopeTracker,
}

impl Fold<Infallible> for CallArgumentBinder<'_> {
    fn enter_scope(&mut self, node: ScopeNode<'_>) -> Result<(), Infallible> {
        self.scope.enter(&node);
        Ok(())
    }

    fn exit_scope(&mut self) {
        self.scope.exit();
    }

    fn fold_fb_call(&mut self, node: FbCall) -> Result<FbCall, Infallible> {
        let mut node = FbCall::recurse_fold(node, self)?;
        let parameters = instance_type(
            &node.var_name,
            &self.scope.current(),
            self.symbols,
            self.types,
        )
        .and_then(|block| self.parameters.of(&block, self.types));
        if let Some(parameters) = parameters {
            node.params = match normal_form(&parameters, node.params) {
                Ok(normal) | Err(normal) => normal,
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
                Ok(normal) | Err(normal) => normal,
            };
        }
        Ok(node)
    }
}

#[cfg(test)]
mod tests {
    use ironplc_dsl::core::FileId;
    use ironplc_dsl::textual::{MethodCall, StmtKind};
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

    #[test]
    fn apply_when_instance_not_declared_then_kept_as_written() {
        let program = "
PROGRAM main VAR x : DINT; END_VAR
missing(b := 2, a := 1);
END_PROGRAM";
        assert_eq!(vec!["b := 2, a := 1"], calls(program));
    }
}
