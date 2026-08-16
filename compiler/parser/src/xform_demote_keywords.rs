//! Demote dialect-gated keyword tokens to identifiers.
//!
//! Several words are keywords in one IEC 61131-3 dialect but valid
//! identifiers in another. The lexer always tokenises them as their keyword
//! variant; this transform "demotes" them back to [`TokenType::Identifier`]
//! when the feature that makes them keywords is not enabled, so standard
//! programs may use those words as ordinary names. The PEG parser dispatches
//! solely on `token_type`, so this rewrite is the complete mechanism that
//! hides a keyword from the grammar.
//!
//! [`apply`] is the single place that defines *which keyword demotes under
//! which flag*: each `match` arm pairs a set of keyword `TokenType`s with the
//! gate that keeps them as keywords. `TIME` is the one context-sensitive
//! case (it depends on neighbouring tokens and has inverted polarity), so it
//! lives in the separate [`apply_time`] helper called at the end.

use crate::{
    options::CompilerOptions,
    token::{Token, TokenType},
};

/// Demote dialect-gated keyword tokens to identifiers when their feature is
/// off.
///
/// The flag-gated groups:
///
/// * **Long-time-type keywords** (`LTIME`, `LDATE`, `LTOD`, `LDT`) — demoted
///   unless `allow_long_time_types` (standardized in IEC 61131-3:2013).
/// * **Reference keywords** (`REF_TO`, `REF`, `NULL`) — demoted unless
///   `allow_ref_to`. Because the long-time types and the reference keywords are
///   now independent flags, the RuSTy dialect can enable `REF_TO` syntax while
///   keeping `LDT` etc. available as identifiers.
/// * **`REFERENCE`** — demoted unless `allow_reference_to` (TwinCAT/CODESYS
///   `REFERENCE TO`).
/// * **`POINTER`** — demoted unless `allow_pointer_to` (TwinCAT/CODESYS
///   `POINTER TO`).
/// * **OOP keywords** (`EXTENDS`, `IMPLEMENTS`, `INTERFACE`, `END_INTERFACE`,
///   `ABSTRACT`, `METHOD`, `END_METHOD`) — demoted unless `allow_fb_inheritance`.
/// * **`AND_THEN`** — demoted unless `allow_short_circuit_operators`.
///
/// The context-sensitive `TIME` keyword is handled by [`apply_time`].
pub fn apply(tokens: &mut [Token], options: &CompilerOptions) {
    // Precompute each gate once. Demotion happens when the gate is `true`.
    let demote_time_types = !options.allow_long_time_types;
    let demote_ref = !options.allow_ref_to;
    let demote_reference = !options.allow_reference_to;
    let demote_pointer = !options.allow_pointer_to;
    let demote_oop = !options.allow_fb_inheritance;
    let demote_and_then = !options.allow_short_circuit_operators;

    for tok in tokens.iter_mut() {
        let demote = match tok.token_type {
            TokenType::Ltime | TokenType::Ldate | TokenType::Ltod | TokenType::Ldt => {
                demote_time_types
            }
            TokenType::RefTo | TokenType::Ref | TokenType::Null => demote_ref,
            TokenType::Reference => demote_reference,
            TokenType::Pointer => demote_pointer,
            TokenType::Extends
            | TokenType::Implements
            | TokenType::Interface
            | TokenType::EndInterface
            | TokenType::Abstract
            | TokenType::Method
            | TokenType::EndMethod => demote_oop,
            TokenType::AndThen => demote_and_then,
            _ => false,
        };
        if demote {
            tok.token_type = TokenType::Identifier;
        }
    }

    apply_time(tokens, options);
}

/// Returns true if the non-whitespace token immediately before `index` has the
/// given token type (skipping over whitespace and newline tokens).
fn preceded_by(tokens: &[Token], index: usize, expected: TokenType) -> bool {
    let mut j = index;
    while j > 0 {
        j -= 1;
        if tokens[j].token_type == TokenType::Whitespace
            || tokens[j].token_type == TokenType::Newline
        {
            continue;
        }
        return tokens[j].token_type == expected;
    }
    false
}

/// Returns true if the non-whitespace token immediately after `index` has the
/// given token type (skipping over whitespace and newline tokens).
fn followed_by(tokens: &[Token], index: usize, expected: TokenType) -> bool {
    let mut j = index + 1;
    while j < tokens.len() {
        if tokens[j].token_type == TokenType::Whitespace
            || tokens[j].token_type == TokenType::Newline
        {
            j += 1;
            continue;
        }
        return tokens[j].token_type == expected;
    }
    false
}

/// Demote the `TIME` keyword token to an identifier in function contexts.
///
/// This allows `TIME()` to be parsed as a function call and `FUNCTION TIME`
/// to be parsed as a function declaration (used by OSCAT to read the PLC
/// system clock) while preserving `TIME` as a keyword for type declarations
/// (`VAR x : TIME;`) and duration literals (`TIME#5s`). Unlike the flag-gated
/// keywords above, `TIME` demotes only in specific neighbour contexts and
/// only when `allow_time_as_function_name` *is* set.
fn apply_time(tokens: &mut [Token], options: &CompilerOptions) {
    if !options.allow_time_as_function_name {
        return;
    }
    for i in 0..tokens.len() {
        if tokens[i].token_type == TokenType::Time {
            // Demote when followed by `(` (function call: `TIME()`)
            if followed_by(tokens, i, TokenType::LeftParen) {
                tokens[i].token_type = TokenType::Identifier;
                continue;
            }
            // Demote when preceded by FUNCTION keyword (declaration: `FUNCTION TIME`)
            if preceded_by(tokens, i, TokenType::Function) {
                tokens[i].token_type = TokenType::Identifier;
                continue;
            }
            // Demote when followed by `:=` (return variable assignment: `TIME := ...`)
            if followed_by(tokens, i, TokenType::Assignment) {
                tokens[i].token_type = TokenType::Identifier;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use dsl::core::SourceSpan;

    use super::apply;
    use crate::{
        options::CompilerOptions,
        token::{Token, TokenType},
    };

    fn make_token(token_type: TokenType, text: &str) -> Token {
        Token {
            token_type,
            span: SourceSpan::default(),
            line: 1,
            col: 1,
            text: String::from(text),
        }
    }

    fn opts_default() -> CompilerOptions {
        CompilerOptions::default()
    }

    /// The Edition-3 keyword set: long-time types *and* the reference keywords
    /// stay as keywords. With the coarse edition boolean gone, that is exactly
    /// the two granular flags the Ed3 preset turns on.
    fn opts_edition3() -> CompilerOptions {
        CompilerOptions {
            allow_long_time_types: true,
            allow_ref_to: true,
            ..CompilerOptions::default()
        }
    }

    fn opts_ref_to() -> CompilerOptions {
        CompilerOptions {
            allow_ref_to: true,
            ..CompilerOptions::default()
        }
    }

    fn opts_reference_to() -> CompilerOptions {
        CompilerOptions {
            allow_reference_to: true,
            ..CompilerOptions::default()
        }
    }

    fn opts_fb_inheritance() -> CompilerOptions {
        CompilerOptions {
            allow_fb_inheritance: true,
            ..CompilerOptions::default()
        }
    }

    fn opts_short_circuit() -> CompilerOptions {
        CompilerOptions {
            allow_short_circuit_operators: true,
            ..CompilerOptions::default()
        }
    }

    fn opts_time_fn() -> CompilerOptions {
        CompilerOptions {
            allow_time_as_function_name: true,
            ..CompilerOptions::default()
        }
    }

    // --- Long-time-type keywords: demoted when edition3 is off ---

    #[test]
    fn apply_when_ltime_and_not_edition3_then_demoted_to_identifier() {
        let mut tokens = vec![make_token(TokenType::Ltime, "LTIME")];
        apply(&mut tokens, &opts_default());
        assert_eq!(tokens[0].token_type, TokenType::Identifier);
        assert_eq!(tokens[0].text, "LTIME");
    }

    #[test]
    fn apply_when_ltime_and_edition3_then_stays_keyword() {
        let mut tokens = vec![make_token(TokenType::Ltime, "LTIME")];
        apply(&mut tokens, &opts_edition3());
        assert_eq!(tokens[0].token_type, TokenType::Ltime);
    }

    #[test]
    fn apply_when_ldate_and_not_edition3_then_demoted_to_identifier() {
        let mut tokens = vec![make_token(TokenType::Ldate, "LDATE")];
        apply(&mut tokens, &opts_default());
        assert_eq!(tokens[0].token_type, TokenType::Identifier);
        assert_eq!(tokens[0].text, "LDATE");
    }

    #[test]
    fn apply_when_ldate_and_edition3_then_stays_keyword() {
        let mut tokens = vec![make_token(TokenType::Ldate, "LDATE")];
        apply(&mut tokens, &opts_edition3());
        assert_eq!(tokens[0].token_type, TokenType::Ldate);
    }

    #[test]
    fn apply_when_ltod_and_not_edition3_then_demoted_to_identifier() {
        let mut tokens = vec![make_token(TokenType::Ltod, "LTOD")];
        apply(&mut tokens, &opts_default());
        assert_eq!(tokens[0].token_type, TokenType::Identifier);
        assert_eq!(tokens[0].text, "LTOD");
    }

    #[test]
    fn apply_when_ltod_and_edition3_then_stays_keyword() {
        let mut tokens = vec![make_token(TokenType::Ltod, "LTOD")];
        apply(&mut tokens, &opts_edition3());
        assert_eq!(tokens[0].token_type, TokenType::Ltod);
    }

    #[test]
    fn apply_when_ldt_and_not_edition3_then_demoted_to_identifier() {
        let mut tokens = vec![make_token(TokenType::Ldt, "LDT")];
        apply(&mut tokens, &opts_default());
        assert_eq!(tokens[0].token_type, TokenType::Identifier);
        assert_eq!(tokens[0].text, "LDT");
    }

    #[test]
    fn apply_when_ldt_and_edition3_then_stays_keyword() {
        let mut tokens = vec![make_token(TokenType::Ldt, "LDT")];
        apply(&mut tokens, &opts_edition3());
        assert_eq!(tokens[0].token_type, TokenType::Ldt);
    }

    // --- Long-time-type keywords: still demoted even when allow_ref_to is set ---

    #[test]
    fn apply_when_ldt_and_allow_ref_to_then_still_demoted() {
        let mut tokens = vec![make_token(TokenType::Ldt, "LDT")];
        apply(&mut tokens, &opts_ref_to());
        assert_eq!(tokens[0].token_type, TokenType::Identifier);
    }

    #[test]
    fn apply_when_ltime_and_allow_ref_to_then_still_demoted() {
        let mut tokens = vec![make_token(TokenType::Ltime, "LTIME")];
        apply(&mut tokens, &opts_ref_to());
        assert_eq!(tokens[0].token_type, TokenType::Identifier);
    }

    // --- Reference keywords: demoted only when BOTH edition3 AND ref_to are off ---

    #[test]
    fn apply_when_ref_to_and_not_edition3_then_demoted_to_identifier() {
        let mut tokens = vec![make_token(TokenType::RefTo, "REF_TO")];
        apply(&mut tokens, &opts_default());
        assert_eq!(tokens[0].token_type, TokenType::Identifier);
        assert_eq!(tokens[0].text, "REF_TO");
    }

    #[test]
    fn apply_when_ref_to_and_edition3_then_stays_keyword() {
        let mut tokens = vec![make_token(TokenType::RefTo, "REF_TO")];
        apply(&mut tokens, &opts_edition3());
        assert_eq!(tokens[0].token_type, TokenType::RefTo);
    }

    #[test]
    fn apply_when_ref_to_and_allow_ref_to_then_stays_keyword() {
        let mut tokens = vec![make_token(TokenType::RefTo, "REF_TO")];
        apply(&mut tokens, &opts_ref_to());
        assert_eq!(tokens[0].token_type, TokenType::RefTo);
    }

    #[test]
    fn apply_when_ref_and_not_edition3_then_demoted_to_identifier() {
        let mut tokens = vec![make_token(TokenType::Ref, "REF")];
        apply(&mut tokens, &opts_default());
        assert_eq!(tokens[0].token_type, TokenType::Identifier);
        assert_eq!(tokens[0].text, "REF");
    }

    #[test]
    fn apply_when_ref_and_edition3_then_stays_keyword() {
        let mut tokens = vec![make_token(TokenType::Ref, "REF")];
        apply(&mut tokens, &opts_edition3());
        assert_eq!(tokens[0].token_type, TokenType::Ref);
    }

    #[test]
    fn apply_when_ref_and_allow_ref_to_then_stays_keyword() {
        let mut tokens = vec![make_token(TokenType::Ref, "REF")];
        apply(&mut tokens, &opts_ref_to());
        assert_eq!(tokens[0].token_type, TokenType::Ref);
    }

    #[test]
    fn apply_when_null_and_not_edition3_then_demoted_to_identifier() {
        let mut tokens = vec![make_token(TokenType::Null, "NULL")];
        apply(&mut tokens, &opts_default());
        assert_eq!(tokens[0].token_type, TokenType::Identifier);
        assert_eq!(tokens[0].text, "NULL");
    }

    #[test]
    fn apply_when_null_and_edition3_then_stays_keyword() {
        let mut tokens = vec![make_token(TokenType::Null, "NULL")];
        apply(&mut tokens, &opts_edition3());
        assert_eq!(tokens[0].token_type, TokenType::Null);
    }

    #[test]
    fn apply_when_null_and_allow_ref_to_then_stays_keyword() {
        let mut tokens = vec![make_token(TokenType::Null, "NULL")];
        apply(&mut tokens, &opts_ref_to());
        assert_eq!(tokens[0].token_type, TokenType::Null);
    }

    #[test]
    fn apply_when_non_edition3_token_then_unchanged() {
        let mut tokens = vec![make_token(TokenType::Int, "INT")];
        apply(&mut tokens, &opts_default());
        assert_eq!(tokens[0].token_type, TokenType::Int);
    }

    // --- REFERENCE keyword: demoted unless allow_reference_to ---

    #[test]
    fn apply_when_reference_and_flag_off_then_demoted_to_identifier() {
        let mut tokens = vec![make_token(TokenType::Reference, "REFERENCE")];
        apply(&mut tokens, &opts_default());
        assert_eq!(tokens[0].token_type, TokenType::Identifier);
        assert_eq!(tokens[0].text, "REFERENCE");
    }

    #[test]
    fn apply_when_reference_and_flag_on_then_kept_as_keyword() {
        let mut tokens = vec![make_token(TokenType::Reference, "REFERENCE")];
        apply(&mut tokens, &opts_reference_to());
        assert_eq!(tokens[0].token_type, TokenType::Reference);
    }

    // --- OOP keywords: demoted unless allow_fb_inheritance ---

    #[test]
    fn apply_when_extends_and_disabled_then_demoted_to_identifier() {
        let mut tokens = vec![make_token(TokenType::Extends, "EXTENDS")];
        apply(&mut tokens, &opts_default());
        assert_eq!(tokens[0].token_type, TokenType::Identifier);
        assert_eq!(tokens[0].text, "EXTENDS");
    }

    #[test]
    fn apply_when_extends_and_enabled_then_stays_keyword() {
        let mut tokens = vec![make_token(TokenType::Extends, "EXTENDS")];
        apply(&mut tokens, &opts_fb_inheritance());
        assert_eq!(tokens[0].token_type, TokenType::Extends);
    }

    #[test]
    fn apply_when_implements_and_disabled_then_demoted_to_identifier() {
        let mut tokens = vec![make_token(TokenType::Implements, "IMPLEMENTS")];
        apply(&mut tokens, &opts_default());
        assert_eq!(tokens[0].token_type, TokenType::Identifier);
    }

    #[test]
    fn apply_when_implements_and_enabled_then_stays_keyword() {
        let mut tokens = vec![make_token(TokenType::Implements, "IMPLEMENTS")];
        apply(&mut tokens, &opts_fb_inheritance());
        assert_eq!(tokens[0].token_type, TokenType::Implements);
    }

    #[test]
    fn apply_when_interface_and_disabled_then_demoted_to_identifier() {
        let mut tokens = vec![make_token(TokenType::Interface, "INTERFACE")];
        apply(&mut tokens, &opts_default());
        assert_eq!(tokens[0].token_type, TokenType::Identifier);
    }

    #[test]
    fn apply_when_interface_and_enabled_then_stays_keyword() {
        let mut tokens = vec![make_token(TokenType::Interface, "INTERFACE")];
        apply(&mut tokens, &opts_fb_inheritance());
        assert_eq!(tokens[0].token_type, TokenType::Interface);
    }

    #[test]
    fn apply_when_end_interface_and_disabled_then_demoted_to_identifier() {
        let mut tokens = vec![make_token(TokenType::EndInterface, "END_INTERFACE")];
        apply(&mut tokens, &opts_default());
        assert_eq!(tokens[0].token_type, TokenType::Identifier);
    }

    #[test]
    fn apply_when_end_interface_and_enabled_then_stays_keyword() {
        let mut tokens = vec![make_token(TokenType::EndInterface, "END_INTERFACE")];
        apply(&mut tokens, &opts_fb_inheritance());
        assert_eq!(tokens[0].token_type, TokenType::EndInterface);
    }

    #[test]
    fn apply_when_abstract_and_disabled_then_demoted_to_identifier() {
        let mut tokens = vec![make_token(TokenType::Abstract, "ABSTRACT")];
        apply(&mut tokens, &opts_default());
        assert_eq!(tokens[0].token_type, TokenType::Identifier);
        assert_eq!(tokens[0].text, "ABSTRACT");
    }

    #[test]
    fn apply_when_abstract_and_enabled_then_stays_keyword() {
        let mut tokens = vec![make_token(TokenType::Abstract, "ABSTRACT")];
        apply(&mut tokens, &opts_fb_inheritance());
        assert_eq!(tokens[0].token_type, TokenType::Abstract);
    }

    // --- AND_THEN operator: demoted unless allow_short_circuit_operators ---

    #[test]
    fn apply_when_and_then_and_disabled_then_demoted_to_identifier() {
        let mut tokens = vec![make_token(TokenType::AndThen, "AND_THEN")];
        apply(&mut tokens, &opts_default());
        assert_eq!(tokens[0].token_type, TokenType::Identifier);
        assert_eq!(tokens[0].text, "AND_THEN");
    }

    #[test]
    fn apply_when_and_then_and_enabled_then_stays_keyword() {
        let mut tokens = vec![make_token(TokenType::AndThen, "AND_THEN")];
        apply(&mut tokens, &opts_short_circuit());
        assert_eq!(tokens[0].token_type, TokenType::AndThen);
    }

    #[test]
    fn apply_when_non_short_circuit_token_then_unchanged() {
        let mut tokens = vec![make_token(TokenType::And, "AND")];
        apply(&mut tokens, &opts_default());
        assert_eq!(tokens[0].token_type, TokenType::And);
    }

    // --- TIME keyword: context-sensitive, demoted only when the flag is on ---

    #[test]
    fn apply_when_time_before_left_paren_and_enabled_then_demoted() {
        let mut tokens = vec![
            make_token(TokenType::Time, "TIME"),
            make_token(TokenType::LeftParen, "("),
        ];
        apply(&mut tokens, &opts_time_fn());
        assert_eq!(tokens[0].token_type, TokenType::Identifier);
        assert_eq!(tokens[0].text, "TIME");
    }

    #[test]
    fn apply_when_time_before_hash_and_enabled_then_stays_keyword() {
        let mut tokens = vec![
            make_token(TokenType::Time, "TIME"),
            make_token(TokenType::Hash, "#"),
        ];
        apply(&mut tokens, &opts_time_fn());
        assert_eq!(tokens[0].token_type, TokenType::Time);
    }

    #[test]
    fn apply_when_time_before_semicolon_and_enabled_then_stays_keyword() {
        let mut tokens = vec![
            make_token(TokenType::Time, "TIME"),
            make_token(TokenType::Semicolon, ";"),
        ];
        apply(&mut tokens, &opts_time_fn());
        assert_eq!(tokens[0].token_type, TokenType::Time);
    }

    #[test]
    fn apply_when_time_before_left_paren_and_disabled_then_stays_keyword() {
        let mut tokens = vec![
            make_token(TokenType::Time, "TIME"),
            make_token(TokenType::LeftParen, "("),
        ];
        apply(&mut tokens, &opts_default());
        assert_eq!(tokens[0].token_type, TokenType::Time);
    }

    #[test]
    fn apply_when_time_is_last_token_and_enabled_then_stays_keyword() {
        let mut tokens = vec![make_token(TokenType::Time, "TIME")];
        apply(&mut tokens, &opts_time_fn());
        assert_eq!(tokens[0].token_type, TokenType::Time);
    }

    #[test]
    fn apply_when_non_time_token_then_unchanged() {
        let mut tokens = vec![
            make_token(TokenType::Int, "INT"),
            make_token(TokenType::LeftParen, "("),
        ];
        apply(&mut tokens, &opts_time_fn());
        assert_eq!(tokens[0].token_type, TokenType::Int);
    }

    #[test]
    fn apply_when_time_after_function_and_enabled_then_demoted() {
        let mut tokens = vec![
            make_token(TokenType::Function, "FUNCTION"),
            make_token(TokenType::Whitespace, " "),
            make_token(TokenType::Time, "TIME"),
            make_token(TokenType::Whitespace, " "),
            make_token(TokenType::Colon, ":"),
        ];
        apply(&mut tokens, &opts_time_fn());
        assert_eq!(tokens[2].token_type, TokenType::Identifier);
        assert_eq!(tokens[2].text, "TIME");
    }

    #[test]
    fn apply_when_time_before_assignment_and_enabled_then_demoted() {
        let mut tokens = vec![
            make_token(TokenType::Time, "TIME"),
            make_token(TokenType::Whitespace, " "),
            make_token(TokenType::Assignment, ":="),
        ];
        apply(&mut tokens, &opts_time_fn());
        assert_eq!(tokens[0].token_type, TokenType::Identifier);
        assert_eq!(tokens[0].text, "TIME");
    }

    #[test]
    fn apply_when_time_after_function_and_disabled_then_stays_keyword() {
        let mut tokens = vec![
            make_token(TokenType::Function, "FUNCTION"),
            make_token(TokenType::Whitespace, " "),
            make_token(TokenType::Time, "TIME"),
            make_token(TokenType::Whitespace, " "),
            make_token(TokenType::Colon, ":"),
        ];
        apply(&mut tokens, &opts_default());
        assert_eq!(tokens[2].token_type, TokenType::Time);
    }
}
