//! Behavior policies: the enumerated alternatives for implementer-specific
//! behavior, selected at compile time and encoded in the bytecode (ADR-0049).
//!
//! A policy names one semantic of a standard operation that IEC 61131-3
//! leaves to the implementer, and the deterministic alternatives IronPLC
//! offers for it. The compiler selects an alternative from its options and
//! emits the builtin func_id that carries it; the VM dispatches on what it
//! decodes and holds no policy state.
//!
//! The enums live in this crate rather than with the compiler options because
//! an alternative is an encoding commitment: its discriminant is the offset of
//! its func_id within the policy's block (see `builtin::str_to_num`). The
//! parser, codegen and VM all share this one definition.

/// A behavior policy: a fixed, ordered set of alternatives.
///
/// The order is the encoding order. Alternative `i` of a policy occupies
/// offset `i` in the func_id block for the operation it governs, and the
/// first alternative (offset 0) is the default -- the standard's result, or
/// a trap where the standard says "error" (ADR-0049 rule 4).
pub trait BehaviorPolicy: Copy + Default + Eq + core::fmt::Debug + 'static {
    /// Every alternative, in encoding order.
    const ALL: &'static [Self];

    /// The CLI name of every alternative, parallel to [`ALL`](Self::ALL).
    const NAMES: &'static [&'static str];

    /// The alternative's offset in its func_id block.
    fn index(self) -> u16;

    /// The CLI name of this alternative (as accepted by `--policy-*` flags
    /// and the option maps of the LSP and MCP servers).
    fn cli_name(self) -> &'static str {
        Self::NAMES[self.index() as usize]
    }

    /// One-line description of this alternative.
    fn description(self) -> &'static str;

    /// The alternative with the given CLI name, or `None`.
    fn from_cli_name(name: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .zip(Self::NAMES)
            .find(|(_, candidate)| **candidate == name)
            .map(|(alt, _)| *alt)
    }

    /// The alternative at encoding offset `index`, or `None` when the offset
    /// is past the last alternative.
    fn from_index(index: u16) -> Option<Self> {
        Self::ALL.get(index as usize).copied()
    }
}

/// What `STRING_TO_<numeric>` treats as convertible when the text carries
/// characters that are not part of a numeric literal.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u16)]
pub enum StringToNumNonNumeric {
    /// After trimming surrounding whitespace, the whole text must be a valid
    /// literal of the target type. Anything else is a failure. The strict
    /// default; also what RuSTy does.
    #[default]
    Reject = 0,
    /// After trimming leading whitespace, the longest leading run that is a
    /// valid literal is converted and the rest is ignored. No leading literal
    /// is a failure. CODESYS and TwinCAT (observed).
    IgnoreTrailing = 1,
    /// Leading characters that cannot start a literal are skipped, then as
    /// [`IgnoreTrailing`](Self::IgnoreTrailing). No literal anywhere is a
    /// failure. Rockwell Logix `STOD` (documented).
    IgnoreSurrounding = 2,
}

impl BehaviorPolicy for StringToNumNonNumeric {
    const ALL: &'static [Self] = &[Self::Reject, Self::IgnoreTrailing, Self::IgnoreSurrounding];
    const NAMES: &'static [&'static str] = &["reject", "ignore-trailing", "ignore-surrounding"];

    fn index(self) -> u16 {
        self as u16
    }

    fn description(self) -> &'static str {
        match self {
            Self::Reject => "the whole string (less surrounding whitespace) must be a literal",
            Self::IgnoreTrailing => "convert the leading literal and ignore what follows it",
            Self::IgnoreSurrounding => "skip to the first literal, convert it, ignore the rest",
        }
    }
}

/// What `STRING_TO_<numeric>` does when the text is not convertible: not a
/// literal under the selected [`StringToNumNonNumeric`], or a literal whose
/// value does not fit the target type.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u16)]
pub enum StringToNumFailure {
    /// Trap `V4006 StringNotConvertible` with the offending value. The strict
    /// default (ADR-0049 rule 4).
    #[default]
    Trap = 0,
    /// Produce the target type's zero and continue. CODESYS, TwinCAT, RuSTy.
    Zero = 1,
}

impl BehaviorPolicy for StringToNumFailure {
    const ALL: &'static [Self] = &[Self::Trap, Self::Zero];
    const NAMES: &'static [&'static str] = &["trap", "zero"];

    fn index(self) -> u16 {
        self as u16
    }

    fn description(self) -> &'static str {
        match self {
            Self::Trap => "halt with runtime error V4006 naming the offending string",
            Self::Zero => "produce zero and continue",
        }
    }
}

macro_rules! impl_policy_text {
    ($($ty:ident),*) => {
        $(
            impl core::fmt::Display for $ty {
                fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                    // `pad` so a caller's `{:<20}` width applies.
                    f.pad(self.cli_name())
                }
            }

            impl core::str::FromStr for $ty {
                type Err = UnknownAlternative;

                fn from_str(s: &str) -> Result<Self, Self::Err> {
                    Self::from_cli_name(s).ok_or(UnknownAlternative)
                }
            }
        )*
    };
}

impl_policy_text!(StringToNumNonNumeric, StringToNumFailure);

/// The text named no alternative of the policy it was parsed against.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnknownAlternative;

impl core::fmt::Display for UnknownAlternative {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("unknown policy alternative")
    }
}

#[cfg(feature = "std")]
impl std::error::Error for UnknownAlternative {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::format;

    #[test]
    fn index_when_each_alternative_then_matches_position_in_all() {
        for (position, alt) in StringToNumNonNumeric::ALL.iter().enumerate() {
            assert_eq!(alt.index() as usize, position);
        }
        for (position, alt) in StringToNumFailure::ALL.iter().enumerate() {
            assert_eq!(alt.index() as usize, position);
        }
    }

    #[test]
    fn default_when_each_policy_then_is_first_alternative() {
        assert_eq!(
            StringToNumNonNumeric::default(),
            StringToNumNonNumeric::ALL[0]
        );
        assert_eq!(StringToNumFailure::default(), StringToNumFailure::ALL[0]);
    }

    #[test]
    fn cli_name_when_round_tripped_through_from_cli_name_then_same_alternative() {
        for alt in StringToNumNonNumeric::ALL {
            assert_eq!(
                StringToNumNonNumeric::from_cli_name(alt.cli_name()),
                Some(*alt)
            );
        }
        for alt in StringToNumFailure::ALL {
            assert_eq!(
                StringToNumFailure::from_cli_name(alt.cli_name()),
                Some(*alt)
            );
        }
    }

    #[test]
    fn from_cli_name_when_unknown_then_none() {
        assert_eq!(StringToNumNonNumeric::from_cli_name("wrap"), None);
        assert_eq!(StringToNumFailure::from_cli_name("flag"), None);
    }

    #[test]
    fn from_str_when_known_then_ok_and_display_matches() {
        let alt: StringToNumNonNumeric = "ignore-trailing".parse().unwrap();
        assert_eq!(alt, StringToNumNonNumeric::IgnoreTrailing);
        assert_eq!(format!("{alt}"), "ignore-trailing");
        assert_eq!(format!("{:<8}|", StringToNumFailure::Zero), "zero    |");
    }

    #[test]
    fn from_str_when_unknown_then_err() {
        assert_eq!(
            "nonsense".parse::<StringToNumFailure>(),
            Err(UnknownAlternative)
        );
    }

    #[test]
    fn from_index_when_past_last_alternative_then_none() {
        assert_eq!(StringToNumNonNumeric::from_index(3), None);
        assert_eq!(StringToNumFailure::from_index(2), None);
        assert_eq!(
            StringToNumFailure::from_index(1),
            Some(StringToNumFailure::Zero)
        );
    }

    #[test]
    fn description_when_each_alternative_then_non_empty() {
        for alt in StringToNumNonNumeric::ALL {
            assert!(!alt.description().is_empty());
        }
        for alt in StringToNumFailure::ALL {
            assert!(!alt.description().is_empty());
        }
    }
}
