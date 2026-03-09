#[cfg(feature = "local-llm")]
pub mod ollama;
pub mod prompts;

#[cfg(feature = "local-llm")]
use ollama::OllamaClient;

/// Max bytes for any single LLM output before truncation.
#[cfg(feature = "local-llm")]
const MAX_LLM_OUTPUT: usize = 2000;

/// Tiered LLM integration. Tier 1 (None) is always available.
/// Tier 2 (Local) requires Ollama running locally with a small model.
#[derive(Clone)]
pub enum LlmTier {
    None,
    #[cfg(feature = "local-llm")]
    Local(OllamaClient),
}

impl LlmTier {
    pub fn is_available(&self) -> bool {
        !matches!(self, LlmTier::None)
    }

    pub fn tier_name(&self) -> &'static str {
        match self {
            LlmTier::None => "tier-1 (no LLM)",
            #[cfg(feature = "local-llm")]
            LlmTier::Local(_) => "tier-2 (local Ollama)",
        }
    }
}

/// Auto-detect the best available LLM tier.
/// Only connects to localhost Ollama — remote URLs are rejected to prevent data exfiltration.
pub async fn detect_tier(config: &LlmConfig) -> LlmTier {
    #[cfg(feature = "local-llm")]
    {
        if !is_localhost_url(&config.ollama_url) {
            tracing::warn!(
                "Refusing non-localhost Ollama URL '{}' — only localhost is allowed for security",
                config.ollama_url
            );
            return LlmTier::None;
        }

        let client = OllamaClient::new(
            &config.ollama_url,
            &config.ollama_model,
            config.timeout_secs,
        );
        if client.health_check().await {
            tracing::info!(
                "LLM tier-2 available: Ollama at {} with model {}",
                config.ollama_url,
                config.ollama_model
            );
            return LlmTier::Local(client);
        }
        tracing::debug!("Ollama not available, falling back to tier-1");
    }

    let _ = config; // suppress unused warning when feature is off
    LlmTier::None
}

/// Reject any URL that isn't localhost — prevents memory content from being sent to remote servers.
#[allow(dead_code)]
fn is_localhost_url(url: &str) -> bool {
    let lower = url.to_lowercase();
    let after_scheme = lower
        .strip_prefix("http://")
        .or_else(|| lower.strip_prefix("https://"))
        .unwrap_or(&lower);
    // Strip userinfo (user:pass@) to prevent bypass via http://localhost:x@attacker.com
    let after_userinfo = after_scheme
        .split('/')
        .next()
        .unwrap_or("")
        .rsplit('@')
        .next()
        .unwrap_or("");
    // Handle bracketed IPv6 (e.g. [::1]:11434)
    let host = if after_userinfo.starts_with('[') {
        after_userinfo.split(']').next().unwrap_or("").trim_start_matches('[')
    } else {
        after_userinfo.split(':').next().unwrap_or("")
    };
    matches!(host, "localhost" | "127.0.0.1" | "::1")
}

/// Sanitize LLM output before it enters the storage layer.
/// Enforces max length and strips null bytes.
#[cfg(feature = "local-llm")]
fn sanitize_llm_output(output: &str, max_len: usize) -> String {
    let clean = output.replace('\0', "");
    let trimmed = clean.trim();
    if trimmed.len() > max_len {
        memory_core::safe_truncate(trimmed, max_len).to_string()
    } else {
        trimmed.to_string()
    }
}

/// Summarize session activities into 1-3 sentences.
#[allow(dead_code)]
pub async fn summarize_session(llm: &LlmTier, activities: &str) -> anyhow::Result<String> {
    match llm {
        LlmTier::None => Ok(prompts::extract_key_sentences(activities, 3)),
        #[cfg(feature = "local-llm")]
        LlmTier::Local(client) => {
            let prompt = prompts::session_summary_prompt(activities);
            match client.generate(&prompt, 150).await {
                Ok(response) => Ok(sanitize_llm_output(&response, MAX_LLM_OUTPUT)),
                Err(e) => {
                    tracing::warn!("LLM summarization failed, falling back to tier-1: {e}");
                    Ok(prompts::extract_key_sentences(activities, 3))
                }
            }
        }
    }
}

/// Compress multiple memory values into a single merged value.
pub async fn compress_memories(llm: &LlmTier, values: &[&str]) -> anyhow::Result<String> {
    match llm {
        LlmTier::None => Ok(memory_core::autonomous::consolidation::merge_values(values)),
        #[cfg(feature = "local-llm")]
        LlmTier::Local(client) => {
            let prompt = prompts::compression_prompt(values);
            match client.generate(&prompt, 300).await {
                Ok(response) => Ok(sanitize_llm_output(&response, MAX_LLM_OUTPUT)),
                Err(e) => {
                    tracing::warn!("LLM compression failed, falling back to tier-1: {e}");
                    Ok(memory_core::autonomous::consolidation::merge_values(values))
                }
            }
        }
    }
}

/// Enhance procedural memory extraction from skill outcomes.
pub async fn enhance_procedural(
    llm: &LlmTier,
    skill_name: &str,
    outcome: &str,
) -> anyhow::Result<Option<String>> {
    let _ = (skill_name, outcome); // used in local-llm feature branch
    match llm {
        LlmTier::None => Ok(None), // Tier 1 uses template extraction in skills/templates.rs
        #[cfg(feature = "local-llm")]
        LlmTier::Local(client) => {
            let prompt = prompts::procedural_extraction_prompt(skill_name, outcome);
            match client.generate(&prompt, 200).await {
                Ok(response) => {
                    let clean = sanitize_llm_output(&response, MAX_LLM_OUTPUT);
                    if clean.len() > 30 {
                        Ok(Some(clean))
                    } else {
                        Ok(None)
                    }
                }
                Err(e) => {
                    tracing::warn!("LLM procedural extraction failed: {e}");
                    Ok(None)
                }
            }
        }
    }
}

/// Extract search keywords from a user prompt.
pub async fn extract_keywords(llm: &LlmTier, prompt: &str) -> Vec<String> {
    match llm {
        LlmTier::None => prompts::extract_keywords(prompt, 5),
        #[cfg(feature = "local-llm")]
        LlmTier::Local(client) => {
            let llm_prompt = prompts::keyword_extraction_prompt(prompt);
            match client.generate(&llm_prompt, 50).await {
                Ok(response) => {
                    let clean = sanitize_llm_output(&response, 500);
                    let keywords: Vec<String> = clean
                        .split_whitespace()
                        .filter(|w| w.len() > 1)
                        .take(5)
                        .map(|w| w.to_string())
                        .collect();
                    if keywords.is_empty() {
                        prompts::extract_keywords(prompt, 5)
                    } else {
                        keywords
                    }
                }
                Err(e) => {
                    tracing::warn!("LLM keyword extraction failed: {e}");
                    prompts::extract_keywords(prompt, 5)
                }
            }
        }
    }
}

/// Extract learnings from a transcript excerpt.
pub async fn extract_learnings(llm: &LlmTier, transcript: &str) -> Vec<String> {
    match llm {
        LlmTier::None => {
            // Tier 1: extract key sentences as learnings
            let summary = prompts::extract_key_sentences(transcript, 5);
            if summary.is_empty() {
                Vec::new()
            } else {
                summary.split(". ").map(|s| s.to_string()).collect()
            }
        }
        #[cfg(feature = "local-llm")]
        LlmTier::Local(client) => {
            let prompt = prompts::learning_extraction_prompt(transcript);
            match client.generate(&prompt, 300).await {
                Ok(response) => {
                    let clean = sanitize_llm_output(&response, MAX_LLM_OUTPUT);
                    if clean == "NONE" || clean.is_empty() {
                        return Vec::new();
                    }
                    clean
                        .lines()
                        .map(|l| l.trim().trim_start_matches("- ").to_string())
                        .filter(|l| l.len() > 10)
                        .collect()
                }
                Err(e) => {
                    tracing::warn!("LLM learning extraction failed: {e}");
                    Vec::new()
                }
            }
        }
    }
}

/// Analyze metrics and produce actionable recommendations.
pub async fn analyze_metrics(llm: &LlmTier, metrics_summary: &str) -> anyhow::Result<Option<String>> {
    let _ = metrics_summary; // used in local-llm feature branch
    match llm {
        LlmTier::None => Ok(None),
        #[cfg(feature = "local-llm")]
        LlmTier::Local(client) => {
            let prompt = prompts::metrics_analysis_prompt(metrics_summary);
            match client.generate(&prompt, 300).await {
                Ok(response) => {
                    let clean = sanitize_llm_output(&response, MAX_LLM_OUTPUT);
                    if clean.len() > 20 {
                        Ok(Some(clean))
                    } else {
                        Ok(None)
                    }
                }
                Err(e) => {
                    tracing::warn!("LLM metrics analysis failed: {e}");
                    Ok(None)
                }
            }
        }
    }
}


/// LLM configuration.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct LlmConfig {
    pub ollama_url: String,
    pub ollama_model: String,
    pub timeout_secs: u64,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            ollama_url: "http://localhost:11434".into(),
            ollama_model: "qwen3.5:2b".into(),
            timeout_secs: 30,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- is_localhost_url (always available, no feature gate) ---

    #[test]
    fn localhost_urls_accepted() {
        assert!(is_localhost_url("http://localhost:11434"));
        assert!(is_localhost_url("http://127.0.0.1:11434"));
        assert!(is_localhost_url("http://localhost"));
        assert!(is_localhost_url("http://[::1]:11434"));
    }

    #[test]
    fn remote_urls_rejected() {
        assert!(!is_localhost_url("http://example.com:11434"));
        assert!(!is_localhost_url("https://ollama.myserver.com"));
        assert!(!is_localhost_url("http://192.168.1.100:11434"));
        assert!(!is_localhost_url("http://10.0.0.1:11434"));
        assert!(!is_localhost_url(""));
    }

    #[test]
    fn userinfo_bypass_rejected() {
        assert!(!is_localhost_url("http://localhost:x@attacker.com:11434"));
        assert!(!is_localhost_url("http://user:pass@evil.com:11434"));
        assert!(!is_localhost_url("http://127.0.0.1@evil.com"));
        assert!(is_localhost_url("http://user:pass@localhost:11434"));
        assert!(is_localhost_url("http://user@127.0.0.1:11434"));
    }

    // --- sanitize_llm_output (local-llm feature only) ---

    #[cfg(feature = "local-llm")]
    #[test]
    fn sanitize_strips_nulls_and_truncates() {
        let output = "hello\0world";
        assert_eq!(sanitize_llm_output(output, 100), "helloworld");

        let long = "a".repeat(3000);
        let result = sanitize_llm_output(&long, 2000);
        assert!(result.len() <= 2000);
    }

    #[cfg(feature = "local-llm")]
    #[test]
    fn sanitize_trims_whitespace() {
        assert_eq!(sanitize_llm_output("  hello  ", 100), "hello");
        assert_eq!(sanitize_llm_output("\n\nresult\n\n", 100), "result");
    }
}
