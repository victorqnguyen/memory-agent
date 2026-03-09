use std::sync::{Mutex, OnceLock};

use regex_lite::Regex;

use crate::config::PrivacyConfig;

static PRIVATE_TAG_RE: OnceLock<Regex> = OnceLock::new();
static SECRET_PATTERNS_CACHE: OnceLock<Mutex<CachedPatterns>> = OnceLock::new();

struct CachedPatterns {
    source: Vec<String>,
    compiled: Vec<Regex>,
}

pub fn strip_private_tags(content: &str) -> String {
    let re = PRIVATE_TAG_RE.get_or_init(|| Regex::new(r"(?si)<private>.*?</private>").unwrap());
    re.replace_all(content, "[REDACTED]").to_string()
}

pub fn strip_secrets(content: &str, config: &PrivacyConfig) -> String {
    let compiled = get_or_compile_patterns(config);
    let mut result = content.to_string();
    for re in &compiled {
        result = re.replace_all(&result, "[SECRET_REDACTED]").to_string();
    }
    result
}

fn get_or_compile_patterns(config: &PrivacyConfig) -> Vec<Regex> {
    let current = effective_patterns(config);
    let cache = SECRET_PATTERNS_CACHE.get_or_init(|| {
        Mutex::new(CachedPatterns {
            source: Vec::new(),
            compiled: Vec::new(),
        })
    });
    let mut guard = cache.lock().unwrap_or_else(|e| e.into_inner());
    if guard.source == current {
        return guard.compiled.clone();
    }
    let compiled: Vec<Regex> = current
        .iter()
        .filter_map(|p| match Regex::new(p) {
            Ok(r) => Some(r),
            Err(e) => {
                eprintln!("[memory-core] invalid privacy pattern '{}': {}", p, e);
                None
            }
        })
        .collect();
    guard.source = current;
    guard.compiled = compiled.clone();
    compiled
}

fn effective_patterns(config: &PrivacyConfig) -> Vec<String> {
    if config.replace_defaults {
        config.extra_patterns.clone()
    } else {
        let mut patterns = config.secret_patterns.clone();
        patterns.extend(config.extra_patterns.clone());
        patterns
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::PrivacyConfig;

    #[test]
    fn strips_private_tags() {
        let input = "public info <private>secret stuff</private> more public";
        assert_eq!(
            strip_private_tags(input),
            "public info [REDACTED] more public"
        );
    }

    #[test]
    fn strips_multiline_private_tags() {
        let input = "before <private>\nline1\nline2\n</private> after";
        assert_eq!(strip_private_tags(input), "before [REDACTED] after");
    }

    #[test]
    fn detects_aws_key() {
        let config = PrivacyConfig::default();
        let input = "my key is AKIAIOSFODNN7EXAMPLE";
        assert!(strip_secrets(input, &config).contains("[SECRET_REDACTED]"));
    }

    #[test]
    fn detects_private_key_block() {
        let config = PrivacyConfig::default();
        let input = "-----BEGIN RSA PRIVATE KEY-----\nMIIEpAIB...";
        assert!(strip_secrets(input, &config).contains("[SECRET_REDACTED]"));
    }

    #[test]
    fn detects_api_key_assignment() {
        let config = PrivacyConfig::default();
        let input = "config: api_key=abc123xyz";
        assert!(strip_secrets(input, &config).contains("[SECRET_REDACTED]"));
    }

    #[test]
    fn detects_connection_strings() {
        let config = PrivacyConfig::default();
        for input in [
            "mongodb://user:pass@host:27017/db",
            "postgres://user:pass@host/db",
            "mysql://user:pass@host/db",
            "redis://user:pass@host:6379",
        ] {
            assert!(
                strip_secrets(input, &config).contains("[SECRET_REDACTED]"),
                "Failed to detect: {input}"
            );
        }
    }

    #[test]
    fn detects_github_token() {
        let config = PrivacyConfig::default();
        let input = "token: ghp_ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghij";
        assert!(strip_secrets(input, &config).contains("[SECRET_REDACTED]"));
    }

    #[test]
    fn extra_patterns_additive() {
        let mut config = PrivacyConfig::default();
        config.extra_patterns.push(r"CUSTOM_[0-9]{6}".into());
        let input = "custom: CUSTOM_123456";
        assert!(strip_secrets(input, &config).contains("[SECRET_REDACTED]"));
    }

    #[test]
    fn replace_defaults_removes_builtins() {
        let mut config = PrivacyConfig::default();
        config.replace_defaults = true;
        config.extra_patterns.push(r"ONLY_THIS".into());
        let input = "AKIAIOSFODNN7EXAMPLE should not be caught";
        let result = strip_secrets(input, &config);
        assert!(!result.contains("[SECRET_REDACTED]"));

        let input2 = "ONLY_THIS should be caught";
        assert!(strip_secrets(input2, &config).contains("[SECRET_REDACTED]"));
    }
}
