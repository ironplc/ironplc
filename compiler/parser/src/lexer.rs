//! Primary lexer for IEC 61131-3 language elements. The lexer transforms
//! text into tokens (tokens are the input to the parser).
//!
//! This lexer makes some simplifying assumptions:
//! * there are no pragmas
use dsl::{
    core::FileId,
    diagnostic::{Diagnostic, Label},
};
use logos::Logos;

use crate::token::{Position, Token, TokenType};

/// Tokenize a IEC 61131 program.
///
/// Returns a list of tokens and a list of diagnostics. This does not return a result
/// because we usually continue with parsing even if there are token errors because
/// that will give the context of what was wrong in the location with the error.
pub fn tokenize(source: &str, file_id: &FileId) -> (Vec<Token>, Vec<Diagnostic>) {
    let mut tokens = Vec::new();
    let mut diagnostics = Vec::new();
    let mut lexer = TokenType::lexer(source);

    let mut column = 0;
    let mut line = 0;

    while let Some(token) = lexer.next() {
        match token {
            Ok(token_type) => {
                tokens.push(Token {
                    token_type: token_type.clone(),
                    position: Position { line, column },
                    text: lexer.slice().into(),
                });

                match token_type {
                    TokenType::Newline => {
                        line += 1;
                        column = 0;
                    }
                    _ => column += lexer.span().len(),
                }
            }
            Err(_) => {
                let span = lexer.span();
                diagnostics.push(Diagnostic::problem(
                    ironplc_problems::Problem::UnexpectedToken,
                    Label::offset(
                        file_id.clone(),
                        span,
                        format!("The text {} does not match a token", lexer.slice()),
                    ),
                ))
            }
        }
    }

    (tokens, diagnostics)
}

#[cfg(test)]
mod test {
    use std::{fs, path::PathBuf};

    use crate::token::TokenType;
    use logos::{Lexer, Logos};

    pub fn read_resource(name: &'static str) -> String {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("../resources/test");
        path.push(name);

        fs::read_to_string(path.clone()).expect(format!("Unable to read file {:?}", path).as_str())
    }

    fn assert_no_err<'a>(lexer: &mut Lexer<'a, TokenType>) {
        while let Some(tok) = lexer.next() {
            println!("{:?} {:?}", tok, lexer.slice());
            assert!(!tok.is_err(), "{}", lexer.slice());
        }
    }

    #[test]
    fn tokenize_array() {
        let source = read_resource("array.st");
        let mut lex = TokenType::lexer(source.as_str());
        assert_no_err(&mut lex);
    }

    #[test]
    fn tokenize_comment() {
        let source = read_resource("comment.st");
        let mut lex = TokenType::lexer(source.as_str());
        assert_no_err(&mut lex);
    }

    #[test]
    fn tokenize_conditional() {
        let source = read_resource("conditional.st");
        let mut lex = TokenType::lexer(source.as_str());
        assert_no_err(&mut lex);
    }

    #[test]
    fn tokenize_expressions() {
        let source = read_resource("expressions.st");
        let mut lex = TokenType::lexer(source.as_str());
        assert_no_err(&mut lex);
    }

    #[test]
    fn tokenize_nested() {
        let source = read_resource("nested.st");
        let mut lex = TokenType::lexer(source.as_str());
        assert_no_err(&mut lex);
    }

    #[test]
    fn tokenize_strings() {
        let source = read_resource("strings.st");
        let mut lex = TokenType::lexer(source.as_str());
        assert_no_err(&mut lex);
    }

    #[test]
    fn tokenize_textual() {
        let source = read_resource("textual.st");
        let mut lex = TokenType::lexer(source.as_str());
        assert_no_err(&mut lex);
    }

    #[test]
    fn tokenize_type_decl() {
        let source = read_resource("type_decl.st");
        let mut lex = TokenType::lexer(source.as_str());
        assert_no_err(&mut lex);
    }

    #[test]
    fn tokenize_var_decl() {
        let source = read_resource("var_decl.st");
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
