//! Provides string similarity functions for "did you mean?" suggestions
//! in diagnostics.
//!
//! Uses case-insensitive Levenshtein distance to find the closest match
//! among a set of candidate strings.

use std::cmp::min;

/// Computes the Levenshtein edit distance between two strings.
///
/// The comparison is case-insensitive since IEC 61131-3 identifiers
/// are case-insensitive.
fn levenshtein_distance(a: &str, b: &str) -> usize {
    let a_lower = a.to_lowercase();
    let b_lower = b.to_lowercase();
    let a_chars: Vec<char> = a_lower.chars().collect();
    let b_chars: Vec<char> = b_lower.chars().collect();
    let a_len = a_chars.len();
    let b_len = b_chars.len();

    if a_len == 0 {
        return b_len;
    }
    if b_len == 0 {
        return a_len;
    }

    // Use two rows instead of full matrix for space efficiency
    let mut prev_row: Vec<usize> = (0..=b_len).collect();
    let mut curr_row = vec![0; b_len + 1];

    for i in 1..=a_len {
        curr_row[0] = i;
        for j in 1..=b_len {
            let cost = if a_chars[i - 1] == b_chars[j - 1] {
                0
            } else {
                1
            };
            curr_row[j] = min(
                min(curr_row[j - 1] + 1, prev_row[j] + 1),
                prev_row[j - 1] + cost,
            );
        }
        std::mem::swap(&mut prev_row, &mut curr_row);
    }

    prev_row[b_len]
}

/// Finds the closest matching string from a set of candidates.
///
/// Returns the original (preserving case) version of the best match if
/// the edit distance is within a reasonable threshold. The threshold
/// scales with the length of the input string to avoid poor suggestions
/// for very short identifiers.
///
/// Returns `None` if no candidate is close enough.
pub fn find_closest_match<'a>(
    name: &str,
    candidates: impl Iterator<Item = &'a str>,
) -> Option<String> {
    // Threshold: allow up to ~1/3 of the name length in edits, minimum 1, maximum 3
    let max_distance = (name.len() / 3).clamp(1, 3);

    let mut best_match: Option<(String, usize)> = None;

    for candidate in candidates {
        let distance = levenshtein_distance(name, candidate);
        if distance > 0 && distance <= max_distance {
            match &best_match {
                None => best_match = Some((candidate.to_string(), distance)),
                Some((_, best_distance)) => {
                    if distance < *best_distance {
                        best_match = Some((candidate.to_string(), distance));
                    }
                }
            }
        }
    }

    best_match.map(|(name, _)| name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levenshtein_distance_when_identical_then_zero() {
        assert_eq!(levenshtein_distance("counter", "counter"), 0);
    }

    #[test]
    fn levenshtein_distance_when_case_differs_then_zero() {
        assert_eq!(levenshtein_distance("Counter", "counter"), 0);
    }

    #[test]
    fn levenshtein_distance_when_one_char_different_then_one() {
        assert_eq!(levenshtein_distance("conter", "counter"), 1);
    }

    #[test]
    fn levenshtein_distance_when_empty_string_then_length() {
        assert_eq!(levenshtein_distance("", "abc"), 3);
        assert_eq!(levenshtein_distance("abc", ""), 3);
    }

    #[test]
    fn levenshtein_distance_when_both_empty_then_zero() {
        assert_eq!(levenshtein_distance("", ""), 0);
    }

    #[test]
    fn levenshtein_distance_when_transposition_then_two() {
        assert_eq!(levenshtein_distance("ab", "ba"), 2);
    }

    #[test]
    fn find_closest_match_when_close_match_exists_then_returns_match() {
        let candidates = vec!["counter", "timer", "flag"];
        let result = find_closest_match("conter", candidates.into_iter());
        assert_eq!(result, Some("counter".to_string()));
    }

    #[test]
    fn find_closest_match_when_no_close_match_then_returns_none() {
        let candidates = vec!["completely", "different", "names"];
        let result = find_closest_match("counter", candidates.into_iter());
        assert!(result.is_none());
    }

    #[test]
    fn find_closest_match_when_empty_candidates_then_returns_none() {
        let candidates: Vec<&str> = vec![];
        let result = find_closest_match("counter", candidates.into_iter());
        assert!(result.is_none());
    }

    #[test]
    fn find_closest_match_when_exact_match_then_returns_none() {
        // Exact matches have distance 0, which we skip (not useful as suggestion)
        let candidates = vec!["counter"];
        let result = find_closest_match("counter", candidates.into_iter());
        assert!(result.is_none());
    }

    #[test]
    fn find_closest_match_when_case_insensitive_exact_then_returns_none() {
        let candidates = vec!["Counter"];
        let result = find_closest_match("counter", candidates.into_iter());
        assert!(result.is_none());
    }

    #[test]
    fn find_closest_match_when_multiple_close_then_returns_closest() {
        let candidates = vec!["countr", "conter", "counter"];
        // "counter" is exact (distance 0, skipped), "conter" has distance 1 to "counter"
        let result = find_closest_match("countr", candidates.into_iter());
        // "countr" vs "conter" = distance 2, "countr" vs "counter" = distance 1
        assert_eq!(result, Some("counter".to_string()));
    }

    #[test]
    fn find_closest_match_when_short_name_then_threshold_is_one() {
        // For "ab" (len 2), threshold = max(2/3, 1) = 1
        let candidates = vec!["ac"];
        let result = find_closest_match("ab", candidates.into_iter());
        assert_eq!(result, Some("ac".to_string()));
    }

    #[test]
    fn find_closest_match_when_short_name_exceeds_threshold_then_none() {
        // For "ab" (len 2), threshold = 1. "xy" has distance 2
        let candidates = vec!["xy"];
        let result = find_closest_match("ab", candidates.into_iter());
        assert!(result.is_none());
    }
}
