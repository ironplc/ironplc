//! IR validator.

use ironplc_wasm_ir::*;

fn span() -> Span {
    Span::default()
}

fn module(body: Vec<Stmt>) -> Module {
    Module {
        objects: vec![Object {
            name: "o".into(),
            region: Region::Static,
            align: 4,
            init: vec![0; 8],
        }],
        functions: vec![Function {
            name: "MAIN".into(),
            kind: FuncKind::Program,
            frame: None,
            temps: vec![],
            body,
            span: span(),
        }],
        tasks: vec![],
        sites: vec![],
        leaves: vec![],
    }
}

fn stmt(kind: StmtKind) -> Stmt {
    Stmt { kind, span: span() }
}

fn int(v: i128) -> Expr {
    Expr {
        kind: ExprKind::Const(Const::Int(v)),
        ty: Scalar::Int {
            bits: 32,
            signed: true,
        },
        span: span(),
    }
}

fn place(offset: u32, ty: Scalar) -> Place {
    Place {
        addr: Addr::object(0, offset),
        ty,
    }
}

#[test]
fn arch_020_valid_module_passes() {
    let m = module(vec![stmt(StmtKind::Block(
        1,
        vec![
            stmt(StmtKind::Assign(
                place(
                    4,
                    Scalar::Int {
                        bits: 32,
                        signed: true,
                    },
                ),
                int(3),
            )),
            stmt(StmtKind::Break(1)),
        ],
    ))]);
    assert_eq!(validate(&m), Ok(()));
}

#[test]
fn arch_020_type_mismatch_is_rejected() {
    let m = module(vec![stmt(StmtKind::Assign(place(0, Scalar::Bool), int(3)))]);
    assert!(validate(&m).unwrap_err().contains("assignment"));
}

#[test]
fn arch_020_labels_out_of_scope_are_rejected() {
    let m = module(vec![stmt(StmtKind::Break(7))]);
    assert!(validate(&m).unwrap_err().contains("L7"));
    let m = module(vec![stmt(StmtKind::Block(
        1,
        vec![stmt(StmtKind::Continue(1))],
    ))]);
    assert!(validate(&m).is_err(), "continue needs a loop");
}

#[test]
fn arch_020_places_outside_objects_are_rejected() {
    let m = module(vec![stmt(StmtKind::Assign(
        place(
            6,
            Scalar::Int {
                bits: 32,
                signed: true,
            },
        ),
        int(3),
    ))]);
    assert!(validate(&m).unwrap_err().contains("outside"));
    let m = module(vec![stmt(StmtKind::Assign(
        Place {
            addr: Addr {
                base: Base::SelfPart(0),
                offset: 0,
            },
            ty: Scalar::Bool,
        },
        Expr {
            kind: ExprKind::Const(Const::Bool(true)),
            ty: Scalar::Bool,
            span: span(),
        },
    ))]);
    assert!(validate(&m).unwrap_err().contains("instance part"));
}
