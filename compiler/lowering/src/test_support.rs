//! Analyzes source and lowers the top-level statements of one of its bodies.

use ironplc_analyzer::stages::analyze;
use ironplc_analyzer::CleanAnalysis;
use ironplc_dsl::common::{FunctionBlockBodyKind, Library, LibraryElementKind};
use ironplc_dsl::core::FileId;
use ironplc_dsl::textual::StmtKind;
use ironplc_parser::options::CompilerOptions;

use ironplc_ir::lowered::{NonNumeric, ParseFailure, ParsePolicy, Variable};

use crate::{Body, Lowering, StatementLowering};

/// What lowering made of each top-level statement of one body: the text of
/// the statements it lowered to, one per line, or `unsupported`, and the
/// variables lowering allocated.
pub(crate) struct Outcome {
    pub(crate) statements: Vec<String>,
    pub(crate) variables: Vec<Variable>,
}

/// Lowers each top-level statement of the program, function block or
/// function named `pou` in `source`, in order, sharing one [`Lowering`].
pub(crate) fn lower(source: &str, pou: &str) -> Outcome {
    lower_bodies(source, &[pou])
}

/// Lowers the top-level statements of each POU of `pous`, in order, with one
/// [`Lowering`], so that their variables share one table. A method is named
/// `Block.Method`. The source may use `AND_THEN`, `OR_ELSE`, `CONTINUE`,
/// explicit enumeration values, `REF_TO`, partial access (`x.%B1`),
/// `SIZEOF` and methods.
pub(crate) fn lower_bodies(source: &str, pous: &[&str]) -> Outcome {
    let options = CompilerOptions {
        allow_short_circuit_operators: true,
        allow_continue: true,
        allow_enum_explicit_values: true,
        allow_ref_to: true,
        allow_partial_access_syntax: true,
        allow_sizeof: true,
        allow_fb_inheritance: true,
        ..CompilerOptions::default()
    };
    let library = ironplc_parser::parse_program(source, &FileId::from_string("test.st"), &options)
        .expect("the source parses");
    let (library, context) = analyze(&[&library], &options).expect("the source analyzes");
    let analysis = CleanAnalysis::new(&library, &context).expect("analysis reports nothing");
    let parse = ParsePolicy {
        non_numeric: NonNumeric::Reject,
        failure: ParseFailure::Trap,
    };
    let mut lowering = Lowering::new(&analysis, parse);
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
