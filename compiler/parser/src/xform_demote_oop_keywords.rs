use crate::{
    options::CompilerOptions,
    token::{Token, TokenType},
};

/// Demote CODESYS/TwinCAT OOP keyword tokens (`EXTENDS`, `IMPLEMENTS`,
/// `INTERFACE`, `END_INTERFACE`, `ABSTRACT`) to identifiers when
/// `allow_oop_extensions` is not enabled.
///
/// These words are valid IEC 61131-3 identifiers (e.g. variable or type
/// names). Demoting them back to `Identifier` when the flag is off keeps
/// standard programs that happen to use these names parsing correctly,
/// matching the pattern used for Edition 3 keywords in
/// `xform_demote_edition3_keywords.rs`.
pub fn apply(tokens: &mut [Token], options: &CompilerOptions) {
    if options.allow_oop_extensions {
        return;
    }

    for tok in tokens.iter_mut() {
        if matches!(
            tok.token_type,
            TokenType::Extends
                | TokenType::Implements
                | TokenType::Interface
                | TokenType::EndInterface
                | TokenType::Abstract
        ) {
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
            allow_oop_extensions: false,
            ..CompilerOptions::default()
        }
    }

    fn opts_enabled() -> CompilerOptions {
        CompilerOptions {
            allow_oop_extensions: true,
            ..CompilerOptions::default()
        }
    }

    #[test]
    fn apply_when_extends_and_disabled_then_demoted_to_identifier() {
        let mut tokens = vec![make_token(TokenType::Extends, "EXTENDS")];
        apply(&mut tokens, &opts_disabled());
        assert_eq!(tokens[0].token_type, TokenType::Identifier);
        assert_eq!(tokens[0].text, "EXTENDS");
    }

    #[test]
    fn apply_when_extends_and_enabled_then_stays_keyword() {
        let mut tokens = vec![make_token(TokenType::Extends, "EXTENDS")];
        apply(&mut tokens, &opts_enabled());
        assert_eq!(tokens[0].token_type, TokenType::Extends);
    }

    #[test]
    fn apply_when_implements_and_disabled_then_demoted_to_identifier() {
        let mut tokens = vec![make_token(TokenType::Implements, "IMPLEMENTS")];
        apply(&mut tokens, &opts_disabled());
        assert_eq!(tokens[0].token_type, TokenType::Identifier);
    }

    #[test]
    fn apply_when_implements_and_enabled_then_stays_keyword() {
        let mut tokens = vec![make_token(TokenType::Implements, "IMPLEMENTS")];
        apply(&mut tokens, &opts_enabled());
        assert_eq!(tokens[0].token_type, TokenType::Implements);
    }

    #[test]
    fn apply_when_interface_and_disabled_then_demoted_to_identifier() {
        let mut tokens = vec![make_token(TokenType::Interface, "INTERFACE")];
        apply(&mut tokens, &opts_disabled());
        assert_eq!(tokens[0].token_type, TokenType::Identifier);
    }

    #[test]
    fn apply_when_interface_and_enabled_then_stays_keyword() {
        let mut tokens = vec![make_token(TokenType::Interface, "INTERFACE")];
        apply(&mut tokens, &opts_enabled());
        assert_eq!(tokens[0].token_type, TokenType::Interface);
    }

    #[test]
    fn apply_when_end_interface_and_disabled_then_demoted_to_identifier() {
        let mut tokens = vec![make_token(TokenType::EndInterface, "END_INTERFACE")];
        apply(&mut tokens, &opts_disabled());
        assert_eq!(tokens[0].token_type, TokenType::Identifier);
    }

    #[test]
    fn apply_when_end_interface_and_enabled_then_stays_keyword() {
        let mut tokens = vec![make_token(TokenType::EndInterface, "END_INTERFACE")];
        apply(&mut tokens, &opts_enabled());
        assert_eq!(tokens[0].token_type, TokenType::EndInterface);
    }

    #[test]
    fn apply_when_abstract_and_disabled_then_demoted_to_identifier() {
        let mut tokens = vec![make_token(TokenType::Abstract, "ABSTRACT")];
        apply(&mut tokens, &opts_disabled());
        assert_eq!(tokens[0].token_type, TokenType::Identifier);
        assert_eq!(tokens[0].text, "ABSTRACT");
    }

    #[test]
    fn apply_when_abstract_and_enabled_then_stays_keyword() {
        let mut tokens = vec![make_token(TokenType::Abstract, "ABSTRACT")];
        apply(&mut tokens, &opts_enabled());
        assert_eq!(tokens[0].token_type, TokenType::Abstract);
    }

    #[test]
    fn apply_when_non_oop_token_then_unchanged() {
        let mut tokens = vec![make_token(TokenType::Int, "INT")];
        apply(&mut tokens, &opts_disabled());
        assert_eq!(tokens[0].token_type, TokenType::Int);
    }
}
