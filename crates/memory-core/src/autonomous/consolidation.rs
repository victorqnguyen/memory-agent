use std::collections::HashSet;

/// Jaccard similarity between two text values based on whitespace-split terms.
pub fn term_similarity(a: &str, b: &str) -> f64 {
    let terms_a: HashSet<String> = a.split_whitespace().map(|w| w.to_lowercase()).collect();
    let terms_b: HashSet<String> = b.split_whitespace().map(|w| w.to_lowercase()).collect();

    let intersection = terms_a.intersection(&terms_b).count() as f64;
    let union = terms_a.union(&terms_b).count() as f64;

    if union == 0.0 {
        0.0
    } else {
        intersection / union
    }
}

/// Merge values from multiple memory texts, keeping all unique lines.
pub fn merge_values(values: &[&str]) -> String {
    let mut seen: HashSet<String> = HashSet::new();
    let mut merged_lines: Vec<String> = Vec::new();

    for value in values {
        for line in value.lines() {
            let normalized = line.trim().to_lowercase();
            if !normalized.is_empty() && seen.insert(normalized) {
                merged_lines.push(line.to_string());
            }
        }
    }

    merged_lines.join("\n")
}

/// A group of memory IDs that are candidates for consolidation.
#[derive(Debug, Clone)]
pub struct ConsolidationGroup {
    pub memory_ids: Vec<i64>,
    pub similarity: f64,
    pub key: String,
    pub scope: String,
}

/// Find pairs of memories in the given list that have high term overlap
/// and share the same key AND scope. Returns consolidation groups.
pub fn find_candidates(
    memories: &[(i64, String, String, String)], // (id, key, value, scope)
    threshold: f64,
) -> Vec<ConsolidationGroup> {
    let mut groups: Vec<ConsolidationGroup> = Vec::new();

    for i in 0..memories.len() {
        for j in (i + 1)..memories.len() {
            let (id_a, key_a, val_a, scope_a) = &memories[i];
            let (id_b, key_b, val_b, scope_b) = &memories[j];

            if key_a != key_b || scope_a != scope_b {
                continue;
            }

            let sim = term_similarity(val_a, val_b);
            if sim >= threshold {
                // Check if either ID is already in a group for this key
                let existing = groups.iter_mut().find(|g| {
                    g.key == *key_a
                        && (g.memory_ids.contains(id_a) || g.memory_ids.contains(id_b))
                });

                match existing {
                    Some(group) => {
                        if !group.memory_ids.contains(id_a) {
                            group.memory_ids.push(*id_a);
                        }
                        if !group.memory_ids.contains(id_b) {
                            group.memory_ids.push(*id_b);
                        }
                        if sim < group.similarity {
                            group.similarity = sim;
                        }
                    }
                    None => {
                        groups.push(ConsolidationGroup {
                            memory_ids: vec![*id_a, *id_b],
                            similarity: sim,
                            key: key_a.clone(),
                            scope: scope_a.clone(),
                        });
                    }
                }
            }
        }
    }

    groups
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_texts_have_similarity_one() {
        assert_eq!(term_similarity("hello world", "hello world"), 1.0);
    }

    #[test]
    fn disjoint_texts_have_similarity_zero() {
        assert_eq!(term_similarity("alpha beta", "gamma delta"), 0.0);
    }

    #[test]
    fn partial_overlap() {
        let sim = term_similarity("the quick brown fox", "the quick red fox");
        assert!(sim > 0.5 && sim < 1.0);
    }

    #[test]
    fn empty_texts() {
        assert_eq!(term_similarity("", ""), 0.0);
        assert_eq!(term_similarity("hello", ""), 0.0);
    }

    #[test]
    fn merge_deduplicates_lines() {
        let a = "line one\nline two\nline three";
        let b = "line two\nline four";
        let merged = merge_values(&[a, b]);
        assert_eq!(merged, "line one\nline two\nline three\nline four");
    }

    #[test]
    fn merge_preserves_original_case() {
        let a = "Use BUN for tests";
        let b = "use bun for tests\nAlso run clippy";
        let merged = merge_values(&[a, b]);
        // First occurrence wins (case-insensitive dedup)
        assert!(merged.contains("Use BUN for tests"));
        assert!(merged.contains("Also run clippy"));
        assert!(!merged.contains("use bun for tests"));
    }

    #[test]
    fn find_candidates_groups_same_key_and_scope() {
        let memories = vec![
            (1, "commands/test".to_string(), "run bun test".to_string(), "/proj".to_string()),
            (2, "commands/test".to_string(), "run bun test --watch".to_string(), "/proj".to_string()),
            (3, "commands/build".to_string(), "cargo build --release".to_string(), "/proj".to_string()),
        ];
        let groups = find_candidates(&memories, 0.5);
        assert_eq!(groups.len(), 1);
        assert!(groups[0].memory_ids.contains(&1));
        assert!(groups[0].memory_ids.contains(&2));
        assert_eq!(groups[0].scope, "/proj");
    }

    #[test]
    fn find_candidates_no_cross_key() {
        let memories = vec![
            (1, "key_a".to_string(), "same content here".to_string(), "/".to_string()),
            (2, "key_b".to_string(), "same content here".to_string(), "/".to_string()),
        ];
        let groups = find_candidates(&memories, 0.5);
        assert!(groups.is_empty());
    }

    #[test]
    fn find_candidates_no_cross_scope() {
        let memories = vec![
            (1, "k".to_string(), "same content here".to_string(), "/project/a".to_string()),
            (2, "k".to_string(), "same content here".to_string(), "/project/b".to_string()),
        ];
        let groups = find_candidates(&memories, 0.5);
        assert!(groups.is_empty());
    }

    #[test]
    fn find_candidates_below_threshold() {
        let memories = vec![
            (1, "k".to_string(), "alpha beta gamma delta".to_string(), "/".to_string()),
            (2, "k".to_string(), "epsilon zeta eta theta".to_string(), "/".to_string()),
        ];
        let groups = find_candidates(&memories, 0.5);
        assert!(groups.is_empty());
    }
}
