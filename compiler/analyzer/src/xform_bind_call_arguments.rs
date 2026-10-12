//! Writes the arguments of every function block call in normal form.
//!
//! Which parameter each argument of a call binds to, and the order the
//! arguments are evaluated in, are decisions of the language, so the analyzer
//! makes them and records them in the call itself (ADR-0058). This pass
//! rewrites each function block call whose arguments bind into the normal form
//! `call_binding::normal_form` defines: one named input for each input the
//! call supplies, in the order the block declares them, then its output
//! assignments in declaration order. Every later stage, the rules, the
//! recording pass and code generation, reads that form and binds nothing
//! itself. See `specs/design/call-binding.md`.
//!
//! A call whose arguments do not bind keeps them as written, and
//! `rule_function_block_invocation` reports why. The pass reports nothing
//! itself, so it cannot fail; it runs before the rules, where the named
//! argument rewrite for function calls runs.

use std::convert::Infallible;

use ironplc_dsl::common::Library;
use ironplc_dsl::fold::Fold;
use ironplc_dsl::scope::ScopeNode;
use ironplc_dsl::textual::FbCall;

use crate::call_binding::normal_form;
use crate::callee_resolution::{instance_type, BlockParameters};
use crate::symbol_environment::{ScopeTracker, SymbolEnvironment};
use crate::type_environment::TypeEnvironment;

/// Writes every function block call of `lib` whose arguments bind in normal
/// form.
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
    /// The parameters of every block a call can name.
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
}

#[cfg(test)]
mod tests {
    use ironplc_dsl::core::FileId;
    use ironplc_dsl::textual::StmtKind;
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
