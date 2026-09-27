//! The programs of the gate: the Structured Text programs written in the
//! bytecode code generator's tests, the `.st` test resources, the examples,
//! and optionally the probes of an external directory.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

/// A program and where it comes from.
pub struct Program {
    pub id: String,
    pub source: String,
}

fn compiler_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn files(dir: &Path, ext: &str) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|e| e == ext))
                .collect()
        })
        .unwrap_or_default();
    out.sort();
    out
}

fn name(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// The programs written in the Rust tests of the bytecode code generator.
pub fn codegen_programs() -> Vec<Program> {
    let mut seen = BTreeSet::new();
    let mut out = vec![];
    for path in files(&compiler_dir().join("codegen/tests/it"), "rs") {
        let text = fs::read_to_string(&path).unwrap_or_default();
        for (i, lit) in string_literals(&text).into_iter().enumerate() {
            if lit.to_uppercase().contains("END_PROGRAM") && seen.insert(lit.clone()) {
                out.push(Program {
                    id: format!("codegen/{}#{i}", name(&path)),
                    source: lit,
                });
            }
        }
    }
    out
}

/// `.st` files of a directory.
pub fn st_files(dir: &Path, prefix: &str) -> Vec<Program> {
    files(dir, "st")
        .into_iter()
        .map(|p| Program {
            id: format!("{prefix}/{}", name(&p)),
            source: fs::read_to_string(&p).unwrap_or_default(),
        })
        .filter(|p| p.source.to_uppercase().contains("END_PROGRAM"))
        .collect()
}

/// The `.st` test resources and the examples of the repository.
pub fn repository_programs() -> Vec<Program> {
    let mut out = st_files(&compiler_dir().join("resources/test"), "resources/test");
    out.extend(st_files(&compiler_dir().join("../examples"), "examples"));
    out
}

/// The string literals of Rust source text, unescaped.
pub fn string_literals(text: &str) -> Vec<String> {
    let b = text.as_bytes();
    let mut out = vec![];
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'/' if b.get(i + 1) == Some(&b'/') => {
                while i < b.len() && b[i] != b'\n' {
                    i += 1;
                }
            }
            b'\'' => {
                // A character literal such as '"' or '\''.
                if b.get(i + 1) == Some(&b'\\') {
                    i += 4;
                } else if b.get(i + 2) == Some(&b'\'') {
                    i += 3;
                } else {
                    i += 1;
                }
            }
            b'r' if matches!(b.get(i + 1), Some(b'#') | Some(b'"'))
                && (i == 0 || !b[i - 1].is_ascii_alphanumeric()) =>
            {
                let hashes = b[i + 1..].iter().take_while(|c| **c == b'#').count();
                let open = i + 1 + hashes;
                if b.get(open) != Some(&b'"') {
                    i += 1;
                    continue;
                }
                let close = format!("\"{}", "#".repeat(hashes));
                let start = open + 1;
                match text[start..].find(&close) {
                    Some(end) => {
                        out.push(text[start..start + end].to_string());
                        i = start + end + close.len();
                    }
                    None => break,
                }
            }
            b'"' => {
                let (lit, next) = escaped(text, i + 1);
                out.push(lit);
                i = next;
            }
            _ => i += 1,
        }
    }
    out
}

fn escaped(text: &str, start: usize) -> (String, usize) {
    let mut s = String::new();
    let mut chars = text[start..].char_indices().peekable();
    while let Some((k, c)) = chars.next() {
        match c {
            '"' => return (s, start + k + 1),
            '\\' => match chars.next() {
                Some((_, 'n')) => s.push('\n'),
                Some((_, 't')) => s.push('\t'),
                Some((_, 'r')) => s.push('\r'),
                Some((_, '0')) => s.push('\0'),
                Some((_, '\n')) => {
                    while chars.peek().is_some_and(|(_, c)| c.is_whitespace()) {
                        chars.next();
                    }
                }
                Some((_, other)) => s.push(other),
                None => break,
            },
            c => s.push(c),
        }
    }
    (s, text.len())
}

#[test]
fn string_literals_when_raw_and_escaped_then_both_unescaped() {
    let text = "let a = r#\"x \"y\"\"#; let b = \"p\\nq\"; let c = '\"';";
    assert_eq!(string_literals(text), vec!["x \"y\"", "p\nq"]);
}
