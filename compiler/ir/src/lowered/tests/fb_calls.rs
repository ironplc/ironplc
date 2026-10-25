//! Tests of the `FbCall` statement's constructor and text form.

use super::*;

/// A program with a function block `Counter`, whose one field is a `DINT`,
/// and a variable `c` of it.
struct Block {
    b: ProgramBuilder,
    body: PouId,
    instance: VarId,
    dint: VarId,
}

fn block() -> Block {
    let mut b = ProgramBuilder::new();
    let dint = b
        .push_type(
            entry(TypeKind::Integer {
                width: IntWidth::B32,
                signed: true,
            }),
            &span(),
        )
        .unwrap();
    let main = b.push_pou(name("main"));
    let body = b.push_pou(name("Counter"));
    let counter = b
        .push_type(
            entry(TypeKind::FunctionBlock {
                fields: vec![Field::new("n", span(), Some(Section::Input), dint)],
                body,
                base: None,
            }),
            &span(),
        )
        .unwrap();
    let instance = b
        .push_variable(name("c"), Owner::Pou(main), counter)
        .unwrap();
    let dint = b.push_variable(name("d"), Owner::Pou(main), dint).unwrap();
    Block {
        b,
        body,
        instance,
        dint,
    }
}

/// REQ-LOW-ir-171: a call of an instance runs the body of its type.
#[test]
fn fb_call_when_instance_of_block_then_runs_its_body() {
    let block = block();
    let instance = block.b.var(block.instance, &span()).unwrap();
    let stmt = block.b.fb_call(instance, span()).unwrap();
    assert!(matches!(
        stmt.kind(),
        StmtKind::FbCall { block: called, .. } if *called == block.body
    ));
    assert_eq!(stmt.to_string(), "fb-call v0 pou1");
}

/// REQ-LOW-ir-171: only an instance of a function block is called.
#[test]
fn fb_call_when_place_not_block_then_internal_error() {
    let block = block();
    let place = block.b.var(block.dint, &span()).unwrap();
    assert!(is_internal_error_naming(
        block.b.fb_call(place, span()),
        "Stmt::fb_call"
    ));
}

/// A call names the instance it runs on, which an enclosing statement
/// prints indented with its other statements.
#[test]
fn fb_call_when_inside_if_then_printed_in_its_block() {
    let block = block();
    let instance = block.b.var(block.instance, &span()).unwrap();
    let call = block.b.fb_call(instance, span()).unwrap();
    let stmt = Stmt::if_(Expr::boolean(true, span()), vec![call], vec![], span()).unwrap();
    assert_eq!(
        stmt.to_string(),
        "if (const bool true)\n  fb-call v0 pou1\nend"
    );
}
