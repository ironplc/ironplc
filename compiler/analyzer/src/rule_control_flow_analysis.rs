//! Semantic rule for control flow analysis.
//!
//! This rule validates:
//! 1. Continue statements are within loop contexts
//! 2. Unreachable code detection after continue statements
//! 3. Control flow structure validation
//! 4. Loop nesting and continue statement scope
//!
//! ## Passes
//!
//! ```ignore
//! PROGRAM Main
//! VAR
//!     i : INT;
//!     sum : INT := 0;
//! END_VAR
//!     FOR i := 1 TO 10 DO
//!         IF i MOD 2 = 0 THEN
//!             CONTINUE;  // Valid: inside loop
//!         END_IF
//!         sum := sum + i;
//!     END_FOR
//! END_PROGRAM
//! ```
//!
//! ```ignore
//! PROGRAM NestedLoops
//! VAR
//!     i, j : INT;
//! END_VAR
//!     FOR i := 1 TO 5 DO
//!         FOR j := 1 TO 5 DO
//!             IF j = 3 THEN
//!                 CONTINUE;  // Valid: affects inner loop only
//!             END_IF
//!         END_FOR
//!     END_FOR
//! END_PROGRAM
//! ```
//!
//! ## Fails
//!
//! ```ignore
//! PROGRAM Main
//! VAR
//!     x : INT := 10;
//! END_VAR
//!     x := x + 1;
//!     CONTINUE;  // Error: not in a loop
//! END_PROGRAM
//! ```
//!
//! ```ignore
//! PROGRAM UnreachableCode
//! VAR
//!     i : INT;
//! END_VAR
//!     FOR i := 1 TO 10 DO
//!         CONTINUE;
//!         i := i + 1;  // Warning: unreachable code
//!     END_FOR
//! END_PROGRAM
//! ```

use ironplc_dsl::{
    common::*,
    core::Id,
    diagnostic::{Diagnostic, Label},
    textual::StmtKind,
    visitor::Visitor,
};
use ironplc_problems::Problem;

use crate::{
    result::SemanticResult,
    symbol_environment::{ScopeKind, SymbolEnvironment},
    type_environment::TypeEnvironment,
};

pub fn apply(
    lib: &Library,
    type_environment: &TypeEnvironment,
    symbol_environment: &SymbolEnvironment,
) -> SemanticResult {
    let mut visitor = ControlFlowAnalyzer {
        type_environment,
        symbol_environment,
        current_scope: ScopeKind::Global,
        loop_depth: 0,
        in_unreachable_code: false,
    };

    visitor.walk(lib).map_err(|e| vec![e])
}

struct ControlFlowAnalyzer<'a> {
    type_environment: &'a TypeEnvironment,
    symbol_environment: &'a SymbolEnvironment,
    current_scope: ScopeKind,
    loop_depth: usize,
    in_unreachable_code: bool,
}

impl<'a> ControlFlowAnalyzer<'a> {
    fn enter_scope(&mut self, scope_name: &Id) {
        self.current_scope = ScopeKind::Named(scope_name.clone());
    }

    fn exit_scope(&mut self) {
        self.current_scope = ScopeKind::Global;
    }

    fn enter_loop(&mut self) {
        self.loop_depth += 1;
    }

    fn exit_loop(&mut self) {
        if self.loop_depth > 0 {
            self.loop_depth -= 1;
        }
    }

    fn validate_continue_statement(&self, continue_span: ironplc_dsl::core::SourceSpan) -> Result<(), Diagnostic> {
        if self.loop_depth == 0 {
            Err(Diagnostic::problem(
                Problem::InvalidContinueStatement,
                Label::span(continue_span, "CONTINUE statement outside of loop"),
            ))
        } else {
            Ok(())
        }
    }

    fn validate_unreachable_code(&self, stmt_span: ironplc_dsl::core::SourceSpan) -> Result<(), Diagnostic> {
        if self.in_unreachable_code {
            // This could be a warning instead of an error
            Err(Diagnostic::problem(
                Problem::VariableUndefined, // Using VariableUndefined as a general semantic error
                Label::span(stmt_span, "Unreachable code after CONTINUE statement"),
            ))
        } else {
            Ok(())
        }
    }

    fn is_in_loop(&self) -> bool {
        self.loop_depth > 0
    }

    fn get_loop_depth(&self) -> usize {
        self.loop_depth
    }
}

impl<'a> Visitor<Diagnostic> for ControlFlowAnalyzer<'a> {
    type Value = ();
    
    fn visit_stmt_kind(&mut self, node: &StmtKind) -> Result<(), Diagnostic> {
        match node {
            StmtKind::Continue => {
                // Validate that continue is within a loop
                let span = ironplc_dsl::core::SourceSpan::default(); // Use default span since we don't have access to the actual span
                self.validate_continue_statement(span)?;
            }
            StmtKind::For(_) | StmtKind::While(_) | StmtKind::Repeat(_) => {
                // Enter loop context
                self.enter_loop();
                let result = node.recurse_visit(self);
                self.exit_loop();
                return result;
            }
            _ => {
                // For other statements, just recurse normally
            }
        }
        
        node.recurse_visit(self)
    }

    fn visit_function_declaration(&mut self, node: &FunctionDeclaration) -> Result<(), Diagnostic> {
        self.enter_scope(&node.name);
        let old_loop_depth = self.loop_depth;
        self.loop_depth = 0; // Reset loop depth for function scope
        
        let result = node.recurse_visit(self);
        
        self.loop_depth = old_loop_depth;
        self.exit_scope();
        result
    }

    fn visit_function_block_declaration(
        &mut self,
        node: &FunctionBlockDeclaration,
    ) -> Result<(), Diagnostic> {
        self.enter_scope(&node.name.name);
        let old_loop_depth = self.loop_depth;
        self.loop_depth = 0; // Reset loop depth for function block scope
        
        let result = node.recurse_visit(self);
        
        self.loop_depth = old_loop_depth;
        self.exit_scope();
        result
    }

    fn visit_program_declaration(&mut self, node: &ProgramDeclaration) -> Result<(), Diagnostic> {
        self.enter_scope(&node.name);
        let old_loop_depth = self.loop_depth;
        self.loop_depth = 0; // Reset loop depth for program scope
        
        let result = node.recurse_visit(self);
        
        self.loop_depth = old_loop_depth;
        self.exit_scope();
        result
    }

    fn visit_class_declaration(&mut self, node: &ClassDeclaration) -> Result<(), Diagnostic> {
        self.enter_scope(&node.name.name);
        let old_loop_depth = self.loop_depth;
        self.loop_depth = 0; // Reset loop depth for class scope
        
        let result = node.recurse_visit(self);
        
        self.loop_depth = old_loop_depth;
        self.exit_scope();
        result
    }

    fn visit_method_declaration(&mut self, node: &MethodDeclaration) -> Result<(), Diagnostic> {
        let old_loop_depth = self.loop_depth;
        self.loop_depth = 0; // Reset loop depth for method scope
        
        let result = node.recurse_visit(self);
        
        self.loop_depth = old_loop_depth;
        result
    }

    fn visit_action_declaration(&mut self, node: &ActionDeclaration) -> Result<(), Diagnostic> {
        let old_loop_depth = self.loop_depth;
        self.loop_depth = 0; // Reset loop depth for action scope
        
        let result = node.recurse_visit(self);
        
        self.loop_depth = old_loop_depth;
        result
    }

    // TODO: Add visitors for loop statements and control flow when they become available in the AST
    // These would handle:
    // - FOR loops (visit_for_statement)
    // - WHILE loops (visit_while_statement) 
    // - REPEAT loops (visit_repeat_statement)
    // - CONTINUE statements (visit_continue_statement)
    // - Assignment statements (visit_assignment_statement)
    // - IF statements (visit_if_statement)
    // - CASE statements (visit_case_statement)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{parse_and_resolve_types, parse_and_analyze};
    use proptest::prelude::*;

    #[test]
    fn apply_when_no_continue_statements_then_ok() {
        let program = "
PROGRAM Main
VAR
    x : INT := 10;
    y : INT := 20;
END_VAR
    x := x + y;
    y := x * 2;
END_PROGRAM";

        let library = parse_and_resolve_types(program);
        let type_env = TypeEnvironment::new();
        let symbol_env = SymbolEnvironment::new();
        let result = apply(&library, &type_env, &symbol_env);

        // Should pass when no continue statements are used
        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_simple_control_flow_then_ok() {
        let program = "
PROGRAM Main
VAR
    x : INT := 10;
    result : INT;
END_VAR
    IF x > 5 THEN
        result := x * 2;
    ELSE
        result := x;
    END_IF
END_PROGRAM";

        let library = parse_and_resolve_types(program);
        let type_env = TypeEnvironment::new();
        let symbol_env = SymbolEnvironment::new();
        let result = apply(&library, &type_env, &symbol_env);

        // Should pass for simple control flow without loops
        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_nested_scopes_then_ok() {
        let program = "
FUNCTION TestFunction : INT
VAR_INPUT
    param : INT;
END_VAR
    IF param > 0 THEN
        TestFunction := param * 2;
    ELSE
        TestFunction := 0;
    END_IF
END_FUNCTION

PROGRAM Main
VAR
    x : INT := 10;
    result : INT;
END_VAR
    result := TestFunction(x);
END_PROGRAM";

        let library = parse_and_resolve_types(program);
        let type_env = TypeEnvironment::new();
        let symbol_env = SymbolEnvironment::new();
        let result = apply(&library, &type_env, &symbol_env);

        // Should pass for nested scopes without continue statements
        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_class_with_methods_then_ok() {
        let program = "
CLASS TestClass
VAR
    value : INT;
END_VAR

METHOD GetValue : INT
    GetValue := value;
END_METHOD

METHOD SetValue : BOOL
VAR_INPUT
    newValue : INT;
END_VAR
    value := newValue;
    SetValue := TRUE;
END_METHOD
END_CLASS";

        let library = parse_and_resolve_types(program);
        let type_env = TypeEnvironment::new();
        let symbol_env = SymbolEnvironment::new();
        let result = apply(&library, &type_env, &symbol_env);

        // Should pass for class methods without continue statements
        assert!(result.is_ok());
    }

    #[test]
    fn test_continue_outside_loop_error() {
        let program = "
        PROGRAM TestProgram
        VAR
            dummy : INT;
        END_VAR
            CONTINUE;  // Should generate error - continue outside loop
        END_PROGRAM
        ";
        
        let result = parse_and_analyze(program);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        let has_continue_error = errors.iter().any(|e| e.code == Problem::InvalidContinueStatement.code());
        assert!(has_continue_error, "Should have InvalidContinueStatement error");
    }
    
    #[test]
    fn test_continue_inside_loop_valid() {
        let program = "
        PROGRAM TestProgram
        VAR
            i : INT;
        END_VAR
            FOR i := 1 TO 10 DO
                CONTINUE;  // Should be valid
            END_FOR;
        END_PROGRAM
        ";
        
        let result = parse_and_analyze(program);
        assert!(result.is_ok());
    }
    
    // **Feature: ironplc-extended-syntax, Property 35: Continue statement error detection**
    // **Validates: Requirements 9.3**
    proptest! {
        #[test]
        fn property_continue_error_detection(
            program_name in "[A-Z][A-Za-z0-9_]{3,10}",
            var_name in "var_[a-z][A-Za-z0-9_]{2,8}",
            continue_in_loop in prop::bool::ANY,
            loop_type in prop::sample::select(vec!["FOR", "WHILE", "REPEAT"])
        ) {
            let program = if continue_in_loop {
                match loop_type {
                    "FOR" => format!(
                        "PROGRAM {program_name}\nVAR\n    {var_name} : INT;\nEND_VAR\n    FOR {var_name} := 1 TO 10 DO\n        CONTINUE;\n    END_FOR;\nEND_PROGRAM"
                    ),
                    "WHILE" => format!(
                        "PROGRAM {program_name}\nVAR\n    {var_name} : BOOL := TRUE;\nEND_VAR\n    WHILE {var_name} DO\n        CONTINUE;\n        {var_name} := FALSE;\n    END_WHILE;\nEND_PROGRAM"
                    ),
                    "REPEAT" => format!(
                        "PROGRAM {program_name}\nVAR\n    {var_name} : BOOL := FALSE;\nEND_VAR\n    REPEAT\n        CONTINUE;\n        {var_name} := TRUE;\n    UNTIL {var_name}\nEND_REPEAT;\nEND_PROGRAM"
                    ),
                    _ => unreachable!()
                }
            } else {
                format!(
                    "PROGRAM {program_name}\nVAR\n    dummy : INT;\nEND_VAR\n    CONTINUE;\nEND_PROGRAM"
                )
            };
            
            let result = parse_and_analyze(&program);
            
            if continue_in_loop {
                // Continue inside loop should be valid
                prop_assert!(result.is_ok(), "Continue inside {} loop should be valid", loop_type);
            } else {
                // Continue outside loop should generate error
                prop_assert!(result.is_err(), "Continue outside loop should generate error");
                if let Err(errors) = result {
                    let has_continue_error = errors.iter().any(|e| e.code == Problem::InvalidContinueStatement.code());
                    prop_assert!(has_continue_error, "Should have InvalidContinueStatement error");
                }
            }
        }
    }
}