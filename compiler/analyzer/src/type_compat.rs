//! The type-compatibility predicate shared by the semantic rules that compare
//! a value's resolved type against the type a position requires: a function
//! argument against its parameter, a function result against its assignment
//! target, an assignment value against its variable.
//!
//! The rules must agree with each other. A value that is accepted as a
//! function argument but rejected when assigned to a variable of the same
//! type would be a puzzle, so the relation is stated once, here, and every
//! rule asks the same question of it: `are_types_compatible(expected, actual)`.
//!
//! The relation is exact type matching plus the implicit conversions the
//! project has decided to allow: the generic categories (`ANY_INT`,
//! `ANY_NUM`, ...) that standard-library signatures use, untyped-literal
//! inference (ADR-0028), lossless widening (ADR-0029), and the flag-gated
//! cross-family widening (ADR-0031).

use ironplc_dsl::common::*;
use ironplc_parser::options::CompilerOptions;

/// Returns true if `actual` is type-compatible with `expected`.
///
/// Exact matches always pass. If `actual` is a generic type (ANY_INT,
/// ANY_REAL, etc.) and `expected` is a concrete elementary type, delegates
/// to `GenericTypeName::is_compatible_with`.
///
/// Bare integer literals (ANY_INT) are also accepted where REAL or LREAL
/// is expected. This is type inference for untyped literals, not implicit
/// widening of typed expressions (see ADR-0028).
pub(crate) fn are_types_compatible(
    expected: &TypeName,
    actual: &TypeName,
    options: &CompilerOptions,
) -> bool {
    if *expected == *actual {
        return true;
    }
    // Generic expected type (a standard-library parameter such as ANY_REAL,
    // ANY_NUM, or ANY_ELEMENTARY). The concrete or generic actual type must fall
    // within the generic category. See `is_compatible_with_generic_param`.
    if let Ok(expected_generic) = GenericTypeName::try_from(&expected.name) {
        return is_compatible_with_generic_param(&expected_generic, actual, options);
    }
    if let Ok(generic) = GenericTypeName::try_from(&actual.name) {
        if let Ok(elementary) = ElementaryTypeName::try_from(&expected.name) {
            if generic.is_compatible_with(&elementary) {
                return true;
            }
            // Bare integer literals (ANY_INT) can be inferred as REAL/LREAL.
            // See ADR-0028 for rationale.
            if generic == GenericTypeName::AnyInt
                && matches!(
                    elementary,
                    ElementaryTypeName::REAL | ElementaryTypeName::LREAL
                )
            {
                return true;
            }
            // Bare integer literals (ANY_INT) to ANY_BIT types (BYTE, WORD, etc.)
            // requires --allow-cross-family-widening. See ADR-0031.
            if options.allow_cross_family_widening
                && generic == GenericTypeName::AnyInt
                && matches!(
                    elementary,
                    ElementaryTypeName::BYTE
                        | ElementaryTypeName::WORD
                        | ElementaryTypeName::DWORD
                        | ElementaryTypeName::LWORD
                )
            {
                return true;
            }
        }
    }
    // Implicit widening: integer-to-integer, integer-to-real (lossless),
    // bit-string-to-bit-string. See ADR-0029 and ADR-0031.
    if let Ok(actual_elem) = ElementaryTypeName::try_from(&actual.name) {
        if let Ok(expected_elem) = ElementaryTypeName::try_from(&expected.name) {
            if actual_elem.can_widen_to(&expected_elem) {
                return true;
            }
            // Cross-family widening (bit-string → integer) requires flag.
            if options.allow_cross_family_widening
                && actual_elem.can_widen_cross_family_to(&expected_elem)
            {
                return true;
            }
            // Temporal types come in a short and long form (TIME/LTIME,
            // DATE/LDATE, etc.). Duration and date literals always resolve to
            // the canonical short name regardless of the written form, so treat
            // the two widths of a temporal family as interchangeable here.
            if same_temporal_family(&actual_elem, &expected_elem) {
                return true;
            }
        }
    }
    false
}

/// Returns true if both types belong to the same temporal family (the short and
/// long widths of TIME, DATE, TIME_OF_DAY, or DATE_AND_TIME).
fn same_temporal_family(a: &ElementaryTypeName, b: &ElementaryTypeName) -> bool {
    use ElementaryTypeName::*;
    fn family(t: &ElementaryTypeName) -> Option<u8> {
        match t {
            TIME | LTIME => Some(0),
            DATE | LDATE => Some(1),
            TimeOfDay | LTimeOfDay => Some(2),
            DateAndTime | LDateAndTime => Some(3),
            _ => None,
        }
    }
    matches!((family(a), family(b)), (Some(x), Some(y)) if x == y)
}

/// Returns true if `actual` is acceptable where a generic parameter type
/// `expected` (e.g. `ANY_REAL`, `ANY_NUM`) is required.
///
/// Standard-library functions declare their parameters using the IEC 61131-3
/// generic type categories. A concrete argument type is checked with
/// [`GenericTypeName::is_compatible_with`]. A generic argument type (produced for
/// untyped literals — an untyped integer literal is `ANY_INT`, an untyped real
/// literal is `ANY_REAL`) is checked against the parameter category with
/// [`generic_actual_satisfies`].
fn is_compatible_with_generic_param(
    expected: &GenericTypeName,
    actual: &TypeName,
    options: &CompilerOptions,
) -> bool {
    if let Ok(actual_elem) = ElementaryTypeName::try_from(&actual.name) {
        return expected.is_compatible_with(&actual_elem);
    }
    if let Ok(actual_generic) = GenericTypeName::try_from(&actual.name) {
        return generic_actual_satisfies(&actual_generic, expected, options);
    }
    false
}

/// Returns true if a value whose type is the generic category `actual` can be
/// used where the generic category `expected` is required.
///
/// In practice `actual` originates from an untyped literal (`ANY_INT` for integer
/// literals, `ANY_REAL` for real literals) or an unresolved generic function
/// return. The relation models the IEC 61131-3 generic-type hierarchy plus the
/// integer-literal-to-real inference from ADR-0028 and the flag-gated
/// integer-literal-to-bit-string case from ADR-0031.
fn generic_actual_satisfies(
    actual: &GenericTypeName,
    expected: &GenericTypeName,
    options: &CompilerOptions,
) -> bool {
    use GenericTypeName::*;
    if actual == expected {
        return true;
    }
    match expected {
        Any | AnyElementary => true,
        AnyMagnitude => matches!(actual, AnyInt | AnyReal | AnyNum | AnyMagnitude),
        AnyNum => matches!(actual, AnyInt | AnyReal | AnyNum),
        // Integer literals infer as real (ADR-0028).
        AnyReal => matches!(actual, AnyReal | AnyInt),
        AnyInt => matches!(actual, AnyInt),
        // Integer literals to bit-string require the widening flag (ADR-0031).
        AnyBit => {
            matches!(actual, AnyBit) || (options.allow_cross_family_widening && *actual == AnyInt)
        }
        AnyString => matches!(actual, AnyString),
        AnyDate => matches!(actual, AnyDate),
        AnyDerived => false,
    }
}

/// Returns true if the type name is one [`are_types_compatible`] can judge: an
/// elementary type or a generic category (untyped literal). User-defined types
/// (enums, structures, function blocks, arrays, sized strings, references)
/// return false so that a rule can skip them rather than report them.
pub(crate) fn is_checkable_type(type_name: &TypeName) -> bool {
    ElementaryTypeName::try_from(&type_name.name).is_ok()
        || GenericTypeName::try_from(&type_name.name).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn are_types_compatible_when_exact_match_then_true() {
        let opts = CompilerOptions::default();
        assert!(are_types_compatible(
            &TypeName::from("INT"),
            &TypeName::from("INT"),
            &opts,
        ));
    }

    #[test]
    fn are_types_compatible_when_any_int_to_int_then_true() {
        let opts = CompilerOptions::default();
        assert!(are_types_compatible(
            &TypeName::from("INT"),
            &TypeName::from("ANY_INT"),
            &opts,
        ));
    }

    #[test]
    fn are_types_compatible_when_any_int_to_dint_then_true() {
        let opts = CompilerOptions::default();
        assert!(are_types_compatible(
            &TypeName::from("DINT"),
            &TypeName::from("ANY_INT"),
            &opts,
        ));
    }

    #[test]
    fn are_types_compatible_when_any_real_to_real_then_true() {
        let opts = CompilerOptions::default();
        assert!(are_types_compatible(
            &TypeName::from("REAL"),
            &TypeName::from("ANY_REAL"),
            &opts,
        ));
    }

    #[test]
    fn are_types_compatible_when_any_real_to_lreal_then_true() {
        let opts = CompilerOptions::default();
        assert!(are_types_compatible(
            &TypeName::from("LREAL"),
            &TypeName::from("ANY_REAL"),
            &opts,
        ));
    }

    #[test]
    fn are_types_compatible_when_any_int_to_real_then_true() {
        let opts = CompilerOptions::default();
        assert!(are_types_compatible(
            &TypeName::from("REAL"),
            &TypeName::from("ANY_INT"),
            &opts,
        ));
    }

    #[test]
    fn are_types_compatible_when_any_int_to_lreal_then_true() {
        let opts = CompilerOptions::default();
        assert!(are_types_compatible(
            &TypeName::from("LREAL"),
            &TypeName::from("ANY_INT"),
            &opts,
        ));
    }

    #[test]
    fn are_types_compatible_when_dint_to_int_then_false() {
        let opts = CompilerOptions::default();
        assert!(!are_types_compatible(
            &TypeName::from("INT"),
            &TypeName::from("DINT"),
            &opts,
        ));
    }

    #[test]
    fn are_types_compatible_when_int_to_dint_then_true() {
        let opts = CompilerOptions::default();
        assert!(are_types_compatible(
            &TypeName::from("DINT"),
            &TypeName::from("INT"),
            &opts,
        ));
    }

    #[test]
    fn are_types_compatible_when_usint_to_int_then_true() {
        let opts = CompilerOptions::default();
        assert!(are_types_compatible(
            &TypeName::from("INT"),
            &TypeName::from("USINT"),
            &opts,
        ));
    }

    // --- Standard widening tests (ADR-0031) ---

    #[test]
    fn are_types_compatible_when_int_to_real_then_true() {
        let opts = CompilerOptions::default();
        assert!(are_types_compatible(
            &TypeName::from("REAL"),
            &TypeName::from("INT"),
            &opts,
        ));
    }

    #[test]
    fn are_types_compatible_when_dint_to_real_then_false() {
        let opts = CompilerOptions::default();
        assert!(!are_types_compatible(
            &TypeName::from("REAL"),
            &TypeName::from("DINT"),
            &opts,
        ));
    }

    #[test]
    fn are_types_compatible_when_byte_to_word_then_true() {
        let opts = CompilerOptions::default();
        assert!(are_types_compatible(
            &TypeName::from("WORD"),
            &TypeName::from("BYTE"),
            &opts,
        ));
    }

    // --- are_types_compatible: generic expected (stdlib parameters) ---

    #[test]
    fn are_types_compatible_when_any_real_expected_real_actual_then_true() {
        let opts = CompilerOptions::default();
        assert!(are_types_compatible(
            &TypeName::from("ANY_REAL"),
            &TypeName::from("REAL"),
            &opts,
        ));
    }

    #[test]
    fn are_types_compatible_when_any_real_expected_bool_actual_then_false() {
        let opts = CompilerOptions::default();
        assert!(!are_types_compatible(
            &TypeName::from("ANY_REAL"),
            &TypeName::from("BOOL"),
            &opts,
        ));
    }

    #[test]
    fn are_types_compatible_when_any_num_expected_int_actual_then_true() {
        let opts = CompilerOptions::default();
        assert!(are_types_compatible(
            &TypeName::from("ANY_NUM"),
            &TypeName::from("INT"),
            &opts,
        ));
    }

    #[test]
    fn are_types_compatible_when_any_real_expected_any_int_actual_then_true() {
        // Untyped integer literal (ANY_INT) inferred as real (ADR-0028).
        let opts = CompilerOptions::default();
        assert!(are_types_compatible(
            &TypeName::from("ANY_REAL"),
            &TypeName::from("ANY_INT"),
            &opts,
        ));
    }

    #[test]
    fn are_types_compatible_when_any_int_expected_any_real_actual_then_false() {
        let opts = CompilerOptions::default();
        assert!(!are_types_compatible(
            &TypeName::from("ANY_INT"),
            &TypeName::from("ANY_REAL"),
            &opts,
        ));
    }
}
