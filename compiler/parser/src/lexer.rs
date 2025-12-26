//! Primary lexer for IEC 61131-3 language elements. The lexer transforms
//! text into tokens (tokens are the input to the parser).
//!
//! This lexer makes some simplifying assumptions:
//! * there are no pragmas
use dsl::{
    core::{FileId, SourceSpan},
    diagnostic::{Diagnostic, Label},
};
use logos::Logos;

use crate::token::{Token, TokenType};

/// Validates that a token represents a valid syntax construct
pub fn validate_token(token: &Token) -> bool {
    match &token.token_type {
        TokenType::TimeLiteral => validate_time_literal(&token.text),
        TokenType::StringWithLength => validate_string_with_length(&token.text),
        TokenType::Comment => validate_comment(&token.text),
        _ => true, // Other tokens are validated by the regex patterns
    }
}

/// Validates time literal format (T#5S, T#100MS, T#1.5S, etc.)
fn validate_time_literal(text: &str) -> bool {
    if !text.starts_with("T#") && !text.starts_with("t#") {
        return false;
    }
    
    let time_part = &text[2..];
    if time_part.is_empty() {
        return false;
    }
    
    // Find where digits (including decimal point) end and unit begins
    let mut digit_end = 0;
    let mut has_decimal = false;
    for (i, c) in time_part.chars().enumerate() {
        if c.is_ascii_digit() {
            digit_end = i + 1;
        } else if c == '.' && !has_decimal {
            has_decimal = true;
            digit_end = i + 1;
        } else {
            break;
        }
    }
    
    if digit_end == 0 || digit_end == time_part.len() {
        return false;
    }
    
    // Ensure we don't end with a decimal point
    if time_part.chars().nth(digit_end - 1) == Some('.') {
        return false;
    }
    
    let unit = &time_part[digit_end..];
    matches!(unit.to_uppercase().as_str(), "MS" | "S" | "M" | "H" | "D")
}

/// Validates STRING(n) format
fn validate_string_with_length(text: &str) -> bool {
    if !text.to_uppercase().starts_with("STRING(") || !text.ends_with(')') {
        return false;
    }
    
    let inner = &text[7..text.len()-1];
    if inner.is_empty() || !inner.chars().all(|c| c.is_ascii_digit()) {
        return false;
    }
    
    // Parse the number and check if it's greater than 0
    if let Ok(length) = inner.parse::<u32>() {
        length > 0
    } else {
        false
    }
}

/// Validates comment format and structure
fn validate_comment(text: &str) -> bool {
    if !text.starts_with("(*") || !text.ends_with("*)") {
        return false;
    }
    
    // Check for proper nesting - no unmatched (* or *) inside
    let inner = &text[2..text.len()-2];
    let mut depth = 0;
    let mut chars = inner.chars().peekable();
    
    while let Some(c) = chars.next() {
        if c == '(' && chars.peek() == Some(&'*') {
            chars.next(); // consume the '*'
            depth += 1;
        } else if c == '*' && chars.peek() == Some(&')') {
            chars.next(); // consume the ')'
            if depth == 0 {
                return false; // Unmatched closing
            }
            depth -= 1;
        }
    }
    
    depth == 0 // All nested comments should be closed
}

/// Tokenize a IEC 61131 program.
///
/// Returns a list of tokens and a list of diagnostics. This does not return a result
/// because we usually continue with parsing even if there are token errors because
/// that will give the context of what was wrong in the location with the error.
pub fn tokenize(source: &str, file_id: &FileId) -> (Vec<Token>, Vec<Diagnostic>) {
    let mut tokens = Vec::new();
    let mut diagnostics = Vec::new();
    let mut lexer = TokenType::lexer(source);

    let mut line: usize = 0;
    let mut col: usize = 0;

    while let Some(token) = lexer.next() {
        match token {
            Ok(token_type) => {
                let new_token = Token {
                    token_type: token_type.clone(),
                    span: SourceSpan {
                        // TODO this will be slow
                        file_id: file_id.clone(),
                        start: lexer.span().start,
                        end: lexer.span().end,
                    },
                    line,
                    col,
                    text: lexer.slice().into(),
                };

                // Validate the token
                if !validate_token(&new_token) {
                    let span = SourceSpan::range(lexer.span().start, lexer.span().end).with_file_id(file_id);
                    diagnostics.push(Diagnostic::problem(
                        ironplc_problems::Problem::UnexpectedToken,
                        Label::span(
                            span,
                            format!(
                                "Invalid token format '{}' at line {} column {}.",
                                lexer.slice(),
                                line + 1,
                                col + 1,
                            ),
                        ),
                    ));
                } else {
                    tokens.push(new_token);
                }

                match token_type {
                    TokenType::Newline => {
                        line += 1;
                        col = 0;
                    }
                    TokenType::Comment => {
                        // Comments can have new lines embedded
                        for c in lexer.slice().chars() {
                            match c {
                                '\n' => {
                                    line += 1;
                                    col = 0;
                                }
                                _ => {
                                    col += 0;
                                }
                            }
                        }
                    }
                    TokenType::LineComment => {
                        // Line comments don't contain newlines, just advance column
                        col += lexer.span().len();
                    }
                    _ => col += lexer.span().len(),
                }
            }
            Err(_) => {
                let span = lexer.span();
                let span = SourceSpan::range(span.start, span.end).with_file_id(file_id);
                diagnostics.push(Diagnostic::problem(
                    ironplc_problems::Problem::UnexpectedToken,
                    Label::span(
                        span,
                        format!(
                            "The text '{}' is not valid IEC 61131-3 text at line {} colum {}.",
                            lexer.slice(),
                            // Add +1 to the line and column because these are displayed to users
                            // having 1-index based positions.
                            line + 1,
                            col + 1,
                        ),
                    ),
                ))
            }
        }
    }

    (tokens, diagnostics)
}

#[cfg(test)]
mod test {
    use crate::token::TokenType;
    use ironplc_test::read_shared_resource;
    use logos::{Lexer, Logos};

    fn assert_no_err(lexer: &mut Lexer<'_, TokenType>) {
        while let Some(tok) = lexer.next() {
            assert!(tok.is_ok(), "{}", lexer.slice());
        }
    }

    #[test]
    fn tokenize_array() {
        let source = read_shared_resource("array.st");
        let mut lex = TokenType::lexer(source.as_str());
        assert_no_err(&mut lex);
    }

    #[test]
    fn tokenize_comment() {
        let source = read_shared_resource("comment.st");
        let mut lex = TokenType::lexer(source.as_str());
        assert_no_err(&mut lex);
    }

    #[test]
    fn tokenize_conditional() {
        let source = read_shared_resource("conditional.st");
        let mut lex = TokenType::lexer(source.as_str());
        assert_no_err(&mut lex);
    }

    #[test]
    fn tokenize_expressions() {
        let source = read_shared_resource("expressions.st");
        let mut lex = TokenType::lexer(source.as_str());
        assert_no_err(&mut lex);
    }

    #[test]
    fn tokenize_nested() {
        let source = read_shared_resource("nested.st");
        let mut lex = TokenType::lexer(source.as_str());
        assert_no_err(&mut lex);
    }

    #[test]
    fn tokenize_strings() {
        let source = read_shared_resource("strings.st");
        let mut lex = TokenType::lexer(source.as_str());
        assert_no_err(&mut lex);
    }

    #[test]
    fn tokenize_textual() {
        let source = read_shared_resource("textual.st");
        let mut lex = TokenType::lexer(source.as_str());
        assert_no_err(&mut lex);
    }

    #[test]
    fn tokenize_type_decl() {
        let source = read_shared_resource("type_decl.st");
        let mut lex = TokenType::lexer(source.as_str());
        assert_no_err(&mut lex);
    }

    #[test]
    fn tokenize_var_decl() {
        let source = read_shared_resource("var_decl.st");
        let mut lex = TokenType::lexer(source.as_str());
        assert_no_err(&mut lex);
    }

    #[test]
    fn tokenize_error() {
        // Starting text with question mark is never valid, so use this as a simple
        // check that we can return errors.
        let source = "?INVALID";
        let mut lex = TokenType::lexer(source);
        let token = lex.next();
        assert!(token.unwrap().is_err());
    }
}
