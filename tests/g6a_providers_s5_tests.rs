use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::post;
use axum::{Json, Router};
use nexus_albaai::domain::{ChatMessage, MessageRole};
use nexus_albaai::models::{
    AnthropicClient, GeminiClient, ModelClient, ModelConfig, ModelError, ModelRequest,
    OpenAiCompatibleClient,
};
use nexus_albaai::tools::types::ToolDefinition;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::Mutex;

#[derive(Clone, Default)]
struct MockServerState {
    openai_mode: Arc<Mutex<String>>,
    anthropic_mode: Arc<Mutex<String>>,
    gemini_mode: Arc<Mutex<String>>,
    delay_ms: Arc<AtomicU64>,
    last_openai_headers: Arc<Mutex<Option<HeaderMap>>>,
    last_anthropic_headers: Arc<Mutex<Option<HeaderMap>>>,
    last_gemini_headers: Arc<Mutex<Option<HeaderMap>>>,
    last_openai_body: Arc<Mutex<Option<Value>>>,
    last_anthropic_body: Arc<Mutex<Option<Value>>>,
    last_gemini_body: Arc<Mutex<Option<Value>>>,
    last_gemini_action: Arc<Mutex<Option<String>>>,
}

async fn openai_handler(
    State(state): State<MockServerState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    *state.last_openai_headers.lock().await = Some(headers.clone());
    *state.last_openai_body.lock().await = Some(body.clone());

    let delay = state.delay_ms.load(Ordering::Relaxed);
    if delay > 0 {
        tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;
    }

    let auth = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    if !auth.starts_with("Bearer ") || auth.contains("bad-key") {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({
                "error": {
                    "message": "Incorrect API key provided",
                    "type": "invalid_request_error"
                }
            })),
        );
    }

    let mode = state.openai_mode.lock().await.clone();
    match mode.as_str() {
        "rate_limit" => (
            StatusCode::TOO_MANY_REQUESTS,
            Json(json!({
                "error": {
                    "message": "Rate limit reached",
                    "type": "tokens"
                }
            })),
        ),
        "server_error" => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "error": {
                    "message": "Internal server error from provider",
                    "type": "api_error"
                }
            })),
        ),
        "tool_call" => (
            StatusCode::OK,
            Json(json!({
                "id": "chatcmpl-mock-1",
                "object": "chat.completion",
                "choices": [{
                    "index": 0,
                    "message": {
                        "role": "assistant",
                        "content": null,
                        "tool_calls": [{
                            "id": "call_123",
                            "type": "function",
                            "function": {
                                "name": "get_weather",
                                "arguments": "{\"location\":\"Hanoi\"}"
                            }
                        }]
                    },
                    "finish_reason": "tool_calls"
                }],
                "usage": {
                    "prompt_tokens": 15,
                    "completion_tokens": 25,
                    "total_tokens": 40
                }
            })),
        ),
        _ => (
            StatusCode::OK,
            Json(json!({
                "id": "chatcmpl-mock-2",
                "object": "chat.completion",
                "choices": [{
                    "index": 0,
                    "message": {
                        "role": "assistant",
                        "content": "Hello from OpenAI mock!"
                    },
                    "finish_reason": "stop"
                }],
                "usage": {
                    "prompt_tokens": 10,
                    "completion_tokens": 8,
                    "total_tokens": 18
                }
            })),
        ),
    }
}

async fn anthropic_handler(
    State(state): State<MockServerState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    *state.last_anthropic_headers.lock().await = Some(headers.clone());
    *state.last_anthropic_body.lock().await = Some(body.clone());

    let delay = state.delay_ms.load(Ordering::Relaxed);
    if delay > 0 {
        tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;
    }

    let api_key = headers
        .get("x-api-key")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    let version = headers
        .get("anthropic-version")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();

    if api_key.contains("bad-key") || version != "2023-06-01" {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({
                "type": "error",
                "error": {
                    "type": "authentication_error",
                    "message": "Invalid x-api-key"
                }
            })),
        );
    }

    let mode = state.anthropic_mode.lock().await.clone();
    match mode.as_str() {
        "rate_limit" => (
            StatusCode::TOO_MANY_REQUESTS,
            Json(json!({
                "type": "error",
                "error": {
                    "type": "rate_limit_error",
                    "message": "Anthropic rate limit exceeded"
                }
            })),
        ),
        "server_error" => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "type": "error",
                "error": {
                    "type": "api_error",
                    "message": "Anthropic overloaded"
                }
            })),
        ),
        "tool_call" => (
            StatusCode::OK,
            Json(json!({
                "id": "msg_mock_tool",
                "type": "message",
                "role": "assistant",
                "content": [{
                    "type": "tool_use",
                    "id": "toolu_01",
                    "name": "calculator",
                    "input": {
                        "expr": "2+2"
                    }
                }],
                "usage": {
                    "input_tokens": 20,
                    "output_tokens": 15
                }
            })),
        ),
        _ => (
            StatusCode::OK,
            Json(json!({
                "id": "msg_mock_text",
                "type": "message",
                "role": "assistant",
                "content": [{
                    "type": "text",
                    "text": "Hello from Anthropic mock!"
                }],
                "usage": {
                    "input_tokens": 12,
                    "output_tokens": 6
                }
            })),
        ),
    }
}

async fn gemini_handler(
    State(state): State<MockServerState>,
    Path(action): Path<String>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    *state.last_gemini_action.lock().await = Some(action);
    *state.last_gemini_headers.lock().await = Some(headers.clone());
    *state.last_gemini_body.lock().await = Some(body.clone());

    let delay = state.delay_ms.load(Ordering::Relaxed);
    if delay > 0 {
        tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;
    }

    let api_key = headers
        .get("x-goog-api-key")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();

    if api_key.contains("bad-key") {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": {
                    "code": 400,
                    "message": "API key not valid. Please pass a valid API key (API_KEY_INVALID)",
                    "status": "INVALID_ARGUMENT"
                }
            })),
        );
    }

    let mode = state.gemini_mode.lock().await.clone();
    match mode.as_str() {
        "rate_limit" => (
            StatusCode::TOO_MANY_REQUESTS,
            Json(json!({
                "error": {
                    "code": 429,
                    "message": "Resource has been exhausted",
                    "status": "RESOURCE_EXHAUSTED"
                }
            })),
        ),
        "server_error" => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "error": {
                    "code": 500,
                    "message": "Internal error encountered",
                    "status": "INTERNAL"
                }
            })),
        ),
        "tool_call" => (
            StatusCode::OK,
            Json(json!({
                "candidates": [{
                    "content": {
                        "parts": [{
                            "functionCall": {
                                "name": "search_docs",
                                "args": {
                                    "query": "tauri packaging"
                                }
                            }
                        }],
                        "role": "model"
                    },
                    "finishReason": "STOP",
                    "index": 0
                }],
                "usageMetadata": {
                    "promptTokenCount": 18,
                    "candidatesTokenCount": 22,
                    "totalTokenCount": 40
                }
            })),
        ),
        _ => (
            StatusCode::OK,
            Json(json!({
                "candidates": [{
                    "content": {
                        "parts": [{
                            "text": "Hello from Gemini mock!"
                        }],
                        "role": "model"
                    },
                    "finishReason": "STOP",
                    "index": 0
                }],
                "usageMetadata": {
                    "promptTokenCount": 14,
                    "candidatesTokenCount": 7,
                    "totalTokenCount": 21
                }
            })),
        ),
    }
}

async fn start_mock_provider_server(state: MockServerState) -> String {
    let app = Router::new()
        .route("/v1/chat/completions", post(openai_handler))
        .route("/v1/messages", post(anthropic_handler))
        .route("/v1beta/models/{action}", post(gemini_handler))
        .with_state(state);

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    format!("http://{}", addr)
}

fn sample_request() -> ModelRequest {
    ModelRequest {
        messages: vec![
            ChatMessage {
                role: MessageRole::System,
                content: "System instructions".to_string(),
                tool_calls: None,
                tool_call_id: None,
            },
            ChatMessage {
                role: MessageRole::User,
                content: "Hello!".to_string(),
                tool_calls: None,
                tool_call_id: None,
            },
        ],
        tools: vec![ToolDefinition {
            name: "test_tool".to_string(),
            description: "A test tool".to_string(),
            parameters_schema: json!({
                "type": "object",
                "properties": {
                    "param": { "type": "string" }
                }
            }),
        }],
        temperature: Some(0.7),
        max_tokens: Some(1024),
        model_override: None,
    }
}

#[tokio::test]
async fn test_openai_success_text() {
    let state = MockServerState::default();
    let base_url = start_mock_provider_server(state.clone()).await;

    let config = ModelConfig::new(&base_url, "sk-valid-key", "gpt-4o-mini");
    let client = OpenAiCompatibleClient::new(config).unwrap();

    let resp = client.complete(&sample_request()).await.unwrap();
    assert_eq!(resp.content.as_deref(), Some("Hello from OpenAI mock!"));
    assert!(resp.tool_calls.is_empty());
    assert_eq!(resp.usage.unwrap().total_tokens, 18);

    let headers = state.last_openai_headers.lock().await.clone().unwrap();
    assert_eq!(
        headers.get("authorization").unwrap().to_str().unwrap(),
        "Bearer sk-valid-key"
    );
}

#[tokio::test]
async fn test_openai_success_tool_call() {
    let state = MockServerState::default();
    *state.openai_mode.lock().await = "tool_call".to_string();
    let base_url = start_mock_provider_server(state).await;

    let config = ModelConfig::new(&base_url, "sk-valid-key", "gpt-4o-mini");
    let client = OpenAiCompatibleClient::new(config).unwrap();

    let resp = client.complete(&sample_request()).await.unwrap();
    assert!(resp.content.is_none());
    assert_eq!(resp.tool_calls.len(), 1);
    assert_eq!(resp.tool_calls[0].name, "get_weather");
    assert_eq!(resp.tool_calls[0].arguments["location"], "Hanoi");
    assert_eq!(resp.usage.unwrap().total_tokens, 40);
}

#[tokio::test]
async fn test_openai_auth_error_401() {
    let state = MockServerState::default();
    let base_url = start_mock_provider_server(state).await;

    let config = ModelConfig::new(&base_url, "sk-bad-key", "gpt-4o-mini");
    let client = OpenAiCompatibleClient::new(config).unwrap();

    let err = client.complete(&sample_request()).await.unwrap_err();
    match err {
        ModelError::AuthenticationError(msg) => assert!(msg.contains("HTTP 401")),
        other => panic!("Expected AuthenticationError, got {:?}", other),
    }
}

#[tokio::test]
async fn test_openai_rate_limit_429() {
    let state = MockServerState::default();
    *state.openai_mode.lock().await = "rate_limit".to_string();
    let base_url = start_mock_provider_server(state).await;

    let config = ModelConfig::new(&base_url, "sk-valid-key", "gpt-4o-mini");
    let client = OpenAiCompatibleClient::new(config).unwrap();

    let err = client.complete(&sample_request()).await.unwrap_err();
    match err {
        ModelError::ProviderError {
            status_code,
            message,
        } => {
            assert_eq!(status_code, 429);
            assert!(message.contains("Rate limit reached"));
        }
        other => panic!("Expected ProviderError 429, got {:?}", other),
    }
}

#[tokio::test]
async fn test_openai_server_error_500() {
    let state = MockServerState::default();
    *state.openai_mode.lock().await = "server_error".to_string();
    let base_url = start_mock_provider_server(state).await;

    let config = ModelConfig::new(&base_url, "sk-valid-key", "gpt-4o-mini");
    let client = OpenAiCompatibleClient::new(config).unwrap();

    let err = client.complete(&sample_request()).await.unwrap_err();
    match err {
        ModelError::ProviderError {
            status_code,
            message,
        } => {
            assert_eq!(status_code, 500);
            assert!(message.contains("Internal server error from provider"));
        }
        other => panic!("Expected ProviderError 500, got {:?}", other),
    }
}

#[tokio::test]
async fn test_openai_timeout() {
    let state = MockServerState::default();
    state.delay_ms.store(500, Ordering::Relaxed);
    let base_url = start_mock_provider_server(state).await;

    let fast_client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(50))
        .build()
        .unwrap();
    let direct_url = format!("{}/v1/chat/completions", base_url);
    let err = fast_client
        .post(&direct_url)
        .header("authorization", "Bearer sk-valid-key")
        .json(&json!({ "model": "gpt-4o-mini", "messages": [] }))
        .send()
        .await
        .unwrap_err();
    assert!(err.is_timeout());
}

#[tokio::test]
async fn test_openai_model_override() {
    let state = MockServerState::default();
    let base_url = start_mock_provider_server(state.clone()).await;

    let config = ModelConfig::new(&base_url, "sk-valid-key", "default-model");
    let client = OpenAiCompatibleClient::new(config).unwrap();

    let mut req = sample_request();
    req.model_override = Some("override-model-v1".to_string());

    let _resp = client.complete(&req).await.unwrap();
    let body = state.last_openai_body.lock().await.clone().unwrap();
    assert_eq!(body["model"], "override-model-v1");
}

#[tokio::test]
async fn test_anthropic_success_text() {
    let state = MockServerState::default();
    let base_url = start_mock_provider_server(state.clone()).await;

    let config = ModelConfig::new(&base_url, "sk-ant-valid-key", "claude-3-7-sonnet");
    let client = AnthropicClient::new(config).unwrap();

    let resp = client.complete(&sample_request()).await.unwrap();
    assert_eq!(resp.content.as_deref(), Some("Hello from Anthropic mock!"));
    assert!(resp.tool_calls.is_empty());
    assert_eq!(resp.usage.unwrap().total_tokens, 18);

    let headers = state.last_anthropic_headers.lock().await.clone().unwrap();
    assert_eq!(
        headers.get("x-api-key").unwrap().to_str().unwrap(),
        "sk-ant-valid-key"
    );
    assert_eq!(
        headers.get("anthropic-version").unwrap().to_str().unwrap(),
        "2023-06-01"
    );

    let body = state.last_anthropic_body.lock().await.clone().unwrap();
    assert_eq!(body["system"], "System instructions");
}

#[tokio::test]
async fn test_anthropic_success_tool_call() {
    let state = MockServerState::default();
    *state.anthropic_mode.lock().await = "tool_call".to_string();
    let base_url = start_mock_provider_server(state).await;

    let config = ModelConfig::new(&base_url, "sk-ant-valid-key", "claude-3-7-sonnet");
    let client = AnthropicClient::new(config).unwrap();

    let resp = client.complete(&sample_request()).await.unwrap();
    assert!(resp.content.is_none());
    assert_eq!(resp.tool_calls.len(), 1);
    assert_eq!(resp.tool_calls[0].name, "calculator");
    assert_eq!(resp.tool_calls[0].arguments["expr"], "2+2");
    assert_eq!(resp.usage.unwrap().total_tokens, 35);
}

#[tokio::test]
async fn test_anthropic_auth_error_401() {
    let state = MockServerState::default();
    let base_url = start_mock_provider_server(state).await;

    let config = ModelConfig::new(&base_url, "sk-ant-bad-key", "claude-3-7-sonnet");
    let client = AnthropicClient::new(config).unwrap();

    let err = client.complete(&sample_request()).await.unwrap_err();
    match err {
        ModelError::AuthenticationError(msg) => assert!(msg.contains("HTTP 401")),
        other => panic!("Expected AuthenticationError, got {:?}", other),
    }
}

#[tokio::test]
async fn test_anthropic_rate_limit_429() {
    let state = MockServerState::default();
    *state.anthropic_mode.lock().await = "rate_limit".to_string();
    let base_url = start_mock_provider_server(state).await;

    let config = ModelConfig::new(&base_url, "sk-ant-valid-key", "claude-3-7-sonnet");
    let client = AnthropicClient::new(config).unwrap();

    let err = client.complete(&sample_request()).await.unwrap_err();
    match err {
        ModelError::ProviderError {
            status_code,
            message,
        } => {
            assert_eq!(status_code, 429);
            assert!(message.contains("Anthropic rate limit exceeded"));
        }
        other => panic!("Expected ProviderError 429, got {:?}", other),
    }
}

#[tokio::test]
async fn test_anthropic_server_error_500() {
    let state = MockServerState::default();
    *state.anthropic_mode.lock().await = "server_error".to_string();
    let base_url = start_mock_provider_server(state).await;

    let config = ModelConfig::new(&base_url, "sk-ant-valid-key", "claude-3-7-sonnet");
    let client = AnthropicClient::new(config).unwrap();

    let err = client.complete(&sample_request()).await.unwrap_err();
    match err {
        ModelError::ProviderError {
            status_code,
            message,
        } => {
            assert_eq!(status_code, 500);
            assert!(message.contains("Anthropic overloaded"));
        }
        other => panic!("Expected ProviderError 500, got {:?}", other),
    }
}

#[tokio::test]
async fn test_anthropic_model_override() {
    let state = MockServerState::default();
    let base_url = start_mock_provider_server(state.clone()).await;

    let config = ModelConfig::new(&base_url, "sk-ant-valid-key", "default-claude");
    let client = AnthropicClient::new(config).unwrap();

    let mut req = sample_request();
    req.model_override = Some("claude-3-opus-custom".to_string());

    let _resp = client.complete(&req).await.unwrap();
    let body = state.last_anthropic_body.lock().await.clone().unwrap();
    assert_eq!(body["model"], "claude-3-opus-custom");
}

#[tokio::test]
async fn test_gemini_success_text() {
    let state = MockServerState::default();
    let base_url = start_mock_provider_server(state.clone()).await;

    let config = ModelConfig::new(&base_url, "AIza-valid-key", "gemini-3.8-flash-high");
    let client = GeminiClient::new(config).unwrap();

    let resp = client.complete(&sample_request()).await.unwrap();
    assert_eq!(resp.content.as_deref(), Some("Hello from Gemini mock!"));
    assert!(resp.tool_calls.is_empty());
    assert_eq!(resp.usage.unwrap().total_tokens, 21);

    let headers = state.last_gemini_headers.lock().await.clone().unwrap();
    assert_eq!(
        headers.get("x-goog-api-key").unwrap().to_str().unwrap(),
        "AIza-valid-key"
    );

    let body = state.last_gemini_body.lock().await.clone().unwrap();
    assert_eq!(
        body["systemInstruction"]["parts"][0]["text"],
        "System instructions"
    );
}

#[tokio::test]
async fn test_gemini_success_tool_call() {
    let state = MockServerState::default();
    *state.gemini_mode.lock().await = "tool_call".to_string();
    let base_url = start_mock_provider_server(state).await;

    let config = ModelConfig::new(&base_url, "AIza-valid-key", "gemini-3.8-flash-high");
    let client = GeminiClient::new(config).unwrap();

    let resp = client.complete(&sample_request()).await.unwrap();
    assert!(resp.content.is_none());
    assert_eq!(resp.tool_calls.len(), 1);
    assert_eq!(resp.tool_calls[0].name, "search_docs");
    assert_eq!(resp.tool_calls[0].arguments["query"], "tauri packaging");
    assert_eq!(resp.usage.unwrap().total_tokens, 40);
}

#[tokio::test]
async fn test_gemini_auth_error_400_api_key_invalid() {
    let state = MockServerState::default();
    let base_url = start_mock_provider_server(state).await;

    let config = ModelConfig::new(&base_url, "AIza-bad-key", "gemini-3.8-flash-high");
    let client = GeminiClient::new(config).unwrap();

    let err = client.complete(&sample_request()).await.unwrap_err();
    match err {
        ModelError::AuthenticationError(msg) => {
            assert!(msg.contains("API_KEY_INVALID"));
        }
        other => panic!("Expected AuthenticationError, got {:?}", other),
    }
}

#[tokio::test]
async fn test_gemini_rate_limit_429() {
    let state = MockServerState::default();
    *state.gemini_mode.lock().await = "rate_limit".to_string();
    let base_url = start_mock_provider_server(state).await;

    let config = ModelConfig::new(&base_url, "AIza-valid-key", "gemini-3.8-flash-high");
    let client = GeminiClient::new(config).unwrap();

    let err = client.complete(&sample_request()).await.unwrap_err();
    match err {
        ModelError::ProviderError {
            status_code,
            message,
        } => {
            assert_eq!(status_code, 429);
            assert!(message.contains("Resource has been exhausted"));
        }
        other => panic!("Expected ProviderError 429, got {:?}", other),
    }
}

#[tokio::test]
async fn test_gemini_server_error_500() {
    let state = MockServerState::default();
    *state.gemini_mode.lock().await = "server_error".to_string();
    let base_url = start_mock_provider_server(state).await;

    let config = ModelConfig::new(&base_url, "AIza-valid-key", "gemini-3.8-flash-high");
    let client = GeminiClient::new(config).unwrap();

    let err = client.complete(&sample_request()).await.unwrap_err();
    match err {
        ModelError::ProviderError {
            status_code,
            message,
        } => {
            assert_eq!(status_code, 500);
            assert!(message.contains("Internal error encountered"));
        }
        other => panic!("Expected ProviderError 500, got {:?}", other),
    }
}

#[tokio::test]
async fn test_gemini_model_override() {
    let state = MockServerState::default();
    let base_url = start_mock_provider_server(state.clone()).await;

    let config = ModelConfig::new(&base_url, "AIza-valid-key", "default-gemini");
    let client = GeminiClient::new(config).unwrap();

    let mut req = sample_request();
    req.model_override = Some("gemini-1.5-pro-latest".to_string());

    let _resp = client.complete(&req).await.unwrap();
    let action = state.last_gemini_action.lock().await.clone().unwrap();
    assert!(action.contains("gemini-1.5-pro-latest"));
}
