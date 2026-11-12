use nexus_albaai::checkpoints::SqliteCheckpointStore;
use nexus_albaai::schema::node::{EndNodeSpec, StartNodeSpec, WorkflowNode};
use nexus_albaai::schema::workflow::{WorkflowEdgeSpec, WorkflowModesSpec, WorkflowSpec};
use nexus_albaai::server::state::AppState;
use reqwest::header::AUTHORIZATION;
use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

async fn spawn_test_server() -> (String, tempfile::TempDir) {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("test_contract.db");
    let store = Arc::new(SqliteCheckpointStore::new(&db_path).unwrap());
    let state = AppState::new(store, tmp.path().to_path_buf());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let base_url = format!("http://{}", addr);
    tokio::spawn(async move {
        nexus_albaai::server::serve_app(state, listener)
            .await
            .unwrap();
    });
    (base_url, tmp)
}

fn create_valid_workflow(id: Uuid) -> WorkflowSpec {
    WorkflowSpec {
        id,
        tenant_id: Uuid::new_v4(),
        workspace_id: Uuid::new_v4(),
        name: "Test Automation Workflow".to_string(),
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
async fn test_workflows_crud_and_runs_e2e() {
    let (base_url, _tmp) = spawn_test_server().await;
    let client = reqwest::Client::new();

    let session_res = client
        .post(format!("{}/api/v1/auth/local-session", base_url))
        .send()
        .await
        .unwrap();
    let session_body: Value = session_res.json().await.unwrap();
    let token = session_body["access_token"].as_str().unwrap();

    let list_res = client
        .get(format!("{}/api/v1/workflows", base_url))
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .send()
        .await
        .unwrap();
    assert_eq!(list_res.status(), reqwest::StatusCode::OK);
    let list: Vec<Value> = list_res.json().await.unwrap();
    assert_eq!(list.len(), 0);

    let wf_id = Uuid::new_v4();
    let spec = create_valid_workflow(wf_id);

    let create_res = client
        .post(format!("{}/api/v1/workflows", base_url))
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .json(&spec)
        .send()
        .await
        .unwrap();
    assert_eq!(create_res.status(), reqwest::StatusCode::CREATED);
    let created_body: WorkflowSpec = create_res.json().await.unwrap();
    assert_eq!(created_body.id, wf_id);
    assert_eq!(created_body.name, "Test Automation Workflow");

    let list_res2 = client
        .get(format!("{}/api/v1/workflows", base_url))
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .send()
        .await
        .unwrap();
    let list2: Vec<Value> = list_res2.json().await.unwrap();
    assert_eq!(list2.len(), 1);
    assert_eq!(list2[0]["id"], wf_id.to_string());

    let get_res = client
        .get(format!("{}/api/v1/workflows/{}", base_url, wf_id))
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .send()
        .await
        .unwrap();
    assert_eq!(get_res.status(), reqwest::StatusCode::OK);
    let get_body: WorkflowSpec = get_res.json().await.unwrap();
    assert_eq!(get_body.id, wf_id);

    let mut updated_spec = spec.clone();
    updated_spec.name = "Updated Workflow Name".to_string();

    let put_res = client
        .put(format!("{}/api/v1/workflows/{}", base_url, wf_id))
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .json(&updated_spec)
        .send()
        .await
        .unwrap();
    assert_eq!(put_res.status(), reqwest::StatusCode::OK);
    let put_body: WorkflowSpec = put_res.json().await.unwrap();
    assert_eq!(put_body.name, "Updated Workflow Name");

    let run_create_res = client
        .post(format!("{}/api/v1/workflows/{}/runs", base_url, wf_id))
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .json(&serde_json::json!({
            "input_data": {"test_key": "test_val"}
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(run_create_res.status(), reqwest::StatusCode::ACCEPTED);
    let run_create_body: Value = run_create_res.json().await.unwrap();
    let run_id = run_create_body["run_id"].as_str().unwrap();
    assert_eq!(run_create_body["workflow_id"], wf_id.to_string());
    assert_eq!(run_create_body["status"], "QUEUED");

    let runs_res = client
        .get(format!("{}/api/v1/runs", base_url))
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .send()
        .await
        .unwrap();
    assert_eq!(runs_res.status(), reqwest::StatusCode::OK);
    let runs_list: Vec<Value> = runs_res.json().await.unwrap();
    assert_eq!(runs_list.len(), 1);
    assert_eq!(runs_list[0]["id"], run_id);

    tokio::time::sleep(Duration::from_millis(150)).await;

    let run_detail_res = client
        .get(format!("{}/api/v1/runs/{}", base_url, run_id))
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .send()
        .await
        .unwrap();
    assert_eq!(run_detail_res.status(), reqwest::StatusCode::OK);
    let run_detail: Value = run_detail_res.json().await.unwrap();
    assert_eq!(run_detail["id"], run_id);
    assert_eq!(run_detail["workflow_id"], wf_id.to_string());
    assert_eq!(run_detail["status"], "SUCCEEDED");
    assert!(run_detail["completed_at"].is_string());
}

#[tokio::test]
async fn test_workflow_not_found_returns_404() {
    let (base_url, _tmp) = spawn_test_server().await;
    let client = reqwest::Client::new();
    let session_res = client
        .post(format!("{}/api/v1/auth/local-session", base_url))
        .send()
        .await
        .unwrap();
    let session_body: Value = session_res.json().await.unwrap();
    let token = session_body["access_token"].as_str().unwrap();

    let res = client
        .get(format!("{}/api/v1/workflows/nonexistent-id", base_url))
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), reqwest::StatusCode::NOT_FOUND);
    let body: Value = res.json().await.unwrap();
    assert_eq!(body["detail"], "Workflow not found");
}

#[tokio::test]
async fn test_run_not_found_returns_404() {
    let (base_url, _tmp) = spawn_test_server().await;
    let client = reqwest::Client::new();
    let session_res = client
        .post(format!("{}/api/v1/auth/local-session", base_url))
        .send()
        .await
        .unwrap();
    let session_body: Value = session_res.json().await.unwrap();
    let token = session_body["access_token"].as_str().unwrap();

    let res = client
        .get(format!("{}/api/v1/runs/nonexistent-id", base_url))
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), reqwest::StatusCode::NOT_FOUND);
    let body: Value = res.json().await.unwrap();
    assert_eq!(body["detail"], "Run not found");
}

#[tokio::test]
async fn test_invalid_workflow_creation_returns_400() {
    let (base_url, _tmp) = spawn_test_server().await;
    let client = reqwest::Client::new();
    let session_res = client
        .post(format!("{}/api/v1/auth/local-session", base_url))
        .send()
        .await
        .unwrap();
    let session_body: Value = session_res.json().await.unwrap();
    let token = session_body["access_token"].as_str().unwrap();

    let mut invalid_spec = create_valid_workflow(Uuid::new_v4());
    invalid_spec
        .nodes
        .retain(|n| !matches!(n, WorkflowNode::Start(_)));
    invalid_spec.edges.clear();

    let res = client
        .post(format!("{}/api/v1/workflows", base_url))
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .json(&invalid_spec)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), reqwest::StatusCode::BAD_REQUEST);
    let body: Value = res.json().await.unwrap();
    assert!(body["detail"].as_str().unwrap().contains("Start node"));
}

#[tokio::test]
async fn test_contract_capsules_endpoint() {
    let (base_url, _tmp) = spawn_test_server().await;
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

    let res = client
        .get(format!("{}/api/v1/learning-transfer/capsules", base_url))
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), reqwest::StatusCode::OK);
    let body: Vec<Value> = res.json().await.unwrap();
    assert!(!body.is_empty());
    assert!(body[0]["trigger_pattern"].is_string());
}

#[tokio::test]
async fn test_contract_settings_endpoints() {
    let (base_url, _tmp) = spawn_test_server().await;
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

    let models_res = client
        .get(format!("{}/api/v1/settings/chat-models", base_url))
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .send()
        .await
        .unwrap();
    assert_eq!(models_res.status(), reqwest::StatusCode::OK);
    let models_body: Value = models_res.json().await.unwrap();
    assert!(models_body["models"].is_array());

    let policy_get = client
        .get(format!("{}/api/v1/settings/execution", base_url))
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .send()
        .await
        .unwrap();
    assert_eq!(policy_get.status(), reqwest::StatusCode::OK);

    let policy_put = client
        .put(format!("{}/api/v1/settings/execution", base_url))
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .json(&serde_json::json!({
            "default_mode": "ask_all",
            "allowed_domains": ["api.openai.com"],
            "denied_domains": ["untrusted.com"]
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(policy_put.status(), reqwest::StatusCode::OK);
    let put_body: Value = policy_put.json().await.unwrap();
    assert_eq!(put_body["default_mode"], "ask_all");
}

#[tokio::test]
async fn test_contract_tools_builtin_endpoint() {
    let (base_url, _tmp) = spawn_test_server().await;
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

    let res = client
        .get(format!("{}/api/v1/tools/builtin", base_url))
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), reqwest::StatusCode::OK);
    let tools: Vec<Value> = res.json().await.unwrap();
    assert!(tools.iter().any(|t| t["id"] == "file_read"));
    assert!(tools.iter().any(|t| t["id"] == "file_write"));
}
