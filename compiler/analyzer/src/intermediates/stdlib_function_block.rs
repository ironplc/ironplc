//! Standard library function block definitions for IEC 61131-3.
//!
//! This module defines the standard library function blocks specified in
//! IEC 61131-3 Section 2.5.2.3, including:
//! - Bistable function blocks (SR, RS)
//! - Edge detection (R_TRIG, F_TRIG)
//! - Counters (CTU, CTD, CTUD)
//! - Timers (TON, TOF, TP)
//!
//! These function blocks are automatically available in the type environment
//! and do not need to be declared by the user.

use ironplc_dsl::core::{Id, SourceSpan};

use crate::intermediate_type::{
    ByteSized, FunctionBlockVarType, IntermediateStructField, IntermediateType,
};
use crate::type_attributes::TypeAttributes;

// Type constants for common types used in stdlib function blocks
fn bool_type() -> IntermediateType {
    IntermediateType::Bool
}

fn time_type() -> IntermediateType {
    IntermediateType::Time
}

/// Integer type variants for counter function blocks.
/// Each variant specifies the type name suffix and the IntermediateType for PV/CV fields.
#[allow(clippy::type_complexity)]
const COUNTER_INT_VARIANTS: &[(&str, fn() -> IntermediateType)] = &[
    ("", || IntermediateType::Int {
        size: ByteSized::B16,
    }), // INT (default)
    ("_DINT", || IntermediateType::Int {
        size: ByteSized::B32,
    }), // DINT
    ("_LINT", || IntermediateType::Int {
        size: ByteSized::B64,
    }), // LINT
    ("_UDINT", || IntermediateType::UInt {
        size: ByteSized::B32,
    }), // UDINT
    ("_ULINT", || IntermediateType::UInt {
        size: ByteSized::B64,
    }), // ULINT
];

/// Builds an IntermediateStructField with proper offset calculation.
fn build_field(
    name: &str,
    field_type: IntermediateType,
    var_type: FunctionBlockVarType,
    current_offset: &mut u32,
) -> IntermediateStructField {
    // Calculate alignment
    let alignment = field_type.alignment_bytes() as u32;
    let aligned_offset = if alignment == 0 {
        *current_offset
    } else {
        current_offset.div_ceil(alignment) * alignment
    };

    // Calculate size
    let size = field_type.size_in_bytes().unwrap_or(0) as u32;

    let field = IntermediateStructField {
        name: Id::from(name),
        field_type,
        offset: aligned_offset,
        var_type: Some(var_type),
        has_default: false, // Function block fields don't have defaults in the const sense
    };

    *current_offset = aligned_offset + size;
    field
}

/// Builds TypeAttributes for a standard library function block.
fn build_function_block(
    name: &str,
    field_defs: &[(&str, IntermediateType, FunctionBlockVarType)],
) -> TypeAttributes {
    let mut current_offset = 0u32;
    let fields: Vec<IntermediateStructField> = field_defs
        .iter()
        .map(|(name, field_type, var_type)| {
            build_field(name, field_type.clone(), *var_type, &mut current_offset)
        })
        .collect();

    TypeAttributes::new(
        SourceSpan::builtin(),
        IntermediateType::FunctionBlock {
            name: name.to_string(),
            fields,
        },
    )
}

// =============================================================================
// Bistable Function Blocks (IEC 61131-3 Section 2.5.2.3.1)
// =============================================================================

/// Creates the SR (Set-Reset) bistable function block.
///
/// The SR function block is a bistable element where Set dominates.
/// Q1 := S1 OR (NOT R AND Q1)
fn build_sr() -> TypeAttributes {
    use FunctionBlockVarType::*;
    build_function_block(
        "SR",
        &[
            ("S1", bool_type(), Input),  // Set input (dominant)
            ("R", bool_type(), Input),   // Reset input
            ("Q1", bool_type(), Output), // Output
        ],
    )
}

/// Creates the RS (Reset-Set) bistable function block.
///
/// The RS function block is a bistable element where Reset dominates.
/// Q1 := NOT R1 AND (S OR Q1)
fn build_rs() -> TypeAttributes {
    use FunctionBlockVarType::*;
    build_function_block(
        "RS",
        &[
            ("S", bool_type(), Input),   // Set input
            ("R1", bool_type(), Input),  // Reset input (dominant)
            ("Q1", bool_type(), Output), // Output
        ],
    )
}

// =============================================================================
// Edge Detection Function Blocks (IEC 61131-3 Section 2.5.2.3.2)
// =============================================================================

/// Creates the R_TRIG (Rising Edge Trigger) function block.
///
/// Produces a single pulse when the input changes from FALSE to TRUE.
fn build_r_trig() -> TypeAttributes {
    use FunctionBlockVarType::*;
    build_function_block(
        "R_TRIG",
        &[
            ("CLK", bool_type(), Input),  // Clock input
            ("Q", bool_type(), Output),   // Output pulse
            ("M", bool_type(), Internal), // Internal memory (previous CLK state)
        ],
    )
}

/// Creates the F_TRIG (Falling Edge Trigger) function block.
///
/// Produces a single pulse when the input changes from TRUE to FALSE.
fn build_f_trig() -> TypeAttributes {
    use FunctionBlockVarType::*;
    build_function_block(
        "F_TRIG",
        &[
            ("CLK", bool_type(), Input),  // Clock input
            ("Q", bool_type(), Output),   // Output pulse
            ("M", bool_type(), Internal), // Internal memory (previous CLK state)
        ],
    )
}

// =============================================================================
// Counter Function Blocks (IEC 61131-3 Section 2.5.2.3.3)
// =============================================================================

/// Creates a CTU (Count Up) function block with the specified integer type.
///
/// Counts up on rising edge of CU input until CV >= PV.
/// The `suffix` is appended to "CTU" (e.g., "_DINT" for CTU_DINT).
fn build_ctu_variant(suffix: &str, int_type: IntermediateType) -> TypeAttributes {
    use FunctionBlockVarType::*;
    build_function_block(
        &format!("CTU{}", suffix),
        &[
            ("CU", bool_type(), Input),      // Count up input (R_EDGE)
            ("R", bool_type(), Input),       // Reset input
            ("PV", int_type.clone(), Input), // Preset value
            ("Q", bool_type(), Output),      // Output (CV >= PV)
            ("CV", int_type, Output),        // Current value
        ],
    )
}

/// Creates a CTD (Count Down) function block with the specified integer type.
///
/// Counts down on rising edge of CD input until CV <= 0.
/// The `suffix` is appended to "CTD" (e.g., "_DINT" for CTD_DINT).
fn build_ctd_variant(suffix: &str, int_type: IntermediateType) -> TypeAttributes {
    use FunctionBlockVarType::*;
    build_function_block(
        &format!("CTD{}", suffix),
        &[
            ("CD", bool_type(), Input),      // Count down input (R_EDGE)
            ("LD", bool_type(), Input),      // Load input
            ("PV", int_type.clone(), Input), // Preset value
            ("Q", bool_type(), Output),      // Output (CV <= 0)
            ("CV", int_type, Output),        // Current value
        ],
    )
}

/// Creates a CTUD (Count Up/Down) function block with the specified integer type.
///
/// Counts up on CU rising edge, down on CD rising edge.
/// The `suffix` is appended to "CTUD" (e.g., "_DINT" for CTUD_DINT).
fn build_ctud_variant(suffix: &str, int_type: IntermediateType) -> TypeAttributes {
    use FunctionBlockVarType::*;
    build_function_block(
        &format!("CTUD{}", suffix),
        &[
            ("CU", bool_type(), Input),      // Count up input (R_EDGE)
            ("CD", bool_type(), Input),      // Count down input (R_EDGE)
            ("R", bool_type(), Input),       // Reset input
            ("LD", bool_type(), Input),      // Load input
            ("PV", int_type.clone(), Input), // Preset value
            ("QU", bool_type(), Output),     // Up output (CV >= PV)
            ("QD", bool_type(), Output),     // Down output (CV <= 0)
            ("CV", int_type, Output),        // Current value
        ],
    )
}

/// Generates all counter function block variants (CTU, CTD, CTUD with all integer types).
fn get_all_counter_variants() -> Vec<(&'static str, TypeAttributes)> {
    let mut counters = Vec::new();
    for (suffix, int_type_fn) in COUNTER_INT_VARIANTS {
        let int_type = int_type_fn();
        counters.push((
            leak_lowercase(&format!("ctu{}", suffix)),
            build_ctu_variant(suffix, int_type.clone()),
        ));
        counters.push((
            leak_lowercase(&format!("ctd{}", suffix)),
            build_ctd_variant(suffix, int_type.clone()),
        ));
        counters.push((
            leak_lowercase(&format!("ctud{}", suffix)),
            build_ctud_variant(suffix, int_type),
        ));
    }
    counters
}

/// Leaks a string to get a static lifetime (used for registry keys).
/// This is acceptable because stdlib types are created once at startup.
fn leak_lowercase(s: &str) -> &'static str {
    Box::leak(s.to_lowercase().into_boxed_str())
}

// =============================================================================
// Timer Function Blocks (IEC 61131-3 Section 2.5.2.3.4)
// =============================================================================

/// Creates the TON (On-Delay Timer) function block.
///
/// Q is TRUE after IN has been TRUE for duration PT.
/// ET shows elapsed time while timing.
fn build_ton() -> TypeAttributes {
    use FunctionBlockVarType::*;
    build_function_block(
        "TON",
        &[
            ("IN", bool_type(), Input),  // Timer input
            ("PT", time_type(), Input),  // Preset time
            ("Q", bool_type(), Output),  // Timer output
            ("ET", time_type(), Output), // Elapsed time
        ],
    )
}

/// Creates the TOF (Off-Delay Timer) function block.
///
/// Q goes FALSE after IN has been FALSE for duration PT.
/// ET shows elapsed time while timing.
fn build_tof() -> TypeAttributes {
    use FunctionBlockVarType::*;
    build_function_block(
        "TOF",
        &[
            ("IN", bool_type(), Input),  // Timer input
            ("PT", time_type(), Input),  // Preset time
            ("Q", bool_type(), Output),  // Timer output
            ("ET", time_type(), Output), // Elapsed time
        ],
    )
}

/// Creates the TP (Pulse Timer) function block.
///
/// Generates a pulse of duration PT when triggered by rising edge of IN.
fn build_tp() -> TypeAttributes {
    use FunctionBlockVarType::*;
    build_function_block(
        "TP",
        &[
            ("IN", bool_type(), Input),  // Timer input
            ("PT", time_type(), Input),  // Preset time (pulse duration)
            ("Q", bool_type(), Output),  // Timer output
            ("ET", time_type(), Output), // Elapsed time
        ],
    )
}

// =============================================================================
// Public API
// =============================================================================

/// Returns all standard library function block definitions.
///
/// Each tuple contains (lowercase_name, TypeAttributes).
/// The lowercase name is used for case-insensitive lookup in the type environment.
pub fn get_all_stdlib_function_blocks() -> Vec<(&'static str, TypeAttributes)> {
    let mut fbs = vec![
        // Bistable
        ("sr", build_sr()),
        ("rs", build_rs()),
        // Edge detection
        ("r_trig", build_r_trig()),
        ("f_trig", build_f_trig()),
        // Timers
        ("ton", build_ton()),
        ("tof", build_tof()),
        ("tp", build_tp()),
    ];
    // Counters (all integer type variants)
    fbs.extend(get_all_counter_variants());
    fbs
}

/// Returns the set of standard library function block names.
///
/// This is useful for checking if a type name is a standard library type.
pub fn stdlib_function_block_names() -> &'static [&'static str] {
    &[
        // Bistable
        "SR",
        "RS",
        // Edge detection
        "R_TRIG",
        "F_TRIG",
        // Counters (all integer type variants)
        "CTU",
        "CTU_DINT",
        "CTU_LINT",
        "CTU_UDINT",
        "CTU_ULINT",
        "CTD",
        "CTD_DINT",
        "CTD_LINT",
        "CTD_UDINT",
        "CTD_ULINT",
        "CTUD",
        "CTUD_DINT",
        "CTUD_LINT",
        "CTUD_UDINT",
        "CTUD_ULINT",
        // Timers
        "TON",
        "TOF",
        "TP",
    ]
}

/// Checks if a name is a standard library function block.
///
/// Uses `Id` equality which is case-insensitive per IEC 61131-3.
pub fn is_stdlib_function_block(name: &Id) -> bool {
    stdlib_function_block_names()
        .iter()
        .any(|stdlib_name| *name == Id::from(stdlib_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_all_stdlib_function_blocks_returns_expected_count() {
        let fbs = get_all_stdlib_function_blocks();
        // 2 bistable + 2 edge detection + 3 timers + 15 counters (3 types × 5 int variants)
        assert_eq!(fbs.len(), 22);
    }

    #[test]
    fn build_ton_when_called_then_has_correct_inputs_and_outputs() {
        let ton = build_ton();
        if let IntermediateType::FunctionBlock { name, fields } = &ton.representation {
            assert_eq!(name, "TON");
            assert_eq!(fields.len(), 4);

            // Check inputs
            assert_eq!(fields[0].name.original(), "IN");
            assert_eq!(fields[0].var_type, Some(FunctionBlockVarType::Input));

            assert_eq!(fields[1].name.original(), "PT");
            assert_eq!(fields[1].var_type, Some(FunctionBlockVarType::Input));

            // Check outputs
            assert_eq!(fields[2].name.original(), "Q");
            assert_eq!(fields[2].var_type, Some(FunctionBlockVarType::Output));

            assert_eq!(fields[3].name.original(), "ET");
            assert_eq!(fields[3].var_type, Some(FunctionBlockVarType::Output));
        } else {
            panic!("Expected FunctionBlock type");
        }
    }

    #[test]
    fn build_ctu_variant_when_called_then_has_correct_fields() {
        let ctu = build_ctu_variant(
            "",
            IntermediateType::Int {
                size: ByteSized::B16,
            },
        );
        if let IntermediateType::FunctionBlock { name, fields } = &ctu.representation {
            assert_eq!(name, "CTU");
            assert_eq!(fields.len(), 5);

            // Check inputs: CU, R, PV
            assert_eq!(fields[0].name.original(), "CU");
            assert_eq!(fields[1].name.original(), "R");
            assert_eq!(fields[2].name.original(), "PV");

            // Check outputs: Q, CV
            assert_eq!(fields[3].name.original(), "Q");
            assert_eq!(fields[4].name.original(), "CV");
        } else {
            panic!("Expected FunctionBlock type");
        }
    }

    #[test]
    fn build_ctu_variant_when_dint_then_has_correct_name_and_type() {
        let ctu_dint = build_ctu_variant(
            "_DINT",
            IntermediateType::Int {
                size: ByteSized::B32,
            },
        );
        if let IntermediateType::FunctionBlock { name, fields } = &ctu_dint.representation {
            assert_eq!(name, "CTU_DINT");

            // Check PV field has DINT type
            let pv_field = fields.iter().find(|f| f.name.original() == "PV").unwrap();
            assert_eq!(
                pv_field.field_type,
                IntermediateType::Int {
                    size: ByteSized::B32
                }
            );

            // Check CV field has DINT type
            let cv_field = fields.iter().find(|f| f.name.original() == "CV").unwrap();
            assert_eq!(
                cv_field.field_type,
                IntermediateType::Int {
                    size: ByteSized::B32
                }
            );
        } else {
            panic!("Expected FunctionBlock type");
        }
    }

    #[test]
    fn build_r_trig_when_called_then_has_internal_memory() {
        let r_trig = build_r_trig();
        if let IntermediateType::FunctionBlock { fields, .. } = &r_trig.representation {
            // Check that M is internal
            let m_field = fields.iter().find(|f| f.name.original() == "M").unwrap();
            assert_eq!(m_field.var_type, Some(FunctionBlockVarType::Internal));
        } else {
            panic!("Expected FunctionBlock type");
        }
    }

    #[test]
    fn is_stdlib_function_block_when_ton_then_true() {
        assert!(is_stdlib_function_block(&Id::from("ton")));
        assert!(is_stdlib_function_block(&Id::from("TON")));
        assert!(is_stdlib_function_block(&Id::from("Ton")));
    }

    #[test]
    fn is_stdlib_function_block_when_unknown_then_false() {
        assert!(!is_stdlib_function_block(&Id::from("my_custom_fb")));
    }

    #[test]
    fn stdlib_function_blocks_have_builtin_span() {
        for (_, fb) in get_all_stdlib_function_blocks() {
            assert!(fb.span.is_builtin(), "Expected builtin span for stdlib FB");
        }
    }
}
