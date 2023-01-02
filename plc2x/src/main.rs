use std::{fs::File, io::Read};

use clap::Parser;
use stages::{parse, semantic};

extern crate ironplc_dsl;
extern crate ironplc_parser;

mod rule_constant_vars_initialized;
mod rule_enumeration_values_unique;
mod rule_pous_no_cycles;
mod rule_program_task_definition_exists;
mod rule_use_declared_enumerated_value;
mod rule_use_declared_fb;
mod rule_use_declared_symbolic_var;
mod stages;
mod symbol_table;
mod xform_resolve_late_bound_types;

#[cfg(test)]
mod test_helpers;

#[derive(Parser, Debug)]
struct Args {
    file: String,
}

pub fn main() -> std::io::Result<()> {
    let args = Args::parse();

    let mut file = File::open(args.file)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;

    let library = parse(contents.as_str()).unwrap();
    semantic(&library).unwrap();

    Ok(())
}
