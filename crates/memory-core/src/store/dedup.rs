pub fn normalize_content(content: &str) -> String {
    content
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn compute_hash(key: &str, scope: &str, value: &str) -> String {
    let normalized = normalize_content(value);
    let input = format!("{key}:{scope}:{normalized}");
    blake3::hash(input.as_bytes()).to_hex().to_string()
}
