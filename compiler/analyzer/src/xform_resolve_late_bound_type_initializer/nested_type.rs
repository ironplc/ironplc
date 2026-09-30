//! Resolves the type named inside a variable's type: the element type of
//! `ARRAY ... OF` and the referenced type of `REF_TO`, `REFERENCE TO` and
//! `POINTER TO`.
//!
//! A `TYPE` declaration's element and referenced types are resolved while the
//! type environment is built, and a variable whose own type is undeclared
//! (`x : E_Missing`, P2008) while its initializer is resolved. A type written
//! in place in a variable declaration is resolved here, in the same pass, with
//! the helpers `TYPE` declarations use, so the same mistake gets the same
//! problem code wherever it is written: P2013 for an array element type, P2011
//! for a referenced type. Only these type names are resolved; array bounds are
//! checked elsewhere.
//!
//! ```ignore
//! PROGRAM main
//! VAR
//!     modes : ARRAY[1..2] OF E_Mode;   (* P2013 when E_Mode is undeclared *)
//!     mode : REF_TO E_Mode;            (* P2011 when E_Mode is undeclared *)
//! END_VAR
//! END_PROGRAM
//! ```
use ironplc_dsl::common::*;
use ironplc_dsl::core::{Id, Located};
use ironplc_dsl::diagnostic::Diagnostic;

use crate::intermediates::array;
use crate::type_environment::TypeEnvironment;

/// Resolves the element or referenced type named inside the type of `decl`,
/// returning the diagnostic when that type is not declared. The variable is
/// the diagnostic's primary label, as the type name is for a `TYPE`
/// declaration.
pub(super) fn resolve(types: &TypeEnvironment, decl: &VarDecl) -> Result<(), Diagnostic> {
    let declaring = TypeName::from_id(
        &Id::from(&decl.identifier.to_string()).with_position(decl.identifier.span()),
    );
    match &decl.initializer {
        InitialValueAssignmentKind::Array(ArrayInitialValueAssignment {
            spec: SpecificationKind::Inline(subranges),
            ..
        }) => array::element_type(&declaring, subranges, types).map(|_| ()),
        InitialValueAssignmentKind::Reference(reference) => match &reference.target {
            ReferenceTarget::Named(_) => types
                .resolve_reference_target(&declaring, &reference.target)
                .map(|_| ()),
            ReferenceTarget::Array(subranges) => {
                array::element_type(&declaring, subranges, types).map(|_| ())
            }
        },
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use crate::test_helpers::parse_and_resolve_types_with_options;
    use ironplc_parser::options::{CompilerOptions, Dialect};
    use ironplc_problems::Problem;

    /// The codes type resolution reports for `program` under `dialect`.
    fn codes(dialect: Dialect, program: &str) -> Vec<String> {
        let options = CompilerOptions::from_dialect(dialect);
        let (_, context) = parse_and_resolve_types_with_options(program, &options);
        context
            .diagnostics()
            .iter()
            .map(|d| d.code.clone())
            .collect()
    }

    /// A program declaring `x` as `declaration`.
    fn program_with(declaration: &str) -> String {
        format!(
            "
PROGRAM main
VAR
  x : {declaration};
  y : INT;
END_VAR
y := 1;
END_PROGRAM"
        )
    }

    #[rstest::rstest]
    #[case::array_ed2(Dialect::Iec61131_3Ed2, "ARRAY[1..2] OF E_Missing")]
    #[case::array_twincat(Dialect::TwinCat, "ARRAY[1..2] OF E_Missing")]
    #[case::array_of_ref_to(Dialect::Iec61131_3Ed3, "ARRAY[1..2] OF REF_TO E_Missing")]
    #[case::array_of_pointer_to(Dialect::TwinCat, "ARRAY[1..2] OF POINTER TO E_Missing")]
    #[case::ref_to_array(Dialect::Iec61131_3Ed3, "REF_TO ARRAY[1..2] OF E_Missing")]
    fn resolve_types_when_array_element_type_undeclared_then_p2013(
        #[case] dialect: Dialect,
        #[case] declaration: &str,
    ) {
        let codes = codes(dialect, &program_with(declaration));
        assert_eq!(codes, vec![Problem::ArrayElementTypeNotDeclared.code()]);
    }

    #[rstest::rstest]
    #[case::ref_to(Dialect::Iec61131_3Ed3, "REF_TO E_Missing")]
    #[case::pointer_to(Dialect::TwinCat, "POINTER TO E_Missing")]
    #[case::reference_to(Dialect::TwinCat, "REFERENCE TO E_Missing")]
    fn resolve_types_when_referenced_type_undeclared_then_p2011(
        #[case] dialect: Dialect,
        #[case] declaration: &str,
    ) {
        let codes = codes(dialect, &program_with(declaration));
        assert_eq!(codes, vec![Problem::ParentTypeNotDeclared.code()]);
    }

    #[test]
    fn resolve_types_when_undeclared_in_function_block_and_function_parameters_then_each_reported()
    {
        let mut codes = codes(
            Dialect::TwinCat,
            "
FUNCTION_BLOCK FB
VAR_INPUT
  a : ARRAY[1..2] OF E_Missing;
END_VAR
VAR_OUTPUT
  p : POINTER TO E_Missing;
END_VAR
END_FUNCTION_BLOCK

FUNCTION F : INT
VAR_INPUT
  r : REFERENCE TO E_Missing;
END_VAR
F := 1;
END_FUNCTION",
        );
        // Declarations are reordered before analysis; compare as a multiset.
        codes.sort();
        assert_eq!(
            codes,
            vec![
                Problem::ParentTypeNotDeclared.code(),
                Problem::ParentTypeNotDeclared.code(),
                Problem::ArrayElementTypeNotDeclared.code(),
            ]
        );
    }

    #[test]
    fn resolve_types_when_undeclared_in_configuration_global_then_p2013() {
        let codes = codes(
            Dialect::Iec61131_3Ed2,
            "
CONFIGURATION config
VAR_GLOBAL
  g : ARRAY[1..2] OF E_Missing;
END_VAR
RESOURCE res ON PLC
TASK plc_task(INTERVAL := T#10ms, PRIORITY := 1);
PROGRAM plc_task_instance WITH plc_task : main;
END_RESOURCE
END_CONFIGURATION

PROGRAM main
VAR
  y : INT;
END_VAR
y := 1;
END_PROGRAM",
        );
        assert_eq!(codes, vec![Problem::ArrayElementTypeNotDeclared.code()]);
    }

    #[test]
    fn resolve_types_when_edition_3_types_declared_then_ok() {
        let codes = codes(
            Dialect::Iec61131_3Ed3,
            "
TYPE
  E_Mode : (Idle, Running);
END_TYPE

FUNCTION_BLOCK FB
END_FUNCTION_BLOCK

PROGRAM main
VAR
  modes : ARRAY[1..2] OF E_Mode;
  mode : REF_TO E_Mode;
  mode_array : REF_TO ARRAY[1..2] OF E_Mode;
  mode_refs : ARRAY[1..2] OF REF_TO E_Mode;
  names : ARRAY[1..2] OF STRING[4];
  wide_names : ARRAY[1..2] OF WSTRING;
  blocks : ARRAY[1..2] OF FB;
  timers : ARRAY[1..2] OF TON;
  later : ARRAY[1..2] OF S_Later;
  later_ref : REF_TO S_Later;
  y : INT;
END_VAR
y := 1;
END_PROGRAM

TYPE
  S_Later : STRUCT a : INT; END_STRUCT;
END_TYPE",
        );
        assert!(codes.is_empty(), "{codes:?}");
    }

    #[test]
    fn resolve_types_when_twincat_types_declared_then_ok() {
        let codes = codes(
            Dialect::TwinCat,
            "
TYPE
  E_Mode : (Idle, Running);
END_TYPE

FUNCTION_BLOCK FB
END_FUNCTION_BLOCK

PROGRAM main
VAR
  mode_pointer : POINTER TO E_Mode;
  mode_reference : REFERENCE TO E_Mode;
  mode_pointers : ARRAY[1..2] OF POINTER TO E_Mode;
  block_pointer : POINTER TO FB;
  byte_pointer : POINTER TO BYTE;
  y : INT;
END_VAR
y := 1;
END_PROGRAM",
        );
        assert!(codes.is_empty(), "{codes:?}");
    }
}
