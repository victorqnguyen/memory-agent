/// Extract a structured procedural memory from skill outcome text.
///
/// Returns `None` if the outcome is trivial (< 50 chars) or contains no
/// recognisable pattern headers.
pub fn extract_procedural_memory(
    skill_name: &str,
    outcome: &str,
    files_changed: &[String],
) -> Option<String> {
    if outcome.trim().len() < 50 {
        return None;
    }

    let pattern = extract_field(outcome, &["Pattern:", "## Pattern", "**Pattern**:"]);
    let learned = extract_field(
        outcome,
        &[
            "Learned:",
            "## Learned",
            "**Learned**:",
            "Takeaway:",
            "## Takeaway",
            "**Takeaway**:",
        ],
    );
    let approach = extract_field(outcome, &["Approach:", "## Approach", "**Approach**:"]);

    // Require at least one recognised field
    if pattern.is_none() && learned.is_none() && approach.is_none() {
        return None;
    }

    let mut parts = vec![format!("Skill: {skill_name}")];

    if let Some(p) = pattern {
        parts.push(format!("Pattern: {p}"));
    }
    if let Some(a) = approach {
        parts.push(format!("Approach: {a}"));
    }
    if !files_changed.is_empty() {
        parts.push(format!("Files: {}", files_changed.join(", ")));
    }
    if let Some(l) = learned {
        parts.push(format!("Learned: {l}"));
    }

    Some(parts.join("\n"))
}

/// Extract the first line of content after one of the given header patterns.
fn extract_field(text: &str, headers: &[&str]) -> Option<String> {
    let lines: Vec<&str> = text.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        for header in headers {
            // Match "Header: content" on the same line
            if let Some(rest) = trimmed.strip_prefix(header) {
                let content = rest.trim();
                if !content.is_empty() {
                    return Some(content.to_string());
                }
            }
            // Match bare header line (e.g., "## Pattern") followed by next non-empty line
            if trimmed == header.trim_end_matches(':') {
                for next in &lines[i + 1..] {
                    let next_trimmed = next.trim();
                    if !next_trimmed.is_empty() {
                        return Some(next_trimmed.to_string());
                    }
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trivial_outcome_returns_none() {
        assert!(extract_procedural_memory("foo", "short", &[]).is_none());
        assert!(extract_procedural_memory("foo", "too short to matter", &[]).is_none());
    }

    #[test]
    fn test_no_headers_returns_none() {
        let long = "This is a long outcome but has no recognizable pattern headers at all here.";
        assert!(extract_procedural_memory("foo", long, &[]).is_none());
    }

    #[test]
    fn test_pattern_header_inline() {
        let outcome = "Pattern: Always use spawn_blocking for sync calls\nSome other text here that is long enough to matter for the test.";
        let result = extract_procedural_memory("dispatch", outcome, &[]);
        assert!(result.is_some());
        let text = result.unwrap();
        assert!(text.contains("Skill: dispatch"));
        assert!(text.contains("Pattern: Always use spawn_blocking for sync calls"));
    }

    #[test]
    fn test_learned_header() {
        let outcome = "We worked on the task and it went well overall.\nLearned: Mutex guards must be dropped before await points in async code.";
        let result = extract_procedural_memory("async-patterns", outcome, &["file.rs".to_string()]);
        assert!(result.is_some());
        let text = result.unwrap();
        assert!(text
            .contains("Learned: Mutex guards must be dropped before await points in async code."));
        assert!(text.contains("Files: file.rs"));
    }

    #[test]
    fn test_takeaway_header() {
        let outcome = "Completed the implementation successfully.\nTakeaway: Use json_each for SQLite array membership checks instead of LIKE.";
        let result = extract_procedural_memory("sqlite-search", outcome, &[]);
        assert!(result.is_some());
    }

    #[test]
    fn test_markdown_bold_header() {
        let outcome = "Finished work on auth module.\n**Pattern**: Always validate scope before storing to prevent path traversal attacks in memory keys.";
        let result = extract_procedural_memory("auth", outcome, &[]);
        assert!(result.is_some());
    }
}
