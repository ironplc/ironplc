//! Demote the `REFERENCE` keyword to an identifier unless the Beckhoff /
//! CODESYS `REFERENCE TO` extension is enabled.
//!
//! `REFERENCE` is a vendor-flag-gated keyword (`--allow-reference-to`), not an
//! edition-gated one, so it lives in its own transform rather than in
//! `xform_demote_edition3_keywords`. When the flag is off, `REFERENCE` behaves
//! like any ordinary identifier so existing programs that use it as a name keep
//! parsing. When the flag is on, the always-present `REFERENCE TO` grammar
//! productions fire. See `specs/design/reference-to-twincat.md`.

use crate::{
    options::CompilerOptions,
    token::{Token, TokenType},
};

/// Demote the `Reference` token to `Identifier` when `--allow-reference-to`
/// is not set.
pub fn apply(tokens: &mut [Token], options: &CompilerOptions) {
    if options.allow_reference_to {
        return;
    }
    for tok in tokens.iter_mut() {
        if tok.token_type == TokenType::Reference {
            tok.token_type = TokenType::Identifier;
        }
    }
}

#[cfg(test)]
mod tests {
    use dsl::core::SourceSpan;

    use crate::{
        options::CompilerOptions,
        token::{Token, TokenType},
        xform_demote_reference_keyword::apply,
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
            allow_reference_to: true,
            ..CompilerOptions::default()
        }
    }

    #[test]
    fn apply_when_reference_and_flag_off_then_demoted_to_identifier() {
        let mut tokens = vec![make_token(TokenType::Reference, "REFERENCE")];
        apply(&mut tokens, &CompilerOptions::default());
        assert_eq!(tokens[0].token_type, TokenType::Identifier);
        assert_eq!(tokens[0].text, "REFERENCE");
    }

    #[test]
    fn apply_when_reference_and_flag_on_then_kept_as_keyword() {
        let mut tokens = vec![make_token(TokenType::Reference, "REFERENCE")];
        apply(&mut tokens, &opts_enabled());
        assert_eq!(tokens[0].token_type, TokenType::Reference);
    }

    #[test]
    fn apply_when_non_reference_token_then_unchanged() {
        let mut tokens = vec![make_token(TokenType::Int, "INT")];
        apply(&mut tokens, &CompilerOptions::default());
        assert_eq!(tokens[0].token_type, TokenType::Int);
    }
}
