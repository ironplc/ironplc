use crate::semantic_context::SemanticContext;
use crate::stages::resolve_types;
use ironplc_dsl::common::*;
use ironplc_dsl::core::FileId;

#[cfg(test)]
pub fn parse_only(program: &str) -> Library {
    use ironplc_parser::{options::CompilerOptions, parse_program};

    parse_program(program, &FileId::default(), &CompilerOptions::default()).unwrap()
}

#[cfg(test)]
pub fn parse_and_resolve_types(program: &str) -> Library {
    use ironplc_parser::{options::CompilerOptions, parse_program};

    let library = parse_program(program, &FileId::default(), &CompilerOptions::default()).unwrap();
    let (library, _context) = resolve_types(&[&library], &CompilerOptions::default()).unwrap();
    library
}

/// Parses a program and resolves types, returning both the library and semantic context.
/// Use this when testing rules that need access to the type environment or other context.
#[cfg(test)]
pub fn parse_and_resolve_types_with_context(program: &str) -> (Library, SemanticContext) {
    use ironplc_parser::{options::CompilerOptions, parse_program};

    let library = parse_program(program, &FileId::default(), &CompilerOptions::default()).unwrap();
    resolve_types(&[&library], &CompilerOptions::default()).unwrap()
}

/// Parses a program with custom options and resolves types, returning both library and context.
/// Use this when testing dialect-specific behavior.
#[cfg(test)]
pub fn parse_and_resolve_types_with_options(
    program: &str,
    options: &ironplc_parser::options::CompilerOptions,
) -> (Library, SemanticContext) {
    use ironplc_parser::parse_program;

    let library = parse_program(program, &FileId::default(), options).unwrap();
    resolve_types(&[&library], options).unwrap()
}

/// The qualifier of every declaration named `name`, in library order.
///
/// Transforms and rules that add or check `DeclarationQualifier`s assert on
/// this rather than each re-walking the library.
#[cfg(test)]
pub fn declaration_qualifiers(library: &Library, name: &str) -> Vec<DeclarationQualifier> {
    use ironplc_dsl::core::Id;
    use ironplc_dsl::visitor::Visitor;
    use std::convert::Infallible;

    struct Finder {
        name: Id,
        found: Vec<DeclarationQualifier>,
    }
    impl Visitor<Infallible> for Finder {
        type Value = ();
        fn visit_var_decl(&mut self, node: &VarDecl) -> Result<(), Infallible> {
            if node.identifier.symbolic_id() == Some(&self.name) {
                self.found.push(node.qualifier.clone());
            }
            Ok(())
        }
    }
    let mut finder = Finder {
        name: Id::from(name),
        found: vec![],
    };
    let Ok(()) = finder.walk(library);
    finder.found
}

/// The code of `Problem::NotImplemented`. The variant is `#[deprecated]` so
/// that only `Diagnostic::not_implemented` (which records the compiler
/// location) constructs it, and that also keeps tests from naming it. Rule
/// tests compare against this rather than spelling out the code.
#[cfg(test)]
pub const NOT_IMPLEMENTED_CODE: &str = "P9999";

/// A rule's `apply`, as every analyzer rule declares it.
#[cfg(test)]
pub type Rule = fn(
    &Library,
    &SemanticContext,
    &ironplc_parser::options::CompilerOptions,
) -> crate::result::SemanticResult;

/// Resolves `program` under `options` and runs `rule` on the resolved library
/// and context, returning the diagnostics it reports (none when it accepts).
#[cfg(test)]
pub fn rule_diagnostics(
    rule: Rule,
    program: &str,
    options: &ironplc_parser::options::CompilerOptions,
) -> Vec<ironplc_dsl::diagnostic::Diagnostic> {
    let (library, context) = parse_and_resolve_types_with_options(program, options);
    rule(&library, &context, options).err().unwrap_or_default()
}

/// As [`rule_diagnostics`], returning only the problem codes, in order.
#[cfg(test)]
pub fn rule_codes(
    rule: Rule,
    program: &str,
    options: &ironplc_parser::options::CompilerOptions,
) -> Vec<String> {
    rule_diagnostics(rule, program, options)
        .into_iter()
        .map(|diagnostic| diagnostic.code)
        .collect()
}

/// The codes of `diagnostics`, in order.
#[cfg(test)]
pub fn diagnostic_codes(diagnostics: &[ironplc_dsl::diagnostic::Diagnostic]) -> Vec<&str> {
    diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code.as_str())
        .collect()
}

/// The codes of `problems`, in order, to compare with [`rule_codes`].
#[cfg(test)]
pub fn codes(problems: &[ironplc_problems::Problem]) -> Vec<&str> {
    problems.iter().map(|problem| problem.code()).collect()
}

/// Default options with function block inheritance (`EXTENDS`, methods,
/// properties) enabled.
#[cfg(test)]
pub fn fb_inheritance_options() -> ironplc_parser::options::CompilerOptions {
    ironplc_parser::options::CompilerOptions {
        allow_fb_inheritance: true,
        ..ironplc_parser::options::CompilerOptions::default()
    }
}

/// The options of the IEC 61131-3 Edition 3 dialect.
#[cfg(test)]
pub fn edition3_options() -> ironplc_parser::options::CompilerOptions {
    ironplc_parser::options::CompilerOptions::from_dialect(
        ironplc_parser::options::Dialect::Iec61131_3Ed3,
    )
}

#[cfg(test)]
mod tests {
    use super::{codes, rule_codes, rule_diagnostics, NOT_IMPLEMENTED_CODE};
    use crate::result::SemanticResult;
    use crate::semantic_context::SemanticContext;
    use ironplc_dsl::common::Library;
    use ironplc_dsl::core::SourceSpan;
    use ironplc_dsl::diagnostic::{Diagnostic, Label};
    use ironplc_parser::options::CompilerOptions;
    use ironplc_problems::Problem;

    #[test]
    fn not_implemented_code_when_compared_to_constructor_then_equal() {
        let diagnostic = Diagnostic::not_implemented(Label::span(SourceSpan::default(), "x"));

        assert_eq!(diagnostic.code, NOT_IMPLEMENTED_CODE);
    }

    /// A rule that reports one problem for every library it is given.
    fn always_reports(
        _library: &Library,
        _context: &SemanticContext,
        _options: &CompilerOptions,
    ) -> SemanticResult {
        Err(vec![Diagnostic::not_implemented(Label::span(
            SourceSpan::default(),
            "x",
        ))])
    }

    /// A rule that accepts every library.
    fn always_accepts(
        _library: &Library,
        _context: &SemanticContext,
        _options: &CompilerOptions,
    ) -> SemanticResult {
        Ok(())
    }

    const PROGRAM: &str = "PROGRAM main END_PROGRAM";

    #[test]
    fn rule_codes_when_rule_reports_then_its_codes() {
        let codes = rule_codes(always_reports, PROGRAM, &CompilerOptions::default());

        assert_eq!(codes, [NOT_IMPLEMENTED_CODE]);
    }

    #[test]
    fn rule_diagnostics_when_rule_accepts_then_empty() {
        let diagnostics = rule_diagnostics(always_accepts, PROGRAM, &CompilerOptions::default());

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn codes_when_problems_then_codes_in_order() {
        assert_eq!(
            codes(&[Problem::ConstantOverflow, Problem::RealLiteralOutOfRange]),
            [
                Problem::ConstantOverflow.code(),
                Problem::RealLiteralOutOfRange.code()
            ]
        );
    }
}
