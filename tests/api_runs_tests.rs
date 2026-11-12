use nexus_albaai::checkpoints::types::RunRecord;
use nexus_albaai::checkpoints::SqliteCheckpointStore;
use nexus_albaai::schema::agent::AgentSpec;
use nexus_albaai::schema::node::{
    ConsensusConfigSpec, CrewNodeSpec, CrewProcess, EndNodeSpec, JoinMode, StartNodeSpec,
    WorkflowNode,
};
use nexus_albaai::schema::task::TaskSpec;
use nexus_albaai::schema::workflow::{WorkflowEdgeSpec, WorkflowModesSpec, WorkflowSpec};
use nexus_albaai::server::state::AppState;
use nexus_albaai::server::workflow_runner::ServerModelClient;
use nexus_albaai::workflow::state::{WorkflowExecutionContext, WorkflowRunState};
use reqwest::header::AUTHORIZATION;
use reqwest::StatusCode;
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

mod common;
use common::{MockResponse, TestHttpServer};

async fn spawn_test_server() -> (
    String,
    Arc<SqliteCheckpointStore>,
    AppState,
    PathBuf,
    tempfile::TempDir,
) {
    spawn_test_server_with_model(None).await
}

async fn spawn_test_server_with_model(
    model: Option<ServerModelClient>,
) -> (
    String,
    Arc<SqliteCheckpointStore>,
    AppState,
    PathBuf,
    tempfile::TempDir,
) {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("test_runs.db");
    let workspace_root = tmp.path().join("workspace");
    std::fs::create_dir_all(&workspace_root).unwrap();
    let store = Arc::new(SqliteCheckpointStore::new(&db_path).unwrap());
    let mut state = AppState::new(store.clone(), workspace_root.clone());
    if let Some(m) = model {
        state = state.with_model(m);
    }
    let state_for_server = state.clone();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let base_url = format!("http://{}", addr);
    tokio::spawn(async move {
        nexus_albaai::server::serve_app(state_for_server, listener)
            .await
            .unwrap();
    });
    (base_url, store, state, workspace_root, tmp)
}

async fn get_token(client: &reqwest::Client, base_url: &str) -> String {
    let resp = client
        .post(format!("{}/api/v1/auth/local-session", base_url))
        .send()
        .await
        .unwrap();
    let body: Value = resp.json().await.unwrap();
    body["access_token"].as_str().unwrap().to_string()
}

fn create_crew_workflow(wf_id: Uuid, crew_id: &str) -> WorkflowSpec {
    let agent = AgentSpec {
        id: "agent_runner".to_string(),
        role: "Executor".to_string(),
        goal: "Execute tools".to_string(),
        backstory: "Experienced worker".to_string(),
        model: "default-model".to_string(),
        tool_ids: vec!["write_file".to_string()],
        skill_ids: vec![],
        max_iterations: 10,
        temperature: 0.2,
        guardrails: vec![],
        knowledge_capsule_ids: vec![],
        tier: None,
    };
    let task = TaskSpec {
        id: "task_run".to_string(),
        agent_id: "agent_runner".to_string(),
        description: "Run file write tool".to_string(),
        expected_output: "File written".to_string(),
        output_schema: "text".to_string(),
        output_schema_json: None,
        context_task_ids: vec![],
        expected_artifact_paths: vec![],
    };

    WorkflowSpec {
        id: wf_id,
        tenant_id: Uuid::parse_str(nexus_albaai::server::auth::DEFAULT_LOCAL_TENANT_ID).unwrap(),
        workspace_id: Uuid::parse_str(nexus_albaai::server::auth::DEFAULT_LOCAL_WORKSPACE_ID)
            .unwrap(),
        name: "Runs Test Workflow".to_string(),
        version: 1,
        nodes: vec![
            WorkflowNode::Start(StartNodeSpec {
                id: "start".to_string(),
                label: "Start".to_string(),
                trigger_type: "manual".to_string(),
                input_schema: serde_json::Value::Null,
                schedule: None,
            }),
            WorkflowNode::Crew(CrewNodeSpec {
                id: crew_id.to_string(),
                label: "Crew Worker".to_string(),
                agents: vec![agent],
                tasks: vec![task],
                process: CrewProcess::Sequential,
                consensus_config: ConsensusConfigSpec::default(),
                manager_agent_id: None,
                join: JoinMode::Any,
                expected_artifact_paths: vec![],
                requires_visual_evidence: false,
            }),
            WorkflowNode::End(EndNodeSpec {
                id: "end".to_string(),
                label: "End".to_string(),
            }),
        ],
        edges: vec![
            WorkflowEdgeSpec {
                source: "start".to_string(),
                target: crew_id.to_string(),
                route: None,
                loop_back: false,
            },
            WorkflowEdgeSpec {
                source: crew_id.to_string(),
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
async fn test_api_resume_consumed_tool_not_reexecuted() {
    let mock_server = TestHttpServer::start().await;
    mock_server.enqueue_response(MockResponse::tool_call(
        "call_consumed_789",
        "write_file",
        serde_json::json!({
            "path": "consumed_side_effect.txt",
            "content": "real_execution_content\n"
        }),
    ));
    mock_server.enqueue_response(MockResponse::final_answer("Task completed"));

    let client_config = nexus_albaai::models::types::ModelConfig::new(
        mock_server.url(),
        "mock-key".to_string(),
        "mock-model".to_string(),
    );
    let open_client = nexus_albaai::models::OpenAiCompatibleClient::new(client_config).unwrap();
    let server_model = ServerModelClient::with_client(open_client);

    let (base_url, store, _state, workspace_root, _tmp) =
        spawn_test_server_with_model(Some(server_model)).await;
    let client = reqwest::Client::new();
    let token = get_token(&client, &base_url).await;

    let wf_id = Uuid::new_v4();
    let crew_id = "crew_worker";
    let spec = create_crew_workflow(wf_id, crew_id);

    let wf_resp = client
        .post(format!("{}/api/v1/workflows", base_url))
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .json(&spec)
        .send()
        .await
        .unwrap();
    assert_eq!(wf_resp.status(), StatusCode::CREATED);

    let run_id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let run_rec = RunRecord {
        id: run_id.clone(),
        workflow_id: wf_id.to_string(),
        status: "PAUSED".to_string(),
        input_data: "{}".to_string(),
        output_data: None,
        created_at: now.clone(),
        updated_at: now.clone(),
        completed_at: None,
        verified: false,
    };
    store.insert_run(&run_rec).unwrap();

    let tenant_id = nexus_albaai::server::auth::DEFAULT_LOCAL_TENANT_ID;
    let workspace_id = nexus_albaai::server::auth::DEFAULT_LOCAL_WORKSPACE_ID;
    let context = WorkflowExecutionContext {
        run_id: Uuid::parse_str(&run_id).unwrap(),
        current_node_id: crew_id.to_string(),
        run_state: WorkflowRunState::Running,
        node_states: BTreeMap::new(),
        completed_nodes: vec!["start".to_string()],
        task_outputs: BTreeMap::new(),
        node_outputs: BTreeMap::new(),
        loop_counts: BTreeMap::new(),
        pending_approval_id: None,
        pending_approval_node_id: None,
        author_id: Some("agent-worker".to_string()),
        sequence: 1,
        final_output: None,
        error: None,
        updated_at: chrono::Utc::now(),
    };
    let state_json = serde_json::to_string(&context).unwrap();
    let chk_id = Uuid::new_v4().to_string();

    {
        let conn = store.connection();
        let guard = conn.lock().unwrap();
        guard.execute(
            "INSERT INTO workflow_checkpoints (id, run_id, tenant_id, workspace_id, sequence, node_id, status, state_json, created_at)
             VALUES (?1, ?2, ?3, ?4, 1, ?5, 'running', ?6, ?7)",
            rusqlite::params![chk_id, run_id, tenant_id, workspace_id, crew_id, state_json, now],
        ).unwrap();
        guard.execute(
            "INSERT INTO tool_journal (run_id, tenant_id, workspace_id, node_id, tool_call_id, tool_name, arguments_json, status, output_text, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, 'call_consumed_789', 'write_file', '{\"path\":\"consumed_side_effect.txt\"}', 'consumed', '{\"status\":\"ok\"}', ?5, ?5)",
            rusqlite::params![run_id, tenant_id, workspace_id, crew_id, now],
        ).unwrap();
    }

    let resume_resp = client
        .post(format!("{}/api/v1/runs/{}/resume", base_url, run_id))
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .send()
        .await
        .unwrap();
    assert_eq!(resume_resp.status(), StatusCode::OK);
    let resume_body: Value = resume_resp.json().await.unwrap();
    assert_eq!(resume_body["status"], "RESUMED");

    let mut finished = false;
    for _ in 0..50 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        let get_resp = client
            .get(format!("{}/api/v1/runs/{}", base_url, run_id))
            .header(AUTHORIZATION, format!("Bearer {}", token))
            .send()
            .await
            .unwrap();
        let detail: Value = get_resp.json().await.unwrap();
        let status = detail["status"].as_str().unwrap();
        if status == "SUCCEEDED" || status == "FAILED" {
            finished = true;
            assert_eq!(status, "SUCCEEDED");
            break;
        }
    }
    assert!(finished);

    let side_effect_path = workspace_root.join("consumed_side_effect.txt");
    assert!(
        !side_effect_path.exists(),
        "Consumed tool call was re-executed: side effect file must not exist"
    );
}

#[tokio::test]
async fn test_api_resume_executing_tool_fails_closed() {
    let mock_server = TestHttpServer::start().await;
    mock_server.enqueue_response(MockResponse::tool_call(
        "call_executing_456",
        "write_file",
        serde_json::json!({
            "path": "executing_side_effect.txt",
            "content": "test\n"
        }),
    ));
    mock_server.enqueue_response(MockResponse::final_answer("Task completed"));

    let client_config = nexus_albaai::models::types::ModelConfig::new(
        mock_server.url(),
        "mock-key".to_string(),
        "mock-model".to_string(),
    );
    let open_client = nexus_albaai::models::OpenAiCompatibleClient::new(client_config).unwrap();
    let server_model = ServerModelClient::with_client(open_client);

    let (base_url, store, _state, workspace_root, _tmp) =
        spawn_test_server_with_model(Some(server_model)).await;
    let client = reqwest::Client::new();
    let token = get_token(&client, &base_url).await;

    let wf_id = Uuid::new_v4();
    let crew_id = "crew_crash";
    let spec = create_crew_workflow(wf_id, crew_id);

    client
        .post(format!("{}/api/v1/workflows", base_url))
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .json(&spec)
        .send()
        .await
        .unwrap();

    let run_id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let run_rec = RunRecord {
        id: run_id.clone(),
        workflow_id: wf_id.to_string(),
        status: "PAUSED".to_string(),
        input_data: "{}".to_string(),
        output_data: None,
        created_at: now.clone(),
        updated_at: now.clone(),
        completed_at: None,
        verified: false,
    };
    store.insert_run(&run_rec).unwrap();

    let tenant_id = nexus_albaai::server::auth::DEFAULT_LOCAL_TENANT_ID;
    let workspace_id = nexus_albaai::server::auth::DEFAULT_LOCAL_WORKSPACE_ID;
    let context = WorkflowExecutionContext {
        run_id: Uuid::parse_str(&run_id).unwrap(),
        current_node_id: crew_id.to_string(),
        run_state: WorkflowRunState::Running,
        node_states: BTreeMap::new(),
        completed_nodes: vec!["start".to_string()],
        task_outputs: BTreeMap::new(),
        node_outputs: BTreeMap::new(),
        loop_counts: BTreeMap::new(),
        pending_approval_id: None,
        pending_approval_node_id: None,
        author_id: Some("agent-worker".to_string()),
        sequence: 1,
        final_output: None,
        error: None,
        updated_at: chrono::Utc::now(),
    };
    let state_json = serde_json::to_string(&context).unwrap();
    let chk_id = Uuid::new_v4().to_string();

    {
        let conn = store.connection();
        let guard = conn.lock().unwrap();
        guard.execute(
            "INSERT INTO workflow_checkpoints (id, run_id, tenant_id, workspace_id, sequence, node_id, status, state_json, created_at)
             VALUES (?1, ?2, ?3, ?4, 1, ?5, 'running', ?6, ?7)",
            rusqlite::params![chk_id, run_id, tenant_id, workspace_id, crew_id, state_json, now],
        ).unwrap();
        guard.execute(
            "INSERT INTO tool_journal (run_id, tenant_id, workspace_id, node_id, tool_call_id, tool_name, arguments_json, status, output_text, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, 'call_executing_456', 'write_file', '{\"path\":\"executing_side_effect.txt\"}', 'executing', NULL, ?5, ?5)",
            rusqlite::params![run_id, tenant_id, workspace_id, crew_id, now],
        ).unwrap();
    }

    let resume_resp = client
        .post(format!("{}/api/v1/runs/{}/resume", base_url, run_id))
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .send()
        .await
        .unwrap();
    assert_eq!(resume_resp.status(), StatusCode::OK);

    let mut failed_closed = false;
    for _ in 0..50 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        let get_resp = client
            .get(format!("{}/api/v1/runs/{}", base_url, run_id))
            .header(AUTHORIZATION, format!("Bearer {}", token))
            .send()
            .await
            .unwrap();
        let detail: Value = get_resp.json().await.unwrap();
        let status = detail["status"].as_str().unwrap();
        if status == "FAILED" {
            failed_closed = true;
            break;
        } else if status == "SUCCEEDED" {
            break;
        }
    }

    assert!(
        failed_closed,
        "Run must fail closed when a tool call was left in executing status"
    );

    let side_effect_path = workspace_root.join("executing_side_effect.txt");
    assert!(
        !side_effect_path.exists(),
        "Executing tool call was re-executed: side effect file must not exist"
    );
}

#[tokio::test]
async fn test_api_run_with_no_model_configured_fails_loudly() {
    let (base_url, store, _state, _workspace_root, _tmp) = spawn_test_server().await;
    let client = reqwest::Client::new();
    let token = get_token(&client, &base_url).await;

    let wf_id = Uuid::new_v4();
    let crew_id = "crew_no_model";
    let spec = create_crew_workflow(wf_id, crew_id);

    let create_resp = client
        .post(format!("{}/api/v1/workflows", base_url))
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .json(&spec)
        .send()
        .await
        .unwrap();
    assert_eq!(create_resp.status(), StatusCode::CREATED);

    let run_resp = client
        .post(format!("{}/api/v1/workflows/{}/runs", base_url, wf_id))
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .json(&serde_json::json!({
            "initial_input": {"task": "do work"}
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(run_resp.status(), StatusCode::ACCEPTED);
    let run_body: Value = run_resp.json().await.unwrap();
    let run_id = run_body["run_id"].as_str().unwrap();

    let mut failed_loudly = false;
    for _ in 0..50 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        let get_resp = client
            .get(format!("{}/api/v1/runs/{}", base_url, run_id))
            .header(AUTHORIZATION, format!("Bearer {}", token))
            .send()
            .await
            .unwrap();
        let detail: Value = get_resp.json().await.unwrap();
        let status = detail["status"].as_str().unwrap();
        if status == "FAILED" {
            failed_loudly = true;
            break;
        } else if status == "SUCCEEDED" {
            panic!("Run with no model configured must never succeed");
        }
    }

    assert!(
        failed_loudly,
        "Run with no model configured must fail loudly with FAILED status"
    );

    let get_resp = client
        .get(format!("{}/api/v1/runs/{}", base_url, run_id))
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .send()
        .await
        .unwrap();
    let detail: Value = get_resp.json().await.unwrap();
    let error_text = detail.to_string();
    let events = store.list_run_events(run_id, None).unwrap();

    let has_model_err_msg = error_text.contains("no model configured")
        || events
            .iter()
            .any(|ev| ev.payload.to_string().contains("no model configured"));
    assert!(
        has_model_err_msg,
        "Error message must contain 'no model configured: set OPENAI_API_KEY / --api-base'"
    );
}

#[tokio::test]
async fn test_api_resume_completed_run_rejected() {
    let (base_url, store, _state, _workspace_root, _tmp) = spawn_test_server().await;
    let client = reqwest::Client::new();
    let token = get_token(&client, &base_url).await;

    let run_id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let run_rec = RunRecord {
        id: run_id.clone(),
        workflow_id: Uuid::new_v4().to_string(),
        status: "SUCCEEDED".to_string(),
        input_data: "{}".to_string(),
        output_data: Some("{\"result\":\"done\"}".to_string()),
        created_at: now.clone(),
        updated_at: now.clone(),
        completed_at: Some(now),
        verified: false,
    };
    store.insert_run(&run_rec).unwrap();

    let resume_resp = client
        .post(format!("{}/api/v1/runs/{}/resume", base_url, run_id))
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .send()
        .await
        .unwrap();

    assert_eq!(resume_resp.status(), StatusCode::BAD_REQUEST);
    let body: Value = resume_resp.json().await.unwrap();
    assert!(body["detail"]
        .as_str()
        .unwrap()
        .contains("already completed"));
}

#[tokio::test]
async fn test_api_cancel_run() {
    let (base_url, store, _state, _workspace_root, _tmp) = spawn_test_server().await;
    let client = reqwest::Client::new();
    let token = get_token(&client, &base_url).await;

    let run_id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let run_rec = RunRecord {
        id: run_id.clone(),
        workflow_id: Uuid::new_v4().to_string(),
        status: "RUNNING".to_string(),
        input_data: "{}".to_string(),
        output_data: None,
        created_at: now.clone(),
        updated_at: now.clone(),
        completed_at: None,
        verified: false,
    };
    store.insert_run(&run_rec).unwrap();

    let cancel_resp = client
        .post(format!("{}/api/v1/runs/{}/cancel", base_url, run_id))
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .send()
        .await
        .unwrap();

    assert_eq!(cancel_resp.status(), StatusCode::OK);
    let cancel_body: Value = cancel_resp.json().await.unwrap();
    assert_eq!(cancel_body["status"], "CANCELLED");

    let get_resp = client
        .get(format!("{}/api/v1/runs/{}", base_url, run_id))
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .send()
        .await
        .unwrap();
    let detail: Value = get_resp.json().await.unwrap();
    assert_eq!(detail["status"], "CANCELLED");
}

#[tokio::test]
async fn test_api_checkpoints_fork_rewind_artifacts() {
    let (base_url, store, _state, _workspace_root, _tmp) = spawn_test_server().await;
    let client = reqwest::Client::new();
    let token = get_token(&client, &base_url).await;

    let run_id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let run_rec = RunRecord {
        id: run_id.clone(),
        workflow_id: Uuid::new_v4().to_string(),
        status: "RUNNING".to_string(),
        input_data: "{}".to_string(),
        output_data: None,
        created_at: now.clone(),
        updated_at: now.clone(),
        completed_at: None,
        verified: false,
    };
    store.insert_run(&run_rec).unwrap();

    let chk_id = Uuid::new_v4().to_string();
    let tenant_id = nexus_albaai::server::auth::DEFAULT_LOCAL_TENANT_ID;
    let workspace_id = nexus_albaai::server::auth::DEFAULT_LOCAL_WORKSPACE_ID;
    {
        let conn = store.connection();
        let guard = conn.lock().unwrap();
        guard.execute(
            "INSERT INTO workflow_checkpoints (id, run_id, tenant_id, workspace_id, sequence, node_id, status, state_json, created_at)
             VALUES (?1, ?2, ?3, ?4, 1, 'start_node', 'completed', '{\"input\":1}', ?5)",
            rusqlite::params![chk_id, run_id, tenant_id, workspace_id, now],
        ).unwrap();
        guard.execute(
            "INSERT INTO artifacts (id, run_id, filename, file_path, file_size, mime_type, sha256, verified, created_at)
             VALUES ('art-1', ?1, 'result.pdf', 'artifacts/result.pdf', 1024, 'application/pdf', 'dummy_hash', 1, ?2)",
            rusqlite::params![run_id, now],
        ).unwrap();
    }

    let chk_resp = client
        .get(format!("{}/api/v1/runs/{}/checkpoints", base_url, run_id))
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .send()
        .await
        .unwrap();
    assert_eq!(chk_resp.status(), StatusCode::OK);
    let chk_list: Vec<Value> = chk_resp.json().await.unwrap();
    assert_eq!(chk_list.len(), 1);
    assert_eq!(chk_list[0]["id"], chk_id);
    assert_eq!(chk_list[0]["sequence"], 1);

    let fork_resp = client
        .post(format!(
            "{}/api/v1/runs/{}/checkpoints/{}/fork",
            base_url, run_id, chk_id
        ))
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .json(&serde_json::json!({
            "branch_a": {},
            "branch_b": {}
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(fork_resp.status(), StatusCode::CREATED);
    let fork_body: Value = fork_resp.json().await.unwrap();
    assert!(fork_body["branch_a"]["run_id"].is_string());
    assert!(fork_body["branch_b"]["run_id"].is_string());

    let rewind_resp = client
        .post(format!(
            "{}/api/v1/runs/{}/checkpoints/{}/rewind",
            base_url, run_id, chk_id
        ))
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .json(&serde_json::json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(rewind_resp.status(), StatusCode::OK);
    let rewind_body: Value = rewind_resp.json().await.unwrap();
    assert_eq!(rewind_body["run_id"], run_id);
    assert_eq!(rewind_body["checkpoint_id"], chk_id);

    let art_resp = client
        .get(format!("{}/api/v1/runs/{}/artifacts", base_url, run_id))
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .send()
        .await
        .unwrap();
    assert_eq!(art_resp.status(), StatusCode::OK);
    let art_list: Vec<Value> = art_resp.json().await.unwrap();
    assert_eq!(art_list.len(), 1);
    assert_eq!(art_list[0]["filename"], "result.pdf");
    assert_eq!(art_list[0]["size_bytes"], 1024);
}
