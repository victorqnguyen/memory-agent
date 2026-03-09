use std::path::Path;

use memory_core::{SaveParams, Store};

use super::config;

pub struct ExtractResult {
    pub extracted: i32,
    pub updated: i32,
    pub skipped: i32,
    pub files_scanned: Vec<String>,
}

struct FileExtractor {
    filename: &'static str,
    extract: fn(&str, &str) -> Vec<memory_core::types::ExtractedMemory>,
}

const EXTRACTORS: &[FileExtractor] = &[
    FileExtractor {
        filename: "package.json",
        extract: config::extract_package_json,
    },
    FileExtractor {
        filename: "Cargo.toml",
        extract: config::extract_cargo_toml,
    },
    FileExtractor {
        filename: ".env.example",
        extract: config::extract_env_example,
    },
    FileExtractor {
        filename: ".env.local",
        extract: config::extract_env_example,
    },
    FileExtractor {
        filename: "CLAUDE.md",
        extract: config::extract_claude_md,
    },
    FileExtractor {
        filename: ".cursorrules",
        extract: config::extract_rules_file,
    },
    FileExtractor {
        filename: ".windsurfrules",
        extract: config::extract_rules_file,
    },
    FileExtractor {
        filename: "Makefile",
        extract: config::extract_makefile,
    },
];

const IGNORE_FILE: &str = ".memory-agentignore";

/// Default contents for a new .memory-agentignore file.
pub const DEFAULT_IGNORE: &str = "\
# Files to exclude from memory-agent extract.
# One filename per line. Lines starting with # are comments.
# Glob patterns: * matches anything except /, ** not supported.
.env*
";

/// Parse a .memory-agentignore file and return the set of patterns.
fn load_ignore_patterns(dir: &Path) -> Vec<String> {
    let path = dir.join(IGNORE_FILE);
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    content
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| l.to_string())
        .collect()
}

/// Check if a filename matches any ignore pattern.
fn is_ignored(filename: &str, patterns: &[String]) -> bool {
    patterns.iter().any(|pat| glob_match(pat, filename))
}

/// Minimal glob matching — only `*` (match any chars) is supported.
fn glob_match(pattern: &str, text: &str) -> bool {
    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.len() == 1 {
        return pattern == text;
    }

    let mut pos = 0;
    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        if pos > text.len() {
            return false;
        }
        match text[pos..].find(part) {
            Some(idx) => {
                if i == 0 && idx != 0 {
                    return false;
                }
                pos += idx + part.len();
            }
            None => return false,
        }
    }
    if let Some(last) = parts.last() {
        if !last.is_empty() && !text.ends_with(last) {
            return false;
        }
    }
    true
}

pub fn scope_from_directory(dir: &Path) -> String {
    // Try package.json name
    if let Ok(content) = std::fs::read_to_string(dir.join("package.json")) {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
            if let Some(name) = json.get("name").and_then(|n| n.as_str()) {
                return format!("/project/{name}");
            }
        }
    }

    // Try Cargo.toml [package] name
    if let Ok(content) = std::fs::read_to_string(dir.join("Cargo.toml")) {
        if let Ok(toml_val) = content.parse::<toml::Value>() {
            if let Some(name) = toml_val
                .get("package")
                .and_then(|p| p.get("name"))
                .and_then(|n| n.as_str())
            {
                return format!("/project/{name}");
            }
        }
    }

    // Fallback: directory name
    let name = dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");
    format!("/project/{name}")
}

pub fn extract_from_directory(
    dir: &Path,
    store: &Store,
    scope: &str,
) -> anyhow::Result<ExtractResult> {
    let mut result = ExtractResult {
        extracted: 0,
        updated: 0,
        skipped: 0,
        files_scanned: Vec::new(),
    };

    let ignore_patterns = load_ignore_patterns(dir);

    for extractor in EXTRACTORS {
        if is_ignored(extractor.filename, &ignore_patterns) {
            result.skipped += 1;
            continue;
        }

        let file_path = dir.join(extractor.filename);
        let content = match std::fs::read_to_string(&file_path) {
            Ok(c) => c,
            Err(_) => {
                result.skipped += 1;
                continue;
            }
        };

        result.files_scanned.push(extractor.filename.to_string());

        let file_ref = file_path.display().to_string();
        let memories = (extractor.extract)(&content, &file_ref);

        for mem in memories {
            let action = store.save(SaveParams {
                key: mem.key,
                value: mem.value,
                scope: Some(scope.to_string()),
                source_type: Some(mem.source_type),
                source_ref: Some(mem.source_ref),
                source_commit: None,
                tags: Some(mem.tags),
            })?;

            match action {
                memory_core::SaveAction::Created(_) => result.extracted += 1,
                memory_core::SaveAction::Updated(_) => result.updated += 1,
                memory_core::SaveAction::Deduplicated(_) => result.updated += 1,
            }
        }
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glob_exact_match() {
        assert!(glob_match(".env.local", ".env.local"));
        assert!(!glob_match(".env.local", ".env.example"));
    }

    #[test]
    fn glob_star_suffix() {
        assert!(glob_match(".env*", ".env"));
        assert!(glob_match(".env*", ".env.local"));
        assert!(glob_match(".env*", ".env.example"));
        assert!(!glob_match(".env*", "package.json"));
    }

    #[test]
    fn glob_star_prefix() {
        assert!(glob_match("*.json", "package.json"));
        assert!(!glob_match("*.json", "Makefile"));
    }

    #[test]
    fn glob_star_middle() {
        assert!(glob_match("test_*_file", "test_my_file"));
        assert!(!glob_match("test_*_file", "test_my_thing"));
    }

    #[test]
    fn is_ignored_filters_env_files() {
        let patterns = vec![".env*".to_string()];
        assert!(is_ignored(".env.example", &patterns));
        assert!(is_ignored(".env.local", &patterns));
        assert!(!is_ignored("package.json", &patterns));
        assert!(!is_ignored("Cargo.toml", &patterns));
    }

    #[test]
    fn empty_patterns_ignores_nothing() {
        let patterns: Vec<String> = vec![];
        assert!(!is_ignored(".env.example", &patterns));
        assert!(!is_ignored("package.json", &patterns));
    }
}
