use dsl::core::SourceSpan;

use crate::options::CompilerOptions;
use crate::token::{Token, TokenType};

/// Collapses `{ ... }` pragma blocks into a single opaque `Pragma` token.
///
/// TwinCAT and Siemens SCL emit curly-brace pragmas (e.g.
/// `{attribute 'qualified_only'}`) on nearly every declaration. IronPLC does
/// not yet interpret pragma contents, so they are treated as trivia: parsed
/// and discarded like a comment, with no diagnostic and no AST node.
///
/// Only runs when `options.allow_pragmas` is set; otherwise the token stream
/// is returned unchanged and `{`/`}` continue to surface as parse errors,
/// matching current behavior.
///
/// Pragmas do not nest — the first `RightBrace` encountered closes the run.
/// If a `LeftBrace` has no matching `RightBrace` before the end of the
/// stream, the tokens are left untouched — the `LeftBrace` will produce a
/// parse error downstream, same as today.
pub fn apply(tokens: Vec<Token>, options: &CompilerOptions) -> Vec<Token> {
    if !options.allow_pragmas {
        return tokens;
    }

    let mut output = Vec::with_capacity(tokens.len());
    let mut iter = tokens.into_iter().peekable();

    while let Some(tok) = iter.next() {
        if tok.token_type != TokenType::LeftBrace {
            output.push(tok);
            continue;
        }

        let mut run = vec![tok];
        let mut closed = false;
        for next in iter.by_ref() {
            let is_right_brace = next.token_type == TokenType::RightBrace;
            run.push(next);
            if is_right_brace {
                closed = true;
                break;
            }
        }

        if closed {
            output.push(collapse(&run));
        } else {
            // No matching RightBrace before EOF — leave the run untouched.
            output.extend(run);
        }
    }

    output
}

/// Combines a `LeftBrace ..= RightBrace` token run into a single `Pragma`
/// token spanning the original source range.
fn collapse(run: &[Token]) -> Token {
    let first = &run[0];
    let last = &run[run.len() - 1];
    let text = run.iter().map(|t| t.text.as_str()).collect::<String>();

    Token {
        token_type: TokenType::Pragma,
        span: SourceSpan::join(&first.span, &last.span),
        line: first.line,
        col: first.col,
        text,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::tokenize;
    use dsl::core::FileId;

    fn tokenize_with(source: &str) -> Vec<Token> {
        let (tokens, diagnostics) = tokenize(source, &FileId::default(), 0, 0);
        assert!(
            diagnostics.is_empty(),
            "unexpected diagnostics: {diagnostics:?}"
        );
        tokens
    }

    fn options_with_pragmas(allow: bool) -> CompilerOptions {
        CompilerOptions {
            allow_pragmas: allow,
            ..Default::default()
        }
    }

    #[test]
    fn apply_when_disabled_then_returns_tokens_unchanged() {
        let tokens = tokenize_with("{attribute 'qualified_only'}");
        let len_before = tokens.len();

        let result = apply(tokens, &options_with_pragmas(false));

        assert_eq!(result.len(), len_before);
        assert_eq!(result[0].token_type, TokenType::LeftBrace);
    }

    #[test]
    fn apply_when_simple_pragma_then_collapses_to_single_token() {
        let tokens = tokenize_with("{attribute 'qualified_only'}");

        let result = apply(tokens, &options_with_pragmas(true));

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].token_type, TokenType::Pragma);
        assert_eq!(result[0].text, "{attribute 'qualified_only'}");
    }

    #[test]
    fn apply_when_pragma_between_other_tokens_then_only_pragma_collapses() {
        let tokens = tokenize_with("TYPE {attribute 'strict'} END_TYPE");

        let result = apply(tokens, &options_with_pragmas(true));

        let types: Vec<&TokenType> = result.iter().map(|t| &t.token_type).collect();
        assert!(types.contains(&&TokenType::Pragma));
        assert!(types.contains(&&TokenType::Type));
        assert!(types.contains(&&TokenType::EndType));
        assert_eq!(
            result
                .iter()
                .filter(|t| t.token_type == TokenType::Pragma)
                .count(),
            1
        );
    }

    #[test]
    fn apply_when_unclosed_brace_then_left_as_is() {
        let tokens = tokenize_with("{attribute 'qualified_only'");

        let result = apply(tokens, &options_with_pragmas(true));

        assert!(result.iter().any(|t| t.token_type == TokenType::LeftBrace));
        assert!(!result.iter().any(|t| t.token_type == TokenType::Pragma));
    }

    #[test]
    fn apply_when_multiple_pragmas_then_each_collapses_independently() {
        let tokens = tokenize_with("{attribute 'qualified_only'}\n{attribute 'strict'}");

        let result = apply(tokens, &options_with_pragmas(true));

        let pragma_count = result
            .iter()
            .filter(|t| t.token_type == TokenType::Pragma)
            .count();
        assert_eq!(pragma_count, 2);
    }

    #[test]
    fn apply_when_pragma_then_span_covers_full_source_range() {
        let tokens = tokenize_with("{attribute 'qualified_only'}");
        let expected_end = tokens.last().unwrap().span.end;

        let result = apply(tokens, &options_with_pragmas(true));

        assert_eq!(result[0].span.start, 0);
        assert_eq!(result[0].span.end, expected_end);
    }
}
