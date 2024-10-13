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
                tokens.push(Token {
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
                });

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

/// Adds a semicolon after keyword statements to terminate the statement.
///
/// IEC 61131-3 requires a semicolon after each statement but many programs
/// do not have a semicolon after named keywords. This function inserts the
/// semicolon token after keyword statements (when the semicolon does not
/// exist) so that the token stream is valid.
pub fn insert_keyword_statement_terminators(input: Vec<Token>, _file_id: &FileId) -> Vec<Token> {
    let mut output = Vec::new();

    let mut in_end_statement = false;
    for tok in input {
        if !in_end_statement && tok.token_type == TokenType::EndIf {
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

#[cfg(test)]
mod test {
    use crate::token::TokenType;
    use ironplc_test::read_shared_resource;
    use logos::{Lexer, Logos};

    fn assert_no_err<'a>(lexer: &mut Lexer<'a, TokenType>) {
        while let Some(tok) = lexer.next() {
            println!("{:?} {:?}", tok, lexer.slice());
            assert!(!tok.is_err(), "{}", lexer.slice());
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
