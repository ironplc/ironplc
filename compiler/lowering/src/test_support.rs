//! Analyzes source and lowers the top-level statements of one of its bodies,
//! or the whole of it.

use ironplc_analyzer::stages::analyze;
use ironplc_analyzer::xform_resolve_initial_values;
use ironplc_analyzer::CleanAnalysis;
use ironplc_dsl::common::{FunctionBlockBodyKind, Library, LibraryElementKind};
use ironplc_dsl::core::FileId;
use ironplc_dsl::textual::StmtKind;
use ironplc_parser::options::CompilerOptions;

use ironplc_ir::lowered::{NonNumeric, ParseFailure, ParsePolicy, TypeTable, Variable};

use crate::{Body, Lowering, PartialProgram, StatementLowering};

/// What lowering made of each top-level statement of one body: the text of
/// the statements it lowered to, one per line, or `unsupported`, and the
/// variables and types lowering allocated.
pub(crate) struct Outcome {
    pub(crate) statements: Vec<String>,
    pub(crate) variables: Vec<Variable>,
    /// The types lowering allocated, which the variables' types are in.
    pub(crate) types: TypeTable,
}

/// Lowers each top-level statement of the program, function block or
/// function named `pou` in `source`, in order, sharing one [`Lowering`].
pub(crate) fn lower(source: &str, pou: &str) -> Outcome {
    lower_bodies(source, &[pou])
}

/// Lowers the top-level statements of each POU of `pous`, in order, with one
/// [`Lowering`], so that their variables share one table. A method is named
/// `Block.Method`. The source may use `AND_THEN`, `OR_ELSE`, `CONTINUE`,
/// explicit enumeration values and base types, `REF_TO`, partial access
/// (`x.%B1`), `SIZEOF`, methods, an integer literal where a bit string is
/// expected and an expression in a member initializer.
pub(crate) fn lower_bodies(source: &str, pous: &[&str]) -> Outcome {
    let options = options();
    let library = ironplc_parser::parse_program(source, &FileId::from_string("test.st"), &options)
        .expect("the source parses");
    let (library, context) = analyze(&[&library], &options).expect("the source analyzes");
    let analysis = CleanAnalysis::new(&library, &context).expect("analysis reports nothing");
    let mut lowering = Lowering::new(&analysis, parse_policy());
    let mut statements = Vec::new();
    for pou in pous {
        let (body, stmts) = body_of(&library, pou);
        for stmt in stmts {
            let text = match lowering.lower_statement(body, stmt) {
                Ok(StatementLowering::Lowered(lowered)) => lowered
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("\n"),
                Ok(StatementLowering::Unsupported) => "unsupported".to_string(),
                Err(diagnostic) => panic!("lowering reported {diagnostic:?}"),
            };
            statements.push(text);
        }
    }
    let variables = lowering
        .variables()
        .iter()
        .map(|(_, variable)| variable.clone())
        .collect();
    Outcome {
        statements,
        variables,
        types: lowering.types().clone(),
    }
}

/// Lowers the whole of `source`, with the options [`lower_bodies`] uses,
/// from its library with every initializer completed.
pub(crate) fn lower_whole(source: &str) -> PartialProgram {
    let options = options();
    let library = ironplc_parser::parse_program(source, &FileId::from_string("test.st"), &options)
        .expect("the source parses");
    let (library, context) = analyze(&[&library], &options).expect("the source analyzes");
    // Codegen hands lowering an analysis of a copy whose initializers are
    // completed (specs/design/initial-values.md), as `init` reads them.
    let completed = xform_resolve_initial_values::apply(library, &context);
    let analysis = CleanAnalysis::new(&completed, &context).expect("analysis reports nothing");
    Lowering::new(&analysis, parse_policy())
        .lower_program()
        .unwrap_or_else(|diagnostics| panic!("lowering reported {diagnostics:?}"))
}

/// The options the source of a test is parsed and analyzed with.
fn options() -> CompilerOptions {
    CompilerOptions {
        allow_short_circuit_operators: true,
        allow_continue: true,
        allow_enum_explicit_values: true,
        allow_enum_base_type: true,
        allow_ref_to: true,
        allow_partial_access_syntax: true,
        allow_sizeof: true,
        allow_fb_inheritance: true,
        allow_int_literal_to_bit_string: true,
        allow_struct_initializer_expressions: true,
        ..CompilerOptions::default()
    }
}

/// How a test's `STRING_TO_*` conversions read their string.
fn parse_policy() -> ParsePolicy {
    ParsePolicy {
        non_numeric: NonNumeric::Reject,
        failure: ParseFailure::Trap,
    }
}

fn body_of<'a>(library: &'a Library, pou: &str) -> (Body<'a>, &'a [StmtKind]) {
    if let Some((block_name, method_name)) = pou.split_once('.') {
        let (Body::FunctionBlock(block), _) = body_of(library, block_name) else {
            panic!("no function block {block_name}");
        };
        let method = block
            .methods
            .iter()
            .find(|method| method.name.to_string() == method_name)
            .expect("the block declares the method");
        return (Body::Method { block, method }, &method.body);
    }
    for element in &library.elements {
        match element {
            LibraryElementKind::ProgramDeclaration(program) if program.name.to_string() == pou => {
                return (Body::Program(program), statements(&program.body));
            }
            LibraryElementKind::FunctionBlockDeclaration(block)
                if block.name.name.to_string() == pou =>
            {
                return (Body::FunctionBlock(block), statements(&block.body));
            }
            LibraryElementKind::FunctionDeclaration(function)
                if function.name.to_string() == pou =>
            {
                return (Body::Function(function), &function.body);
            }
            LibraryElementKind::DataTypeDeclaration(_)
            | LibraryElementKind::FunctionDeclaration(_)
            | LibraryElementKind::FunctionBlockDeclaration(_)
            | LibraryElementKind::ProgramDeclaration(_)
            | LibraryElementKind::ConfigurationDeclaration(_)
            | LibraryElementKind::GlobalVarDeclarations(_)
            | LibraryElementKind::InterfaceDeclaration(_) => {}
        }
    }
    panic!("no POU {pou}");
}

fn statements(body: &FunctionBlockBodyKind) -> &[StmtKind] {
    match body {
        FunctionBlockBodyKind::Statements(statements) => &statements.body,
        FunctionBlockBodyKind::Sfc(_) | FunctionBlockBodyKind::Empty => &[],
    }
}
