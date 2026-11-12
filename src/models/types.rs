use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::domain::{ChatMessage, ToolCall};
use crate::tools::types::ToolDefinition;

#[derive(Error, Debug)]
pub enum ModelError {
    #[error("Network connection error: {0}")]
    NetworkError(String),
    #[error("Model provider error (HTTP {status_code}): {message}")]
    ProviderError { status_code: u16, message: String },
    #[error("Authentication failed: {0}")]
    AuthenticationError(String),
    #[error("Invalid model response: {0}")]
    InvalidResponse(String),
    #[error("Request timed out: {0}")]
    Timeout(String),
}

#[derive(Clone)]
pub struct ModelConfig {
    pub api_base: String,
    pub api_key: String,
    pub model: String,
    pub timeout_seconds: u64,
}

impl std::fmt::Debug for ModelConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ModelConfig")
            .field("api_base", &self.api_base)
            .field("api_key", &"[REDACTED]")
            .field("model", &self.model)
            .field("timeout_seconds", &self.timeout_seconds)
            .finish()
    }
}

impl ModelConfig {
    pub fn new(
        api_base: impl Into<String>,
        api_key: impl Into<String>,
        model: impl Into<String>,
    ) -> Self {
        Self {
            api_base: api_base.into(),
            api_key: api_key.into(),
            model: model.into(),
            timeout_seconds: 30,
        }
    }

    pub fn with_timeout(mut self, seconds: u64) -> Self {
        self.timeout_seconds = seconds;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

#[derive(Debug, Clone)]
pub struct ModelRequest {
    pub messages: Vec<ChatMessage>,
    pub tools: Vec<ToolDefinition>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub model_override: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelResponse {
    pub content: Option<String>,
    pub tool_calls: Vec<ToolCall>,
    pub usage: Option<TokenUsage>,
}
