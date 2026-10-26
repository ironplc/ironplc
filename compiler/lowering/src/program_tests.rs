//! Tests of lowering a whole program: what it declares, the text of the
//! program it makes, and what it reports when a part does not lower yet.

use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_ir::lowered::{Declaration, Owner, PouId, Section, StandardFunctionBlock};

use crate::test_support::lower_whole;
use crate::PartialProgram;

/// One task running the instance `inst` of the program `main`.
const CONFIGURATION: &str = "
CONFIGURATION config
  VAR_GLOBAL g : DINT; END_VAR
  RESOURCE res ON PLC
    TASK t(INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM inst WITH t : main;
  END_RESOURCE
END_CONFIGURATION
";

fn whole(source: &str) -> PartialProgram {
    lower_whole(&format!("{source}{CONFIGURATION}"))
}

/// The text each P9999 `program` gives points at in `source`.
fn unsupported_at(program: PartialProgram, source: &str) -> Vec<String> {
    let diagnostics: Vec<Diagnostic> = program
        .into_program()
        .expect_err("the program does not lower whole");
    diagnostics
        .iter()
        .map(|diagnostic| {
            assert_eq!(diagnostic.code, "P9999");
            let at = &diagnostic.primary.location;
            source[at.start..at.end].to_string()
        })
        .collect()
}

/// The POU named `name`.
fn pou(program: &PartialProgram, name: &str) -> PouId {
    let pous = program.pous();
    pous.ids()
        .find(|pou| pous.name(*pou).is_some_and(|n| n.to_string() == name))
        .unwrap_or_else(|| panic!("no POU {name}"))
}

/// REQ-LOW-lowering-001, REQ-LOW-lowering-193: a program, a function, a
/// block and a method lower with every declaration and body, and the
/// execution model bound to them. The POUs are in the library's order once
/// analysis has sorted it, each POU after those it uses.
#[test]
fn lower_program_when_program_function_block_and_method_then_prints_whole() {
    let program = whole(
        "
FUNCTION Twice : INT
  VAR_INPUT x : INT; END_VAR
  Twice := x * 2;
END_FUNCTION

FUNCTION_BLOCK Counter
  VAR_INPUT delta : INT; END_VAR
  VAR count : INT; END_VAR
  count := count + delta;
  METHOD Bump : INT
    VAR_INPUT n : INT; END_VAR
    VAR_TEMP t : INT; END_VAR
    t := n;
    count := count + t;
    Bump := count;
  END_METHOD
END_FUNCTION_BLOCK

PROGRAM main
  VAR_EXTERNAL g : DINT; END_VAR
  VAR c : Counter; y : INT; END_VAR
  y := c.Bump(n := Twice(y));
  g := g + 1;
END_PROGRAM
",
    );
    assert!(program.declarations_complete());

    assert_eq!(
        program.into_program().unwrap().to_string(),
        "types
  t0 \"dint\" i32
  t1 \"int\" i16
  t2 \"Counter\" function-block pou0
    f0 delta : t1 input
    f1 count : t1 local
  t3 ref non-null t2
  t4 ref non-null t2
  t5 \"main\" program pou3
    f0 c : t2 local
    f1 y : t1 local
  t6 ref non-null t5
variables
  v0 g : t0 global
  v1 THIS : t3 instance of pou0
  v2 THIS : t4 instance of pou1
  v3 n : t1 input of pou1
  v4 t : t1 temporary of pou1
  v5 Bump : t1 output of pou1
  v6 x : t1 input of pou2
  v7 Twice : t1 output of pou2
  v8 THIS : t6 instance of pou3
  v9 inst : t5 instance
pou0 function-block Counter instance v1
  assign v1^.f1 (truncate 16 (add i32s (read i32s v1^.f1) (read i32s v1^.f0)))
end
pou1 method Bump of pou0 instance v2 parameters v2, v3 result v5 locals v4
  assign v4 (const i32s 0)
  assign v5 (const i32s 0)
  assign v4 (read i32s v3)
  assign v2^.f1 (truncate 16 (add i32s (read i32s v2^.f1) (read i32s v4)))
  assign v5 (read i32s v2^.f1)
end
pou2 function Twice parameters v6 result v7
  assign v7 (const i32s 0)
  assign v7 (truncate 16 (mul i32s (read i32s v6) (const i32s 2)))
end
pou3 program main instance v8
  assign v8^.f1 (truncate 16 (call pou1 i32s (ref v8^.f0) (call pou2 i32s (read i32s v8^.f1))))
  assign v0 (add i32s (read i32s v0) (const i32s 1))
end
init
  assign v0 (const i32s 0)
  assign v9.f0.f0 (const i32s 0)
  assign v9.f0.f1 (const i32s 0)
  assign v9.f1 (const i32s 0)
end
program main pou3
global g v0
instance inst v9
"
    );
}

/// REQ-LOW-lowering-025: a POU no program instance reaches is not lowered.
#[test]
fn lower_program_when_pou_unreachable_then_absent() {
    let program = whole(
        "
FUNCTION Unused : INT
  Unused := 1;
END_FUNCTION
PROGRAM main
  VAR y : INT; END_VAR
  y := 1;
END_PROGRAM
",
    );

    let names: Vec<String> = program
        .pous()
        .ids()
        .filter_map(|pou| program.pous().name(pou).map(ToString::to_string))
        .collect();
    assert_eq!(names, ["main"]);
}

/// REQ-LOW-lowering-193: a declaration no statement names still has its
/// variable, in declaration order within its POU, and the POUs in the
/// library's sorted order.
#[test]
fn lower_program_when_variable_unused_then_present() {
    let program = whole(
        "
FUNCTION F : INT
  VAR_INPUT x : INT; END_VAR
  VAR unused : INT; also : BOOL; END_VAR
  F := x;
END_FUNCTION
FUNCTION_BLOCK Fb
  VAR_TEMP scratch : INT; END_VAR
  VAR v : INT; END_VAR
  v := 1;
END_FUNCTION_BLOCK
PROGRAM main
  VAR y : INT; b : Fb; END_VAR
  y := F(y);
  b();
END_PROGRAM
",
    );

    let owned: Vec<(String, Section)> = program
        .variables()
        .iter()
        .filter(|(_, variable)| variable.owner() != Owner::Global)
        .filter(|(_, variable)| variable.section() != Section::Instance)
        .map(|(_, variable)| (variable.name().to_string(), variable.section()))
        .collect();
    assert_eq!(
        owned,
        [
            ("scratch".to_string(), Section::Temporary),
            ("x".to_string(), Section::Input),
            ("unused".to_string(), Section::Local),
            ("also".to_string(), Section::Local),
            ("F".to_string(), Section::Output),
        ]
    );
}

/// REQ-LOW-lowering-170: a block's fields are its declarations other than
/// a `VAR_TEMP`, in the order it declares them.
#[test]
fn lower_program_when_block_declares_fields_then_in_declaration_order() {
    let program = whole(
        "
FUNCTION_BLOCK Fb
  VAR_OUTPUT q : INT; END_VAR
  VAR v : INT; END_VAR
  VAR_TEMP t : INT; END_VAR
  VAR_INPUT i : INT; END_VAR
  q := i + v + t;
END_FUNCTION_BLOCK
PROGRAM main
  VAR b : Fb; END_VAR
  b(i := 1);
END_PROGRAM
",
    );

    let fb = program
        .types()
        .iter()
        .find(|(_, entry)| entry.name.as_ref().is_some_and(|n| n.to_string() == "Fb"))
        .map(|(id, _)| id)
        .unwrap();
    let fields: Vec<(String, Option<Section>)> = program
        .types()
        .fields(fb)
        .unwrap()
        .iter()
        .map(|field| (field.name.to_string(), field.section))
        .collect();
    assert_eq!(
        fields,
        [
            ("q".to_string(), Some(Section::Output)),
            ("v".to_string(), Some(Section::Local)),
            ("i".to_string(), Some(Section::Input)),
        ]
    );
}

/// REQ-LOW-lowering-022: every global of the execution model is one
/// variable, which each `VAR_EXTERNAL` naming it reaches.
#[test]
fn lower_program_when_two_bodies_name_global_then_one_bound_variable() {
    let program = whole(
        "
FUNCTION_BLOCK Fb
  VAR_EXTERNAL g : DINT; END_VAR
  g := 2;
END_FUNCTION_BLOCK
PROGRAM main
  VAR_EXTERNAL g : DINT; END_VAR
  VAR b : Fb; END_VAR
  g := 1;
  b();
END_PROGRAM
",
    );

    let globals: Vec<String> = program
        .variables()
        .iter()
        .filter(|(_, variable)| variable.owner() == Owner::Global)
        .map(|(_, variable)| variable.name().to_string())
        .collect();
    assert_eq!(globals, ["g", "inst"]);
    let whole = program.into_program().unwrap();
    let (id, _) = whole.execution().globals().next().unwrap();
    assert_eq!(
        whole.bindings().global(id).map(|var| var.to_string()),
        Some("v0".to_string())
    );
}

/// REQ-LOW-lowering-110, REQ-LOW-lowering-111: each statement that does not
/// lower yet is a P9999 of its own, at the statement, in every POU, and the
/// rest of each body still lowers.
#[test]
fn into_program_when_statements_in_two_pous_unsupported_then_p9999_at_each() {
    let source = "
FUNCTION F : DINT
  VAR_INPUT x : DINT; END_VAR
  VAR a : DINT; r : REAL; END_VAR
  FOR a := 1 TO 3 BY x DO r := r; END_FOR;
  F := x;
END_FUNCTION
PROGRAM main
  VAR a : DINT; r : REAL; END_VAR
  a := F(a);
  FOR a := 1 TO 3 BY a DO r := r; END_FOR;
END_PROGRAM
";
    let program = whole(source);
    let f = pou(&program, "F");
    assert!(matches!(program.statement(f, 0), Some(Err(_))));
    assert!(matches!(program.statement(f, 1), Some(Ok(_))));
    assert!(program.statement(f, 2).is_none());

    assert_eq!(
        unsupported_at(program, source),
        [
            "FOR a := 1 TO 3 BY x DO r := r; END_FOR",
            "FOR a := 1 TO 3 BY a DO r := r; END_FOR"
        ]
    );
}

/// REQ-LOW-lowering-112: a declaration lowering does not express yet leaves
/// the declarations incomplete, which is reported once at its POU, not an
/// error; a call of that POU does not lower either, and is reported at the
/// statement, whose span is its assignment's `:=`.
#[test]
fn lower_program_when_declaration_not_lowered_then_declarations_incomplete() {
    let source = "
FUNCTION F : INT
  VAR_IN_OUT s : STRING; END_VAR
  F := 1;
END_FUNCTION
PROGRAM main
  VAR a : INT; b : STRING; END_VAR
  a := F(s := b);
  a := a + 1;
END_PROGRAM
";
    let program = whole(source);
    assert!(!program.declarations_complete());
    let main = pou(&program, "main");
    assert!(matches!(program.statement(main, 0), Some(Err(_))));
    assert!(matches!(program.statement(main, 1), Some(Ok(_))));
    assert!(program.statement(pou(&program, "F"), 0).is_none());

    assert_eq!(unsupported_at(program, source), ["F", ":="]);
}

/// REQ-LOW-ir-203: a standard function block has no body in the library,
/// so its POU stands for the block, which the type of its name is, and the
/// program is whole without a body for it.
#[test]
fn into_program_when_standard_function_block_then_pou_names_block() {
    let program = whole(
        "
FUNCTION_BLOCK CTU_INT
  VAR_INPUT CU : BOOL; END_VAR
END_FUNCTION_BLOCK
PROGRAM main
  VAR timer : TON; counter : CTU_INT; END_VAR
  timer(IN := TRUE, PT := T#1s);
  counter(CU := TRUE);
END_PROGRAM
",
    );
    let ton = pou(&program, "TON");
    let user = pou(&program, "CTU_INT");
    assert_eq!(
        program.standard_block(ton),
        Some(StandardFunctionBlock::Ton)
    );
    assert_eq!(program.standard_block(user), None);

    let whole = program.into_program().unwrap();

    assert_eq!(
        whole.pous().standard_block(ton),
        Some(StandardFunctionBlock::Ton)
    );
    assert!(whole.parts(ton).is_none());
    assert!(whole.parts(user).is_some());
}

/// What each declaration of the body of the POU `name` declares, by kind
/// and name.
fn declared(program: &PartialProgram, name: &str) -> Vec<String> {
    let pou = pou(program, name);
    let variable = |var| program.variables().get(var).unwrap().name().to_string();
    let instance = program.parts(pou).unwrap().instance;
    let fields = instance.map(|this| {
        let ty = program.variables().get(this).unwrap().ty();
        let (instance_type, _) = program.types().reference(ty).unwrap();
        program.types().fields(instance_type).unwrap().to_vec()
    });
    program
        .declarations(pou)
        .unwrap()
        .iter()
        .map(|declaration| match declaration {
            Declaration::Field(field) => {
                let fields = fields.as_ref().unwrap();
                format!("field {}", fields[field.index() as usize].name)
            }
            Declaration::Variable(var) => format!("variable {}", variable(*var)),
            Declaration::External(var) => format!("external {}", variable(*var)),
        })
        .collect()
}

/// REQ-LOW-lowering-197: each body's declarations are in the order it
/// declares them: a program's fields and the global a `VAR_EXTERNAL` names,
/// and every variable of a function, a `VAR_OUTPUT` and a `VAR_IN_OUT`
/// included.
#[test]
fn declarations_when_body_declares_then_in_declaration_order() {
    let program = whole(
        "
FUNCTION F : INT
  VAR_INPUT x : INT; END_VAR
  VAR_OUTPUT o : INT; END_VAR
  VAR l : INT; END_VAR
  VAR_IN_OUT io : INT; END_VAR
  o := x;
  F := l + io;
END_FUNCTION
PROGRAM main
  VAR a : INT; END_VAR
  VAR_EXTERNAL g : DINT; END_VAR
  VAR b : INT; r : INT; END_VAR
  a := F(x := b, o => r, io := a);
  g := g + 1;
END_PROGRAM
",
    );

    assert_eq!(
        declared(&program, "main"),
        ["field a", "external g", "field b", "field r"]
    );
    assert_eq!(
        declared(&program, "F"),
        ["variable x", "variable o", "variable l", "variable io"]
    );
}

/// REQ-LOW-lowering-197: the variable that holds a result is the one a
/// function returns, and, for a method whose result is a structure, the
/// variable of its body named after it; a method that returns nothing has
/// none.
#[test]
fn result_when_function_or_method_then_variable_holding_it() {
    let program = whole(
        "
TYPE P : STRUCT x : DINT; END_STRUCT; END_TYPE
FUNCTION F : INT
  F := 1;
END_FUNCTION
FUNCTION_BLOCK B
  VAR n : DINT; END_VAR
  METHOD MKP : P
    n := n + 1;
  END_METHOD
  METHOD Bump
    n := n + F();
  END_METHOD
END_FUNCTION_BLOCK
PROGRAM main
  VAR b : B; END_VAR
  b.MKP();
  b.Bump();
END_PROGRAM
",
    );

    let result = |name| {
        program
            .result(pou(&program, name))
            .map(|var| program.variables().get(var).unwrap().name().to_string())
    };
    assert_eq!(result("F").as_deref(), Some("F"));
    assert_eq!(result("MKP").as_deref(), Some("MKP"));
    assert_eq!(result("Bump"), None);
}

/// REQ-LOW-lowering-197: a variable a declaration declares is declared; a
/// temporary lowering makes for a call's output is not.
#[test]
fn is_declared_when_temporary_for_output_then_false() {
    let program = whole(
        "
FUNCTION F : INT
  VAR_OUTPUT o : INT; END_VAR
  o := 1;
  F := 2;
END_FUNCTION
PROGRAM main
  VAR a : INT; r : INT; END_VAR
  a := F(o => r);
END_PROGRAM
",
    );

    let undeclared: Vec<Section> = program
        .variables()
        .iter()
        .filter(|(var, _)| !program.is_declared(*var))
        .map(|(_, variable)| variable.section())
        .collect();
    assert!(!undeclared.is_empty());
    assert!(undeclared
        .iter()
        .all(|section| *section == Section::Temporary));
    let f = pou(&program, "F");
    assert!(program
        .declarations(f)
        .unwrap()
        .iter()
        .all(|declaration| match declaration {
            Declaration::Variable(var) | Declaration::External(var) => program.is_declared(*var),
            Declaration::Field(_) => true,
        }));
}

/// Each top-level statement keeps its own span, which a backend's line map
/// reads; a `RETURN`, which the AST keeps no span for, has an empty one.
#[test]
fn statement_span_when_statements_then_each_its_own() {
    let source = "
FUNCTION F : DINT
  F := 1;
  RETURN;
END_FUNCTION
PROGRAM main
  VAR a : DINT; END_VAR
  a := F();
END_PROGRAM
";
    let program = whole(source);
    let f = pou(&program, "F");

    let first = program.statement_span(f, 0).unwrap();
    assert_eq!(&source[first.start..first.end], ":=");
    assert_eq!(first.start, source.find(":= 1").unwrap());
    let ret = program.statement_span(f, 1).unwrap();
    assert_eq!(ret.start, ret.end);
    assert!(program.statement_span(f, 2).is_none());
}
