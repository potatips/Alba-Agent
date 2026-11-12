use std::collections::BTreeMap;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::symlink;
use std::sync::Arc;
#[cfg(target_os = "linux")]
use std::time::Duration;

use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use uuid::Uuid;

use nexus_albaai::agent::engine::AgentEngine;
use nexus_albaai::agent::state::EngineOutcome;
use nexus_albaai::checkpoints::sqlite::SqliteCheckpointStore;
use nexus_albaai::checkpoints::store::CheckpointStore;
use nexus_albaai::checkpoints::types::ForkRunRequest;
use nexus_albaai::domain::{
    compact_history, ActionIntentKind, ApprovalDecision, ChatMessage, ExecutionScope,
    HistoryLimits, MessageRole, ToolCall,
};
use nexus_albaai::events::sink::InMemoryEventSink;
use nexus_albaai::events::types::RuntimeEventType;
use nexus_albaai::governance::policy::DefaultApprovalPolicy;
use nexus_albaai::models::adapter::OpenAiCompatibleClient;
use nexus_albaai::models::types::ModelConfig;
use nexus_albaai::sandbox::generate_seatbelt_profile;
use nexus_albaai::sandbox::SandboxPolicy;
#[cfg(target_os = "linux")]
use nexus_albaai::sandbox::{SandboxBackendKind, SandboxRunner};
#[cfg(target_os = "linux")]
use nexus_albaai::tools::RunCommandTool;
use nexus_albaai::tools::{
    register_workspace_tools, ListDirTool, Tool, ToolError, ToolRegistry, WriteFileTool,
};
use nexus_albaai::workflow::{
    FlowExecutor, FlowNode, FlowNodeKind, FlowOutcome, FlowRouteRule, FlowSpec,
};
use tempfile::tempdir;

mod common;
use common::{MockResponse, TestHttpServer};

fn make_scope() -> ExecutionScope {
    ExecutionScope::new(Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4()).unwrap()
}

#[tokio::test]
async fn test_multi_tool_calls_sequential_and_approval_pause() {
    let server = TestHttpServer::start().await;
    server.enqueue_response(MockResponse::multi_tool_calls(&[
        (
            "call_m_1",
            "read_file",
            serde_json::json!({ "path": "a.txt" }),
        ),
        (
            "call_m_2",
            "write_file",
            serde_json::json!({ "path": "b.txt", "content": "updated by m2\n" }),
        ),
        (
            "call_m_3",
            "read_file",
            serde_json::json!({ "path": "c.txt" }),
        ),
    ]));
    server.enqueue_response(MockResponse::final_answer(
        "All three tool calls completed sequentially.",
    ));

    let ws_dir = tempdir().unwrap();
    fs::write(ws_dir.path().join("a.txt"), "alpha content").unwrap();
    fs::write(ws_dir.path().join("b.txt"), "original b\n").unwrap();
    fs::write(ws_dir.path().join("c.txt"), "charlie content").unwrap();

    let db_dir = tempdir().unwrap();
    let db_path = db_dir.path().join("multi.db");
    let store = Arc::new(SqliteCheckpointStore::new(&db_path).unwrap());
    let events = Arc::new(InMemoryEventSink::new());
    let policy = Arc::new(DefaultApprovalPolicy::new().with_workspace_root(ws_dir.path()));

    let mut registry = ToolRegistry::new();
    register_workspace_tools(&mut registry, ws_dir.path(), Some(events.clone()));

    let model_cfg = ModelConfig::new(server.url(), "test_key", "test_model");
    let client = Arc::new(OpenAiCompatibleClient::new(model_cfg).unwrap());
    let engine = AgentEngine::new(client, store.clone(), events.clone(), policy, registry)
        .with_author_id("author-1");

    let scope = make_scope();
    let first = engine
        .start_run(scope, "Read a, update b, read c.")
        .await
        .unwrap();

    let approval_id = match first {
        EngineOutcome::Paused { approval_id, .. } => approval_id,
        other => panic!("Expected Paused, got {:?}", other),
    };

    assert_eq!(server.received_requests().len(), 1);
    assert_eq!(
        fs::read_to_string(ws_dir.path().join("b.txt")).unwrap(),
        "original b\n"
    );

    let paused_cp = store.get_latest_checkpoint(&scope).await.unwrap().unwrap();
    assert_eq!(
        paused_cp.active_tool_call.as_ref().map(|c| c.id.as_str()),
        Some("call_m_2")
    );
    assert_eq!(paused_cp.pending_tool_calls.len(), 2);
    assert_eq!(paused_cp.pending_tool_calls[0].id, "call_m_2");
    assert_eq!(paused_cp.pending_tool_calls[1].id, "call_m_3");

    store
        .resolve_approval(
            &scope,
            approval_id,
            ApprovalDecision::AllowOnce,
            "reviewer-1",
        )
        .await
        .unwrap();

    let resumed = engine.resume_run(scope).await.unwrap();
    assert_eq!(
        resumed,
        EngineOutcome::Completed("All three tool calls completed sequentially.".to_string())
    );

    assert_eq!(
        fs::read_to_string(ws_dir.path().join("b.txt")).unwrap(),
        "updated by m2\n"
    );

    let reqs = server.received_requests();
    assert_eq!(reqs.len(), 2);
    let second_body = reqs[1].split("\r\n\r\n").nth(1).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(second_body).unwrap();
    let tool_ids: Vec<&str> = parsed["messages"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|m| m["role"] == "tool")
        .map(|m| m["tool_call_id"].as_str().unwrap())
        .collect();
    assert_eq!(tool_ids, vec!["call_m_1", "call_m_2", "call_m_3"]);
}

#[test]
fn test_bounded_conversation_history_preserves_system_and_unanswered_tools() {
    let messages = vec![
        ChatMessage::system("You are Alba Agent."),
        ChatMessage::user("Turn 1 question with long padding ".repeat(4).as_str()),
        ChatMessage::assistant("Turn 1 answer with long padding ".repeat(4).as_str()),
        ChatMessage::assistant_tool_calls(
            "",
            vec![ToolCall {
                id: "call_open_1".to_string(),
                name: "read_file".to_string(),
                arguments: serde_json::json!({ "path": "doc.txt" }),
            }],
        ),
        ChatMessage::tool("call_open_1", "doc result"),
        ChatMessage::assistant_tool_calls(
            "",
            vec![ToolCall {
                id: "call_unanswered".to_string(),
                name: "write_file".to_string(),
                arguments: serde_json::json!({ "path": "out.txt", "content": "hi" }),
            }],
        ),
    ];

    let limits = HistoryLimits {
        max_messages: 3,
        max_bytes: 320,
    };
    let compacted = compact_history(&messages, &limits);

    assert_eq!(compacted[0].role, MessageRole::System);
    assert!(compacted.iter().any(|m| {
        m.tool_calls
            .as_ref()
            .is_some_and(|calls| calls.iter().any(|tc| tc.id == "call_unanswered"))
    }));
    let non_system_bytes: usize = compacted.iter().map(|m| m.estimated_bytes()).sum();
    assert!(compacted.len() <= 3 || non_system_bytes <= 400);
}

#[tokio::test]
async fn test_credential_hardening_rejects_cli_flag_and_reads_stdin() {
    let bin_path = env!("CARGO_BIN_EXE_nexus-albaai");
    let ws_dir = tempdir().unwrap();
    let db_dir = tempdir().unwrap();
    let db_path = db_dir.path().join("cred.db");

    let rejected = Command::new(bin_path)
        .args([
            "run",
            "--prompt",
            "Hello",
            "--workspace-root",
            ws_dir.path().to_str().unwrap(),
            "--db-path",
            db_path.to_str().unwrap(),
            "--api-key",
            "leaked_argv_secret",
        ])
        .output()
        .await
        .unwrap();
    assert!(!rejected.status.success());

    let server = TestHttpServer::start().await;
    server.enqueue_response(MockResponse::final_answer("Authenticated from stdin."));
    let secret = "stdin_secret_token_987654";

    let mut child = Command::new(bin_path)
        .env_remove("NEXUS_API_KEY")
        .args([
            "run",
            "--prompt",
            "Hello stdin",
            "--workspace-root",
            ws_dir.path().to_str().unwrap(),
            "--db-path",
            db_path.to_str().unwrap(),
            "--api-base",
            &server.url(),
            "--api-key-stdin",
        ])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(format!("{}\n", secret).as_bytes())
            .await
            .unwrap();
        drop(stdin);
    }

    let out = child.wait_with_output().await.unwrap();
    assert!(
        out.status.success(),
        "stdin run failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(String::from_utf8_lossy(&out.stdout).contains("Authenticated from stdin."));

    let reqs = server.received_requests();
    assert!(reqs
        .iter()
        .any(|r| r.contains(&format!("Bearer {}", secret))));
    let db_bytes = fs::read(&db_path).unwrap();
    let db_lossy = String::from_utf8_lossy(&db_bytes);
    assert!(!db_lossy.contains(secret));
}

#[tokio::test]
async fn test_separation_of_duties_and_write_diff_audit() {
    let server = TestHttpServer::start().await;
    server.enqueue_response(MockResponse::tool_call(
        "call_sod_write",
        "write_file",
        serde_json::json!({
            "path": "config.txt",
            "content": "v1\nv2_approved\n"
        }),
    ));
    server.enqueue_response(MockResponse::final_answer("Write complete."));

    let ws_dir = tempdir().unwrap();
    fs::write(ws_dir.path().join("config.txt"), "v1\n").unwrap();

    let db_dir = tempdir().unwrap();
    let db_path = db_dir.path().join("sod.db");
    let store = Arc::new(SqliteCheckpointStore::new(&db_path).unwrap());
    let events = Arc::new(InMemoryEventSink::new());
    let policy = Arc::new(DefaultApprovalPolicy::new().with_workspace_root(ws_dir.path()));

    let mut registry = ToolRegistry::new();
    register_workspace_tools(&mut registry, ws_dir.path(), Some(events.clone()));

    let model_cfg = ModelConfig::new(server.url(), "key", "model-a");
    let client = Arc::new(OpenAiCompatibleClient::new(model_cfg).unwrap());
    let engine = AgentEngine::new(client, store.clone(), events.clone(), policy, registry);

    let scope = make_scope();
    let outcome = engine
        .start_run_as(scope, "Update config.txt", Some("alice@engineering.local"))
        .await
        .unwrap();
    let approval_id = match outcome {
        EngineOutcome::Paused { approval_id, .. } => approval_id,
        other => panic!("Expected Paused, got {:?}", other),
    };

    let req = store
        .get_approval(&scope, approval_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(req.author_id.as_deref(), Some("alice@engineering.local"));

    let self_err = store
        .resolve_approval(
            &scope,
            approval_id,
            ApprovalDecision::AllowOnce,
            "alice@engineering.local",
        )
        .await
        .unwrap_err();
    assert!(self_err.to_string().contains("Separation of duties"));

    store
        .resolve_approval(
            &scope,
            approval_id,
            ApprovalDecision::AllowOnce,
            "bob@security.local",
        )
        .await
        .unwrap();

    let final_outcome = engine.resume_run(scope).await.unwrap();
    assert_eq!(
        final_outcome,
        EngineOutcome::Completed("Write complete.".to_string())
    );
    assert_eq!(
        fs::read_to_string(ws_dir.path().join("config.txt")).unwrap(),
        "v1\nv2_approved\n"
    );

    let logs = store.list_audit_logs(&scope).await.unwrap();
    assert!(logs
        .iter()
        .any(|l| l.action == "approval.self_approval_rejected"));
    assert!(logs.iter().any(|l| {
        l.action == "approval.resolved"
            && l.actor_id.as_deref() == Some("bob@security.local")
            && l.details["author_id"] == "alice@engineering.local"
            && l.details["approver_id"] == "bob@security.local"
    }));

    let evs = events.events();
    let diff_ev = evs
        .iter()
        .find(|e| e.event_type == RuntimeEventType::SandboxFileDiff)
        .expect("Expected SANDBOX_FILE_DIFF event");
    assert_eq!(diff_ev.payload["path"], "config.txt");
    assert!(diff_ev.payload["diff"]
        .as_str()
        .unwrap()
        .contains("+v2_approved"));
}

#[tokio::test]
async fn test_extended_tools_list_dir_write_file_and_native_sandbox() {
    let ws_dir = tempdir().unwrap();
    let outside_dir = tempdir().unwrap();
    fs::write(ws_dir.path().join(".gitignore"), "*.tmp\nignored_dir/\n").unwrap();
    fs::write(ws_dir.path().join(".env"), "TOP_SECRET=123\n").unwrap();
    fs::create_dir_all(ws_dir.path().join(".git")).unwrap();
    fs::write(ws_dir.path().join(".git/HEAD"), "ref: refs/heads/main\n").unwrap();
    fs::create_dir_all(ws_dir.path().join("src")).unwrap();
    fs::write(ws_dir.path().join("src/lib.rs"), "pub fn ok() {}\n").unwrap();
    fs::write(ws_dir.path().join("skip.tmp"), "ignored").unwrap();

    #[cfg(unix)]
    symlink(outside_dir.path(), ws_dir.path().join("escape_link")).unwrap();

    let scope = make_scope();
    let list_tool = ListDirTool::new(ws_dir.path());
    let listed = list_tool
        .execute(&scope, &serde_json::json!({ "path": ".", "max_depth": 4 }))
        .await
        .unwrap();
    assert!(listed.contains("src/lib.rs"));
    assert!(!listed.contains("skip.tmp"));
    assert!(!listed.contains(".env"));
    assert!(!listed.contains(".git/HEAD"));

    let events = Arc::new(InMemoryEventSink::new());
    let write_tool = WriteFileTool::new(ws_dir.path()).with_event_sink(events.clone());

    for bad_path in ["../outside.txt", ".env", "escape_link/pwn.txt"] {
        let err = write_tool
            .execute(
                &scope,
                &serde_json::json!({ "path": bad_path, "content": "bad", "create_dirs": true }),
            )
            .await
            .unwrap_err();
        assert!(
            matches!(
                err,
                ToolError::InvalidArguments(_)
                    | ToolError::ExecutionFailed(_)
                    | ToolError::PathTraversal(_)
                    | ToolError::AccessDenied(_)
            ),
            "Expected failure for {}",
            bad_path
        );
    }

    let ok_write = write_tool
        .execute(
            &scope,
            &serde_json::json!({
                "path": "src/new_mod.rs",
                "content": "pub fn added() -> i32 { 7 }\n",
                "create_dirs": true
            }),
        )
        .await
        .unwrap();
    assert!(ok_write.contains("src/new_mod.rs"));
    assert_eq!(
        fs::read_to_string(ws_dir.path().join("src/new_mod.rs")).unwrap(),
        "pub fn added() -> i32 { 7 }\n"
    );

    let seatbelt_blocked = generate_seatbelt_profile(
        ws_dir.path(),
        &SandboxPolicy::new(ws_dir.path()).with_network(false),
    );
    assert!(seatbelt_blocked.contains("(deny network*)"));
    assert!(seatbelt_blocked.contains("(deny default)"));

    let seatbelt_net = generate_seatbelt_profile(
        ws_dir.path(),
        &SandboxPolicy::new(ws_dir.path()).with_network(true),
    );
    assert!(seatbelt_net.contains("(allow network-outbound)"));

    #[cfg(target_os = "linux")]
    {
        for backend in [SandboxBackendKind::Bwrap, SandboxBackendKind::Unshare] {
            let cmd_tool = RunCommandTool::new(ws_dir.path())
                .with_event_sink(events.clone())
                .with_backend(backend);

            let ok_json = cmd_tool
                .execute(
                    &scope,
                    &serde_json::json!({
                        "program": "sh",
                        "args": ["-c", "echo sandbox_marker > marker.txt && cat marker.txt && ls -A /home"]
                    }),
                )
                .await
                .unwrap();
            let parsed: serde_json::Value = serde_json::from_str(&ok_json).unwrap();
            assert_eq!(parsed["exit_code"], 0);
            assert_eq!(parsed["stdout"].as_str().unwrap().trim(), "sandbox_marker");

            let write_etc_json = cmd_tool
                .execute(
                    &scope,
                    &serde_json::json!({
                        "program": "sh",
                        "args": ["-c", "touch /etc/alba_escape_test 2>&1"]
                    }),
                )
                .await
                .unwrap();
            let write_etc: serde_json::Value = serde_json::from_str(&write_etc_json).unwrap();
            assert_ne!(write_etc["exit_code"], 0);

            let net_json = cmd_tool
                .execute(
                    &scope,
                    &serde_json::json!({
                        "program": "python3",
                        "args": [
                            "-c",
                            "import socket; s=socket.socket(); s.settimeout(1.0); s.connect(('1.1.1.1', 53))"
                        ],
                        "allow_network": false
                    }),
                )
                .await
                .unwrap();
            let net_res: serde_json::Value = serde_json::from_str(&net_json).unwrap();
            assert_ne!(net_res["exit_code"], 0);
        }

        let raw_output = SandboxRunner::execute(
            &SandboxPolicy::new(ws_dir.path()).with_timeout(Duration::from_secs(5)),
            "sh",
            &["-c".to_string(), "echo direct_runner".to_string()],
            None,
        )
        .await
        .unwrap();
        assert_eq!(raw_output.exit_code, 0);
        assert_eq!(raw_output.stdout.trim(), "direct_runner");
    }
}

#[tokio::test]
async fn test_resumable_dag_flow_parallel_router_approval_and_fork() {
    let ws_dir = tempdir().unwrap();
    fs::write(ws_dir.path().join("input_a.txt"), "branch A").unwrap();

    let db_dir = tempdir().unwrap();
    let db_path = db_dir.path().join("flow.db");
    let store = Arc::new(SqliteCheckpointStore::new(&db_path).unwrap());
    let events = Arc::new(InMemoryEventSink::new());
    let policy = Arc::new(DefaultApprovalPolicy::new().with_workspace_root(ws_dir.path()));

    let mut registry = ToolRegistry::new();
    register_workspace_tools(&mut registry, ws_dir.path(), Some(events.clone()));

    let mut spec = FlowSpec::new(Uuid::new_v4(), "ci-deploy-flow", "start");
    spec.add_node(FlowNode {
        id: "start".to_string(),
        kind: FlowNodeKind::Start,
    });
    let mut vars = BTreeMap::new();
    vars.insert("mode".to_string(), serde_json::json!("deploy"));
    spec.add_node(FlowNode {
        id: "prep".to_string(),
        kind: FlowNodeKind::Transform { assignments: vars },
    });
    spec.add_node(FlowNode {
        id: "branch_read".to_string(),
        kind: FlowNodeKind::ToolTask {
            tool_name: "read_file".to_string(),
            arguments: serde_json::json!({ "path": "input_a.txt" }),
            output_key: "read_out".to_string(),
        },
    });
    spec.add_node(FlowNode {
        id: "branch_cmd".to_string(),
        kind: FlowNodeKind::CommandTask {
            program: "sh".to_string(),
            args: vec!["-c".to_string(), "echo parallel_ok".to_string()],
            allow_network: false,
            output_key: "cmd_out".to_string(),
        },
    });
    spec.add_node(FlowNode {
        id: "parallel".to_string(),
        kind: FlowNodeKind::ParallelBranch {
            branch_nodes: vec!["branch_read".to_string(), "branch_cmd".to_string()],
            output_key: "parallel_results".to_string(),
        },
    });
    spec.add_node(FlowNode {
        id: "router".to_string(),
        kind: FlowNodeKind::Router {
            routes: vec![FlowRouteRule {
                state_key: "mode".to_string(),
                equals: serde_json::json!("deploy"),
                target_node_id: "gate".to_string(),
            }],
            default_node_id: "end_dry".to_string(),
        },
    });
    spec.add_node(FlowNode {
        id: "gate".to_string(),
        kind: FlowNodeKind::ApprovalGate {
            description: "Approve production artifact write".to_string(),
            intent_kind: ActionIntentKind::DestructiveCommand,
        },
    });
    spec.add_node(FlowNode {
        id: "apply".to_string(),
        kind: FlowNodeKind::ToolTask {
            tool_name: "write_file".to_string(),
            arguments: serde_json::json!({ "path": "deployed.txt", "content": "done\n" }),
            output_key: "write_out".to_string(),
        },
    });
    spec.add_node(FlowNode {
        id: "end_deploy".to_string(),
        kind: FlowNodeKind::End {
            output_key: Some("parallel_results".to_string()),
        },
    });
    spec.add_node(FlowNode {
        id: "end_dry".to_string(),
        kind: FlowNodeKind::End {
            output_key: Some("mode".to_string()),
        },
    });

    spec.add_edge("start", "prep");
    spec.add_edge("prep", "parallel");
    spec.add_edge("parallel", "router");
    spec.add_edge("gate", "apply");
    spec.add_edge("apply", "end_deploy");

    let executor = FlowExecutor::new(
        store.clone(),
        events.clone(),
        policy.clone(),
        registry,
        ws_dir.path(),
    )
    .with_author_id("flow-author");
    let scope = make_scope();
    let paused = executor
        .start_flow(&spec, scope, BTreeMap::new())
        .await
        .unwrap();

    let approval_id = match paused {
        FlowOutcome::Paused {
            node_id,
            approval_id,
            ..
        } => {
            assert_eq!(node_id, "gate");
            approval_id
        }
        other => panic!("Expected Paused at gate, got {:?}", other),
    };

    let steps_before = store.list_workflow_steps(&scope).await.unwrap();
    let parallel_seq = steps_before
        .iter()
        .find(|s| s.node_id == "parallel")
        .unwrap()
        .sequence;

    store
        .resolve_approval(
            &scope,
            approval_id,
            ApprovalDecision::AllowOnce,
            "flow-approver",
        )
        .await
        .unwrap();

    let mut fresh_registry = ToolRegistry::new();
    register_workspace_tools(&mut fresh_registry, ws_dir.path(), Some(events.clone()));
    let restarted_executor = FlowExecutor::new(
        store.clone(),
        events.clone(),
        policy,
        fresh_registry,
        ws_dir.path(),
    );
    let paused_apply = restarted_executor.resume_flow(&spec, scope).await.unwrap();
    let apply_approval_id = match paused_apply {
        FlowOutcome::Paused {
            node_id,
            approval_id,
            ..
        } => {
            assert_eq!(node_id, "apply");
            approval_id
        }
        other => panic!("Expected Paused at apply write_file, got {:?}", other),
    };
    store
        .resolve_approval(
            &scope,
            apply_approval_id,
            ApprovalDecision::AllowOnce,
            "flow-approver",
        )
        .await
        .unwrap();

    let done = restarted_executor.resume_flow(&spec, scope).await.unwrap();

    match done {
        FlowOutcome::Completed(val) => {
            assert!(val["branch_read"].as_str().unwrap().contains("branch A"));
        }
        other => panic!("Expected Completed flow, got {:?}", other),
    }
    assert_eq!(
        fs::read_to_string(ws_dir.path().join("deployed.txt")).unwrap(),
        "done\n"
    );

    let mut patch = BTreeMap::new();
    patch.insert("mode".to_string(), serde_json::json!("dry_run"));
    let forked_scope = restarted_executor
        .rewind_and_fork_flow(scope, parallel_seq, None, patch)
        .await
        .unwrap();
    let forked_out = restarted_executor
        .resume_flow(&spec, forked_scope)
        .await
        .unwrap();
    assert_eq!(
        forked_out,
        FlowOutcome::Completed(serde_json::json!("dry_run"))
    );
}

#[tokio::test]
async fn test_time_travel_trajectory_fork_engine_and_cli() {
    let server = TestHttpServer::start().await;
    server.enqueue_response(MockResponse::tool_call(
        "call_tt_1",
        "read_file",
        serde_json::json!({ "path": "data.txt" }),
    ));
    server.enqueue_response(MockResponse::final_answer("Original branch answer."));
    server.enqueue_response(MockResponse::final_answer("Branched A/B answer."));
    server.enqueue_response(MockResponse::final_answer("CLI forked answer."));

    let ws_dir = tempdir().unwrap();
    fs::write(ws_dir.path().join("data.txt"), "time travel seed").unwrap();
    let db_dir = tempdir().unwrap();
    let db_path = db_dir.path().join("time_travel.db");

    let store = Arc::new(SqliteCheckpointStore::new(&db_path).unwrap());
    let events = Arc::new(InMemoryEventSink::new());
    let policy = Arc::new(DefaultApprovalPolicy::new());
    let mut registry = ToolRegistry::new();
    register_workspace_tools(&mut registry, ws_dir.path(), Some(events.clone()));

    let model_cfg = ModelConfig::new(server.url(), "key", "base-model");
    let client = Arc::new(OpenAiCompatibleClient::new(model_cfg).unwrap());
    let engine = AgentEngine::new(client, store.clone(), events.clone(), policy, registry);

    let scope = make_scope();
    let original_res = engine
        .start_run_as(scope, "Read data.txt first.", Some("alice"))
        .await
        .unwrap();
    assert_eq!(
        original_res,
        EngineOutcome::Completed("Original branch answer.".to_string())
    );

    let cps = store.list_checkpoints(&scope).await.unwrap();
    assert!(cps.len() >= 2);
    let target_seq = cps[1].sequence;

    let (fork_cp, branch) = engine
        .fork_trajectory(ForkRunRequest {
            source_scope: scope,
            target_sequence: target_seq,
            new_run_id: None,
            branch_label: "counterfactual-b".to_string(),
            model_override: Some("fast-alt-model".to_string()),
            prompt_override: Some("Answer concisely for branch B.".to_string()),
            author_id: Some("alice".to_string()),
        })
        .await
        .unwrap();
    assert_eq!(branch.branch_label, "counterfactual-b");

    let forked_outcome = engine.resume_run(fork_cp.scope).await.unwrap();
    assert_eq!(
        forked_outcome,
        EngineOutcome::Completed("Branched A/B answer.".to_string())
    );

    let reqs = server.received_requests();
    assert!(reqs.iter().any(|r| r.contains("fast-alt-model")));

    let foreign_scope =
        ExecutionScope::new(Uuid::new_v4(), scope.workspace_id, scope.run_id).unwrap();
    let isolated = engine
        .fork_trajectory(ForkRunRequest {
            source_scope: foreign_scope,
            target_sequence: target_seq,
            new_run_id: None,
            branch_label: "bad".to_string(),
            model_override: None,
            prompt_override: None,
            author_id: None,
        })
        .await;
    assert!(isolated.is_err());

    let bin_path = env!("CARGO_BIN_EXE_nexus-albaai");
    let cli_fork_run_id = Uuid::new_v4();
    let fork_cli = Command::new(bin_path)
        .env("NEXUS_TENANT_ID", scope.tenant_id.to_string())
        .env("NEXUS_WORKSPACE_ID", scope.workspace_id.to_string())
        .args([
            "fork",
            "--run-id",
            &scope.run_id.to_string(),
            "--sequence",
            &target_seq.to_string(),
            "--new-run-id",
            &cli_fork_run_id.to_string(),
            "--branch-label",
            "cli-branch",
            "--prompt-override",
            "CLI fork instruction",
            "--db-path",
            db_path.to_str().unwrap(),
        ])
        .output()
        .await
        .unwrap();
    assert!(
        fork_cli.status.success(),
        "cli fork failed: {}",
        String::from_utf8_lossy(&fork_cli.stderr)
    );
    assert!(String::from_utf8_lossy(&fork_cli.stdout).contains("cli-branch"));

    let resume_cli = Command::new(bin_path)
        .env("NEXUS_TENANT_ID", scope.tenant_id.to_string())
        .env("NEXUS_WORKSPACE_ID", scope.workspace_id.to_string())
        .env("NEXUS_API_KEY", "key")
        .args([
            "resume",
            "--run-id",
            &cli_fork_run_id.to_string(),
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
    assert!(
        resume_cli.status.success(),
        "cli resume after fork failed: {}",
        String::from_utf8_lossy(&resume_cli.stderr)
    );
    assert!(String::from_utf8_lossy(&resume_cli.stdout).contains("CLI forked answer."));
}
