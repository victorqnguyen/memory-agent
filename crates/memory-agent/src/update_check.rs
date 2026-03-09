use std::time::Duration;

const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const LATEST_URL: &str =
    "https://raw.githubusercontent.com/victorqnguyen/memory-agent/main/LATEST.json";

pub struct UpdateInfo {
    pub version: String,
    pub message: Option<String>,
}

impl UpdateInfo {
    /// One-line summary for help bars and inline display.
    pub fn short(&self) -> String {
        format!(
            "Update available: {} → {}  (run: cargo install memory-agent)",
            CURRENT_VERSION, self.version
        )
    }

    /// Full notice including optional release message.
    pub fn full(&self) -> String {
        let mut s = self.short();
        if let Some(msg) = &self.message {
            s.push('\n');
            s.push_str(msg);
        }
        s
    }
}

/// Fetch LATEST.json from GitHub. Returns `Some(UpdateInfo)` if a newer version is available.
/// Silently returns `None` on any error (network, parse, timeout).
pub async fn check_for_update() -> Option<UpdateInfo> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(3))
        .build()
        .ok()?;

    let resp = client
        .get(LATEST_URL)
        .header("User-Agent", format!("memory-agent/{CURRENT_VERSION}"))
        .send()
        .await
        .ok()?;

    let json: serde_json::Value = resp.json().await.ok()?;
    let latest = json.get("version")?.as_str()?.to_string();
    let message = json
        .get("message")
        .and_then(|v| v.as_str())
        .map(str::to_string);

    if is_newer(&latest, CURRENT_VERSION) {
        Some(UpdateInfo { version: latest, message })
    } else {
        None
    }
}

fn is_newer(latest: &str, current: &str) -> bool {
    let parse = |v: &str| -> Option<(u32, u32, u32)> {
        let mut parts = v.split('.');
        Some((
            parts.next()?.parse().ok()?,
            parts.next()?.parse().ok()?,
            parts.next()?.parse().ok()?,
        ))
    };
    match (parse(latest), parse(current)) {
        (Some(l), Some(c)) => l > c,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newer_patch() { assert!(is_newer("0.1.4", "0.1.3")); }
    #[test]
    fn newer_minor() { assert!(is_newer("0.2.0", "0.1.9")); }
    #[test]
    fn newer_major() { assert!(is_newer("1.0.0", "0.9.9")); }
    #[test]
    fn same_version() { assert!(!is_newer("0.1.3", "0.1.3")); }
    #[test]
    fn older_version() { assert!(!is_newer("0.1.2", "0.1.3")); }
}
