use std::time::Duration;

use reqwest::header::{HeaderMap, HeaderName, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
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
            .redirect(reqwest::redirect::Policy::none())
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

#[derive(Debug, Serialize)]
struct AnthropicRequest<'a> {
    model: &'a str,
    messages: Vec<AnthropicMessage>,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<AnthropicTool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
}

#[derive(Debug, Serialize, Deserialize)]
struct AnthropicMessage {
    role: String,
    content: Vec<AnthropicContentBlock>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
enum AnthropicContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        content: String,
    },
}

#[derive(Debug, Serialize, Deserialize)]
struct AnthropicTool {
    name: String,
    description: String,
    input_schema: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct AnthropicResponse {
    #[serde(default)]
    content: Vec<AnthropicResponseBlock>,
    #[serde(default)]
    usage: Option<AnthropicUsage>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum AnthropicResponseBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
}

#[derive(Debug, Deserialize)]
struct AnthropicUsage {
    #[serde(default)]
    input_tokens: u32,
    #[serde(default)]
    output_tokens: u32,
}

#[derive(Debug, Deserialize)]
struct AnthropicErrorWrapper {
    error: Option<AnthropicErrorDetails>,
}

#[derive(Debug, Deserialize)]
struct AnthropicErrorDetails {
    message: Option<String>,
}

pub struct AnthropicClient {
    client: reqwest::Client,
    config: ModelConfig,
}

impl AnthropicClient {
    pub fn new(config: ModelConfig) -> Result<Self, ModelError> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(config.timeout_seconds))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|e| ModelError::NetworkError(e.to_string()))?;
        Ok(Self { client, config })
    }

    fn endpoint_url(&self) -> String {
        let base = self.config.api_base.trim_end_matches('/');
        if base.ends_with("/messages") {
            base.to_string()
        } else if base.ends_with("/v1") {
            format!("{}/messages", base)
        } else {
            format!("{}/v1/messages", base)
        }
    }

    fn build_headers(&self) -> Result<HeaderMap, ModelError> {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(
            HeaderName::from_static("anthropic-version"),
            HeaderValue::from_static("2023-06-01"),
        );
        let key_val = HeaderValue::from_str(&self.config.api_key).map_err(|_| {
            ModelError::AuthenticationError("Invalid authorization header format".to_string())
        })?;
        headers.insert(HeaderName::from_static("x-api-key"), key_val);
        Ok(headers)
    }
}

impl ModelClient for AnthropicClient {
    async fn complete(&self, request: &ModelRequest) -> Result<ModelResponse, ModelError> {
        let url = self.endpoint_url();
        let headers = self.build_headers()?;

        let mut system_prompts: Vec<String> = Vec::new();
        let mut messages: Vec<AnthropicMessage> = Vec::new();

        for msg in &request.messages {
            match msg.role {
                MessageRole::System => {
                    system_prompts.push(msg.content.clone());
                }
                MessageRole::User => {
                    messages.push(AnthropicMessage {
                        role: "user".to_string(),
                        content: vec![AnthropicContentBlock::Text {
                            text: msg.content.clone(),
                        }],
                    });
                }
                MessageRole::Assistant => {
                    let mut blocks = Vec::new();
                    if !msg.content.is_empty() {
                        blocks.push(AnthropicContentBlock::Text {
                            text: msg.content.clone(),
                        });
                    }
                    if let Some(tool_calls) = &msg.tool_calls {
                        for tc in tool_calls {
                            blocks.push(AnthropicContentBlock::ToolUse {
                                id: tc.id.clone(),
                                name: tc.name.clone(),
                                input: tc.arguments.clone(),
                            });
                        }
                    }
                    messages.push(AnthropicMessage {
                        role: "assistant".to_string(),
                        content: blocks,
                    });
                }
                MessageRole::Tool => {
                    messages.push(AnthropicMessage {
                        role: "user".to_string(),
                        content: vec![AnthropicContentBlock::ToolResult {
                            tool_use_id: msg.tool_call_id.clone().unwrap_or_default(),
                            content: msg.content.clone(),
                        }],
                    });
                }
            }
        }

        let tools: Vec<AnthropicTool> = request
            .tools
            .iter()
            .map(|t| AnthropicTool {
                name: t.name.clone(),
                description: t.description.clone(),
                input_schema: t.parameters_schema.clone(),
            })
            .collect();

        let model = request
            .model_override
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or(&self.config.model);

        let system = if system_prompts.is_empty() {
            None
        } else {
            Some(system_prompts.join("\n\n"))
        };

        let body = AnthropicRequest {
            model,
            messages,
            max_tokens: request.max_tokens.unwrap_or(4096),
            system,
            tools,
            temperature: request.temperature,
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
            let parsed_msg = serde_json::from_str::<AnthropicErrorWrapper>(&text)
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

        let parsed: AnthropicResponse = serde_json::from_str(&payload_text)
            .map_err(|e| ModelError::InvalidResponse(format!("JSON parse error: {}", e)))?;

        let mut text_parts: Vec<String> = Vec::new();
        let mut tool_calls: Vec<ToolCall> = Vec::new();

        for block in parsed.content {
            match block {
                AnthropicResponseBlock::Text { text } => {
                    text_parts.push(text);
                }
                AnthropicResponseBlock::ToolUse { id, name, input } => {
                    tool_calls.push(ToolCall {
                        id,
                        name,
                        arguments: input,
                    });
                }
            }
        }

        let content = if text_parts.is_empty() {
            None
        } else {
            Some(text_parts.join(""))
        };

        let usage = parsed.usage.map(|u| TokenUsage {
            prompt_tokens: u.input_tokens,
            completion_tokens: u.output_tokens,
            total_tokens: u.input_tokens.saturating_add(u.output_tokens),
        });

        Ok(ModelResponse {
            content,
            tool_calls,
            usage,
        })
    }
}

#[derive(Debug, Serialize)]
struct GeminiGenerateContentRequest<'a> {
    #[serde(skip_serializing_if = "Option::is_none", rename = "systemInstruction")]
    system_instruction: Option<GeminiContent>,
    contents: Vec<GeminiContent>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<GeminiTool>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "generationConfig")]
    generation_config: Option<GeminiGenerationConfig>,
    #[serde(skip_serializing)]
    _phantom: std::marker::PhantomData<&'a ()>,
}

#[derive(Debug, Serialize, Deserialize)]
struct GeminiContent {
    #[serde(skip_serializing_if = "Option::is_none")]
    role: Option<String>,
    parts: Vec<GeminiPart>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum GeminiPart {
    Text {
        text: String,
    },
    FunctionCall {
        #[serde(rename = "functionCall")]
        function_call: GeminiFunctionCall,
    },
    FunctionResponse {
        #[serde(rename = "functionResponse")]
        function_response: GeminiFunctionResponse,
    },
}

#[derive(Debug, Serialize, Deserialize)]
struct GeminiFunctionCall {
    name: String,
    args: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
struct GeminiFunctionResponse {
    name: String,
    response: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
struct GeminiTool {
    #[serde(rename = "functionDeclarations")]
    function_declarations: Vec<GeminiFunctionDeclaration>,
}

#[derive(Debug, Serialize, Deserialize)]
struct GeminiFunctionDeclaration {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
struct GeminiGenerationConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "maxOutputTokens")]
    max_output_tokens: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct GeminiGenerateContentResponse {
    #[serde(default)]
    candidates: Vec<GeminiCandidate>,
    #[serde(default, rename = "usageMetadata")]
    usage_metadata: Option<GeminiUsageMetadata>,
}

#[derive(Debug, Deserialize)]
struct GeminiCandidate {
    content: Option<GeminiCandidateContent>,
}

#[derive(Debug, Deserialize)]
struct GeminiCandidateContent {
    #[serde(default)]
    parts: Vec<GeminiResponsePart>,
}

#[derive(Debug, Deserialize)]
struct GeminiResponsePart {
    #[serde(default)]
    text: Option<String>,
    #[serde(default, rename = "functionCall")]
    function_call: Option<GeminiFunctionCall>,
}

#[derive(Debug, Deserialize)]
struct GeminiUsageMetadata {
    #[serde(default, rename = "promptTokenCount")]
    prompt_token_count: u32,
    #[serde(default, rename = "candidatesTokenCount")]
    candidates_token_count: u32,
    #[serde(default, rename = "totalTokenCount")]
    total_token_count: u32,
}

#[derive(Debug, Deserialize)]
struct GeminiErrorWrapper {
    error: Option<GeminiErrorDetails>,
}

#[derive(Debug, Deserialize)]
struct GeminiErrorDetails {
    message: Option<String>,
    status: Option<String>,
}

pub struct GeminiClient {
    client: reqwest::Client,
    config: ModelConfig,
}

impl GeminiClient {
    pub fn new(config: ModelConfig) -> Result<Self, ModelError> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(config.timeout_seconds))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|e| ModelError::NetworkError(e.to_string()))?;
        Ok(Self { client, config })
    }

    fn endpoint_url(&self, model: &str) -> String {
        let base = self.config.api_base.trim_end_matches('/');
        if base.contains(":generateContent") {
            base.to_string()
        } else if base.ends_with("/v1beta") {
            format!("{}/models/{}:generateContent", base, model)
        } else {
            format!("{}/v1beta/models/{}:generateContent", base, model)
        }
    }

    fn build_headers(&self) -> Result<HeaderMap, ModelError> {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        let key_val = HeaderValue::from_str(&self.config.api_key)
            .map_err(|_| ModelError::AuthenticationError("Invalid API key format".to_string()))?;
        headers.insert(HeaderName::from_static("x-goog-api-key"), key_val);
        Ok(headers)
    }
}

impl ModelClient for GeminiClient {
    async fn complete(&self, request: &ModelRequest) -> Result<ModelResponse, ModelError> {
        let model = request
            .model_override
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or(&self.config.model);

        let url = self.endpoint_url(model);
        let headers = self.build_headers()?;

        let mut system_prompts: Vec<String> = Vec::new();
        let mut contents: Vec<GeminiContent> = Vec::new();

        for msg in &request.messages {
            match msg.role {
                MessageRole::System => {
                    system_prompts.push(msg.content.clone());
                }
                MessageRole::User => {
                    contents.push(GeminiContent {
                        role: Some("user".to_string()),
                        parts: vec![GeminiPart::Text {
                            text: msg.content.clone(),
                        }],
                    });
                }
                MessageRole::Assistant => {
                    let mut parts = Vec::new();
                    if !msg.content.is_empty() {
                        parts.push(GeminiPart::Text {
                            text: msg.content.clone(),
                        });
                    }
                    if let Some(tool_calls) = &msg.tool_calls {
                        for tc in tool_calls {
                            parts.push(GeminiPart::FunctionCall {
                                function_call: GeminiFunctionCall {
                                    name: tc.name.clone(),
                                    args: tc.arguments.clone(),
                                },
                            });
                        }
                    }
                    contents.push(GeminiContent {
                        role: Some("model".to_string()),
                        parts,
                    });
                }
                MessageRole::Tool => {
                    contents.push(GeminiContent {
                        role: Some("user".to_string()),
                        parts: vec![GeminiPart::FunctionResponse {
                            function_response: GeminiFunctionResponse {
                                name: msg
                                    .tool_call_id
                                    .clone()
                                    .unwrap_or_else(|| "tool".to_string()),
                                response: serde_json::json!({ "result": msg.content }),
                            },
                        }],
                    });
                }
            }
        }

        let system_instruction = if system_prompts.is_empty() {
            None
        } else {
            Some(GeminiContent {
                role: None,
                parts: vec![GeminiPart::Text {
                    text: system_prompts.join("\n\n"),
                }],
            })
        };

        let tools = if request.tools.is_empty() {
            Vec::new()
        } else {
            vec![GeminiTool {
                function_declarations: request
                    .tools
                    .iter()
                    .map(|t| GeminiFunctionDeclaration {
                        name: t.name.clone(),
                        description: t.description.clone(),
                        parameters: t.parameters_schema.clone(),
                    })
                    .collect(),
            }]
        };

        let generation_config = if request.temperature.is_some() || request.max_tokens.is_some() {
            Some(GeminiGenerationConfig {
                temperature: request.temperature,
                max_output_tokens: request.max_tokens,
            })
        } else {
            None
        };

        let body = GeminiGenerateContentRequest {
            system_instruction,
            contents,
            tools,
            generation_config,
            _phantom: std::marker::PhantomData,
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
            let parsed = serde_json::from_str::<GeminiErrorWrapper>(&text).ok();
            let error_details = parsed.and_then(|p| p.error);
            if let Some(err) = &error_details {
                if err.status.as_deref() == Some("INVALID_ARGUMENT")
                    && err
                        .message
                        .as_deref()
                        .unwrap_or("")
                        .contains("API_KEY_INVALID")
                {
                    return Err(ModelError::AuthenticationError(
                        err.message
                            .clone()
                            .unwrap_or_else(|| "API_KEY_INVALID".to_string()),
                    ));
                }
            }
            let msg = error_details
                .and_then(|e| e.message)
                .unwrap_or_else(|| text.clone());
            return Err(ModelError::ProviderError {
                status_code,
                message: msg,
            });
        }

        let payload_text = response
            .text()
            .await
            .map_err(|e| ModelError::NetworkError(e.to_string()))?;

        let parsed: GeminiGenerateContentResponse = serde_json::from_str(&payload_text)
            .map_err(|e| ModelError::InvalidResponse(format!("JSON parse error: {}", e)))?;

        let mut text_parts: Vec<String> = Vec::new();
        let mut tool_calls: Vec<ToolCall> = Vec::new();

        for candidate in parsed.candidates {
            if let Some(content) = candidate.content {
                for part in content.parts {
                    if let Some(t) = part.text {
                        text_parts.push(t);
                    }
                    if let Some(fc) = part.function_call {
                        tool_calls.push(ToolCall {
                            id: uuid::Uuid::new_v4().to_string(),
                            name: fc.name,
                            arguments: fc.args,
                        });
                    }
                }
            }
        }

        let content = if text_parts.is_empty() {
            None
        } else {
            Some(text_parts.join(""))
        };

        let usage = parsed.usage_metadata.map(|u| TokenUsage {
            prompt_tokens: u.prompt_token_count,
            completion_tokens: u.candidates_token_count,
            total_tokens: u.total_token_count,
        });

        Ok(ModelResponse {
            content,
            tool_calls,
            usage,
        })
    }
}
