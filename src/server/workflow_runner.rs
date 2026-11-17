use chrono::Utc;
use serde_json::Value;
use std::sync::Arc;
use uuid::Uuid;

use crate::checkpoints::types::AuditLogEntry;
use crate::checkpoints::{CheckpointStore, SqliteCheckpointStore};
use crate::credentials::CredentialStore;
use crate::domain::ExecutionScope;
use crate::events::sink::EventSink;
use crate::events::types::{RuntimeEvent, RuntimeEventType};
use crate::governance::DefaultApprovalPolicy;
use crate::models::types::{ModelConfig, ModelError, ModelRequest, ModelResponse};
use crate::models::{AnthropicClient, GeminiClient, ModelClient, OpenAiCompatibleClient};
use crate::schema::WorkflowSpec;
use crate::server::state::AppState;
use crate::tools::register_workspace_tools_with_audit;
use crate::tools::types::ToolRegistry;
use crate::workflow::engine::{WorkflowExecutor, WorkflowOutcome};

#[derive(Clone)]
pub struct BroadcastEventSink {
    store: Arc<SqliteCheckpointStore>,
    tx: tokio::sync::broadcast::Sender<RuntimeEvent>,
}

impl BroadcastEventSink {
    pub fn new(
        store: Arc<SqliteCheckpointStore>,
        tx: tokio::sync::broadcast::Sender<RuntimeEvent>,
    ) -> Self {
        Self { store, tx }
    }
}

impl EventSink for BroadcastEventSink {
    fn emit(&self, event: RuntimeEvent) -> Result<(), crate::events::EventError> {
        if let Err(e) = self.store.insert_run_event(&event) {
            eprintln!("Failed to insert run event: {}", e);
        }
        if self.tx.send(event).is_err() {
            eprintln!("No active broadcast subscribers for event");
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct ServerModelClient {
    openai_client: Option<Arc<OpenAiCompatibleClient>>,
    store: Option<Arc<SqliteCheckpointStore>>,
    credentials: Option<Arc<dyn CredentialStore>>,
    sub: Option<String>,
    scope: Option<ExecutionScope>,
}

impl ServerModelClient {
    pub fn new() -> Self {
        let api_key = std::env::var("OPENAI_API_KEY")
            .or_else(|_| std::env::var("NEXUS_API_KEY"))
            .unwrap_or_default();
        if !api_key.trim().is_empty() {
            let api_base = std::env::var("OPENAI_BASE_URL")
                .or_else(|_| std::env::var("NEXUS_MODEL_BASE_URL"))
                .unwrap_or_else(|_| "https://api.openai.com/v1".to_string());
            let model = std::env::var("OPENAI_MODEL")
                .or_else(|_| std::env::var("NEXUS_MODEL"))
                .unwrap_or_else(|_| "gpt-4o-mini".to_string());
            let client = OpenAiCompatibleClient::new(ModelConfig::new(api_base, api_key, model))
                .ok()
                .map(Arc::new);
            Self {
                openai_client: client,
                store: None,
                credentials: None,
                sub: None,
                scope: None,
            }
        } else {
            Self {
                openai_client: None,
                store: None,
                credentials: None,
                sub: None,
                scope: None,
            }
        }
    }

    pub fn with_client(client: OpenAiCompatibleClient) -> Self {
        Self {
            openai_client: Some(Arc::new(client)),
            store: None,
            credentials: None,
            sub: None,
            scope: None,
        }
    }

    pub fn for_account(
        store: Arc<SqliteCheckpointStore>,
        credentials: Arc<dyn CredentialStore>,
        sub: String,
        scope: Option<ExecutionScope>,
    ) -> Self {
        Self {
            openai_client: None,
            store: Some(store),
            credentials: Some(credentials),
            sub: Some(sub),
            scope,
        }
    }
}

impl Default for ServerModelClient {
    fn default() -> Self {
        Self::new()
    }
}

impl ModelClient for ServerModelClient {
    async fn complete(&self, request: &ModelRequest) -> Result<ModelResponse, ModelError> {
        if let Some(ref sub) = self.sub {
            let store = self.store.as_ref().ok_or_else(|| {
                ModelError::ConfigurationError(
                    "checkpoint store missing for account model client".to_string(),
                )
            })?;
            let credentials = self.credentials.as_ref().ok_or_else(|| {
                ModelError::ConfigurationError(
                    "credential store missing for account model client".to_string(),
                )
            })?;

            let account_opt = store
                .get_google_account_by_sub(sub)
                .map_err(|e| ModelError::ConfigurationError(e.to_string()))?;
            let account = match account_opt {
                Some(acc) if acc.active => acc,
                _ => {
                    return Err(ModelError::AuthenticationError(format!(
                        "account {} is inactive or not found",
                        sub
                    )));
                }
            };

            let connections = store
                .get_provider_connections_by_profile(&account.sub)
                .map_err(|e| ModelError::ConfigurationError(e.to_string()))?;
            if connections.is_empty() {
                let err = ModelError::ConfigurationError(format!(
                    "no provider connections configured for account {}",
                    sub
                ));
                if let Some(scope) = self.scope {
                    let entry = AuditLogEntry {
                        id: Uuid::new_v4(),
                        scope,
                        actor_id: Some(sub.clone()),
                        action: "model.completion_failed".to_string(),
                        target_id: None,
                        details: serde_json::json!({
                            "error": "no provider connections configured",
                        }),
                        created_at: Utc::now(),
                    };
                    if let Err(e) = store.record_audit_log(&entry).await {
                        eprintln!("Failed to record audit log: {}", e);
                    }
                }
                return Err(err);
            }

            let default_models: Option<Value> = store
                .get_account_default_models(sub)
                .ok()
                .flatten()
                .and_then(|s| serde_json::from_str(&s).ok());
            let chat_default = default_models
                .as_ref()
                .and_then(|dm| dm.get("chat_model").or_else(|| dm.get("chat")))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            let (selected_conn, target_model) = if let Some(ref req_model) = request.model_override
            {
                let conn = connections
                    .iter()
                    .find(|c| c.available_models.iter().any(|m| m == req_model))
                    .or_else(|| {
                        if req_model.starts_with("claude") {
                            connections.iter().find(|c| c.preset_id == "anthropic")
                        } else if req_model.starts_with("gemini") {
                            connections.iter().find(|c| c.preset_id == "google_gemini")
                        } else if req_model.starts_with("deepseek") {
                            connections.iter().find(|c| c.preset_id == "deepseek")
                        } else if req_model.starts_with("gpt-") || req_model.starts_with("o1") {
                            connections.iter().find(|c| c.preset_id == "openai")
                        } else {
                            None
                        }
                    })
                    .or_else(|| connections.iter().find(|c| c.is_default))
                    .unwrap_or(&connections[0]);
                (conn, req_model.clone())
            } else {
                let conn = if let Some(ref default_m) = chat_default {
                    connections
                        .iter()
                        .find(|c| c.available_models.iter().any(|m| m == default_m))
                        .or_else(|| connections.iter().find(|c| c.is_default))
                        .unwrap_or(&connections[0])
                } else {
                    connections
                        .iter()
                        .find(|c| c.is_default)
                        .unwrap_or(&connections[0])
                };
                let m = chat_default
                    .or_else(|| {
                        conn.default_models
                            .get("chat")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                    })
                    .or_else(|| conn.available_models.first().cloned())
                    .unwrap_or_else(|| match conn.preset_id.as_str() {
                        "anthropic" => "claude-3-5-haiku".to_string(),
                        "google_gemini" => "gemini-1.5-pro".to_string(),
                        "deepseek" => "deepseek-chat".to_string(),
                        _ => "gpt-4o-mini".to_string(),
                    });
                (conn, m)
            };

            let endpoint_base_url = match crate::profiles::resolve_and_verify_provider_endpoint(
                &selected_conn.preset_id,
                selected_conn.base_url.as_deref(),
            ) {
                Ok(u) => u,
                Err(err) => {
                    let model_err = ModelError::ConfigurationError(err.clone());
                    if let Some(scope) = self.scope {
                        let entry = AuditLogEntry {
                            id: Uuid::new_v4(),
                            scope,
                            actor_id: Some(sub.clone()),
                            action: "model.completion_failed".to_string(),
                            target_id: Uuid::parse_str(&selected_conn.id).ok(),
                            details: serde_json::json!({
                                "provider_id": selected_conn.id,
                                "preset_id": selected_conn.preset_id,
                                "error": err,
                            }),
                            created_at: Utc::now(),
                        };
                        if let Err(e) = store.record_audit_log(&entry).await {
                            eprintln!("Failed to record audit log: {}", e);
                        }
                    }
                    return Err(model_err);
                }
            };

            let secret_key = format!("account:{}:provider:{}", sub, selected_conn.id);
            let secret_opt = credentials.get_secret(&secret_key).map_err(|e| {
                ModelError::AuthenticationError(format!("failed to read secret: {}", e))
            })?;

            let api_key = match secret_opt {
                Some(key) if !key.trim().is_empty() => key,
                _ => {
                    let err = ModelError::AuthenticationError(format!(
                        "credential missing or revoked for provider {}",
                        selected_conn.id
                    ));
                    if let Some(scope) = self.scope {
                        let entry = AuditLogEntry {
                            id: Uuid::new_v4(),
                            scope,
                            actor_id: Some(sub.clone()),
                            action: "model.completion_failed".to_string(),
                            target_id: Uuid::parse_str(&selected_conn.id).ok(),
                            details: serde_json::json!({
                                "provider_id": selected_conn.id,
                                "preset_id": selected_conn.preset_id,
                                "error": "credential missing or revoked",
                            }),
                            created_at: Utc::now(),
                        };
                        if let Err(e) = store.record_audit_log(&entry).await {
                            eprintln!("Failed to record audit log: {}", e);
                        }
                    }
                    return Err(err);
                }
            };

            let mut req = request.clone();
            req.model_override = Some(target_model.clone());

            let result = match selected_conn.preset_id.as_str() {
                "anthropic" => {
                    let client = AnthropicClient::new(ModelConfig::new(
                        endpoint_base_url,
                        api_key,
                        target_model.clone(),
                    ))?;
                    client.complete(&req).await
                }
                "google_gemini" => {
                    let client = GeminiClient::new(ModelConfig::new(
                        endpoint_base_url,
                        api_key,
                        target_model.clone(),
                    ))?;
                    client.complete(&req).await
                }
                "deepseek" => {
                    let client = OpenAiCompatibleClient::new(ModelConfig::new(
                        endpoint_base_url,
                        api_key,
                        target_model.clone(),
                    ))?;
                    client.complete(&req).await
                }
                "openai" | "custom_openai" | "custom" => {
                    let client = OpenAiCompatibleClient::new(ModelConfig::new(
                        endpoint_base_url,
                        api_key,
                        target_model.clone(),
                    ))?;
                    client.complete(&req).await
                }
                unknown => {
                    let err = format!("unknown or unsupported provider preset: {}", unknown);
                    return Err(ModelError::ConfigurationError(err));
                }
            };

            if let Some(scope) = self.scope {
                match &result {
                    Ok(resp) => {
                        let entry = AuditLogEntry {
                            id: Uuid::new_v4(),
                            scope,
                            actor_id: Some(sub.clone()),
                            action: "model.completion".to_string(),
                            target_id: Uuid::parse_str(&selected_conn.id).ok(),
                            details: serde_json::json!({
                                "provider_id": selected_conn.id,
                                "preset_id": selected_conn.preset_id,
                                "model": target_model,
                                "prompt_tokens": resp.usage.as_ref().map(|u| u.prompt_tokens),
                                "completion_tokens": resp.usage.as_ref().map(|u| u.completion_tokens),
                            }),
                            created_at: Utc::now(),
                        };
                        if let Err(e) = store.record_audit_log(&entry).await {
                            eprintln!("Failed to record audit log: {}", e);
                        }
                    }
                    Err(err) => {
                        let entry = AuditLogEntry {
                            id: Uuid::new_v4(),
                            scope,
                            actor_id: Some(sub.clone()),
                            action: "model.completion_failed".to_string(),
                            target_id: Uuid::parse_str(&selected_conn.id).ok(),
                            details: serde_json::json!({
                                "provider_id": selected_conn.id,
                                "preset_id": selected_conn.preset_id,
                                "model": target_model,
                                "error": err.to_string(),
                            }),
                            created_at: Utc::now(),
                        };
                        if let Err(e) = store.record_audit_log(&entry).await {
                            eprintln!("Failed to record audit log: {}", e);
                        }
                    }
                }
            }

            result
        } else if let Some(ref client) = self.openai_client {
            client.complete(request).await
        } else {
            Err(ModelError::ConfigurationError(
                "no model configured: set OPENAI_API_KEY / --api-base".to_string(),
            ))
        }
    }
}

pub async fn run_workflow_background(
    state: AppState,
    scope: ExecutionScope,
    spec: WorkflowSpec,
    input_data: Option<Value>,
) {
    let run_id_str = scope.run_id.to_string();
    if let Err(e) = state
        .store
        .update_run_status(&run_id_str, "RUNNING", None, None)
    {
        eprintln!("Failed to update run status: {}", e);
    }

    let event_sink = Arc::new(BroadcastEventSink::new(
        state.store.clone(),
        state.events_tx.clone(),
    ));
    let start_event = RuntimeEvent {
        id: Uuid::new_v4(),
        scope,
        node_id: None,
        event_type: RuntimeEventType::FlowStateChanged,
        occurred_at: Utc::now(),
        payload: serde_json::json!({
            "status": "running",
            "run_id": run_id_str,
        }),
    };
    if let Err(e) = event_sink.emit(start_event) {
        eprintln!("Failed to emit start event: {}", e);
    }

    let model = Arc::new({
        let tenant_id_str = scope.tenant_id.to_string();
        if let Ok(Some(account)) = state.store.get_account_by_tenant_id(&tenant_id_str) {
            ServerModelClient::for_account(
                state.store.clone(),
                state.credentials.clone(),
                account.sub,
                Some(scope),
            )
        } else if let Some(ref m) = state.model {
            m.clone()
        } else {
            ServerModelClient::default()
        }
    });

    let policy = Arc::new(DefaultApprovalPolicy::new().with_workspace_root(&state.workspace_root));
    let mut registry = ToolRegistry::new();
    register_workspace_tools_with_audit(
        &mut registry,
        &state.workspace_root,
        Some(Arc::clone(&event_sink) as Arc<dyn EventSink>),
        Some(state.store.clone()),
    );

    let executor = WorkflowExecutor::new(
        model,
        state.store.clone(),
        event_sink.clone(),
        policy,
        registry,
        &state.workspace_root,
    )
    .with_author_id("agent-worker");

    let outcome_result = executor.start_workflow(&spec, scope, input_data).await;
    handle_outcome(state, scope, event_sink, outcome_result);
}

pub async fn resume_workflow_background(
    state: AppState,
    scope: ExecutionScope,
    spec: WorkflowSpec,
) {
    let run_id_str = scope.run_id.to_string();
    if let Err(e) = state
        .store
        .update_run_status(&run_id_str, "RUNNING", None, None)
    {
        eprintln!("Failed to update run status: {}", e);
    }

    let event_sink = Arc::new(BroadcastEventSink::new(
        state.store.clone(),
        state.events_tx.clone(),
    ));
    let resume_event = RuntimeEvent {
        id: Uuid::new_v4(),
        scope,
        node_id: None,
        event_type: RuntimeEventType::FlowStateChanged,
        occurred_at: Utc::now(),
        payload: serde_json::json!({
            "status": "running",
            "run_id": run_id_str,
        }),
    };
    if let Err(e) = event_sink.emit(resume_event) {
        eprintln!("Failed to emit resume event: {}", e);
    }

    let model = Arc::new({
        let tenant_id_str = scope.tenant_id.to_string();
        if let Ok(Some(account)) = state.store.get_account_by_tenant_id(&tenant_id_str) {
            ServerModelClient::for_account(
                state.store.clone(),
                state.credentials.clone(),
                account.sub,
                Some(scope),
            )
        } else if let Some(ref m) = state.model {
            m.clone()
        } else {
            ServerModelClient::default()
        }
    });

    let policy = Arc::new(DefaultApprovalPolicy::new().with_workspace_root(&state.workspace_root));
    let mut registry = ToolRegistry::new();
    register_workspace_tools_with_audit(
        &mut registry,
        &state.workspace_root,
        Some(Arc::clone(&event_sink) as Arc<dyn EventSink>),
        Some(state.store.clone()),
    );

    let executor = WorkflowExecutor::new(
        model,
        state.store.clone(),
        event_sink.clone(),
        policy,
        registry,
        &state.workspace_root,
    )
    .with_author_id("agent-worker");

    let outcome_result = executor.resume_workflow(&spec, scope).await;
    handle_outcome(state, scope, event_sink, outcome_result);
}

fn handle_outcome(
    state: AppState,
    scope: ExecutionScope,
    event_sink: Arc<BroadcastEventSink>,
    outcome_result: Result<WorkflowOutcome, crate::workflow::engine::WorkflowExecutionError>,
) {
    let run_id_str = scope.run_id.to_string();
    let now = Utc::now().to_rfc3339();

    match outcome_result {
        Ok(WorkflowOutcome::Completed(output)) => {
            let out_str = output.to_string();
            if let Err(e) =
                state
                    .store
                    .update_run_status(&run_id_str, "SUCCEEDED", Some(&out_str), Some(&now))
            {
                eprintln!("Failed to update run status: {}", e);
            }
        }
        Ok(WorkflowOutcome::AwaitingApproval { .. }) => {
            if let Err(e) =
                state
                    .store
                    .update_run_status(&run_id_str, "AWAITING_APPROVAL", None, None)
            {
                eprintln!("Failed to update run status: {}", e);
            }
        }
        Ok(WorkflowOutcome::Paused { node_id }) => {
            if let Err(e) = state
                .store
                .update_run_status(&run_id_str, "PAUSED", None, None)
            {
                eprintln!("Failed to update run status: {}", e);
            }
            let pause_event = RuntimeEvent {
                id: Uuid::new_v4(),
                scope,
                node_id: Some(node_id),
                event_type: RuntimeEventType::FlowStateChanged,
                occurred_at: Utc::now(),
                payload: serde_json::json!({
                    "status": "paused",
                    "run_id": run_id_str,
                }),
            };
            if let Err(e) = event_sink.emit(pause_event) {
                eprintln!("Failed to emit pause event: {}", e);
            }
        }
        Ok(WorkflowOutcome::Failed(err)) => {
            if let Err(e) = state
                .store
                .update_run_status(&run_id_str, "FAILED", None, Some(&now))
            {
                eprintln!("Failed to update run status: {}", e);
            }
            let fail_event = RuntimeEvent {
                id: Uuid::new_v4(),
                scope,
                node_id: None,
                event_type: RuntimeEventType::RunFailed,
                occurred_at: Utc::now(),
                payload: serde_json::json!({
                    "run_id": run_id_str,
                    "message": err,
                }),
            };
            if let Err(e) = event_sink.emit(fail_event) {
                eprintln!("Failed to emit fail event: {}", e);
            }
        }
        Ok(WorkflowOutcome::Cancelled) => {
            if let Err(e) =
                state
                    .store
                    .update_run_status(&run_id_str, "CANCELLED", None, Some(&now))
            {
                eprintln!("Failed to update run status: {}", e);
            }
            let cancel_event = RuntimeEvent {
                id: Uuid::new_v4(),
                scope,
                node_id: None,
                event_type: RuntimeEventType::FlowStateChanged,
                occurred_at: Utc::now(),
                payload: serde_json::json!({
                    "status": "cancelled",
                    "run_id": run_id_str,
                }),
            };
            if let Err(e) = event_sink.emit(cancel_event) {
                eprintln!("Failed to emit cancel event: {}", e);
            }
        }
        Err(err) => {
            if let Err(e) = state
                .store
                .update_run_status(&run_id_str, "FAILED", None, Some(&now))
            {
                eprintln!("Failed to update run status: {}", e);
            }
            let fail_event = RuntimeEvent {
                id: Uuid::new_v4(),
                scope,
                node_id: None,
                event_type: RuntimeEventType::RunFailed,
                occurred_at: Utc::now(),
                payload: serde_json::json!({
                    "run_id": run_id_str,
                    "message": err.to_string(),
                }),
            };
            if let Err(e) = event_sink.emit(fail_event) {
                eprintln!("Failed to emit fail event: {}", e);
            }
        }
    }
}
