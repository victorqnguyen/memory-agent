/// Tier 1 fallback: extract the most information-dense sentences.
pub fn extract_key_sentences(text: &str, max: usize) -> String {
    let sentences: Vec<&str> = text
        .split(['.', '\n'])
        .map(|s| s.trim())
        .filter(|s| s.len() > 20)
        .collect();

    if sentences.is_empty() {
        return truncate(text, 200).to_string();
    }

    // Score by information density (specifics, code references, length)
    let mut scored: Vec<(&str, f64)> = sentences
        .iter()
        .map(|s| {
            let mut score = 0.0;
            if s.contains('/') || s.contains("::") || s.contains('_') {
                score += 1.0;
            }
            if s.contains('`') || s.contains('{') {
                score += 0.5;
            }
            let words = s.split_whitespace().count();
            if (5..50).contains(&words) {
                score += 0.5;
            }
            (*s, score)
        })
        .collect();

    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    scored
        .iter()
        .take(max)
        .map(|(s, _)| *s)
        .collect::<Vec<_>>()
        .join(". ")
}

#[cfg(feature = "local-llm")]
pub fn session_summary_prompt(activities: &str) -> String {
    format!(
        "Summarize this coding session in 2-3 sentences. Focus on decisions made and problems solved. Be specific about files and technologies.\n\n{}\n\nSummary:",
        truncate(activities, 1500)
    )
}

#[cfg(feature = "local-llm")]
pub fn compression_prompt(values: &[&str]) -> String {
    let combined = values.join("\n---\n");
    format!(
        "Merge these related notes into one concise note. Keep all facts, remove redundancy. Output only the merged note.\n\n{}\n\nMerged:",
        truncate(&combined, 1500)
    )
}

#[cfg(feature = "local-llm")]
pub fn procedural_extraction_prompt(skill_name: &str, outcome: &str) -> String {
    format!(
        "Extract the key pattern or lesson from this skill execution. Output a concise procedural memory in this format:\nSkill: {skill_name}\nPattern: <the reusable pattern>\nLearned: <what to remember>\n\nOutcome:\n{}\n\nProcedural memory:",
        truncate(outcome, 1000)
    )
}

/// Tier 1 fallback: extract search keywords from a user prompt.
/// Simple heuristic: remove stop words, take most specific terms.
pub fn extract_keywords(prompt: &str, max_terms: usize) -> Vec<String> {
    const STOP_WORDS: &[&str] = &[
        "a", "an", "the", "is", "are", "was", "were", "be", "been", "being", "have", "has", "had",
        "do", "does", "did", "will", "would", "could", "should", "may", "might", "can", "shall",
        "to", "of", "in", "for", "on", "with", "at", "by", "from", "as", "into", "through",
        "during", "before", "after", "and", "but", "or", "nor", "not", "so", "yet", "if", "then",
        "else", "when", "where", "how", "what", "which", "who", "that", "this", "these", "those",
        "it", "its", "i", "me", "my", "we", "our", "you", "your", "he", "she", "they", "them",
        "his", "her", "let", "make", "use", "using", "want", "need", "help", "please", "just",
        "also", "like", "get", "set", "all", "any", "some", "about",
    ];

    let words: Vec<String> = prompt
        .split(|c: char| c.is_whitespace() || c == ',' || c == ';' || c == '?')
        .map(|w| {
            w.trim_matches(|c: char| {
                !c.is_alphanumeric() && c != '_' && c != '-' && c != '.' && c != '/'
            })
        })
        .filter(|w| w.len() > 1)
        .filter(|w| !STOP_WORDS.contains(&w.to_lowercase().as_str()))
        .map(|w| w.to_string())
        .collect();

    // Prioritize: code-like terms first, then by length
    let mut scored: Vec<(String, f64)> = words
        .into_iter()
        .map(|w| {
            let mut score = 0.0;
            if w.contains('/') || w.contains("::") || w.contains('_') || w.contains('.') {
                score += 3.0; // file paths, modules, dotted names
            }
            if w.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) {
                score += 1.0; // proper nouns, type names
            }
            if w.len() > 5 {
                score += 1.0; // longer = more specific
            }
            (w, score)
        })
        .collect();

    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    scored.dedup_by(|a, b| a.0.to_lowercase() == b.0.to_lowercase());
    scored.into_iter().take(max_terms).map(|(w, _)| w).collect()
}

#[cfg(feature = "local-llm")]
pub fn keyword_extraction_prompt(prompt: &str) -> String {
    format!(
        "Extract 3-5 search keywords from this user message. Focus on technical terms, file names, concepts, and specific nouns. Output ONLY the keywords separated by spaces, nothing else.\n\nMessage: {}\n\nKeywords:",
        truncate(prompt, 500)
    )
}

#[cfg(feature = "local-llm")]
pub fn learning_extraction_prompt(transcript_excerpt: &str) -> String {
    format!(
        "Extract key decisions, patterns, or learnings from this conversation excerpt. Output each as a separate line starting with '- '. Focus on:\n- Architecture decisions\n- Bug fixes and their root causes\n- Patterns or conventions established\n- Important file paths or configurations\n\nSkip trivial observations. If nothing significant, output NONE.\n\n{}\n\nLearnings:",
        truncate(transcript_excerpt, 2000)
    )
}

#[cfg(feature = "local-llm")]
pub fn metrics_analysis_prompt(metrics_summary: &str) -> String {
    format!(
        "Analyze these memory system metrics and give 2-4 actionable recommendations. Focus on:\n- Memories with high injections but zero hits (wasting context tokens)\n- Memories that should be deleted, consolidated, or rewritten\n- Overall system health\n\nOutput each recommendation as a line starting with '- ACTION:' where ACTION is DELETE, REWRITE, MERGE, or KEEP.\n\n{}\n\nRecommendations:",
        truncate(metrics_summary, 1500)
    )
}

fn truncate(s: &str, max: usize) -> &str {
    if s.len() <= max {
        s
    } else {
        // Find char boundary at or before max (compatible with MSRV 1.85)
        let mut end = max;
        while end > 0 && !s.is_char_boundary(end) {
            end -= 1;
        }
        &s[..end]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_key_sentences_picks_specific() {
        let text = "This is generic fluff about nothing. Fixed the bug in src/store/memory_rs::save() by adding dedup check. Another boring sentence here about something.";
        let result = extract_key_sentences(text, 1);
        assert!(result.contains("memory_rs::save()"));
    }

    #[test]
    fn extract_key_sentences_handles_empty() {
        assert_eq!(extract_key_sentences("", 3), "");
    }

    #[test]
    fn extract_key_sentences_handles_short() {
        let result = extract_key_sentences("short", 3);
        assert_eq!(result, "short");
    }

    #[test]
    fn extract_keywords_filters_stop_words() {
        let result = extract_keywords("fix the authentication bug in the login handler", 5);
        assert!(!result.iter().any(|w| w == "the" || w == "in"));
        assert!(result
            .iter()
            .any(|w| w.to_lowercase().contains("authentication")
                || w.to_lowercase().contains("login")));
    }

    #[test]
    fn extract_keywords_prioritizes_code_terms() {
        let result = extract_keywords("update src/store/memory.rs to handle the error", 3);
        assert_eq!(result[0], "src/store/memory.rs");
    }

    #[test]
    fn extract_keywords_empty_prompt() {
        let result = extract_keywords("", 5);
        assert!(result.is_empty());
    }

    #[test]
    fn extract_keywords_short_prompt() {
        let result = extract_keywords("a", 5);
        assert!(result.is_empty());
    }

    #[test]
    #[cfg(feature = "local-llm")]
    fn session_summary_prompt_truncates() {
        let long = "a".repeat(3000);
        let prompt = session_summary_prompt(&long);
        assert!(prompt.len() < 2000);
    }
}
