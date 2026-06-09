// ┌──────────────────────────────────────────────────────────┐
// │  MORPHIC LLM MODULE (v0.3)                               │
// │  LLM-powered candidate generation via Ollama              │
// │  Supported: ollama, openai, anthropic (feature flags)     │
// └──────────────────────────────────────────────────────────┘

pub mod client;
pub mod prompt;
pub mod parser;

pub use client::{LlmClient, LlmConfig, LlmProvider, LlmResponse, CodeCandidate};
pub use prompt::build_synthesis_prompt;
pub use parser::parse_llm_response;
