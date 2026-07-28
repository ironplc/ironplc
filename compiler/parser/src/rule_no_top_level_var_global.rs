use dsl::diagnostic::{Diagnostic, Label};

use crate::{
    options::CompilerOptions,
    token::{Token, TokenType},
};

/// Rejects `VAR_GLOBAL` declarations that appear at the top level of a file,
/// i.e. outside any `CONFIGURATION` or `RESOURCE` block.
///
/// In strict IEC 61131-3, `VAR_GLOBAL` is only permitted inside
/// `CONFIGURATION` and `RESOURCE`. Accepting it at the top level is a vendor
/// extension gated by `--allow-top-level-var-global`. When the flag is on the
/// rule is a no-op.
///
/// "Top level" is determined by tracking block-nesting depth over the token
/// stream: `CONFIGURATION`/`RESOURCE` open a block and
/// `END_CONFIGURATION`/`END_RESOURCE` close it. A `VAR_GLOBAL` keyword seen at
/// depth zero is top-level.
pub fn apply(tokens: &[Token], options: &CompilerOptions) -> Result<(), Vec<Diagnostic>> {
    if options.allow_top_level_var_global {
        return Ok(());
    }

    let mut errors = Vec::new();
    let mut depth: usize = 0;

    for tok in tokens {
        match tok.token_type {
            TokenType::Configuration | TokenType::Resource => depth += 1,
            TokenType::EndConfiguration | TokenType::EndResource => {
                depth = depth.saturating_sub(1);
            }
            TokenType::VarGlobal if depth == 0 => {
                errors.push(
                    Diagnostic::problem(
                        ironplc_problems::Problem::TopLevelVarGlobalNotAllowed,
                        Label::span(tok.span.clone(), "Top-level VAR_GLOBAL"),
                    )
                    .with_help(
                        "Move the VAR_GLOBAL block inside a CONFIGURATION (or RESOURCE) \
                         block, or select a dialect that supports top-level VAR_GLOBAL.",
                    ),
                );
            }
            _ => {}
        }
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
        rule_no_top_level_var_global::apply,
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
    fn apply_when_top_level_var_global_and_not_allowed_then_error() {
        let tokens = vec![
            make_token(TokenType::VarGlobal),
            make_token(TokenType::EndVar),
        ];

        let result = apply(
            &tokens,
            &CompilerOptions {
                allow_top_level_var_global: false,
                ..CompilerOptions::default()
            },
        );
        assert!(result.is_err());
    }

    #[test]
    fn apply_when_top_level_var_global_and_not_allowed_then_diagnostic_has_help() {
        let tokens = vec![
            make_token(TokenType::VarGlobal),
            make_token(TokenType::EndVar),
        ];

        let result = apply(
            &tokens,
            &CompilerOptions {
                allow_top_level_var_global: false,
                ..CompilerOptions::default()
            },
        );
        let diagnostics = result.unwrap_err();
        assert!(!diagnostics[0].help().is_empty());
    }

    #[test]
    fn apply_when_top_level_var_global_and_allowed_then_ok() {
        let tokens = vec![
            make_token(TokenType::VarGlobal),
            make_token(TokenType::EndVar),
        ];

        let result = apply(
            &tokens,
            &CompilerOptions {
                allow_top_level_var_global: true,
                ..CompilerOptions::default()
            },
        );
        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_var_global_inside_configuration_then_ok() {
        // CONFIGURATION cfg VAR_GLOBAL END_VAR ... END_CONFIGURATION
        let tokens = vec![
            make_token(TokenType::Configuration),
            make_token(TokenType::Identifier),
            make_token(TokenType::VarGlobal),
            make_token(TokenType::EndVar),
            make_token(TokenType::EndConfiguration),
        ];

        let result = apply(
            &tokens,
            &CompilerOptions {
                allow_top_level_var_global: false,
                ..CompilerOptions::default()
            },
        );
        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_var_global_inside_resource_then_ok() {
        // RESOURCE res ON PLC VAR_GLOBAL END_VAR ... END_RESOURCE
        let tokens = vec![
            make_token(TokenType::Resource),
            make_token(TokenType::Identifier),
            make_token(TokenType::VarGlobal),
            make_token(TokenType::EndVar),
            make_token(TokenType::EndResource),
        ];

        let result = apply(
            &tokens,
            &CompilerOptions {
                allow_top_level_var_global: false,
                ..CompilerOptions::default()
            },
        );
        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_top_level_var_global_after_closed_configuration_then_error() {
        // A configuration closes, then a top-level VAR_GLOBAL follows: the
        // depth counter must return to zero so the trailing block is flagged.
        let tokens = vec![
            make_token(TokenType::Configuration),
            make_token(TokenType::Identifier),
            make_token(TokenType::EndConfiguration),
            make_token(TokenType::VarGlobal),
            make_token(TokenType::EndVar),
        ];

        let result = apply(
            &tokens,
            &CompilerOptions {
                allow_top_level_var_global: false,
                ..CompilerOptions::default()
            },
        );
        assert!(result.is_err());
    }

    #[test]
    fn apply_when_no_var_global_then_ok() {
        let tokens = vec![
            make_token(TokenType::Program),
            make_token(TokenType::Identifier),
            make_token(TokenType::EndProgram),
        ];

        let result = apply(
            &tokens,
            &CompilerOptions {
                allow_top_level_var_global: false,
                ..CompilerOptions::default()
            },
        );
        assert!(result.is_ok());
    }
}
