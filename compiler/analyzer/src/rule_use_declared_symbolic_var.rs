//! Semantic rule that reference in a function block, function or program to
//! a symbolic variable must be to a symbolic variable that is
//! declared in that scope.
//!
//! ## Passes
//!
//! ```ignore
//! FUNCTION_BLOCK LOGGER
//!    VAR
//!       TRIG : BOOL;
//!       TRIG0 : BOOL;
//!    END_VAR
//!
//!    TRIG := TRIG0;
//! END_FUNCTION_BLOCK
//! ```
//!
//! ```ignore
//! TYPE
//!     MyColors: (Red, Green);
//! END_TYPE
//! FUNCTION_BLOCK
//!     VAR
//!         Color: MyColors := Red;
//!     END_VAR
//!     Color := Green;
//! END_FUNCTION_BLOCK
//! ```
//!   
//! ## Fails
//!
//! ```ignore
//! FUNCTION_BLOCK LOGGER
//!    VAR
//!       TRIG0 : BOOL;
//!    END_VAR
//!
//!    TRIG := TRIG0;
//! END_FUNCTION_BLOCK
//! ```
use std::collections::HashMap;

use ironplc_dsl::{
    common::*,
    core::{Id, Located},
    diagnostic::{Diagnostic, Label},
    scope::ScopeNode,
    visitor::Visitor,
};
use ironplc_problems::Problem;

use crate::{
    intermediates::inherited_fields::collect_inherited_fields,
    result::SemanticResult,
    scoped_table::{self, Key, ScopedTable, Value},
    semantic_context::SemanticContext,
    string_similarity::find_closest_match,
};
use ironplc_parser::options::CompilerOptions;

pub fn apply(
    lib: &Library,
    _context: &SemanticContext,
    options: &CompilerOptions,
) -> SemanticResult {
    let mut checker = SymbolScopeChecker {
        table: scoped_table::ScopedTable::new(),
        inherited_fields: collect_inherited_fields(lib),
    };

    // Seed implicit system globals so direct references don't trigger P4007.
    if options.allow_system_uptime_global {
        checker
            .table
            .add(&Id::from("__SYSTEM_UP_TIME"), DummyNode {});
        checker
            .table
            .add(&Id::from("__SYSTEM_UP_LTIME"), DummyNode {});
    }

    checker.walk(lib).map_err(|e| vec![e])
}

#[derive(Debug)]
struct DummyNode {}
impl Value for DummyNode {}

impl Key for Id {}
impl Key for TypeName {}

/// Wraps `ScopedTable` with the `EXTENDS`-inherited fields per function
/// block (see `intermediates::inherited_fields`), so that a derived
/// function block's own scope also includes fields declared only on its
/// ancestor chain.
struct SymbolScopeChecker<'a> {
    table: ScopedTable<'a, Id, DummyNode>,
    inherited_fields: HashMap<TypeName, Vec<VarDecl>>,
}

impl Visitor<Diagnostic> for SymbolScopeChecker<'_> {
    type Value = ();

    /// Opens the scope of a declaration and seeds the names that are in
    /// scope by virtue of the declaration itself.
    ///
    /// The traversal calls this for every declaration marked
    /// `#[recurse(scope)]`, so this rule states what a scope *contains*
    /// and never which node kinds have one. The match is exhaustive on
    /// purpose: a new kind of scope must be a compile error here rather
    /// than a silently unseeded scope.
    fn enter_scope(&mut self, node: ScopeNode<'_>) -> Result<(), Diagnostic> {
        self.table.enter();

        match node {
            // A function's own name is its implicit result variable, so
            // `FOO := ...` inside `FUNCTION FOO` resolves.
            ScopeNode::Function(node) => {
                self.table.add(&node.name, DummyNode {});
            }
            ScopeNode::Program(node) => {
                self.table.add(&node.name, DummyNode {});
            }
            // A derived function block's scope also holds the fields it
            // inherits through `EXTENDS`, so an unqualified reference to
            // an ancestor's field resolves.
            ScopeNode::FunctionBlock(node) => {
                self.table.add(&node.name.name, DummyNode {});
                if let Some(fields) = self.inherited_fields.get(&node.name).cloned() {
                    for field in &fields {
                        self.table
                            .add_if(field.identifier.symbolic_id(), DummyNode {});
                    }
                }
            }
            // A method's own name is its result variable, exactly as a
            // function's is -- but only when it declares a return type.
            // A method without one is a procedure with no result to
            // assign, so `Foo := ...` inside `METHOD Foo` stays
            // undefined rather than becoming silently legal.
            //
            // The enclosing function block's scope stays open beneath
            // this one, so a method still reads and writes the
            // instance's fields, which is the point of a method.
            ScopeNode::Method(node) => {
                if node.return_type.is_some() {
                    self.table.add(&node.name, DummyNode {});
                }
            }
        }

        Ok(())
    }

    fn exit_scope(&mut self) {
        self.table.exit();
    }

    fn visit_var_decl(&mut self, node: &VarDecl) -> Result<Self::Value, Diagnostic> {
        self.table
            .add_if(node.identifier.symbolic_id(), DummyNode {});
        node.recurse_visit(self)
    }

    fn visit_named_variable(
        &mut self,
        node: &ironplc_dsl::textual::NamedVariable,
    ) -> Result<(), Diagnostic> {
        match self.table.find(&node.name) {
            Some(_) => {
                // We found the variable being referred to
                Ok(())
            }
            None => {
                let suggestion = find_closest_match(
                    node.name.original(),
                    self.table.keys().iter().map(|k| k.original().as_str()),
                );
                let mut diagnostic = Diagnostic::problem(
                    Problem::VariableUndefined,
                    Label::span(node.name.span(), "Undefined variable"),
                )
                .with_context_id("variable", &node.name);
                if let Some(suggestion) = suggestion {
                    diagnostic = diagnostic.with_context("did you mean", &suggestion);
                }
                Err(diagnostic)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::semantic_context::SemanticContextBuilder;
    use crate::test_helpers::parse_and_resolve_types;

    use super::*;

    #[test]
    fn apply_when_function_block_undeclared_symbol_then_error() {
        let program = "
FUNCTION_BLOCK LOGGER
VAR
TRIG0 : BOOL;
END_VAR
         
TRIG := TRIG0.A;
END_FUNCTION_BLOCK";

        let library = parse_and_resolve_types(program);
        let context = SemanticContextBuilder::new().build().unwrap();
        let result = apply(&library, &context, &CompilerOptions::default());

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .first()
            .unwrap()
            .described
            .contains(&"variable=TRIG".to_owned()))
    }

    rule_ok!(
        apply_when_function_block_all_symbol_declared_then_ok,
        "
FUNCTION_BLOCK LOGGER
VAR
TRIG : BOOL;
TRIG0 : BOOL;
END_VAR
         
TRIG := TRIG0;
END_FUNCTION_BLOCK"
    );

    rule_ok!(
        apply_when_function_all_symbol_declared_then_ok,
        "
FUNCTION LOGGER : REAL
VAR_INPUT
TRIG : BOOL;
TRIG0 : BOOL;
END_VAR
         
TRIG := TRIG0;
END_FUNCTION"
    );

    rule_ok!(
        apply_when_program_all_symbol_declared_then_ok,
        "
PROGRAM LOGGER
VAR
TRIG : BOOL;
TRIG0 : BOOL;
END_VAR
         
TRIG := TRIG0;
END_PROGRAM"
    );

    rule_ok!(
        apply_when_assign_enum_variant_then_ok,
        "
TYPE
    MyColors: (Red, Green);
END_TYPE

FUNCTION_BLOCK FB_EXAMPLE
    VAR
        Color: MyColors := Red;
    END_VAR
    Color := Green;
END_FUNCTION_BLOCK"
    );

    #[test]
    fn apply_when_typo_in_variable_name_then_suggests_closest_match() {
        let program = "
FUNCTION_BLOCK LOGGER
VAR
counter : INT;
END_VAR

conter := 1;
END_FUNCTION_BLOCK";

        let library = parse_and_resolve_types(program);
        let context = SemanticContextBuilder::new().build().unwrap();
        let result = apply(&library, &context, &CompilerOptions::default());

        assert!(result.is_err());
        let errors = result.unwrap_err();
        let error = errors.first().unwrap();
        assert!(error.described.contains(&"variable=conter".to_owned()));
        assert!(error.described.contains(&"did you mean=counter".to_owned()));
    }

    #[test]
    fn apply_when_no_similar_variable_then_no_suggestion() {
        let program = "
FUNCTION_BLOCK LOGGER
VAR
x : INT;
END_VAR

completely_different := 1;
END_FUNCTION_BLOCK";

        let library = parse_and_resolve_types(program);
        let context = SemanticContextBuilder::new().build().unwrap();
        let result = apply(&library, &context, &CompilerOptions::default());

        assert!(result.is_err());
        let errors = result.unwrap_err();
        let error = errors.first().unwrap();
        assert!(error
            .described
            .contains(&"variable=completely_different".to_owned()));
        assert!(!error
            .described
            .iter()
            .any(|d| d.starts_with("did you mean")));
    }

    rule_ok!(
        apply_when_enum_value_in_comparison_then_ok,
        "
TYPE
    MotorState : (STOPPED, RUNNING, FAULTED);
END_TYPE

FUNCTION_BLOCK FB_MotorControl
    VAR
        State : MotorState := STOPPED;
        CONTACTOR : BOOL;
        Seal : BOOL;
    END_VAR
    CONTACTOR := (State = RUNNING) AND Seal;
END_FUNCTION_BLOCK"
    );

    #[test]
    fn apply_when_system_uptime_global_enabled_then_direct_access_ok() {
        let program = "
PROGRAM main
VAR
    t : TIME;
END_VAR

t := __SYSTEM_UP_TIME;
END_PROGRAM";

        let library = parse_and_resolve_types(program);
        let context = SemanticContextBuilder::new().build().unwrap();
        let options = CompilerOptions {
            allow_system_uptime_global: true,
            ..CompilerOptions::default()
        };
        let result = apply(&library, &context, &options);

        assert!(result.is_ok());
    }

    rule_err!(
        apply_when_system_uptime_global_disabled_then_direct_access_error,
        "
PROGRAM main
VAR
    t : TIME;
END_VAR

t := __SYSTEM_UP_TIME;
END_PROGRAM"
    );

    // ---------------------------------------------------------------------
    // EXTENDS field inheritance.
    // ---------------------------------------------------------------------

    fn opts_with_fb_inheritance() -> CompilerOptions {
        CompilerOptions {
            allow_fb_inheritance: true,
            ..CompilerOptions::default()
        }
    }

    #[test]
    fn apply_when_unqualified_inherited_field_then_ok() {
        let program = "
FUNCTION_BLOCK FB_Base
VAR
    bEnabled : BOOL;
END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Derived EXTENDS FB_Base
VAR
    bRunning : BOOL;
END_VAR
bRunning := bEnabled;
END_FUNCTION_BLOCK";

        let (library, context) = crate::test_helpers::parse_and_resolve_types_with_options(
            program,
            &opts_with_fb_inheritance(),
        );
        let result = apply(&library, &context, &opts_with_fb_inheritance());

        assert!(result.is_ok(), "unexpected errors: {result:?}");
    }

    #[test]
    fn apply_when_multi_level_inherited_field_then_ok() {
        let program = "
FUNCTION_BLOCK FB_A
VAR
    a : BOOL;
END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_B EXTENDS FB_A
VAR
    b : BOOL;
END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_C EXTENDS FB_B
VAR
    c : BOOL;
END_VAR
c := a AND b;
END_FUNCTION_BLOCK";

        let (library, context) = crate::test_helpers::parse_and_resolve_types_with_options(
            program,
            &opts_with_fb_inheritance(),
        );
        let result = apply(&library, &context, &opts_with_fb_inheritance());

        assert!(result.is_ok(), "unexpected errors: {result:?}");
    }

    #[test]
    fn apply_when_extends_and_genuinely_undeclared_field_then_error() {
        let program = "
FUNCTION_BLOCK FB_Base
VAR
    bEnabled : BOOL;
END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Derived EXTENDS FB_Base
VAR
    bRunning : BOOL;
END_VAR
bRunning := bNotDeclaredAnywhere;
END_FUNCTION_BLOCK";

        let (library, context) = crate::test_helpers::parse_and_resolve_types_with_options(
            program,
            &opts_with_fb_inheritance(),
        );
        let result = apply(&library, &context, &opts_with_fb_inheritance());

        assert!(result.is_err());
    }

    // ---------------------------------------------------------------------
    // METHOD scoping.
    // See specs/plans/2026-08-28-method-scoping-and-scope-paths.md and
    // https://github.com/ironplc/ironplc/issues/1439.
    // ---------------------------------------------------------------------

    /// The standard way a method produces its result, and the same
    /// spelling a `FUNCTION` body already uses.
    #[test]
    fn apply_when_method_assigns_own_name_then_ok() {
        let program = "
FUNCTION_BLOCK FB_Motor
VAR
    speed : REAL;
END_VAR
METHOD GetSpeed : REAL
    GetSpeed := speed;
END_METHOD
END_FUNCTION_BLOCK";

        let (library, context) = crate::test_helpers::parse_and_resolve_types_with_options(
            program,
            &opts_with_fb_inheritance(),
        );
        let result = apply(&library, &context, &opts_with_fb_inheritance());

        assert!(result.is_ok(), "unexpected errors: {result:?}");
    }

    /// A method with no return type has no result to assign, so its name
    /// is not a variable and must stay undefined rather than becoming
    /// silently assignable.
    #[test]
    fn apply_when_method_without_return_type_assigns_own_name_then_error() {
        let program = "
FUNCTION_BLOCK FB_Motor
METHOD DoThing
    DoThing := 1;
END_METHOD
END_FUNCTION_BLOCK";

        let (library, context) = crate::test_helpers::parse_and_resolve_types_with_options(
            program,
            &opts_with_fb_inheritance(),
        );
        let result = apply(&library, &context, &opts_with_fb_inheritance());

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .first()
            .unwrap()
            .described
            .contains(&"variable=DoThing".to_owned()));
    }

    /// Each method's parameters and locals belong to that method. Before
    /// the method scope existed they all landed in the function block's
    /// scope, so this program was accepted.
    #[test]
    fn apply_when_method_references_sibling_method_local_then_error() {
        let program = "
FUNCTION_BLOCK FB_Motor
VAR
    speed : INT;
END_VAR
METHOD SetSpeed
VAR_INPUT
    newSpeed : INT;
END_VAR
    speed := newSpeed;
END_METHOD
METHOD Other
    speed := newSpeed;
END_METHOD
END_FUNCTION_BLOCK";

        let (library, context) = crate::test_helpers::parse_and_resolve_types_with_options(
            program,
            &opts_with_fb_inheritance(),
        );
        let result = apply(&library, &context, &opts_with_fb_inheritance());

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .first()
            .unwrap()
            .described
            .contains(&"variable=newSpeed".to_owned()));
    }

    /// The method scope nests inside the function block's rather than
    /// replacing it -- reading and writing the instance's fields is the
    /// point of a method.
    #[test]
    fn apply_when_method_references_function_block_field_then_ok() {
        let program = "
FUNCTION_BLOCK FB_Motor
VAR
    speed : INT;
END_VAR
METHOD SetSpeed
VAR_INPUT
    newSpeed : INT;
END_VAR
    speed := newSpeed;
END_METHOD
END_FUNCTION_BLOCK";

        let (library, context) = crate::test_helpers::parse_and_resolve_types_with_options(
            program,
            &opts_with_fb_inheritance(),
        );
        let result = apply(&library, &context, &opts_with_fb_inheritance());

        assert!(result.is_ok(), "unexpected errors: {result:?}");
    }

    /// Nesting reaches the whole `EXTENDS` chain, not just the immediately
    /// enclosing function block's own fields.
    #[test]
    fn apply_when_method_references_inherited_field_then_ok() {
        let program = "
FUNCTION_BLOCK FB_Base
VAR
    bEnabled : BOOL;
END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Derived EXTENDS FB_Base
METHOD Enable
    bEnabled := TRUE;
END_METHOD
END_FUNCTION_BLOCK";

        let (library, context) = crate::test_helpers::parse_and_resolve_types_with_options(
            program,
            &opts_with_fb_inheritance(),
        );
        let result = apply(&library, &context, &opts_with_fb_inheritance());

        assert!(result.is_ok(), "unexpected errors: {result:?}");
    }

    /// Sibling scopes, so the same name in two methods is two variables
    /// and not a redeclaration.
    #[test]
    fn apply_when_two_methods_declare_same_local_name_then_ok() {
        let program = "
FUNCTION_BLOCK FB_Motor
METHOD A
VAR
    q : INT;
END_VAR
    q := 1;
END_METHOD
METHOD B
VAR
    q : INT;
END_VAR
    q := 2;
END_METHOD
END_FUNCTION_BLOCK";

        let (library, context) = crate::test_helpers::parse_and_resolve_types_with_options(
            program,
            &opts_with_fb_inheritance(),
        );
        let result = apply(&library, &context, &opts_with_fb_inheritance());

        assert!(result.is_ok(), "unexpected errors: {result:?}");
    }
}
