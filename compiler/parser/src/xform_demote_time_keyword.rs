use crate::{
    options::CompilerOptions,
    token::{Token, TokenType},
};

/// Demote the TIME keyword token to an identifier when followed by `(`.
///
/// This allows `TIME()` to be parsed as a function call (used by OSCAT
/// to read the PLC system clock) while preserving TIME as a keyword for
/// type declarations (`VAR x : TIME;`) and duration literals (`TIME#5s`).
pub fn apply(tokens: &mut [Token], options: &CompilerOptions) {
    if !options.allow_time_as_function_name {
        return;
    }
    for i in 0..tokens.len() {
        if tokens[i].token_type == TokenType::Time {
            if let Some(next) = tokens.get(i + 1) {
                if next.token_type == TokenType::LeftParen {
                    tokens[i].token_type = TokenType::Identifier;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use dsl::core::SourceSpan;

    use crate::{
        options::CompilerOptions,
        token::{Token, TokenType},
        xform_demote_time_keyword::apply,
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

    fn opts_enabled() -> CompilerOptions {
        CompilerOptions {
            allow_time_as_function_name: true,
            ..CompilerOptions::default()
        }
    }

    fn opts_disabled() -> CompilerOptions {
        CompilerOptions::default()
    }

    #[test]
    fn apply_when_time_before_left_paren_and_enabled_then_demoted() {
        let mut tokens = vec![
            make_token(TokenType::Time, "TIME"),
            make_token(TokenType::LeftParen, "("),
        ];
        apply(&mut tokens, &opts_enabled());
        assert_eq!(tokens[0].token_type, TokenType::Identifier);
        assert_eq!(tokens[0].text, "TIME");
    }

    #[test]
    fn apply_when_time_before_hash_and_enabled_then_stays_keyword() {
        let mut tokens = vec![
            make_token(TokenType::Time, "TIME"),
            make_token(TokenType::Hash, "#"),
        ];
        apply(&mut tokens, &opts_enabled());
        assert_eq!(tokens[0].token_type, TokenType::Time);
    }

    #[test]
    fn apply_when_time_before_semicolon_and_enabled_then_stays_keyword() {
        let mut tokens = vec![
            make_token(TokenType::Time, "TIME"),
            make_token(TokenType::Semicolon, ";"),
        ];
        apply(&mut tokens, &opts_enabled());
        assert_eq!(tokens[0].token_type, TokenType::Time);
    }

    #[test]
    fn apply_when_time_before_left_paren_and_disabled_then_stays_keyword() {
        let mut tokens = vec![
            make_token(TokenType::Time, "TIME"),
            make_token(TokenType::LeftParen, "("),
        ];
        apply(&mut tokens, &opts_disabled());
        assert_eq!(tokens[0].token_type, TokenType::Time);
    }

    #[test]
    fn apply_when_time_is_last_token_and_enabled_then_stays_keyword() {
        let mut tokens = vec![make_token(TokenType::Time, "TIME")];
        apply(&mut tokens, &opts_enabled());
        assert_eq!(tokens[0].token_type, TokenType::Time);
    }

    #[test]
    fn apply_when_non_time_token_then_unchanged() {
        let mut tokens = vec![
            make_token(TokenType::Int, "INT"),
            make_token(TokenType::LeftParen, "("),
        ];
        apply(&mut tokens, &opts_enabled());
        assert_eq!(tokens[0].token_type, TokenType::Int);
    }
}
