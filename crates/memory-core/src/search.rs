/// Sanitize user input for FTS5 queries.
/// Whitelist approach: only alphanumeric, spaces, hyphens, underscores, dots.
/// All FTS5 operators (NEAR, AND, OR, NOT, *, ^, column:) are stripped.
pub fn sanitize_fts_query(input: &str) -> String {
    let cleaned: String = input
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' || c == '.' {
                c
            } else {
                ' '
            }
        })
        .collect();

    let fts_operators = ["NEAR", "AND", "OR", "NOT"];

    let terms: Vec<String> = cleaned
        .split_whitespace()
        .filter(|term| {
            let upper = term.to_uppercase();
            !fts_operators.contains(&upper.as_str())
        })
        .filter(|term| !term.contains(':'))
        .map(|term| format!("\"{term}\""))
        .collect();

    terms.join(" ")
}

/// Build an OR fallback query from an already-sanitized AND query.
/// Returns `None` for single-term queries (OR fallback has no effect).
///
/// Input:  `"authentication" "auth" "security"`
/// Output: `Some("\"authentication\" OR \"auth\" OR \"security\"")`
pub fn make_or_fallback(and_query: &str) -> Option<String> {
    let terms: Vec<&str> = and_query.split_whitespace().collect();
    if terms.len() <= 1 {
        return None;
    }
    Some(terms.join(" OR "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_normal_query() {
        assert_eq!(sanitize_fts_query("hello world"), "\"hello\" \"world\"");
    }

    #[test]
    fn strips_sql_injection() {
        let result = sanitize_fts_query("'; DROP TABLE memories; --");
        // Special chars like ' and ; are stripped
        assert!(!result.contains(';'));
        assert!(!result.contains('\''));
        // Remaining words are safely quoted as FTS5 search terms
        // "--" becomes a quoted term (harmless in FTS5 MATCH)
        assert!(result.contains("\"DROP\""));
        assert!(result.contains("\"TABLE\""));
        assert!(result.contains("\"memories\""));
    }

    #[test]
    fn strips_fts5_operators() {
        let result = sanitize_fts_query("NEAR(password admin)");
        assert!(!result.contains("NEAR"));
    }

    #[test]
    fn strips_column_filters() {
        let result = sanitize_fts_query("key:* OR 1=1");
        assert!(!result.contains("key:"));
        assert!(!result.contains("OR"));
    }

    #[test]
    fn strips_special_chars() {
        let result = sanitize_fts_query("test * ^ { } ( )");
        assert_eq!(result, "\"test\"");
    }

    #[test]
    fn empty_input_returns_empty() {
        assert_eq!(sanitize_fts_query(""), "");
        assert_eq!(sanitize_fts_query("   "), "");
    }

    #[test]
    fn all_operators_returns_empty() {
        assert_eq!(sanitize_fts_query("AND OR NOT"), "");
    }

    #[test]
    fn preserves_hyphens_underscores_dots() {
        let result = sanitize_fts_query("my-key some_thing file.rs");
        assert_eq!(result, "\"my-key\" \"some_thing\" \"file.rs\"");
    }

    #[test]
    fn make_or_fallback_joins_with_or() {
        let and_q = sanitize_fts_query("authentication auth security");
        assert_eq!(and_q, "\"authentication\" \"auth\" \"security\"");
        let or_q = make_or_fallback(&and_q).unwrap();
        assert_eq!(or_q, "\"authentication\" OR \"auth\" OR \"security\"");
    }

    #[test]
    fn make_or_fallback_single_term_returns_none() {
        let and_q = sanitize_fts_query("authentication");
        assert!(make_or_fallback(&and_q).is_none());
    }

    #[test]
    fn make_or_fallback_empty_returns_none() {
        assert!(make_or_fallback("").is_none());
    }
}
