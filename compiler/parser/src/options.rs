//! Options affecting compilation (parsing, analysis, and code generation).
//!
//! Use [`Dialect`] to select a preset configuration, then optionally
//! override individual flags and behavior policies.  Use the
//! [`define_compiler_options`] macro to declare dialect-extension fields and
//! policy fields so that [`CompilerOptions::from_dialect`] is the single place
//! that maps dialects to flags and policies.
//!
//! A flag enables one syntax extension; a behavior policy (ADR-0049) selects
//! one of several documented alternatives for an operation's runtime
//! behavior. Flags only ever enable, and a preset is a bundle of them; a
//! policy always has exactly one alternative selected, and a preset names the
//! one a real target uses.

use std::fmt;
use std::str::FromStr;

pub use ironplc_container::policy::{
    BehaviorPolicy, StringToNumFailure, StringToNumNonNumeric, UnknownAlternative,
};

/// A named configuration preset that sets the IEC edition and
/// dialect-extension flags in one shot.
///
/// Individual `--allow-*` CLI flags can still override on top of a dialect.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Dialect {
    /// Strict IEC 61131-3:2003 (Edition 2).  No extensions.
    #[default]
    Iec61131_3Ed2,
    /// Strict IEC 61131-3:2013 (Edition 3).  No extensions.
    Iec61131_3Ed3,
    /// RuSTy-compatible dialect: Edition 2 base (so long-time keywords
    /// like `LDT` stay as identifiers) plus `REF_TO` support and all
    /// extensions enabled.
    Rusty,
    /// CODESYS-compatible dialect: Edition 2 base plus `REF_TO` support
    /// and the extensions that CODESYS accepts.  Does not bind the
    /// implicit `__SYSTEM_UP_TIME` globals, which are an IronPLC runtime
    /// convention rather than a CODESYS feature.
    Codesys,
    /// Beckhoff TwinCAT-compatible dialect: Edition 2 base plus the dialect
    /// extensions that TwinCAT accepts.  TwinCAT 3 is built on the CODESYS V3
    /// runtime, so this is close to [`Dialect::Codesys`], but does not enable
    /// the `REF_TO` / `REF()` / `NULL` reference extensions: TwinCAT spells
    /// references and pointers `REFERENCE TO` / `POINTER TO`, so enabling the
    /// CODESYS `REF_TO` syntax here would accept code TwinCAT itself rejects.
    /// Like CODESYS, it does not bind the implicit `__SYSTEM_UP_TIME` globals
    /// (an IronPLC runtime convention).
    TwinCat,
}

impl Dialect {
    /// All known dialect variants.
    pub const ALL: &[Dialect] = &[
        Dialect::Iec61131_3Ed2,
        Dialect::Iec61131_3Ed3,
        Dialect::Rusty,
        Dialect::Codesys,
        Dialect::TwinCat,
    ];

    /// A short human-readable name suitable for display in UIs and tool output.
    pub fn display_name(&self) -> &'static str {
        match self {
            Dialect::Iec61131_3Ed2 => "IEC 61131-3 Ed. 2",
            Dialect::Iec61131_3Ed3 => "IEC 61131-3 Ed. 3",
            Dialect::Rusty => "RuSTy-compatible",
            Dialect::Codesys => "CODESYS-compatible",
            Dialect::TwinCat => "TwinCAT-compatible",
        }
    }

    /// A one-line description of what this dialect enables.
    pub fn description(&self) -> &'static str {
        match self {
            Dialect::Iec61131_3Ed2 => {
                "Strict IEC 61131-3:2003 (Edition 2). No extensions. [default]"
            }
            Dialect::Iec61131_3Ed3 => "Strict IEC 61131-3:2013 (Edition 3). No extensions.",
            Dialect::Rusty => {
                "RuSTy-compatible: Edition 2 base with REF_TO and the extensions RuSTy accepts."
            }
            Dialect::Codesys => {
                "CODESYS-compatible: Edition 2 base with REF_TO and CODESYS extensions."
            }
            Dialect::TwinCat => {
                "TwinCAT-compatible: Edition 2 base with the extensions TwinCAT accepts."
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
            Dialect::TwinCat => "twincat",
        }
    }
}

impl fmt::Display for Dialect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // `pad` rather than `write_str`: the latter bypasses the formatter's
        // width, so a `{:<20}` in a caller would silently do nothing.
        f.pad(self.cli_name())
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

/// Metadata for a single dialect-extension feature flag.
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

/// Metadata for a single behavior policy (ADR-0049).
///
/// A policy is not a flag: it selects one of several enumerated alternatives
/// rather than enabling a feature, so it has its own descriptor and its own
/// accessors ([`CompilerOptions::set_policy_by_key`],
/// [`CompilerOptions::get_policy_by_key`]).
pub struct PolicyDescriptor {
    /// The CLI flag name (e.g. `"--policy-string-to-num-failure"`).
    pub cli_flag: &'static str,
    /// The option key used in the MCP `options` object and, in lowerCamelCase,
    /// in the LSP `initializationOptions`. Matches the corresponding
    /// [`CompilerOptions`] field name.
    pub option_key: &'static str,
    /// A short human-readable description of what the policy governs.
    pub description: &'static str,
    /// The CLI name of every alternative, in encoding order.
    pub alternatives: &'static [&'static str],
    /// The CLI name of the alternative every strict dialect selects.
    pub default: &'static str,
}

/// The outcome of [`CompilerOptions::set_policy_by_key`] when nothing was set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetPolicyError {
    /// The key names no behavior policy.
    UnknownKey,
    /// The key names a policy, but the value is not one of its alternatives.
    UnknownAlternative,
}

/// Declares [`CompilerOptions`] with a set of dialect-extension boolean flags
/// and a set of behavior policies.
///
/// Each flag carries a description string and a list of [`Dialect`] variants
/// that enable it. Each policy carries a description, its enum type, and the
/// alternative each non-default dialect selects (a dialect not listed keeps
/// the policy's default). The macro auto-generates the struct, its `Default`
/// impl, [`CompilerOptions::from_dialect`], [`CompilerOptions::FEATURE_DESCRIPTORS`],
/// and [`CompilerOptions::POLICY_DESCRIPTORS`], so this invocation is the single
/// place that maps dialects to flags and policies.
macro_rules! define_compiler_options {
    (
        $(
            $desc:literal,
            $cli_flag:literal,
            [$($dialect:ident),* $(,)?],
            $flag_field:ident
        ),* $(,)?
        policies {
            $(
                $policy_desc:literal,
                $policy_cli_flag:literal,
                $policy_type:ident,
                [$($policy_dialect:ident => $policy_alt:ident),* $(,)?],
                $policy_field:ident
            ),* $(,)?
        }
    ) => {
        #[derive(Debug, Default, Clone, Copy)]
        pub struct CompilerOptions {
            $(pub $flag_field: bool,)*
            $(pub $policy_field: $policy_type,)*
        }

        impl CompilerOptions {
            /// Build a [`CompilerOptions`] from a [`Dialect`] preset.
            ///
            /// Individual flags can be set to `true` afterwards to layer
            /// additional extensions on top of the dialect, and individual
            /// policies can be reassigned.
            pub fn from_dialect(dialect: Dialect) -> Self {
                let mut opts = Self::default();
                $(
                    if [$(Dialect::$dialect),*].contains(&dialect) {
                        opts.$flag_field = true;
                    }
                )*
                $(
                    $(
                        if dialect == Dialect::$policy_dialect {
                            opts.$policy_field = $policy_type::$policy_alt;
                        }
                    )*
                )*
                opts
            }

            /// Metadata for every dialect-extension feature flag.
            pub const FEATURE_DESCRIPTORS: &[FeatureDescriptor] = &[
                $(
                    FeatureDescriptor {
                        cli_flag: $cli_flag,
                        option_key: stringify!($flag_field),
                        description: $desc,
                        dialects: &[$(Dialect::$dialect),*],
                    },
                )*
            ];

            /// Metadata for every behavior policy.
            pub const POLICY_DESCRIPTORS: &[PolicyDescriptor] = &[
                $(
                    PolicyDescriptor {
                        cli_flag: $policy_cli_flag,
                        option_key: stringify!($policy_field),
                        description: $policy_desc,
                        alternatives: <$policy_type as BehaviorPolicy>::NAMES,
                        // The first alternative is the default by the
                        // `BehaviorPolicy` contract.
                        default: <$policy_type as BehaviorPolicy>::NAMES[0],
                    },
                )*
            ];

            /// Select a behavior policy alternative by the policy's
            /// `option_key` (the field name from [`PolicyDescriptor`]) and
            /// the alternative's CLI name.
            pub fn set_policy_by_key(&mut self, key: &str, value: &str) -> Result<(), SetPolicyError> {
                match key {
                    $(
                        stringify!($policy_field) => {
                            let alt = <$policy_type as BehaviorPolicy>::from_cli_name(value)
                                .ok_or(SetPolicyError::UnknownAlternative)?;
                            self.$policy_field = alt;
                            Ok(())
                        }
                    )*
                    _ => Err(SetPolicyError::UnknownKey),
                }
            }

            /// The CLI name of the alternative currently selected for the
            /// policy with the given `option_key`, or `None` if the key names
            /// no policy.
            pub fn get_policy_by_key(&self, key: &str) -> Option<&'static str> {
                match key {
                    $(
                        stringify!($policy_field) => Some(self.$policy_field.cli_name()),
                    )*
                    _ => None,
                }
            }

            /// Set a dialect-extension feature flag by its `option_key` (the
            /// field name from [`FeatureDescriptor`]).
            ///
            /// Returns `true` if the key matched a known flag.
            pub fn set_flag_by_key(&mut self, key: &str, value: bool) -> bool {
                match key {
                    $(
                        stringify!($flag_field) => {
                            self.$flag_field = value;
                            true
                        }
                    )*
                    _ => false,
                }
            }

            /// Get a dialect-extension feature flag by its `option_key` (the
            /// field name from [`FeatureDescriptor`]).
            ///
            /// Returns `None` if the key does not match a known flag.
            pub fn get_flag_by_key(&self, key: &str) -> Option<bool> {
                match key {
                    $(
                        stringify!($flag_field) => Some(self.$flag_field),
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
    [Rusty, Codesys, TwinCat],
    allow_c_style_comments,

    "Allow missing semicolons after keyword statements like END_IF and END_STRUCT",
    "--allow-missing-semicolon",
    [Rusty, Codesys, TwinCat],
    allow_missing_semicolon,

    "Allow VAR_GLOBAL declarations at the top level outside CONFIGURATION",
    "--allow-top-level-var-global",
    [Rusty, Codesys, TwinCat],
    allow_top_level_var_global,

    "Allow constant references in type parameters (e.g. STRING[MY_CONST])",
    "--allow-constant-type-params",
    [Rusty, Codesys, TwinCat],
    allow_constant_type_params,

    "Allow empty variable blocks (VAR END_VAR)",
    "--allow-empty-var-blocks",
    [Rusty, Codesys, TwinCat],
    allow_empty_var_blocks,

    "Allow TIME as a function name (OSCAT compatibility)",
    "--allow-time-as-function-name",
    [Rusty, Codesys, TwinCat],
    allow_time_as_function_name,

    "Allow IEC 61131-3:2013 long-time-type keywords (LTIME, LDATE, LTOD, LDT)",
    "--allow-long-time-types",
    [Iec61131_3Ed3, Codesys, TwinCat],
    allow_long_time_types,

    "Allow REF_TO, REF(), and NULL (standardized in IEC 61131-3:2013)",
    "--allow-ref-to",
    [Rusty, Codesys, Iec61131_3Ed3],
    allow_ref_to,

    "Allow Beckhoff TwinCAT/CODESYS REFERENCE TO reference types and the REF= binding operator",
    "--allow-reference-to",
    [Codesys, TwinCat],
    allow_reference_to,

    "Allow POINTER TO pointer types with explicit dereference (^)",
    "--allow-pointer-to",
    [Codesys, TwinCat],
    allow_pointer_to,

    "Allow the ADR() address-of operator (returns a typed pointer to a variable)",
    "--allow-adr",
    [Codesys, TwinCat],
    allow_adr,

    "Allow the PERSISTENT variable qualifier (Beckhoff TwinCAT/CODESYS extension)",
    "--allow-persistent-var",
    [Codesys, TwinCat],
    allow_persistent_var,

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
    [Rusty, Codesys, TwinCat],
    allow_int_to_bool_initializer,

    "Allow SIZEOF() operator (returns size in bytes of a variable or type)",
    "--allow-sizeof",
    [Rusty, Codesys, TwinCat],
    allow_sizeof,

    "Expose __SYSTEM_UP_TIME and __SYSTEM_UP_LTIME as implicit VAR_GLOBALs (runtime monotonic uptime)",
    "--allow-system-uptime-global",
    [Rusty],
    allow_system_uptime_global,

    "Allow implicit widening from a bit-string type to a strictly wider integer type (BYTE->INT)",
    "--allow-cross-family-widening",
    [Rusty, Codesys, TwinCat],
    allow_cross_family_widening,

    "Allow implicit conversion between UDINT and DWORD, in both directions, at equal width",
    "--allow-cross-family-conversion",
    [Rusty, Codesys, TwinCat],
    allow_cross_family_conversion,

    "Allow a bare integer literal where a bit-string type is expected (0 -> BYTE)",
    "--allow-int-literal-to-bit-string",
    [Rusty, Codesys, TwinCat],
    allow_int_literal_to_bit_string,

    "Allow IEC 61131-3:2013 partial-access bit syntax (.%Xn) as an alias for .n",
    "--allow-partial-access-syntax",
    [Rusty, Iec61131_3Ed3, Codesys, TwinCat],
    allow_partial_access_syntax,

    "Allow curly-brace pragmas ({attribute 'name'}) as opaque, skipped trivia",
    "--allow-pragmas",
    [Rusty, Codesys, TwinCat],
    allow_pragmas,

    "Allow the AND_THEN and OR_ELSE short-circuit boolean operators (Beckhoff/CODESYS extension)",
    "--allow-short-circuit-operators",
    [Rusty, Codesys, TwinCat],
    allow_short_circuit_operators,

    "Allow AT-located variables (e.g. AT%I*) mixed with plain variables in the same VAR/VAR_INPUT/VAR_OUTPUT block",
    "--allow-mixed-located-var-declarations",
    [Rusty, Codesys, TwinCat],
    allow_mixed_located_var_declarations,

    "Allow constant expressions (not just bare literals) in VAR initializers, e.g. SCALE*4.0",
    "--allow-constant-initializer-expressions",
    [Rusty, Codesys, TwinCat],
    allow_constant_initializer_expressions,

    "Allow hex/binary/octal bit-string literals (16#D012, 2#1010) as CASE labels",
    "--allow-bit-string-case-labels",
    [Rusty, Codesys, TwinCat],
    allow_bit_string_case_labels,

    "Allow STRING(n)/WSTRING(n) parenthesis length delimiter in addition to the standard STRING[n]/WSTRING[n] brackets",
    "--allow-paren-string-length",
    [Rusty, Codesys, TwinCat],
    allow_paren_string_length,

    "Allow general (non-constant) expressions as struct/FB-instance initializer values, e.g. (PT := pDevice^.Delta)",
    "--allow-struct-initializer-expressions",
    [Rusty, Codesys, TwinCat],
    allow_struct_initializer_expressions,

    "Allow IEC 61131-3:2013 object-oriented syntax: EXTENDS/IMPLEMENTS/ABSTRACT on FUNCTION_BLOCK declarations, INTERFACE declarations, METHOD declarations, and THIS/SUPER",
    "--allow-fb-inheritance",
    [Rusty, Iec61131_3Ed3, Codesys, TwinCat],
    allow_fb_inheritance,

    "Allow explicit per-member values in an enumeration declaration, e.g. (Deutsch := 1, English := 2) (standardized in IEC 61131-3:2013)",
    "--allow-enum-explicit-values",
    [Rusty, Iec61131_3Ed3, Codesys, TwinCat],
    allow_enum_explicit_values,

    "Allow the base-type suffix on an enumeration declaration, e.g. (A, B) WORD, naming the elementary type the members are stored in",
    "--allow-enum-base-type",
    [Rusty, Codesys, TwinCat],
    allow_enum_base_type,

    policies {
        "What STRING_TO_<numeric> treats as convertible when the string has non-numeric characters",
        "--policy-string-to-num-non-numeric",
        StringToNumNonNumeric,
        [Codesys => IgnoreTrailing, TwinCat => IgnoreTrailing],
        policy_string_to_num_non_numeric,

        "What STRING_TO_<numeric> does when the string is not convertible",
        "--policy-string-to-num-failure",
        StringToNumFailure,
        [Rusty => Zero, Codesys => Zero, TwinCat => Zero],
        policy_string_to_num_failure,
    }
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
        if features.is_empty() {
            out.push_str("  (none)\n");
        } else {
            for f in &features {
                out.push_str(&format!("  {:<34} {}\n", f.cli_flag, f.description));
            }
        }
    }

    for dialect in Dialect::ALL {
        let options = CompilerOptions::from_dialect(*dialect);
        out.push_str(&format!(
            "\nBehavior policies selected by \"{}\":\n",
            dialect
        ));
        for p in CompilerOptions::POLICY_DESCRIPTORS {
            let selected = options.get_policy_by_key(p.option_key).unwrap_or("?");
            out.push_str(&format!(
                "  {:<34} {:<20} {}\n",
                p.cli_flag, selected, p.description
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests;
