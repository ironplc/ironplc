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
use std::convert::Infallible;

use ironplc_dsl::{
    common::*,
    core::{Id, Located},
    diagnostic::{Diagnostic, Label},
    scope::ScopeNode,
    visitor::Visitor,
};
use ironplc_problems::Problem;

use crate::{
    result::SemanticResult,
    rule_support::{run_rule, DiagnosticVisitor},
    semantic_context::SemanticContext,
    string_similarity::find_closest_match,
    symbol_environment::{ScopeKind, ScopeTracker, SymbolEnvironment, SymbolInfo, SymbolKind},
};
use ironplc_parser::options::CompilerOptions;

pub fn apply(
    lib: &Library,
    context: &SemanticContext,
    options: &CompilerOptions,
) -> SemanticResult {
    run_rule(
        SymbolScopeChecker {
            symbols: context.symbols(),
            scope: ScopeTracker::default(),
            units: Vec::new(),
            bare_globals: options.allow_top_level_var_global,
            enclosing_properties: Vec::new(),
            diagnostics: Vec::new(),
        },
        lib,
    )
}

/// Checks each name a body uses against the symbol environment, from the
/// scope the body is in. The environment answers for the scope's own
/// variables, those of the scopes enclosing it, the fields a function
/// block inherits through `EXTENDS`, and the globals.
struct SymbolScopeChecker<'a> {
    symbols: &'a SymbolEnvironment,
    /// Where the traversal is, to look names up in the symbol environment.
    scope: ScopeTracker,
    /// One entry per open scope: the name of the function block or program
    /// that opened it, which is in scope within its body, and `None` for a
    /// function or method, whose own name is its result variable.
    units: Vec<Option<Id>>,
    /// Whether a body may use a `VAR_GLOBAL` directly, without a
    /// `VAR_EXTERNAL` naming it. The vendor dialects that declare globals
    /// in top-level lists (`--allow-top-level-var-global`) allow it; IEC
    /// 61131-3 reaches a global only through `VAR_EXTERNAL`.
    bare_globals: bool,
    /// One entry per open scope: the property names of the function block
    /// that opened it, `None` for any other scope. A name that is not a
    /// variable but is a property of the enclosing function block is a
    /// property access, which is not implemented yet, rather than an
    /// undefined variable.
    enclosing_properties: Vec<Option<Vec<Id>>>,
    diagnostics: Vec<Diagnostic>,
}

impl SymbolScopeChecker<'_> {
    fn is_enclosing_property(&self, name: &Id) -> bool {
        self.enclosing_properties
            .iter()
            .rev()
            .find_map(|properties| properties.as_ref())
            .is_some_and(|properties| properties.contains(name))
    }

    /// Whether a variable `info` describes can be used by name from here.
    ///
    /// A `VAR_GLOBAL` is reached through a `VAR_EXTERNAL`, which is a
    /// variable of the scope that declares it, unless the dialect lets a
    /// body use globals directly. A global the compiler provides is always
    /// usable.
    fn is_usable_variable(&self, info: &SymbolInfo) -> bool {
        let is_variable = matches!(
            info.kind,
            SymbolKind::Variable
                | SymbolKind::Parameter
                | SymbolKind::OutputParameter
                | SymbolKind::InOutParameter
                | SymbolKind::EdgeVariable
                | SymbolKind::Constant
                | SymbolKind::ResultVariable
        );
        if !is_variable {
            return false;
        }
        let is_declared_global =
            info.scope == ScopeKind::Global && info.variable_type == Some(VariableType::Global);
        !is_declared_global || info.compiler_provided || self.bare_globals
    }

    /// Whether `name` is in scope where the traversal is.
    fn is_in_scope(&self, name: &Id) -> bool {
        if self.units.iter().flatten().any(|unit| unit == name) {
            return true;
        }
        self.symbols
            .find(name, &self.scope.current())
            .is_some_and(|info| self.is_usable_variable(info))
    }
}

impl DiagnosticVisitor for SymbolScopeChecker<'_> {
    fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

impl Visitor<Infallible> for SymbolScopeChecker<'_> {
    type Value = ();

    /// Tracks the scope of a declaration.
    ///
    /// The traversal calls this for every declaration marked
    /// `#[recurse(scope)]`. The match is exhaustive on purpose: a new kind
    /// of scope must be a compile error here rather than a scope whose own
    /// name is silently out of reach. A function's or a method's own name
    /// is its result variable, which the symbol environment holds.
    fn enter_scope(&mut self, node: ScopeNode<'_>) -> Result<(), Infallible> {
        self.scope.enter(&node);
        self.enclosing_properties.push(match &node {
            ScopeNode::FunctionBlock(node) => {
                Some(node.properties.iter().map(|p| p.name.clone()).collect())
            }
            _ => None,
        });
        self.units.push(match node {
            ScopeNode::FunctionBlock(node) => Some(node.name.name.clone()),
            ScopeNode::Program(node) => Some(node.name.clone()),
            ScopeNode::Function(_) | ScopeNode::Method(_) => None,
        });
        Ok(())
    }

    fn exit_scope(&mut self) {
        self.scope.exit();
        self.enclosing_properties.pop();
        self.units.pop();
    }

    fn visit_named_variable(
        &mut self,
        node: &ironplc_dsl::textual::NamedVariable,
    ) -> Result<(), Infallible> {
        if self.is_in_scope(&node.name) {
            // We found the variable being referred to
            return Ok(());
        }

        if self.is_enclosing_property(&node.name) {
            self.diagnostics.push(
                Diagnostic::not_implemented(Label::span(node.name.span(), "Use of a PROPERTY"))
                    .with_context_id("property", &node.name),
            );
            return Ok(());
        }

        let visible = self.symbols.visible_variables(&self.scope.current());
        let suggestion = find_closest_match(
            node.name.original(),
            visible
                .iter()
                .filter(|(_, info)| self.is_usable_variable(info))
                .map(|(name, _)| name.original().as_str()),
        );
        let mut diagnostic = Diagnostic::problem(
            Problem::VariableUndefined,
            Label::span(node.name.span(), "Undefined variable"),
        )
        .with_context_id("variable", &node.name);
        if let Some(suggestion) = suggestion {
            diagnostic = diagnostic.with_context("did you mean", &suggestion);
        }
        self.diagnostics.push(diagnostic);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::test_helpers::fb_inheritance_options;
    use crate::test_helpers::NOT_IMPLEMENTED_CODE;
    use crate::test_helpers::{diagnostic_codes, rule_diagnostics};

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

        let diagnostics = rule_diagnostics(apply, program, &CompilerOptions::default());

        assert_eq!(
            diagnostic_codes(&diagnostics),
            [Problem::VariableUndefined.code()]
        );
        assert!(diagnostics[0]
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

        let diagnostics = rule_diagnostics(apply, program, &CompilerOptions::default());

        assert_eq!(
            diagnostic_codes(&diagnostics),
            [Problem::VariableUndefined.code()]
        );
        let error = &diagnostics[0];
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

        let diagnostics = rule_diagnostics(apply, program, &CompilerOptions::default());

        assert_eq!(
            diagnostic_codes(&diagnostics),
            [Problem::VariableUndefined.code()]
        );
        let error = &diagnostics[0];
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

        let options = CompilerOptions {
            allow_system_uptime_global: true,
            ..CompilerOptions::default()
        };
        let diagnostics = rule_diagnostics(apply, program, &options);

        assert!(diagnostics.is_empty());
    }

    rule_err!(
        apply_when_system_uptime_global_disabled_then_direct_access_error,
        "
PROGRAM main
VAR
    t : TIME;
END_VAR

t := __SYSTEM_UP_TIME;
END_PROGRAM",
        [Problem::VariableUndefined]
    );

    // ---------------------------------------------------------------------
    // EXTENDS field inheritance.
    // ---------------------------------------------------------------------

    rule_ok!(
        apply_when_unqualified_inherited_field_then_ok,
        "
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
END_FUNCTION_BLOCK",
        fb_inheritance_options()
    );

    rule_ok!(
        apply_when_multi_level_inherited_field_then_ok,
        "
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
END_FUNCTION_BLOCK",
        fb_inheritance_options()
    );

    rule_err!(
        apply_when_extends_and_genuinely_undeclared_field_then_error,
        "
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
END_FUNCTION_BLOCK",
        [Problem::VariableUndefined],
        fb_inheritance_options()
    );

    // ---------------------------------------------------------------------
    // METHOD scoping.
    // See https://github.com/ironplc/ironplc/issues/1439.
    // ---------------------------------------------------------------------

    rule_ok!(
        /// The standard way a method produces its result, and the same
        /// spelling a `FUNCTION` body already uses.
        apply_when_method_assigns_own_name_then_ok,
        "
FUNCTION_BLOCK FB_Motor
VAR
    speed : REAL;
END_VAR
METHOD GetSpeed : REAL
    GetSpeed := speed;
END_METHOD
END_FUNCTION_BLOCK",
        fb_inheritance_options()
    );

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

        let diagnostics = rule_diagnostics(apply, program, &fb_inheritance_options());

        assert_eq!(
            diagnostic_codes(&diagnostics),
            [Problem::VariableUndefined.code()]
        );
        assert!(diagnostics[0]
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

        let diagnostics = rule_diagnostics(apply, program, &fb_inheritance_options());

        assert_eq!(
            diagnostic_codes(&diagnostics),
            [Problem::VariableUndefined.code()]
        );
        assert!(diagnostics[0]
            .described
            .contains(&"variable=newSpeed".to_owned()));
    }

    rule_ok!(
        /// The method scope nests inside the function block's rather than
        /// replacing it -- reading and writing the instance's fields is the
        /// point of a method.
        apply_when_method_references_function_block_field_then_ok,
        "
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
END_FUNCTION_BLOCK",
        fb_inheritance_options()
    );

    rule_ok!(
        /// Nesting reaches the whole `EXTENDS` chain, not just the immediately
        /// enclosing function block's own fields.
        apply_when_method_references_inherited_field_then_ok,
        "
FUNCTION_BLOCK FB_Base
VAR
    bEnabled : BOOL;
END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Derived EXTENDS FB_Base
METHOD Enable
    bEnabled := TRUE;
END_METHOD
END_FUNCTION_BLOCK",
        fb_inheritance_options()
    );

    rule_ok!(
        /// Sibling scopes, so the same name in two methods is two variables
        /// and not a redeclaration.
        apply_when_two_methods_declare_same_local_name_then_ok,
        "
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
END_FUNCTION_BLOCK",
        fb_inheritance_options()
    );

    /// Reproduces issue #1566: the rule used to abort at the first undefined
    /// variable, so a program with two of them reported one.
    #[test]
    fn apply_when_two_undefined_variables_then_reports_both() {
        let program = "
PROGRAM prog_two
VAR
  x : INT;
END_VAR
  x := UNDECLARED_ONE;
  x := UNDECLARED_TWO;
END_PROGRAM";

        let diagnostics = rule_diagnostics(apply, program, &CompilerOptions::default());

        assert_eq!(
            diagnostic_codes(&diagnostics),
            [Problem::VariableUndefined.code(); 2]
        );

        let reported: Vec<&String> = diagnostics.iter().flat_map(|d| &d.described).collect();
        assert!(
            reported
                .iter()
                .any(|d| d.as_str() == "variable=UNDECLARED_ONE"),
            "expected UNDECLARED_ONE, got {reported:?}"
        );
        assert!(
            reported
                .iter()
                .any(|d| d.as_str() == "variable=UNDECLARED_TWO"),
            "expected UNDECLARED_TWO, got {reported:?}"
        );
    }

    /// The other half of issue #1566: a second POU must not displace the
    /// first POU's diagnostics.
    #[test]
    fn apply_when_undefined_variables_in_two_pous_then_reports_both() {
        let program = "
PROGRAM a
VAR
  x : INT;
END_VAR
  x := AAA_ONE;
END_PROGRAM

PROGRAM b
VAR
  y : INT;
END_VAR
  y := BBB_ONE;
END_PROGRAM";

        let diagnostics = rule_diagnostics(apply, program, &CompilerOptions::default());

        assert_eq!(
            diagnostic_codes(&diagnostics),
            [Problem::VariableUndefined.code(); 2]
        );

        let reported: Vec<&String> = diagnostics.iter().flat_map(|d| &d.described).collect();
        assert!(
            reported.iter().any(|d| d.as_str() == "variable=AAA_ONE"),
            "expected AAA_ONE, got {reported:?}"
        );
        assert!(
            reported.iter().any(|d| d.as_str() == "variable=BBB_ONE"),
            "expected BBB_ONE, got {reported:?}"
        );
    }

    // ---------------------------------------------------------------------
    // PROPERTY accessors and property use.
    // ---------------------------------------------------------------------

    rule_ok!(
        /// Each accessor is a method (see `PropertyDeclaration`), so its body
        /// sees the property name (GET result, SET input), its own variables,
        /// and the function block's fields.
        apply_when_property_accessors_use_property_name_and_fields_then_ok,
        "
FUNCTION_BLOCK FB_Motor
VAR
    _speed : REAL;
END_VAR
PROPERTY Speed : REAL
GET
VAR
    tmp : REAL;
END_VAR
    tmp := _speed;
    Speed := tmp;
END_GET
SET
    _speed := Speed;
END_SET
END_PROPERTY
END_FUNCTION_BLOCK",
        fb_inheritance_options()
    );

    /// Reading a property by its bare name in the function block body is a
    /// property access, which is not implemented yet. It must not be
    /// reported as an undefined variable.
    #[test]
    fn apply_when_fb_body_uses_own_property_then_not_implemented_names_property() {
        let program = "
FUNCTION_BLOCK FB_Motor
VAR
    _speed : REAL;
    y : REAL;
END_VAR
y := Running;
PROPERTY Running : BOOL
GET
    Running := _speed > 0.0;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK";

        let errors = rule_diagnostics(apply, program, &fb_inheritance_options());

        assert_eq!(diagnostic_codes(&errors), [NOT_IMPLEMENTED_CODE]);
        assert!(errors[0].described.contains(&"property=Running".to_owned()));
    }

    /// A method body sees the enclosing function block's properties too.
    #[test]
    fn apply_when_method_uses_enclosing_property_then_not_implemented() {
        let program = "
FUNCTION_BLOCK FB_Motor
VAR
    _speed : REAL;
END_VAR
METHOD Stop
    Speed := 0.0;
END_METHOD
PROPERTY Speed : REAL
SET
    _speed := Speed;
END_SET
END_PROPERTY
END_FUNCTION_BLOCK";

        let errors = rule_diagnostics(apply, program, &fb_inheritance_options());

        assert_eq!(diagnostic_codes(&errors), [NOT_IMPLEMENTED_CODE]);
        assert!(errors[0].described.contains(&"property=Speed".to_owned()));
    }

    /// Property names are visible only inside their own function block.
    #[test]
    fn apply_when_program_uses_name_of_some_property_then_undefined_variable() {
        let program = "
FUNCTION_BLOCK FB_Motor
PROPERTY Speed : REAL
GET
    Speed := 1.0;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

PROGRAM main
VAR
    y : REAL;
END_VAR
y := Speed;
END_PROGRAM";

        let errors = rule_diagnostics(apply, program, &fb_inheritance_options());

        assert_eq!(
            diagnostic_codes(&errors),
            [Problem::VariableUndefined.code()]
        );

        assert!(errors[0].described.contains(&"variable=Speed".to_owned()));
    }

    const CONFIG_WITH_GLOBAL: &str = "
CONFIGURATION config
  VAR_GLOBAL
    g : INT;
  END_VAR
  RESOURCE res ON PLC
    TASK t(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM inst WITH t : main;
  END_RESOURCE
END_CONFIGURATION
";

    const PROGRAM_USING_GLOBAL: &str = "
PROGRAM main
VAR
  x : INT;
END_VAR
  x := g;
END_PROGRAM
";

    fn top_level_globals() -> CompilerOptions {
        CompilerOptions {
            allow_top_level_var_global: true,
            ..CompilerOptions::default()
        }
    }

    rule_err!(
        apply_when_global_used_without_external_after_configuration_then_error,
        &format!("{CONFIG_WITH_GLOBAL}{PROGRAM_USING_GLOBAL}"),
        [Problem::VariableUndefined]
    );

    rule_err!(
        apply_when_global_used_without_external_before_configuration_then_error,
        &format!("{PROGRAM_USING_GLOBAL}{CONFIG_WITH_GLOBAL}"),
        [Problem::VariableUndefined]
    );

    rule_ok!(
        apply_when_global_used_through_external_then_ok,
        &format!(
            "{CONFIG_WITH_GLOBAL}
PROGRAM main
VAR_EXTERNAL
  g : INT;
END_VAR
VAR
  x : INT;
END_VAR
  x := g;
END_PROGRAM"
        )
    );

    rule_ok!(
        apply_when_top_level_globals_allowed_and_global_used_directly_then_ok,
        &format!("{PROGRAM_USING_GLOBAL}{CONFIG_WITH_GLOBAL}"),
        top_level_globals()
    );

    rule_ok!(
        apply_when_top_level_global_used_directly_then_ok,
        "
VAR_GLOBAL
  g : INT;
END_VAR
PROGRAM main
VAR
  x : INT;
END_VAR
  x := g;
END_PROGRAM",
        top_level_globals()
    );
}
