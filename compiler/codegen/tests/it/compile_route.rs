//! Tests of the route a statement takes: from the AST, or from the lowered
//! program (`specs/design/lowered-program.md`, "Delivery Constraints").

use ironplc_analyzer::CleanAnalysis;
use ironplc_codegen::{compile_with_route_counts, CodegenOptions, EmptyLookup, Route, RouteCounts};
use ironplc_parser::options::CompilerOptions;

use crate::common::{bc, compile_by_route, parse, Snapshot};

/// Three statements lowering supports, the function's and two of the
/// program's, and one it does not yet: a function block's string declared
/// through a type alias, which the route that reads the AST compiles.
const SOURCE: &str = "
TYPE Name : STRING[20]; END_TYPE
FUNCTION_BLOCK B
  VAR s : Name; END_VAR
  s := 'abc';
END_FUNCTION_BLOCK
FUNCTION One : DINT
  One := 1;
END_FUNCTION
PROGRAM main
  VAR a : DINT; f : B; END_VAR
  a := a + One();
  f();
END_PROGRAM
";

/// Compiles `source` with every statement taking `route`, and counts the
/// statements each route compiled.
fn route_counts(source: &str, route: Route) -> Result<RouteCounts, String> {
    let options = CompilerOptions::default();
    let (library, context) = parse(source, &options);
    let analysis = CleanAnalysis::new(&library, &context).expect("analysis reported diagnostics");
    let codegen_options = CodegenOptions {
        route,
        ..CodegenOptions::from(&options)
    };
    compile_with_route_counts(analysis, &codegen_options, &EmptyLookup)
        .map(|(_, counts)| counts)
        .map_err(|diagnostic| diagnostic.code)
}

#[test]
fn compile_when_ast_route_then_every_statement_from_ast() {
    let counts = route_counts(SOURCE, Route::Ast).unwrap();
    assert_eq!((counts.lowered, counts.from_ast), (0, 0));
}

#[test]
fn compile_when_prefer_lowered_then_unsupported_statement_from_ast() {
    let counts = route_counts(SOURCE, Route::PreferLowered).unwrap();
    assert_eq!((counts.lowered, counts.from_ast), (3, 1));
}

#[test]
fn compile_when_require_lowered_and_statement_unsupported_then_not_implemented() {
    assert_eq!(
        route_counts(SOURCE, Route::RequireLowered),
        Err("P9999".to_string())
    );
}

/// Every control-flow statement takes the lowered route, and runs as the
/// source says.
#[test]
fn compile_when_require_lowered_and_control_flow_then_runs() {
    let source = "
TYPE COLOR : (RED, GREEN, BLUE); END_TYPE
PROGRAM main
  VAR i : DINT; s : DINT; c : DINT; w : DINT; r : DINT; k : INT; n : DINT;
      e : COLOR; ce : DINT; END_VAR
  FOR i := 1 TO 10 DO
    IF i = 3 THEN CONTINUE; END_IF;
    IF i > 6 THEN EXIT; END_IF;
    s := s + i;
  END_FOR;
  CASE s OF
    1..10: c := 1;
    18, 20: c := 2;
  ELSE
    c := 3;
  END_CASE;
  WHILE w < 5 DO w := w + 2; END_WHILE;
  REPEAT r := r + 3; UNTIL r > 7 END_REPEAT;
  FOR k := 10 TO 1 BY -3 DO n := n + 1; END_FOR;
  e := BLUE;
  CASE e OF RED: ce := 1; BLUE: ce := 2; END_CASE;
END_PROGRAM
";
    let options = CompilerOptions {
        allow_continue: true,
        ..CompilerOptions::default()
    };
    let (library, context) = parse(source, &options);
    let container = compile_by_route(&library, &context, &options, Route::RequireLowered).unwrap();
    let snapshot = Snapshot::run_compiled(container);
    let read = |name| snapshot.read_as::<i64>(name);
    assert_eq!(read("s"), 18);
    assert_eq!(read("c"), 2);
    assert_eq!(read("w"), 6);
    assert_eq!(read("r"), 9);
    assert_eq!(read("n"), 4);
    assert_eq!(read("ce"), 2);
}

/// REQ-LOW-codegen-124: a lowered `CASE` evaluates its selector once, and
/// tests each label against a copy of it.
#[test]
fn compile_when_case_lowered_then_selector_evaluated_once() {
    let source = "
PROGRAM main
  VAR x : DINT; y : DINT; END_VAR
  CASE x OF
    1, 2: y := 10;
  ELSE
    y := 20;
  END_CASE;
END_PROGRAM
";
    let options = CompilerOptions::default();
    let (library, context) = parse(source, &options);
    let container = compile_by_route(&library, &context, &options, Route::RequireLowered).unwrap();
    let bytecode = container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap();
    assert_bytecode!(
        bytecode,
        [
            bc::load_var_i32(0), // the selector, once
            bc::dup(),
            bc::load_const_i32(0), // 1
            bc::eq_i32(),
            bc::jmp_if_not(3), // to the next label
            bc::jmp(8),        // to the arm
            bc::dup(),
            bc::load_const_i32(1), // 2
            bc::eq_i32(),
            bc::jmp_if_not(10), // to the next arm
            bc::pop(),          // the arm
            bc::load_const_i32(2),
            bc::store_var_i32(1),
            bc::jmp(7), // to the end
            bc::pop(),  // the ELSE
            bc::load_const_i32(3),
            bc::store_var_i32(1),
            bc::ret_void(),
        ]
    );
}
