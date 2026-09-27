//! `ironplcc compile --target wasm`: a WebAssembly logic module and its
//! symbol map (`specs/design/wasm-target.md`).

use std::path::{Path, PathBuf};

use ironplc_parser::options::CompilerOptions;
use ironplc_problems::Problem;
use ironplc_project::Project;
use ironplc_sources::LibraryName;
use ironplc_wasm::WasmOptions;

use crate::cli::{create_project, diagnostic, finish, output_conflicts_with_source};

/// The path of the symbol map written next to a module: `out.wasm` gives
/// `out.symbols.json`.
pub fn symbols_path(output: &Path) -> PathBuf {
    output.with_extension("symbols.json")
}

/// Compiles source files into a WebAssembly module at `output`, with its
/// symbol map as JSON beside it.
pub fn compile_wasm(
    paths: &[PathBuf],
    output: &Path,
    compiler_options: CompilerOptions,
    libraries: &[LibraryName],
    options: WasmOptions,
    suppress_output: bool,
) -> Result<(), String> {
    let (mut project, mut diagnostics) = create_project(paths, compiler_options, libraries);
    for path in [output.to_path_buf(), symbols_path(output)] {
        if output_conflicts_with_source(&project, &path) {
            diagnostics.extend(diagnostic(
                Problem::OutputPathConflictsWithInput,
                &path,
                String::from("Choose an output path that is not an input source file"),
            ));
        }
    }
    let sources: Vec<_> = project
        .sources()
        .iter()
        .map(|s| (s.file_id().clone(), s.as_string().to_string()))
        .collect();
    let result = match ironplc_project::analyze(&mut project, diagnostics) {
        Ok((library, context)) => {
            ironplc_wasm::compile(library, context, &options, &sources).map_err(|d| vec![d])
        }
        Err(diagnostics) => Err(diagnostics),
    };
    let diagnostics = match result {
        Ok(out) => {
            std::fs::write(output, &out.wasm)
                .map_err(|e| format!("Failed to write output file: {e}"))?;
            std::fs::write(symbols_path(output), out.symbols.to_json_pretty())
                .map_err(|e| format!("Failed to write symbol map: {e}"))?;
            vec![]
        }
        Err(diagnostics) => diagnostics,
    };
    finish("Compile", diagnostics, Some(&project), suppress_output)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use ironplc_parser::options::CompilerOptions;
    use ironplc_wasm::WasmOptions;

    use super::{compile_wasm, symbols_path};

    fn write_source(dir: &Path, text: &str) -> std::path::PathBuf {
        let path = dir.join("main.st");
        std::fs::write(&path, text).unwrap();
        path
    }

    #[test]
    fn compile_wasm_when_valid_program_then_module_and_symbol_map() {
        let dir = tempfile::tempdir().unwrap();
        let src = write_source(
            dir.path(),
            "PROGRAM main VAR x : INT; END_VAR x := x + 1; END_PROGRAM",
        );
        let out = dir.path().join("out.wasm");
        compile_wasm(
            &[src],
            &out,
            CompilerOptions::default(),
            &[],
            WasmOptions::default(),
            true,
        )
        .unwrap();
        let wasm = std::fs::read(&out).unwrap();
        assert_eq!(&wasm[..4], b"\0asm");
        let json = std::fs::read_to_string(symbols_path(&out)).unwrap();
        assert!(json.contains("\"MAIN.X\""));
    }

    #[test]
    fn compile_wasm_when_fuel_then_module_exports_plc_fuel() {
        let dir = tempfile::tempdir().unwrap();
        let src = write_source(
            dir.path(),
            "PROGRAM main VAR x : INT; END_VAR x := 1; END_PROGRAM",
        );
        let out = dir.path().join("out.wasm");
        let options = WasmOptions {
            fuel: true,
            ..WasmOptions::default()
        };
        compile_wasm(&[src], &out, CompilerOptions::default(), &[], options, true).unwrap();
        let wasm = std::fs::read(&out).unwrap();
        let needle = b"plc_fuel";
        assert!(wasm.windows(needle.len()).any(|w| w == needle));
    }

    #[test]
    fn compile_wasm_when_semantic_error_then_no_module() {
        let dir = tempfile::tempdir().unwrap();
        let src = write_source(dir.path(), "PROGRAM main x := y; END_PROGRAM");
        let out = dir.path().join("out.wasm");
        let r = compile_wasm(
            &[src],
            &out,
            CompilerOptions::default(),
            &[],
            WasmOptions::default(),
            true,
        );
        assert!(r.is_err());
        assert!(!out.exists());
    }

    #[test]
    fn compile_wasm_when_output_is_source_then_error() {
        let dir = tempfile::tempdir().unwrap();
        let src = write_source(dir.path(), "PROGRAM main END_PROGRAM");
        let r = compile_wasm(
            std::slice::from_ref(&src),
            &src,
            CompilerOptions::default(),
            &[],
            WasmOptions::default(),
            true,
        );
        assert!(r.is_err());
    }
}
