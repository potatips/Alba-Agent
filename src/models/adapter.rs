use std::time::Duration;

use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};

use crate::domain::{ChatMessage, MessageRole, ToolCall};
use crate::models::types::{ModelConfig, ModelError, ModelRequest, ModelResponse, TokenUsage};

pub trait ModelClient: Send + Sync {
    fn complete(
        &self,
        request: &ModelRequest,
    ) -> impl std::future::Future<Output = Result<ModelResponse, ModelError>> + Send;
}

#[derive(Debug, Serialize)]
struct OpenAiChatRequest<'a> {
    model: &'a str,
    messages: Vec<OpenAiMessage>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<OpenAiTool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize)]
struct OpenAiMessage {
    role: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<OpenAiToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct OpenAiTool {
    #[serde(rename = "type")]
    tool_type: String,
    function: OpenAiFunctionDefinition,
}

#[derive(Debug, Serialize, Deserialize)]
struct OpenAiFunctionDefinition {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
struct OpenAiToolCall {
    id: String,
    #[serde(rename = "type")]
    tool_type: String,
    function: OpenAiFunctionCall,
}

#[derive(Debug, Serialize, Deserialize)]
struct OpenAiFunctionCall {
    name: String,
    arguments: String,
}

#[derive(Debug, Deserialize)]
struct OpenAiChatResponse {
    choices: Vec<OpenAiChoice>,
    #[serde(default)]
    usage: Option<OpenAiUsage>,
}

#[derive(Debug, Deserialize)]
struct OpenAiChoice {
    message: OpenAiResponseMessage,
}

#[derive(Debug, Deserialize)]
struct OpenAiResponseMessage {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<OpenAiToolCall>>,
}

#[derive(Debug, Deserialize)]
struct OpenAiUsage {
    #[serde(default)]
    prompt_tokens: u32,
    #[serde(default)]
    completion_tokens: u32,
    #[serde(default)]
    total_tokens: u32,
}

#[derive(Debug, Deserialize)]
struct OpenAiErrorWrapper {
    error: Option<OpenAiErrorDetails>,
}

#[derive(Debug, Deserialize)]
struct OpenAiErrorDetails {
    message: Option<String>,
}

pub struct OpenAiCompatibleClient {
    client: reqwest::Client,
    config: ModelConfig,
}

impl OpenAiCompatibleClient {
    pub fn new(config: ModelConfig) -> Result<Self, ModelError> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(config.timeout_seconds))
            .build()
            .map_err(|e| ModelError::NetworkError(e.to_string()))?;
        Ok(Self { client, config })
    }

    fn endpoint_url(&self) -> String {
        let base = self.config.api_base.trim_end_matches('/');
        if base.ends_with("/chat/completions") {
            base.to_string()
        } else if base.ends_with("/v1") {
            format!("{}/chat/completions", base)
        } else {
            format!("{}/v1/chat/completions", base)
        }
    }

    fn build_headers(&self) -> Result<HeaderMap, ModelError> {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        let auth_val = format!("Bearer {}", self.config.api_key);
        let header_val = HeaderValue::from_str(&auth_val).map_err(|_| {
            ModelError::AuthenticationError("Invalid authorization header format".to_string())
        })?;
        headers.insert(AUTHORIZATION, header_val);
        Ok(headers)
    }

    fn convert_message(msg: &ChatMessage) -> OpenAiMessage {
        match msg.role {
            MessageRole::System => OpenAiMessage {
                role: "system".to_string(),
                content: Some(msg.content.clone()),
                tool_calls: None,
                tool_call_id: None,
            },
            MessageRole::User => OpenAiMessage {
                role: "user".to_string(),
                content: Some(msg.content.clone()),
                tool_calls: None,
                tool_call_id: None,
            },
            MessageRole::Assistant => {
                let tool_calls = msg.tool_calls.as_ref().map(|calls| {
                    calls
                        .iter()
                        .map(|tc| OpenAiToolCall {
                            id: tc.id.clone(),
                            tool_type: "function".to_string(),
                            function: OpenAiFunctionCall {
                                name: tc.name.clone(),
                                arguments: tc.arguments.to_string(),
                            },
                        })
                        .collect()
                });
                OpenAiMessage {
                    role: "assistant".to_string(),
                    content: if msg.content.is_empty() && tool_calls.is_some() {
                        None
                    } else {
                        Some(msg.content.clone())
                    },
                    tool_calls,
                    tool_call_id: None,
                }
            }
            MessageRole::Tool => OpenAiMessage {
                role: "tool".to_string(),
                content: Some(msg.content.clone()),
                tool_calls: None,
                tool_call_id: msg.tool_call_id.clone(),
            },
        }
    }
}

impl ModelClient for OpenAiCompatibleClient {
    async fn complete(&self, request: &ModelRequest) -> Result<ModelResponse, ModelError> {
        let url = self.endpoint_url();
        let headers = self.build_headers()?;

        let converted_messages = request.messages.iter().map(Self::convert_message).collect();

        let converted_tools = request
            .tools
            .iter()
            .map(|td| OpenAiTool {
                tool_type: "function".to_string(),
                function: OpenAiFunctionDefinition {
                    name: td.name.clone(),
                    description: td.description.clone(),
                    parameters: td.parameters_schema.clone(),
                },
            })
            .collect();

        let model = request
            .model_override
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or(&self.config.model);

        let body = OpenAiChatRequest {
            model,
            messages: converted_messages,
            tools: converted_tools,
            temperature: request.temperature,
            max_tokens: request.max_tokens,
        };

        let response = self
            .client
            .post(&url)
            .headers(headers)
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    ModelError::Timeout(e.to_string())
                } else {
                    ModelError::NetworkError(e.to_string())
                }
            })?;

        let status = response.status();
        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            return Err(ModelError::AuthenticationError(format!(
                "HTTP {}",
                status.as_u16()
            )));
        }

        if !status.is_success() {
            let status_code = status.as_u16();
            let text = response.text().await.unwrap_or_default();
            let parsed_msg = serde_json::from_str::<OpenAiErrorWrapper>(&text)
                .ok()
                .and_then(|w| w.error)
                .and_then(|d| d.message)
                .unwrap_or_else(|| text.clone());
            return Err(ModelError::ProviderError {
                status_code,
                message: parsed_msg,
            });
        }

        let payload_text = response
            .text()
            .await
            .map_err(|e| ModelError::NetworkError(e.to_string()))?;

        let parsed: OpenAiChatResponse = serde_json::from_str(&payload_text)
            .map_err(|e| ModelError::InvalidResponse(format!("JSON parse error: {}", e)))?;

        let choice = parsed.choices.into_iter().next().ok_or_else(|| {
            ModelError::InvalidResponse("No choices returned from model".to_string())
        })?;

        let mut tool_calls = Vec::new();
        if let Some(calls) = choice.message.tool_calls {
            for call in calls {
                let parsed_args =
                    serde_json::from_str::<serde_json::Value>(&call.function.arguments)
                        .unwrap_or(serde_json::Value::String(call.function.arguments));
                tool_calls.push(ToolCall {
                    id: call.id,
                    name: call.function.name,
                    arguments: parsed_args,
                });
            }
        }

        let usage = parsed.usage.map(|u| TokenUsage {
            prompt_tokens: u.prompt_tokens,
            completion_tokens: u.completion_tokens,
            total_tokens: u.total_tokens,
        });

        Ok(ModelResponse {
            content: choice.message.content,
            tool_calls,
            usage,
        })
    }
}
