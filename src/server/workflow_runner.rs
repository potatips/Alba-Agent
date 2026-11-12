use chrono::Utc;
use serde_json::Value;
use std::sync::Arc;
use uuid::Uuid;

use crate::domain::ExecutionScope;
use crate::events::sink::EventSink;
use crate::events::types::{RuntimeEvent, RuntimeEventType};
use crate::governance::DefaultApprovalPolicy;
use crate::models::types::{ModelConfig, ModelError, ModelRequest, ModelResponse};
use crate::models::{ModelClient, OpenAiCompatibleClient};
use crate::schema::WorkflowSpec;
use crate::server::state::AppState;
use crate::tools::register_workspace_tools;
use crate::tools::types::ToolRegistry;
use crate::workflow::engine::{WorkflowExecutor, WorkflowOutcome};

#[derive(Clone)]
pub struct BroadcastEventSink {
    tx: tokio::sync::broadcast::Sender<RuntimeEvent>,
}

impl BroadcastEventSink {
    pub fn new(tx: tokio::sync::broadcast::Sender<RuntimeEvent>) -> Self {
        Self { tx }
    }
}

impl EventSink for BroadcastEventSink {
    fn emit(&self, event: RuntimeEvent) -> Result<(), crate::events::EventError> {
        let _ = self.tx.send(event);
        Ok(())
    }
}

#[derive(Clone)]
pub struct ServerModelClient {
    openai_client: Option<Arc<OpenAiCompatibleClient>>,
}

impl ServerModelClient {
    pub fn new() -> Self {
        let api_key = std::env::var("NEXUS_API_KEY").unwrap_or_default();
        let mock_mode = std::env::var("NEXUS_MOCK_MODEL").unwrap_or_default() == "true";
        if !api_key.trim().is_empty() && !mock_mode {
            let api_base = std::env::var("NEXUS_MODEL_BASE_URL")
                .unwrap_or_else(|_| "https://api.openai.com/v1".to_string());
            let model = std::env::var("NEXUS_MODEL").unwrap_or_else(|_| "gpt-4o-mini".to_string());
            let client = OpenAiCompatibleClient::new(ModelConfig::new(api_base, api_key, model))
                .ok()
                .map(Arc::new);
            Self {
                openai_client: client,
            }
        } else {
            Self {
                openai_client: None,
            }
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
        if let Some(ref client) = self.openai_client {
            client.complete(request).await
        } else {
            let content = "Task executed successfully by agent in workflow.";
            Ok(ModelResponse {
                content: Some(content.to_string()),
                tool_calls: vec![],
                usage: None,
            })
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
    let _ = state
        .store
        .update_run_status(&run_id_str, "RUNNING", None, None);

    let event_sink = Arc::new(BroadcastEventSink::new(state.events_tx.clone()));
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
    let _ = event_sink.emit(start_event);

    let model = Arc::new(ServerModelClient::new());
    let policy = Arc::new(DefaultApprovalPolicy::new().with_workspace_root(&state.workspace_root));
    let mut registry = ToolRegistry::new();
    register_workspace_tools(
        &mut registry,
        &state.workspace_root,
        Some(Arc::clone(&event_sink) as Arc<dyn EventSink>),
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
    let _ = state
        .store
        .update_run_status(&run_id_str, "RUNNING", None, None);

    let event_sink = Arc::new(BroadcastEventSink::new(state.events_tx.clone()));
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
    let _ = event_sink.emit(resume_event);

    let model = Arc::new(ServerModelClient::new());
    let policy = Arc::new(DefaultApprovalPolicy::new().with_workspace_root(&state.workspace_root));
    let mut registry = ToolRegistry::new();
    register_workspace_tools(
        &mut registry,
        &state.workspace_root,
        Some(Arc::clone(&event_sink) as Arc<dyn EventSink>),
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
            let _ =
                state
                    .store
                    .update_run_status(&run_id_str, "SUCCEEDED", Some(&out_str), Some(&now));
            let comp_event = RuntimeEvent {
                id: Uuid::new_v4(),
                scope,
                node_id: None,
                event_type: RuntimeEventType::RunCompleted,
                occurred_at: Utc::now(),
                payload: serde_json::json!({
                    "run_id": run_id_str,
                    "status": "SUCCEEDED",
                    "output": output,
                }),
            };
            let _ = event_sink.emit(comp_event);
        }
        Ok(WorkflowOutcome::AwaitingApproval {
            node_id,
            approval_id,
            action_description,
        }) => {
            let _ = state
                .store
                .update_run_status(&run_id_str, "AWAITING_APPROVAL", None, None);
            let app_event = RuntimeEvent {
                id: Uuid::new_v4(),
                scope,
                node_id: Some(node_id),
                event_type: RuntimeEventType::ApprovalRequired,
                occurred_at: Utc::now(),
                payload: serde_json::json!({
                    "approval_id": approval_id.to_string(),
                    "action": action_description,
                }),
            };
            let _ = event_sink.emit(app_event);
        }
        Ok(WorkflowOutcome::Paused { node_id }) => {
            let _ = state
                .store
                .update_run_status(&run_id_str, "PAUSED", None, None);
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
            let _ = event_sink.emit(pause_event);
        }
        Ok(WorkflowOutcome::Failed(err)) => {
            let _ = state
                .store
                .update_run_status(&run_id_str, "FAILED", None, Some(&now));
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
            let _ = event_sink.emit(fail_event);
        }
        Ok(WorkflowOutcome::Cancelled) => {
            let _ = state
                .store
                .update_run_status(&run_id_str, "CANCELLED", None, Some(&now));
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
            let _ = event_sink.emit(cancel_event);
        }
        Err(err) => {
            let _ = state
                .store
                .update_run_status(&run_id_str, "FAILED", None, Some(&now));
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
            let _ = event_sink.emit(fail_event);
        }
    }
}
