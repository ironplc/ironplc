use std::{
    collections::BTreeSet,
    env,
    error::Error,
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
    process,
};

fn generate_io_codes() -> Result<(), Box<dyn Error>> {
    println!("cargo:rerun-if-changed=resources/problem-codes.csv");

    let mut src_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    src_path.push("resources");
    src_path.push("problem-codes.csv");

    let src = fs::read_to_string(src_path).expect("Unable to read 'problem-codes.csv'");
    let src = src.as_bytes();

    let out_path = PathBuf::from(env::var("OUT_DIR")?).join("io_codes.rs");
    let mut out = File::create(out_path)?;

    let mut rdr = csv::Reader::from_reader(src);
    for result in rdr.records() {
        let record = result?;
        let code = record
            .get(0)
            .ok_or_else(|| format!("Record {record:?} is not valid at column 0"))?;
        let name = record
            .get(1)
            .ok_or_else(|| format!("Record {record:?} is not valid at column 1"))?;
        let message = record
            .get(2)
            .ok_or_else(|| format!("Record {record:?} is not valid at column 2"))?;

        // Convert PascalCase name to SCREAMING_SNAKE_CASE for the constant
        let const_name = pascal_to_screaming_snake(name);

        out.write_all(
            format!("/// {message}\npub const {const_name}: &str = \"{code}\";\n\n").as_bytes(),
        )?;
    }

    out.flush()?;
    Ok(())
}

/// Converts PascalCase to SCREAMING_SNAKE_CASE.
fn pascal_to_screaming_snake(s: &str) -> String {
    let mut result = String::new();
    for (i, ch) in s.chars().enumerate() {
        if ch.is_uppercase() && i > 0 {
            result.push('_');
        }
        result.push(ch.to_ascii_uppercase());
    }
    result
}

/// Generates `spec_requirements.rs` from `**REQ-VC-NNN**` markers in
/// `specs/design/vm-cli.md`. Scans `src/` and `tests/` for `spec_test(REQ_VC_`
/// references so the meta-test can flag any requirement without a test.
///
/// Mirrors the scheme in `compiler/container/build.rs`; see
/// `specs/design/spec-conformance-testing.md`.
fn generate_spec_requirements() -> Result<(), Box<dyn Error>> {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR")?;
    let spec_path = Path::new(&manifest_dir).join("../../specs/design/vm-cli.md");
    println!("cargo:rerun-if-changed={}", spec_path.display());

    let mut requirements = BTreeSet::new();
    if let Ok(content) = fs::read_to_string(&spec_path) {
        extract_requirements(&content, &mut requirements);
    }

    let mut tested = BTreeSet::new();
    for dir in ["src", "tests"] {
        let scan_dir = Path::new(&manifest_dir).join(dir);
        for path in collect_rs_files(&scan_dir) {
            println!("cargo:rerun-if-changed={}", path.display());
            if let Ok(content) = fs::read_to_string(&path) {
                extract_tested_requirements(&content, &mut tested);
            }
        }
    }

    let out_path = PathBuf::from(env::var("OUT_DIR")?).join("spec_requirements.rs");
    let mut code = String::from("// Auto-generated from specs/design/vm-cli.md — do not edit.\n\n");
    for req in &requirements {
        let ident = req.replace('-', "_");
        code.push_str(&format!(
            "#[allow(dead_code)] pub const {ident}: &str = \"{req}\";\n"
        ));
    }
    code.push('\n');
    code.push_str("#[allow(dead_code)]\npub const ALL: &[&str] = &[\n");
    for req in &requirements {
        code.push_str(&format!("    \"{req}\",\n"));
    }
    code.push_str("];\n\n");

    let untested: Vec<&String> = requirements
        .iter()
        .filter(|r| !tested.contains(&r.replace('-', "_")))
        .collect();
    code.push_str("#[allow(dead_code)]\npub const UNTESTED: &[&str] = &[\n");
    for req in &untested {
        code.push_str(&format!("    \"{req}\",\n"));
    }
    code.push_str("];\n");

    fs::write(&out_path, code)?;
    Ok(())
}

fn collect_rs_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                files.extend(collect_rs_files(&path));
            } else if path.extension().is_some_and(|e| e == "rs") {
                files.push(path);
            }
        }
    }
    files
}

/// Extracts `**REQ-XX-NNN**` bold markers from markdown content.
fn extract_requirements(content: &str, out: &mut BTreeSet<String>) {
    let mut rest = content;
    while let Some(start) = rest.find("**REQ-") {
        let after_open = &rest[start + 2..];
        if let Some(end) = after_open.find("**") {
            let id = &after_open[..end];
            if id.starts_with("REQ-") && id.len() >= 8 {
                out.insert(id.to_string());
            }
            rest = &after_open[end + 2..];
        } else {
            break;
        }
    }
}

/// Extracts requirement identifiers (underscore form) from
/// `#[spec_test(REQ_XX_NNN)]` attributes in Rust source.
fn extract_tested_requirements(content: &str, out: &mut BTreeSet<String>) {
    let mut rest = content;
    let needle = "spec_test(REQ_";
    while let Some(start) = rest.find(needle) {
        let after = &rest[start + "spec_test(".len()..];
        if let Some(end) = after.find(')') {
            let ident = &after[..end];
            if ident.starts_with("REQ_") && ident.len() >= 8 {
                out.insert(ident.to_string());
            }
            rest = &after[end + 1..];
        } else {
            break;
        }
    }
}

fn main() {
    println!(
        "cargo:rustc-env=BUILD_OPT_LEVEL={}",
        std::env::var("OPT_LEVEL").unwrap()
    );

    if let Err(err) = generate_io_codes() {
        eprintln!("problem generating io_codes.rs: {err}");
        process::exit(1);
    }
    if let Err(err) = generate_spec_requirements() {
        eprintln!("problem generating spec_requirements.rs: {err}");
        process::exit(1);
    }
}
