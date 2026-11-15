use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

use nexus_albaai::checkpoints::SqliteCheckpointStore;
use nexus_albaai::schema::node::{ApprovalGateNodeSpec, EndNodeSpec, StartNodeSpec, WorkflowNode};
use nexus_albaai::schema::workflow::{WorkflowEdgeSpec, WorkflowModesSpec, WorkflowSpec};
use nexus_albaai::server::state::AppState;
use reqwest::header::AUTHORIZATION;
use serde_json::Value;
use uuid::Uuid;

fn get_desktop_bin_path() -> PathBuf {
    if let Ok(target_dir) = std::env::var("CARGO_TARGET_DIR") {
        PathBuf::from(target_dir).join("debug/alba-agent")
    } else {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/debug/alba-agent")
    }
}

async fn wait_for_port(port_file: &PathBuf) -> u16 {
    for _ in 0..60 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        if port_file.exists() {
            if let Ok(content) = std::fs::read_to_string(port_file) {
                if let Ok(p) = content.trim().parse::<u16>() {
                    return p;
                }
            }
        }
    }
    panic!("timed out waiting for port file at {:?}", port_file);
}

#[tokio::test]
async fn test_second_launch_focuses_first_and_exits_cleanly() {
    let tmp = tempfile::tempdir().unwrap();
    let data_dir = tmp.path().to_path_buf();
    let static_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("web/out");
    let bin_path = get_desktop_bin_path();

    assert!(bin_path.exists());

    let mut child1 = Command::new(&bin_path)
        .env("ALBA_DATA_DIR", &data_dir)
        .env("ALBA_STATIC_DIR", &static_dir)
        .spawn()
        .expect("failed to spawn child1");

    let port_file = data_dir.join("backend_port");
    let port1 = wait_for_port(&port_file).await;

    let client = reqwest::Client::new();
    let h1 = client
        .get(format!("http://127.0.0.1:{}/health", port1))
        .send()
        .await
        .unwrap();
    assert_eq!(h1.status(), reqwest::StatusCode::OK);

    let mut child2 = Command::new(&bin_path)
        .env("ALBA_DATA_DIR", &data_dir)
        .env("ALBA_STATIC_DIR", &static_dir)
        .spawn()
        .expect("failed to spawn child2");

    let exit_status2 = child2.wait().expect("child2 failed to wait");
    assert!(
        exit_status2.success(),
        "second launch must exit with code 0"
    );

    let h2 = client
        .get(format!("http://127.0.0.1:{}/health", port1))
        .send()
        .await
        .unwrap();
    assert_eq!(
        h2.status(),
        reqwest::StatusCode::OK,
        "first instance must stay alive after second instance exits"
    );

    let _ = child1.kill();
    let _ = child1.wait();
}

#[tokio::test]
async fn test_sigkill_sqlite_integrity_and_run_resumption() {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("crash_test.db");
    let store = Arc::new(SqliteCheckpointStore::new(&db_path).unwrap());
    let workspace_root = tmp.path().to_path_buf();

    let wf_id = Uuid::new_v4();
    let spec = WorkflowSpec {
        id: wf_id,
        tenant_id: Uuid::new_v4(),
        workspace_id: Uuid::new_v4(),
        name: "Crash Resilience Workflow".to_string(),
        version: 1,
        nodes: vec![
            WorkflowNode::Start(StartNodeSpec {
                id: "start".to_string(),
                label: "Start".to_string(),
                trigger_type: "manual".to_string(),
                input_schema: serde_json::Value::Null,
                schedule: None,
            }),
            WorkflowNode::Approval(ApprovalGateNodeSpec {
                id: "gate".to_string(),
                label: "Approve Payment".to_string(),
                action_description: "Approve payment processing".to_string(),
                required_role: "admin".to_string(),
                timeout_seconds: 3600,
                boundary_rules: vec![],
            }),
            WorkflowNode::End(EndNodeSpec {
                id: "end".to_string(),
                label: "End".to_string(),
            }),
        ],
        edges: vec![
            WorkflowEdgeSpec {
                source: "start".to_string(),
                target: "gate".to_string(),
                route: None,
                loop_back: false,
            },
            WorkflowEdgeSpec {
                source: "gate".to_string(),
                target: "end".to_string(),
                route: None,
                loop_back: false,
            },
        ],
        learning_edges: vec![],
        layout: BTreeMap::new(),
        modes: WorkflowModesSpec::default(),
    };

    let state = AppState::new(store.clone(), workspace_root.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let base_url = format!("http://127.0.0.1:{}", port);

    let server_handle =
        tokio::spawn(async move { nexus_albaai::server::serve_app(state, listener).await });

    let client = reqwest::Client::new();
    let session_res = client
        .post(format!("{}/api/v1/auth/local-session", base_url))
        .send()
        .await
        .unwrap();
    let token = session_res.json::<Value>().await.unwrap()["access_token"]
        .as_str()
        .unwrap()
        .to_string();

    let create_res = client
        .post(format!("{}/api/v1/workflows", base_url))
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .json(&spec)
        .send()
        .await
        .unwrap();
    assert_eq!(create_res.status(), reqwest::StatusCode::CREATED);

    let run_res = client
        .post(format!("{}/api/v1/workflows/{}/runs", base_url, wf_id))
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .json(&serde_json::json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(run_res.status(), reqwest::StatusCode::ACCEPTED);
    let run_id = run_res.json::<Value>().await.unwrap()["run_id"]
        .as_str()
        .unwrap()
        .to_string();

    let mut reached_gate = false;
    for _ in 0..50 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        let r = client
            .get(format!("{}/api/v1/runs/{}", base_url, run_id))
            .header(AUTHORIZATION, format!("Bearer {}", token))
            .send()
            .await
            .unwrap();
        if r.status() == reqwest::StatusCode::OK {
            let body: Value = r.json().await.unwrap();
            let status = body["status"].as_str().unwrap_or("");
            if status == "AWAITING_APPROVAL" || status == "PAUSED" {
                reached_gate = true;
                break;
            }
        }
    }
    assert!(reached_gate, "run must pause at approval gate");

    server_handle.abort();
    drop(store);

    {
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        let integrity: String = conn
            .query_row("PRAGMA integrity_check;", [], |row| row.get(0))
            .unwrap();
        assert_eq!(integrity, "ok");
    }

    let store2 = Arc::new(SqliteCheckpointStore::new(&db_path).unwrap());
    let state2 = AppState::new(store2.clone(), workspace_root);
    let listener2 = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port2 = listener2.local_addr().unwrap().port();
    let base_url2 = format!("http://127.0.0.1:{}", port2);

    tokio::spawn(async move { nexus_albaai::server::serve_app(state2, listener2).await });

    let approvals_res = client
        .get(format!("{}/api/v1/approvals", base_url2))
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .send()
        .await
        .unwrap();
    assert_eq!(approvals_res.status(), reqwest::StatusCode::OK);
    let approvals: Vec<Value> = approvals_res.json().await.unwrap();
    assert!(!approvals.is_empty(), "pending approval must survive crash");
    let approval_id = approvals[0]["id"].as_str().unwrap();

    let resolve_res = client
        .post(format!(
            "{}/api/v1/approvals/{}/resolve",
            base_url2, approval_id
        ))
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .header("x-actor-id", "manager-reviewer-1")
        .json(&serde_json::json!({
            "decision": "allow_once"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resolve_res.status(), reqwest::StatusCode::OK);

    let mut resumed_and_completed = false;
    for _ in 0..50 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        let r = client
            .get(format!("{}/api/v1/runs/{}", base_url2, run_id))
            .header(AUTHORIZATION, format!("Bearer {}", token))
            .send()
            .await
            .unwrap();
        if r.status() == reqwest::StatusCode::OK {
            let body: Value = r.json().await.unwrap();
            if body["status"] == "SUCCEEDED" {
                resumed_and_completed = true;
                break;
            }
        }
    }
    assert!(resumed_and_completed, "run must complete after resumption");

    let final_run = store2.get_run(&run_id).unwrap().unwrap();
    assert_eq!(final_run.status, "SUCCEEDED");
}
