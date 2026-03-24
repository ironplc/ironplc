use dsl::core::FileId;

use crate::options::ParseOptions;
use crate::token::{Token, TokenType};

/// Adds a semicolon after keyword statements to terminate the statement.
///
/// IEC 61131-3 requires a semicolon after each statement but many programs
/// do not have a semicolon after named keywords. This function inserts the
/// semicolon token after keyword statements (when the semicolon does not
/// exist) so that the token stream is valid.
///
/// This fixup is only applied when `options.allow_missing_semicolon` is set.
pub fn insert_keyword_statement_terminators(
    input: Vec<Token>,
    _file_id: &FileId,
    options: &ParseOptions,
) -> Vec<Token> {
    if !options.allow_missing_semicolon {
        return input;
    }

    let mut output = Vec::new();

    let mut in_end_statement = false;
    for tok in input {
        if !in_end_statement
            && matches!(
                tok.token_type,
                TokenType::EndIf
                    | TokenType::EndStruct
                    | TokenType::EndWhile
                    | TokenType::EndFor
                    | TokenType::EndCase
                    | TokenType::EndRepeat
            )
        {
            in_end_statement = true;
        } else if in_end_statement
            && tok.token_type != TokenType::Semicolon
            && tok.token_type != TokenType::Comment
            && tok.token_type != TokenType::Whitespace
        {
            // TODO remove the span and line/col
            output.push(Token {
                token_type: TokenType::Semicolon,
                span: tok.span.clone(),
                line: tok.line,
                col: tok.col,
                text: "".to_owned(),
            });
            in_end_statement = false;
        }

        output.push(tok);
    }

    output
}
