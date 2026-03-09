use std::time::Duration;

#[derive(Clone)]
pub struct OllamaClient {
    base_url: String,
    model: String,
    client: reqwest::Client,
}

#[derive(serde::Deserialize)]
struct GenerateResponse {
    response: String,
}

impl OllamaClient {
    pub fn new(base_url: &str, model: &str, timeout_secs: u64) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(timeout_secs))
            .build()
            .unwrap_or_default();

        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            model: model.to_string(),
            client,
        }
    }

    pub async fn health_check(&self) -> bool {
        self.client
            .get(format!("{}/api/tags", self.base_url))
            .timeout(Duration::from_secs(3))
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }

    pub async fn generate(&self, prompt: &str, max_tokens: u32) -> anyhow::Result<String> {
        let body = serde_json::json!({
            "model": self.model,
            "prompt": prompt,
            "stream": false,
            "options": {
                "temperature": 0.1,
                "num_predict": max_tokens
            }
        });

        let response = self
            .client
            .post(format!("{}/api/generate", self.base_url))
            .json(&body)
            .send()
            .await?;

        if !response.status().is_success() {
            anyhow::bail!(
                "Ollama returned status {}: {}",
                response.status(),
                response.text().await.unwrap_or_default()
            );
        }

        let result: GenerateResponse = response.json().await?;
        Ok(result.response)
    }
}
