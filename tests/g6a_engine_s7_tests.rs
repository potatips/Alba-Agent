use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use axum::extract::State as AxumState;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::post;
use axum::{Json, Router};
use chrono::Utc;
use nexus_albaai::checkpoints::{CheckpointStore, SqliteCheckpointStore};
use nexus_albaai::credentials::{CredentialStore, MemoryCredentialStore};
use nexus_albaai::domain::{ChatMessage, ExecutionScope};
use nexus_albaai::governance::DefaultApprovalPolicy;
use nexus_albaai::models::types::{ModelError, ModelRequest};
use nexus_albaai::models::ModelClient;
use nexus_albaai::profiles::{GoogleAccountRecord, ProviderConnection};
use nexus_albaai::schema::agent::AgentSpec;
use nexus_albaai::schema::node::{
    ConsensusConfigSpec, CrewNodeSpec, CrewProcess, EndNodeSpec, JoinMode, StartNodeSpec,
    WorkflowNode,
};
use nexus_albaai::schema::task::TaskSpec;
use nexus_albaai::schema::workflow::{WorkflowEdgeSpec, WorkflowModesSpec, WorkflowSpec};
use nexus_albaai::server::workflow_runner::ServerModelClient;
use nexus_albaai::tools::types::ToolRegistry;
use nexus_albaai::workflow::engine::{WorkflowExecutor, WorkflowOutcome};
use serde_json::{json, Value};
use tokio::net::TcpListener;
use uuid::Uuid;

#[derive(Clone)]
struct MockState {
    request_count: Arc<AtomicUsize>,
}

async fn mock_openai_handler(
    AxumState(state): AxumState<MockState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> (StatusCode, Json<Value>) {
    state.request_count.fetch_add(1, Ordering::SeqCst);

    let auth = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    if !auth.starts_with("Bearer ") || auth.contains("invalid") {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({
                "error": {
                    "message": "Invalid API key",
                    "type": "invalid_request_error"
                }
            })),
        );
    }

    let model = body["model"].as_str().unwrap_or("gpt-4o");
    (
        StatusCode::OK,
        Json(json!({
            "id": "chatcmpl-mock-123",
            "object": "chat.completion",
            "created": 1700000000,
            "model": model,
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": "Engine integration test success"
                },
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": 15,
                "completion_tokens": 25,
                "total_tokens": 40
            }
        })),
    )
}

async fn start_mock_server() -> (String, Arc<AtomicUsize>) {
    let request_count = Arc::new(AtomicUsize::new(0));
    let state = MockState {
        request_count: request_count.clone(),
    };

    let app = Router::new()
        .route("/v1/chat/completions", post(mock_openai_handler))
        .with_state(state);

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    (format!("http://{}", addr), request_count)
}

fn create_test_workflow(wf_id: Uuid, tenant_id: Uuid, workspace_id: Uuid) -> WorkflowSpec {
    let agent = AgentSpec {
        id: "worker-agent".to_string(),
        role: "Researcher".to_string(),
        goal: "Analyze data".to_string(),
        backstory: "Expert analyzer".to_string(),
        model: "gpt-4o".to_string(),
        tool_ids: vec![],
        skill_ids: vec![],
        max_iterations: 3,
        temperature: 0.1,
        guardrails: vec![],
        knowledge_capsule_ids: vec![],
        tier: None,
    };
    let task = TaskSpec {
        id: "task-analyze".to_string(),
        agent_id: "worker-agent".to_string(),
        description: "Analyze the provided dataset".to_string(),
        expected_output: "Analysis complete".to_string(),
        output_schema: "text".to_string(),
        output_schema_json: None,
        context_task_ids: vec![],
        expected_artifact_paths: vec![],
    };

    WorkflowSpec {
        id: wf_id,
        tenant_id,
        workspace_id,
        name: "Test Engine Workflow".to_string(),
        version: 1,
        nodes: vec![
            WorkflowNode::Start(StartNodeSpec {
                id: "start".to_string(),
                label: "Start".to_string(),
                trigger_type: "manual".to_string(),
                input_schema: Value::Null,
                schedule: None,
            }),
            WorkflowNode::Crew(CrewNodeSpec {
                id: "crew-node".to_string(),
                label: "Crew Worker".to_string(),
                agents: vec![agent],
                tasks: vec![task],
                process: CrewProcess::Sequential,
                consensus_config: ConsensusConfigSpec::default(),
                manager_agent_id: None,
                join: JoinMode::Any,
                expected_artifact_paths: vec![],
                requires_visual_evidence: false,
                verification_command: None,
            }),
            WorkflowNode::End(EndNodeSpec {
                id: "end".to_string(),
                label: "End".to_string(),
            }),
        ],
        edges: vec![
            WorkflowEdgeSpec {
                source: "start".to_string(),
                target: "crew-node".to_string(),
                route: None,
                loop_back: false,
            },
            WorkflowEdgeSpec {
                source: "crew-node".to_string(),
                target: "end".to_string(),
                route: None,
                loop_back: false,
            },
        ],
        learning_edges: vec![],
        layout: BTreeMap::new(),
        modes: WorkflowModesSpec::default(),
    }
}

#[tokio::test]
async fn test_engine_run_with_active_profile_provider_succeeds() {
    let (mock_url, req_count) = start_mock_server().await;
    let temp_db = tempfile::NamedTempFile::new().unwrap();
    let store = Arc::new(SqliteCheckpointStore::new(temp_db.path()).unwrap());

    let credentials = Arc::new(MemoryCredentialStore::default());

    let alice_sub = "google-sub-alice-1";
    let tenant_id = Uuid::new_v4();
    let workspace_id = Uuid::new_v4();
    let run_id = Uuid::new_v4();
    let scope = ExecutionScope::new(tenant_id, workspace_id, run_id).unwrap();

    let now = Utc::now().timestamp();
    let acct = GoogleAccountRecord {
        sub: alice_sub.to_string(),
        email: "alice@example.com".to_string(),
        display_name: "Alice".to_string(),
        picture: None,
        tenant_id: tenant_id.to_string(),
        workspace_id: workspace_id.to_string(),
        active: true,
        created_at: now,
        updated_at: now,
    };
    store.upsert_google_account(&acct).unwrap();

    let conn_id = "conn-alice-openai";
    let provider_conn = ProviderConnection {
        id: conn_id.to_string(),
        profile_id: alice_sub.to_string(),
        provider: "OpenAI-compatible".to_string(),
        preset_id: "custom_openai".to_string(),
        auth_type: "api_key".to_string(),
        account_label: Some("Alice Work".to_string()),
        status: "connected".to_string(),
        base_url: Some(format!("{}/v1", mock_url)),
        is_default: true,
        available_models: vec!["gpt-4o".to_string()],
        default_models: json!({ "chat": "gpt-4o" }),
        settings: json!({}),
        masked_key: Some("sk-...alice".to_string()),
        safe_error: None,
        created_at: 1000,
        updated_at: 1000,
    };
    store
        .create_or_update_provider_connection(&provider_conn)
        .unwrap();

    let secret_key = format!("account:{}:provider:{}", alice_sub, conn_id);
    let raw_secret = "secret-alice-key-999";
    credentials.set_secret(&secret_key, raw_secret).unwrap();

    let client = ServerModelClient::for_account(
        store.clone(),
        credentials.clone(),
        alice_sub.to_string(),
        Some(scope),
    );

    let req = ModelRequest {
        messages: vec![ChatMessage::user("Hello engine")],
        tools: vec![],
        temperature: None,
        max_tokens: None,
        model_override: Some("gpt-4o".to_string()),
    };

    let resp = client.complete(&req).await.unwrap();
    assert_eq!(
        resp.content.as_deref(),
        Some("Engine integration test success")
    );
    assert_eq!(req_count.load(Ordering::SeqCst), 1);

    let logs = store.list_audit_logs(&scope).await.unwrap();
    let completion_log = logs
        .iter()
        .find(|l| l.action == "model.completion")
        .expect("model.completion audit log must exist");

    assert_eq!(completion_log.actor_id.as_deref(), Some(alice_sub));
    assert_eq!(completion_log.details["provider_id"], conn_id);
    assert_eq!(completion_log.details["model"], "gpt-4o");
    assert_eq!(completion_log.details["prompt_tokens"], 15);
    assert_eq!(completion_log.details["completion_tokens"], 25);

    let details_str = serde_json::to_string(&completion_log.details).unwrap();
    assert!(
        !details_str.contains(raw_secret),
        "Audit details must NEVER leak secrets"
    );
}

#[tokio::test]
async fn test_engine_run_missing_or_revoked_credentials_fails_closed() {
    let temp_db = tempfile::NamedTempFile::new().unwrap();
    let store = Arc::new(SqliteCheckpointStore::new(temp_db.path()).unwrap());

    let credentials = Arc::new(MemoryCredentialStore::default());

    let alice_sub = "google-sub-alice-2";
    let tenant_id = Uuid::new_v4();
    let workspace_id = Uuid::new_v4();
    let run_id = Uuid::new_v4();
    let scope = ExecutionScope::new(tenant_id, workspace_id, run_id).unwrap();

    let now = Utc::now().timestamp();
    let acct = GoogleAccountRecord {
        sub: alice_sub.to_string(),
        email: "alice2@example.com".to_string(),
        display_name: "Alice 2".to_string(),
        picture: None,
        tenant_id: tenant_id.to_string(),
        workspace_id: workspace_id.to_string(),
        active: true,
        created_at: now,
        updated_at: now,
    };
    store.upsert_google_account(&acct).unwrap();

    let conn_id = "conn-alice-revoked";
    let provider_conn = ProviderConnection {
        id: conn_id.to_string(),
        profile_id: alice_sub.to_string(),
        provider: "OpenAI-compatible".to_string(),
        preset_id: "custom_openai".to_string(),
        auth_type: "api_key".to_string(),
        account_label: None,
        status: "connected".to_string(),
        base_url: Some("http://127.0.0.1:9".to_string()),
        is_default: true,
        available_models: vec!["gpt-4o".to_string()],
        default_models: json!({ "chat": "gpt-4o" }),
        settings: json!({}),
        masked_key: Some("sk-...none".to_string()),
        safe_error: None,
        created_at: 1000,
        updated_at: 1000,
    };
    store
        .create_or_update_provider_connection(&provider_conn)
        .unwrap();

    let client = ServerModelClient::for_account(
        store.clone(),
        credentials.clone(),
        alice_sub.to_string(),
        Some(scope),
    );

    let req = ModelRequest {
        messages: vec![ChatMessage::user("Fail closed test")],
        tools: vec![],
        temperature: None,
        max_tokens: None,
        model_override: Some("gpt-4o".to_string()),
    };

    let err = client.complete(&req).await.unwrap_err();
    match err {
        ModelError::AuthenticationError(msg) => {
            assert!(msg.contains("credential missing or revoked"));
        }
        other => panic!("Expected AuthenticationError, got: {:?}", other),
    }

    let logs = store.list_audit_logs(&scope).await.unwrap();
    let fail_log = logs
        .iter()
        .find(|l| l.action == "model.completion_failed")
        .expect("model.completion_failed audit log must exist");

    assert_eq!(fail_log.actor_id.as_deref(), Some(alice_sub));
    assert_eq!(fail_log.details["error"], "credential missing or revoked");
}

#[tokio::test]
async fn test_engine_run_inactive_profile_fails_closed() {
    let temp_db = tempfile::NamedTempFile::new().unwrap();
    let store = Arc::new(SqliteCheckpointStore::new(temp_db.path()).unwrap());

    let credentials = Arc::new(MemoryCredentialStore::default());

    let alice_sub = "google-sub-alice-3";
    let tenant_id = Uuid::new_v4();
    let workspace_id = Uuid::new_v4();
    let run_id = Uuid::new_v4();
    let scope = ExecutionScope::new(tenant_id, workspace_id, run_id).unwrap();

    let now = Utc::now().timestamp();
    let acct = GoogleAccountRecord {
        sub: alice_sub.to_string(),
        email: "alice3@example.com".to_string(),
        display_name: "Alice 3".to_string(),
        picture: None,
        tenant_id: tenant_id.to_string(),
        workspace_id: workspace_id.to_string(),
        active: true,
        created_at: now,
        updated_at: now,
    };
    store.upsert_google_account(&acct).unwrap();

    store.deactivate_all_google_accounts().unwrap();

    let client = ServerModelClient::for_account(
        store.clone(),
        credentials.clone(),
        alice_sub.to_string(),
        Some(scope),
    );

    let req = ModelRequest {
        messages: vec![ChatMessage::user("Inactive profile test")],
        tools: vec![],
        temperature: None,
        max_tokens: None,
        model_override: None,
    };

    let err = client.complete(&req).await.unwrap_err();
    match err {
        ModelError::AuthenticationError(msg) => {
            assert!(msg.contains("inactive or not found"));
        }
        other => panic!("Expected AuthenticationError, got: {:?}", other),
    }
}

#[tokio::test]
async fn test_engine_run_cross_profile_isolation() {
    let temp_db = tempfile::NamedTempFile::new().unwrap();
    let store = Arc::new(SqliteCheckpointStore::new(temp_db.path()).unwrap());

    let credentials = Arc::new(MemoryCredentialStore::default());

    let alice_sub = "google-sub-alice-4";
    let bob_sub = "google-sub-bob-4";
    let tenant_alice = Uuid::new_v4();
    let workspace_alice = Uuid::new_v4();
    let tenant_bob = Uuid::new_v4();
    let workspace_bob = Uuid::new_v4();
    let bob_run_id = Uuid::new_v4();
    let bob_scope = ExecutionScope::new(tenant_bob, workspace_bob, bob_run_id).unwrap();

    let now = Utc::now().timestamp();
    let acct_alice = GoogleAccountRecord {
        sub: alice_sub.to_string(),
        email: "alice4@example.com".to_string(),
        display_name: "Alice 4".to_string(),
        picture: None,
        tenant_id: tenant_alice.to_string(),
        workspace_id: workspace_alice.to_string(),
        active: true,
        created_at: now,
        updated_at: now,
    };
    let acct_bob = GoogleAccountRecord {
        sub: bob_sub.to_string(),
        email: "bob4@example.com".to_string(),
        display_name: "Bob 4".to_string(),
        picture: None,
        tenant_id: tenant_bob.to_string(),
        workspace_id: workspace_bob.to_string(),
        active: true,
        created_at: now,
        updated_at: now,
    };

    store.upsert_google_account(&acct_alice).unwrap();
    store.upsert_google_account(&acct_bob).unwrap();

    let conn_alice = ProviderConnection {
        id: "conn-alice-isolated".to_string(),
        profile_id: alice_sub.to_string(),
        provider: "OpenAI-compatible".to_string(),
        preset_id: "custom_openai".to_string(),
        auth_type: "api_key".to_string(),
        account_label: None,
        status: "connected".to_string(),
        base_url: Some("http://127.0.0.1:8080/v1".to_string()),
        is_default: true,
        available_models: vec!["gpt-4o".to_string()],
        default_models: json!({ "chat": "gpt-4o" }),
        settings: json!({}),
        masked_key: Some("sk-...alice".to_string()),
        safe_error: None,
        created_at: 1000,
        updated_at: 1000,
    };
    store
        .create_or_update_provider_connection(&conn_alice)
        .unwrap();

    credentials
        .set_secret(
            &format!("account:{}:provider:conn-alice-isolated", alice_sub),
            "alice-secret-key",
        )
        .unwrap();

    let bob_client = ServerModelClient::for_account(
        store.clone(),
        credentials.clone(),
        bob_sub.to_string(),
        Some(bob_scope),
    );

    let req = ModelRequest {
        messages: vec![ChatMessage::user("Bob trying to use models")],
        tools: vec![],
        temperature: None,
        max_tokens: None,
        model_override: Some("gpt-4o".to_string()),
    };

    let err = bob_client.complete(&req).await.unwrap_err();
    match err {
        ModelError::ConfigurationError(msg) => {
            assert!(msg.contains("no provider connections configured for account"));
            assert!(msg.contains(bob_sub));
        }
        other => panic!("Expected ConfigurationError, got: {:?}", other),
    }
}

#[tokio::test]
async fn test_workflow_execution_end_to_end_with_scoped_model() {
    let (mock_url, req_count) = start_mock_server().await;
    let temp_db = tempfile::NamedTempFile::new().unwrap();
    let store = Arc::new(SqliteCheckpointStore::new(temp_db.path()).unwrap());

    let credentials = Arc::new(MemoryCredentialStore::default());

    let alice_sub = "google-sub-alice-e2e";
    let tenant_id = Uuid::new_v4();
    let workspace_id = Uuid::new_v4();
    let run_id = Uuid::new_v4();
    let scope = ExecutionScope::new(tenant_id, workspace_id, run_id).unwrap();

    let now = Utc::now().timestamp();
    let acct = GoogleAccountRecord {
        sub: alice_sub.to_string(),
        email: "alice.e2e@example.com".to_string(),
        display_name: "Alice E2E".to_string(),
        picture: None,
        tenant_id: tenant_id.to_string(),
        workspace_id: workspace_id.to_string(),
        active: true,
        created_at: now,
        updated_at: now,
    };
    store.upsert_google_account(&acct).unwrap();

    let conn_id = "conn-alice-e2e";
    let provider_conn = ProviderConnection {
        id: conn_id.to_string(),
        profile_id: alice_sub.to_string(),
        provider: "OpenAI-compatible".to_string(),
        preset_id: "custom_openai".to_string(),
        auth_type: "api_key".to_string(),
        account_label: None,
        status: "connected".to_string(),
        base_url: Some(format!("{}/v1", mock_url)),
        is_default: true,
        available_models: vec!["gpt-4o".to_string()],
        default_models: json!({ "chat": "gpt-4o" }),
        settings: json!({}),
        masked_key: Some("sk-...e2e".to_string()),
        safe_error: None,
        created_at: 1000,
        updated_at: 1000,
    };
    store
        .create_or_update_provider_connection(&provider_conn)
        .unwrap();

    credentials
        .set_secret(
            &format!("account:{}:provider:{}", alice_sub, conn_id),
            "valid-e2e-secret-key",
        )
        .unwrap();

    let model = Arc::new(ServerModelClient::for_account(
        store.clone(),
        credentials.clone(),
        alice_sub.to_string(),
        Some(scope),
    ));

    let policy = Arc::new(DefaultApprovalPolicy::new());
    let registry = ToolRegistry::new();
    let temp_ws = tempfile::tempdir().unwrap();

    let (events_tx, _events_rx) = tokio::sync::broadcast::channel(16);
    let event_sink = Arc::new(
        nexus_albaai::server::workflow_runner::BroadcastEventSink::new(store.clone(), events_tx),
    );

    let executor = WorkflowExecutor::new(
        model,
        store.clone(),
        event_sink,
        policy,
        registry,
        temp_ws.path(),
    );

    let wf_id = Uuid::new_v4();
    let spec = create_test_workflow(wf_id, tenant_id, workspace_id);

    let outcome = executor.start_workflow(&spec, scope, None).await.unwrap();
    match outcome {
        WorkflowOutcome::Completed(_) => {}
        other => panic!("Expected Completed outcome, got: {:?}", other),
    }

    assert!(req_count.load(Ordering::SeqCst) >= 1);

    let logs = store.list_audit_logs(&scope).await.unwrap();
    let completion_log = logs
        .iter()
        .find(|l| l.action == "model.completion")
        .expect("model.completion audit log must exist");
    assert_eq!(completion_log.actor_id.as_deref(), Some(alice_sub));
    assert_eq!(completion_log.details["provider_id"], conn_id);

    let details_str = serde_json::to_string(&completion_log.details).unwrap();
    assert!(
        !details_str.contains("valid-e2e-secret-key"),
        "Secrets must not appear in audit log"
    );
}

#[tokio::test]
async fn test_sign_out_purges_credentials_and_engine_fails_closed() {
    let (mock_url, _req_count) = start_mock_server().await;
    let temp_db = tempfile::NamedTempFile::new().unwrap();
    let store = Arc::new(SqliteCheckpointStore::new(temp_db.path()).unwrap());

    let credentials = Arc::new(MemoryCredentialStore::default());

    let alice_sub = "google-sub-alice-signout";
    let tenant_id = Uuid::new_v4();
    let workspace_id = Uuid::new_v4();
    let run_id = Uuid::new_v4();
    let scope = ExecutionScope::new(tenant_id, workspace_id, run_id).unwrap();

    let now = Utc::now().timestamp();
    let acct = GoogleAccountRecord {
        sub: alice_sub.to_string(),
        email: "alice.signout@example.com".to_string(),
        display_name: "Alice Signout".to_string(),
        picture: None,
        tenant_id: tenant_id.to_string(),
        workspace_id: workspace_id.to_string(),
        active: true,
        created_at: now,
        updated_at: now,
    };
    store.upsert_google_account(&acct).unwrap();

    let conn_id = "conn-alice-signout";
    let provider_conn = ProviderConnection {
        id: conn_id.to_string(),
        profile_id: alice_sub.to_string(),
        provider: "OpenAI-compatible".to_string(),
        preset_id: "custom_openai".to_string(),
        auth_type: "api_key".to_string(),
        account_label: None,
        status: "connected".to_string(),
        base_url: Some(format!("{}/v1", mock_url)),
        is_default: true,
        available_models: vec!["gpt-4o".to_string()],
        default_models: json!({ "chat": "gpt-4o" }),
        settings: json!({}),
        masked_key: Some("sk-...so".to_string()),
        safe_error: None,
        created_at: 1000,
        updated_at: 1000,
    };
    store
        .create_or_update_provider_connection(&provider_conn)
        .unwrap();

    credentials
        .set_secret(
            &format!("account:{}:provider:{}", alice_sub, conn_id),
            "signout-secret-key",
        )
        .unwrap();

    let client = ServerModelClient::for_account(
        store.clone(),
        credentials.clone(),
        alice_sub.to_string(),
        Some(scope),
    );

    let req = ModelRequest {
        messages: vec![ChatMessage::user("Pre-signout message")],
        tools: vec![],
        temperature: None,
        max_tokens: None,
        model_override: Some("gpt-4o".to_string()),
    };

    let first_resp = client.complete(&req).await.unwrap();
    assert_eq!(
        first_resp.content.as_deref(),
        Some("Engine integration test success")
    );

    store
        .delete_all_provider_connections_for_profile(alice_sub)
        .unwrap();
    credentials.delete_profile_secrets(alice_sub).unwrap();
    store.deactivate_all_google_accounts().unwrap();

    let second_res = client.complete(&req).await;
    assert!(
        second_res.is_err(),
        "Model call must fail closed immediately after sign out"
    );
}
