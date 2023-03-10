use std::{fs::File, io::Read, path::PathBuf};

use clap::Parser;
use codespan_reporting::{
    files::SimpleFile,
    term::{
        self,
        termcolor::{ColorChoice, StandardStream},
    },
};
use ironplcc::analyze;

#[derive(Parser, Debug)]
#[command(name = "ironplcc", about = "IronPLC compiler")]
struct Args {
    #[arg(
        long = "check",
        help = "Run checks and lints.",
        conflicts_with_all = &["lsp"],
    )]
    check: bool,

    #[arg(
        id = "files",
        value_name = "FILE",
        help = "Files to evaluate.",
        conflicts_with_all = &["lsp"],
    )]
    files: Vec<PathBuf>,
}

pub fn main() -> Result<(), String> {
    let args = Args::parse();

    for filename in args.files {
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
                let file = SimpleFile::new(filename.display().to_string(), contents);
                term::emit(&mut writer.lock(), &config, &file, &diagnostic)
                    .map_err(|_| "Failed writing to terminal")?;
            }
        }
    }

    Ok(())
}
