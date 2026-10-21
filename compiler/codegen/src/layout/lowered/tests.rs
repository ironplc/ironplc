//! Fault-injection tests for the layout comparison (REQ-LOW-codegen-198).

use ironplc_analyzer::CleanAnalysis;
use ironplc_container::debug_section::StringLayoutEntry;
use ironplc_container::{ContainerBuilder, VarIndex};
use ironplc_dsl::core::{FileId, Id};
use ironplc_lowering::{Lowering, PartialProgram};
use ironplc_parser::options::CompilerOptions;
use rstest::rstest;

use super::check;
use crate::compile::{CodegenOptions, CompileContext};

/// Build the AST layout independently of the lowered layout, as compilation
/// does, so corrupting the former exercises the real comparison.
fn layouts() -> (PartialProgram, CompileContext, ContainerBuilder, Id) {
    let source = "
TYPE Text : STRING[11]; END_TYPE
PROGRAM main
VAR
    first : STRING[7];
    wide : WSTRING[4];
    letters : ARRAY[1..2] OF STRING[3];
    last : Text;
END_VAR
END_PROGRAM
";
    let options = CompilerOptions::default();
    let library = ironplc_parser::parse_program(source, &FileId::default(), &options).unwrap();
    let (library, context) = ironplc_analyzer::stages::analyze(&[&library], &options).unwrap();
    let library = ironplc_analyzer::xform_resolve_initial_values::apply(library, &context);
    let analysis = CleanAnalysis::new(&library, &context).unwrap();
    let codegen = CodegenOptions::from(&options);
    let lowering = Lowering::new(
        &analysis,
        crate::lowered::parse_policy(&codegen.string_to_num),
    )
    .lower_program()
    .unwrap();
    let program = crate::execution::runnable(&analysis).unwrap().program;
    let mut ctx = CompileContext::new();
    let mut builder = ContainerBuilder::new();
    crate::layout::lay_out_program(&mut ctx, &mut builder, program, context.types()).unwrap();
    (lowering, ctx, builder, program.name.clone())
}

#[test]
fn check_when_string_debug_layouts_match_then_accepts_narrow_wide_and_alias_strings() {
    let (lowering, ctx, builder, program) = layouts();
    assert_eq!(
        ctx.debug_string_layouts,
        [
            StringLayoutEntry {
                var_index: VarIndex::new(0),
                data_offset: 0,
                max_length: 7,
            },
            StringLayoutEntry {
                var_index: VarIndex::new(1),
                data_offset: 13,
                max_length: 4,
            },
            StringLayoutEntry {
                var_index: VarIndex::new(3),
                data_offset: 45,
                max_length: 11,
            },
        ]
    );
    assert!(check(&lowering, &ctx, &builder, &program).is_ok());
}

#[rstest]
#[case::slot(|entries: &mut Vec<StringLayoutEntry>| entries[0].var_index = VarIndex::new(42))]
#[case::offset(|entries: &mut Vec<StringLayoutEntry>| entries[0].data_offset += 1)]
#[case::capacity(|entries: &mut Vec<StringLayoutEntry>| entries[0].max_length += 1)]
#[case::missing(|entries: &mut Vec<StringLayoutEntry>| entries.clear())]
#[case::extra(|entries: &mut Vec<StringLayoutEntry>| entries.push(entries[0]))]
#[case::order(|entries: &mut Vec<StringLayoutEntry>| entries.swap(0, 1))]
fn check_when_string_debug_layout_differs_then_reports_internal_error(
    #[case] corrupt: fn(&mut Vec<StringLayoutEntry>),
) {
    let (lowering, mut ctx, builder, program) = layouts();
    corrupt(&mut ctx.debug_string_layouts);
    let diagnostic = check(&lowering, &ctx, &builder, &program).unwrap_err();
    assert_eq!(diagnostic.code, "P9998");
    assert!(diagnostic.primary.message.contains("debug string layouts"));
}
