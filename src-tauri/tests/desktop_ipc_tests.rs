use std::collections::VecDeque;
use std::fs;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use uuid::Uuid;

use alba_agent_desktop_lib::service::{
    ApproveActionPayload, DesktopEngineService, ForkTrajectoryPayload, StartRunPayload,
};

struct LocalMockServer {
    addr: SocketAddr,
    responses: Arc<Mutex<VecDeque<String>>>,
}

impl LocalMockServer {
    async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let responses = Arc::new(Mutex::new(VecDeque::<String>::new()));
        let queue = responses.clone();

        tokio::spawn(async move {
            loop {
                let Ok((mut socket, _)) = listener.accept().await else {
                    break;
                };
                let q = queue.clone();
                tokio::spawn(async move {
                    let mut buf = vec![0u8; 65536];
                    let _ = socket.read(&mut buf).await;
                    let body = q.lock().unwrap().pop_front().unwrap_or_else(|| {
                        serde_json::json!({
                            "choices": [{ "message": { "role": "assistant", "content": "fallback" } }]
                        })
                        .to_string()
                    });
                    let resp = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        body.len(),
                        body
                    );
                    let _ = socket.write_all(resp.as_bytes()).await;
                    let _ = socket.shutdown().await;
                });
            }
        });

        Self { addr, responses }
    }

    fn url(&self) -> String {
        format!("http://{}", self.addr)
    }

    fn enqueue_tool_call(&self, id: &str, name: &str, args: serde_json::Value) {
        let body = serde_json::json!({
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": id,
                        "type": "function",
                        "function": {
                            "name": name,
                            "arguments": args.to_string()
                        }
                    }]
                }
            }]
        })
        .to_string();
        self.responses.lock().unwrap().push_back(body);
    }

    fn enqueue_final(&self, content: &str) {
        let body = serde_json::json!({
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": content
                }
            }]
        })
        .to_string();
        self.responses.lock().unwrap().push_back(body);
    }
}

#[tokio::test]
async fn test_desktop_ipc_approval_diff_separation_and_time_travel_fork() {
    let server = LocalMockServer::start().await;
    server.enqueue_tool_call(
        "call_desktop_write",
        "write_file",
        serde_json::json!({
            "path": "notes/plan.txt",
            "content": "line 1\nline 2 desktop\n"
        }),
    );
    server.enqueue_final("Wrote plan.txt safely.");
    server.enqueue_final("Forked trajectory finished.");

    let ws_dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(ws_dir.path().join("notes")).unwrap();
    fs::write(ws_dir.path().join("notes/plan.txt"), "line 1\n").unwrap();

    let db_dir = tempfile::tempdir().unwrap();
    let db_path = db_dir.path().join("desktop_ipc.db");

    let captured_events = Arc::new(Mutex::new(Vec::new()));
    let cap_clone = captured_events.clone();
    let service = DesktopEngineService::new(Some(Arc::new(move |ev| {
        cap_clone.lock().unwrap().push(ev);
    })));

    let tenant_id = Uuid::new_v4().to_string();
    let workspace_id = Uuid::new_v4().to_string();
    let ws_str = ws_dir.path().to_str().unwrap().to_string();
    let db_str = db_path.to_str().unwrap().to_string();

    let start_out = service
        .start_run(StartRunPayload {
            workspace_root: ws_str.clone(),
            prompt: "Update notes/plan.txt".to_string(),
            model: Some("test-model".to_string()),
            api_base: Some(server.url()),
            api_key: Some("desktop_key".to_string()),
            author_id: "alice@alba.local".to_string(),
            require_approval_for_reads: Some(false),
            tenant_id: Some(tenant_id.clone()),
            workspace_id: Some(workspace_id.clone()),
            run_id: None,
            db_path: Some(db_str.clone()),
        })
        .await
        .unwrap();

    assert_eq!(start_out.status, "paused_for_approval");
    let pending = start_out.pending_approval.unwrap();
    assert_eq!(pending.tool_name, "write_file");
    assert_eq!(pending.author_id.as_deref(), Some("alice@alba.local"));
    assert_eq!(pending.path.as_deref(), Some("notes/plan.txt"));
    let diff = pending.diff.unwrap();
    assert!(diff.contains("+line 2 desktop"));

    let self_approve_err = service
        .approve_action(ApproveActionPayload {
            tenant_id: tenant_id.clone(),
            workspace_id: workspace_id.clone(),
            run_id: start_out.run_id.clone(),
            approval_id: pending.id.clone(),
            decision: "allow_once".to_string(),
            approver_id: "alice@alba.local".to_string(),
            db_path: Some(db_str.clone()),
            resume_after: Some(true),
            workspace_root: Some(ws_str.clone()),
            model: Some("test-model".to_string()),
            api_base: Some(server.url()),
            api_key: Some("desktop_key".to_string()),
        })
        .await
        .unwrap_err();
    assert!(self_approve_err.contains("Separation of duties"));

    let approved_out = service
        .approve_action(ApproveActionPayload {
            tenant_id: tenant_id.clone(),
            workspace_id: workspace_id.clone(),
            run_id: start_out.run_id.clone(),
            approval_id: pending.id,
            decision: "allow_once".to_string(),
            approver_id: "bob@alba.local".to_string(),
            db_path: Some(db_str.clone()),
            resume_after: Some(true),
            workspace_root: Some(ws_str.clone()),
            model: Some("test-model".to_string()),
            api_base: Some(server.url()),
            api_key: Some("desktop_key".to_string()),
        })
        .await
        .unwrap();

    assert_eq!(approved_out.status, "completed");
    assert_eq!(
        approved_out.message.as_deref(),
        Some("Wrote plan.txt safely.")
    );

    let file_view = service
        .read_workspace_file(&ws_str, "notes/plan.txt")
        .unwrap();
    assert_eq!(file_view.content, "line 1\nline 2 desktop\n");

    let entries = service.list_workspace_files(&ws_str, Some(4)).unwrap();
    assert!(entries.iter().any(|e| e.path == "notes/plan.txt"));

    let audit_logs = service
        .get_audit_logs(&tenant_id, &workspace_id, &start_out.run_id, Some(&db_str))
        .await
        .unwrap();
    assert!(audit_logs
        .iter()
        .any(|a| a.event_type == "approval.self_approval_rejected"));
    assert!(audit_logs.iter().any(|a| {
        a.event_type == "approval.resolved"
            && a.actor.as_deref() == Some("bob@alba.local")
            && a.details_json.contains("alice@alba.local")
    }));

    let cps = service
        .get_checkpoints(&tenant_id, &workspace_id, &start_out.run_id, Some(&db_str))
        .await
        .unwrap();
    assert!(cps.len() >= 2);

    let fork_res = service
        .fork_trajectory(ForkTrajectoryPayload {
            tenant_id: tenant_id.clone(),
            workspace_id: workspace_id.clone(),
            source_run_id: start_out.run_id.clone(),
            checkpoint_sequence: cps[0].sequence,
            branch_label: "desktop-alt".to_string(),
            override_prompt: Some("Summarize only".to_string()),
            override_model: Some("alt-model".to_string()),
            author_id: "alice@alba.local".to_string(),
            db_path: Some(db_str.clone()),
            auto_resume: Some(true),
            workspace_root: Some(ws_str),
            api_base: Some(server.url()),
            api_key: Some("desktop_key".to_string()),
        })
        .await
        .unwrap();

    assert_eq!(fork_res.branch.branch_label, "desktop-alt");
    assert_eq!(
        fork_res.outcome.unwrap().message.as_deref(),
        Some("Forked trajectory finished.")
    );

    let branches = service
        .get_branches(&tenant_id, &workspace_id, &start_out.run_id, Some(&db_str))
        .await
        .unwrap();
    assert_eq!(branches.len(), 1);

    let event_types: Vec<String> = captured_events
        .lock()
        .unwrap()
        .iter()
        .map(|e| e.event_type.as_str().to_string())
        .collect();
    assert!(event_types.iter().any(|t| t == "APPROVAL_REQUIRED"));
    assert!(event_types.iter().any(|t| t == "SANDBOX_FILE_DIFF"));
    assert!(event_types.iter().any(|t| t == "TRAJECTORY_FORKED"));
    assert!(event_types.iter().any(|t| t == "RUN_COMPLETED"));
}
