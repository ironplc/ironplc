//! Options affecting parsing.
//!
//! Use the [`define_parse_options`] macro to declare fields so that
//! [`ParseOptions::apply_allow_all`] is always kept in sync.

/// Declares [`ParseOptions`] with two groups of boolean flags:
///
/// * **standard** — flags that are *not* affected by `allow_all`
///   (e.g. version selectors, internal-only knobs).
/// * **vendor** — vendor-extension flags that `allow_all` should enable.
///
/// The macro auto-generates [`ParseOptions::apply_allow_all`] which sets
/// every *vendor* flag to `true` when `allow_all` is set.  Adding a new
/// vendor extension is a single-line change in the macro invocation — no
/// additional wiring is needed.
macro_rules! define_parse_options {
    (
        standard { $($(#[$std_meta:meta])* $std_field:ident),* $(,)? }
        vendor { $($(#[$vendor_meta:meta])* $vendor_field:ident),* $(,)? }
    ) => {
        #[derive(Debug, Default, Clone, Copy)]
        pub struct ParseOptions {
            $($(#[$std_meta])* pub $std_field: bool,)*
            $($(#[$vendor_meta])* pub $vendor_field: bool,)*
            /// When set, [`apply_allow_all`](ParseOptions::apply_allow_all)
            /// enables every vendor-extension flag.
            pub allow_all: bool,
        }

        impl ParseOptions {
            /// Sets all vendor extension flags to `true` when `allow_all` is
            /// set.  Call this after constructing options to apply the
            /// `--allow-all` CLI flag (or its LSP equivalent).
            pub fn apply_allow_all(&mut self) {
                if self.allow_all {
                    $(self.$vendor_field = true;)*
                }
            }
        }
    };
}

define_parse_options! {
    standard {
        allow_iec_61131_3_2013,
    }
    vendor {
        allow_c_style_comments,
        allow_missing_semicolon,
        allow_top_level_var_global,
        allow_constant_type_params,
        allow_empty_var_blocks,
        allow_time_as_function_name,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_allow_all_when_set_then_enables_all_vendor_flags() {
        let mut options = ParseOptions {
            allow_all: true,
            ..Default::default()
        };
        options.apply_allow_all();

        assert!(options.allow_c_style_comments);
        assert!(options.allow_missing_semicolon);
        assert!(options.allow_top_level_var_global);
        assert!(options.allow_constant_type_params);
        assert!(options.allow_empty_var_blocks);
        assert!(options.allow_time_as_function_name);
    }

    #[test]
    fn apply_allow_all_when_not_set_then_leaves_vendor_flags_unchanged() {
        let mut options = ParseOptions::default();
        options.apply_allow_all();

        assert!(!options.allow_c_style_comments);
        assert!(!options.allow_missing_semicolon);
        assert!(!options.allow_top_level_var_global);
        assert!(!options.allow_constant_type_params);
        assert!(!options.allow_empty_var_blocks);
        assert!(!options.allow_time_as_function_name);
    }

    #[test]
    fn apply_allow_all_when_set_then_does_not_affect_standard_flags() {
        let mut options = ParseOptions {
            allow_all: true,
            ..Default::default()
        };
        options.apply_allow_all();

        assert!(!options.allow_iec_61131_3_2013);
    }
}
