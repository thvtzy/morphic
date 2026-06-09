// ┌──────────────────────────────────────────────────────────┐
// │  MORPHIC LLM CLIENT                                      │
// │  Unified interface for Ollama, OpenAI, Anthropic          │
// └──────────────────────────────────────────────────────────┘

use serde::{Deserialize, Serialize};
use crate::spec::ast::FunctionSpec;

// ── Configuration ──────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct LlmConfig {
    pub provider: LlmProvider,
    pub model: String,
    pub temperature: f32,
    pub max_tokens: usize,
    pub timeout_secs: u64,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            provider: LlmProvider::Ollama {
                endpoint: "http://localhost:11434".into(),
            },
            model: "codellama:13b".into(),
            temperature: 0.7,
            max_tokens: 2048,
            timeout_secs: 60,
        }
    }
}

#[derive(Debug, Clone)]
pub enum LlmProvider {
    Ollama { endpoint: String },
    #[cfg(feature = "llm-remote")]
    OpenAI { api_key: String, endpoint: String },
    #[cfg(feature = "llm-remote")]
    Anthropic { api_key: String },
}

impl LlmProvider {
    pub fn ollama(endpoint: &str) -> Self {
        LlmProvider::Ollama { endpoint: endpoint.into() }
    }
}

// ── Response Types ─────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct OllamaResponse {
    pub model: String,
    pub response: String,
    #[serde(default)]
    pub done: bool,
    #[serde(rename = "total_duration")]
    pub total_duration: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct LlmResponse {
    pub model: String,
    pub content: String,
    pub duration_ms: u64,
    pub tokens_used: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeCandidate {
    pub source: String,
    pub language: String,
    pub confidence: f32,
    pub explanation: Option<String>,
}

// ── LLM Client ─────────────────────────────────────────────

pub struct LlmClient {
    config: LlmConfig,
    client: reqwest::blocking::Client,
}

impl LlmClient {
    pub fn new(config: LlmConfig) -> Result<Self, String> {
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(config.timeout_secs))
            .build()
            .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

        Ok(Self { config, client })
    }

    /// Send a prompt and get the raw completion response
    pub fn complete(&self, prompt: &str) -> Result<LlmResponse, String> {
        match &self.config.provider {
            LlmProvider::Ollama { endpoint } => {
                self.ollama_complete(endpoint, prompt)
            }
            #[cfg(feature = "llm-remote")]
            LlmProvider::OpenAI { api_key, endpoint } => {
                self.openai_complete(api_key, endpoint, prompt)
            }
            #[cfg(feature = "llm-remote")]
            LlmProvider::Anthropic { api_key } => {
                self.anthropic_complete(api_key, prompt)
            }
        }
    }

    /// Generate implementation candidates for a Morphic spec
    pub fn generate_candidates(
        &self,
        spec: &FunctionSpec,
        count: usize,
    ) -> Result<Vec<CodeCandidate>, String> {
        let prompt = crate::llm::prompt::build_synthesis_prompt(spec, count);
        let response = self.complete(&prompt)?;
        let candidates = crate::llm::parser::parse_llm_response(&response.content, spec)?;
        Ok(candidates)
    }

    // ── Ollama Backend ──────────────────────────────────────

    fn ollama_complete(&self, endpoint: &str, prompt: &str) -> Result<LlmResponse, String> {
        let body = serde_json::json!({
            "model": self.config.model,
            "prompt": prompt,
            "temperature": self.config.temperature,
            "max_tokens": self.config.max_tokens,
            "stream": false,
        });

        let resp = self.client
            .post(format!("{}/api/generate", endpoint))
            .json(&body)
            .send()
            .map_err(|e| format!("Ollama request failed: {}. Is Ollama running? Try: ollama serve", e))?;

        let ollama: OllamaResponse = resp
            .json()
            .map_err(|e| format!("Failed to parse Ollama response: {}", e))?;

        Ok(LlmResponse {
            model: ollama.model.clone(),
            content: ollama.response,
            duration_ms: ollama.total_duration.unwrap_or(0) / 1_000_000,
            tokens_used: 0, // Ollama doesn't always report token count
        })
    }

    // ── OpenAI Backend ──────────────────────────────────────

    #[cfg(feature = "llm-remote")]
    fn openai_complete(
        &self,
        api_key: &str,
        endpoint: &str,
        prompt: &str,
    ) -> Result<LlmResponse, String> {
        let body = serde_json::json!({
            "model": self.config.model,
            "messages": [
                {"role": "system", "content": "You are a code synthesis engine. Generate correct, idiomatic Rust code."},
                {"role": "user", "content": prompt}
            ],
            "temperature": self.config.temperature,
            "max_tokens": self.config.max_tokens,
        });

        let resp = self.client
            .post(format!("{}/v1/chat/completions", endpoint))
            .bearer_auth(api_key)
            .json(&body)
            .send()
            .map_err(|e| format!("OpenAI request failed: {}", e))?;

        let json: serde_json::Value = resp
            .json()
            .map_err(|e| format!("Failed to parse OpenAI response: {}", e))?;

        let content = json["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string();

        Ok(LlmResponse {
            model: self.config.model.clone(),
            content,
            duration_ms: 0,
            tokens_used: json["usage"]["total_tokens"].as_u64().unwrap_or(0) as usize,
        })
    }

    // ── Anthropic Backend ───────────────────────────────────

    #[cfg(feature = "llm-remote")]
    fn anthropic_complete(
        &self,
        api_key: &str,
        prompt: &str,
    ) -> Result<LlmResponse, String> {
        let body = serde_json::json!({
            "model": self.config.model,
            "max_tokens": self.config.max_tokens,
            "temperature": self.config.temperature,
            "messages": [
                {"role": "user", "content": prompt}
            ],
        });

        let resp = self.client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&body)
            .send()
            .map_err(|e| format!("Anthropic request failed: {}", e))?;

        let json: serde_json::Value = resp
            .json()
            .map_err(|e| format!("Failed to parse Anthropic response: {}", e))?;

        let content = json["content"][0]["text"]
            .as_str()
            .unwrap_or("")
            .to_string();

        Ok(LlmResponse {
            model: self.config.model.clone(),
            content,
            duration_ms: 0,
            tokens_used: 0,
        })
    }
}

// ── Tests ──────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ollama_config_defaults() {
        let config = LlmConfig::default();
        assert!(matches!(config.provider, LlmProvider::Ollama { .. }));
        assert_eq!(config.model, "codellama:13b");
    }

    #[test]
    fn test_client_connection_error() {
        // Ollama not running → should get a connection error
        let config = LlmConfig {
            provider: LlmProvider::Ollama { endpoint: "http://localhost:1".into() },
            ..Default::default()
        };
        let client = LlmClient::new(config).unwrap();
        let result = client.complete("fn hello()");
        assert!(result.is_err(), "Should fail — no Ollama on port 1");
    }
}
