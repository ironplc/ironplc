//! Tests of the control-flow statements' constructors, their text form, and
//! the expansion of a `For`.

use super::*;

/// `place := value` at a `DINT` place, for a statement inside another.
fn set(f: &Fixture, var: VarId, value: i128) -> Stmt {
    f.b.assign(f.place(var), int(value), span()).unwrap()
}

fn compare(op: CompareOp, lhs: Expr, rhs: Expr) -> Expr {
    Expr::compare(op, lhs, rhs, span()).unwrap()
}

#[test]
fn if_when_condition_not_bool_then_internal_error() {
    assert!(is_internal_error_naming(
        Stmt::if_(int(1), vec![], vec![], span()),
        "Stmt::if_"
    ));
}

#[test]
fn if_when_statements_name_places_of_two_programs_then_internal_error() {
    let f = fixture();
    let g = fixture();
    let cond = compare(CompareOp::Gt, f.read(f.dint), int(0));
    assert!(is_internal_error_naming(
        Stmt::if_(cond, vec![set(&g, g.dint, 1)], vec![], span()),
        "Stmt::if_"
    ));
}

#[test]
fn if_when_built_then_prints_branches() {
    let f = fixture();
    let cond = compare(CompareOp::Gt, f.read(f.dint), int(0));
    let stmt = Stmt::if_(
        cond,
        vec![set(&f, f.dint, 1)],
        vec![set(&f, f.dint, 2)],
        span(),
    )
    .unwrap();
    assert_eq!(
        stmt.to_string(),
        "if (gt (read i32s v0) (const i32s 0))\n  assign v0 (const i32s 1)\nelse\n  assign v0 (const i32s 2)\nend"
    );
}

/// REQ-LOW-lowering-064: a label is a constant of the selector's type.
#[test]
fn case_when_label_not_of_selector_type_then_internal_error() {
    let f = fixture();
    let arm = CaseArm::new(vec![CaseLabel::Value(Const::I64(1))], vec![]);
    assert!(is_internal_error_naming(
        Stmt::case(f.read(f.dint), vec![arm], vec![], span()),
        "Stmt::case"
    ));
}

/// REQ-LOW-lowering-064: a range runs from its lower value to its higher one.
#[test]
fn case_when_range_reversed_then_internal_error() {
    let f = fixture();
    let arm = CaseArm::new(vec![CaseLabel::Range(Const::I32(5), Const::I32(1))], vec![]);
    assert!(is_internal_error_naming(
        Stmt::case(f.read(f.dint), vec![arm], vec![], span()),
        "Stmt::case"
    ));
}

#[test]
fn case_when_selector_not_integer_then_internal_error() {
    let f = fixture();
    assert!(is_internal_error_naming(
        Stmt::case(f.read(f.bool), vec![], vec![], span()),
        "Stmt::case"
    ));
}

#[test]
fn case_when_built_then_prints_arms() {
    let f = fixture();
    let labels = vec![
        CaseLabel::Value(Const::I32(1)),
        CaseLabel::Range(Const::I32(3), Const::I32(5)),
    ];
    let arm = CaseArm::new(labels, vec![set(&f, f.dint, 7)]);
    let stmt = Stmt::case(f.read(f.dint), vec![arm], vec![set(&f, f.dint, 0)], span()).unwrap();
    assert_eq!(
        stmt.to_string(),
        "case (read i32s v0)\nof 1, 3..5\n  assign v0 (const i32s 7)\nelse\n  assign v0 (const i32s 0)\nend"
    );
}

/// REQ-LOW-lowering-062: an `Exit` is inside the loop it names.
#[test]
fn loop_when_exit_outside_it_then_internal_error() {
    let mut f = fixture();
    let scope = f.b.open_loop();
    let _outside = scope.exit(span());
    assert!(is_internal_error_naming(
        f.b.loop_(scope, vec![], vec![], span()),
        "Stmt::loop_"
    ));
}

/// REQ-LOW-lowering-062: an `Exit` nested in a statement of the loop's body
/// names it.
#[test]
fn loop_when_exit_inside_then_names_loop() {
    let mut f = fixture();
    let scope = f.b.open_loop();
    let cond = compare(CompareOp::Gt, f.read(f.dint), int(9));
    let leave = Stmt::if_(cond, vec![scope.exit(span())], vec![], span()).unwrap();
    let stmt =
        f.b.loop_(scope, vec![leave], vec![set(&f, f.dint, 1)], span())
            .unwrap();
    assert_eq!(
        stmt.to_string(),
        "loop loop0\n  if (gt (read i32s v0) (const i32s 9))\n    exit loop0\n  end\ncontinuing\n  assign v0 (const i32s 1)\nend"
    );
}

#[test]
fn loop_when_scope_of_another_program_then_internal_error() {
    let f = fixture();
    let mut g = fixture();
    let scope = g.b.open_loop();
    assert!(is_internal_error_naming(
        f.b.loop_(scope, vec![], vec![], span()),
        "ProgramBuilder::loop_"
    ));
}

/// A `For` counting `control` from 1 to 10 by `step`.
fn for_parts(control: Place, step: Const, body: Vec<Stmt>) -> ForParts {
    ForParts {
        control,
        from: int(1),
        to: int(10),
        step,
        body,
    }
}

/// REQ-LOW-lowering-067: the step of a `For` is a nonzero constant.
#[test]
fn for_when_step_zero_then_internal_error() {
    let mut f = fixture();
    let scope = f.b.open_loop();
    let parts = for_parts(f.place(f.dint), Const::I32(0), vec![]);
    assert!(is_internal_error_naming(
        f.b.for_(scope, parts, span()),
        "Stmt::for_"
    ));
}

/// REQ-LOW-lowering-067: the control of a `For` is an integer.
#[test]
fn for_when_control_not_integer_then_internal_error() {
    let mut f = fixture();
    let scope = f.b.open_loop();
    let parts = for_parts(f.place(f.lreal), Const::I32(1), vec![]);
    assert!(is_internal_error_naming(
        f.b.for_(scope, parts, span()),
        "Stmt::for_"
    ));
}

/// REQ-LOW-lowering-066: the first value already fits the control's storage.
#[test]
fn for_when_first_value_not_narrowed_then_internal_error() {
    let mut f = fixture();
    let scope = f.b.open_loop();
    let unnarrowed = Expr::binary(BinaryOp::Add, f.read(f.int), int(1), span()).unwrap();
    let parts = ForParts {
        from: unnarrowed,
        ..for_parts(f.place(f.int), Const::I32(1), vec![])
    };
    assert!(is_internal_error_naming(
        f.b.for_(scope, parts, span()),
        "Stmt::for_"
    ));
}

/// REQ-LOW-ir-068: a `For` expands into an `Assign` of the first value and a
/// `Loop` that leaves past the bound, runs the body, and adds the step,
/// wrapped to the control's storage. The body's `Continue` goes on to the
/// step.
#[test]
fn expand_for_when_for_then_assign_and_loop_of_same_meaning() {
    let mut f = fixture();
    let scope = f.b.open_loop();
    let body = vec![scope.continue_(span())];
    let parts = for_parts(f.place(f.int), Const::I32(-1), body);
    let stmt = f.b.for_(scope, parts, span()).unwrap();
    let printed: Vec<String> =
        f.b.expand_for(stmt)
            .unwrap()
            .iter()
            .map(ToString::to_string)
            .collect();
    assert_eq!(
        printed,
        [
            "assign v1 (const i32s 1)",
            "loop loop0\n  if (ge (read i32s v1) (const i32s 10))\n  else\n    exit loop0\n  end\n  continue loop0\ncontinuing\n  assign v1 (truncate 16 (add i32s (read i32s v1) (const i32s -1)))\nend",
        ]
    );
}

#[test]
fn expand_for_when_not_for_then_statement_unchanged() {
    let f = fixture();
    let printed: Vec<String> =
        f.b.expand_for(Stmt::return_(span()))
            .unwrap()
            .iter()
            .map(ToString::to_string)
            .collect();
    assert_eq!(printed, ["return"]);
}

#[test]
fn enumeration_ordinal_when_explicit_values_then_ordinal_at_position() {
    let mut b = ProgramBuilder::new();
    let value = |text: &str, ordinal| EnumerationValue {
        name: name(text),
        ordinal,
    };
    let underlying = b
        .push_type(
            entry(TypeKind::Integer {
                width: IntWidth::B16,
                signed: true,
            }),
            &span(),
        )
        .unwrap();
    let ty = b
        .push_type(
            entry(TypeKind::Enumeration {
                underlying,
                values: vec![value("RED", 0), value("GREEN", 5), value("BLUE", 6)],
            }),
            &span(),
        )
        .unwrap();
    assert_eq!(b.types().enumeration_ordinal(ty, 1), Some(5));
    assert_eq!(b.types().enumeration_ordinal(ty, 3), None);
}

/// REQ-LOW-lowering-067: the control of a `For` may be what a reference
/// variable refers to, as a function's `VAR_OUTPUT` parameter is.
#[test]
fn for_when_control_what_reference_refers_to_then_counts_it() {
    let mut f = fixture();
    let scope = f.b.open_loop();
    let control = f.b.deref(f.place(f.reference), &span()).unwrap();
    let parts = for_parts(control, Const::I32(1), vec![]);
    let stmt = f.b.for_(scope, parts, span()).unwrap();
    assert_eq!(
        stmt.to_string(),
        "for loop0 v7^ from (const i32s 1) to (const i32s 10) by 1\nend"
    );
}
