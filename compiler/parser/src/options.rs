//! Options affecting compilation (parsing, analysis, and code generation).
//!
//! Use [`Dialect`] to select a preset configuration, then optionally
//! override individual flags.  Use the [`define_compiler_options`] macro
//! to declare vendor-extension fields so that [`CompilerOptions::from_dialect`]
//! is the single place that maps dialects to flags.

use std::fmt;
use std::str::FromStr;

/// A named configuration preset that sets the IEC edition and
/// vendor-extension flags in one shot.
///
/// Individual `--allow-*` CLI flags can still override on top of a dialect.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Dialect {
    /// Strict IEC 61131-3:2003 (Edition 2).  No vendor extensions.
    #[default]
    Iec61131_3Ed2,
    /// Strict IEC 61131-3:2013 (Edition 3).  No vendor extensions.
    Iec61131_3Ed3,
    /// RuSTy-compatible dialect: Edition 2 base (so long-time keywords
    /// like `LDT` stay as identifiers) plus `REF_TO` support and all
    /// vendor extensions enabled.
    Rusty,
    /// CODESYS-compatible dialect: Edition 2 base plus `REF_TO` support
    /// and the vendor extensions that CODESYS accepts.  Does not bind the
    /// implicit `__SYSTEM_UP_TIME` globals, which are an IronPLC runtime
    /// convention rather than a CODESYS feature.
    Codesys,
}

impl Dialect {
    /// All known dialect variants.
    pub const ALL: &[Dialect] = &[
        Dialect::Iec61131_3Ed2,
        Dialect::Iec61131_3Ed3,
        Dialect::Rusty,
        Dialect::Codesys,
    ];

    /// A short human-readable name suitable for display in UIs and tool output.
    pub fn display_name(&self) -> &'static str {
        match self {
            Dialect::Iec61131_3Ed2 => "IEC 61131-3 Ed. 2",
            Dialect::Iec61131_3Ed3 => "IEC 61131-3 Ed. 3",
            Dialect::Rusty => "RuSTy-compatible",
            Dialect::Codesys => "CODESYS-compatible",
        }
    }

    /// A one-line description of what this dialect enables.
    pub fn description(&self) -> &'static str {
        match self {
            Dialect::Iec61131_3Ed2 => {
                "Strict IEC 61131-3:2003 (Edition 2). No vendor extensions. [default]"
            }
            Dialect::Iec61131_3Ed3 => "Strict IEC 61131-3:2013 (Edition 3). No vendor extensions.",
            Dialect::Rusty => {
                "RuSTy-compatible: Edition 2 base with REF_TO and all vendor extensions."
            }
            Dialect::Codesys => {
                "CODESYS-compatible: Edition 2 base with REF_TO and CODESYS vendor extensions."
            }
        }
    }

    /// The CLI / LSP string form of this dialect (also produced by `Display`).
    pub fn cli_name(&self) -> &'static str {
        match self {
            Dialect::Iec61131_3Ed2 => "iec61131-3-ed2",
            Dialect::Iec61131_3Ed3 => "iec61131-3-ed3",
            Dialect::Rusty => "rusty",
            Dialect::Codesys => "codesys",
        }
    }
}

impl fmt::Display for Dialect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.cli_name())
    }
}

/// Parse a [`Dialect`] from its CLI / LSP string form (see [`fmt::Display`]).
impl FromStr for Dialect {
    type Err = ParseDialectError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        for dialect in Self::ALL {
            if dialect.to_string() == s {
                return Ok(*dialect);
            }
        }
        Err(ParseDialectError {
            input: s.to_string(),
        })
    }
}

/// Error returned by [`Dialect::from_str`] when no dialect matches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseDialectError {
    pub input: String,
}

impl fmt::Display for ParseDialectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unknown dialect: {}", self.input)
    }
}

impl std::error::Error for ParseDialectError {}

/// Metadata for a single vendor-extension feature flag.
pub struct FeatureDescriptor {
    /// The CLI flag name (e.g. `"--allow-c-style-comments"`).
    pub cli_flag: &'static str,
    /// The option key used in the MCP `options` object (e.g. `"allow_c_style_comments"`).
    /// Matches the corresponding [`CompilerOptions`] field name.
    pub option_key: &'static str,
    /// A short human-readable description.
    pub description: &'static str,
    /// Dialects that enable this feature by default.
    pub dialects: &'static [Dialect],
}

/// Declares [`CompilerOptions`] with a set of vendor-extension boolean flags.
///
/// Each field carries a description string and a list of [`Dialect`] variants
/// that enable it.  The macro auto-generates the struct, its `Default` impl,
/// [`CompilerOptions::from_dialect`], and [`CompilerOptions::FEATURE_DESCRIPTORS`].
macro_rules! define_compiler_options {
    (
        $(
            $desc:literal,
            $cli_flag:literal,
            [$($dialect:ident),* $(,)?],
            $vendor_field:ident
        ),* $(,)?
    ) => {
        #[derive(Debug, Default, Clone, Copy)]
        pub struct CompilerOptions {
            /// When `true`, IEC 61131-3:2013 keywords (`LTIME`, `LDATE`,
            /// `LTOD`, `LDT`, `REF_TO`, `REF`, `NULL`) are recognised.
            pub allow_iec_61131_3_2013: bool,
            $(pub $vendor_field: bool,)*
        }

        impl CompilerOptions {
            /// Build a [`CompilerOptions`] from a [`Dialect`] preset.
            ///
            /// Individual flags can be set to `true` afterwards to layer
            /// additional extensions on top of the dialect.
            pub fn from_dialect(dialect: Dialect) -> Self {
                let mut opts = Self::default();
                if dialect == Dialect::Iec61131_3Ed3 {
                    opts.allow_iec_61131_3_2013 = true;
                }
                $(
                    if [$(Dialect::$dialect),*].contains(&dialect) {
                        opts.$vendor_field = true;
                    }
                )*
                opts
            }

            /// Metadata for every vendor-extension feature flag.
            pub const FEATURE_DESCRIPTORS: &[FeatureDescriptor] = &[
                $(
                    FeatureDescriptor {
                        cli_flag: $cli_flag,
                        option_key: stringify!($vendor_field),
                        description: $desc,
                        dialects: &[$(Dialect::$dialect),*],
                    },
                )*
            ];

            /// Set a vendor-extension feature flag by its `option_key` (the
            /// field name from [`FeatureDescriptor`]).
            ///
            /// Returns `true` if the key matched a known flag.
            pub fn set_flag_by_key(&mut self, key: &str, value: bool) -> bool {
                match key {
                    $(
                        stringify!($vendor_field) => {
                            self.$vendor_field = value;
                            true
                        }
                    )*
                    _ => false,
                }
            }

            /// Get a vendor-extension feature flag by its `option_key` (the
            /// field name from [`FeatureDescriptor`]).
            ///
            /// Returns `None` if the key does not match a known flag.
            pub fn get_flag_by_key(&self, key: &str) -> Option<bool> {
                match key {
                    $(
                        stringify!($vendor_field) => Some(self.$vendor_field),
                    )*
                    _ => None,
                }
            }
        }
    };
}

define_compiler_options! {
    "Allow C-style comments (// and /* */)",
    "--allow-c-style-comments",
    [Rusty, Codesys],
    allow_c_style_comments,

    "Allow missing semicolons after keyword statements like END_IF and END_STRUCT",
    "--allow-missing-semicolon",
    [Rusty, Codesys],
    allow_missing_semicolon,

    "Allow VAR_GLOBAL declarations at the top level outside CONFIGURATION",
    "--allow-top-level-var-global",
    [Rusty, Codesys],
    allow_top_level_var_global,

    "Allow constant references in type parameters (e.g. STRING[MY_CONST])",
    "--allow-constant-type-params",
    [Rusty, Codesys],
    allow_constant_type_params,

    "Allow empty variable blocks (VAR END_VAR)",
    "--allow-empty-var-blocks",
    [Rusty, Codesys],
    allow_empty_var_blocks,

    "Allow TIME as a function name (OSCAT compatibility)",
    "--allow-time-as-function-name",
    [Rusty, Codesys],
    allow_time_as_function_name,

    "Allow REF_TO, REF(), and NULL without full Edition 3",
    "--allow-ref-to",
    [Rusty, Codesys],
    allow_ref_to,

    "Allow Beckhoff TwinCAT/CODESYS REFERENCE TO reference types and the REF= binding operator",
    "--allow-reference-to",
    [Codesys],
    allow_reference_to,

    "Allow arithmetic (+, -) and ordering comparisons (<, >, <=, >=) on REF_TO types",
    "--allow-ref-arithmetic",
    [Rusty, Codesys],
    allow_ref_arithmetic,

    "Allow REF() on stack-allocated variables (VAR_TEMP, FUNCTION VAR_INPUT/VAR_OUTPUT)",
    "--allow-ref-stack-variables",
    [Rusty, Codesys],
    allow_ref_stack_variables,

    "Allow assigning between REF_TO types of different base types (type punning)",
    "--allow-ref-type-punning",
    [Rusty, Codesys],
    allow_ref_type_punning,

    "Allow integer literals (0/1) as BOOL variable initializers",
    "--allow-int-to-bool-initializer",
    [Rusty, Codesys],
    allow_int_to_bool_initializer,

    "Allow SIZEOF() operator (returns size in bytes of a variable or type)",
    "--allow-sizeof",
    [Rusty, Codesys],
    allow_sizeof,

    "Expose __SYSTEM_UP_TIME and __SYSTEM_UP_LTIME as implicit VAR_GLOBALs (runtime monotonic uptime)",
    "--allow-system-uptime-global",
    [Rusty],
    allow_system_uptime_global,

    "Allow implicit widening between bit-string and integer type families (BYTE->INT, literal->BYTE)",
    "--allow-cross-family-widening",
    [Rusty, Codesys],
    allow_cross_family_widening,

    "Allow IEC 61131-3:2013 partial-access bit syntax (.%Xn) as an alias for .n",
    "--allow-partial-access-syntax",
    [Rusty, Iec61131_3Ed3, Codesys],
    allow_partial_access_syntax,

    "Allow curly-brace pragmas ({attribute 'name'}) as opaque, skipped trivia",
    "--allow-pragmas",
    [Rusty, Codesys],
    allow_pragmas,
}

/// Format a human-readable summary of all dialects and which features each
/// enables.  Used by the `dialects` CLI subcommand.
pub fn describe_dialects() -> String {
    let mut out = String::from("Dialects:\n");
    for dialect in Dialect::ALL {
        out.push_str(&format!("  {:<20} {}\n", dialect, dialect.description()));
    }

    for dialect in Dialect::ALL {
        out.push_str(&format!("\nFeatures enabled by \"{}\":\n", dialect));
        let features: Vec<&FeatureDescriptor> = CompilerOptions::FEATURE_DESCRIPTORS
            .iter()
            .filter(|f| f.dialects.contains(dialect))
            .collect();
        if features.is_empty() && *dialect != Dialect::Iec61131_3Ed3 {
            out.push_str("  (none)\n");
        } else {
            if *dialect == Dialect::Iec61131_3Ed3 {
                out.push_str(
                    "  IEC 61131-3:2013 keywords (LTIME, LDATE, LTOD, LDT, REF_TO, REF, NULL)\n",
                );
            }
            for f in &features {
                out.push_str(&format!("  {:<34} {}\n", f.cli_flag, f.description));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Collect the vendor-flag `option_key`s that `from_dialect(dialect)`
    /// turns on, sorted for order-independent comparison.
    fn enabled_vendor_flags(dialect: Dialect) -> Vec<&'static str> {
        let options = CompilerOptions::from_dialect(dialect);
        let mut enabled: Vec<&'static str> = CompilerOptions::FEATURE_DESCRIPTORS
            .iter()
            .filter(|f| options.get_flag_by_key(f.option_key) == Some(true))
            .map(|f| f.option_key)
            .collect();
        enabled.sort_unstable();
        enabled
    }

    /// Assert that a dialect enables *exactly* the given set of vendor flags --
    /// no more, no less. This is the guard against a newly added option
    /// silently leaking into a dialect it should not belong to: adding an
    /// option to a dialect's macro tags forces a matching update here, and an
    /// accidental extra tag makes that dialect's expected set mismatch.
    fn assert_enabled_vendor_flags(dialect: Dialect, expected: &[&str]) {
        let mut expected_sorted = expected.to_vec();
        expected_sorted.sort_unstable();
        assert_eq!(
            enabled_vendor_flags(dialect),
            expected_sorted,
            "dialect {dialect} does not enable exactly the expected vendor flags"
        );
    }

    /// IEC 61131-3 Ed. 2 (the default) enables no vendor extensions at all.
    #[test]
    fn ed2_dialect_enables_no_vendor_flags() {
        assert!(!CompilerOptions::from_dialect(Dialect::Iec61131_3Ed2).allow_iec_61131_3_2013);
        assert_enabled_vendor_flags(Dialect::Iec61131_3Ed2, &[]);
    }

    /// IEC 61131-3 Ed. 3 turns on the Edition-3 keyword set and, among vendor
    /// extensions, only partial-access syntax (standardized in Edition 3).
    #[test]
    fn ed3_dialect_enables_only_partial_access_syntax() {
        assert!(CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3).allow_iec_61131_3_2013);
        assert_enabled_vendor_flags(Dialect::Iec61131_3Ed3, &["allow_partial_access_syntax"]);
    }

    /// The RuSTy dialect stays on the Edition-2 keyword base and enables every
    /// vendor extension. Listed explicitly (not derived) so a new option that
    /// is meant to be Rusty-only, or accidentally left off Rusty, is caught.
    #[test]
    fn rusty_dialect_enables_exactly_these_vendor_flags() {
        assert!(!CompilerOptions::from_dialect(Dialect::Rusty).allow_iec_61131_3_2013);
        assert_enabled_vendor_flags(
            Dialect::Rusty,
            &[
                "allow_c_style_comments",
                "allow_missing_semicolon",
                "allow_top_level_var_global",
                "allow_constant_type_params",
                "allow_empty_var_blocks",
                "allow_time_as_function_name",
                "allow_ref_to",
                "allow_ref_arithmetic",
                "allow_ref_stack_variables",
                "allow_ref_type_punning",
                "allow_int_to_bool_initializer",
                "allow_sizeof",
                "allow_system_uptime_global",
                "allow_cross_family_widening",
                "allow_partial_access_syntax",
                "allow_pragmas",
            ],
        );
    }

    /// The CODESYS dialect matches RuSTy except it does *not* bind the
    /// `__SYSTEM_UP_TIME` globals (`allow_system_uptime_global`), which are an
    /// IronPLC/RuSTy runtime convention rather than a CODESYS feature. Listed
    /// explicitly so that omission is asserted rather than assumed.
    #[test]
    fn codesys_dialect_enables_exactly_these_vendor_flags() {
        assert!(!CompilerOptions::from_dialect(Dialect::Codesys).allow_iec_61131_3_2013);
        assert_enabled_vendor_flags(
            Dialect::Codesys,
            &[
                "allow_c_style_comments",
                "allow_missing_semicolon",
                "allow_top_level_var_global",
                "allow_constant_type_params",
                "allow_empty_var_blocks",
                "allow_time_as_function_name",
                "allow_ref_to",
                "allow_reference_to",
                "allow_ref_arithmetic",
                "allow_ref_stack_variables",
                "allow_ref_type_punning",
                "allow_int_to_bool_initializer",
                "allow_sizeof",
                "allow_cross_family_widening",
                "allow_partial_access_syntax",
                "allow_pragmas",
            ],
        );
    }

    /// REQ-PAB-051: The `rusty` dialect preset enables partial-access syntax.
    #[test]
    fn options_spec_req_pab_051_rusty_dialect_enables_partial_access_syntax() {
        let options = CompilerOptions::from_dialect(Dialect::Rusty);
        assert!(options.allow_partial_access_syntax);
    }

    /// REQ-PAB-052: The `iec61131-3-ed3` dialect preset enables partial-access syntax.
    #[test]
    fn options_spec_req_pab_052_ed3_dialect_enables_partial_access_syntax() {
        let options = CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3);
        assert!(options.allow_partial_access_syntax);
    }

    #[test]
    fn from_dialect_when_default_then_ed2() {
        let options = CompilerOptions::from_dialect(Dialect::default());

        assert!(!options.allow_iec_61131_3_2013);
        assert!(!options.allow_ref_to);
    }

    #[test]
    fn feature_descriptors_when_called_then_non_empty_and_stably_ordered() {
        assert!(!CompilerOptions::FEATURE_DESCRIPTORS.is_empty());
        assert_eq!(
            CompilerOptions::FEATURE_DESCRIPTORS[0].cli_flag,
            "--allow-c-style-comments"
        );
    }

    #[test]
    fn describe_dialects_when_called_then_contains_all_dialects() {
        let output = describe_dialects();
        assert!(output.contains("iec61131-3-ed2"));
        assert!(output.contains("iec61131-3-ed3"));
        assert!(output.contains("rusty"));
        assert!(output.contains("codesys"));
    }

    #[test]
    fn describe_dialects_when_called_then_contains_feature_flags() {
        let output = describe_dialects();
        assert!(output.contains("--allow-c-style-comments"));
        assert!(output.contains("--allow-ref-to"));
    }

    #[test]
    fn dialect_display_when_ed2_then_cli_name() {
        assert_eq!(format!("{}", Dialect::Iec61131_3Ed2), "iec61131-3-ed2");
    }

    #[test]
    fn dialect_display_when_rusty_then_cli_name() {
        assert_eq!(format!("{}", Dialect::Rusty), "rusty");
    }

    #[test]
    fn dialect_display_when_codesys_then_cli_name() {
        assert_eq!(format!("{}", Dialect::Codesys), "codesys");
    }

    #[test]
    fn dialect_from_str_when_known_name_then_returns_variant() {
        assert_eq!("iec61131-3-ed2".parse(), Ok(Dialect::Iec61131_3Ed2));
        assert_eq!("iec61131-3-ed3".parse(), Ok(Dialect::Iec61131_3Ed3));
        assert_eq!("rusty".parse(), Ok(Dialect::Rusty));
        assert_eq!("codesys".parse(), Ok(Dialect::Codesys));
    }

    #[test]
    fn dialect_from_str_when_unknown_name_then_returns_err() {
        let result: Result<Dialect, _> = "nonsense".parse();
        assert!(result.is_err());
    }

    #[test]
    fn dialect_from_str_when_round_trip_then_equal() {
        for dialect in Dialect::ALL {
            assert_eq!(dialect.to_string().parse::<Dialect>(), Ok(*dialect));
        }
    }

    #[test]
    fn feature_descriptors_when_called_then_option_key_matches_field_name() {
        let fd = &CompilerOptions::FEATURE_DESCRIPTORS[0];
        assert_eq!(fd.option_key, "allow_c_style_comments");
    }

    #[test]
    fn feature_descriptors_when_called_then_all_option_keys_start_with_allow() {
        for fd in CompilerOptions::FEATURE_DESCRIPTORS {
            assert!(
                fd.option_key.starts_with("allow_"),
                "option_key {} does not start with allow_",
                fd.option_key
            );
        }
    }

    #[test]
    fn dialect_display_name_when_ed2_then_human_readable() {
        assert_eq!(Dialect::Iec61131_3Ed2.display_name(), "IEC 61131-3 Ed. 2");
    }

    #[test]
    fn dialect_description_when_ed2_then_contains_edition_2() {
        assert!(Dialect::Iec61131_3Ed2.description().contains("Edition 2"));
    }
}
