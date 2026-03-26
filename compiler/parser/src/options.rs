//! Options affecting parsing.
//!
//! Use [`Dialect`] to select a preset configuration, then optionally
//! override individual flags.  Use the [`define_parse_options`] macro
//! to declare vendor-extension fields so that [`ParseOptions::from_dialect`]
//! is the single place that maps dialects to flags.

/// A named configuration preset that sets the IEC edition and
/// vendor-extension flags in one shot.
///
/// Individual `--allow-*` CLI flags can still override on top of a dialect.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Dialect {
    /// Strict IEC 61131-3:2003 (Edition 2).  No vendor extensions.
    Iec61131_3Ed2,
    /// Strict IEC 61131-3:2013 (Edition 3).  No vendor extensions.
    #[default]
    Iec61131_3Ed3,
    /// RuSTy-compatible dialect: Edition 2 base (so long-time keywords
    /// like `LDT` stay as identifiers) plus `REF_TO` support and all
    /// vendor extensions enabled.
    Rusty,
}

/// Declares [`ParseOptions`] with a set of vendor-extension boolean flags.
///
/// The macro auto-generates the struct, its `Default` impl, and
/// [`ParseOptions::from_dialect`] which maps each [`Dialect`] variant
/// to the correct combination of flags.
macro_rules! define_parse_options {
    (
        $( $(#[$vendor_meta:meta])* $vendor_field:ident ),* $(,)?
    ) => {
        #[derive(Debug, Default, Clone, Copy)]
        pub struct ParseOptions {
            /// When `true`, long date-and-time keywords (`LTIME`, `LDATE`,
            /// `LTOD`, `LDT`) are recognised as type keywords rather than
            /// identifiers.
            pub allow_long_date_and_time: bool,
            $($(#[$vendor_meta])* pub $vendor_field: bool,)*
        }

        impl ParseOptions {
            /// Build a [`ParseOptions`] from a [`Dialect`] preset.
            ///
            /// Individual flags can be set to `true` afterwards to layer
            /// additional extensions on top of the dialect.
            pub fn from_dialect(dialect: Dialect) -> Self {
                match dialect {
                    Dialect::Iec61131_3Ed2 => Self::default(),
                    Dialect::Iec61131_3Ed3 => Self {
                        allow_long_date_and_time: true,
                        ..Self::default()
                    },
                    Dialect::Rusty => Self {
                        allow_long_date_and_time: false,
                        // Enable every vendor extension.
                        $($vendor_field: true,)*
                    },
                }
            }
        }
    };
}

define_parse_options! {
    allow_c_style_comments,
    allow_missing_semicolon,
    allow_top_level_var_global,
    allow_constant_type_params,
    allow_empty_var_blocks,
    allow_time_as_function_name,
    allow_ref_to,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_dialect_when_ed2_then_all_flags_false() {
        let options = ParseOptions::from_dialect(Dialect::Iec61131_3Ed2);

        assert!(!options.allow_long_date_and_time);
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

        assert!(options.allow_long_date_and_time);
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

        assert!(!options.allow_long_date_and_time);
        assert!(options.allow_c_style_comments);
        assert!(options.allow_missing_semicolon);
        assert!(options.allow_top_level_var_global);
        assert!(options.allow_constant_type_params);
        assert!(options.allow_empty_var_blocks);
        assert!(options.allow_time_as_function_name);
        assert!(options.allow_ref_to);
    }

    #[test]
    fn from_dialect_when_default_then_ed3() {
        let options = ParseOptions::from_dialect(Dialect::default());

        assert!(options.allow_long_date_and_time);
        assert!(!options.allow_ref_to);
    }
}
