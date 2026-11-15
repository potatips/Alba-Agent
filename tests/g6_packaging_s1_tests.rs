use std::collections::BTreeMap;
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

async fn spawn_test_server_with_static() -> (String, Arc<SqliteCheckpointStore>, tempfile::TempDir)
{
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("s1_test.db");
    let store = Arc::new(SqliteCheckpointStore::new(&db_path).unwrap());
    let static_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("web/out");
    let state = AppState::new(store.clone(), tmp.path().to_path_buf()).with_static_dir(static_dir);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let base_url = format!("http://{}", addr);

    tokio::spawn(async move {
        nexus_albaai::server::serve_app(state, listener)
            .await
            .unwrap();
    });

    (base_url, store, tmp)
}

fn create_test_workflow(id: Uuid) -> WorkflowSpec {
    WorkflowSpec {
        id,
        tenant_id: Uuid::new_v4(),
        workspace_id: Uuid::new_v4(),
        name: "Static Frontend E2E Workflow".to_string(),
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
    }
}

#[tokio::test]
async fn test_static_frontend_served_root_and_routes() {
    let (base_url, _store, _tmp) = spawn_test_server_with_static().await;
    let client = reqwest::Client::new();

    let root_res = client.get(format!("{}/", base_url)).send().await.unwrap();
    assert_eq!(root_res.status(), reqwest::StatusCode::OK);
    let content_type = root_res
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .unwrap()
        .to_str()
        .unwrap();
    assert!(content_type.contains("text/html"));
    let root_body = root_res.text().await.unwrap();
    assert!(root_body.contains("<!DOCTYPE html>"));

    let studio_res = client
        .get(format!("{}/studio", base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(studio_res.status(), reqwest::StatusCode::OK);
    let studio_body = studio_res.text().await.unwrap();
    assert!(studio_body.contains("<!DOCTYPE html>"));

    let cap_res = client
        .get(format!("{}/capabilities", base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(cap_res.status(), reqwest::StatusCode::OK);
    let cap_body = cap_res.text().await.unwrap();
    assert!(cap_body.contains("<!DOCTYPE html>"));

    let mode_res = client
        .get(format!("{}/api/v1/auth/local-mode", base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(mode_res.status(), reqwest::StatusCode::OK);
    let mode_body: Value = mode_res.json().await.unwrap();
    assert_eq!(mode_body["enabled"], true);

    let unknown_api_res = client
        .get(format!("{}/api/v1/not-a-real-endpoint", base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(unknown_api_res.status(), reqwest::StatusCode::NOT_FOUND);
    let unknown_body: Value = unknown_api_res.json().await.unwrap();
    assert_eq!(unknown_body["detail"], "Not Found");
}

#[tokio::test]
async fn test_static_frontend_e2e_workflow_run_and_sqlite_readback() {
    let (base_url, store, _tmp) = spawn_test_server_with_static().await;
    let client = reqwest::Client::new();

    let session_res = client
        .post(format!("{}/api/v1/auth/local-session", base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(session_res.status(), reqwest::StatusCode::OK);
    let session_body: Value = session_res.json().await.unwrap();
    let token = session_body["access_token"].as_str().unwrap().to_string();

    let wf_id = Uuid::new_v4();
    let spec = create_test_workflow(wf_id);

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
        .json(&serde_json::json!({
            "input_data": {"run_test_key": "run_test_value"}
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(run_res.status(), reqwest::StatusCode::ACCEPTED);
    let run_create_body: Value = run_res.json().await.unwrap();
    let run_id = run_create_body["run_id"].as_str().unwrap().to_string();

    let mut completed = false;
    for _ in 0..50 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        let get_run_res = client
            .get(format!("{}/api/v1/runs/{}", base_url, run_id))
            .header(AUTHORIZATION, format!("Bearer {}", token))
            .send()
            .await
            .unwrap();
        if get_run_res.status() == reqwest::StatusCode::OK {
            let body: Value = get_run_res.json().await.unwrap();
            if body["status"] == "SUCCEEDED" {
                completed = true;
                break;
            }
        }
    }
    assert!(completed);

    let db_run = store.get_run(&run_id).unwrap();
    assert!(db_run.is_some());
    let run_record = db_run.unwrap();
    assert_eq!(run_record.id, run_id);
    assert_eq!(run_record.workflow_id, wf_id.to_string());
    assert_eq!(run_record.status, "SUCCEEDED");
}
