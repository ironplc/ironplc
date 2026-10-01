//! Tests of the type an unqualified enumerated value takes from where it is
//! used (see `crate::enumerated_value_type`).

use super::*;
use ironplc_dsl::common::{TypeName, VarDecl};
use ironplc_dsl::type_id::TypeId;

/// Two enumerations that share the value name `GREEN`.
const TYPES: &str = "
TYPE
    COLOR : (RED, GREEN);
    LIGHT : (OFF, GREEN);
    PAINT : COLOR;
END_TYPE";

/// The type of every enumerated value expression, in source order.
fn enumerated_value_types(resolved: &Resolved) -> Vec<Option<ExprType>> {
    struct Collect(Vec<Option<ExprType>>);
    impl Fold<()> for Collect {
        fn fold_expr(&mut self, node: Expr) -> Result<Expr, ()> {
            if let ExprKind::EnumeratedValue(_) = node.kind {
                self.0.push(node.expr_type.clone());
            }
            node.recurse_fold(self)
        }
    }
    let mut collect = Collect(vec![]);
    let _ = collect.fold_library(resolved.library.clone());
    collect.0
}

/// The type id of the variable `name` the library declares.
fn declared_type_id(resolved: &Resolved, name: &str) -> TypeId {
    struct Find(&'static str, Option<TypeId>);
    impl Fold<()> for Find {
        fn fold_var_decl(&mut self, node: VarDecl) -> Result<VarDecl, ()> {
            if node
                .identifier
                .symbolic_id()
                .is_some_and(|id| *id == ironplc_dsl::core::Id::from(self.0))
            {
                self.1 = node.type_id;
            }
            Ok(node)
        }
    }
    let name: &'static str = Box::leak(name.to_string().into_boxed_str());
    let mut find = Find(name, None);
    let _ = find.fold_library(resolved.library.clone());
    find.1.unwrap()
}

fn named(resolved: &Resolved, name: &str) -> Option<ExprType> {
    Some(ExprType::Concrete(
        resolved.types.id_of(&TypeName::from(name)).unwrap(),
    ))
}

fn codes(resolved: &Resolved) -> Vec<&str> {
    resolved
        .diagnostics
        .iter()
        .map(|d| d.code.as_str())
        .collect()
}

#[test]
fn apply_when_assigned_then_value_has_target_type() {
    let resolved = run_pass(&format!(
        "{TYPES}
PROGRAM main
VAR c : COLOR; l : LIGHT; p : PAINT; END_VAR
    c := GREEN;
    l := GREEN;
    p := GREEN;
END_PROGRAM"
    ));

    assert_eq!(
        enumerated_value_types(&resolved),
        vec![
            named(&resolved, "COLOR"),
            named(&resolved, "LIGHT"),
            named(&resolved, "PAINT")
        ]
    );
    assert!(codes(&resolved).is_empty());
}

#[test]
fn apply_when_compared_then_value_has_other_operand_type() {
    let resolved = run_pass(&format!(
        "{TYPES}
PROGRAM main
VAR l : LIGHT; b : BOOL; END_VAR
    b := l = GREEN;
    b := GREEN <> l;
END_PROGRAM"
    ));

    assert_eq!(
        enumerated_value_types(&resolved),
        vec![named(&resolved, "LIGHT"), named(&resolved, "LIGHT")]
    );
}

#[test]
fn apply_when_inline_enumeration_target_then_value_has_its_anonymous_type() {
    let resolved = run_pass(&format!(
        "{TYPES}
PROGRAM main
VAR e : (IDLE, GREEN); END_VAR
    e := GREEN;
END_PROGRAM"
    ));

    let e = declared_type_id(&resolved, "e");
    assert_eq!(
        enumerated_value_types(&resolved),
        vec![Some(ExprType::Concrete(e))]
    );
}

#[test]
fn apply_when_function_block_input_then_value_has_input_type() {
    let resolved = run_pass(
        "
FUNCTION_BLOCK FB1
VAR_INPUT i : (P, Q, R); END_VAR
END_FUNCTION_BLOCK
FUNCTION_BLOCK FB2
VAR_INPUT j : (R, S); END_VAR
END_FUNCTION_BLOCK
PROGRAM main
VAR fb : FB1; END_VAR
    fb(i := R);
END_PROGRAM",
    );

    let i = declared_type_id(&resolved, "i");
    assert_eq!(
        enumerated_value_types(&resolved),
        vec![Some(ExprType::Concrete(i))]
    );
}

#[test]
fn apply_when_one_enumeration_declares_value_then_value_has_its_type() {
    let resolved = run_pass(&format!(
        "{TYPES}
PROGRAM main
VAR d : DINT; END_VAR
    d := RED;
END_PROGRAM"
    ));

    assert_eq!(
        enumerated_value_types(&resolved),
        vec![named(&resolved, "COLOR")]
    );
}

#[test]
fn apply_when_no_context_and_two_enumerations_declare_value_then_ambiguous() {
    let resolved = run_pass(&format!(
        "{TYPES}
PROGRAM main
VAR d : DINT; END_VAR
    d := GREEN;
END_PROGRAM"
    ));

    assert_eq!(enumerated_value_types(&resolved), vec![None]);
    assert_eq!(codes(&resolved), vec!["P2043"]);
}

#[test]
fn apply_when_inline_enumeration_of_other_pou_then_not_a_candidate() {
    let resolved = run_pass(
        "
FUNCTION_BLOCK FB1
VAR e : (IDLE, BUSY); END_VAR
END_FUNCTION_BLOCK
PROGRAM main
VAR s : (BUSY, IDLE); d : DINT; END_VAR
    d := BUSY;
END_PROGRAM",
    );

    let s = declared_type_id(&resolved, "s");
    assert_eq!(
        enumerated_value_types(&resolved),
        vec![Some(ExprType::Concrete(s))]
    );
    assert!(codes(&resolved).is_empty());
}

#[test]
fn apply_when_compared_with_structure_field_then_value_has_field_type() {
    let resolved = run_pass(&format!(
        "{TYPES}
TYPE S : STRUCT f : LIGHT; END_STRUCT; END_TYPE
PROGRAM main
VAR s : S; b : BOOL; END_VAR
    b := s.f = GREEN;
    s.f := GREEN;
END_PROGRAM"
    ));

    assert_eq!(
        enumerated_value_types(&resolved),
        vec![named(&resolved, "LIGHT"), named(&resolved, "LIGHT")]
    );
    assert!(codes(&resolved).is_empty());
}
