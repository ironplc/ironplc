//! Reads the parts of a `Cargo.toml` that the workspace guards check.
//!
//! Each reader looks for one shape the compiler's manifests use rather than
//! parsing TOML in general, so a guard needs no dependency to read them.

/// The string literals inside the first `[…]` list that follows `key` in
/// `text`. Used for TOML arrays and Ruby argument lists alike, both of which
/// quote every element.
pub fn quoted_items_after(text: &str, key: &str) -> Vec<String> {
    let Some(idx) = text.find(key) else {
        return Vec::new();
    };
    let after = &text[idx + key.len()..];
    let end = after.find(']').unwrap_or(after.len());
    quoted_strings(&after[..end])
}

/// Every double-quoted string literal in `span`, in order.
pub fn quoted_strings(span: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = span;
    while let Some(open) = rest.find('"') {
        let after = &rest[open + 1..];
        let Some(close) = after.find('"') else {
            break;
        };
        out.push(after[..close].to_string());
        rest = &after[close + 1..];
    }
    out
}

/// The `members` list of a workspace `Cargo.toml`.
pub fn workspace_members(cargo_toml: &str) -> Vec<String> {
    quoted_items_after(cargo_toml, "members = [")
}

/// The `name` in a crate's `[package]` section.
pub fn package_name(cargo_toml: &str) -> Option<String> {
    section(cargo_toml, "[package]")
        .into_iter()
        .find_map(|line| key_of(line).filter(|key| *key == "name").map(|_| line))
        .and_then(|line| quoted_strings(line).into_iter().next())
}

/// The crates a crate's `[dependencies]` section names, in order. The
/// `[dev-dependencies]` and `[build-dependencies]` are not included.
pub fn dependencies(cargo_toml: &str) -> Vec<String> {
    section(cargo_toml, "[dependencies]")
        .into_iter()
        .filter_map(key_of)
        .map(str::to_string)
        .collect()
}

/// The lines of the section that `header` opens, up to the next header.
fn section<'a>(cargo_toml: &'a str, header: &str) -> Vec<&'a str> {
    cargo_toml
        .lines()
        .skip_while(|line| line.trim() != header)
        .skip(1)
        .take_while(|line| !line.trim_start().starts_with('['))
        .collect()
}

/// The key a `key = value` line assigns, or none for a blank or comment line.
fn key_of(line: &str) -> Option<&str> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }
    line.split_once('=').map(|(key, _)| key.trim())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_members_when_multiline_list_then_extracts_all() {
        let toml = r#"
[workspace]
members = [
    "analyzer",
    "vm-cli",
]
resolver = "2"
"#;
        assert_eq!(workspace_members(toml), vec!["analyzer", "vm-cli"]);
    }

    #[test]
    fn package_name_when_package_section_then_reads_its_name() {
        let toml = r#"
[package]
name = "ironplc-codegen"
version = "0.1.0"
"#;
        assert_eq!(package_name(toml).as_deref(), Some("ironplc-codegen"));
    }

    #[test]
    fn dependencies_when_dev_and_build_sections_then_reads_only_dependencies() {
        let toml = r#"
[package]
name = "a"

[dependencies]
# What the crate reads.
ironplc-ir = { path = "../ir" }
blake3 = "1"

[build-dependencies]
ironplc-spec-requirements-gen = { path = "../spec_requirements_gen" }

[dev-dependencies]
ironplc-dsl = { path = "../dsl" }
"#;
        assert_eq!(dependencies(toml), vec!["ironplc-ir", "blake3"]);
    }

    #[test]
    fn dependencies_when_no_section_then_empty() {
        assert!(dependencies("[package]\nname = \"a\"\n").is_empty());
    }
}
