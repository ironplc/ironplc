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
        // Columns and lengths count UTF-16 code units.
        let start = utf16_to_byte_offset(row, col);
        let end = utf16_to_byte_offset(row, col + t.length);
        out.push((line, col, &row[start..end], t.token_type));
    }
    out
}

/// The byte offset in `row` of the character that starts `units` UTF-16 code
/// units in, or the length of `row` when it is shorter.
fn utf16_to_byte_offset(row: &str, units: u32) -> usize {
    let mut seen: u32 = 0;
    for (offset, c) in row.char_indices() {
        if seen >= units {
            return offset;
        }
        seen += c.len_utf16() as u32;
    }
    row.len()
}
