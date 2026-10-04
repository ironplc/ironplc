//! Transformation rule that records each enumeration alias, so that the
//! alias has the values of the enumeration it names.
//!
//! An alias is what its declaration says it is: `TYPE B : A; END_TYPE` where
//! `A` is an enumeration. It is never inferred from two types having an equal
//! representation, because every enumeration with the same underlying type
//! has an equal representation whatever its values (issue #1945).

use crate::symbol_environment::SymbolEnvironment;
use ironplc_dsl::common::*;
use ironplc_dsl::diagnostic::Diagnostic;

pub fn apply(
    lib: Library,
    symbol_environment: &mut SymbolEnvironment,
) -> Result<Library, Vec<Diagnostic>> {
    for element in &lib.elements {
        if let LibraryElementKind::DataTypeDeclaration(DataTypeDeclarationKind::Enumeration(decl)) =
            element
        {
            if let SpecificationKind::Named(base) = &decl.spec_init.spec {
                symbol_environment.insert_enumeration_alias(&decl.type_name, base);
            }
        }
    }
    Ok(lib)
}
