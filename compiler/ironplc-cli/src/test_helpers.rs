use std::path::PathBuf;

use lsp_types::SemanticToken;

#[cfg(test)]
pub fn resource_path(name: &'static str) -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("resources/test");
    path.push(name);
    path
}

/// Reconstruct a `(line, col, length, token_type)` table from the
/// delta-encoded `SemanticToken` stream and resolve each entry against
/// `source` to return the actual substring covered. Used by the
/// semantic token tests to detect regressions where the LSP overlay would land on
/// the wrong characters.
pub fn resolve_lsp_tokens<'a>(
    source: &'a str,
    tokens: &[SemanticToken],
) -> Vec<(u32, u32, &'a str, u32)> {
    let lines: Vec<&str> = source.split('\n').collect();
    let mut line: u32 = 0;
    let mut col: u32 = 0;
    let mut out = Vec::new();
    for t in tokens {
        if t.delta_line == 0 {
            col += t.delta_start;
        } else {
            line += t.delta_line;
            col = t.delta_start;
        }
        let row = lines.get(line as usize).copied().unwrap_or("");
        let start = col as usize;
        let end = (col + t.length) as usize;
        // Operate on bytes — the test inputs are ASCII-only.
        let slice = &row[start.min(row.len())..end.min(row.len())];
        out.push((line, col, slice, t.token_type));
    }
    out
}
