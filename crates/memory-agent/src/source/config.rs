use memory_core::types::{ExtractedMemory, SourceType};

/// Extract memories from package.json content.
/// `has_lockfile` indicates whether a lockfile exists in the same directory.
pub fn extract_package_json(content: &str, file_path: &str) -> Vec<ExtractedMemory> {
    let Ok(json) = serde_json::from_str::<serde_json::Value>(content) else {
        return vec![];
    };

    let mut results = Vec::new();

    // Package name
    if let Some(name) = json.get("name").and_then(|v| v.as_str()) {
        results.push(ExtractedMemory {
            key: "project/name".to_string(),
            value: name.to_string(),
            source_type: SourceType::Codebase,
            source_ref: file_path.to_string(),
            tags: vec!["project".to_string(), "package-json".to_string()],
        });
    }

    // Package manager detection from packageManager field
    if let Some(pm) = json.get("packageManager").and_then(|v| v.as_str()) {
        let manager = pm.split('@').next().unwrap_or(pm);
        results.push(ExtractedMemory {
            key: "tooling/package-manager".to_string(),
            value: manager.to_string(),
            source_type: SourceType::Codebase,
            source_ref: file_path.to_string(),
            tags: vec!["tooling".to_string(), "package-manager".to_string()],
        });
    }

    // Scripts
    if let Some(scripts) = json.get("scripts").and_then(|v| v.as_object()) {
        let script_names: Vec<String> = scripts.keys().cloned().collect();
        if !script_names.is_empty() {
            results.push(ExtractedMemory {
                key: "project/scripts".to_string(),
                value: script_names.join(", "),
                source_type: SourceType::Codebase,
                source_ref: file_path.to_string(),
                tags: vec!["project".to_string(), "scripts".to_string()],
            });
        }
    }

    // Key dependencies to track
    let key_deps = [
        "react",
        "react-dom",
        "next",
        "vue",
        "express",
        "vitest",
        "jest",
        "prisma",
        "@prisma/client",
        "tailwindcss",
        "typescript",
        "vite",
        "webpack",
        "esbuild",
        "fastify",
        "koa",
        "hono",
        "drizzle-orm",
        "mongoose",
        "sequelize",
        "@tanstack/react-query",
        "zustand",
        "redux",
        "@reduxjs/toolkit",
        "trpc",
        "@trpc/server",
        "graphql",
        "apollo-server",
    ];

    let all_deps: Vec<(&str, &serde_json::Value)> = [
        json.get("dependencies"),
        json.get("devDependencies"),
        json.get("peerDependencies"),
    ]
    .into_iter()
    .flatten()
    .filter_map(|v| v.as_object())
    .flat_map(|m| m.iter().map(|(k, v)| (k.as_str(), v)))
    .collect();

    let mut found_key_deps: Vec<String> = Vec::new();
    for (dep_name, dep_version) in &all_deps {
        if key_deps.contains(dep_name) {
            let version = dep_version.as_str().unwrap_or("*").to_string();
            found_key_deps.push(format!("{}@{}", dep_name, version));
        }
    }

    if !found_key_deps.is_empty() {
        results.push(ExtractedMemory {
            key: "project/key-dependencies".to_string(),
            value: found_key_deps.join(", "),
            source_type: SourceType::Codebase,
            source_ref: file_path.to_string(),
            tags: vec!["dependencies".to_string(), "package-json".to_string()],
        });
    }

    results
}

/// Extract memories from Cargo.toml content.
pub fn extract_cargo_toml(content: &str, file_path: &str) -> Vec<ExtractedMemory> {
    let Ok(toml_val) = content.parse::<toml::Value>() else {
        return vec![];
    };

    let mut results = Vec::new();

    if let Some(package) = toml_val.get("package") {
        if let Some(name) = package.get("name").and_then(|v| v.as_str()) {
            results.push(ExtractedMemory {
                key: "project/name".to_string(),
                value: name.to_string(),
                source_type: SourceType::Codebase,
                source_ref: file_path.to_string(),
                tags: vec!["project".to_string(), "cargo".to_string()],
            });
        }

        if let Some(edition) = package.get("edition").and_then(|v| v.as_str()) {
            results.push(ExtractedMemory {
                key: "tooling/rust-edition".to_string(),
                value: edition.to_string(),
                source_type: SourceType::Codebase,
                source_ref: file_path.to_string(),
                tags: vec!["tooling".to_string(), "rust".to_string()],
            });
        }

        if let Some(rust_version) = package.get("rust-version").and_then(|v| v.as_str()) {
            results.push(ExtractedMemory {
                key: "tooling/rust-version".to_string(),
                value: rust_version.to_string(),
                source_type: SourceType::Codebase,
                source_ref: file_path.to_string(),
                tags: vec!["tooling".to_string(), "rust".to_string()],
            });
        }
    }

    // Dependencies
    let key_dep_sections = ["dependencies", "dev-dependencies", "build-dependencies"];
    let mut dep_names: Vec<String> = Vec::new();

    for section in &key_dep_sections {
        if let Some(deps) = toml_val.get(*section).and_then(|v| v.as_table()) {
            for name in deps.keys() {
                dep_names.push(name.clone());
            }
        }
    }

    if !dep_names.is_empty() {
        results.push(ExtractedMemory {
            key: "project/dependencies".to_string(),
            value: dep_names.join(", "),
            source_type: SourceType::Codebase,
            source_ref: file_path.to_string(),
            tags: vec!["dependencies".to_string(), "cargo".to_string()],
        });
    }

    // Workspace members if it's a workspace
    if let Some(workspace) = toml_val.get("workspace") {
        if let Some(members) = workspace.get("members").and_then(|v| v.as_array()) {
            let member_names: Vec<String> = members
                .iter()
                .filter_map(|m| m.as_str())
                .map(|s| s.to_string())
                .collect();
            if !member_names.is_empty() {
                results.push(ExtractedMemory {
                    key: "project/workspace-members".to_string(),
                    value: member_names.join(", "),
                    source_type: SourceType::Codebase,
                    source_ref: file_path.to_string(),
                    tags: vec!["project".to_string(), "cargo".to_string(), "workspace".to_string()],
                });
            }
        }
    }

    results
}

/// Extract variable names (never values) from .env.example or .env.local files.
pub fn extract_env_example(content: &str, file_path: &str) -> Vec<ExtractedMemory> {
    let mut var_names: Vec<String> = Vec::new();

    for line in content.lines() {
        let line = line.trim();
        // Skip empty lines and comments
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        // Extract only the variable name (before the = sign)
        if let Some(eq_pos) = line.find('=') {
            let name = line[..eq_pos].trim().to_string();
            if !name.is_empty() {
                var_names.push(name);
            }
        }
    }

    if var_names.is_empty() {
        return vec![];
    }

    vec![ExtractedMemory {
        key: "project/env-variables".to_string(),
        value: var_names.join(", "),
        source_type: SourceType::Codebase,
        source_ref: file_path.to_string(),
        tags: vec!["configuration".to_string(), "env".to_string()],
    }]
}

/// Extract content from CLAUDE.md, chunking by heading to avoid truncation.
/// Files under 1800 chars are stored as a single memory; larger files are
/// split into sections by top-level markdown headings.
pub fn extract_claude_md(content: &str, file_path: &str) -> Vec<ExtractedMemory> {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return vec![];
    }

    // Small files: single memory
    if trimmed.len() <= 1800 {
        return vec![ExtractedMemory {
            key: "project/claude-instructions".to_string(),
            value: trimmed.to_string(),
            source_type: SourceType::Explicit,
            source_ref: file_path.to_string(),
            tags: vec!["instructions".to_string(), "claude-md".to_string()],
        }];
    }

    // Large files: chunk by ## headings
    let mut sections: Vec<(String, String)> = Vec::new();
    let mut current_heading = String::new();
    let mut current_body = String::new();

    for line in trimmed.lines() {
        if line.starts_with("## ") || (line.starts_with("# ") && !current_heading.is_empty()) {
            if !current_body.trim().is_empty() || !current_heading.is_empty() {
                sections.push((current_heading.clone(), current_body.trim().to_string()));
            }
            current_heading = line
                .trim_start_matches('#')
                .trim()
                .to_lowercase()
                .replace(' ', "-");
            current_body.clear();
        } else {
            current_body.push_str(line);
            current_body.push('\n');
        }
    }
    if !current_body.trim().is_empty() || !current_heading.is_empty() {
        sections.push((current_heading, current_body.trim().to_string()));
    }

    if sections.is_empty() {
        return vec![ExtractedMemory {
            key: "project/claude-instructions".to_string(),
            value: trimmed.to_string(),
            source_type: SourceType::Explicit,
            source_ref: file_path.to_string(),
            tags: vec!["instructions".to_string(), "claude-md".to_string()],
        }];
    }

    sections
        .into_iter()
        .filter(|(_, body)| !body.is_empty())
        .map(|(heading, body)| {
            let key = if heading.is_empty() {
                "project/claude-instructions".to_string()
            } else {
                format!("project/claude-instructions/{heading}")
            };
            ExtractedMemory {
                key,
                value: body,
                source_type: SourceType::Explicit,
                source_ref: file_path.to_string(),
                tags: vec!["instructions".to_string(), "claude-md".to_string()],
            }
        })
        .collect()
}

/// Extract content from .cursorrules or .windsurfrules.
/// Large files are chunked to avoid truncation.
pub fn extract_rules_file(content: &str, file_path: &str) -> Vec<ExtractedMemory> {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return vec![];
    }

    let base_key = if file_path.contains("cursor") {
        "project/cursor-rules"
    } else if file_path.contains("windsurf") {
        "project/windsurf-rules"
    } else {
        "project/rules"
    };

    if trimmed.len() <= 1800 {
        return vec![ExtractedMemory {
            key: base_key.to_string(),
            value: trimmed.to_string(),
            source_type: SourceType::Explicit,
            source_ref: file_path.to_string(),
            tags: vec!["instructions".to_string(), "rules".to_string()],
        }];
    }

    // Chunk by paragraphs (double newline)
    let chunks: Vec<&str> = trimmed.split("\n\n").filter(|c| !c.trim().is_empty()).collect();
    let mut results = Vec::new();
    let mut buffer = String::new();
    let mut idx = 0;

    for chunk in chunks {
        if buffer.len() + chunk.len() > 1800 && !buffer.is_empty() {
            results.push(ExtractedMemory {
                key: format!("{base_key}/part-{idx}"),
                value: buffer.trim().to_string(),
                source_type: SourceType::Explicit,
                source_ref: file_path.to_string(),
                tags: vec!["instructions".to_string(), "rules".to_string()],
            });
            buffer.clear();
            idx += 1;
        }
        if !buffer.is_empty() {
            buffer.push_str("\n\n");
        }
        buffer.push_str(chunk);
    }

    if !buffer.trim().is_empty() {
        results.push(ExtractedMemory {
            key: if idx == 0 { base_key.to_string() } else { format!("{base_key}/part-{idx}") },
            value: buffer.trim().to_string(),
            source_type: SourceType::Explicit,
            source_ref: file_path.to_string(),
            tags: vec!["instructions".to_string(), "rules".to_string()],
        });
    }

    results
}

/// Extract target names from a Makefile.
pub fn extract_makefile(content: &str, file_path: &str) -> Vec<ExtractedMemory> {
    let mut targets: Vec<String> = Vec::new();

    for line in content.lines() {
        // A make target line starts with a word character and contains ':' not preceded by '/'
        // (exclude pattern rules like %.o: %.c and variable assignments)
        let trimmed = line.trim_end();
        if trimmed.starts_with('\t') || trimmed.starts_with('#') || trimmed.is_empty() {
            continue;
        }
        // Target lines: don't start with whitespace, contain ':', not a variable assignment
        if let Some(colon_pos) = trimmed.find(':') {
            let target = trimmed[..colon_pos].trim();
            // Skip variable assignments (contain '=') and pattern rules (contain '%')
            if target.contains('=') || target.contains('%') || target.is_empty() {
                continue;
            }
            // Skip if the target name contains spaces (like "ifeq (x, y):")
            if target.contains(' ') {
                continue;
            }
            targets.push(target.to_string());
        }
    }

    if targets.is_empty() {
        return vec![];
    }

    vec![ExtractedMemory {
        key: "project/make-targets".to_string(),
        value: targets.join(", "),
        source_type: SourceType::Codebase,
        source_ref: file_path.to_string(),
        tags: vec!["tooling".to_string(), "makefile".to_string()],
    }]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_package_json_basic() {
        let content = r#"{"name":"my-app","scripts":{"build":"tsc","test":"vitest"},"dependencies":{"react":"^18.0.0","next":"^14.0.0"}}"#;
        let results = extract_package_json(content, "package.json");
        assert!(!results.is_empty());
        let keys: Vec<&str> = results.iter().map(|m| m.key.as_str()).collect();
        assert!(keys.contains(&"project/name"));
        assert!(keys.contains(&"project/scripts"));
        assert!(keys.contains(&"project/key-dependencies"));
    }

    #[test]
    fn test_extract_package_json_invalid() {
        let results = extract_package_json("not json", "package.json");
        assert!(results.is_empty());
    }

    #[test]
    fn test_extract_cargo_toml_basic() {
        let content = "[package]\nname = \"my-crate\"\nedition = \"2021\"\n\n[dependencies]\nserde = \"1\"\n";
        let results = extract_cargo_toml(content, "Cargo.toml");
        assert!(!results.is_empty());
        let keys: Vec<&str> = results.iter().map(|m| m.key.as_str()).collect();
        assert!(keys.contains(&"project/name"));
        assert!(keys.contains(&"tooling/rust-edition"));
        assert!(keys.contains(&"project/dependencies"));
    }

    #[test]
    fn test_extract_env_example_names_only() {
        let content = "DATABASE_URL=postgres://localhost/db\nSECRET_KEY=super-secret\n# comment\nAPI_KEY=\n";
        let results = extract_env_example(content, ".env.example");
        assert_eq!(results.len(), 1);
        let val = &results[0].value;
        assert!(val.contains("DATABASE_URL"));
        assert!(val.contains("SECRET_KEY"));
        assert!(val.contains("API_KEY"));
        // Values must not be stored
        assert!(!val.contains("postgres://localhost/db"));
        assert!(!val.contains("super-secret"));
    }

    #[test]
    fn test_extract_env_example_empty() {
        let results = extract_env_example("# just comments\n", ".env.example");
        assert!(results.is_empty());
    }

    #[test]
    fn test_extract_claude_md() {
        let content = "# Project Instructions\nDo things this way.\n";
        let results = extract_claude_md(content, "CLAUDE.md");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].key, "project/claude-instructions");
        assert!(matches!(results[0].source_type, SourceType::Explicit));
    }

    #[test]
    fn test_extract_rules_file_cursor() {
        let content = "Always use TypeScript.\n";
        let results = extract_rules_file(content, ".cursorrules");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].key, "project/cursor-rules");
    }

    #[test]
    fn test_extract_makefile_targets() {
        let content = "build:\n\tcargo build\n\ntest:\n\tcargo test\n\nclean:\n\trm -rf target\n\n.PHONY: build test clean\n";
        let results = extract_makefile(content, "Makefile");
        assert_eq!(results.len(), 1);
        let val = &results[0].value;
        assert!(val.contains("build"));
        assert!(val.contains("test"));
        assert!(val.contains("clean"));
    }

    #[test]
    fn test_extract_makefile_empty() {
        let results = extract_makefile("# just comments\n", "Makefile");
        assert!(results.is_empty());
    }
}
