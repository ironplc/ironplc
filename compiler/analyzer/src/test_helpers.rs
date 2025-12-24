use crate::stages::resolve_types;
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
    let (library, _type_environment, _symbol_environment) = resolve_types(&[&library]).unwrap();
    library
}

#[cfg(test)]
pub fn parse_and_analyze(program: &str) -> Result<(), Vec<ironplc_dsl::diagnostic::Diagnostic>> {
    use ironplc_parser::{options::ParseOptions, parse_program};
    use crate::stages::semantic;

    let library = parse_program(program, &FileId::default(), &ParseOptions::default()).unwrap();
    let (library, type_environment, symbol_environment) = resolve_types(&[&library]).unwrap();
    semantic(&library, &type_environment, &symbol_environment)
}
