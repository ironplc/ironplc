//! Semantic rule that an `EXIT` statement is inside a loop.
//!
//! `EXIT` terminates the innermost enclosing `FOR`, `WHILE`, or `REPEAT`
//! loop, so outside of one it has nothing to terminate.
//!
//! Code generation checks the same thing, because it needs the loop's exit
//! label to emit the jump. That check is not reached by `check`, which stops
//! after semantic analysis, so the editor never showed this error.
//!
//! ## Passes
//!
//! ```ignore
//! PROGRAM main
//!   VAR x : INT; END_VAR
//!   FOR x := 1 TO 10 DO
//!     IF x = 5 THEN
//!       EXIT;
//!     END_IF;
//!   END_FOR;
//! END_PROGRAM
//! ```
//!
//! ## Fails
//!
//! ```ignore
//! PROGRAM main
//!   EXIT;
//! END_PROGRAM
//! ```
use ironplc_dsl::{
    common::*,
    diagnostic::{Diagnostic, Label},
    textual::*,
    visitor::Visitor,
};
use ironplc_problems::Problem;
use std::convert::Infallible;

use crate::{
    result::SemanticResult,
    rule_support::{run_rule, DiagnosticVisitor},
    semantic_context::SemanticContext,
};
use ironplc_parser::options::CompilerOptions;

pub fn apply(
    lib: &Library,
    _context: &SemanticContext,
    _options: &CompilerOptions,
) -> SemanticResult {
    run_rule(RuleExitInsideLoop::default(), lib)
}

#[derive(Default)]
struct RuleExitInsideLoop {
    /// Number of loops enclosing the statement being visited.
    loop_depth: usize,
    diagnostics: Vec<Diagnostic>,
}

impl RuleExitInsideLoop {
    fn visit_loop_body<T>(
        &mut self,
        node: &T,
        recurse: fn(&T, &mut Self) -> Result<(), Infallible>,
    ) -> Result<(), Infallible> {
        self.loop_depth += 1;
        let result = recurse(node, self);
        self.loop_depth -= 1;
        result
    }
}

impl Visitor<Infallible> for RuleExitInsideLoop {
    type Value = ();

    fn visit_for(&mut self, node: &For) -> Result<(), Infallible> {
        self.visit_loop_body(node, |n, v| n.recurse_visit(v))
    }

    fn visit_while(&mut self, node: &While) -> Result<(), Infallible> {
        self.visit_loop_body(node, |n, v| n.recurse_visit(v))
    }

    fn visit_repeat(&mut self, node: &Repeat) -> Result<(), Infallible> {
        self.visit_loop_body(node, |n, v| n.recurse_visit(v))
    }

    fn visit_stmt_kind(&mut self, node: &StmtKind) -> Result<(), Infallible> {
        if let StmtKind::Exit(span) = node {
            if self.loop_depth == 0 {
                self.diagnostics.push(Diagnostic::problem(
                    Problem::ExitOutsideLoop,
                    Label::span(
                        span.clone(),
                        "EXIT must be inside a FOR, WHILE, or REPEAT loop",
                    ),
                ));
            }
        }
        node.recurse_visit(self)
    }
}

impl DiagnosticVisitor for RuleExitInsideLoop {
    fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

#[cfg(test)]
mod tests {
    use ironplc_problems::Problem;

    rule_err_code!(
        apply_when_exit_in_program_body_then_p4021,
        "
        PROGRAM main
            EXIT;
        END_PROGRAM",
        Problem::ExitOutsideLoop
    );

    rule_err_code!(
        apply_when_exit_after_loop_then_p4021,
        "
        PROGRAM main
        VAR x : INT; END_VAR
            FOR x := 1 TO 3 DO
                x := x;
            END_FOR;
            EXIT;
        END_PROGRAM",
        Problem::ExitOutsideLoop
    );

    rule_err_code!(
        apply_when_exit_in_if_outside_loop_then_p4021,
        "
        FUNCTION_BLOCK fb
        VAR x : BOOL; END_VAR
            IF x THEN
                EXIT;
            END_IF;
        END_FUNCTION_BLOCK",
        Problem::ExitOutsideLoop
    );

    rule_ok!(
        apply_when_exit_in_for_then_ok,
        "
        PROGRAM main
        VAR x : INT; END_VAR
            FOR x := 1 TO 10 DO
                IF x = 5 THEN
                    EXIT;
                END_IF;
            END_FOR;
        END_PROGRAM"
    );

    rule_ok!(
        apply_when_exit_in_while_then_ok,
        "
        PROGRAM main
        VAR x : BOOL; END_VAR
            WHILE x DO
                EXIT;
            END_WHILE;
        END_PROGRAM"
    );

    rule_ok!(
        apply_when_exit_in_repeat_then_ok,
        "
        FUNCTION f : INT
        VAR x : BOOL; END_VAR
            REPEAT
                EXIT;
            UNTIL x
            END_REPEAT;
            f := 0;
        END_FUNCTION"
    );

    rule_ok!(
        apply_when_exit_in_nested_loop_then_ok,
        "
        PROGRAM main
        VAR x : INT; y : BOOL; END_VAR
            FOR x := 1 TO 10 DO
                WHILE y DO
                    EXIT;
                END_WHILE;
                EXIT;
            END_FOR;
        END_PROGRAM"
    );
}
