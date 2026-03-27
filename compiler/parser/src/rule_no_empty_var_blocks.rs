use dsl::diagnostic::{Diagnostic, Label};

use crate::{
    options::CompilerOptions,
    token::{Token, TokenType},
};

/// Returns true if the token type is a VAR-family keyword that opens a
/// variable declaration block.
fn is_var_keyword(tt: &TokenType) -> bool {
    matches!(
        tt,
        TokenType::Var
            | TokenType::VarInput
            | TokenType::VarOutput
            | TokenType::VarInOut
            | TokenType::VarExternal
            | TokenType::VarGlobal
            | TokenType::VarTemp
            | TokenType::VarAccess
            | TokenType::VarConfig
    )
}

/// Returns true if the token type is a qualifier keyword that may appear
/// between a VAR keyword and the first declaration (e.g. CONSTANT, RETAIN,
/// NON_RETAIN).
fn is_qualifier(tt: &TokenType) -> bool {
    matches!(
        tt,
        TokenType::Constant | TokenType::Retain | TokenType::NonRetain
    )
}

/// Returns true if the token is whitespace, a newline, or a comment —
/// tokens that carry no semantic meaning between keywords.
fn is_ignorable(tt: &TokenType) -> bool {
    matches!(
        tt,
        TokenType::Whitespace | TokenType::Newline | TokenType::Comment
    )
}

pub fn apply(tokens: &[Token], options: &CompilerOptions) -> Result<(), Vec<Diagnostic>> {
    if options.allow_empty_var_blocks {
        return Ok(());
    }

    let mut errors = Vec::new();

    let mut i = 0;
    while i < tokens.len() {
        if is_var_keyword(&tokens[i].token_type) {
            let var_tok = &tokens[i];
            // Skip past the VAR keyword and any qualifiers/whitespace/comments
            let mut j = i + 1;
            while j < tokens.len() {
                let tt = &tokens[j].token_type;
                if is_ignorable(tt) || is_qualifier(tt) {
                    j += 1;
                } else {
                    break;
                }
            }
            // Check if the next meaningful token is END_VAR
            if j < tokens.len() && tokens[j].token_type == TokenType::EndVar {
                errors.push(Diagnostic::problem(
                    ironplc_problems::Problem::EmptyVarBlock,
                    Label::span(var_tok.span.clone(), "Empty variable block"),
                ));
            }
        }
        i += 1;
    }

    if !errors.is_empty() {
        return Err(errors);
    }
    Ok(())
}

#[cfg(test)]
mod test {
    use dsl::core::SourceSpan;

    use crate::{
        options::CompilerOptions,
        rule_no_empty_var_blocks::apply,
        token::{Token, TokenType},
    };

    fn make_token(token_type: TokenType) -> Token {
        Token {
            token_type,
            span: SourceSpan::default(),
            line: 1,
            col: 1,
            text: String::new(),
        }
    }

    #[test]
    fn apply_when_empty_var_block_and_not_allowed_then_error() {
        let tokens = vec![make_token(TokenType::Var), make_token(TokenType::EndVar)];

        let result = apply(
            &tokens,
            &CompilerOptions {
                allow_empty_var_blocks: false,
                ..CompilerOptions::default()
            },
        );
        assert!(result.is_err());
    }

    #[test]
    fn apply_when_empty_var_block_and_allowed_then_ok() {
        let tokens = vec![make_token(TokenType::Var), make_token(TokenType::EndVar)];

        let result = apply(
            &tokens,
            &CompilerOptions {
                allow_empty_var_blocks: true,
                ..CompilerOptions::default()
            },
        );
        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_var_with_qualifier_empty_and_not_allowed_then_error() {
        let tokens = vec![
            make_token(TokenType::Var),
            make_token(TokenType::Whitespace),
            make_token(TokenType::Constant),
            make_token(TokenType::Whitespace),
            make_token(TokenType::EndVar),
        ];

        let result = apply(
            &tokens,
            &CompilerOptions {
                allow_empty_var_blocks: false,
                ..CompilerOptions::default()
            },
        );
        assert!(result.is_err());
    }

    #[test]
    fn apply_when_empty_var_input_and_not_allowed_then_error() {
        let tokens = vec![
            make_token(TokenType::VarInput),
            make_token(TokenType::EndVar),
        ];

        let result = apply(
            &tokens,
            &CompilerOptions {
                allow_empty_var_blocks: false,
                ..CompilerOptions::default()
            },
        );
        assert!(result.is_err());
    }

    #[test]
    fn apply_when_non_empty_var_block_then_ok() {
        let tokens = vec![
            make_token(TokenType::Var),
            make_token(TokenType::Whitespace),
            make_token(TokenType::Identifier),
            make_token(TokenType::EndVar),
        ];

        let result = apply(
            &tokens,
            &CompilerOptions {
                allow_empty_var_blocks: false,
                ..CompilerOptions::default()
            },
        );
        assert!(result.is_ok());
    }
}
