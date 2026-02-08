use crate::semantic_context::SemanticContext;
use crate::stages::resolve_types;
use crate::type_environment::{TypeEnvironment, TypeEnvironmentBuilder};
use crate::{
    xform_resolve_late_bound_expr_kind, xform_resolve_late_bound_type_initializer,
    xform_resolve_type_decl_environment, xform_toposort_declarations,
};
use ironplc_dsl::common::*;
use ironplc_dsl::core::FileId;

#[cfg(test)]
#[macro_export]
macro_rules! cast {
    // For tuple variants like LibraryElementKind::DataTypeDeclaration(inner)
    ($target: expr, $pat: path) => {{
        if let $pat(a) = $target {
            a
        } else {
            panic!(
                "mismatch variant when cast to {} for {:?}",
                stringify!($pat),
                $target
            );
        }
    }};
}

#[cfg(test)]
#[macro_export]
macro_rules! assert_variant {
    // For unit variants like IntermediateType::Bool
    ($target: expr, $pat: path) => {{
        if !matches!($target, $pat) {
            panic!(
                "expected variant {} but got {:?}",
                stringify!($pat),
                $target
            );
        }
    }};
    // For struct variants like IntermediateType::Array { .. }
    ($target: expr, $pat: path { .. }) => {{
        if !matches!($target, $pat { .. }) {
            panic!(
                "expected variant {} but got {:?}",
                stringify!($pat),
                $target
            );
        }
    }};
}

#[cfg(test)]
pub fn parse_only(program: &str) -> Library {
    use ironplc_parser::{options::ParseOptions, parse_program};

    parse_program(program, &FileId::default(), &ParseOptions::default()).unwrap()
}

#[cfg(test)]
pub fn parse_and_resolve_types(program: &str) -> Library {
    use ironplc_parser::{options::ParseOptions, parse_program};

    let library = parse_program(program, &FileId::default(), &ParseOptions::default()).unwrap();
    let (library, _context) = resolve_types(&[&library]).unwrap();
    library
}

/// Parses a program and resolves types, returning both the library and semantic context.
/// Use this when testing rules that need access to the type environment or other context.
#[cfg(test)]
pub fn parse_and_resolve_types_with_context(program: &str) -> (Library, SemanticContext) {
    use ironplc_parser::{options::ParseOptions, parse_program};

    let library = parse_program(program, &FileId::default(), &ParseOptions::default()).unwrap();
    resolve_types(&[&library]).unwrap()
}

/// Parses a program and resolves types, returning both the library and type environment.
/// This stops before the symbol/function environment transform, useful for testing that transform.
#[cfg(test)]
pub fn parse_and_resolve_types_with_type_env(program: &str) -> (Library, TypeEnvironment) {
    use ironplc_parser::{options::ParseOptions, parse_program};

    let library = parse_program(program, &FileId::default(), &ParseOptions::default()).unwrap();

    let mut type_environment = TypeEnvironmentBuilder::new()
        .with_elementary_types()
        .with_stdlib_function_blocks()
        .build()
        .unwrap();

    let mut library = xform_toposort_declarations::apply(library).unwrap();

    library = xform_resolve_type_decl_environment::apply(library, &mut type_environment).unwrap();
    library = xform_resolve_late_bound_expr_kind::apply(library, &mut type_environment).unwrap();
    library =
        xform_resolve_late_bound_type_initializer::apply(library, &mut type_environment).unwrap();

    (library, type_environment)
}
