use std::fs;
#[cfg(unix)]
use std::os::unix::fs::{symlink, PermissionsExt};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::process::Command;
use tokio::sync::Barrier;
use tokio::task::JoinSet;

use nexus_albaai::agent::engine::{AgentEngine, EngineError};
use nexus_albaai::agent::state::EngineOutcome;
use nexus_albaai::checkpoints::sqlite::SqliteCheckpointStore;
use nexus_albaai::checkpoints::store::{CheckpointError, CheckpointStore};
use nexus_albaai::domain::{ApprovalDecision, ApprovalStatus, ExecutionScope, RunStatus};
use nexus_albaai::events::sink::{InMemoryEventSink, JsonLinesEventSink};
use nexus_albaai::events::types::RuntimeEventType;
use nexus_albaai::governance::policy::DefaultApprovalPolicy;
use nexus_albaai::models::adapter::{ModelClient, OpenAiCompatibleClient};
use nexus_albaai::models::types::{ModelConfig, ModelError, ModelRequest};
use nexus_albaai::tools::file_read::ReadFileTool;
use nexus_albaai::tools::types::{BoxFuture, Tool, ToolDefinition, ToolError, ToolRegistry};
use tempfile::tempdir;
use uuid::Uuid;

mod common;
use common::{MockResponse, TestHttpServer};

fn make_scope() -> ExecutionScope {
    ExecutionScope::new(Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4()).unwrap()
}

#[tokio::test]
async fn test_model_direct_final_response() {
    let server = TestHttpServer::start().await;
    server.enqueue_response(MockResponse::final_answer(
        "The capital of France is Paris.",
    ));

    let db_dir = tempdir().unwrap();
    let db_path = db_dir.path().join("test.db");
    let store = Arc::new(SqliteCheckpointStore::new(&db_path).unwrap());
    let events = Arc::new(InMemoryEventSink::new());
    let policy = Arc::new(DefaultApprovalPolicy::new());

    let ws_dir = tempdir().unwrap();
    let mut registry = ToolRegistry::new();
    registry.register(Arc::new(ReadFileTool::new(ws_dir.path())));

    let model_config = ModelConfig::new(server.url(), "dummy_key", "test_model");
    let client = Arc::new(OpenAiCompatibleClient::new(model_config).unwrap());

    let engine = AgentEngine::new(client, store.clone(), events.clone(), policy, registry);
    let scope = make_scope();

    let outcome = engine
        .start_run(scope, "What is the capital of France?")
        .await
        .unwrap();

    assert_eq!(
        outcome,
        EngineOutcome::Completed("The capital of France is Paris.".to_string())
    );

    let latest = store.get_latest_checkpoint(&scope).await.unwrap().unwrap();
    assert_eq!(latest.status, RunStatus::Completed);
    assert_eq!(latest.sequence, 2);
}

#[tokio::test]
async fn test_authorized_tool_execution() {
    let server = TestHttpServer::start().await;
    server.enqueue_response(MockResponse::tool_call(
        "call_1",
        "read_file",
        serde_json::json!({ "path": "sensor.txt" }),
    ));
    server.enqueue_response(MockResponse::final_answer("Sensor reading is 42 degrees."));

    let ws_dir = tempdir().unwrap();
    let file_path = ws_dir.path().join("sensor.txt");
    fs::write(&file_path, "42 degrees").unwrap();

    let db_dir = tempdir().unwrap();
    let db_path = db_dir.path().join("test.db");
    let store = Arc::new(SqliteCheckpointStore::new(&db_path).unwrap());
    let events = Arc::new(InMemoryEventSink::new());
    let policy = Arc::new(DefaultApprovalPolicy::new());

    let mut registry = ToolRegistry::new();
    registry.register(Arc::new(ReadFileTool::new(ws_dir.path())));

    let model_config = ModelConfig::new(server.url(), "dummy_key", "test_model");
    let client = Arc::new(OpenAiCompatibleClient::new(model_config).unwrap());

    let engine = AgentEngine::new(client, store.clone(), events.clone(), policy, registry);
    let scope = make_scope();

    let outcome = engine
        .start_run(scope, "Read sensor.txt and report temperature")
        .await
        .unwrap();

    assert_eq!(
        outcome,
        EngineOutcome::Completed("Sensor reading is 42 degrees.".to_string())
    );

    let reqs = server.received_requests();
    assert_eq!(reqs.len(), 2);
    assert!(reqs[1].contains("42 degrees"));

    let latest = store.get_latest_checkpoint(&scope).await.unwrap().unwrap();
    assert_eq!(latest.status, RunStatus::Completed);
    assert_eq!(latest.sequence, 3);
}

#[tokio::test]
async fn test_tool_requiring_approval_pauses_execution() {
    let server = TestHttpServer::start().await;
    server.enqueue_response(MockResponse::tool_call(
        "call_sec",
        "read_file",
        serde_json::json!({ "path": "sensitive/config.json" }),
    ));

    let ws_dir = tempdir().unwrap();
    let sensitive_dir = ws_dir.path().join("sensitive");
    fs::create_dir(&sensitive_dir).unwrap();
    fs::write(
        sensitive_dir.join("config.json"),
        "{\"db\": \"prod_secret_99\"}",
    )
    .unwrap();

    let db_dir = tempdir().unwrap();
    let db_path = db_dir.path().join("test.db");
    let store = Arc::new(SqliteCheckpointStore::new(&db_path).unwrap());
    let events = Arc::new(InMemoryEventSink::new());

    let policy = Arc::new(DefaultApprovalPolicy::new().with_sensitive_path_prefix("sensitive/"));

    let mut registry = ToolRegistry::new();
    registry.register(Arc::new(ReadFileTool::new(ws_dir.path())));

    let model_config = ModelConfig::new(server.url(), "dummy_key", "test_model");
    let client = Arc::new(OpenAiCompatibleClient::new(model_config).unwrap());

    let engine = AgentEngine::new(client, store.clone(), events.clone(), policy, registry);
    let scope = make_scope();

    let outcome = engine
        .start_run(scope, "Read sensitive/config.json")
        .await
        .unwrap();

    let (approval_id, tool_name) = match outcome {
        EngineOutcome::Paused {
            approval_id,
            tool_name,
            ..
        } => (approval_id, tool_name),
        other => panic!("Expected Paused outcome, got {:?}", other),
    };

    assert_eq!(tool_name, "read_file");

    let emitted = events.events();
    assert!(emitted
        .iter()
        .all(|e| e.event_type != RuntimeEventType::ToolInvoked));
    assert!(emitted
        .iter()
        .any(|e| e.event_type == RuntimeEventType::ApprovalRequired));

    let latest = store.get_latest_checkpoint(&scope).await.unwrap().unwrap();
    assert_eq!(latest.status, RunStatus::Paused);
    assert_eq!(latest.pending_approval_id, Some(approval_id));
    assert!(latest
        .history
        .iter()
        .all(|m| !m.content.contains("prod_secret_99")));

    let approval = store
        .get_approval(&scope, approval_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(approval.status, ApprovalStatus::Pending);
}

#[tokio::test]
async fn test_denial_prevents_resource_access() {
    let server = TestHttpServer::start().await;
    server.enqueue_response(MockResponse::tool_call(
        "call_sec",
        "read_file",
        serde_json::json!({ "path": "restricted/data.txt" }),
    ));

    let ws_dir = tempdir().unwrap();
    let res_dir = ws_dir.path().join("restricted");
    fs::create_dir(&res_dir).unwrap();
    fs::write(res_dir.join("data.txt"), "secret payload 12345").unwrap();

    let db_dir = tempdir().unwrap();
    let db_path = db_dir.path().join("test.db");
    let store = Arc::new(SqliteCheckpointStore::new(&db_path).unwrap());
    let events = Arc::new(InMemoryEventSink::new());

    let policy = Arc::new(DefaultApprovalPolicy::new().with_sensitive_path_prefix("restricted/"));

    let mut registry = ToolRegistry::new();
    registry.register(Arc::new(ReadFileTool::new(ws_dir.path())));

    let model_config = ModelConfig::new(server.url(), "dummy_key", "test_model");
    let client = Arc::new(OpenAiCompatibleClient::new(model_config).unwrap());

    let engine = AgentEngine::new(client, store.clone(), events.clone(), policy, registry);
    let scope = make_scope();

    let outcome = engine
        .start_run(scope, "Read restricted/data.txt")
        .await
        .unwrap();

    let approval_id = match outcome {
        EngineOutcome::Paused { approval_id, .. } => approval_id,
        other => panic!("Expected Paused outcome, got {:?}", other),
    };

    store
        .resolve_approval(&scope, approval_id, ApprovalDecision::Deny, "test-operator")
        .await
        .unwrap();

    let resume_outcome = engine.resume_run(scope).await.unwrap();
    assert_eq!(
        resume_outcome,
        EngineOutcome::Failed("Approval denied by human operator".to_string())
    );

    let emitted = events.events();
    assert!(emitted
        .iter()
        .all(|e| e.event_type != RuntimeEventType::ToolInvoked));

    let latest = store.get_latest_checkpoint(&scope).await.unwrap().unwrap();
    assert_eq!(latest.status, RunStatus::Failed);
    assert!(latest
        .history
        .iter()
        .all(|m| !m.content.contains("secret payload 12345")));
}

#[tokio::test]
async fn test_allow_once_executes_and_resumes_to_final_response() {
    let server = TestHttpServer::start().await;
    server.enqueue_response(MockResponse::tool_call(
        "call_app",
        "read_file",
        serde_json::json!({ "path": "guard/report.txt" }),
    ));
    server.enqueue_response(MockResponse::final_answer(
        "The report content is verified.",
    ));

    let ws_dir = tempdir().unwrap();
    let guard_dir = ws_dir.path().join("guard");
    fs::create_dir(&guard_dir).unwrap();
    fs::write(guard_dir.join("report.txt"), "Q4 Audit passed").unwrap();

    let db_dir = tempdir().unwrap();
    let db_path = db_dir.path().join("test.db");
    let store = Arc::new(SqliteCheckpointStore::new(&db_path).unwrap());
    let events = Arc::new(InMemoryEventSink::new());

    let policy = Arc::new(DefaultApprovalPolicy::new().with_sensitive_path_prefix("guard/"));

    let mut registry = ToolRegistry::new();
    registry.register(Arc::new(ReadFileTool::new(ws_dir.path())));

    let model_config = ModelConfig::new(server.url(), "dummy_key", "test_model");
    let client = Arc::new(OpenAiCompatibleClient::new(model_config).unwrap());

    let engine = AgentEngine::new(client, store.clone(), events.clone(), policy, registry);
    let scope = make_scope();

    let outcome = engine
        .start_run(scope, "Read guard/report.txt")
        .await
        .unwrap();

    let approval_id = match outcome {
        EngineOutcome::Paused { approval_id, .. } => approval_id,
        other => panic!("Expected Paused, got {:?}", other),
    };

    store
        .resolve_approval(
            &scope,
            approval_id,
            ApprovalDecision::AllowOnce,
            "test-operator",
        )
        .await
        .unwrap();

    let resume_outcome = engine.resume_run(scope).await.unwrap();
    assert_eq!(
        resume_outcome,
        EngineOutcome::Completed("The report content is verified.".to_string())
    );

    let approval = store
        .get_approval(&scope, approval_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(approval.status, ApprovalStatus::Consumed);
    assert!(approval.consumed_at.is_some());
}

#[tokio::test]
async fn test_allow_once_cannot_be_reused() {
    let store = SqliteCheckpointStore::in_memory().unwrap();
    let scope = make_scope();
    let approval_id = Uuid::new_v4();

    let approval = nexus_albaai::domain::ApprovalRequest {
        id: approval_id,
        scope,
        tool_name: "test_tool".to_string(),
        action_description: "test action".to_string(),
        intent: nexus_albaai::domain::ActionIntentKind::ExternalAction,
        status: ApprovalStatus::Pending,
        author_id: None,
        created_at: chrono::Utc::now(),
        decided_by: None,
        decided_at: None,
        consumed_at: None,
    };

    store.save_approval(&approval).await.unwrap();

    store
        .resolve_approval(
            &scope,
            approval_id,
            ApprovalDecision::AllowOnce,
            "test-operator",
        )
        .await
        .unwrap();

    let first_consume = store.consume_approval(&scope, approval_id).await.unwrap();
    assert!(first_consume);

    let second_consume = store.consume_approval(&scope, approval_id).await.unwrap();
    assert!(!second_consume);
}

#[tokio::test]
async fn test_restart_and_resume_preserves_state() {
    let server = TestHttpServer::start().await;
    server.enqueue_response(MockResponse::tool_call(
        "call_restart",
        "read_file",
        serde_json::json!({ "path": "prod/data.txt" }),
    ));
    server.enqueue_response(MockResponse::final_answer(
        "Final answer after crash restart.",
    ));

    let ws_dir = tempdir().unwrap();
    let prod_dir = ws_dir.path().join("prod");
    fs::create_dir(&prod_dir).unwrap();
    fs::write(prod_dir.join("data.txt"), "Important production metric").unwrap();

    let db_dir = tempdir().unwrap();
    let db_path = db_dir.path().join("test.db");
    let scope = make_scope();
    let approval_id;

    {
        let store = Arc::new(SqliteCheckpointStore::new(&db_path).unwrap());
        let events = Arc::new(InMemoryEventSink::new());
        let policy = Arc::new(DefaultApprovalPolicy::new().with_sensitive_path_prefix("prod/"));
        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(ReadFileTool::new(ws_dir.path())));
        let model_config = ModelConfig::new(server.url(), "dummy_key", "test_model");
        let client = Arc::new(OpenAiCompatibleClient::new(model_config).unwrap());

        let engine = AgentEngine::new(client, store.clone(), events, policy, registry);
        let outcome = engine.start_run(scope, "Read prod/data.txt").await.unwrap();

        approval_id = match outcome {
            EngineOutcome::Paused { approval_id, .. } => approval_id,
            other => panic!("Expected Paused, got {:?}", other),
        };
    }

    {
        let store = SqliteCheckpointStore::new(&db_path).unwrap();
        store
            .resolve_approval(
                &scope,
                approval_id,
                ApprovalDecision::AllowOnce,
                "test-operator",
            )
            .await
            .unwrap();
    }

    {
        let store = Arc::new(SqliteCheckpointStore::new(&db_path).unwrap());
        let events = Arc::new(InMemoryEventSink::new());
        let policy = Arc::new(DefaultApprovalPolicy::new());
        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(ReadFileTool::new(ws_dir.path())));
        let model_config = ModelConfig::new(server.url(), "dummy_key", "test_model");
        let client = Arc::new(OpenAiCompatibleClient::new(model_config).unwrap());

        let engine = AgentEngine::new(client, store.clone(), events, policy, registry);
        let outcome = engine.resume_run(scope).await.unwrap();

        assert_eq!(
            outcome,
            EngineOutcome::Completed("Final answer after crash restart.".to_string())
        );

        let latest = store.get_latest_checkpoint(&scope).await.unwrap().unwrap();
        assert_eq!(latest.status, RunStatus::Completed);
    }
}

#[tokio::test]
async fn test_different_scope_cannot_inspect_or_resume() {
    let db_dir = tempdir().unwrap();
    let db_path = db_dir.path().join("test.db");
    let store = Arc::new(SqliteCheckpointStore::new(&db_path).unwrap());

    let scope_a = make_scope();
    let scope_b = make_scope();

    let cp = nexus_albaai::checkpoints::types::RunCheckpoint {
        id: Uuid::new_v4(),
        scope: scope_a,
        sequence: 1,
        status: RunStatus::Running,
        history: Vec::new(),
        active_tool_call: None,
        pending_tool_calls: Vec::new(),
        pending_approval_id: None,
        author_id: None,
        model_override: None,
        created_at: chrono::Utc::now(),
    };

    store.save_checkpoint(&cp).await.unwrap();

    let found_a = store.get_latest_checkpoint(&scope_a).await.unwrap();
    assert!(found_a.is_some());

    let found_b = store.get_latest_checkpoint(&scope_b).await.unwrap();
    assert!(found_b.is_none());
}

#[tokio::test]
async fn test_path_traversal_and_sensitive_file_protections() {
    let ws_dir = tempdir().unwrap();
    let tool = ReadFileTool::new(ws_dir.path());

    let outside_dir = tempdir().unwrap();
    let outside_file = outside_dir.path().join("escaped.txt");
    fs::write(&outside_file, "escaped data").unwrap();

    let err = tool.validate_and_resolve("../escaped.txt").unwrap_err();
    assert!(matches!(err, ToolError::PathTraversal(_)));

    let err = tool.validate_and_resolve("/etc/passwd").unwrap_err();
    assert!(matches!(err, ToolError::PathTraversal(_)));

    let err = tool
        .validate_and_resolve("a/../../escaped.txt")
        .unwrap_err();
    assert!(matches!(err, ToolError::PathTraversal(_)));

    let env_file = ws_dir.path().join(".env");
    fs::write(&env_file, "SECRET=123").unwrap();
    let err = tool.validate_and_resolve(".env").unwrap_err();
    assert!(matches!(err, ToolError::AccessDenied(_)));

    let env_local = ws_dir.path().join(".env.production");
    fs::write(&env_local, "KEY=456").unwrap();
    let err = tool.validate_and_resolve(".env.production").unwrap_err();
    assert!(matches!(err, ToolError::AccessDenied(_)));

    let ssh_dir = ws_dir.path().join(".ssh");
    fs::create_dir(&ssh_dir).unwrap();
    fs::write(ssh_dir.join("id_rsa"), "private key").unwrap();
    let err = tool.validate_and_resolve(".ssh/id_rsa").unwrap_err();
    assert!(matches!(err, ToolError::AccessDenied(_)));

    let symlink_path = ws_dir.path().join("symlink_outside");
    symlink(&outside_file, &symlink_path).unwrap();
    let err = tool.validate_and_resolve("symlink_outside").unwrap_err();
    assert!(matches!(err, ToolError::PathTraversal(_)));
}

#[tokio::test]
async fn test_cancellation_stops_execution() {
    let server = TestHttpServer::start().await;
    let ws_dir = tempdir().unwrap();
    let db_dir = tempdir().unwrap();
    let db_path = db_dir.path().join("test.db");
    let store = Arc::new(SqliteCheckpointStore::new(&db_path).unwrap());
    let events = Arc::new(InMemoryEventSink::new());
    let policy = Arc::new(DefaultApprovalPolicy::new());

    let mut registry = ToolRegistry::new();
    registry.register(Arc::new(ReadFileTool::new(ws_dir.path())));

    let model_config = ModelConfig::new(server.url(), "dummy_key", "test_model");
    let client = Arc::new(OpenAiCompatibleClient::new(model_config).unwrap());

    let engine = AgentEngine::new(client, store.clone(), events, policy, registry);
    let scope = make_scope();

    store.request_cancellation(&scope).await.unwrap();

    let outcome = engine.start_run(scope, "Hello").await.unwrap();
    assert_eq!(outcome, EngineOutcome::Cancelled);
}

#[tokio::test]
async fn test_execution_turn_limit() {
    let server = TestHttpServer::start().await;
    for _ in 0..15 {
        server.enqueue_response(MockResponse::tool_call(
            "loop_call",
            "read_file",
            serde_json::json!({ "path": "test.txt" }),
        ));
    }

    let ws_dir = tempdir().unwrap();
    fs::write(ws_dir.path().join("test.txt"), "ping").unwrap();

    let db_dir = tempdir().unwrap();
    let db_path = db_dir.path().join("test.db");
    let store = Arc::new(SqliteCheckpointStore::new(&db_path).unwrap());
    let events = Arc::new(InMemoryEventSink::new());
    let policy = Arc::new(DefaultApprovalPolicy::new());

    let mut registry = ToolRegistry::new();
    registry.register(Arc::new(ReadFileTool::new(ws_dir.path())));

    let model_config = ModelConfig::new(server.url(), "dummy_key", "test_model");
    let client = Arc::new(OpenAiCompatibleClient::new(model_config).unwrap());

    let engine =
        AgentEngine::new(client, store.clone(), events, policy, registry).with_max_turns(3);
    let scope = make_scope();

    let outcome = engine.start_run(scope, "Infinite loop").await.unwrap();
    assert!(matches!(outcome, EngineOutcome::Failed(_)));

    let latest = store.get_latest_checkpoint(&scope).await.unwrap().unwrap();
    assert_eq!(latest.status, RunStatus::Failed);
}

#[tokio::test]
async fn test_provider_error_handling() {
    let server = TestHttpServer::start().await;
    server.enqueue_response(MockResponse::error(500, "Internal Server Error"));

    let ws_dir = tempdir().unwrap();
    let db_dir = tempdir().unwrap();
    let db_path = db_dir.path().join("test.db");
    let store = Arc::new(SqliteCheckpointStore::new(&db_path).unwrap());
    let events = Arc::new(InMemoryEventSink::new());
    let policy = Arc::new(DefaultApprovalPolicy::new());

    let mut registry = ToolRegistry::new();
    registry.register(Arc::new(ReadFileTool::new(ws_dir.path())));

    let model_config = ModelConfig::new(server.url(), "dummy_key", "test_model");
    let client = Arc::new(OpenAiCompatibleClient::new(model_config).unwrap());

    let engine = AgentEngine::new(client, store.clone(), events, policy, registry);
    let scope = make_scope();

    let outcome = engine.start_run(scope, "Hello").await.unwrap();
    assert!(matches!(outcome, EngineOutcome::Failed(_)));
}

#[tokio::test]
async fn test_model_http_timeout() {
    let server = TestHttpServer::start().await;
    server.enqueue_response(MockResponse::final_answer("Too late").with_delay_ms(1500));

    let ws_dir = tempdir().unwrap();
    let db_dir = tempdir().unwrap();
    let db_path = db_dir.path().join("test.db");
    let store = Arc::new(SqliteCheckpointStore::new(&db_path).unwrap());
    let events = Arc::new(InMemoryEventSink::new());
    let policy = Arc::new(DefaultApprovalPolicy::new());

    let mut registry = ToolRegistry::new();
    registry.register(Arc::new(ReadFileTool::new(ws_dir.path())));

    let model_config = ModelConfig::new(server.url(), "dummy_key", "test_model").with_timeout(1);
    let client = Arc::new(OpenAiCompatibleClient::new(model_config).unwrap());

    let direct_req = ModelRequest {
        messages: vec![nexus_albaai::domain::ChatMessage::user("hi")],
        tools: Vec::new(),
        temperature: None,
        max_tokens: None,
        model_override: None,
    };
    let direct_res = client.complete(&direct_req).await;
    assert!(matches!(direct_res, Err(ModelError::Timeout(_))));

    server.enqueue_response(MockResponse::final_answer("Too late").with_delay_ms(1500));
    let engine = AgentEngine::new(client, store.clone(), events, policy, registry);
    let scope = make_scope();
    let outcome = engine.start_run(scope, "Hello").await.unwrap();
    assert!(matches!(outcome, EngineOutcome::Failed(ref msg) if msg.contains("timed out")));
}

#[tokio::test]
async fn test_model_auth_invalid_json_and_network_errors() {
    let server = TestHttpServer::start().await;
    server.enqueue_response(MockResponse::error(401, "Invalid API key"));
    server.enqueue_response(MockResponse::ok_json("{broken_json"));

    let req = ModelRequest {
        messages: vec![nexus_albaai::domain::ChatMessage::user("hi")],
        tools: Vec::new(),
        temperature: None,
        max_tokens: None,
        model_override: None,
    };

    let client = Arc::new(
        OpenAiCompatibleClient::new(ModelConfig::new(server.url(), "bad_key", "m")).unwrap(),
    );

    let err_auth = client.complete(&req).await;
    assert!(matches!(err_auth, Err(ModelError::AuthenticationError(_))));

    let err_json = client.complete(&req).await;
    assert!(matches!(err_json, Err(ModelError::InvalidResponse(_))));

    let unreachable_client = Arc::new(
        OpenAiCompatibleClient::new(ModelConfig::new("http://127.0.0.1:1", "k", "m")).unwrap(),
    );
    let err_net = unreachable_client.complete(&req).await;
    assert!(matches!(err_net, Err(ModelError::NetworkError(_))));
}

#[tokio::test]
async fn test_read_file_size_limit_enforced() {
    let ws_dir = tempdir().unwrap();
    let file_path = ws_dir.path().join("large.txt");
    fs::write(&file_path, "0123456789abcdef0123456789abcdef").unwrap();

    let tool = ReadFileTool::new(ws_dir.path()).with_max_bytes(16);
    let scope = make_scope();
    let args = serde_json::json!({ "path": "large.txt" });

    let err = tool.execute(&scope, &args).await.unwrap_err();
    assert!(matches!(
        err,
        ToolError::FileSizeExceeded {
            size: 32,
            limit: 16
        }
    ));
}

#[tokio::test]
async fn test_unassigned_and_malformed_tool_requests() {
    let server = TestHttpServer::start().await;
    server.enqueue_response(MockResponse::tool_call(
        "c1",
        "shell_exec",
        serde_json::json!({ "cmd": "id" }),
    ));
    server.enqueue_response(MockResponse::tool_call(
        "c2",
        "read_file",
        serde_json::json!({ "wrong_param": 123 }),
    ));
    server.enqueue_response(MockResponse::final_answer("Handled rejected tools."));

    let ws_dir = tempdir().unwrap();
    let db_dir = tempdir().unwrap();
    let db_path = db_dir.path().join("test.db");
    let store = Arc::new(SqliteCheckpointStore::new(&db_path).unwrap());
    let events = Arc::new(InMemoryEventSink::new());
    let policy = Arc::new(DefaultApprovalPolicy::new());

    let mut registry = ToolRegistry::new();
    registry.register(Arc::new(ReadFileTool::new(ws_dir.path())));

    let client = Arc::new(
        OpenAiCompatibleClient::new(ModelConfig::new(server.url(), "dummy_key", "m")).unwrap(),
    );
    let engine = AgentEngine::new(client, store, events, policy, registry);
    let scope = make_scope();

    let outcome = engine
        .start_run(scope, "Try unassigned and malformed tools")
        .await
        .unwrap();
    assert_eq!(
        outcome,
        EngineOutcome::Completed("Handled rejected tools.".to_string())
    );

    let reqs = server.received_requests();
    assert_eq!(reqs.len(), 3);
    assert!(reqs[1].contains("is not registered"));
    assert!(reqs[2].contains("Missing required parameter 'path'"));
}

#[tokio::test]
async fn test_different_scope_cannot_resume_or_approve_via_engine() {
    let server = TestHttpServer::start().await;
    server.enqueue_response(MockResponse::tool_call(
        "call_scope",
        "read_file",
        serde_json::json!({ "path": "guard/secret.txt" }),
    ));

    let ws_dir = tempdir().unwrap();
    let guard_dir = ws_dir.path().join("guard");
    fs::create_dir(&guard_dir).unwrap();
    fs::write(guard_dir.join("secret.txt"), "top_secret").unwrap();

    let db_dir = tempdir().unwrap();
    let db_path = db_dir.path().join("test.db");
    let store = Arc::new(SqliteCheckpointStore::new(&db_path).unwrap());
    let events = Arc::new(InMemoryEventSink::new());
    let policy = Arc::new(DefaultApprovalPolicy::new().with_sensitive_path_prefix("guard/"));

    let mut registry = ToolRegistry::new();
    registry.register(Arc::new(ReadFileTool::new(ws_dir.path())));

    let client = Arc::new(
        OpenAiCompatibleClient::new(ModelConfig::new(server.url(), "dummy_key", "m")).unwrap(),
    );
    let engine = AgentEngine::new(client, store.clone(), events, policy, registry);

    let scope_a = make_scope();
    let outcome = engine
        .start_run(scope_a, "Read guard/secret.txt")
        .await
        .unwrap();
    let approval_id = match outcome {
        EngineOutcome::Paused { approval_id, .. } => approval_id,
        other => panic!("Expected Paused, got {:?}", other),
    };

    let scope_diff_tenant =
        ExecutionScope::new(Uuid::new_v4(), scope_a.workspace_id, scope_a.run_id).unwrap();
    let scope_diff_ws =
        ExecutionScope::new(scope_a.tenant_id, Uuid::new_v4(), scope_a.run_id).unwrap();

    let err_resume_tenant = engine.resume_run(scope_diff_tenant).await.unwrap_err();
    assert!(matches!(err_resume_tenant, EngineError::InvalidState(_)));

    let err_resume_ws = engine.resume_run(scope_diff_ws).await.unwrap_err();
    assert!(matches!(err_resume_ws, EngineError::InvalidState(_)));

    let err_approve_tenant = store
        .resolve_approval(
            &scope_diff_tenant,
            approval_id,
            ApprovalDecision::AllowOnce,
            "test-operator",
        )
        .await
        .unwrap_err();
    assert!(matches!(
        err_approve_tenant,
        CheckpointError::InvalidState(_)
    ));

    let still_pending = store
        .get_approval(&scope_a, approval_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(still_pending.status, ApprovalStatus::Pending);
}

#[tokio::test]
async fn test_allow_once_resume_cannot_be_replayed() {
    let server = TestHttpServer::start().await;
    server.enqueue_response(MockResponse::tool_call(
        "call_once",
        "read_file",
        serde_json::json!({ "path": "once/data.txt" }),
    ));
    server.enqueue_response(MockResponse::final_answer("Read once successfully."));

    let ws_dir = tempdir().unwrap();
    let once_dir = ws_dir.path().join("once");
    fs::create_dir(&once_dir).unwrap();
    fs::write(once_dir.join("data.txt"), "one time payload").unwrap();

    let db_dir = tempdir().unwrap();
    let db_path = db_dir.path().join("test.db");
    let store = Arc::new(SqliteCheckpointStore::new(&db_path).unwrap());
    let events = Arc::new(InMemoryEventSink::new());
    let policy = Arc::new(DefaultApprovalPolicy::new().with_sensitive_path_prefix("once/"));

    let mut registry = ToolRegistry::new();
    registry.register(Arc::new(ReadFileTool::new(ws_dir.path())));

    let client = Arc::new(
        OpenAiCompatibleClient::new(ModelConfig::new(server.url(), "dummy_key", "m")).unwrap(),
    );
    let engine = AgentEngine::new(client, store.clone(), events, policy, registry);
    let scope = make_scope();

    let outcome = engine.start_run(scope, "Read once/data.txt").await.unwrap();
    let approval_id = match outcome {
        EngineOutcome::Paused { approval_id, .. } => approval_id,
        other => panic!("Expected Paused, got {:?}", other),
    };

    store
        .resolve_approval(
            &scope,
            approval_id,
            ApprovalDecision::AllowOnce,
            "test-operator",
        )
        .await
        .unwrap();

    let resumed = engine.resume_run(scope).await.unwrap();
    assert_eq!(
        resumed,
        EngineOutcome::Completed("Read once successfully.".to_string())
    );

    assert!(!store.consume_approval(&scope, approval_id).await.unwrap());
    assert_eq!(server.received_requests().len(), 2);
}

#[tokio::test]
async fn test_cancelled_paused_run_does_not_execute_on_resume() {
    let server = TestHttpServer::start().await;
    server.enqueue_response(MockResponse::tool_call(
        "call_cancel",
        "read_file",
        serde_json::json!({ "path": "cancel/data.txt" }),
    ));

    let ws_dir = tempdir().unwrap();
    let cancel_dir = ws_dir.path().join("cancel");
    fs::create_dir(&cancel_dir).unwrap();
    fs::write(cancel_dir.join("data.txt"), "must not read").unwrap();

    let db_dir = tempdir().unwrap();
    let db_path = db_dir.path().join("test.db");
    let store = Arc::new(SqliteCheckpointStore::new(&db_path).unwrap());
    let events = Arc::new(InMemoryEventSink::new());
    let policy = Arc::new(DefaultApprovalPolicy::new().with_sensitive_path_prefix("cancel/"));

    let mut registry = ToolRegistry::new();
    registry.register(Arc::new(ReadFileTool::new(ws_dir.path())));

    let client = Arc::new(
        OpenAiCompatibleClient::new(ModelConfig::new(server.url(), "dummy_key", "m")).unwrap(),
    );
    let engine = AgentEngine::new(client, store.clone(), events.clone(), policy, registry);
    let scope = make_scope();

    let outcome = engine
        .start_run(scope, "Read cancel/data.txt")
        .await
        .unwrap();
    let approval_id = match outcome {
        EngineOutcome::Paused { approval_id, .. } => approval_id,
        other => panic!("Expected Paused, got {:?}", other),
    };

    store
        .resolve_approval(
            &scope,
            approval_id,
            ApprovalDecision::AllowOnce,
            "test-operator",
        )
        .await
        .unwrap();

    store.request_cancellation(&scope).await.unwrap();

    let resumed = engine.resume_run(scope).await.unwrap();
    assert_eq!(resumed, EngineOutcome::Cancelled);

    let approval = store
        .get_approval(&scope, approval_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(approval.status, ApprovalStatus::AllowedOnce);

    let emitted = events.events();
    assert!(emitted
        .iter()
        .all(|e| e.event_type != RuntimeEventType::ToolInvoked));
}

#[tokio::test]
async fn test_cli_end_to_end_run_status_approve_resume_cancel() {
    let bin_path = env!("CARGO_BIN_EXE_nexus-albaai");
    let server = TestHttpServer::start().await;
    server.enqueue_response(MockResponse::tool_call(
        "call_cli",
        "read_file",
        serde_json::json!({ "path": "doc.txt" }),
    ));
    server.enqueue_response(MockResponse::final_answer("CLI completed run."));

    let ws_dir = tempdir().unwrap();
    fs::write(ws_dir.path().join("doc.txt"), "hello cli").unwrap();

    let db_dir = tempdir().unwrap();
    let db_path = db_dir.path().join("cli.db");
    let run_id = Uuid::new_v4().to_string();

    let run_out = Command::new(bin_path)
        .env("NEXUS_API_KEY", "cli_key")
        .args([
            "run",
            "--prompt",
            "Read doc.txt",
            "--run-id",
            &run_id,
            "--workspace-root",
            ws_dir.path().to_str().unwrap(),
            "--db-path",
            db_path.to_str().unwrap(),
            "--api-base",
            &server.url(),
            "--model",
            "test_model",
            "--require-approval",
        ])
        .output()
        .await
        .unwrap();
    assert!(
        run_out.status.success(),
        "run failed: {}",
        String::from_utf8_lossy(&run_out.stderr)
    );
    let run_stdout = String::from_utf8_lossy(&run_out.stdout);
    assert!(run_stdout.contains("Run paused for human approval."));

    let approval_line = run_stdout
        .lines()
        .find(|l| l.starts_with("Approval ID: "))
        .expect("missing Approval ID in CLI output");
    let approval_id = approval_line.trim_start_matches("Approval ID: ").trim();

    let status_out = Command::new(bin_path)
        .args([
            "status",
            "--run-id",
            &run_id,
            "--db-path",
            db_path.to_str().unwrap(),
        ])
        .output()
        .await
        .unwrap();
    assert!(status_out.status.success());
    let status_stdout = String::from_utf8_lossy(&status_out.stdout);
    assert!(status_stdout.contains("Status: paused"));
    assert!(status_stdout.contains("Approval status: pending"));

    let approve_out = Command::new(bin_path)
        .env("NEXUS_ACTOR_ID", "operator-cli")
        .args([
            "approve",
            "--run-id",
            &run_id,
            "--approval-id",
            approval_id,
            "--decision",
            "allow_once",
            "--db-path",
            db_path.to_str().unwrap(),
        ])
        .output()
        .await
        .unwrap();
    assert!(approve_out.status.success());

    let resume_out = Command::new(bin_path)
        .env("NEXUS_API_KEY", "cli_key")
        .args([
            "resume",
            "--run-id",
            &run_id,
            "--workspace-root",
            ws_dir.path().to_str().unwrap(),
            "--db-path",
            db_path.to_str().unwrap(),
            "--api-base",
            &server.url(),
            "--model",
            "test_model",
        ])
        .output()
        .await
        .unwrap();
    assert!(resume_out.status.success());
    let resume_stdout = String::from_utf8_lossy(&resume_out.stdout);
    assert!(resume_stdout.contains("CLI completed run."));

    server.enqueue_response(MockResponse::tool_call(
        "call_cli_2",
        "read_file",
        serde_json::json!({ "path": "doc.txt" }),
    ));
    let cancel_run_id = Uuid::new_v4().to_string();
    let run2_out = Command::new(bin_path)
        .env("NEXUS_API_KEY", "cli_key")
        .args([
            "run",
            "--prompt",
            "Read doc.txt again",
            "--run-id",
            &cancel_run_id,
            "--workspace-root",
            ws_dir.path().to_str().unwrap(),
            "--db-path",
            db_path.to_str().unwrap(),
            "--api-base",
            &server.url(),
            "--require-approval",
        ])
        .output()
        .await
        .unwrap();
    assert!(run2_out.status.success());

    let cancel_out = Command::new(bin_path)
        .args([
            "cancel",
            "--run-id",
            &cancel_run_id,
            "--db-path",
            db_path.to_str().unwrap(),
        ])
        .output()
        .await
        .unwrap();
    assert!(cancel_out.status.success());

    let status2_out = Command::new(bin_path)
        .args([
            "status",
            "--run-id",
            &cancel_run_id,
            "--db-path",
            db_path.to_str().unwrap(),
        ])
        .output()
        .await
        .unwrap();
    let status2_stdout = String::from_utf8_lossy(&status2_out.stdout);
    assert!(status2_stdout.contains("Cancellation requested: true"));
}

fn validate_event_against_schema(
    event_val: &serde_json::Value,
    schema: &serde_json::Value,
) -> Result<(), String> {
    let obj = event_val
        .as_object()
        .ok_or_else(|| "Event must be a JSON object".to_string())?;

    let props = schema
        .get("properties")
        .and_then(|v| v.as_object())
        .ok_or_else(|| "Schema missing properties".to_string())?;

    if schema.get("additionalProperties").and_then(|v| v.as_bool()) == Some(false) {
        for key in obj.keys() {
            if !props.contains_key(key) {
                return Err(format!("Unexpected top-level property '{}'", key));
            }
        }
    }

    if let Some(required) = schema.get("required").and_then(|v| v.as_array()) {
        for req in required {
            let req_name = req
                .as_str()
                .ok_or_else(|| "Invalid required entry".to_string())?;
            if !obj.contains_key(req_name) {
                return Err(format!("Missing required property '{}'", req_name));
            }
        }
    }

    let enum_vals: Vec<&str> = schema
        .pointer("/$defs/RuntimeEventType/enum")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "Schema missing RuntimeEventType enum".to_string())?
        .iter()
        .filter_map(|v| v.as_str())
        .collect();

    for (key, val) in obj {
        let prop_schema = props
            .get(key)
            .ok_or_else(|| format!("Property '{}' not found in schema", key))?;

        if let Some(ref_str) = prop_schema.get("$ref").and_then(|v| v.as_str()) {
            if ref_str == "#/$defs/RuntimeEventType" {
                let s = val
                    .as_str()
                    .ok_or_else(|| format!("Property '{}' must be string", key))?;
                if !enum_vals.contains(&s) {
                    return Err(format!(
                        "Event type '{}' not in allowed schema enum {:?}",
                        s, enum_vals
                    ));
                }
            }
            continue;
        }

        if prop_schema.get("anyOf").is_some() {
            if !(val.is_string() || val.is_null()) {
                return Err(format!("Property '{}' must be string or null", key));
            }
            continue;
        }

        match prop_schema.get("type").and_then(|v| v.as_str()) {
            Some("string") => {
                let s = val
                    .as_str()
                    .ok_or_else(|| format!("Property '{}' must be string", key))?;
                match prop_schema.get("format").and_then(|v| v.as_str()) {
                    Some("uuid") => {
                        Uuid::parse_str(s).map_err(|e| {
                            format!("Property '{}' is not a valid UUID: {}", key, e)
                        })?;
                    }
                    Some("date-time") => {
                        chrono::DateTime::parse_from_rfc3339(s).map_err(|e| {
                            format!("Property '{}' is not valid RFC3339 date-time: {}", key, e)
                        })?;
                    }
                    _ => {}
                }
            }
            Some("object") => {
                if !val.is_object() {
                    return Err(format!("Property '{}' must be JSON object", key));
                }
            }
            other => {
                return Err(format!("Unsupported schema type {:?} for '{}'", other, key));
            }
        }
    }

    Ok(())
}

#[tokio::test]
async fn test_jsonl_file_events_validate_against_schema() {
    let schema: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/runtime_event.schema.json")).unwrap();

    assert!(
        validate_event_against_schema(&serde_json::json!({"type": "RUN_COMPLETED"}), &schema)
            .is_err()
    );
    assert!(validate_event_against_schema(
        &serde_json::json!({
            "id": Uuid::new_v4().to_string(),
            "tenant_id": Uuid::new_v4().to_string(),
            "workspace_id": Uuid::new_v4().to_string(),
            "run_id": Uuid::new_v4().to_string(),
            "type": "CREW_AGENT_THOUGHT",
            "occurred_at": chrono::Utc::now().to_rfc3339(),
            "payload": {},
            "extra": true
        }),
        &schema
    )
    .is_err());

    let server = TestHttpServer::start().await;
    server.enqueue_response(MockResponse::tool_call(
        "call_ev_1",
        "read_file",
        serde_json::json!({ "path": "guarded/info.txt" }),
    ));
    server.enqueue_response(MockResponse::final_answer("Validated all events."));

    let ws_dir = tempdir().unwrap();
    let gdir = ws_dir.path().join("guarded");
    fs::create_dir(&gdir).unwrap();
    fs::write(gdir.join("info.txt"), "hello schema").unwrap();

    let db_dir = tempdir().unwrap();
    let db_path = db_dir.path().join("events_test.db");
    let event_file = db_dir.path().join("events.jsonl");

    let store = Arc::new(SqliteCheckpointStore::new(&db_path).unwrap());
    let sink = Arc::new(JsonLinesEventSink::to_file(&event_file));
    let policy = Arc::new(DefaultApprovalPolicy::new().with_sensitive_path_prefix("guarded/"));

    let mut registry = ToolRegistry::new();
    registry.register(Arc::new(ReadFileTool::new(ws_dir.path())));

    let client = Arc::new(
        OpenAiCompatibleClient::new(ModelConfig::new(server.url(), "dummy_key", "m")).unwrap(),
    );
    let engine = AgentEngine::new(client, store.clone(), sink, policy, registry);
    let scope = make_scope();

    let outcome = engine
        .start_run(scope, "Read guarded/info.txt")
        .await
        .unwrap();
    let approval_id = match outcome {
        EngineOutcome::Paused { approval_id, .. } => approval_id,
        other => panic!("Expected Paused, got {:?}", other),
    };

    store
        .resolve_approval(
            &scope,
            approval_id,
            ApprovalDecision::AllowOnce,
            "test-operator",
        )
        .await
        .unwrap();

    let final_outcome = engine.resume_run(scope).await.unwrap();
    assert_eq!(
        final_outcome,
        EngineOutcome::Completed("Validated all events.".to_string())
    );

    let content = fs::read_to_string(&event_file).unwrap();
    let lines: Vec<&str> = content.lines().collect();
    assert_eq!(lines.len(), 5);

    let mut event_types = Vec::new();
    for line in lines {
        let parsed: serde_json::Value = serde_json::from_str(line).unwrap();
        validate_event_against_schema(&parsed, &schema).unwrap();
        assert_eq!(
            parsed["tenant_id"].as_str().unwrap(),
            scope.tenant_id.to_string()
        );
        assert_eq!(
            parsed["workspace_id"].as_str().unwrap(),
            scope.workspace_id.to_string()
        );
        assert_eq!(parsed["run_id"].as_str().unwrap(), scope.run_id.to_string());
        event_types.push(parsed["type"].as_str().unwrap().to_string());
    }

    assert_eq!(
        event_types,
        vec![
            "APPROVAL_REQUIRED",
            "APPROVAL_RESOLVED",
            "TOOL_INVOKED",
            "TOOL_COMPLETED",
            "RUN_COMPLETED"
        ]
    );
}

#[tokio::test]
async fn test_jsonl_stdout_events_validate_against_schema() {
    let schema: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/runtime_event.schema.json")).unwrap();
    let bin_path = env!("CARGO_BIN_EXE_nexus-albaai");

    let server = TestHttpServer::start().await;
    server.enqueue_response(MockResponse::tool_call(
        "call_stdout_1",
        "read_file",
        serde_json::json!({ "path": "open.txt" }),
    ));
    server.enqueue_response(MockResponse::final_answer("Stdout events verified."));

    let ws_dir = tempdir().unwrap();
    fs::write(ws_dir.path().join("open.txt"), "stdout payload").unwrap();

    let db_dir = tempdir().unwrap();
    let db_path = db_dir.path().join("stdout.db");
    let run_id = Uuid::new_v4().to_string();

    let out = Command::new(bin_path)
        .env("NEXUS_API_KEY", "cli_key")
        .args([
            "run",
            "--prompt",
            "Read open.txt",
            "--run-id",
            &run_id,
            "--workspace-root",
            ws_dir.path().to_str().unwrap(),
            "--db-path",
            db_path.to_str().unwrap(),
            "--api-base",
            &server.url(),
            "--event-log",
            "stdout",
        ])
        .output()
        .await
        .unwrap();

    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), 3);

    let mut types = Vec::new();
    for line in lines {
        let parsed: serde_json::Value = serde_json::from_str(line).unwrap();
        validate_event_against_schema(&parsed, &schema).unwrap();
        assert_eq!(parsed["run_id"].as_str().unwrap(), run_id);
        types.push(parsed["type"].as_str().unwrap().to_string());
    }
    assert_eq!(
        types,
        vec!["TOOL_INVOKED", "TOOL_COMPLETED", "RUN_COMPLETED"]
    );
}

#[tokio::test]
async fn test_missing_actor_rejected_for_approve_and_deny() {
    let bin_path = env!("CARGO_BIN_EXE_nexus-albaai");
    let server = TestHttpServer::start().await;
    server.enqueue_response(MockResponse::tool_call(
        "call_actor_missing",
        "read_file",
        serde_json::json!({ "path": "doc.txt" }),
    ));

    let ws_dir = tempdir().unwrap();
    fs::write(ws_dir.path().join("doc.txt"), "protected content").unwrap();

    let db_dir = tempdir().unwrap();
    let db_path = db_dir.path().join("actor_missing.db");
    let run_id = Uuid::new_v4().to_string();
    let tenant_id = Uuid::new_v4().to_string();
    let workspace_id = Uuid::new_v4().to_string();

    let run_out = Command::new(bin_path)
        .env("NEXUS_TENANT_ID", &tenant_id)
        .env("NEXUS_WORKSPACE_ID", &workspace_id)
        .env("NEXUS_API_KEY", "cli_key")
        .args([
            "run",
            "--prompt",
            "Read doc.txt",
            "--run-id",
            &run_id,
            "--workspace-root",
            ws_dir.path().to_str().unwrap(),
            "--db-path",
            db_path.to_str().unwrap(),
            "--api-base",
            &server.url(),
            "--require-approval",
        ])
        .output()
        .await
        .unwrap();
    assert!(run_out.status.success());

    let run_stdout = String::from_utf8_lossy(&run_out.stdout);
    let approval_id = run_stdout
        .lines()
        .find(|l| l.starts_with("Approval ID: "))
        .unwrap()
        .trim_start_matches("Approval ID: ")
        .trim();

    let approve_missing = Command::new(bin_path)
        .env_remove("NEXUS_ACTOR_ID")
        .env_remove("NEXUS_CONFIG_FILE")
        .env("NEXUS_TENANT_ID", &tenant_id)
        .env("NEXUS_WORKSPACE_ID", &workspace_id)
        .args([
            "approve",
            "--run-id",
            &run_id,
            "--approval-id",
            approval_id,
            "--db-path",
            db_path.to_str().unwrap(),
        ])
        .output()
        .await
        .unwrap();
    assert!(!approve_missing.status.success());
    assert!(String::from_utf8_lossy(&approve_missing.stderr).contains("Missing trusted actor"));

    let approve_blank = Command::new(bin_path)
        .env_remove("NEXUS_CONFIG_FILE")
        .env("NEXUS_ACTOR_ID", "   ")
        .env("NEXUS_TENANT_ID", &tenant_id)
        .env("NEXUS_WORKSPACE_ID", &workspace_id)
        .args([
            "approve",
            "--run-id",
            &run_id,
            "--approval-id",
            approval_id,
            "--db-path",
            db_path.to_str().unwrap(),
        ])
        .output()
        .await
        .unwrap();
    assert!(!approve_blank.status.success());

    let deny_missing = Command::new(bin_path)
        .env_remove("NEXUS_ACTOR_ID")
        .env_remove("NEXUS_CONFIG_FILE")
        .env("NEXUS_TENANT_ID", &tenant_id)
        .env("NEXUS_WORKSPACE_ID", &workspace_id)
        .args([
            "deny",
            "--run-id",
            &run_id,
            "--approval-id",
            approval_id,
            "--db-path",
            db_path.to_str().unwrap(),
        ])
        .output()
        .await
        .unwrap();
    assert!(!deny_missing.status.success());
    assert!(String::from_utf8_lossy(&deny_missing.stderr).contains("Missing trusted actor"));

    let no_actor_cfg = db_dir.path().join("no_actor_cfg.json");
    fs::write(
        &no_actor_cfg,
        serde_json::json!({
            "tenant_id": tenant_id,
            "workspace_id": workspace_id
        })
        .to_string(),
    )
    .unwrap();

    let deny_cfg_missing = Command::new(bin_path)
        .env_remove("NEXUS_ACTOR_ID")
        .env_remove("NEXUS_CONFIG_FILE")
        .args([
            "deny",
            "--run-id",
            &run_id,
            "--approval-id",
            approval_id,
            "--config",
            no_actor_cfg.to_str().unwrap(),
            "--db-path",
            db_path.to_str().unwrap(),
        ])
        .output()
        .await
        .unwrap();
    assert!(!deny_cfg_missing.status.success());

    let conn = rusqlite::Connection::open(&db_path).unwrap();
    let status: String = conn
        .query_row(
            "SELECT status FROM approvals WHERE id = ?1",
            [approval_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(status, "pending");

    let audit_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM audit_logs WHERE run_id = ?1 AND action = 'approval.resolved'",
            [&run_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(audit_count, 0);
}

#[tokio::test]
async fn test_trusted_actor_recorded_in_audit_log_for_approve_and_deny() {
    let bin_path = env!("CARGO_BIN_EXE_nexus-albaai");
    let server = TestHttpServer::start().await;
    server.enqueue_response(MockResponse::tool_call(
        "call_approve_actor",
        "read_file",
        serde_json::json!({ "path": "doc.txt" }),
    ));
    server.enqueue_response(MockResponse::tool_call(
        "call_deny_actor",
        "read_file",
        serde_json::json!({ "path": "doc.txt" }),
    ));

    let ws_dir = tempdir().unwrap();
    fs::write(ws_dir.path().join("doc.txt"), "audited payload").unwrap();

    let db_dir = tempdir().unwrap();
    let db_path = db_dir.path().join("audit_actors.db");
    let tenant_id = Uuid::new_v4().to_string();
    let workspace_id = Uuid::new_v4().to_string();
    let run1 = Uuid::new_v4().to_string();
    let run2 = Uuid::new_v4().to_string();

    let run1_out = Command::new(bin_path)
        .env("NEXUS_TENANT_ID", &tenant_id)
        .env("NEXUS_WORKSPACE_ID", &workspace_id)
        .env("NEXUS_API_KEY", "cli_key")
        .args([
            "run",
            "--prompt",
            "Read doc.txt",
            "--run-id",
            &run1,
            "--workspace-root",
            ws_dir.path().to_str().unwrap(),
            "--db-path",
            db_path.to_str().unwrap(),
            "--api-base",
            &server.url(),
            "--require-approval",
        ])
        .output()
        .await
        .unwrap();
    assert!(run1_out.status.success());
    let approval1_id = String::from_utf8_lossy(&run1_out.stdout)
        .lines()
        .find(|l| l.starts_with("Approval ID: "))
        .unwrap()
        .trim_start_matches("Approval ID: ")
        .trim()
        .to_string();

    let approve_out = Command::new(bin_path)
        .env_remove("NEXUS_CONFIG_FILE")
        .env("NEXUS_TENANT_ID", &tenant_id)
        .env("NEXUS_WORKSPACE_ID", &workspace_id)
        .env("NEXUS_ACTOR_ID", "alice@ops.local")
        .args([
            "approve",
            "--run-id",
            &run1,
            "--approval-id",
            &approval1_id,
            "--db-path",
            db_path.to_str().unwrap(),
        ])
        .output()
        .await
        .unwrap();
    assert!(
        approve_out.status.success(),
        "approve failed: {}",
        String::from_utf8_lossy(&approve_out.stderr)
    );

    let cfg_path = db_dir.path().join("trusted_actor_cfg.json");
    fs::write(
        &cfg_path,
        serde_json::json!({
            "tenant_id": tenant_id,
            "workspace_id": workspace_id,
            "actor_id": "bob@sec.local"
        })
        .to_string(),
    )
    .unwrap();

    let run2_out = Command::new(bin_path)
        .env("NEXUS_API_KEY", "cli_key")
        .args([
            "run",
            "--prompt",
            "Read doc.txt",
            "--run-id",
            &run2,
            "--config",
            cfg_path.to_str().unwrap(),
            "--workspace-root",
            ws_dir.path().to_str().unwrap(),
            "--db-path",
            db_path.to_str().unwrap(),
            "--api-base",
            &server.url(),
            "--require-approval",
        ])
        .output()
        .await
        .unwrap();
    assert!(run2_out.status.success());
    let approval2_id = String::from_utf8_lossy(&run2_out.stdout)
        .lines()
        .find(|l| l.starts_with("Approval ID: "))
        .unwrap()
        .trim_start_matches("Approval ID: ")
        .trim()
        .to_string();

    let deny_out = Command::new(bin_path)
        .env_remove("NEXUS_ACTOR_ID")
        .env_remove("NEXUS_CONFIG_FILE")
        .args([
            "deny",
            "--run-id",
            &run2,
            "--approval-id",
            &approval2_id,
            "--config",
            cfg_path.to_str().unwrap(),
            "--db-path",
            db_path.to_str().unwrap(),
        ])
        .output()
        .await
        .unwrap();
    assert!(
        deny_out.status.success(),
        "deny failed: {}",
        String::from_utf8_lossy(&deny_out.stderr)
    );

    let conn = rusqlite::Connection::open(&db_path).unwrap();
    let decided_by_1: Option<String> = conn
        .query_row(
            "SELECT decided_by FROM approvals WHERE id = ?1",
            [&approval1_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(decided_by_1.as_deref(), Some("alice@ops.local"));

    let (audit_actor_1, audit_details_1): (Option<String>, String) = conn
        .query_row(
            "SELECT actor_id, details_json FROM audit_logs WHERE run_id = ?1 AND action = 'approval.resolved'",
            [&run1],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(audit_actor_1.as_deref(), Some("alice@ops.local"));
    assert!(audit_details_1.contains("allow_once"));

    let decided_by_2: Option<String> = conn
        .query_row(
            "SELECT decided_by FROM approvals WHERE id = ?1",
            [&approval2_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(decided_by_2.as_deref(), Some("bob@sec.local"));

    let (audit_actor_2, audit_details_2): (Option<String>, String) = conn
        .query_row(
            "SELECT actor_id, details_json FROM audit_logs WHERE run_id = ?1 AND action = 'approval.resolved'",
            [&run2],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(audit_actor_2.as_deref(), Some("bob@sec.local"));
    assert!(audit_details_2.contains("deny"));
}

#[tokio::test]
async fn test_wrong_scope_rejected_by_cli() {
    let bin_path = env!("CARGO_BIN_EXE_nexus-albaai");
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("wrong_scope.db");
    let run_id = Uuid::new_v4().to_string();
    let t1 = Uuid::new_v4().to_string();
    let t2 = Uuid::new_v4().to_string();
    let w1 = Uuid::new_v4().to_string();
    let w2 = Uuid::new_v4().to_string();

    let nil_tenant_out = Command::new(bin_path)
        .env("NEXUS_TENANT_ID", "00000000-0000-0000-0000-000000000000")
        .env("NEXUS_WORKSPACE_ID", &w1)
        .args([
            "status",
            "--run-id",
            &run_id,
            "--db-path",
            db_path.to_str().unwrap(),
        ])
        .output()
        .await
        .unwrap();
    assert!(!nil_tenant_out.status.success());

    let malformed_ws_out = Command::new(bin_path)
        .args([
            "status",
            "--run-id",
            &run_id,
            "--tenant-id",
            &t1,
            "--workspace-id",
            "not-a-uuid",
            "--db-path",
            db_path.to_str().unwrap(),
        ])
        .output()
        .await
        .unwrap();
    assert!(!malformed_ws_out.status.success());

    let mismatch_env_out = Command::new(bin_path)
        .env("NEXUS_TENANT_ID", &t1)
        .env("NEXUS_WORKSPACE_ID", &w1)
        .args([
            "status",
            "--run-id",
            &run_id,
            "--tenant-id",
            &t2,
            "--db-path",
            db_path.to_str().unwrap(),
        ])
        .output()
        .await
        .unwrap();
    assert!(!mismatch_env_out.status.success());
    assert!(String::from_utf8_lossy(&mismatch_env_out.stderr).contains("Scope mismatch"));

    let cfg_path = dir.path().join("scope_cfg.json");
    fs::write(
        &cfg_path,
        serde_json::json!({
            "tenant_id": t1,
            "workspace_id": w1,
            "actor_id": "op@local"
        })
        .to_string(),
    )
    .unwrap();

    let mismatch_cfg_out = Command::new(bin_path)
        .args([
            "status",
            "--run-id",
            &run_id,
            "--config",
            cfg_path.to_str().unwrap(),
            "--workspace-id",
            &w2,
            "--db-path",
            db_path.to_str().unwrap(),
        ])
        .output()
        .await
        .unwrap();
    assert!(!mismatch_cfg_out.status.success());
    assert!(String::from_utf8_lossy(&mismatch_cfg_out.stderr).contains("Scope mismatch"));

    let other_cfg_path = dir.path().join("other_cfg.json");
    fs::write(&other_cfg_path, "{}").unwrap();
    let conflicting_cfg_paths = Command::new(bin_path)
        .env("NEXUS_CONFIG_FILE", other_cfg_path.to_str().unwrap())
        .args([
            "status",
            "--run-id",
            &run_id,
            "--config",
            cfg_path.to_str().unwrap(),
            "--db-path",
            db_path.to_str().unwrap(),
        ])
        .output()
        .await
        .unwrap();
    assert!(!conflicting_cfg_paths.status.success());
}

#[tokio::test]
async fn test_cross_scope_run_id_isolated_across_cli() {
    let bin_path = env!("CARGO_BIN_EXE_nexus-albaai");
    let server = TestHttpServer::start().await;
    server.enqueue_response(MockResponse::tool_call(
        "call_cross_scope",
        "read_file",
        serde_json::json!({ "path": "doc.txt" }),
    ));
    server.enqueue_response(MockResponse::final_answer("Owner scope finished."));

    let ws_dir = tempdir().unwrap();
    fs::write(ws_dir.path().join("doc.txt"), "tenant A confidential").unwrap();

    let db_dir = tempdir().unwrap();
    let db_path = db_dir.path().join("cross_scope.db");
    let run_id = Uuid::new_v4().to_string();
    let tenant_a = Uuid::new_v4().to_string();
    let workspace_a = Uuid::new_v4().to_string();
    let tenant_b = Uuid::new_v4().to_string();
    let workspace_b = Uuid::new_v4().to_string();

    let run_out = Command::new(bin_path)
        .env("NEXUS_TENANT_ID", &tenant_a)
        .env("NEXUS_WORKSPACE_ID", &workspace_a)
        .env("NEXUS_API_KEY", "cli_key")
        .args([
            "run",
            "--prompt",
            "Read doc.txt",
            "--run-id",
            &run_id,
            "--workspace-root",
            ws_dir.path().to_str().unwrap(),
            "--db-path",
            db_path.to_str().unwrap(),
            "--api-base",
            &server.url(),
            "--require-approval",
        ])
        .output()
        .await
        .unwrap();
    assert!(run_out.status.success());
    let approval_id = String::from_utf8_lossy(&run_out.stdout)
        .lines()
        .find(|l| l.starts_with("Approval ID: "))
        .unwrap()
        .trim_start_matches("Approval ID: ")
        .trim()
        .to_string();

    for (foreign_t, foreign_w) in [
        (&tenant_b, &workspace_a),
        (&tenant_a, &workspace_b),
        (&tenant_b, &workspace_b),
    ] {
        let status_other = Command::new(bin_path)
            .env("NEXUS_TENANT_ID", foreign_t)
            .env("NEXUS_WORKSPACE_ID", foreign_w)
            .args([
                "status",
                "--run-id",
                &run_id,
                "--db-path",
                db_path.to_str().unwrap(),
            ])
            .output()
            .await
            .unwrap();
        assert!(status_other.status.success());
        let status_text = String::from_utf8_lossy(&status_other.stdout);
        assert!(status_text.contains("No checkpoints found"));
        assert!(!status_text.contains("paused"));

        let approve_other = Command::new(bin_path)
            .env("NEXUS_TENANT_ID", foreign_t)
            .env("NEXUS_WORKSPACE_ID", foreign_w)
            .env("NEXUS_ACTOR_ID", "intruder")
            .args([
                "approve",
                "--run-id",
                &run_id,
                "--approval-id",
                &approval_id,
                "--db-path",
                db_path.to_str().unwrap(),
            ])
            .output()
            .await
            .unwrap();
        assert!(!approve_other.status.success());

        let deny_other = Command::new(bin_path)
            .env("NEXUS_TENANT_ID", foreign_t)
            .env("NEXUS_WORKSPACE_ID", foreign_w)
            .env("NEXUS_ACTOR_ID", "intruder")
            .args([
                "deny",
                "--run-id",
                &run_id,
                "--approval-id",
                &approval_id,
                "--db-path",
                db_path.to_str().unwrap(),
            ])
            .output()
            .await
            .unwrap();
        assert!(!deny_other.status.success());

        let resume_other = Command::new(bin_path)
            .env("NEXUS_TENANT_ID", foreign_t)
            .env("NEXUS_WORKSPACE_ID", foreign_w)
            .env("NEXUS_API_KEY", "cli_key")
            .args([
                "resume",
                "--run-id",
                &run_id,
                "--workspace-root",
                ws_dir.path().to_str().unwrap(),
                "--db-path",
                db_path.to_str().unwrap(),
                "--api-base",
                &server.url(),
            ])
            .output()
            .await
            .unwrap();
        assert!(!resume_other.status.success());

        let cancel_other = Command::new(bin_path)
            .env("NEXUS_TENANT_ID", foreign_t)
            .env("NEXUS_WORKSPACE_ID", foreign_w)
            .args([
                "cancel",
                "--run-id",
                &run_id,
                "--db-path",
                db_path.to_str().unwrap(),
            ])
            .output()
            .await
            .unwrap();
        assert!(!cancel_other.status.success());
    }

    let owner_status = Command::new(bin_path)
        .env("NEXUS_TENANT_ID", &tenant_a)
        .env("NEXUS_WORKSPACE_ID", &workspace_a)
        .args([
            "status",
            "--run-id",
            &run_id,
            "--db-path",
            db_path.to_str().unwrap(),
        ])
        .output()
        .await
        .unwrap();
    assert!(owner_status.status.success());
    let owner_text = String::from_utf8_lossy(&owner_status.stdout);
    assert!(owner_text.contains("Status: paused"));
    assert!(owner_text.contains("Cancellation requested: false"));
    assert!(owner_text.contains("Approval status: pending"));

    let owner_approve = Command::new(bin_path)
        .env("NEXUS_TENANT_ID", &tenant_a)
        .env("NEXUS_WORKSPACE_ID", &workspace_a)
        .env("NEXUS_ACTOR_ID", "owner@tenant-a.local")
        .args([
            "approve",
            "--run-id",
            &run_id,
            "--approval-id",
            &approval_id,
            "--db-path",
            db_path.to_str().unwrap(),
        ])
        .output()
        .await
        .unwrap();
    assert!(owner_approve.status.success());

    let owner_resume = Command::new(bin_path)
        .env("NEXUS_TENANT_ID", &tenant_a)
        .env("NEXUS_WORKSPACE_ID", &workspace_a)
        .env("NEXUS_API_KEY", "cli_key")
        .args([
            "resume",
            "--run-id",
            &run_id,
            "--workspace-root",
            ws_dir.path().to_str().unwrap(),
            "--db-path",
            db_path.to_str().unwrap(),
            "--api-base",
            &server.url(),
        ])
        .output()
        .await
        .unwrap();
    assert!(owner_resume.status.success());
    assert!(String::from_utf8_lossy(&owner_resume.stdout).contains("Owner scope finished."));
}

struct CountingReadTool {
    inner: ReadFileTool,
    executions: Arc<AtomicUsize>,
    delay: Duration,
}

impl Tool for CountingReadTool {
    fn definition(&self) -> ToolDefinition {
        self.inner.definition()
    }

    fn execute<'a>(
        &'a self,
        scope: &'a ExecutionScope,
        arguments: &'a serde_json::Value,
    ) -> BoxFuture<'a, Result<String, ToolError>> {
        Box::pin(async move {
            self.executions.fetch_add(1, Ordering::SeqCst);
            tokio::time::sleep(self.delay).await;
            self.inner.execute(scope, arguments).await
        })
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_concurrent_thread_resume_executes_tool_once() {
    let server = TestHttpServer::start().await;
    server.enqueue_response(MockResponse::tool_call(
        "call_conc_thread",
        "read_file",
        serde_json::json!({ "path": "guard/item.txt" }),
    ));
    server.enqueue_response(MockResponse::final_answer("Concurrent winner done."));

    let ws_dir = tempdir().unwrap();
    let guard_dir = ws_dir.path().join("guard");
    fs::create_dir(&guard_dir).unwrap();
    fs::write(guard_dir.join("item.txt"), "single-execution-payload").unwrap();

    let db_dir = tempdir().unwrap();
    let db_path = db_dir.path().join("concurrent_threads.db");
    let exec_count = Arc::new(AtomicUsize::new(0));
    let scope = make_scope();

    {
        let store = Arc::new(SqliteCheckpointStore::new(&db_path).unwrap());
        let events = Arc::new(InMemoryEventSink::new());
        let policy = Arc::new(DefaultApprovalPolicy::new().with_sensitive_path_prefix("guard/"));
        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(CountingReadTool {
            inner: ReadFileTool::new(ws_dir.path()),
            executions: exec_count.clone(),
            delay: Duration::from_millis(40),
        }));
        let client = Arc::new(
            OpenAiCompatibleClient::new(ModelConfig::new(server.url(), "k", "m")).unwrap(),
        );
        let engine = AgentEngine::new(client, store.clone(), events, policy, registry);

        let paused = engine
            .start_run(scope, "Read guard/item.txt")
            .await
            .unwrap();
        let approval_id = match paused {
            EngineOutcome::Paused { approval_id, .. } => approval_id,
            other => panic!("Expected Paused, got {:?}", other),
        };
        store
            .resolve_approval(&scope, approval_id, ApprovalDecision::AllowOnce, "race-op")
            .await
            .unwrap();
    }

    let worker_count = 8usize;
    let barrier = Arc::new(Barrier::new(worker_count));
    let mut join_set = JoinSet::new();

    for _ in 0..worker_count {
        let b = barrier.clone();
        let db = db_path.clone();
        let ws = ws_dir.path().to_path_buf();
        let url = server.url();
        let count = exec_count.clone();
        join_set.spawn(async move {
            let store = Arc::new(SqliteCheckpointStore::new(&db).unwrap());
            let events = Arc::new(InMemoryEventSink::new());
            let policy = Arc::new(DefaultApprovalPolicy::new());
            let mut registry = ToolRegistry::new();
            registry.register(Arc::new(CountingReadTool {
                inner: ReadFileTool::new(&ws),
                executions: count,
                delay: Duration::from_millis(40),
            }));
            let client =
                Arc::new(OpenAiCompatibleClient::new(ModelConfig::new(url, "k", "m")).unwrap());
            let engine = AgentEngine::new(client, store, events, policy, registry);
            b.wait().await;
            engine.resume_run(scope).await
        });
    }

    let mut winners = 0usize;
    let mut losers = 0usize;
    while let Some(res) = join_set.join_next().await {
        match res.unwrap() {
            Ok(EngineOutcome::Completed(msg)) => {
                assert_eq!(msg, "Concurrent winner done.");
                winners += 1;
            }
            Ok(other) => panic!("Unexpected non-completed outcome in race: {:?}", other),
            Err(EngineError::ApprovalAlreadyConsumed(_)) | Err(EngineError::InvalidState(_)) => {
                losers += 1;
            }
            Err(other) => panic!("Unexpected error in race: {:?}", other),
        }
    }

    assert_eq!(winners, 1);
    assert_eq!(losers, worker_count - 1);
    assert_eq!(exec_count.load(Ordering::SeqCst), 1);
    assert_eq!(server.received_requests().len(), 2);

    let final_store = SqliteCheckpointStore::new(&db_path).unwrap();
    let latest = final_store
        .get_latest_checkpoint(&scope)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(latest.status, RunStatus::Completed);
}

#[tokio::test]
async fn test_concurrent_process_resume_executes_tool_once() {
    let bin_path = env!("CARGO_BIN_EXE_nexus-albaai");
    let server = TestHttpServer::start().await;
    server.enqueue_response(MockResponse::tool_call(
        "call_conc_proc",
        "read_file",
        serde_json::json!({ "path": "doc.txt" }),
    ));
    server.enqueue_response(MockResponse::final_answer("Process winner completed."));

    let ws_dir = tempdir().unwrap();
    fs::write(ws_dir.path().join("doc.txt"), "process race content").unwrap();

    let db_dir = tempdir().unwrap();
    let db_path = db_dir.path().join("concurrent_procs.db");
    let run_id = Uuid::new_v4().to_string();
    let tenant_id = Uuid::new_v4().to_string();
    let workspace_id = Uuid::new_v4().to_string();

    let run_out = Command::new(bin_path)
        .env("NEXUS_TENANT_ID", &tenant_id)
        .env("NEXUS_WORKSPACE_ID", &workspace_id)
        .env("NEXUS_API_KEY", "cli_key")
        .args([
            "run",
            "--prompt",
            "Read doc.txt",
            "--run-id",
            &run_id,
            "--workspace-root",
            ws_dir.path().to_str().unwrap(),
            "--db-path",
            db_path.to_str().unwrap(),
            "--api-base",
            &server.url(),
            "--require-approval",
        ])
        .output()
        .await
        .unwrap();
    assert!(run_out.status.success());

    let approval_id = String::from_utf8_lossy(&run_out.stdout)
        .lines()
        .find(|l| l.starts_with("Approval ID: "))
        .unwrap()
        .trim_start_matches("Approval ID: ")
        .trim()
        .to_string();

    let approve_out = Command::new(bin_path)
        .env("NEXUS_TENANT_ID", &tenant_id)
        .env("NEXUS_WORKSPACE_ID", &workspace_id)
        .env("NEXUS_ACTOR_ID", "proc-race-op")
        .args([
            "approve",
            "--run-id",
            &run_id,
            "--approval-id",
            &approval_id,
            "--db-path",
            db_path.to_str().unwrap(),
        ])
        .output()
        .await
        .unwrap();
    assert!(approve_out.status.success());

    let proc_count = 4usize;
    let barrier = Arc::new(Barrier::new(proc_count));
    let mut join_set = JoinSet::new();

    for idx in 0..proc_count {
        let b = barrier.clone();
        let tenant = tenant_id.clone();
        let workspace = workspace_id.clone();
        let rid = run_id.clone();
        let ws = ws_dir.path().to_str().unwrap().to_string();
        let db = db_path.to_str().unwrap().to_string();
        let url = server.url();
        let event_path = db_dir.path().join(format!("proc_{}.jsonl", idx));

        join_set.spawn(async move {
            b.wait().await;
            let output = Command::new(bin_path)
                .env("NEXUS_TENANT_ID", &tenant)
                .env("NEXUS_WORKSPACE_ID", &workspace)
                .env("NEXUS_API_KEY", "cli_key")
                .args([
                    "resume",
                    "--run-id",
                    &rid,
                    "--workspace-root",
                    &ws,
                    "--db-path",
                    &db,
                    "--api-base",
                    &url,
                    "--event-log",
                    event_path.to_str().unwrap(),
                ])
                .output()
                .await
                .unwrap();
            (output, event_path)
        });
    }

    let mut successes = 0usize;
    let mut failures = 0usize;
    let mut tool_invoked_events = 0usize;
    let mut completed_events = 0usize;

    while let Some(res) = join_set.join_next().await {
        let (output, event_path) = res.unwrap();
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            assert!(stdout.contains("Process winner completed."));
            successes += 1;
        } else {
            failures += 1;
        }
        if event_path.exists() {
            let content = fs::read_to_string(&event_path).unwrap();
            for line in content.lines() {
                let parsed: serde_json::Value = serde_json::from_str(line).unwrap();
                match parsed["type"].as_str().unwrap() {
                    "TOOL_INVOKED" => tool_invoked_events += 1,
                    "RUN_COMPLETED" => completed_events += 1,
                    _ => {}
                }
            }
        }
    }

    assert!(successes >= 1);
    assert_eq!(successes + failures, proc_count);
    assert_eq!(tool_invoked_events, 1);
    assert_eq!(completed_events, 1);
    assert_eq!(server.received_requests().len(), 2);

    let conn = rusqlite::Connection::open(&db_path).unwrap();
    let app_status: String = conn
        .query_row(
            "SELECT status FROM approvals WHERE id = ?1",
            [&approval_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(app_status, "consumed");

    let executing_audits: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM audit_logs WHERE run_id = ?1 AND action = 'approval.executing'",
            [&run_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(executing_audits, 1);

    let consumed_audits: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM audit_logs WHERE run_id = ?1 AND action = 'approval.consumed'",
            [&run_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(consumed_audits, 1);
}

#[cfg(unix)]
#[tokio::test]
async fn test_sqlite_checkpoint_file_created_with_mode_0600() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("nested").join("secure_state.db");

    let store = SqliteCheckpointStore::new(&db_path).unwrap();
    let scope = make_scope();
    let cp = nexus_albaai::checkpoints::types::RunCheckpoint {
        id: Uuid::new_v4(),
        scope,
        sequence: 1,
        status: RunStatus::Running,
        history: vec![nexus_albaai::domain::ChatMessage::user("hello")],
        active_tool_call: None,
        pending_tool_calls: Vec::new(),
        pending_approval_id: None,
        author_id: None,
        model_override: None,
        created_at: chrono::Utc::now(),
    };
    store.save_checkpoint(&cp).await.unwrap();

    let mode = fs::metadata(&db_path).unwrap().permissions().mode() & 0o777;
    assert_eq!(
        mode, 0o600,
        "Expected SQLite file mode 0600, found {:o}",
        mode
    );

    for suffix in ["-wal", "-shm"] {
        let sidecar = dir
            .path()
            .join("nested")
            .join(format!("secure_state.db{}", suffix));
        if sidecar.exists() {
            let sidecar_mode = fs::metadata(&sidecar).unwrap().permissions().mode() & 0o777;
            assert_eq!(
                sidecar_mode, 0o600,
                "Expected sidecar {} mode 0600, found {:o}",
                suffix, sidecar_mode
            );
        }
    }
}

#[tokio::test]
async fn test_no_api_key_or_authorization_header_in_db_or_logs() {
    let bin_path = env!("CARGO_BIN_EXE_nexus-albaai");
    let server = TestHttpServer::start().await;
    server.enqueue_response(MockResponse::tool_call(
        "call_sec_live",
        "read_file",
        serde_json::json!({ "path": "doc.txt" }),
    ));
    server.enqueue_response(MockResponse::final_answer("Secret check completed."));
    server.enqueue_response(MockResponse::error(401, "Invalid API key provided"));

    let secret_live = "nx-secret-live-token-9988776655";
    let secret_err = "nx-secret-err-token-4433221100";

    let redacted_debug = format!(
        "{:?}",
        ModelConfig::new("https://example.invalid/v1", "nx-secret-debug-0000", "m")
    );
    assert!(!redacted_debug.contains("nx-secret-debug-0000"));
    assert!(redacted_debug.contains("[REDACTED]"));

    let ws_dir = tempdir().unwrap();
    fs::write(ws_dir.path().join("doc.txt"), "non-secret workspace doc").unwrap();

    let db_dir = tempdir().unwrap();
    let db_path = db_dir.path().join("no_secret.db");
    let run_events_path = db_dir.path().join("run_events.jsonl");
    let err_events_path = db_dir.path().join("err_events.jsonl");

    let tenant_id = Uuid::new_v4().to_string();
    let workspace_id = Uuid::new_v4().to_string();
    let run_id = Uuid::new_v4().to_string();
    let err_run_id = Uuid::new_v4().to_string();

    let run_out = Command::new(bin_path)
        .env("NEXUS_TENANT_ID", &tenant_id)
        .env("NEXUS_WORKSPACE_ID", &workspace_id)
        .env("NEXUS_API_KEY", secret_live)
        .args([
            "run",
            "--prompt",
            "Read doc.txt",
            "--run-id",
            &run_id,
            "--workspace-root",
            ws_dir.path().to_str().unwrap(),
            "--db-path",
            db_path.to_str().unwrap(),
            "--api-base",
            &server.url(),
            "--require-approval",
            "--event-log",
            run_events_path.to_str().unwrap(),
        ])
        .output()
        .await
        .unwrap();
    assert!(run_out.status.success());

    let approval_id = String::from_utf8_lossy(&run_out.stdout)
        .lines()
        .find(|l| l.starts_with("Approval ID: "))
        .unwrap()
        .trim_start_matches("Approval ID: ")
        .trim()
        .to_string();

    let approve_out = Command::new(bin_path)
        .env("NEXUS_TENANT_ID", &tenant_id)
        .env("NEXUS_WORKSPACE_ID", &workspace_id)
        .env("NEXUS_ACTOR_ID", "sec-auditor")
        .args([
            "approve",
            "--run-id",
            &run_id,
            "--approval-id",
            &approval_id,
            "--db-path",
            db_path.to_str().unwrap(),
        ])
        .output()
        .await
        .unwrap();
    assert!(approve_out.status.success());

    let resume_out = Command::new(bin_path)
        .env("NEXUS_TENANT_ID", &tenant_id)
        .env("NEXUS_WORKSPACE_ID", &workspace_id)
        .env("NEXUS_API_KEY", secret_live)
        .args([
            "resume",
            "--run-id",
            &run_id,
            "--workspace-root",
            ws_dir.path().to_str().unwrap(),
            "--db-path",
            db_path.to_str().unwrap(),
            "--api-base",
            &server.url(),
            "--event-log",
            run_events_path.to_str().unwrap(),
        ])
        .output()
        .await
        .unwrap();
    assert!(resume_out.status.success());

    let err_out = Command::new(bin_path)
        .env("NEXUS_TENANT_ID", &tenant_id)
        .env("NEXUS_WORKSPACE_ID", &workspace_id)
        .env("NEXUS_API_KEY", secret_err)
        .args([
            "run",
            "--prompt",
            "Fail auth check",
            "--run-id",
            &err_run_id,
            "--workspace-root",
            ws_dir.path().to_str().unwrap(),
            "--db-path",
            db_path.to_str().unwrap(),
            "--api-base",
            &server.url(),
            "--event-log",
            err_events_path.to_str().unwrap(),
        ])
        .output()
        .await
        .unwrap();
    assert!(err_out.status.success());

    let http_reqs = server.received_requests();
    assert!(http_reqs
        .iter()
        .any(|r| r.contains(&format!("Bearer {}", secret_live))));
    assert!(http_reqs
        .iter()
        .any(|r| r.contains(&format!("Bearer {}", secret_err))));

    let forbidden = [
        secret_live.to_string(),
        secret_err.to_string(),
        format!("Bearer {}", secret_live),
        format!("Bearer {}", secret_err),
        "authorization: bearer".to_string(),
    ];

    let check_text = |label: &str, text: &str| {
        let lower = text.to_lowercase();
        for needle in &forbidden {
            assert!(
                !lower.contains(&needle.to_lowercase()),
                "Found forbidden secret/header '{}' in {}",
                needle,
                label
            );
        }
    };

    check_text("run stdout", &String::from_utf8_lossy(&run_out.stdout));
    check_text("run stderr", &String::from_utf8_lossy(&run_out.stderr));
    check_text(
        "resume stdout",
        &String::from_utf8_lossy(&resume_out.stdout),
    );
    check_text(
        "resume stderr",
        &String::from_utf8_lossy(&resume_out.stderr),
    );
    check_text("err stdout", &String::from_utf8_lossy(&err_out.stdout));
    check_text("err stderr", &String::from_utf8_lossy(&err_out.stderr));
    check_text(
        "run events log",
        &fs::read_to_string(&run_events_path).unwrap(),
    );
    check_text(
        "err events log",
        &fs::read_to_string(&err_events_path).unwrap(),
    );

    let raw_db = fs::read(&db_path).unwrap();
    check_text("raw sqlite file", &String::from_utf8_lossy(&raw_db));

    let wal_path = db_dir.path().join("no_secret.db-wal");
    if wal_path.exists() {
        let raw_wal = fs::read(&wal_path).unwrap();
        check_text("raw sqlite wal", &String::from_utf8_lossy(&raw_wal));
    }

    let conn = rusqlite::Connection::open(&db_path).unwrap();
    let mut cp_stmt = conn.prepare("SELECT state_json FROM checkpoints").unwrap();
    let cp_rows = cp_stmt.query_map([], |r| r.get::<_, String>(0)).unwrap();
    for row in cp_rows {
        check_text("checkpoints.state_json", &row.unwrap());
    }

    let mut audit_stmt = conn.prepare("SELECT details_json FROM audit_logs").unwrap();
    let audit_rows = audit_stmt.query_map([], |r| r.get::<_, String>(0)).unwrap();
    for row in audit_rows {
        check_text("audit_logs.details_json", &row.unwrap());
    }
}

#[tokio::test]
async fn test_check_no_comments_script() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let script_path = format!("{}/scripts/check_no_comments.sh", manifest_dir);

    let repo_out = Command::new(&script_path).output().await.unwrap();
    assert!(
        repo_out.status.success(),
        "check_no_comments.sh failed on repo: {}\n{}",
        String::from_utf8_lossy(&repo_out.stdout),
        String::from_utf8_lossy(&repo_out.stderr)
    );

    let clean_dir = tempdir().unwrap();
    fs::create_dir_all(clean_dir.path().join("src")).unwrap();
    fs::create_dir_all(clean_dir.path().join("scripts")).unwrap();
    fs::write(
        clean_dir.path().join("src/clean.rs"),
        r##"
pub fn strings_only() -> &'static str {
    let url = "https://example.com/a//b";
    let raw = r#"// not a comment /* neither */"#;
    let slash_char = '/';
    let _c = slash_char;
    let _ = raw;
    url
}
"##,
    )
    .unwrap();
    fs::write(
        clean_dir.path().join("scripts/clean.sh"),
        "#!/usr/bin/env bash\nVAL=\"foo # not comment\"\necho \"${#VAL}\"\n",
    )
    .unwrap();
    fs::write(
        clean_dir.path().join("scripts/clean.py"),
        "#!/usr/bin/env python3\nx = 'https://a//b # ok'\n",
    )
    .unwrap();
    fs::write(
        clean_dir.path().join("Cargo.toml"),
        "[package]\nname = \"x\"\n",
    )
    .unwrap();
    fs::write(clean_dir.path().join(".gitignore"), "/target\n").unwrap();
    fs::write(
        clean_dir.path().join("Cargo.lock"),
        "# lockfile comment is excluded\n",
    )
    .unwrap();

    let clean_out = Command::new(&script_path)
        .arg(clean_dir.path().to_str().unwrap())
        .output()
        .await
        .unwrap();
    assert!(
        clean_out.status.success(),
        "Expected clean synthetic repo to pass, got: {}",
        String::from_utf8_lossy(&clean_out.stdout)
    );

    let dirty_dir = tempdir().unwrap();
    fs::create_dir_all(dirty_dir.path().join("src")).unwrap();
    fs::create_dir_all(dirty_dir.path().join("scripts")).unwrap();
    fs::write(
        dirty_dir.path().join("src/dirty.rs"),
        "pub fn f() {\n    // real comment\n    /* block comment */\n}\n",
    )
    .unwrap();
    fs::write(
        dirty_dir.path().join("Cargo.toml"),
        "# toml comment\n[package]\nname = \"x\"\n",
    )
    .unwrap();
    fs::write(
        dirty_dir.path().join(".gitignore"),
        "# ignore comment\n/target\n",
    )
    .unwrap();
    fs::write(
        dirty_dir.path().join("scripts/dirty.py"),
        "def f():\n    \"\"\"doc\"\"\"\n    # py comment\n    return 1\n",
    )
    .unwrap();

    let dirty_out = Command::new(&script_path)
        .arg(dirty_dir.path().to_str().unwrap())
        .output()
        .await
        .unwrap();
    assert!(!dirty_out.status.success());
    let stdout = String::from_utf8_lossy(&dirty_out.stdout);
    assert!(stdout.contains("src/dirty.rs:2:"));
    assert!(stdout.contains("src/dirty.rs:3:"));
    assert!(stdout.contains("Cargo.toml:1:"));
    assert!(stdout.contains(".gitignore:1:"));
    assert!(stdout.contains("scripts/dirty.py:2:"));
    assert!(stdout.contains("scripts/dirty.py:3:"));
}
