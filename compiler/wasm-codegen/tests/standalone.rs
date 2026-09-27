//! The code generator as a back end of another front end
//! (`compiler/wasm-symbols/docs/backend.md`): a module of the IR built by
//! hand, without the IronPLC parser nor its semantic analysis, becomes a
//! logic module that runs.

use ironplc_wasm_codegen::{generate, CodegenOptions};
use ironplc_wasm_ir::*;
use ironplc_wasm_runner::{Plc, Value};

const I16: Scalar = Scalar::Int {
    bits: 16,
    signed: true,
};

fn span() -> Span {
    Span::default()
}

fn expr(kind: ExprKind, ty: Scalar) -> Expr {
    Expr {
        kind,
        ty,
        span: span(),
    }
}

/// `PROGRAM Main VAR n : INT; END_VAR n := n + 1; IF n > 2 THEN n := 0; END_IF; END_PROGRAM`
fn counter() -> Module {
    let n = Place {
        addr: Addr::object(0, 0),
        ty: I16,
    };
    let load = || expr(ExprKind::Load(n.clone()), I16);
    let int = |v| expr(ExprKind::Const(Const::Int(v)), I16);
    let incr = Stmt {
        kind: StmtKind::Assign(
            n.clone(),
            expr(
                ExprKind::Binary(BinaryOp::Add, Box::new(load()), Box::new(int(1)), None),
                I16,
            ),
        ),
        span: span(),
    };
    let reset = Stmt {
        kind: StmtKind::If(
            expr(
                ExprKind::Binary(BinaryOp::Gt, Box::new(load()), Box::new(int(2)), None),
                Scalar::Bool,
            ),
            vec![Stmt {
                kind: StmtKind::Assign(n.clone(), int(0)),
                span: span(),
            }],
            vec![],
        ),
        span: span(),
    };
    Module {
        objects: vec![Object {
            name: "MAIN".into(),
            region: Region::Static,
            align: 2,
            init: vec![0, 0],
        }],
        functions: vec![Function {
            name: "MAIN".into(),
            kind: FuncKind::Program,
            frame: None,
            temps: vec![],
            body: vec![incr, reset],
            span: span(),
        }],
        tasks: vec![Task {
            name: "FAST".into(),
            interval_ns: 1_000_000,
            priority: 0,
            programs: vec![(0, "MAIN".into())],
        }],
        sites: vec![span()],
        leaves: vec![Leaf {
            path: "MAIN.N".into(),
            type_name: "INT".into(),
            object: 0,
            offset: 0,
            size: 2,
            flags: 0,
            location: None,
            declared: 0,
            enumeration: None,
            array: None,
        }],
    }
}

#[test]
fn a_hand_built_module_is_generated_and_runs() {
    let m = counter();
    validate(&m).expect("valid IR");
    let opts = CodegenOptions {
        fuel: false,
        debug_hooks: false,
        bounds_checks: true,
        static_limit: 1 << 20,
        compiler: "hand-built".into(),
        files: vec![("main.st".into(), String::new())],
    };
    let out = generate(&m, &opts).expect("a logic module");
    let mut plc = Plc::new(&out.wasm).unwrap();
    plc.init().unwrap();
    let mut seen = vec![];
    for _ in 0..4 {
        plc.cycle(0, 1_000_000).unwrap();
        seen.push(plc.read("MAIN.N").unwrap());
    }
    assert_eq!(
        seen,
        [Value::Int(1), Value::Int(2), Value::Int(0), Value::Int(1)]
    );
    assert_eq!(plc.symbols().tasks[0].name, "FAST");
}

#[test]
fn the_validator_rejects_a_power_as_binary_operation() {
    let mut m = counter();
    let n = Place {
        addr: Addr::object(0, 0),
        ty: I16,
    };
    let int = |v| expr(ExprKind::Const(Const::Int(v)), I16);
    m.functions[0].body = vec![Stmt {
        kind: StmtKind::Assign(
            n,
            expr(
                ExprKind::Binary(BinaryOp::Pow, Box::new(int(2)), Box::new(int(3)), None),
                I16,
            ),
        ),
        span: span(),
    }];
    assert!(validate(&m).is_err());
}
