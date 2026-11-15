use std::collections::BTreeMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use nexus_albaai::checkpoints::SqliteCheckpointStore;
use nexus_albaai::schema::node::{EndNodeSpec, StartNodeSpec, WorkflowNode};
use nexus_albaai::schema::workflow::{WorkflowEdgeSpec, WorkflowModesSpec, WorkflowSpec};
use nexus_albaai::server::state::AppState;
use reqwest::header::AUTHORIZATION;
use serde_json::Value;
use uuid::Uuid;

#[tokio::test]
async fn test_loopback_only_binding_and_non_loopback_refused() {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("hardening_test.db");
    let store = Arc::new(SqliteCheckpointStore::new(&db_path).unwrap());
    let state = AppState::new(store, tmp.path().to_path_buf());

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let local_addr = listener.local_addr().unwrap();
    assert!(
        local_addr.ip().is_loopback(),
        "listener IP must be loopback"
    );
    let port = local_addr.port();

    tokio::spawn(async move {
        nexus_albaai::server::serve_app(state, listener)
            .await
            .unwrap();
    });

    let loopback_conn =
        tokio::net::TcpStream::connect(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port))
            .await;
    assert!(loopback_conn.is_ok(), "connection to loopback must succeed");

    let non_loopback_candidates = [
        Ipv4Addr::new(192, 168, 1, 1),
        Ipv4Addr::new(10, 0, 0, 1),
        Ipv4Addr::new(172, 16, 0, 1),
    ];

    for candidate in non_loopback_candidates {
        let conn_result = tokio::time::timeout(
            Duration::from_millis(100),
            tokio::net::TcpStream::connect(SocketAddr::new(IpAddr::V4(candidate), port)),
        )
        .await;
        let refused_or_timed_out = match conn_result {
            Ok(Err(_)) | Err(_) => true,
            Ok(Ok(_)) => false,
        };
        assert!(
            refused_or_timed_out,
            "non-loopback connection to {:?} must not reach loopback listener",
            candidate
        );
    }
}

#[test]
fn test_csp_configuration_strictly_enforces_local_origin() {
    let tauri_conf_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src-tauri/tauri.conf.json");
    assert!(
        tauri_conf_path.exists(),
        "tauri.conf.json must exist at {:?}",
        tauri_conf_path
    );

    let content = std::fs::read_to_string(&tauri_conf_path).unwrap();
    let json: Value = serde_json::from_str(&content).unwrap();

    let csp = json["app"]["security"]["csp"].as_str().unwrap();

    assert!(csp.contains("default-src 'self'"));
    assert!(!csp.contains("https://"));
    assert!(!csp.contains("http://*"));
    assert!(!csp.contains("cdn."));
    assert!(!csp.contains("googleapis.com"));
    assert!(!csp.contains("cloudflare.com"));
    assert!(!csp.contains("unpkg.com"));
    assert!(csp.contains("connect-src 'self' http://127.0.0.1:* ws://127.0.0.1:*"));
}

#[tokio::test]
async fn test_outbound_network_blocked_full_stack_execution() {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("offline_test.db");
    let store = Arc::new(SqliteCheckpointStore::new(&db_path).unwrap());
    let static_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("web/out");
    let state = AppState::new(store.clone(), tmp.path().to_path_buf()).with_static_dir(static_dir);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let base_url = format!("http://127.0.0.1:{}", port);

    tokio::spawn(async move {
        nexus_albaai::server::serve_app(state, listener)
            .await
            .unwrap();
    });

    let client = reqwest::Client::builder().no_proxy().build().unwrap();

    let health_res = client
        .get(format!("{}/health", base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(health_res.status(), reqwest::StatusCode::OK);

    let root_res = client.get(format!("{}/", base_url)).send().await.unwrap();
    assert_eq!(root_res.status(), reqwest::StatusCode::OK);
    let root_body = root_res.text().await.unwrap();
    assert!(root_body.contains("<!DOCTYPE html>"));

    let session_res = client
        .post(format!("{}/api/v1/auth/local-session", base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(session_res.status(), reqwest::StatusCode::OK);
    let token = session_res.json::<Value>().await.unwrap()["access_token"]
        .as_str()
        .unwrap()
        .to_string();

    let wf_id = Uuid::new_v4();
    let spec = WorkflowSpec {
        id: wf_id,
        tenant_id: Uuid::new_v4(),
        workspace_id: Uuid::new_v4(),
        name: "Offline Hardened Workflow".to_string(),
        version: 1,
        nodes: vec![
            WorkflowNode::Start(StartNodeSpec {
                id: "start".to_string(),
                label: "Start".to_string(),
                trigger_type: "manual".to_string(),
                input_schema: serde_json::Value::Null,
                schedule: None,
            }),
            WorkflowNode::End(EndNodeSpec {
                id: "end".to_string(),
                label: "End".to_string(),
            }),
        ],
        edges: vec![WorkflowEdgeSpec {
            source: "start".to_string(),
            target: "end".to_string(),
            route: None,
            loop_back: false,
        }],
        learning_edges: vec![],
        layout: BTreeMap::new(),
        modes: WorkflowModesSpec::default(),
    };

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

    let mut succeeded = false;
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
            if body["status"] == "SUCCEEDED" {
                succeeded = true;
                break;
            }
        }
    }
    assert!(
        succeeded,
        "workflow run must complete in offline hardened environment"
    );

    let run_record = store.get_run(&run_id).unwrap().unwrap();
    assert_eq!(run_record.status, "SUCCEEDED");
}
