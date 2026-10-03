//! Checks that rule tests follow the rule-test conventions in
//! `specs/steering/compiler-standards.md` (Rule Tests).
//!
//! A crate's conventions test calls [`violations`] over its own `src`
//! directory. Each check reads the source text, so a convention a check
//! cannot express (an exact assertion inside a hand-written body) is left to
//! review.
//!
//! A line may opt out of one check with a comment on that line or in the
//! comment block directly above it:
//!
//! ```text
//! // rule-test-conventions: allow(pipeline) -- the test shows the rule is wired in
//! ```

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

/// One convention a rule's tests must follow.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Convention {
    /// A problem code spelled as a string (`"P2037"`) instead of a `Problem`.
    StringCode,
    /// An assertion that holds for any error (`.is_err()`, `has_diagnostics()`).
    AnyError,
    /// An empty context handed to a rule whose `apply` reads its context.
    EmptyContext,
    /// A rule test that runs the whole `analyze` pipeline.
    Pipeline,
    /// A rule with no test that expects no problems.
    MissingOk,
    /// A rule with no test that names a problem it reports.
    MissingErr,
}

impl Convention {
    /// Every convention, as the analyzer checks them.
    pub const ALL: &'static [Convention] = &[
        Convention::StringCode,
        Convention::AnyError,
        Convention::EmptyContext,
        Convention::Pipeline,
        Convention::MissingOk,
        Convention::MissingErr,
    ];

    /// The name an `allow(...)` comment uses.
    pub fn id(self) -> &'static str {
        match self {
            Convention::StringCode => "string-code",
            Convention::AnyError => "any-error",
            Convention::EmptyContext => "empty-context",
            Convention::Pipeline => "pipeline",
            Convention::MissingOk => "missing-ok",
            Convention::MissingErr => "missing-err",
        }
    }

    fn explanation(self) -> &'static str {
        match self {
            Convention::StringCode => {
                "name the problem as a `Problem` variant, not a \"P####\" string"
            }
            Convention::AnyError => {
                "assert the exact problems reported, not that some error occurred"
            }
            Convention::EmptyContext => {
                "this rule reads its context; test it against the resolved one (rule_ok!, rule_err!, test_helpers::rule_codes)"
            }
            Convention::Pipeline => "call the rule's own apply rather than the analyze pipeline",
            Convention::MissingOk => "add a test where the rule reports no problems",
            Convention::MissingErr => "add a test naming a problem the rule reports",
        }
    }
}

/// A place where a rule's tests break a convention.
#[derive(Debug, PartialEq, Eq)]
pub struct Violation {
    pub file: PathBuf,
    /// The 1-based line, or `None` for a check on the rule as a whole.
    pub line: Option<usize>,
    pub convention: Convention,
}

impl fmt::Display for Violation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.line {
            Some(line) => write!(f, "{}:{line}", self.file.display())?,
            None => write!(f, "{}", self.file.display())?,
        }
        write!(
            f,
            ": [{}] {}",
            self.convention.id(),
            self.convention.explanation()
        )
    }
}

/// The test source of one file: its text and the line number of its first line.
#[derive(Debug)]
pub struct TestText {
    pub file: PathBuf,
    pub text: String,
    pub first_line: usize,
}

/// One rule: whether its `apply` reads its context, and its test sources.
#[derive(Debug)]
pub struct RuleSource {
    pub file: PathBuf,
    pub reads_context: bool,
    pub tests: Vec<TestText>,
}

impl RuleSource {
    /// Splits a rule module at its `#[cfg(test)]`: what comes before is the
    /// rule, what comes after is its inline tests.
    pub fn from_text(file: PathBuf, text: &str) -> RuleSource {
        let (implementation, tests) = match text.find("#[cfg(test)]") {
            Some(at) => (&text[..at], vec![(at, &text[at..])]),
            None => (text, vec![]),
        };
        RuleSource {
            reads_context: apply_reads_context(implementation),
            tests: tests
                .into_iter()
                .map(|(at, tests)| TestText {
                    file: file.clone(),
                    text: tests.to_string(),
                    first_line: text[..at].lines().count() + 1,
                })
                .collect(),
            file,
        }
    }
}

/// Whether the `apply` signature names its context without a leading `_`.
fn apply_reads_context(implementation: &str) -> bool {
    let Some(start) = implementation.find("pub fn apply(") else {
        return false;
    };
    let signature = &implementation[start..];
    let signature = &signature[..signature.find(')').unwrap_or(signature.len())];
    signature
        .match_indices("context: &SemanticContext")
        .any(|(at, _)| !signature[..at].ends_with('_'))
}

/// Reads every `rule_*.rs` module in `src` and the files of its `rule_*/`
/// directory, if any. `rule_support.rs` holds shared rule code, not a rule.
pub fn read_rule_sources(src: &Path) -> Vec<RuleSource> {
    let mut modules: Vec<PathBuf> = fs::read_dir(src)
        .expect("source directory is readable")
        .map(|entry| entry.expect("directory entry is readable").path())
        .filter(|path| {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            name.starts_with("rule_") && name.ends_with(".rs") && name != "rule_support.rs"
        })
        .collect();
    modules.sort();

    modules
        .into_iter()
        .map(|module| {
            let text = fs::read_to_string(&module).expect("rule module is readable");
            let mut rule = RuleSource::from_text(module.clone(), &text);
            let directory = module.with_extension("");
            if directory.is_dir() {
                let mut files: Vec<PathBuf> = fs::read_dir(&directory)
                    .expect("rule directory is readable")
                    .map(|entry| entry.expect("directory entry is readable").path())
                    .filter(|path| path.extension().is_some_and(|e| e == "rs"))
                    .collect();
                files.sort();
                rule.tests.extend(files.into_iter().map(|file| TestText {
                    text: fs::read_to_string(&file).expect("rule test file is readable"),
                    file,
                    first_line: 1,
                }));
            }
            rule
        })
        .collect()
}

/// Every violation of `conventions` by the rules in `src`, with paths
/// relative to the crate that holds `src`.
pub fn violations(src: &Path, conventions: &[Convention]) -> Vec<Violation> {
    let root = src.parent().unwrap_or(src);
    read_rule_sources(src)
        .iter()
        .flat_map(|rule| check(rule, conventions))
        .map(|violation| Violation {
            file: violation
                .file
                .strip_prefix(root)
                .map(Path::to_path_buf)
                .unwrap_or(violation.file),
            ..violation
        })
        .collect()
}

/// The rules in `src` whose `apply` the `registry` source never calls as
/// `rule_x::apply`: rules that exist but are not part of the pipeline. A
/// rule's own tests call its `apply` directly, so they pass either way.
pub fn unregistered_rules(src: &Path, registry: &Path) -> Vec<PathBuf> {
    let registry = fs::read_to_string(registry).expect("registry source is readable");
    read_rule_sources(src)
        .into_iter()
        .map(|rule| rule.file)
        .filter(|file| !registry.contains(&format!("{}::apply", module_name(file))))
        .collect()
}

fn module_name(file: &Path) -> String {
    file.file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or_default()
        .to_string()
}

const EMPTY_CONTEXT: &[&str] = &["SemanticContextBuilder::new()"];

const ANY_ERROR: &[&str] = &[".is_err()", "has_diagnostics()"];

const PIPELINE: &[&str] = &["analyze(&"];

/// Markers of a test where the rule reports nothing; `&[]` is an empty
/// expected problem list (`codes(&[])`, `const OK: &[Problem] = &[]`).
const OK_SIDE: &[&str] = &[
    "rule_ok!(",
    "token_rule_ok!(",
    ".is_ok()",
    "is_empty()",
    "&[]",
];

const ERR_SIDE: &[&str] = &["Problem::", "NOT_IMPLEMENTED_CODE"];

/// Every violation of `conventions` by one rule's tests.
pub fn check(rule: &RuleSource, conventions: &[Convention]) -> Vec<Violation> {
    let mut found = Vec::new();
    for tests in &rule.tests {
        let lines: Vec<&str> = tests.text.lines().collect();
        for (index, line) in lines.iter().enumerate() {
            if line.trim_start().starts_with("//") {
                continue;
            }
            for &convention in conventions {
                let hit = match convention {
                    Convention::StringCode => contains_string_code(line),
                    Convention::AnyError => contains_any(line, ANY_ERROR),
                    Convention::EmptyContext => {
                        rule.reads_context && contains_any(line, EMPTY_CONTEXT)
                    }
                    Convention::Pipeline => contains_any(line, PIPELINE),
                    Convention::MissingOk | Convention::MissingErr => false,
                };
                if hit && !allowed(&lines, index, convention) {
                    found.push(Violation {
                        file: tests.file.clone(),
                        line: Some(tests.first_line + index),
                        convention,
                    });
                }
            }
        }
    }

    let all_tests = || rule.tests.iter().map(|t| t.text.as_str());
    let sides = [
        (Convention::MissingOk, OK_SIDE),
        (Convention::MissingErr, ERR_SIDE),
    ];
    for (convention, markers) in sides {
        if conventions.contains(&convention) && !all_tests().any(|t| contains_any(t, markers)) {
            found.push(Violation {
                file: rule.file.clone(),
                line: None,
                convention,
            });
        }
    }
    found
}

/// Whether `needle` occurs in `text` not as the tail of a longer identifier,
/// so that `rule_ok!(` does not match inside a longer name such as `my_rule_ok!(`.
fn contains_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| {
        let is_identifier = |c: char| c.is_alphanumeric() || c == '_';
        let whole_word = needle.starts_with(is_identifier);
        text.match_indices(needle)
            .any(|(at, _)| !whole_word || !text[..at].ends_with(is_identifier))
    })
}

/// Whether `line` holds a string literal `"P####"`.
fn contains_string_code(line: &str) -> bool {
    line.as_bytes().windows(7).any(|w| {
        w[0] == b'"' && w[1] == b'P' && w[2..6].iter().all(u8::is_ascii_digit) && w[6] == b'"'
    })
}

/// Whether the line at `index`, or the comment block directly above it,
/// opts out of `convention`.
fn allowed(lines: &[&str], index: usize, convention: Convention) -> bool {
    let marker = format!("rule-test-conventions: allow({})", convention.id());
    if lines[index].contains(&marker) {
        return true;
    }
    lines[..index]
        .iter()
        .rev()
        .take_while(|line| line.trim_start().starts_with("//"))
        .any(|line| line.contains(&marker))
}

#[cfg(test)]
mod tests {
    use super::*;

    const APPLY_IGNORES_CONTEXT: &str = "pub fn apply(\n    lib: &Library,\n    _context: &SemanticContext,\n    _options: &CompilerOptions,\n) -> SemanticResult {\n}\n";
    const APPLY_READS_CONTEXT: &str = "pub fn apply(\n    lib: &Library,\n    context: &SemanticContext,\n    _options: &CompilerOptions,\n) -> SemanticResult {\n}\n";

    /// A rule whose tests are `tests`, with an ok test and an err test so that
    /// only the convention under test can fire.
    fn rule(apply: &str, tests: &str) -> RuleSource {
        let text = format!(
            "{apply}#[cfg(test)]\nmod tests {{\n    rule_ok!(ok, \"\");\n    rule_err!(err, \"\", [Problem::X]);\n{tests}\n}}\n"
        );
        RuleSource::from_text(PathBuf::from("rule_x.rs"), &text)
    }

    fn found(rule: &RuleSource) -> Vec<Convention> {
        check(rule, Convention::ALL)
            .into_iter()
            .map(|v| v.convention)
            .collect()
    }

    #[test]
    fn check_when_tests_follow_conventions_then_no_violations() {
        let rule = rule(
            APPLY_READS_CONTEXT,
            "    assert_eq!(codes, [Problem::X.code()]);",
        );

        assert_eq!(found(&rule), vec![]);
    }

    #[test]
    fn check_when_string_code_then_string_code() {
        let rule = rule(APPLY_IGNORES_CONTEXT, "    assert_eq!(code, \"P2037\");");

        assert_eq!(found(&rule), vec![Convention::StringCode]);
    }

    #[test]
    fn check_when_is_err_then_any_error() {
        let rule = rule(APPLY_IGNORES_CONTEXT, "    assert!(result.is_err());");

        assert_eq!(found(&rule), vec![Convention::AnyError]);
    }

    #[test]
    fn check_when_has_diagnostics_then_any_error() {
        let rule = rule(
            APPLY_IGNORES_CONTEXT,
            "    assert!(context.has_diagnostics());",
        );

        assert_eq!(found(&rule), vec![Convention::AnyError]);
    }

    #[test]
    fn check_when_hand_built_empty_context_and_rule_ignores_context_then_no_violations() {
        let rule = rule(
            APPLY_IGNORES_CONTEXT,
            "    let context = SemanticContextBuilder::new().build().unwrap();",
        );

        assert_eq!(found(&rule), vec![]);
    }

    #[test]
    fn check_when_hand_built_empty_context_then_empty_context() {
        let rule = rule(
            APPLY_READS_CONTEXT,
            "    let context = SemanticContextBuilder::new().build().unwrap();",
        );

        assert_eq!(found(&rule), vec![Convention::EmptyContext]);
    }

    #[test]
    fn check_when_analyze_then_pipeline() {
        let rule = rule(
            APPLY_IGNORES_CONTEXT,
            "    let (_, context) = analyze(&[&library], &options).unwrap();",
        );

        assert_eq!(found(&rule), vec![Convention::Pipeline]);
    }

    #[test]
    fn check_when_allowed_in_comment_above_then_no_violations() {
        let rule = rule(
            APPLY_IGNORES_CONTEXT,
            "    // rule-test-conventions: allow(pipeline) -- shows the rule is wired in\n    // A second comment line.\n    let (_, context) = analyze(&[&library], &options).unwrap();",
        );

        assert_eq!(found(&rule), vec![]);
    }

    #[test]
    fn check_when_allowed_for_other_convention_then_violation() {
        let rule = rule(
            APPLY_IGNORES_CONTEXT,
            "    // rule-test-conventions: allow(any-error)\n    let (_, context) = analyze(&[&library], &options).unwrap();",
        );

        assert_eq!(found(&rule), vec![Convention::Pipeline]);
    }

    #[test]
    fn check_when_violation_in_comment_then_no_violations() {
        let rule = rule(APPLY_IGNORES_CONTEXT, "    // assert!(result.is_err());");

        assert_eq!(found(&rule), vec![]);
    }

    #[test]
    fn check_when_no_ok_test_then_missing_ok() {
        let rule = RuleSource::from_text(
            PathBuf::from("rule_x.rs"),
            "#[cfg(test)]\nmod tests {\n    rule_err!(err, \"\", [Problem::X]);\n}\n",
        );

        assert_eq!(found(&rule), vec![Convention::MissingOk]);
    }

    #[test]
    fn check_when_ok_side_is_token_rule_macro_then_no_missing_ok() {
        let rule = RuleSource::from_text(
            PathBuf::from("rule_x.rs"),
            "#[cfg(test)]\nmod test {\n    token_rule_ok!(ok, vec![]);\n    token_rule_err!(err, vec![], [Problem::X]);\n}\n",
        );

        assert_eq!(found(&rule), vec![]);
    }

    #[test]
    fn unregistered_rules_when_registry_omits_rule_then_rule_listed() {
        let dir = std::env::temp_dir().join(format!("rule_conventions_{}", std::process::id()));
        let src = dir.join("src");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("rule_a.rs"), "pub fn apply() {}").unwrap();
        fs::write(src.join("rule_b.rs"), "pub fn apply() {}").unwrap();
        let registry = src.join("stages.rs");
        fs::write(&registry, "rule_a::apply,").unwrap();

        let unregistered = unregistered_rules(&src, &registry);

        fs::remove_dir_all(&dir).unwrap();
        assert_eq!(unregistered, vec![src.join("rule_b.rs")]);
    }

    #[test]
    fn check_when_ok_side_is_empty_problem_list_then_no_missing_ok() {
        let rule = RuleSource::from_text(
            PathBuf::from("rule_x.rs"),
            "#[cfg(test)]\nmod tests {\n    const OK: &[Problem] = &[];\n    rule_err!(err, \"\", [Problem::X]);\n}\n",
        );

        assert_eq!(found(&rule), vec![]);
    }

    #[test]
    fn check_when_no_problem_named_then_missing_err() {
        let rule = RuleSource::from_text(
            PathBuf::from("rule_x.rs"),
            "#[cfg(test)]\nmod tests {\n    rule_ok!(ok, \"\");\n}\n",
        );

        assert_eq!(found(&rule), vec![Convention::MissingErr]);
    }

    #[test]
    fn check_when_violation_then_reports_line_in_file() {
        let rule = rule(APPLY_IGNORES_CONTEXT, "    assert!(result.is_err());");

        let violations = check(&rule, Convention::ALL);

        // The apply above spans 6 lines; then #[cfg(test)], mod, ok, err.
        assert_eq!(violations[0].line, Some(11));
        assert_eq!(
            violations[0].to_string(),
            "rule_x.rs:11: [any-error] assert the exact problems reported, not that some error occurred"
        );
    }
}
