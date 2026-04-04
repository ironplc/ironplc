use crate::{
    options::CompilerOptions,
    token::{Token, TokenType},
};

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

/// Demote the TIME keyword token to an identifier in function contexts.
///
/// This allows `TIME()` to be parsed as a function call and
/// `FUNCTION TIME` to be parsed as a function declaration (used by OSCAT
/// to read the PLC system clock) while preserving TIME as a keyword for
/// type declarations (`VAR x : TIME;`) and duration literals (`TIME#5s`).
pub fn apply(tokens: &mut [Token], options: &CompilerOptions) {
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

    #[test]
    fn apply_when_time_after_function_and_enabled_then_demoted() {
        let mut tokens = vec![
            make_token(TokenType::Function, "FUNCTION"),
            make_token(TokenType::Whitespace, " "),
            make_token(TokenType::Time, "TIME"),
            make_token(TokenType::Whitespace, " "),
            make_token(TokenType::Colon, ":"),
        ];
        apply(&mut tokens, &opts_enabled());
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
        apply(&mut tokens, &opts_enabled());
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
        apply(&mut tokens, &opts_disabled());
        assert_eq!(tokens[2].token_type, TokenType::Time);
    }
}
