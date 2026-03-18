use crate::{
    options::ParseOptions,
    token::{Token, TokenType},
};

/// Demote IEC 61131-3 Edition 3 keyword tokens to identifiers when
/// Edition 3 mode is not enabled. This allows programs to use these
/// names (LTIME, REF_TO, REF, NULL) as regular identifiers in
/// Edition 2 mode.
pub fn apply(tokens: &mut [Token], options: &ParseOptions) {
    if options.allow_iec_61131_3_2013 {
        return;
    }
    for tok in tokens.iter_mut() {
        match tok.token_type {
            TokenType::Ltime
            | TokenType::Ldate
            | TokenType::Ltod
            | TokenType::Ldt
            | TokenType::RefTo
            | TokenType::Ref
            | TokenType::Null => {
                tok.token_type = TokenType::Identifier;
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use dsl::core::SourceSpan;

    use crate::{
        options::ParseOptions,
        token::{Token, TokenType},
        xform_demote_edition3_keywords::apply,
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

    fn opts_no_edition3() -> ParseOptions {
        ParseOptions {
            allow_iec_61131_3_2013: false,
            ..ParseOptions::default()
        }
    }

    fn opts_edition3() -> ParseOptions {
        ParseOptions {
            allow_iec_61131_3_2013: true,
            ..ParseOptions::default()
        }
    }

    #[test]
    fn apply_when_ltime_and_not_edition3_then_demoted_to_identifier() {
        let mut tokens = vec![make_token(TokenType::Ltime, "LTIME")];
        apply(&mut tokens, &opts_no_edition3());
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
    fn apply_when_ref_to_and_not_edition3_then_demoted_to_identifier() {
        let mut tokens = vec![make_token(TokenType::RefTo, "REF_TO")];
        apply(&mut tokens, &opts_no_edition3());
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
    fn apply_when_ref_and_not_edition3_then_demoted_to_identifier() {
        let mut tokens = vec![make_token(TokenType::Ref, "REF")];
        apply(&mut tokens, &opts_no_edition3());
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
    fn apply_when_null_and_not_edition3_then_demoted_to_identifier() {
        let mut tokens = vec![make_token(TokenType::Null, "NULL")];
        apply(&mut tokens, &opts_no_edition3());
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
    fn apply_when_ldate_and_not_edition3_then_demoted_to_identifier() {
        let mut tokens = vec![make_token(TokenType::Ldate, "LDATE")];
        apply(&mut tokens, &opts_no_edition3());
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
        apply(&mut tokens, &opts_no_edition3());
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
        apply(&mut tokens, &opts_no_edition3());
        assert_eq!(tokens[0].token_type, TokenType::Identifier);
        assert_eq!(tokens[0].text, "LDT");
    }

    #[test]
    fn apply_when_ldt_and_edition3_then_stays_keyword() {
        let mut tokens = vec![make_token(TokenType::Ldt, "LDT")];
        apply(&mut tokens, &opts_edition3());
        assert_eq!(tokens[0].token_type, TokenType::Ldt);
    }

    #[test]
    fn apply_when_non_edition3_token_then_unchanged() {
        let mut tokens = vec![make_token(TokenType::Int, "INT")];
        apply(&mut tokens, &opts_no_edition3());
        assert_eq!(tokens[0].token_type, TokenType::Int);
    }
}
