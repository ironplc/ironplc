//! Tests of the route a statement takes: from the AST, or from the lowered
//! program (`specs/design/lowered-program.md`, "Delivery Constraints").

use ironplc_analyzer::CleanAnalysis;
use ironplc_codegen::{compile_with_route_counts, CodegenOptions, EmptyLookup, Route, RouteCounts};
use ironplc_parser::options::CompilerOptions;

use crate::common::parse;

const SOURCE: &str = "
PROGRAM main
  VAR a : DINT; s : STRING; END_VAR
  a := a + 1;
  s := 'x';
  a := a * 2;
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
    assert_eq!((counts.lowered, counts.from_ast), (2, 1));
}

#[test]
fn compile_when_require_lowered_and_statement_unsupported_then_not_implemented() {
    assert_eq!(
        route_counts(SOURCE, Route::RequireLowered),
        Err("P9999".to_string())
    );
}
