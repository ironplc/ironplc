//! Options affecting parsing.
//!
//! Use [`Dialect`] to select a preset configuration, then optionally
//! override individual flags.  Use the [`define_parse_options`] macro
//! to declare vendor-extension fields so that [`ParseOptions::from_dialect`]
//! is the single place that maps dialects to flags.

use std::fmt;

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
}

impl Dialect {
    /// All known dialect variants.
    pub const ALL: &[Dialect] = &[
        Dialect::Iec61131_3Ed2,
        Dialect::Iec61131_3Ed3,
        Dialect::Rusty,
    ];
}

impl fmt::Display for Dialect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Dialect::Iec61131_3Ed2 => write!(f, "iec61131-3-ed2"),
            Dialect::Iec61131_3Ed3 => write!(f, "iec61131-3-ed3"),
            Dialect::Rusty => write!(f, "rusty"),
        }
    }
}

/// Metadata for a single vendor-extension feature flag.
pub struct FeatureDescriptor {
    /// The CLI flag name (e.g. `"--allow-c-style-comments"`).
    pub cli_flag: &'static str,
    /// A short human-readable description.
    pub description: &'static str,
    /// Dialects that enable this feature by default.
    pub dialects: &'static [Dialect],
}

/// Declares [`ParseOptions`] with a set of vendor-extension boolean flags.
///
/// Each field carries a description string and a list of [`Dialect`] variants
/// that enable it.  The macro auto-generates the struct, its `Default` impl,
/// [`ParseOptions::from_dialect`], and [`ParseOptions::FEATURE_DESCRIPTORS`].
macro_rules! define_parse_options {
    (
        $(
            $desc:literal,
            $cli_flag:literal,
            [$($dialect:ident),* $(,)?],
            $vendor_field:ident
        ),* $(,)?
    ) => {
        #[derive(Debug, Default, Clone, Copy)]
        pub struct ParseOptions {
            /// When `true`, IEC 61131-3:2013 keywords (`LTIME`, `LDATE`,
            /// `LTOD`, `LDT`, `REF_TO`, `REF`, `NULL`) are recognised.
            pub allow_iec_61131_3_2013: bool,
            $(pub $vendor_field: bool,)*
        }

        impl ParseOptions {
            /// Build a [`ParseOptions`] from a [`Dialect`] preset.
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
                        description: $desc,
                        dialects: &[$(Dialect::$dialect),*],
                    },
                )*
            ];
        }
    };
}

define_parse_options! {
    "Allow C-style comments (// and /* */)",
    "--allow-c-style-comments",
    [Rusty],
    allow_c_style_comments,

    "Allow missing semicolons after keyword statements like END_IF and END_STRUCT",
    "--allow-missing-semicolon",
    [Rusty],
    allow_missing_semicolon,

    "Allow VAR_GLOBAL declarations at the top level outside CONFIGURATION",
    "--allow-top-level-var-global",
    [Rusty],
    allow_top_level_var_global,

    "Allow constant references in type parameters (e.g. STRING[MY_CONST])",
    "--allow-constant-type-params",
    [Rusty],
    allow_constant_type_params,

    "Allow empty variable blocks (VAR END_VAR)",
    "--allow-empty-var-blocks",
    [Rusty],
    allow_empty_var_blocks,

    "Allow TIME as a function name (OSCAT compatibility)",
    "--allow-time-as-function-name",
    [Rusty],
    allow_time_as_function_name,

    "Allow REF_TO, REF(), and NULL without full Edition 3",
    "--allow-ref-to",
    [Rusty],
    allow_ref_to,
}

/// Format a human-readable summary of all dialects and which features each
/// enables.  Used by the `dialects` CLI subcommand.
pub fn describe_dialects() -> String {
    let mut out = String::from("Dialects:\n");
    for dialect in Dialect::ALL {
        let summary = match dialect {
            Dialect::Iec61131_3Ed2 => {
                "Strict IEC 61131-3:2003 (Edition 2). No vendor extensions. [default]"
            }
            Dialect::Iec61131_3Ed3 => "Strict IEC 61131-3:2013 (Edition 3). No vendor extensions.",
            Dialect::Rusty => {
                "RuSTy-compatible: Edition 2 base with REF_TO and all vendor extensions."
            }
        };
        out.push_str(&format!("  {:<20} {}\n", dialect, summary));
    }

    for dialect in Dialect::ALL {
        out.push_str(&format!("\nFeatures enabled by \"{}\":\n", dialect));
        let features: Vec<&FeatureDescriptor> = ParseOptions::FEATURE_DESCRIPTORS
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

    #[test]
    fn from_dialect_when_ed2_then_all_flags_false() {
        let options = ParseOptions::from_dialect(Dialect::Iec61131_3Ed2);

        assert!(!options.allow_iec_61131_3_2013);
        assert!(!options.allow_c_style_comments);
        assert!(!options.allow_missing_semicolon);
        assert!(!options.allow_top_level_var_global);
        assert!(!options.allow_constant_type_params);
        assert!(!options.allow_empty_var_blocks);
        assert!(!options.allow_time_as_function_name);
        assert!(!options.allow_ref_to);
    }

    #[test]
    fn from_dialect_when_ed3_then_edition3_enabled_and_vendor_flags_false() {
        let options = ParseOptions::from_dialect(Dialect::Iec61131_3Ed3);

        assert!(options.allow_iec_61131_3_2013);
        assert!(!options.allow_c_style_comments);
        assert!(!options.allow_missing_semicolon);
        assert!(!options.allow_top_level_var_global);
        assert!(!options.allow_constant_type_params);
        assert!(!options.allow_empty_var_blocks);
        assert!(!options.allow_time_as_function_name);
        assert!(!options.allow_ref_to);
    }

    #[test]
    fn from_dialect_when_rusty_then_all_vendor_flags_enabled_and_edition3_disabled() {
        let options = ParseOptions::from_dialect(Dialect::Rusty);

        assert!(!options.allow_iec_61131_3_2013);
        assert!(options.allow_c_style_comments);
        assert!(options.allow_missing_semicolon);
        assert!(options.allow_top_level_var_global);
        assert!(options.allow_constant_type_params);
        assert!(options.allow_empty_var_blocks);
        assert!(options.allow_time_as_function_name);
        assert!(options.allow_ref_to);
    }

    #[test]
    fn from_dialect_when_default_then_ed2() {
        let options = ParseOptions::from_dialect(Dialect::default());

        assert!(!options.allow_iec_61131_3_2013);
        assert!(!options.allow_ref_to);
    }

    #[test]
    fn feature_descriptors_when_called_then_contains_all_vendor_flags() {
        assert_eq!(ParseOptions::FEATURE_DESCRIPTORS.len(), 7);
        assert_eq!(
            ParseOptions::FEATURE_DESCRIPTORS[0].cli_flag,
            "--allow-c-style-comments"
        );
    }

    #[test]
    fn feature_descriptors_when_rusty_then_all_features_listed() {
        let rusty_features: Vec<&str> = ParseOptions::FEATURE_DESCRIPTORS
            .iter()
            .filter(|f| f.dialects.contains(&Dialect::Rusty))
            .map(|f| f.cli_flag)
            .collect();
        assert_eq!(rusty_features.len(), 7);
    }

    #[test]
    fn describe_dialects_when_called_then_contains_all_dialects() {
        let output = describe_dialects();
        assert!(output.contains("iec61131-3-ed2"));
        assert!(output.contains("iec61131-3-ed3"));
        assert!(output.contains("rusty"));
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
}
