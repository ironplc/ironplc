use crate::{
    options::CompilerOptions,
    token::{Token, TokenType},
};

/// Demote the CODESYS/TwinCAT short-circuit boolean operator token
/// (`AND_THEN`) to an identifier when `allow_short_circuit_operators` is
/// not enabled.
///
/// This word is a valid IEC 61131-3 identifier (e.g. a variable or type
/// name). Demoting it back to `Identifier` when the flag is off keeps
/// standard programs that happen to use this name parsing correctly,
/// matching the pattern used for OOP keywords in
/// `xform_demote_oop_keywords.rs`.
pub fn apply(tokens: &mut [Token], options: &CompilerOptions) {
    if options.allow_short_circuit_operators {
        return;
    }

    for tok in tokens.iter_mut() {
        if tok.token_type == TokenType::AndThen {
            tok.token_type = TokenType::Identifier;
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

    fn opts_disabled() -> CompilerOptions {
        CompilerOptions {
            allow_short_circuit_operators: false,
            ..CompilerOptions::default()
        }
    }

    fn opts_enabled() -> CompilerOptions {
        CompilerOptions {
            allow_short_circuit_operators: true,
            ..CompilerOptions::default()
        }
    }

    #[test]
    fn apply_when_and_then_and_disabled_then_demoted_to_identifier() {
        let mut tokens = vec![make_token(TokenType::AndThen, "AND_THEN")];
        apply(&mut tokens, &opts_disabled());
        assert_eq!(tokens[0].token_type, TokenType::Identifier);
        assert_eq!(tokens[0].text, "AND_THEN");
    }

    #[test]
    fn apply_when_and_then_and_enabled_then_stays_keyword() {
        let mut tokens = vec![make_token(TokenType::AndThen, "AND_THEN")];
        apply(&mut tokens, &opts_enabled());
        assert_eq!(tokens[0].token_type, TokenType::AndThen);
    }

    #[test]
    fn apply_when_non_short_circuit_token_then_unchanged() {
        let mut tokens = vec![make_token(TokenType::And, "AND")];
        apply(&mut tokens, &opts_disabled());
        assert_eq!(tokens[0].token_type, TokenType::And);
    }
}
