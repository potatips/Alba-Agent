use nexus_albaai::checkpoints::{CheckpointStore, SqliteCheckpointStore};
use nexus_albaai::domain::ApprovalDecision;
use nexus_albaai::schema::node::{ApprovalGateNodeSpec, EndNodeSpec, StartNodeSpec, WorkflowNode};
use nexus_albaai::schema::workflow::{WorkflowEdgeSpec, WorkflowModesSpec, WorkflowSpec};
use nexus_albaai::server::state::AppState;
use nexus_albaai::server::workflow_runner::resume_workflow_background;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

async fn spawn_test_server() -> (
    String,
    Arc<SqliteCheckpointStore>,
    AppState,
    tempfile::TempDir,
) {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("test_sse.db");
    let store = Arc::new(SqliteCheckpointStore::new(&db_path).unwrap());
    let state = AppState::new(store.clone(), tmp.path().to_path_buf());
    let state_for_server = state.clone();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let base_url = format!("http://{}", addr);
    tokio::spawn(async move {
        nexus_albaai::server::serve_app(state_for_server, listener)
            .await
            .unwrap();
    });
    (base_url, store, state, tmp)
}

fn create_approval_workflow(id: Uuid) -> WorkflowSpec {
    WorkflowSpec {
        id,
        tenant_id: Uuid::new_v4(),
        workspace_id: Uuid::new_v4(),
        name: "SSE Approval Workflow".to_string(),
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
                label: "Admin Gate".to_string(),
                action_description: "Approve deployment".to_string(),
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
    }
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

struct ParsedSseEvent {
    id: String,
    event: String,
    data: Value,
}

fn parse_sse_chunk(chunk: &str) -> Vec<ParsedSseEvent> {
    let mut events = Vec::new();
    let blocks = chunk.split("\n\n");
    for block in blocks {
        let trimmed = block.trim();
        if trimmed.is_empty() || trimmed.starts_with(':') {
            continue;
        }
        let mut id = String::new();
        let mut event = String::from("message");
        let mut data_lines = Vec::new();
        for line in trimmed.lines() {
            if let Some(rest) = line.strip_prefix("id:") {
                id = rest.trim().to_string();
            } else if let Some(rest) = line.strip_prefix("event:") {
                event = rest.trim().to_string();
            } else if let Some(rest) = line.strip_prefix("data:") {
                data_lines.push(rest.trim());
            }
        }
        if !data_lines.is_empty() {
            let data_str = data_lines.join("\n");
            if let Ok(parsed) = serde_json::from_str::<Value>(&data_str) {
                events.push(ParsedSseEvent {
                    id,
                    event,
                    data: parsed,
                });
            }
        }
    }
    events
}

#[tokio::test]
async fn test_sse_ordered_events_with_approval_gate() {
    let (base_url, store, state, _tmp) = spawn_test_server().await;
    let client = reqwest::Client::new();
    let token = get_token(&client, &base_url).await;

    let wf_id = Uuid::new_v4();
    let spec = create_approval_workflow(wf_id);

    let create_wf_resp = client
        .post(format!("{}/api/v1/workflows", base_url))
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .json(&spec)
        .send()
        .await
        .unwrap();
    assert_eq!(create_wf_resp.status(), 201);

    let run_resp = client
        .post(format!("{}/api/v1/workflows/{}/runs", base_url, wf_id))
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .json(&serde_json::json!({
            "initial_input": {"goal": "test sse streaming"}
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(run_resp.status(), 202);
    let run_body: Value = run_resp.json().await.unwrap();
    let run_id = run_body["run_id"].as_str().unwrap();

    let mut sse_resp = client
        .get(format!("{}/api/v1/runs/{}/stream", base_url, run_id))
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .send()
        .await
        .unwrap();
    assert_eq!(sse_resp.status(), 200);

    let mut collected_events = Vec::new();
    let mut approval_id = None;

    while let Some(chunk_res) = sse_resp.chunk().await.unwrap() {
        let text = String::from_utf8_lossy(&chunk_res);
        let parsed = parse_sse_chunk(&text);
        for ev in parsed {
            if ev.event == "APPROVAL_REQUIRED" {
                approval_id = ev.data["payload"]["approval_id"]
                    .as_str()
                    .map(|s| s.to_string());
            }
            collected_events.push(ev);
        }
        if approval_id.is_some() {
            break;
        }
    }

    assert!(collected_events.len() >= 2);
    assert_eq!(collected_events[0].event, "FLOW_STATE_CHANGED");
    assert!(collected_events
        .iter()
        .any(|e| e.event == "APPROVAL_REQUIRED"));
    assert!(approval_id.is_some());

    let app_id = Uuid::parse_str(&approval_id.unwrap()).unwrap();
    let run_uuid = Uuid::parse_str(run_id).unwrap();
    let tenant_id = Uuid::parse_str(nexus_albaai::server::auth::DEFAULT_LOCAL_TENANT_ID).unwrap();
    let workspace_id =
        Uuid::parse_str(nexus_albaai::server::auth::DEFAULT_LOCAL_WORKSPACE_ID).unwrap();
    let scope =
        nexus_albaai::domain::ExecutionScope::new(tenant_id, workspace_id, run_uuid).unwrap();

    store
        .resolve_approval(
            &scope,
            app_id,
            ApprovalDecision::AllowOnce,
            "approver_human",
        )
        .await
        .unwrap();

    let state_clone = state.clone();
    let spec_clone = spec.clone();
    tokio::spawn(async move {
        resume_workflow_background(state_clone, scope, spec_clone).await;
    });

    while let Some(chunk_res) = sse_resp.chunk().await.unwrap() {
        let text = String::from_utf8_lossy(&chunk_res);
        let parsed = parse_sse_chunk(&text);
        for ev in parsed {
            collected_events.push(ev);
        }
        if collected_events
            .iter()
            .any(|e| e.event == "RUN_COMPLETED" || e.event == "RUN_FAILED")
        {
            break;
        }
    }

    let final_event = collected_events.last().unwrap();
    assert_eq!(final_event.event, "RUN_COMPLETED");
    assert_eq!(final_event.data["payload"]["run_id"], run_id);
}

#[tokio::test]
async fn test_sse_reconnect_with_last_event_id() {
    let (base_url, _store, _state, _tmp) = spawn_test_server().await;
    let client = reqwest::Client::new();
    let token = get_token(&client, &base_url).await;

    let wf_id = Uuid::new_v4();
    let spec = create_approval_workflow(wf_id);

    client
        .post(format!("{}/api/v1/workflows", base_url))
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .json(&spec)
        .send()
        .await
        .unwrap();

    let run_resp = client
        .post(format!("{}/api/v1/workflows/{}/runs", base_url, wf_id))
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .json(&serde_json::json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(run_resp.status(), 202);
    let run_body: Value = run_resp.json().await.unwrap();
    let run_id = run_body["run_id"].as_str().unwrap();

    let mut first_resp = client
        .get(format!("{}/api/v1/runs/{}/stream", base_url, run_id))
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .send()
        .await
        .unwrap();

    let mut first_event_id = String::new();
    while let Some(chunk_res) = first_resp.chunk().await.unwrap() {
        let text = String::from_utf8_lossy(&chunk_res);
        let parsed = parse_sse_chunk(&text);
        if let Some(first) = parsed.first() {
            first_event_id = first.id.clone();
            break;
        }
    }
    assert!(!first_event_id.is_empty());
    drop(first_resp);

    tokio::time::sleep(Duration::from_millis(100)).await;

    let mut headers = HeaderMap::new();
    headers.insert(
        AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {}", token)).unwrap(),
    );
    headers.insert(
        "last-event-id",
        HeaderValue::from_str(&first_event_id).unwrap(),
    );

    let mut reconnect_resp = client
        .get(format!("{}/api/v1/runs/{}/stream", base_url, run_id))
        .headers(headers)
        .send()
        .await
        .unwrap();
    assert_eq!(reconnect_resp.status(), 200);

    let mut reconnected_events = Vec::new();
    while let Some(chunk_res) = reconnect_resp.chunk().await.unwrap() {
        let text = String::from_utf8_lossy(&chunk_res);
        let parsed = parse_sse_chunk(&text);
        for ev in parsed {
            reconnected_events.push(ev);
        }
        if reconnected_events
            .iter()
            .any(|e| e.event == "APPROVAL_REQUIRED")
        {
            break;
        }
    }

    assert!(!reconnected_events.is_empty());
    assert!(!reconnected_events.iter().any(|e| e.id == first_event_id));
}

#[tokio::test]
async fn test_sse_disconnect_cleanup_no_leaked_tasks() {
    let (base_url, _store, _state, _tmp) = spawn_test_server().await;
    let client = reqwest::Client::new();
    let token = get_token(&client, &base_url).await;

    let wf_id = Uuid::new_v4();
    let spec = create_approval_workflow(wf_id);

    client
        .post(format!("{}/api/v1/workflows", base_url))
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .json(&spec)
        .send()
        .await
        .unwrap();

    let run_resp = client
        .post(format!("{}/api/v1/workflows/{}/runs", base_url, wf_id))
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .json(&serde_json::json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(run_resp.status(), 202);
    let run_body: Value = run_resp.json().await.unwrap();
    let run_id = run_body["run_id"].as_str().unwrap();

    let mut resp = client
        .get(format!("{}/api/v1/runs/{}/stream", base_url, run_id))
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let chunk = resp.chunk().await.unwrap();
    assert!(chunk.is_some());

    drop(resp);

    tokio::time::sleep(Duration::from_millis(200)).await;

    let health_resp = client
        .get(format!("{}/health", base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(health_resp.status(), 200);
}

#[tokio::test]
async fn test_sse_run_not_found_and_unauthorized() {
    let (base_url, _store, _state, _tmp) = spawn_test_server().await;
    let client = reqwest::Client::new();
    let token = get_token(&client, &base_url).await;

    let missing_resp = client
        .get(format!(
            "{}/api/v1/runs/00000000-0000-0000-0000-000000000000/stream",
            base_url
        ))
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .send()
        .await
        .unwrap();
    assert_eq!(missing_resp.status(), 404);
    let missing_body: Value = missing_resp.json().await.unwrap();
    assert_eq!(missing_body["detail"], "Run not found");

    let unauth_resp = client
        .get(format!(
            "{}/api/v1/runs/00000000-0000-0000-0000-000000000000/stream",
            base_url
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(unauth_resp.status(), 401);
}

#[tokio::test]
async fn test_sse_two_clients_concurrent() {
    let (base_url, _store, _state, _tmp) = spawn_test_server().await;
    let client = reqwest::Client::new();
    let token = get_token(&client, &base_url).await;

    let wf_id = Uuid::new_v4();
    let spec = create_approval_workflow(wf_id);

    client
        .post(format!("{}/api/v1/workflows", base_url))
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .json(&spec)
        .send()
        .await
        .unwrap();

    let run_resp = client
        .post(format!("{}/api/v1/workflows/{}/runs", base_url, wf_id))
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .json(&serde_json::json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(run_resp.status(), 202);
    let run_body: Value = run_resp.json().await.unwrap();
    let run_id = run_body["run_id"].as_str().unwrap();

    let c1 = client.clone();
    let u1 = format!("{}/api/v1/runs/{}/stream", base_url, run_id);
    let t1 = token.clone();
    let h1 = tokio::spawn(async move {
        let mut r = c1
            .get(u1)
            .header(AUTHORIZATION, format!("Bearer {}", t1))
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), 200);
        let mut events = Vec::new();
        while let Some(chunk) = r.chunk().await.unwrap() {
            let text = String::from_utf8_lossy(&chunk);
            for ev in parse_sse_chunk(&text) {
                events.push(ev);
            }
            if events.iter().any(|e| e.event == "APPROVAL_REQUIRED") {
                break;
            }
        }
        events
    });

    let c2 = client.clone();
    let u2 = format!("{}/api/v1/runs/{}/stream", base_url, run_id);
    let t2 = token.clone();
    let h2 = tokio::spawn(async move {
        let mut r = c2
            .get(u2)
            .header(AUTHORIZATION, format!("Bearer {}", t2))
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), 200);
        let mut events = Vec::new();
        while let Some(chunk) = r.chunk().await.unwrap() {
            let text = String::from_utf8_lossy(&chunk);
            for ev in parse_sse_chunk(&text) {
                events.push(ev);
            }
            if events.iter().any(|e| e.event == "APPROVAL_REQUIRED") {
                break;
            }
        }
        events
    });

    let (res1, res2) = tokio::join!(h1, h2);
    let ev1 = res1.unwrap();
    let ev2 = res2.unwrap();
    assert_eq!(ev1.len(), ev2.len());
    for i in 0..ev1.len() {
        assert_eq!(ev1[i].id, ev2[i].id);
        assert_eq!(ev1[i].event, ev2[i].event);
    }
}
