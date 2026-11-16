use axum::extract::{Path, State};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;

use crate::checkpoints::store::CheckpointStore;
use crate::checkpoints::types::ExecutionPolicyRecord;
use crate::domain::{ChatMessage, MessageRole};
use crate::models::{
    AnthropicClient, GeminiClient, ModelClient, ModelConfig, ModelRequest, OpenAiCompatibleClient,
};
use crate::profiles::{mask_api_key, CredentialResponse, ProviderConnection};
use crate::server::auth::AuthenticatedUser;
use crate::server::error::{ApiError, AppJson};
use crate::server::state::AppState;

#[derive(Debug, Deserialize)]
pub struct CreateCredentialRequest {
    pub preset_id: String,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub is_default: Option<bool>,
    pub default_models: Option<Value>,
    pub settings: Option<Value>,
    pub secrets: Option<HashMap<String, String>>,
}

pub async fn get_provider_presets_handler() -> Result<Json<Value>, ApiError> {
    Ok(Json(json!({
        "categories": {
            "direct": [
                {
                    "id": "openai",
                    "name": "OpenAI",
                    "category": "direct",
                    "description": "GPT-4o, GPT-4o-mini, o1",
                    "protocol": "openai",
                    "base_url": "https://api.openai.com/v1",
                    "models": [
                        { "id": "gpt-4o", "kind": "chat", "prompt_usd_per_million": "2.5", "completion_usd_per_million": "10.0" },
                        { "id": "gpt-4o-mini", "kind": "chat", "prompt_usd_per_million": "0.15", "completion_usd_per_million": "0.6" }
                    ],
                    "auth_fields": [
                        { "name": "api_key", "label": "API Key", "required": true, "secret": true, "placeholder": "sk-..." }
                    ],
                    "icon": "openai",
                    "local": false
                },
                {
                    "id": "anthropic",
                    "name": "Anthropic",
                    "category": "direct",
                    "description": "Claude 3.7 Sonnet, Claude 3.5 Haiku",
                    "protocol": "anthropic",
                    "base_url": "https://api.anthropic.com/v1",
                    "models": [
                        { "id": "claude-3-7-sonnet", "kind": "chat", "prompt_usd_per_million": "3.0", "completion_usd_per_million": "15.0" },
                        { "id": "claude-3-5-haiku", "kind": "chat", "prompt_usd_per_million": "0.8", "completion_usd_per_million": "4.0" }
                    ],
                    "auth_fields": [
                        { "name": "api_key", "label": "API Key", "required": true, "secret": true, "placeholder": "sk-ant-..." }
                    ],
                    "icon": "anthropic",
                    "local": false
                },
                {
                    "id": "google_gemini",
                    "name": "Google Gemini",
                    "category": "direct",
                    "description": "Gemini 3.8 Flash, Gemini 1.5 Pro",
                    "protocol": "gemini",
                    "base_url": "https://generativelanguage.googleapis.com",
                    "models": [
                        { "id": "gemini-3.8-flash-high", "kind": "chat", "prompt_usd_per_million": "0.1", "completion_usd_per_million": "0.4" },
                        { "id": "gemini-1.5-pro", "kind": "chat", "prompt_usd_per_million": "1.25", "completion_usd_per_million": "5.0" }
                    ],
                    "auth_fields": [
                        { "name": "api_key", "label": "API Key", "required": true, "secret": true, "placeholder": "AIza..." }
                    ],
                    "icon": "google",
                    "local": false
                },
                {
                    "id": "deepseek",
                    "name": "DeepSeek",
                    "category": "direct",
                    "description": "DeepSeek-V3, DeepSeek-R1",
                    "protocol": "openai",
                    "base_url": "https://api.deepseek.com/v1",
                    "models": [
                        { "id": "deepseek-chat", "kind": "chat", "prompt_usd_per_million": "0.14", "completion_usd_per_million": "0.28" },
                        { "id": "deepseek-reasoner", "kind": "chat", "prompt_usd_per_million": "0.55", "completion_usd_per_million": "2.19" }
                    ],
                    "auth_fields": [
                        { "name": "api_key", "label": "API Key", "required": true, "secret": true, "placeholder": "sk-..." }
                    ],
                    "icon": "deepseek",
                    "local": false
                }
            ],
            "subscription": [],
            "unified": [],
            "local": [],
            "media": [],
            "tools": [],
            "channel": []
        }
    })))
}

pub async fn get_credentials_handler(
    auth: AuthenticatedUser,
    State(state): State<AppState>,
) -> Result<Json<Vec<CredentialResponse>>, ApiError> {
    let user = auth.0;
    let conns = state
        .store
        .get_provider_connections_by_profile(&user.sub)
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let resp: Vec<CredentialResponse> = conns
        .into_iter()
        .map(|c| c.to_credential_response())
        .collect();
    Ok(Json(resp))
}

pub async fn create_credential_handler(
    auth: AuthenticatedUser,
    State(state): State<AppState>,
    AppJson(body): AppJson<CreateCredentialRequest>,
) -> Result<Json<CredentialResponse>, ApiError> {
    let user = auth.0;
    let preset_id = body.preset_id.trim();
    if preset_id.is_empty() {
        return Err(ApiError::bad_request("preset_id cannot be empty"));
    }

    let existing_conns = state
        .store
        .get_provider_connections_by_profile(&user.sub)
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let existing = existing_conns
        .into_iter()
        .find(|c| c.preset_id == preset_id);

    let conn_id = existing
        .as_ref()
        .map(|c| c.id.clone())
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    let api_key = body
        .api_key
        .as_deref()
        .or_else(|| {
            body.secrets
                .as_ref()
                .and_then(|s| s.get("api_key").map(|k| k.as_str()))
        })
        .map(str::trim)
        .filter(|k| !k.is_empty());

    let secret_key = format!("account:{}:provider:{}", user.sub, conn_id);

    let masked_key = match api_key {
        Some(k) => {
            state
                .credentials
                .set_secret(&secret_key, k)
                .map_err(|e| ApiError::internal(format!("Failed to store secret: {e}")))?;
            Some(mask_api_key(k))
        }
        None => {
            if let Some(ref ext) = existing {
                ext.masked_key.clone()
            } else {
                return Err(ApiError::bad_request("api_key is required"));
            }
        }
    };

    let (provider, base_url_default, models) = match preset_id {
        "openai" => (
            "openai".to_string(),
            Some("https://api.openai.com/v1".to_string()),
            vec!["gpt-4o".to_string(), "gpt-4o-mini".to_string()],
        ),
        "anthropic" => (
            "anthropic".to_string(),
            Some("https://api.anthropic.com/v1".to_string()),
            vec![
                "claude-3-7-sonnet".to_string(),
                "claude-3-5-haiku".to_string(),
            ],
        ),
        "google_gemini" => (
            "gemini".to_string(),
            Some("https://generativelanguage.googleapis.com".to_string()),
            vec![
                "gemini-3.8-flash-high".to_string(),
                "gemini-1.5-pro".to_string(),
            ],
        ),
        "deepseek" => (
            "deepseek".to_string(),
            Some("https://api.deepseek.com/v1".to_string()),
            vec!["deepseek-chat".to_string(), "deepseek-reasoner".to_string()],
        ),
        other => (other.to_string(), body.base_url.clone(), Vec::new()),
    };

    let base_url = body
        .base_url
        .filter(|s| !s.trim().is_empty())
        .or(base_url_default);
    let now = chrono::Utc::now().timestamp();

    let conn = ProviderConnection {
        id: conn_id.clone(),
        profile_id: user.sub.clone(),
        provider: provider.clone(),
        preset_id: preset_id.to_string(),
        auth_type: "api_key".to_string(),
        account_label: None,
        status: "CONNECTED".to_string(),
        base_url,
        is_default: body.is_default.unwrap_or(false),
        available_models: models,
        default_models: body.default_models.unwrap_or_else(|| json!({})),
        settings: body.settings.unwrap_or_else(|| json!({})),
        masked_key: masked_key.clone(),
        safe_error: None,
        created_at: existing.as_ref().map(|c| c.created_at).unwrap_or(now),
        updated_at: now,
    };

    state
        .store
        .create_or_update_provider_connection(&conn)
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let audit_tenant = uuid::Uuid::parse_str(&user.tenant_id).unwrap_or_else(|_| {
        uuid::Uuid::parse_str(crate::server::auth::DEFAULT_LOCAL_TENANT_ID).unwrap()
    });
    let audit_workspace = uuid::Uuid::parse_str(&user.workspace_id).unwrap_or_else(|_| {
        uuid::Uuid::parse_str(crate::server::auth::DEFAULT_LOCAL_WORKSPACE_ID).unwrap()
    });
    let audit_scope =
        crate::domain::ExecutionScope::new(audit_tenant, audit_workspace, uuid::Uuid::new_v4())
            .unwrap_or_else(|_| crate::domain::ExecutionScope {
                tenant_id: audit_tenant,
                workspace_id: audit_workspace,
                run_id: uuid::Uuid::new_v4(),
            });

    let entry = crate::checkpoints::types::AuditLogEntry {
        id: uuid::Uuid::new_v4(),
        scope: audit_scope,
        actor_id: Some(user.sub.clone()),
        action: "provider_connection_created".to_string(),
        target_id: uuid::Uuid::parse_str(&conn_id).ok(),
        details: json!({
            "connection_id": conn_id,
            "preset_id": preset_id,
            "provider": provider,
            "masked_key": masked_key,
        }),
        created_at: chrono::Utc::now(),
    };
    state
        .store
        .record_audit_log(&entry)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok(Json(conn.to_credential_response()))
}

async fn run_provider_health_check(
    provider: &str,
    base_url: Option<&str>,
    secret: &str,
    model: &str,
) -> Result<(), String> {
    let req = ModelRequest {
        messages: vec![ChatMessage {
            role: MessageRole::User,
            content: "ping".to_string(),
            tool_calls: None,
            tool_call_id: None,
        }],
        tools: Vec::new(),
        temperature: None,
        max_tokens: Some(1),
        model_override: None,
    };

    match provider {
        "anthropic" => {
            let base = base_url.unwrap_or("https://api.anthropic.com/v1");
            let config = ModelConfig::new(base, secret, model).with_timeout(5);
            let client = AnthropicClient::new(config).map_err(|e| e.to_string())?;
            client.complete(&req).await.map_err(|e| e.to_string())?;
        }
        "gemini" => {
            let base = base_url.unwrap_or("https://generativelanguage.googleapis.com");
            let config = ModelConfig::new(base, secret, model).with_timeout(5);
            let client = GeminiClient::new(config).map_err(|e| e.to_string())?;
            client.complete(&req).await.map_err(|e| e.to_string())?;
        }
        _ => {
            let base = base_url.unwrap_or("https://api.openai.com/v1");
            let config = ModelConfig::new(base, secret, model).with_timeout(5);
            let client = OpenAiCompatibleClient::new(config).map_err(|e| e.to_string())?;
            client.complete(&req).await.map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

pub async fn test_credential_handler(
    auth: AuthenticatedUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let user = auth.0;
    let conn = state
        .store
        .get_provider_connection_by_id(&id)
        .map_err(|e| ApiError::internal(e.to_string()))?
        .ok_or_else(|| ApiError::not_found("Credential not found"))?;

    if conn.profile_id != user.sub {
        return Err(ApiError::not_found("Credential not found"));
    }

    let secret_key = format!("account:{}:provider:{}", user.sub, id);
    let secret = state
        .credentials
        .get_secret(&secret_key)
        .map_err(|e| ApiError::internal(format!("Credential store error: {e}")))?
        .ok_or_else(|| ApiError::bad_request("No secret found for this credential"))?;

    let test_model = conn
        .available_models
        .first()
        .map(|s| s.as_str())
        .unwrap_or("default");

    let start = std::time::Instant::now();
    let test_res = run_provider_health_check(
        &conn.provider,
        conn.base_url.as_deref(),
        &secret,
        test_model,
    )
    .await;
    let latency_ms = start.elapsed().as_millis() as u64;

    let (status, safe_error) = match test_res {
        Ok(()) => ("CONNECTED".to_string(), None),
        Err(e) => ("ERROR".to_string(), Some(e)),
    };

    let mut updated_conn = conn.clone();
    updated_conn.status = status.clone();
    updated_conn.safe_error = safe_error.clone();
    updated_conn.updated_at = chrono::Utc::now().timestamp();
    state
        .store
        .create_or_update_provider_connection(&updated_conn)
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let audit_tenant = uuid::Uuid::parse_str(&user.tenant_id).unwrap_or_else(|_| {
        uuid::Uuid::parse_str(crate::server::auth::DEFAULT_LOCAL_TENANT_ID).unwrap()
    });
    let audit_workspace = uuid::Uuid::parse_str(&user.workspace_id).unwrap_or_else(|_| {
        uuid::Uuid::parse_str(crate::server::auth::DEFAULT_LOCAL_WORKSPACE_ID).unwrap()
    });
    let audit_scope =
        crate::domain::ExecutionScope::new(audit_tenant, audit_workspace, uuid::Uuid::new_v4())
            .unwrap_or_else(|_| crate::domain::ExecutionScope {
                tenant_id: audit_tenant,
                workspace_id: audit_workspace,
                run_id: uuid::Uuid::new_v4(),
            });

    let entry = crate::checkpoints::types::AuditLogEntry {
        id: uuid::Uuid::new_v4(),
        scope: audit_scope,
        actor_id: Some(user.sub.clone()),
        action: "provider_connection_tested".to_string(),
        target_id: uuid::Uuid::parse_str(&id).ok(),
        details: json!({
            "connection_id": id,
            "provider": conn.provider,
            "status": status,
            "latency_ms": latency_ms,
            "error": safe_error,
        }),
        created_at: chrono::Utc::now(),
    };
    state
        .store
        .record_audit_log(&entry)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok(Json(json!({
        "status": status,
        "latency_ms": latency_ms,
        "available_models": conn.available_models,
        "balance": null,
        "error": safe_error,
    })))
}

pub async fn delete_credential_handler(
    auth: AuthenticatedUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let user = auth.0;
    let conn = state
        .store
        .get_provider_connection_by_id(&id)
        .map_err(|e| ApiError::internal(e.to_string()))?
        .ok_or_else(|| ApiError::not_found("Credential not found"))?;

    if conn.profile_id != user.sub {
        return Err(ApiError::not_found("Credential not found"));
    }

    let secret_key = format!("account:{}:provider:{}", user.sub, id);
    if let Err(e) = state.credentials.delete_secret(&secret_key) {
        eprintln!("Failed to delete provider secret from keychain: {e}");
    }

    state
        .store
        .delete_provider_connection(&id, &user.sub)
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let audit_tenant = uuid::Uuid::parse_str(&user.tenant_id).unwrap_or_else(|_| {
        uuid::Uuid::parse_str(crate::server::auth::DEFAULT_LOCAL_TENANT_ID).unwrap()
    });
    let audit_workspace = uuid::Uuid::parse_str(&user.workspace_id).unwrap_or_else(|_| {
        uuid::Uuid::parse_str(crate::server::auth::DEFAULT_LOCAL_WORKSPACE_ID).unwrap()
    });
    let audit_scope =
        crate::domain::ExecutionScope::new(audit_tenant, audit_workspace, uuid::Uuid::new_v4())
            .unwrap_or_else(|_| crate::domain::ExecutionScope {
                tenant_id: audit_tenant,
                workspace_id: audit_workspace,
                run_id: uuid::Uuid::new_v4(),
            });

    let entry = crate::checkpoints::types::AuditLogEntry {
        id: uuid::Uuid::new_v4(),
        scope: audit_scope,
        actor_id: Some(user.sub.clone()),
        action: "provider_connection_deleted".to_string(),
        target_id: uuid::Uuid::parse_str(&id).ok(),
        details: json!({
            "connection_id": id,
            "provider": conn.provider,
            "preset_id": conn.preset_id,
        }),
        created_at: chrono::Utc::now(),
    };
    state
        .store
        .record_audit_log(&entry)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok(Json(json!({ "deleted": true })))
}

pub async fn get_default_models_handler(
    auth: AuthenticatedUser,
    State(state): State<AppState>,
) -> Result<Json<Value>, ApiError> {
    let user = auth.0;
    let saved = state
        .store
        .get_account_default_models(&user.sub)
        .map_err(|e| ApiError::internal(e.to_string()))?;
    if let Some(s) = saved {
        let val: Value = serde_json::from_str(&s).unwrap_or(json!({}));
        Ok(Json(val))
    } else {
        Ok(Json(json!({
            "orchestrator": "google_gemini/gemini-3.8-flash-high",
            "fast": "google_gemini/gemini-3.8-flash-high",
            "coding": "anthropic/claude-3-7-sonnet"
        })))
    }
}

pub async fn update_default_models_handler(
    auth: AuthenticatedUser,
    State(state): State<AppState>,
    AppJson(body): AppJson<Value>,
) -> Result<Json<Value>, ApiError> {
    let user = auth.0;
    let s = serde_json::to_string(&body).map_err(|e| ApiError::bad_request(e.to_string()))?;
    state
        .store
        .set_account_default_models(&user.sub, &s)
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(body))
}

pub async fn get_chat_models_handler(_auth: AuthenticatedUser) -> Result<Json<Value>, ApiError> {
    Ok(Json(json!({
        "models": [
            {
                "id": "gemini-3.8-flash-high",
                "name": "Gemini 3.8 Flash (High)",
                "provider": "google",
                "context_length": 1048576,
            },
            {
                "id": "claude-3-7-sonnet",
                "name": "Claude 3.7 Sonnet",
                "provider": "anthropic",
                "context_length": 200000,
            }
        ],
        "default": "gemini-3.8-flash-high"
    })))
}

pub async fn get_execution_policy_handler(
    _auth: AuthenticatedUser,
    State(state): State<AppState>,
) -> Result<Json<ExecutionPolicyRecord>, ApiError> {
    let policy = state
        .store
        .get_execution_policy()
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(policy))
}

pub async fn update_execution_policy_handler(
    _auth: AuthenticatedUser,
    State(state): State<AppState>,
    AppJson(body): AppJson<ExecutionPolicyRecord>,
) -> Result<Json<ExecutionPolicyRecord>, ApiError> {
    state
        .store
        .update_execution_policy(&body)
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(body))
}
