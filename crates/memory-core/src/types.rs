use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum SourceType {
    Codebase,
    Explicit,
    Observed,
    Derived,
    Procedural,
}

impl fmt::Display for SourceType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Codebase => write!(f, "codebase"),
            Self::Explicit => write!(f, "explicit"),
            Self::Observed => write!(f, "observed"),
            Self::Derived => write!(f, "derived"),
            Self::Procedural => write!(f, "procedural"),
        }
    }
}

impl std::str::FromStr for SourceType {
    type Err = crate::Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "codebase" => Ok(Self::Codebase),
            "explicit" => Ok(Self::Explicit),
            "observed" => Ok(Self::Observed),
            "derived" => Ok(Self::Derived),
            "procedural" => Ok(Self::Procedural),
            other => Err(crate::Error::InvalidSourceType(other.to_string())),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Memory {
    pub id: i64,
    pub key: String,
    pub value: String,
    pub scope: String,
    pub source_type: SourceType,
    pub source_ref: Option<String>,
    pub source_commit: Option<String>,
    pub confidence: f64,
    pub tags: Option<Vec<String>>,
    pub revision_count: i32,
    pub duplicate_count: i32,
    pub created_at: String,
    pub accessed_at: String,
    pub last_seen_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub id: i64,
    pub key: String,
    pub value_preview: String,
    pub scope: String,
    pub source_type: SourceType,
    pub confidence: f64,
    pub rank: f64,
}

#[derive(Debug, Clone, Default)]
pub struct SaveParams {
    pub key: String,
    pub value: String,
    pub scope: Option<String>,
    pub source_type: Option<SourceType>,
    pub source_ref: Option<String>,
    pub source_commit: Option<String>,
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Clone)]
pub struct SearchParams {
    pub query: String,
    pub scope: Option<String>,
    pub source_type: Option<SourceType>,
    pub limit: Option<i32>,
}

/// Result of a save operation indicating what action was taken
#[derive(Debug, Clone, PartialEq)]
pub enum SaveAction {
    Created(i64),
    Updated(i64),
    Deduplicated(i64),
}

impl SaveAction {
    pub fn id(&self) -> i64 {
        match self {
            Self::Created(id) | Self::Updated(id) | Self::Deduplicated(id) => *id,
        }
    }

    pub fn action_str(&self) -> &'static str {
        match self {
            Self::Created(_) => "created",
            Self::Updated(_) => "updated",
            Self::Deduplicated(_) => "deduplicated",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryMetric {
    pub id: i64,
    pub key: String,
    pub scope: String,
    pub injections: i32,
    pub hits: i32,
    pub tokens_injected: i32,
    pub hit_rate: f64,
}

#[derive(Debug, Clone, Default)]
pub struct TokenStats {
    pub injections: i64,
    pub hits: i64,
    pub unique_memories_injected: i64,
    pub tokens_injected: i64,
}

/// A single event in the live activity feed.
#[derive(Debug, Clone)]
pub struct EventLogEntry {
    pub id: i64,
    pub action: String,
    pub key: String,
    pub scope: String,
    pub tokens: i32,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct ExtractedMemory {
    pub key: String,
    pub value: String,
    pub source_type: SourceType,
    pub source_ref: String,
    pub tags: Vec<String>,
}

/// Status snapshot for maintenance scheduling.
#[derive(Debug, Clone)]
pub struct MaintenanceStatus {
    pub vacuum_overdue: bool,
    pub last_vacuum_at: String,
    pub purge_candidates: u32,
}

pub fn normalize_scope(scope: &str) -> String {
    let trimmed = scope.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        "/".to_string()
    } else if !trimmed.starts_with('/') {
        format!("/{trimmed}")
    } else {
        trimmed.to_string()
    }
}

pub fn validate_scope(scope: &str) -> crate::Result<()> {
    if scope.contains("..") {
        return Err(crate::Error::InvalidScope("path traversal".to_string()));
    }
    if scope.contains('\0') {
        return Err(crate::Error::InvalidScope("null byte".to_string()));
    }
    Ok(())
}

/// Parse a source ref string into (file, start_line, end_line).
///
/// "src/auth.rs:15-45" => ("src/auth.rs", Some(15), Some(45))
/// "src/auth.rs:42"    => ("src/auth.rs", Some(42), Some(42))
/// "package.json"      => ("package.json", None, None)
pub fn parse_source_ref(source_ref: &str) -> (String, Option<usize>, Option<usize>) {
    if let Some((file, lines)) = source_ref.rsplit_once(':') {
        if let Some((start, end)) = lines.split_once('-') {
            let s = start.parse().ok();
            let e = end.parse().ok();
            if s.is_some() || e.is_some() {
                return (file.to_string(), s, e);
            }
        } else if let Ok(line) = lines.parse::<usize>() {
            return (file.to_string(), Some(line), Some(line));
        }
    }
    (source_ref.to_string(), None, None)
}

#[cfg(test)]
mod tests {
    use super::parse_source_ref;

    #[test]
    fn test_parse_source_ref_range() {
        let (file, start, end) = parse_source_ref("src/auth.rs:15-45");
        assert_eq!(file, "src/auth.rs");
        assert_eq!(start, Some(15));
        assert_eq!(end, Some(45));
    }

    #[test]
    fn test_parse_source_ref_single_line() {
        let (file, start, end) = parse_source_ref("src/auth.rs:42");
        assert_eq!(file, "src/auth.rs");
        assert_eq!(start, Some(42));
        assert_eq!(end, Some(42));
    }

    #[test]
    fn test_parse_source_ref_no_line() {
        let (file, start, end) = parse_source_ref("package.json");
        assert_eq!(file, "package.json");
        assert_eq!(start, None);
        assert_eq!(end, None);
    }

    #[test]
    fn test_parse_source_ref_non_numeric_suffix() {
        // colon present but lines part is not numeric — treat as plain file
        let (file, start, end) = parse_source_ref("src/auth.rs:main");
        assert_eq!(file, "src/auth.rs:main");
        assert_eq!(start, None);
        assert_eq!(end, None);
    }
}
