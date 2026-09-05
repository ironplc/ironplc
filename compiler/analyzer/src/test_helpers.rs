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

/// Resolves `program` under `options` but pairs the resolved library with a
/// *fresh, empty* [`SemanticContext`] rather than the resolved one.
///
/// This mirrors the recurring rule-test scaffold that discards the resolved
/// context (`let (input, _context) = parse_and_resolve_types_with_options(...)`)
/// and builds `SemanticContextBuilder::new().build().unwrap()` instead. Rules in
/// this group operate on the library and do not consult the type environment, so
/// the empty context is intentional. Used by the `rule_ok!/rule_err!/…` macros.
#[cfg(test)]
pub fn resolve_fresh_with(
    program: &str,
    options: &ironplc_parser::options::CompilerOptions,
) -> (Library, SemanticContext) {
    use crate::semantic_context::SemanticContextBuilder;

    let (library, _resolved) = parse_and_resolve_types_with_options(program, options);
    let context = SemanticContextBuilder::new().build().unwrap();
    (library, context)
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
