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
}
