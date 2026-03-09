use std::collections::HashSet;

const ENTROPY_THRESHOLD: f64 = 0.35;

const FILLER_PHRASES: &[&str] = &[
    "i'll help",
    "let me",
    "sure thing",
    "of course",
    "no problem",
    "here's what",
    "as you can see",
    "happy to help",
    "certainly",
    "absolutely",
];

/// Score the information density of a memory value.
/// Returns 0.0 (no information) to 1.0 (high information).
pub fn information_score(value: &str) -> f64 {
    let words: Vec<&str> = value.split_whitespace().collect();
    let word_count = words.len() as f64;

    if word_count == 0.0 {
        return 0.0;
    }

    let mut score = 0.0;

    // Factor 1: Unique word ratio (vocabulary diversity)
    let unique: HashSet<String> = words.iter().map(|w| w.to_lowercase()).collect();
    let uniqueness = unique.len() as f64 / word_count;
    score += uniqueness * 0.25;

    // Factor 2: Contains specific identifiers (paths, qualified names, config keys)
    let has_specifics =
        value.contains('/') || value.contains("::") || value.contains('.') || value.contains('_');
    if has_specifics {
        score += 0.2;
    }

    // Factor 3: Contains code or structured data
    let has_code =
        value.contains('{') || value.contains('(') || value.contains('[') || value.contains('`');
    if has_code {
        score += 0.15;
    }

    // Factor 4: Length — very short is suspect, medium is ideal, very long is verbose
    let length_score = if word_count < 3.0 {
        0.0
    } else if word_count < 6.0 {
        0.1
    } else if word_count < 100.0 {
        0.2
    } else {
        0.15
    };
    score += length_score;

    // Factor 5: Filler phrases are a strong negative signal
    let lower = value.to_lowercase();
    let filler_count = FILLER_PHRASES
        .iter()
        .filter(|f| lower.contains(**f))
        .count();
    if filler_count > 0 {
        score -= 0.15 * filler_count as f64;
    } else {
        score += 0.05;
    }

    score.clamp(0.0, 1.0)
}

/// Returns true if the value has enough information density to store.
pub fn should_store(value: &str) -> bool {
    information_score(value) >= ENTROPY_THRESHOLD
}

/// Returns true if the value has enough information density for the given threshold.
pub fn should_store_with_threshold(value: &str, threshold: f64) -> bool {
    information_score(value) >= threshold
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn high_info_content_passes() {
        let value = "The auth module uses JWT tokens stored in HttpOnly cookies. \
                     See src/auth/middleware.rs::validate_token() for the verification flow.";
        assert!(should_store(value));
        assert!(information_score(value) > 0.6);
    }

    #[test]
    fn low_info_filler_rejected() {
        assert!(!should_store("Sure thing, I'll help you with that"));
        assert!(!should_store("Of course, let me take a look"));
    }

    #[test]
    fn empty_string_rejected() {
        assert!(!should_store(""));
        assert_eq!(information_score(""), 0.0);
    }

    #[test]
    fn terse_but_specific_passes() {
        // Short but contains specifics — should pass
        let value = "use bun, not npm. config in package.json";
        assert!(should_store(value));
    }

    #[test]
    fn code_snippet_passes() {
        let value = "cargo test -p memory-core -- search";
        assert!(should_store(value));
    }

    #[test]
    fn single_word_rejected() {
        assert!(!should_store("hello"));
    }

    #[test]
    fn score_capped_at_one() {
        let value = "src/store/memory.rs::save() handles upsert with blake3 dedup. \
                     See also store/dedup.rs for normalize() and hash_content().";
        assert!(information_score(value) <= 1.0);
    }

    #[test]
    fn custom_threshold() {
        let value = "test value with some content";
        let score = information_score(value);
        assert!(should_store_with_threshold(value, score - 0.01));
        assert!(!should_store_with_threshold(value, score + 0.01));
    }
}
