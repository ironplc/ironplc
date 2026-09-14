//! Rule that a declaration name is used at most once in a library.
//!
//! Functions, function blocks, programs, configurations, interfaces and data
//! types share one namespace, so a second declaration of a name is reported
//! whatever kinds the two declarations are. The code says which kind the
//! later declaration is: `P4016` for a function that repeats a function,
//! `P2007` for a data type that repeats a data type, and `P4013` otherwise.
//!
//! This rule runs on the merged library *before* `xform_toposort_declarations`
//! rather than with the other semantic rules, because the toposort keeps one
//! declaration per name and every later pass sees only that one. An activated
//! compatibility library's declarations are part of the merge, so a user
//! declaration that repeats a library declaration is reported here like any
//! other duplicate (`REQ-CL-analyzer-007`).
//!
//! ## Passes
//!
//! ```ignore
//! FUNCTION_BLOCK Callee
//!    VAR
//!       IN1: BOOL;
//!    END_VAR
//! END_FUNCTION_BLOCK
//!
//! FUNCTION_BLOCK Caller
//!    VAR
//!       CalleeInstance : Callee;
//!    END_VAR
//! END_FUNCTION_BLOCK
//! ```
//!
//! ## Fails
//!
//! ```ignore
//! FUNCTION_BLOCK Fb
//!    VAR
//!       X : BOOL;
//!    END_VAR
//! END_FUNCTION_BLOCK
//!
//! FUNCTION_BLOCK Fb
//!    VAR
//!       Y : BOOL;
//!    END_VAR
//! END_FUNCTION_BLOCK
//! ```
use std::collections::HashMap;

use ironplc_dsl::{
    common::{DataTypeDeclarationKind, Library, LibraryElementKind},
    core::{Id, Located},
    diagnostic::{Diagnostic, Label},
};
use ironplc_problems::Problem;

/// Reports every declaration whose name an earlier declaration already uses.
pub fn apply(lib: &Library) -> Vec<Diagnostic> {
    let mut seen: HashMap<&Id, DeclKind> = HashMap::new();
    let mut diagnostics = Vec::new();

    for element in &lib.elements {
        let Some((name, kind)) = name_and_kind(element) else {
            continue;
        };
        match seen.get(name) {
            Some(first) => {
                let problem = match (first, &kind) {
                    (DeclKind::Function, DeclKind::Function) => Problem::FunctionDeclNameDuplicated,
                    (DeclKind::DataType, DeclKind::DataType) => Problem::TypeDeclNameDuplicated,
                    _ => Problem::PouDeclNameDuplicated,
                };
                diagnostics.push(
                    Diagnostic::problem(
                        problem,
                        Label::span(name.span(), "Declaration repeats an earlier name"),
                    )
                    .with_context_id("name", name)
                    .with_secondary(Label::span(seen_span(&seen, name), "First declaration")),
                );
            }
            None => {
                seen.insert(name, kind);
            }
        }
    }

    diagnostics
}

/// The kinds that pick the problem code; every other kind reports `P4013`.
#[derive(Clone, Copy)]
enum DeclKind {
    Function,
    DataType,
    Pou,
}

/// The declared name of a library element, with its kind, or `None` for an
/// element that declares no name (a `VAR_GLOBAL` block).
fn name_and_kind(element: &LibraryElementKind) -> Option<(&Id, DeclKind)> {
    match element {
        LibraryElementKind::DataTypeDeclaration(decl) => {
            let name = match decl {
                DataTypeDeclarationKind::Enumeration(d) => &d.type_name.name,
                DataTypeDeclarationKind::Subrange(d) => &d.type_name.name,
                DataTypeDeclarationKind::Simple(d) => &d.type_name.name,
                DataTypeDeclarationKind::Array(d) => &d.type_name.name,
                DataTypeDeclarationKind::Structure(d) => &d.type_name.name,
                DataTypeDeclarationKind::StructureInitialization(d) => &d.type_name.name,
                DataTypeDeclarationKind::String(d) => &d.type_name.name,
                DataTypeDeclarationKind::Reference(d) => &d.type_name.name,
                DataTypeDeclarationKind::LateBound(d) => &d.data_type_name.name,
            };
            Some((name, DeclKind::DataType))
        }
        LibraryElementKind::FunctionDeclaration(decl) => Some((&decl.name, DeclKind::Function)),
        LibraryElementKind::FunctionBlockDeclaration(decl) => {
            Some((&decl.name.name, DeclKind::Pou))
        }
        LibraryElementKind::ProgramDeclaration(decl) => Some((&decl.name, DeclKind::Pou)),
        LibraryElementKind::ConfigurationDeclaration(decl) => Some((&decl.name, DeclKind::Pou)),
        LibraryElementKind::InterfaceDeclaration(decl) => Some((&decl.name, DeclKind::Pou)),
        LibraryElementKind::GlobalVarDeclarations(_) => None,
    }
}

/// The span of the first declaration of `name`. `Id` compares
/// case-insensitively, so the key held in the map is the earlier spelling.
fn seen_span(seen: &HashMap<&Id, DeclKind>, name: &Id) -> ironplc_dsl::core::SourceSpan {
    seen.get_key_value(name)
        .map(|(first, _)| first.span())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::apply;
    use crate::test_helpers::parse_only;
    use ironplc_problems::Problem;

    /// The problem codes the rule reports for `program`, in order.
    fn codes(program: &str) -> Vec<String> {
        apply(&parse_only(program))
            .into_iter()
            .map(|d| d.code)
            .collect()
    }

    #[test]
    fn apply_when_names_unique_then_no_diagnostics() {
        assert!(codes(
            "
        FUNCTION_BLOCK Callee
            VAR
                IN1 : BOOL;
            END_VAR
        END_FUNCTION_BLOCK

        FUNCTION_BLOCK Caller
            VAR
                CalleeInstance : Callee;
            END_VAR
        END_FUNCTION_BLOCK"
        )
        .is_empty());
    }

    #[test]
    fn apply_when_function_repeats_function_then_p4016() {
        assert_eq!(
            codes(
                "
        FUNCTION Foo : BOOL
            Foo := FALSE;
        END_FUNCTION

        FUNCTION Foo : BOOL
            Foo := TRUE;
        END_FUNCTION"
            ),
            [Problem::FunctionDeclNameDuplicated.code()]
        );
    }

    #[test]
    fn apply_when_function_block_repeats_function_block_then_p4013() {
        assert_eq!(
            codes(
                "
        FUNCTION_BLOCK Bar
            VAR
                X : BOOL;
            END_VAR
        END_FUNCTION_BLOCK

        FUNCTION_BLOCK Bar
            VAR
                Y : BOOL;
            END_VAR
        END_FUNCTION_BLOCK"
            ),
            [Problem::PouDeclNameDuplicated.code()]
        );
    }

    #[test]
    fn apply_when_program_repeats_program_then_p4013() {
        assert_eq!(
            codes(
                "
        PROGRAM Baz
            VAR
                X : BOOL;
            END_VAR
        END_PROGRAM

        PROGRAM Baz
            VAR
                Y : BOOL;
            END_VAR
        END_PROGRAM"
            ),
            [Problem::PouDeclNameDuplicated.code()]
        );
    }

    #[test]
    fn apply_when_configuration_repeats_configuration_then_p4013() {
        assert_eq!(
            codes(
                "
        PROGRAM Prg1
            VAR
                X : BOOL;
            END_VAR
        END_PROGRAM

        CONFIGURATION Cfg1
            RESOURCE Res1 ON PLC
                TASK Main(INTERVAL := T#20ms, PRIORITY := 1);
                PROGRAM P1 WITH Main : Prg1;
            END_RESOURCE
        END_CONFIGURATION

        CONFIGURATION Cfg1
            RESOURCE Res2 ON PLC
                TASK Main(INTERVAL := T#20ms, PRIORITY := 1);
                PROGRAM P2 WITH Main : Prg1;
            END_RESOURCE
        END_CONFIGURATION"
            ),
            [Problem::PouDeclNameDuplicated.code()]
        );
    }

    #[test]
    fn apply_when_function_block_repeats_function_then_p4013() {
        assert_eq!(
            codes(
                "
        FUNCTION Compute : INT
            Compute := 0;
        END_FUNCTION

        FUNCTION_BLOCK Compute
            VAR
                X : INT;
            END_VAR
        END_FUNCTION_BLOCK"
            ),
            [Problem::PouDeclNameDuplicated.code()]
        );
    }

    #[test]
    fn apply_when_type_repeats_type_then_p2007() {
        assert_eq!(
            codes(
                "
        TYPE
            the_struct : STRUCT
                member : BOOL;
            END_STRUCT;
            the_struct : STRUCT
                member : BOOL;
            END_STRUCT;
        END_TYPE"
            ),
            [Problem::TypeDeclNameDuplicated.code()]
        );
    }

    #[test]
    fn apply_when_function_block_repeats_type_then_p4013() {
        assert_eq!(
            codes(
                "
        TYPE
            Shared : INT := 0;
        END_TYPE

        FUNCTION_BLOCK Shared
            VAR
                X : INT;
            END_VAR
        END_FUNCTION_BLOCK"
            ),
            [Problem::PouDeclNameDuplicated.code()]
        );
    }

    #[test]
    fn apply_when_names_differ_only_in_case_then_reported() {
        assert_eq!(
            codes(
                "
        FUNCTION foo : BOOL
            foo := FALSE;
        END_FUNCTION

        FUNCTION FOO : BOOL
            FOO := TRUE;
        END_FUNCTION"
            ),
            [Problem::FunctionDeclNameDuplicated.code()]
        );
    }

    #[test]
    fn apply_when_name_declared_three_times_then_reports_each_later_one() {
        assert_eq!(
            codes(
                "
        FUNCTION Foo : BOOL
            Foo := FALSE;
        END_FUNCTION

        FUNCTION Foo : BOOL
            Foo := TRUE;
        END_FUNCTION

        FUNCTION Foo : BOOL
            Foo := TRUE;
        END_FUNCTION"
            )
            .len(),
            2
        );
    }

    /// The primary label is on the later declaration and the secondary on
    /// the first, so the user sees both.
    #[test]
    fn apply_when_duplicate_then_labels_point_at_both_declarations() {
        let program = "
        FUNCTION Foo : BOOL
            Foo := FALSE;
        END_FUNCTION

        FUNCTION Foo : BOOL
            Foo := TRUE;
        END_FUNCTION";
        let diagnostics = apply(&parse_only(program));
        assert_eq!(diagnostics.len(), 1);
        let first = program.find("Foo").unwrap();
        let second = program.rfind("FUNCTION Foo").unwrap() + "FUNCTION ".len();
        assert_eq!(diagnostics[0].primary.location.start, second);
        assert_eq!(diagnostics[0].secondary[0].location.start, first);
    }
}
