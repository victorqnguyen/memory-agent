use std::path::{Path, PathBuf};

use memory_core::Config;

use crate::llm::LlmConfig;

#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(default)]
pub struct AgentConfig {
    #[serde(flatten)]
    pub core: Config,
    pub llm: LlmConfig,
    pub hooks: HooksConfig,
}

/// Configuration for hook context injection.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct HooksConfig {
    /// Static text always injected verbatim into Claude context at every hook event.
    /// No LLM required. Editable in the TUI Hook Config tab.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub injection_prompt: Option<String>,
    /// When true, agent-review-gate.sh prompts to dispatch the reviewer after each Agent tool call.
    /// Defaults to false (opt-in).
    #[serde(default)]
    pub agent_review_gate: bool,
}

pub fn default_data_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("MEMORY_AGENT_DATA_DIR") {
        return PathBuf::from(dir);
    }
    if let Some(home) = dirs::home_dir() {
        return home.join(".memory-agent");
    }
    // Home directory is unavailable — this is a misconfigured environment.
    // Warn clearly rather than silently using CWD. Callers should use
    // MEMORY_AGENT_DATA_DIR to override when home is not available.
    eprintln!(
        "warning: cannot determine home directory; set MEMORY_AGENT_DATA_DIR \
         to specify the data directory explicitly"
    );
    PathBuf::from(".memory-agent")
}

pub fn load(data_dir: &Path) -> anyhow::Result<AgentConfig> {
    let config_path = if let Ok(path) = std::env::var("MEMORY_AGENT_CONFIG") {
        PathBuf::from(path)
    } else {
        data_dir.join("config.toml")
    };

    let mut config: AgentConfig = if config_path.exists() {
        let content = std::fs::read_to_string(&config_path)?;
        toml::from_str(&content)?
    } else {
        AgentConfig::default()
    };

    apply_env_overrides(&mut config.core);
    apply_llm_env_overrides(&mut config.llm);
    Ok(config)
}

pub fn apply_encryption_env_override(config: &mut Config) {
    if let Ok(val) = std::env::var("MEMORY_AGENT_STORAGE_ENCRYPTION_ENABLED") {
        config.storage.encryption_enabled = val == "true" || val == "1";
    }
}

/// Retrieve the database passphrase from available sources.
/// Priority: env var > system keychain.
pub fn retrieve_passphrase() -> anyhow::Result<Option<String>> {
    // 1. Environment variable (highest priority)
    if let Ok(val) = std::env::var("MEMORY_AGENT_PASSPHRASE") {
        if !val.is_empty() {
            return Ok(Some(val));
        }
    }

    // 2. System keychain
    keychain_get_passphrase()
}

/// Returns the source of the passphrase ("env", "keychain", or None).
pub fn passphrase_source() -> Option<&'static str> {
    if std::env::var("MEMORY_AGENT_PASSPHRASE").is_ok_and(|v| !v.is_empty()) {
        return Some("env");
    }
    if keychain_get_passphrase().is_ok_and(|v| v.is_some()) {
        return Some("keychain");
    }
    None
}

/// Store a passphrase in the system keychain.
pub fn store_passphrase(passphrase: &str) -> anyhow::Result<()> {
    keychain_set_passphrase(passphrase)
}

/// Delete the passphrase from the system keychain.
pub fn delete_passphrase() -> anyhow::Result<()> {
    keychain_delete_passphrase()
}

/// Generate a random passphrase (32 bytes of entropy, hex-encoded).
/// Uses OS random source + timestamp + PID through blake3.
pub fn generate_passphrase() -> String {
    use blake3::Hasher;
    let mut hasher = Hasher::new();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    hasher.update(&now.as_nanos().to_le_bytes());
    hasher.update(&std::process::id().to_le_bytes());
    let random_bytes = os_random_bytes();
    hasher.update(&random_bytes);
    let hash = hasher.finalize();
    hash.to_hex().to_string()
}

#[cfg(unix)]
fn os_random_bytes() -> [u8; 32] {
    use std::io::Read;
    let mut buf = [0u8; 32];
    if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
        let _ = f.read_exact(&mut buf);
    }
    buf
}

#[cfg(windows)]
fn os_random_bytes() -> [u8; 32] {
    let mut buf = [0u8; 32];
    getrandom::getrandom(&mut buf).ok();
    buf
}

#[cfg(target_os = "macos")]
fn keychain_get_passphrase() -> anyhow::Result<Option<String>> {
    let output = std::process::Command::new("security")
        .args(["find-generic-password", "-s", "memory-agent", "-a", "encryption", "-w"])
        .output();
    match output {
        Ok(o) if o.status.success() => {
            let pw = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if pw.is_empty() { Ok(None) } else { Ok(Some(pw)) }
        }
        _ => Ok(None),
    }
}

#[cfg(target_os = "macos")]
fn keychain_set_passphrase(passphrase: &str) -> anyhow::Result<()> {
    let status = std::process::Command::new("security")
        .args([
            "add-generic-password",
            "-s", "memory-agent",
            "-a", "encryption",
            "-w", passphrase,
            "-U", // update if exists
        ])
        .status()?;
    if !status.success() {
        anyhow::bail!("failed to store passphrase in keychain (exit {})", status);
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn keychain_delete_passphrase() -> anyhow::Result<()> {
    let status = std::process::Command::new("security")
        .args(["delete-generic-password", "-s", "memory-agent", "-a", "encryption"])
        .status()?;
    if !status.success() {
        anyhow::bail!("failed to delete passphrase from keychain (exit {})", status);
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn keychain_get_passphrase() -> anyhow::Result<Option<String>> {
    let output = std::process::Command::new("secret-tool")
        .args(["lookup", "service", "memory-agent", "account", "encryption"])
        .output();
    match output {
        Ok(o) if o.status.success() => {
            let pw = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if pw.is_empty() { Ok(None) } else { Ok(Some(pw)) }
        }
        _ => Ok(None),
    }
}

#[cfg(target_os = "linux")]
fn keychain_set_passphrase(passphrase: &str) -> anyhow::Result<()> {
    use std::io::Write;
    let mut child = std::process::Command::new("secret-tool")
        .args(["store", "--label", "memory-agent", "service", "memory-agent", "account", "encryption"])
        .stdin(std::process::Stdio::piped())
        .spawn()?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(passphrase.as_bytes())?;
    }
    let status = child.wait()?;
    if !status.success() {
        anyhow::bail!("failed to store passphrase via secret-tool (exit {})", status);
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn keychain_delete_passphrase() -> anyhow::Result<()> {
    let status = std::process::Command::new("secret-tool")
        .args(["clear", "service", "memory-agent", "account", "encryption"])
        .status()?;
    if !status.success() {
        anyhow::bail!("failed to delete passphrase via secret-tool (exit {})", status);
    }
    Ok(())
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn keychain_get_passphrase() -> anyhow::Result<Option<String>> {
    Ok(None) // No keychain support; use MEMORY_AGENT_PASSPHRASE env var
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn keychain_set_passphrase(_passphrase: &str) -> anyhow::Result<()> {
    anyhow::bail!("keychain storage not supported on this platform — use MEMORY_AGENT_PASSPHRASE env var")
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn keychain_delete_passphrase() -> anyhow::Result<()> {
    Ok(()) // Nothing to delete
}

fn apply_env_overrides(config: &mut Config) {
    apply_encryption_env_override(config);
    if let Ok(val) = std::env::var("MEMORY_AGENT_SEARCH_DEFAULT_LIMIT") {
        if let Ok(v) = val.parse::<u32>() {
            config.search.default_limit = v.clamp(1, 10_000);
        }
    }
    if let Ok(val) = std::env::var("MEMORY_AGENT_SEARCH_MAX_LIMIT") {
        if let Ok(v) = val.parse::<u32>() {
            config.search.max_limit = v.clamp(1, 10_000);
        }
    }
    if let Ok(val) = std::env::var("MEMORY_AGENT_STORAGE_RETENTION_DAYS") {
        if let Ok(v) = val.parse::<u32>() {
            config.storage.retention_days = v.clamp(1, 36_500);
        }
    }
    if let Ok(val) = std::env::var("MEMORY_AGENT_VALIDATION_MAX_KEY_LENGTH") {
        if let Ok(v) = val.parse::<usize>() {
            config.validation.max_key_length = v.clamp(1, 4096);
        }
    }
    if let Ok(val) = std::env::var("MEMORY_AGENT_VALIDATION_MAX_VALUE_LENGTH") {
        if let Ok(v) = val.parse::<usize>() {
            config.validation.max_value_length = v.clamp(1, 100_000);
        }
    }
}

fn apply_llm_env_overrides(config: &mut LlmConfig) {
    if let Ok(val) = std::env::var("MEMORY_AGENT_LLM_OLLAMA_URL") {
        config.ollama_url = val;
    }
    if let Ok(val) = std::env::var("MEMORY_AGENT_LLM_OLLAMA_MODEL") {
        config.ollama_model = val;
    }
    if let Ok(val) = std::env::var("MEMORY_AGENT_LLM_TIMEOUT_SECS") {
        if let Ok(v) = val.parse() {
            config.timeout_secs = v;
        }
    }
}

pub fn create_default_config(data_dir: &Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(data_dir)?;
    let config_path = data_dir.join("config.toml");
    if config_path.exists() {
        anyhow::bail!("Config file already exists: {}", config_path.display());
    }
    std::fs::write(
        &config_path,
        DEFAULT_CONFIG_TEMPLATE,
    )?;
    Ok(())
}

const DEFAULT_CONFIG_TEMPLATE: &str = r#"# memory-agent configuration
# All values shown are defaults. Uncomment to override.

[storage]
# retention_days = 90
# vacuum_interval_secs = 604800
# max_db_size_mb = 500
# busy_timeout_ms = 5000
# cache_size_kb = 2048
# dedup_window_secs = 900
# encryption_enabled = false

[search]
# default_limit = 10
# max_limit = 50

[validation]
# max_key_length = 256
# max_value_length = 2000
# max_tags = 20
# max_tag_length = 64

[privacy]
# Secret patterns are regex. Matched content is replaced with [SECRET_REDACTED].
# These are ADDITIVE to built-in patterns. To replace built-ins, set replace_defaults = true.
# replace_defaults = false
# extra_patterns = []

[llm]
# Local LLM configuration (requires --features local-llm)
# ollama_url = "http://localhost:11434"
# ollama_model = "qwen3.5:2b"
# timeout_secs = 30

[hooks]
# Static text injected verbatim into Claude's context at every hook event. No LLM required.
# injection_prompt = "Dispatch Opus for planning. Haiku for discovery."
# agent_review_gate = true   # opt-in: prompt to dispatch reviewer after each Agent tool call
"#;

/// Serialize `hooks` into the `[hooks]` TOML section and merge it into the
/// existing config file, replacing any prior `[hooks]` section.
pub fn save_hooks_config(data_dir: &Path, hooks: &HooksConfig) -> anyhow::Result<()> {
    let config_path = data_dir.join("config.toml");

    // Serialize via a wrapper so toml produces the right section headers.
    #[derive(serde::Serialize)]
    struct Wrapper<'a> {
        hooks: &'a HooksConfig,
    }
    let new_section = toml::to_string_pretty(&Wrapper { hooks })?;

    if !config_path.exists() {
        std::fs::create_dir_all(data_dir)?;
        std::fs::write(&config_path, new_section)?;
        return Ok(());
    }

    let content = std::fs::read_to_string(&config_path)?;
    let lines: Vec<&str> = content.lines().collect();

    // Find the start of the [hooks] section.
    let hooks_start = lines.iter().position(|l| l.trim() == "[hooks]");

    let output = if let Some(start) = hooks_start {
        // Find where [hooks] ends: next top-level section that isn't [hooks.*]
        let end = lines[start + 1..]
            .iter()
            .position(|l| {
                let t = l.trim();
                t.starts_with('[') && !t.starts_with("[hooks]") && !t.starts_with("[hooks.")
            })
            .map(|i| start + 1 + i)
            .unwrap_or(lines.len());

        let before = lines[..start].join("\n").trim_end().to_string();
        let after = if end < lines.len() {
            format!("\n\n{}", lines[end..].join("\n"))
        } else {
            String::new()
        };
        format!("{}\n\n{}{}", before, new_section.trim_end(), after)
    } else {
        // No existing [hooks] section — append.
        format!("{}\n\n{}", content.trim_end(), new_section.trim_end())
    };

    let output = if output.ends_with('\n') { output } else { format!("{output}\n") };
    std::fs::write(&config_path, output)?;
    Ok(())
}
