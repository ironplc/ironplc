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

use ironplc_dsl::core::Id;

use crate::semantic_type::{ByteSized, FunctionBlockVarType, SemanticType};
use crate::standard_function_block::{CounterWidth, StandardFunctionBlock};
use crate::type_attributes::TypeAttributes;

// Type constants for common types used in stdlib function blocks
fn bool_type() -> SemanticType {
    SemanticType::Bool
}

fn time_type() -> SemanticType {
    SemanticType::Time {
        size: ByteSized::B32,
    }
}

/// Integer type variants for counter function blocks.
/// Each variant specifies the type name suffix, the width it counts in and
/// the SemanticType for PV/CV fields.
#[allow(clippy::type_complexity)]
const COUNTER_INT_VARIANTS: &[(&str, CounterWidth, fn() -> SemanticType)] = &[
    ("", CounterWidth::Int, || SemanticType::Int {
        size: ByteSized::B16,
    }), // INT (default)
    ("_DINT", CounterWidth::Dint, || SemanticType::Int {
        size: ByteSized::B32,
    }), // DINT
    ("_LINT", CounterWidth::Lint, || SemanticType::Int {
        size: ByteSized::B64,
    }), // LINT
    ("_UDINT", CounterWidth::Udint, || SemanticType::UInt {
        size: ByteSized::B32,
    }), // UDINT
    ("_ULINT", CounterWidth::Ulint, || SemanticType::UInt {
        size: ByteSized::B64,
    }), // ULINT
];

/// A standard function block: its type, and its fields, which the type
/// environment records by the block's id rather than in its type (see
/// `TypeEnvironment::block_fields`).
pub struct StandardBlock {
    pub attributes: TypeAttributes,
    /// The block's inputs, outputs and internal variables, in order.
    pub fields: Vec<StandardField>,
}

/// A field of a standard function block, every one of which has an
/// elementary type.
pub struct StandardField {
    pub name: Id,
    pub field_type: SemanticType,
    pub var_type: FunctionBlockVarType,
}

/// Builds the standard library function block `block`.
fn build_function_block(
    block: StandardFunctionBlock,
    name: &str,
    field_defs: &[(&str, SemanticType, FunctionBlockVarType)],
) -> StandardBlock {
    let fields = field_defs
        .iter()
        .map(|(name, field_type, var_type)| StandardField {
            name: Id::from(name),
            field_type: field_type.clone(),
            var_type: *var_type,
        })
        .collect();
    StandardBlock {
        attributes: TypeAttributes::new(SemanticType::FunctionBlock {
            name: name.to_string(),
        })
        .with_standard_block(block),
        fields,
    }
}

// =============================================================================
// Bistable Function Blocks (IEC 61131-3 Section 2.5.2.3.1)
// =============================================================================

/// Creates the SR (Set-Reset) bistable function block.
///
/// The SR function block is a bistable element where Set dominates.
/// Q1 := S1 OR (NOT R AND Q1)
fn build_sr() -> StandardBlock {
    use FunctionBlockVarType::*;
    build_function_block(
        StandardFunctionBlock::Sr,
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
fn build_rs() -> StandardBlock {
    use FunctionBlockVarType::*;
    build_function_block(
        StandardFunctionBlock::Rs,
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
fn build_r_trig() -> StandardBlock {
    use FunctionBlockVarType::*;
    build_function_block(
        StandardFunctionBlock::RTrig,
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
fn build_f_trig() -> StandardBlock {
    use FunctionBlockVarType::*;
    build_function_block(
        StandardFunctionBlock::FTrig,
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
fn build_ctu_variant(suffix: &str, width: CounterWidth, int_type: SemanticType) -> StandardBlock {
    use FunctionBlockVarType::*;
    build_function_block(
        StandardFunctionBlock::Ctu(width),
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
fn build_ctd_variant(suffix: &str, width: CounterWidth, int_type: SemanticType) -> StandardBlock {
    use FunctionBlockVarType::*;
    build_function_block(
        StandardFunctionBlock::Ctd(width),
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
fn build_ctud_variant(suffix: &str, width: CounterWidth, int_type: SemanticType) -> StandardBlock {
    use FunctionBlockVarType::*;
    build_function_block(
        StandardFunctionBlock::Ctud(width),
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
fn get_all_counter_variants() -> Vec<(&'static str, StandardBlock)> {
    let mut counters = Vec::new();
    for (suffix, width, int_type_fn) in COUNTER_INT_VARIANTS {
        let int_type = int_type_fn();
        counters.push((
            leak_lowercase(&format!("ctu{}", suffix)),
            build_ctu_variant(suffix, *width, int_type.clone()),
        ));
        counters.push((
            leak_lowercase(&format!("ctd{}", suffix)),
            build_ctd_variant(suffix, *width, int_type.clone()),
        ));
        counters.push((
            leak_lowercase(&format!("ctud{}", suffix)),
            build_ctud_variant(suffix, *width, int_type),
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
fn build_ton() -> StandardBlock {
    use FunctionBlockVarType::*;
    build_function_block(
        StandardFunctionBlock::Ton,
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
fn build_tof() -> StandardBlock {
    use FunctionBlockVarType::*;
    build_function_block(
        StandardFunctionBlock::Tof,
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
fn build_tp() -> StandardBlock {
    use FunctionBlockVarType::*;
    build_function_block(
        StandardFunctionBlock::Tp,
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
/// Each tuple contains (lowercase_name, StandardBlock).
/// The lowercase name is used for case-insensitive lookup in the type environment.
pub fn get_all_stdlib_function_blocks() -> Vec<(&'static str, StandardBlock)> {
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
    use ironplc_test::cast_struct;
    use spec_test_macro::spec_test;

    #[test]
    fn get_all_stdlib_function_blocks_returns_expected_count() {
        let fbs = get_all_stdlib_function_blocks();
        // 2 bistable + 2 edge detection + 3 timers + 15 counters (3 types × 5 int variants)
        assert_eq!(fbs.len(), 22);
    }

    #[test]
    fn build_ton_when_called_then_has_correct_inputs_and_outputs() {
        let ton = build_ton();
        let name = cast_struct!(
            &ton.attributes.representation,
            SemanticType::FunctionBlock { name }
        );
        let fields = &ton.fields;
        assert_eq!(name, "TON");
        assert_eq!(fields.len(), 4);

        // Check inputs
        assert_eq!(fields[0].name.original(), "IN");
        assert_eq!(fields[0].var_type, FunctionBlockVarType::Input);

        assert_eq!(fields[1].name.original(), "PT");
        assert_eq!(fields[1].var_type, FunctionBlockVarType::Input);

        // Check outputs
        assert_eq!(fields[2].name.original(), "Q");
        assert_eq!(fields[2].var_type, FunctionBlockVarType::Output);

        assert_eq!(fields[3].name.original(), "ET");
        assert_eq!(fields[3].var_type, FunctionBlockVarType::Output);
    }

    #[test]
    fn build_tof_when_called_then_has_correct_inputs_and_outputs() {
        let tof = build_tof();
        let name = cast_struct!(
            &tof.attributes.representation,
            SemanticType::FunctionBlock { name }
        );
        let fields = &tof.fields;
        assert_eq!(name, "TOF");
        assert_eq!(fields.len(), 4);

        // Check inputs
        assert_eq!(fields[0].name.original(), "IN");
        assert_eq!(fields[0].var_type, FunctionBlockVarType::Input);

        assert_eq!(fields[1].name.original(), "PT");
        assert_eq!(fields[1].var_type, FunctionBlockVarType::Input);

        // Check outputs
        assert_eq!(fields[2].name.original(), "Q");
        assert_eq!(fields[2].var_type, FunctionBlockVarType::Output);

        assert_eq!(fields[3].name.original(), "ET");
        assert_eq!(fields[3].var_type, FunctionBlockVarType::Output);
    }

    #[test]
    fn build_ctu_variant_when_called_then_has_correct_fields() {
        let ctu = build_ctu_variant(
            "",
            CounterWidth::Int,
            SemanticType::Int {
                size: ByteSized::B16,
            },
        );
        let name = cast_struct!(
            &ctu.attributes.representation,
            SemanticType::FunctionBlock { name }
        );
        let fields = &ctu.fields;
        assert_eq!(name, "CTU");
        assert_eq!(fields.len(), 5);

        // Check inputs: CU, R, PV
        assert_eq!(fields[0].name.original(), "CU");
        assert_eq!(fields[1].name.original(), "R");
        assert_eq!(fields[2].name.original(), "PV");

        // Check outputs: Q, CV
        assert_eq!(fields[3].name.original(), "Q");
        assert_eq!(fields[4].name.original(), "CV");
    }

    #[test]
    fn build_ctu_variant_when_dint_then_has_correct_name_and_type() {
        let ctu_dint = build_ctu_variant(
            "_DINT",
            CounterWidth::Dint,
            SemanticType::Int {
                size: ByteSized::B32,
            },
        );
        let name = cast_struct!(
            &ctu_dint.attributes.representation,
            SemanticType::FunctionBlock { name }
        );
        let fields = &ctu_dint.fields;
        assert_eq!(name, "CTU_DINT");

        // Check PV field has DINT type
        let pv_field = fields.iter().find(|f| f.name.original() == "PV").unwrap();
        assert_eq!(
            pv_field.field_type,
            SemanticType::Int {
                size: ByteSized::B32
            }
        );

        // Check CV field has DINT type
        let cv_field = fields.iter().find(|f| f.name.original() == "CV").unwrap();
        assert_eq!(
            cv_field.field_type,
            SemanticType::Int {
                size: ByteSized::B32
            }
        );
    }

    #[test]
    fn build_r_trig_when_called_then_has_internal_memory() {
        let r_trig = build_r_trig();
        let fields = &r_trig.fields;
        // Check that M is internal
        let m_field = fields.iter().find(|f| f.name.original() == "M").unwrap();
        assert_eq!(m_field.var_type, FunctionBlockVarType::Internal);
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

    #[spec_test(REQ_CB_analyzer_001)]
    fn get_all_stdlib_function_blocks_when_built_then_each_records_its_identity() {
        let identities: Vec<(&str, Option<StandardFunctionBlock>)> =
            get_all_stdlib_function_blocks()
                .into_iter()
                .map(|(name, block)| (name, block.attributes.standard_block))
                .collect();
        assert!(identities.iter().all(|(_, block)| block.is_some()));
        assert!(identities.contains(&("ton", Some(StandardFunctionBlock::Ton))));
        assert!(identities.contains(&("r_trig", Some(StandardFunctionBlock::RTrig))));
        assert!(identities.contains(&("ctu", Some(StandardFunctionBlock::Ctu(CounterWidth::Int)))));
        assert!(identities.contains(&(
            "ctud_ulint",
            Some(StandardFunctionBlock::Ctud(CounterWidth::Ulint))
        )));
    }

    #[test]
    fn get_all_stdlib_function_blocks_when_built_then_identities_are_distinct() {
        let mut identities: Vec<StandardFunctionBlock> = get_all_stdlib_function_blocks()
            .into_iter()
            .filter_map(|(_, block)| block.attributes.standard_block)
            .collect();
        let count = identities.len();
        identities.sort_by_key(|block| format!("{block:?}"));
        identities.dedup();
        assert_eq!(count, identities.len());
    }
}
