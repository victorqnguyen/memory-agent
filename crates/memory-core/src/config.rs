use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub storage: StorageConfig,
    pub search: SearchConfig,
    pub validation: ValidationConfig,
    pub privacy: PrivacyConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct StorageConfig {
    pub retention_days: u32,
    pub vacuum_interval_secs: u64,
    pub max_db_size_mb: u64,
    pub busy_timeout_ms: u32,
    pub cache_size_kb: u32,
    pub dedup_window_secs: u64,
    pub encryption_enabled: bool,
    /// Minimum information score (0.0–1.0) for non-explicit saves.
    /// Set to 0.0 to disable filtering. Default: 0.35.
    pub entropy_threshold: f64,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            retention_days: 90,
            vacuum_interval_secs: 604800,
            max_db_size_mb: 500,
            busy_timeout_ms: 5000,
            cache_size_kb: 2048,
            dedup_window_secs: 900,
            encryption_enabled: false,
            entropy_threshold: 0.35,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SearchConfig {
    pub default_limit: u32,
    pub max_limit: u32,
    /// BM25 score cutoff. When set, only results with `bm25() < threshold` are returned.
    /// BM25 returns negative values for rare-term matches and positive values for common-term
    /// matches in large corpora. Example: `Some(-0.3)` drops near-zero noise in production.
    /// Default `None` means no threshold filtering (safe for small/test corpora).
    pub min_relevance_score: Option<f64>,
    /// Preview length in chars for the best match (position 0).
    pub preview_max_chars: usize,
    /// Preview length in chars for weak matches (position 3+).
    pub preview_min_chars: usize,
    /// Per-column BM25 weights. Keys are FTS5 column names; unknown keys are ignored.
    /// Defaults: key=10, value=1, tags=5, source_type=0.5, scope=0.5.
    pub column_weights: BTreeMap<String, f64>,
}

fn default_column_weights() -> BTreeMap<String, f64> {
    [
        ("key".to_string(), 10.0),
        ("value".to_string(), 1.0),
        ("tags".to_string(), 5.0),
        ("source_type".to_string(), 0.5),
        ("scope".to_string(), 0.5),
    ]
    .into_iter()
    .collect()
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            default_limit: 10,
            max_limit: 50,
            min_relevance_score: None,
            preview_max_chars: 400,
            preview_min_chars: 80,
            column_weights: default_column_weights(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ValidationConfig {
    pub max_key_length: usize,
    pub max_value_length: usize,
    pub max_tags: usize,
    pub max_tag_length: usize,
}

impl Default for ValidationConfig {
    fn default() -> Self {
        Self {
            max_key_length: 256,
            max_value_length: 2000,
            max_tags: 20,
            max_tag_length: 64,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PrivacyConfig {
    pub secret_patterns: Vec<String>,
    pub extra_patterns: Vec<String>,
    pub replace_defaults: bool,
    pub file_deny_list: Vec<String>,
}

impl Default for PrivacyConfig {
    fn default() -> Self {
        Self {
            secret_patterns: default_secret_patterns(),
            extra_patterns: Vec::new(),
            replace_defaults: false,
            file_deny_list: vec![
                ".env".into(),
                ".env.*".into(),
                "*.pem".into(),
                "*.key".into(),
                "*.p12".into(),
                "*.pfx".into(),
                "id_rsa".into(),
                "id_ed25519".into(),
                "id_ecdsa".into(),
                "*.secret".into(),
                "credentials.json".into(),
            ],
        }
    }
}

pub fn default_secret_patterns() -> Vec<String> {
    vec![
        r"AKIA[0-9A-Z]{16}".into(),
        r"-----BEGIN [A-Z ]*PRIVATE KEY-----".into(),
        r"(?i)(api[_-]?key|token|secret|password)\s*[:=]\s*\S+".into(),
        r"(?i)mongodb(\+srv)?://[^\s]+".into(),
        r"(?i)postgres(ql)?://[^\s]+".into(),
        r"(?i)mysql://[^\s]+".into(),
        r"(?i)redis://[^\s]+".into(),
        r"ghp_[a-zA-Z0-9]{36}".into(),
        r"sk-[a-zA-Z0-9]{48}".into(),
        r"xoxb-[0-9]+-[0-9]+-[a-zA-Z0-9]+".into(),
    ]
}
