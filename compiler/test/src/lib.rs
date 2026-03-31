use std::{fs, path::PathBuf};

/// Extracts the inner value from a tuple enum variant, panicking with a
/// descriptive message if the variant does not match.
///
/// # Examples
/// ```ignore
/// let decl = cast!(expr, LibraryElementKind::FunctionDeclaration);
/// ```
#[macro_export]
macro_rules! cast {
    ($target:expr, $pat:path) => {{
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

/// Extracts fields from a struct enum variant, panicking with a descriptive
/// message if the variant does not match. Returns a tuple of the extracted
/// fields.
///
/// # Examples
/// ```ignore
/// let (element_type, dimensions) = cast_struct!(
///     attrs.representation,
///     IntermediateType::Array { element_type, dimensions }
/// );
/// ```
#[macro_export]
macro_rules! cast_struct {
    ($target:expr, $pat:path { $($field:ident),+ }) => {{
        if let $pat { $($field),+, .. } = $target {
            ($($field),+)
        } else {
            panic!(
                "expected {} but got {:?}",
                stringify!($pat),
                $target
            );
        }
    }};
}

pub fn read_shared_resource(name: &'static str) -> String {
    fs::read_to_string(shared_resource_path(name)).expect("Unable to read file")
}

pub fn shared_resource_path(name: &'static str) -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("..");
    path.push("resources");
    path.push("test");
    path.push(name);
    path
}
