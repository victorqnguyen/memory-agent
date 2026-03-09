/// Query complexity level for adaptive retrieval depth.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Complexity {
    Simple,
    Moderate,
    Complex,
}

const QUESTION_WORDS: &[&str] = &["how", "why", "what", "when", "where", "which"];
const MULTI_CONCEPT_MARKERS: &[&str] = &[" and ", " with ", " across ", " between "];
const ABSTRACTION_WORDS: &[&str] = &[
    "architecture",
    "design",
    "pattern",
    "flow",
    "system",
    "structure",
    "approach",
    "strategy",
    "overview",
];

/// Estimate the complexity of a search query.
pub fn estimate_complexity(query: &str) -> Complexity {
    let words: Vec<&str> = query.split_whitespace().collect();
    let word_count = words.len();

    let has_question = query.contains('?')
        || words
            .first()
            .map(|w| QUESTION_WORDS.contains(&w.to_lowercase().as_str()))
            .unwrap_or(false);

    let lower = query.to_lowercase();
    let has_multi_concept = MULTI_CONCEPT_MARKERS.iter().any(|m| lower.contains(m));
    let has_abstraction = ABSTRACTION_WORDS.iter().any(|w| lower.contains(w));

    if has_multi_concept || has_abstraction || (has_question && word_count > 8) {
        Complexity::Complex
    } else if word_count <= 3 && !has_question {
        Complexity::Simple
    } else {
        Complexity::Moderate
    }
}

/// Return an adaptive result limit based on query complexity.
pub fn adaptive_limit(query: &str, base_limit: i32) -> i32 {
    match estimate_complexity(query) {
        Complexity::Simple => (base_limit as f64 * 0.5).max(3.0) as i32,
        Complexity::Moderate => base_limit,
        Complexity::Complex => (base_limit as f64 * 2.0).min(20.0) as i32,
    }
}

/// Estimate token count from text (rough: 1 token ≈ 4 chars).
pub fn estimate_tokens(text: &str) -> i32 {
    (text.len() as f64 / 4.0).ceil() as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_query() {
        assert_eq!(estimate_complexity("test command"), Complexity::Simple);
        assert_eq!(estimate_complexity("auth"), Complexity::Simple);
    }

    #[test]
    fn moderate_query() {
        assert_eq!(
            estimate_complexity("how does auth work"),
            Complexity::Moderate
        );
        assert_eq!(
            estimate_complexity("database connection setup guide"),
            Complexity::Moderate
        );
    }

    #[test]
    fn complex_query() {
        assert_eq!(
            estimate_complexity("how does the authentication flow work across services"),
            Complexity::Complex
        );
        assert_eq!(
            estimate_complexity("system architecture overview"),
            Complexity::Complex
        );
        assert_eq!(
            estimate_complexity("design pattern for user auth and session management"),
            Complexity::Complex
        );
    }

    #[test]
    fn adaptive_limit_scales() {
        let base = 10;
        assert_eq!(adaptive_limit("test", base), 5);
        assert_eq!(adaptive_limit("how does auth work", base), 10);
        assert_eq!(
            adaptive_limit("how does the authentication flow work across services", base),
            20
        );
    }

    #[test]
    fn adaptive_limit_respects_cap() {
        assert!(adaptive_limit("complex architecture design overview", 15) <= 20);
    }

    #[test]
    fn token_estimate() {
        assert_eq!(estimate_tokens("hello world"), 3); // 11 chars / 4 = 2.75 -> 3
        assert_eq!(estimate_tokens(""), 0);
    }
}
