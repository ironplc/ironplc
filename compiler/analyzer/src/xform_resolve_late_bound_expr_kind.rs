//! Transformation rule that decides what a bare identifier in an expression
//! is: a variable or an enumerated value.
//!
//! The parser cannot tell `x := g;` (a variable) from `x := RUNNING;` (an
//! enumerated value), so it records the right-hand side as
//! `ExprKind::LateBound`. This pass decides, once, after the symbol
//! environment exists, with one rule:
//!
//! 1. If the symbol environment finds a variable, parameter or result
//!    variable of that name from the current scope (searching outward, then
//!    the `EXTENDS` chain, then the globals), it is a variable. A variable in
//!    scope hides an enumerated value of the same name.
//! 2. Otherwise, if the name is a known enumeration value, it is an
//!    `EnumeratedValue` with no type name. The expression type pass gives it
//!    its type from the context it is used in.
//! 3. Otherwise it is a variable, so the undeclared-variable rule reports it.
//!
//! The same rule decides a bare identifier used as a structure or function
//! block member initializer, `(x := g)`.
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_dsl::fold::Fold;
use ironplc_dsl::scope::ScopeNode;
use ironplc_dsl::textual::*;
use ironplc_dsl::{
    common::*,
    core::{Id, Located},
};
use std::collections::HashSet;

use crate::symbol_environment::{ScopeTracker, SymbolEnvironment, SymbolKind};

pub fn apply(
    lib: Library,
    symbols: &SymbolEnvironment,
) -> Result<(Library, Vec<Diagnostic>), Vec<Diagnostic>> {
    let enum_values = collect_enum_values(&lib);

    let mut resolver = LateBoundResolver {
        symbols,
        scope: ScopeTracker::default(),
        enum_values,
    };
    let result = resolver.fold_library(lib).map_err(|e| vec![e])?;

    Ok((result, Vec::new()))
}

/// Pre-scans the library to collect all known enumeration value names.
///
/// This enables resolving unqualified enum values (e.g., `RUNNING`) in
/// expression contexts where no enum type context is available, such as
/// comparisons (`State = RUNNING`) or boolean expressions.
///
/// The members of an inline enumeration (`state : (IDLE, RUNNING)`) are
/// collected from the variables of a program, function, function block or
/// method.
fn collect_enum_values(lib: &Library) -> HashSet<Id> {
    let mut values = HashSet::new();

    for element in &lib.elements {
        match element {
            LibraryElementKind::DataTypeDeclaration(DataTypeDeclarationKind::Enumeration(decl)) => {
                if let SpecificationKind::Inline(spec) = &decl.spec_init.spec {
                    for v in &spec.values {
                        values.insert(v.value.clone());
                    }
                }
            }
            LibraryElementKind::FunctionBlockDeclaration(fb) => {
                collect_enum_values_from_vars(&fb.variables, &mut values);
                for method in &fb.methods {
                    collect_enum_values_from_vars(&method.variables, &mut values);
                }
            }
            LibraryElementKind::ProgramDeclaration(prog) => {
                collect_enum_values_from_vars(&prog.variables, &mut values);
            }
            LibraryElementKind::FunctionDeclaration(func) => {
                collect_enum_values_from_vars(&func.variables, &mut values);
            }
            _ => {}
        }
    }

    values
}

fn collect_enum_values_from_vars(variables: &[VarDecl], values: &mut HashSet<Id>) {
    for var in variables {
        if let InitialValueAssignmentKind::EnumeratedValues(init) = &var.initializer {
            for v in &init.values {
                values.insert(v.value.clone());
            }
        }
    }
}

struct LateBoundResolver<'a> {
    symbols: &'a SymbolEnvironment,
    scope: ScopeTracker,
    // Known enumeration value names collected from type declarations
    enum_values: HashSet<Id>,
}

impl LateBoundResolver<'_> {
    /// Whether `name` names a variable from the current scope.
    fn is_variable_in_scope(&self, name: &Id) -> bool {
        self.symbols
            .find(name, &self.scope.current())
            .is_some_and(|symbol| {
                matches!(
                    symbol.kind,
                    SymbolKind::Variable
                        | SymbolKind::Parameter
                        | SymbolKind::OutputParameter
                        | SymbolKind::InOutParameter
                        | SymbolKind::EdgeVariable
                        | SymbolKind::ResultVariable
                )
            })
    }

    /// Resolves a late-bound identifier as either an enum value or variable.
    fn resolve_late_bound(&self, value: Id) -> ExprKind {
        if !self.is_variable_in_scope(&value) && self.enum_values.contains(&value) {
            ExprKind::EnumeratedValue(EnumeratedValue {
                type_name: None,
                value,
                explicit_value: None,
            })
        } else {
            ExprKind::Variable(Variable::Symbolic(SymbolicVariableKind::Named(
                NamedVariable { name: value },
            )))
        }
    }
}

impl Fold<Diagnostic> for LateBoundResolver<'_> {
    fn enter_scope(&mut self, node: ScopeNode<'_>) -> Result<(), Diagnostic> {
        self.scope.enter(&node);
        Ok(())
    }

    fn exit_scope(&mut self) {
        self.scope.exit();
    }

    /// Resolves a bare identifier used as a structure or function-block
    /// member initializer value.
    ///
    /// `(x := g)` is one token in a position that accepts both an enumerated
    /// value and a variable reference, so the parser records it as
    /// `LateBound` rather than guessing. It is decided by the same rule as
    /// every other bare identifier.
    ///
    /// A variable reference becomes an `Expression` -- a value read at
    /// instantiation time, gated by `--allow-struct-initializer-expressions`
    /// (P4043). Anything else is an enumerated value, standard syntax that
    /// the gate must never see.
    fn fold_struct_initial_value_assignment_kind(
        &mut self,
        node: StructInitialValueAssignmentKind,
    ) -> Result<StructInitialValueAssignmentKind, Diagnostic> {
        if let StructInitialValueAssignmentKind::LateBound(late_bound) = node {
            return Ok(match self.resolve_late_bound(late_bound.value) {
                ExprKind::EnumeratedValue(value) => {
                    StructInitialValueAssignmentKind::EnumeratedValue(value)
                }
                resolved => StructInitialValueAssignmentKind::Expression(Expr::new(resolved)),
            });
        }
        node.recurse_fold(self)
    }

    fn fold_assignment(&mut self, node: Assignment) -> Result<Assignment, Diagnostic> {
        // A bare THIS^/SUPER^ is not a value IronPLC supports as a target.
        // Report it, so the construct is not silently accepted.
        // See issue #1406.
        if let Variable::Symbolic(SymbolicVariableKind::SelfRef(self_ref)) = &node.target {
            return Err(Diagnostic::not_implemented(Label::span(
                self_ref.span(),
                format!(
                    "{} is recognized but its members are not yet resolved by IronPLC",
                    self_ref.kind.spelling()
                ),
            )));
        }
        node.recurse_fold(self)
    }

    fn fold_expr_kind(&mut self, node: ExprKind) -> Result<ExprKind, Diagnostic> {
        match node {
            ExprKind::LateBound(late_bound) => Ok(self.resolve_late_bound(late_bound.value)),
            other => other.recurse_fold(self),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::convert::Infallible;

    use ironplc_dsl::common::*;
    use ironplc_dsl::textual::*;
    use ironplc_dsl::visitor::Visitor;
    use ironplc_parser::options::CompilerOptions;
    use rstest::rstest;

    use crate::test_helpers::parse_and_resolve_types_with_options;

    /// What a bare identifier became.
    #[derive(Debug, PartialEq)]
    enum Became {
        EnumeratedValue,
        Variable,
        /// A variable read through its reference (`REFERENCE TO`).
        Dereferenced,
        /// A variable whose address is taken (`ADR(x)`).
        Referenced,
        /// A structure initializer value that reads a variable.
        InitializerExpression,
        /// A structure initializer value that is an enumerated value.
        InitializerEnumeratedValue,
    }

    fn options() -> CompilerOptions {
        CompilerOptions {
            allow_fb_inheritance: true,
            allow_top_level_var_global: true,
            allow_reference_to: true,
            allow_adr: true,
            allow_pointer_to: true,
            allow_struct_initializer_expressions: true,
            ..CompilerOptions::default()
        }
    }

    /// What the value of each assignment and each structure initializer
    /// element of `program` became, in source order.
    fn became(program: &str) -> Vec<Became> {
        struct Collect(Vec<Became>);
        impl Visitor<Infallible> for Collect {
            type Value = ();
            fn visit_assignment(&mut self, node: &Assignment) -> Result<(), Infallible> {
                self.0.push(match &node.value.kind {
                    ExprKind::EnumeratedValue(_) => Became::EnumeratedValue,
                    ExprKind::Variable(_) => Became::Variable,
                    ExprKind::Deref(inner) if matches!(inner.kind, ExprKind::Variable(_)) => {
                        Became::Dereferenced
                    }
                    ExprKind::Ref(_) => Became::Referenced,
                    other => panic!("unexpected value {other:?}"),
                });
                Ok(())
            }
            fn visit_struct_initial_value_assignment_kind(
                &mut self,
                node: &StructInitialValueAssignmentKind,
            ) -> Result<(), Infallible> {
                match node {
                    StructInitialValueAssignmentKind::Expression(_) => {
                        self.0.push(Became::InitializerExpression)
                    }
                    StructInitialValueAssignmentKind::EnumeratedValue(_) => {
                        self.0.push(Became::InitializerEnumeratedValue)
                    }
                    _ => {}
                }
                Ok(())
            }
        }
        let (library, _) = parse_and_resolve_types_with_options(program, &options());
        let mut collect = Collect(vec![]);
        let _ = collect.walk(&library);
        collect.0
    }

    #[rstest]
    #[case::inline_enumeration(
        "FUNCTION_BLOCK FB VAR Color : (Red, Green, Blue); END_VAR Color := Green; END_FUNCTION_BLOCK"
    )]
    #[case::named_enumeration(
        "TYPE Colors : (Red, Green); END_TYPE
         FUNCTION_BLOCK FB VAR Color : Colors; END_VAR Color := Green; END_FUNCTION_BLOCK"
    )]
    #[case::enumeration_with_initial_value(
        "TYPE Colors : (Red, Green); END_TYPE
         FUNCTION_BLOCK FB VAR Color : Colors := Red; END_VAR Color := Green; END_FUNCTION_BLOCK"
    )]
    #[case::inline_enumeration_in_a_method(
        "FUNCTION_BLOCK FB METHOD M VAR t : (T0, T1) := T1; END_VAR t := T0; END_METHOD END_FUNCTION_BLOCK"
    )]
    fn apply_when_no_variable_has_the_name_then_enumerated_value(#[case] program: &str) {
        assert_eq!(vec![Became::EnumeratedValue], became(program));
    }

    /// A variable in scope hides an enumerated value of the same name,
    /// wherever the variable is declared.
    #[rstest]
    #[case::own_variable(
        "FUNCTION_BLOCK FB VAR b : BOOL; Error : BOOL; END_VAR b := Error; END_FUNCTION_BLOCK"
    )]
    #[case::method_input(
        "FUNCTION_BLOCK FB VAR b : BOOL; END_VAR METHOD M VAR_INPUT Error : BOOL; END_VAR b := Error; END_METHOD END_FUNCTION_BLOCK"
    )]
    #[case::method_local(
        "FUNCTION_BLOCK FB VAR b : BOOL; END_VAR METHOD M VAR Error : BOOL; END_VAR b := Error; END_METHOD END_FUNCTION_BLOCK"
    )]
    #[case::global(
        "VAR_GLOBAL Error : BOOL; END_VAR
         PROGRAM main VAR b : BOOL; END_VAR b := Error; END_PROGRAM"
    )]
    #[case::inherited_field(
        "FUNCTION_BLOCK FB_Base VAR Error : BOOL; END_VAR END_FUNCTION_BLOCK
         FUNCTION_BLOCK FB EXTENDS FB_Base VAR b : BOOL; END_VAR METHOD M b := Error; END_METHOD END_FUNCTION_BLOCK"
    )]
    #[case::property_set_input(
        "FUNCTION_BLOCK FB VAR b : BOOL; END_VAR PROPERTY Error : BOOL SET b := Error; END_SET END_PROPERTY END_FUNCTION_BLOCK"
    )]
    #[case::function_input(
        "FUNCTION F : BOOL VAR_INPUT Error : BOOL; END_VAR F := Error; END_FUNCTION"
    )]
    fn apply_when_a_variable_has_the_name_then_variable(#[case] program: &str) {
        // The enumeration is declared in front of every program.
        let program = format!("TYPE E_State : (Idle, Error); END_TYPE\n{program}");
        assert_eq!(vec![Became::Variable], became(&program));
    }

    /// The target being an enumeration does not make the name an enumerated
    /// value: the variable in scope still wins.
    #[test]
    fn apply_when_target_is_enumeration_and_variable_has_the_name_then_variable() {
        let program = "
TYPE E_State : (Idle, Error); END_TYPE
FUNCTION_BLOCK FB
VAR
    s : E_State;
    Error : E_State;
END_VAR
    s := Error;
END_FUNCTION_BLOCK";
        assert_eq!(vec![Became::Variable], became(program));
    }

    #[test]
    fn apply_when_name_is_neither_variable_nor_enumerated_value_then_variable() {
        let program = "
FUNCTION_BLOCK FB
VAR
    b : BOOL;
END_VAR
    b := Undeclared;
END_FUNCTION_BLOCK";
        assert_eq!(vec![Became::Variable], became(program));
    }

    #[rstest]
    #[case::global(
        "VAR_GLOBAL Error : E_State; END_VAR
         PROGRAM main VAR s : S := (x := Error); END_VAR END_PROGRAM",
        Became::InitializerExpression
    )]
    #[case::method_variable(
        "FUNCTION_BLOCK FB METHOD M VAR Error : E_State; s : S := (x := Error); END_VAR END_METHOD END_FUNCTION_BLOCK",
        Became::InitializerExpression
    )]
    #[case::enumerated_value(
        "PROGRAM main VAR s : S := (x := Error); END_VAR END_PROGRAM",
        Became::InitializerEnumeratedValue
    )]
    fn apply_when_structure_initializer_names_something_then_decided_like_any_name(
        #[case] program: &str,
        #[case] expected: Became,
    ) {
        let program = format!(
            "TYPE E_State : (Idle, Error); END_TYPE
             TYPE S : STRUCT x : E_State; END_STRUCT; END_TYPE
             {program}"
        );
        assert_eq!(vec![expected], became(&program));
    }

    /// Implicit dereference and `ADR` need bare names to already be
    /// variables, so a variable named like an enumerated value must be one
    /// before they run.
    #[test]
    fn apply_when_reference_to_variable_has_the_name_then_read_through_the_reference() {
        let program = "
TYPE E_State : (Idle, Error); END_TYPE
PROGRAM main
VAR
    b : BOOL;
    target : BOOL;
    Error : REFERENCE TO BOOL;
END_VAR
    Error REF= target;
    b := Error;
END_PROGRAM";
        // `Error REF= target` binds the reference, then `b := Error` reads
        // through it.
        assert_eq!(
            vec![Became::Referenced, Became::Dereferenced],
            became(program)
        );
    }

    #[test]
    fn apply_when_method_reference_to_variable_has_the_name_then_read_through_the_reference() {
        let program = "
TYPE E_State : (Idle, Error); END_TYPE
FUNCTION_BLOCK FB
VAR
    b : BOOL;
END_VAR
METHOD M
VAR
    target : BOOL;
    Error : REFERENCE TO BOOL;
END_VAR
    Error REF= target;
    b := Error;
END_METHOD
END_FUNCTION_BLOCK";
        assert!(became(program).contains(&Became::Dereferenced));
    }

    #[test]
    fn apply_when_adr_operand_has_the_name_then_address_of_the_variable() {
        let program = "
TYPE E_State : (Idle, Error); END_TYPE
FUNCTION_BLOCK FB
VAR
    p : POINTER TO BOOL;
END_VAR
METHOD M
VAR
    Error : BOOL;
END_VAR
    p := ADR(Error);
END_METHOD
END_FUNCTION_BLOCK";
        assert_eq!(vec![Became::Referenced], became(program));
    }

    #[test]
    fn collect_enum_values_when_inline_enum_in_method_then_collects_members() {
        let program = "
FUNCTION_BLOCK FB_TEST
    METHOD M1 : DINT
        VAR
            t : (T0, T1) := T1;
        END_VAR
        IF t = T1 THEN
            M1 := 1;
        END_IF;
    END_METHOD
END_FUNCTION_BLOCK";
        let options = CompilerOptions {
            allow_fb_inheritance: true,
            ..CompilerOptions::default()
        };
        let library =
            ironplc_parser::parse_program(program, &ironplc_dsl::core::FileId::default(), &options)
                .unwrap();
        let values = super::collect_enum_values(&library);
        assert!(values.contains(&ironplc_dsl::core::Id::from("T0")));
        assert!(values.contains(&ironplc_dsl::core::Id::from("T1")));
    }
}
