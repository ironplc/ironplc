use std::{fs::File, io::Read};

use clap::Parser;
use codespan_reporting::{
    diagnostic::Diagnostic,
    files::SimpleFile,
    term::{
        self,
        termcolor::{ColorChoice, StandardStream},
    },
};
use stages::{parse, semantic};

extern crate ironplc_dsl;
extern crate ironplc_parser;

mod error;
mod rule_decl_struct_element_unique_names;
mod rule_decl_subrange_limits;
mod rule_enumeration_values_unique;
mod rule_pous_no_cycles;
mod rule_program_task_definition_exists;
mod rule_use_declared_enumerated_value;
mod rule_use_declared_fb;
mod rule_use_declared_symbolic_var;
mod rule_var_decl_const_initialized;
mod rule_var_decl_const_not_fb;
mod rule_var_decl_global_const_requires_external_const;
mod stages;
mod symbol_table;
mod xform_resolve_late_bound_types;

#[cfg(test)]
mod test_helpers;

#[derive(Parser, Debug)]
struct Args {
    file: String,
}

pub fn main() -> Result<(), String> {
    let args = Args::parse();

    let filename = args.file;
    let mut file = File::open(filename.clone()).map_err(|_| "Failed opening file")?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)
        .map_err(|_| "Failed to reach file")?;

    let writer = StandardStream::stderr(ColorChoice::Always);
    let config = codespan_reporting::term::Config::default();

    match analyze(&contents) {
        Ok(_) => {
            println!("OK");
        }
        Err(diagnostic) => {
            let file = SimpleFile::new(filename, contents);
            term::emit(&mut writer.lock(), &config, &file, &diagnostic)
                .map_err(|_| "Failed writing to terminal")?;
        }
    }

    Ok(())
}

fn analyze(contents: &str) -> Result<(), Diagnostic<()>> {
    let library = parse(contents)?;
    semantic(&library)
}

#[cfg(test)]
mod test {
    use crate::{analyze, test_helpers::read_resource};

    #[test]
    fn analyze_when_first_steps_then_result_is_ok() {
        let src = read_resource("first_steps.st");
        let res = analyze(&src);
        assert!(res.is_ok())
    }

    #[test]
    fn analyze_when_first_steps_syntax_error_then_result_is_err() {
        let src = read_resource("first_steps_syntax_error.st");
        let res = analyze(&src);
        assert!(res.is_err())
    }

    #[test]
    fn analyze_when_first_steps_semantic_error_then_result_is_err() {
        let src = read_resource("first_steps_semantic_error.st");
        let res = analyze(&src);
        assert!(res.is_err())
    }
}
