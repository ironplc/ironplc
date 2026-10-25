//! Compares two directories of containers, such as the ones the
//! `container-dump` recipe of `compiler/justfile` writes for two versions of
//! the compiler, and reports each container that differs.
//!
//! A refactor of code generation shows that it changes nothing by leaving
//! every container byte for byte as it was. Where one differs, the report
//! says where: the disassembly path of each instruction or constant that
//! differs, or, when the code is the same, the first lines of the debug and
//! type sections that differ.
//!
//! The comparison is an `#[ignore]`d test, run by `just container-diff`,
//! because `ironplc-codegen`, whose tests write the containers, cannot depend
//! on this crate, which depends on it.

use std::collections::BTreeSet;
use std::path::Path;

use ironplc_container::Container;
use serde_json::Value;

use crate::disassemble::disassemble;

/// How two directories of containers compare.
#[derive(Debug, Default)]
struct Comparison {
    identical: usize,
    /// Each container that differs, with where.
    differ: Vec<(String, Vec<String>)>,
    only_before: Vec<String>,
    only_after: Vec<String>,
}

fn compare_directories(before: &Path, after: &Path) -> Comparison {
    let names = |dir: &Path| -> BTreeSet<String> {
        std::fs::read_dir(dir)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", dir.display()))
            .map(|entry| entry.expect("a directory entry").file_name())
            .map(|name| name.to_string_lossy().into_owned())
            .collect()
    };
    let (before_names, after_names) = (names(before), names(after));
    let mut comparison = Comparison {
        only_before: before_names.difference(&after_names).cloned().collect(),
        only_after: after_names.difference(&before_names).cloned().collect(),
        ..Comparison::default()
    };
    for name in before_names.intersection(&after_names) {
        let read = |dir: &Path| std::fs::read(dir.join(name)).expect("a dumped container");
        let (old, new) = (read(before), read(after));
        if old == new {
            comparison.identical += 1;
        } else {
            comparison
                .differ
                .push((name.clone(), container_differences(&old, &new)));
        }
    }
    comparison
}

/// Where the containers `old` and `new`, which differ, differ.
fn container_differences(old: &[u8], new: &[u8]) -> Vec<String> {
    let parse = |bytes: &[u8]| Container::read_from(&mut &bytes[..]);
    let (old, new) = match (parse(old), parse(new)) {
        (Ok(old), Ok(new)) => (old, new),
        (old, new) => return vec![format!("unreadable: {:?} / {:?}", old.err(), new.err())],
    };
    let mut differences = Vec::new();
    json_differences("", &disassemble(&old), &disassemble(&new), &mut differences);
    if differences.is_empty() {
        let sections = |c: &Container| format!("{:#?}\n{:#?}", c.debug_section, c.type_section);
        differences = line_differences(&sections(&old), &sections(&new));
    }
    differences
}

/// Appends to `out` the path of each leaf at which `old` and `new` differ,
/// with both values.
fn json_differences(path: &str, old: &Value, new: &Value, out: &mut Vec<String>) {
    match (old, new) {
        (Value::Object(o), Value::Object(n)) => {
            let keys: BTreeSet<&String> = o.keys().chain(n.keys()).collect();
            for key in keys {
                let (o, n) = (o.get(key), n.get(key));
                let child = format!("{path}.{key}");
                json_differences(
                    &child,
                    o.unwrap_or(&Value::Null),
                    n.unwrap_or(&Value::Null),
                    out,
                );
            }
        }
        (Value::Array(o), Value::Array(n)) => {
            for index in 0..o.len().max(n.len()) {
                let (o, n) = (o.get(index), n.get(index));
                let child = format!("{path}[{index}]");
                json_differences(
                    &child,
                    o.unwrap_or(&Value::Null),
                    n.unwrap_or(&Value::Null),
                    out,
                );
            }
        }
        (old, new) if old != new => out.push(format!("{path}: {old} -> {new}")),
        _ => {}
    }
}

/// The lines at which `old` and `new` first differ, compared line by line.
fn line_differences(old: &str, new: &str) -> Vec<String> {
    const SHOWN: usize = 5;
    let (old, new): (Vec<&str>, Vec<&str>) = (old.lines().collect(), new.lines().collect());
    (0..old.len().max(new.len()))
        .filter_map(|line| {
            let (o, n) = (old.get(line).unwrap_or(&""), new.get(line).unwrap_or(&""));
            (o != n).then(|| format!("line {}: {} -> {}", line + 1, o.trim(), n.trim()))
        })
        .take(SHOWN)
        .collect()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    /// Compares the directories `IRONPLC_CONTAINER_DIFF_BEFORE` and
    /// `IRONPLC_CONTAINER_DIFF_AFTER` name, prints what differs, and fails
    /// unless every container is identical. Run by `just container-diff`.
    #[test]
    #[ignore = "compares two container dumps; run by `just container-diff`"]
    fn container_diff_when_two_dumps_then_every_container_identical() {
        let dir =
            |var: &str| std::env::var_os(var).unwrap_or_else(|| panic!("{var} names no directory"));
        let (before, after) = (
            dir("IRONPLC_CONTAINER_DIFF_BEFORE"),
            dir("IRONPLC_CONTAINER_DIFF_AFTER"),
        );
        let comparison = compare_directories(Path::new(&before), Path::new(&after));
        for (name, differences) in &comparison.differ {
            println!("{name}");
            for difference in differences {
                println!("  {difference}");
            }
        }
        let compared = comparison.identical + comparison.differ.len();
        println!(
            "{compared} containers compared: {} identical, {} differ, {} only before, {} only after",
            comparison.identical,
            comparison.differ.len(),
            comparison.only_before.len(),
            comparison.only_after.len()
        );
        assert!(
            comparison.differ.is_empty()
                && comparison.only_before.is_empty()
                && comparison.only_after.is_empty(),
            "the dumps differ; only before: {:?}; only after: {:?}",
            comparison.only_before,
            comparison.only_after
        );
    }

    #[test]
    fn json_differences_when_leaves_differ_then_paths_and_values() {
        let old = json!({"functions": [{"ops": ["LOAD 1", "STORE 0"]}], "header": {"n": 2}});
        let new = json!({"functions": [{"ops": ["LOAD 1", "STORE 3"]}], "header": {"n": 2}});
        let mut out = Vec::new();

        json_differences("", &old, &new, &mut out);

        assert_eq!(out, [r#".functions[0].ops[1]: "STORE 0" -> "STORE 3""#]);
    }

    #[test]
    fn json_differences_when_array_longer_then_extra_element_against_null() {
        let mut out = Vec::new();

        json_differences("", &json!([1]), &json!([1, 2]), &mut out);

        assert_eq!(out, ["[1]: null -> 2"]);
    }

    #[test]
    fn line_differences_when_one_line_differs_then_that_line() {
        let out = line_differences("a\nb\nc", "a\nx\nc");

        assert_eq!(out, ["line 2: b -> x"]);
    }

    #[test]
    fn compare_directories_when_one_differs_and_one_missing_then_reported() {
        let before = tempfile::tempdir().unwrap();
        let after = tempfile::tempdir().unwrap();
        let container = |value: i32| {
            let mut bytes = Vec::new();
            ironplc_container::ContainerBuilder::new()
                .add_i32_constant(value)
                .build()
                .write_to(&mut bytes)
                .unwrap();
            bytes
        };
        std::fs::write(before.path().join("same.iplc"), container(1)).unwrap();
        std::fs::write(after.path().join("same.iplc"), container(1)).unwrap();
        std::fs::write(before.path().join("changed.iplc"), container(1)).unwrap();
        std::fs::write(after.path().join("changed.iplc"), container(2)).unwrap();
        std::fs::write(before.path().join("gone.iplc"), container(1)).unwrap();

        let comparison = compare_directories(before.path(), after.path());

        assert_eq!(comparison.identical, 1);
        assert_eq!(comparison.differ.len(), 1);
        assert_eq!(comparison.differ[0].0, "changed.iplc");
        assert!(!comparison.differ[0].1.is_empty());
        assert_eq!(comparison.only_before, ["gone.iplc"]);
        assert!(comparison.only_after.is_empty());
    }
}
