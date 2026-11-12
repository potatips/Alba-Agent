use nexus_albaai::checkpoints::SqliteCheckpointStore;
use nexus_albaai::server::state::AppState;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use reqwest::StatusCode;
use serde_json::Value;
use std::sync::Arc;
use uuid::Uuid;

async fn spawn_test_server() -> (
    String,
    Arc<SqliteCheckpointStore>,
    AppState,
    tempfile::TempDir,
) {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("test_approvals.db");
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

async fn get_token(client: &reqwest::Client, base_url: &str) -> String {
    let resp = client
        .post(format!("{}/api/v1/auth/local-session", base_url))
        .send()
        .await
        .unwrap();
    let body: Value = resp.json().await.unwrap();
    body["access_token"].as_str().unwrap().to_string()
}

fn seed_pending_approval(store: &SqliteCheckpointStore, app_id: &str, run_id: &str, author: &str) {
    let tenant_id = nexus_albaai::server::auth::DEFAULT_LOCAL_TENANT_ID;
    let workspace_id = nexus_albaai::server::auth::DEFAULT_LOCAL_WORKSPACE_ID;
    let conn = store.connection();
    let guard = conn.lock().unwrap();
    guard
        .execute(
            "INSERT INTO approvals (id, run_id, tenant_id, workspace_id, tool_name, action_description, intent_kind, author_id, status, created_at)
             VALUES (?1, ?2, ?3, ?4, 'run_cmd', 'cat /etc/hosts', 'destructive', ?5, 'pending', datetime('now'))",
            rusqlite::params![app_id, run_id, tenant_id, workspace_id, author],
        )
        .unwrap();
}

#[tokio::test]
async fn test_api_approvals_self_approval_rejected() {
    let (base_url, store, _state, _tmp) = spawn_test_server().await;
    let client = reqwest::Client::new();
    let token = get_token(&client, &base_url).await;

    let app_id = Uuid::new_v4().to_string();
    let run_id = Uuid::new_v4().to_string();
    let author_name = "alice_author";

    seed_pending_approval(&store, &app_id, &run_id, author_name);

    let mut headers = HeaderMap::new();
    headers.insert(
        AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {}", token)).unwrap(),
    );
    headers.insert("x-actor-id", HeaderValue::from_str(author_name).unwrap());

    let resp = client
        .post(format!("{}/api/v1/approvals/{}/resolve", base_url, app_id))
        .headers(headers)
        .json(&serde_json::json!({
            "decision": "allow_once",
            "feedback": "Self-approving my own request"
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    let body: Value = resp.json().await.unwrap();
    assert!(body["detail"]
        .as_str()
        .unwrap()
        .contains("Separation of duties violation"));

    let conn = store.connection();
    let guard = conn.lock().unwrap();
    let status: String = guard
        .query_row(
            "SELECT status FROM approvals WHERE id = ?1",
            rusqlite::params![app_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(status, "pending");

    let audit_count: i64 = guard
        .query_row(
            "SELECT count(*) FROM audit_logs WHERE action = 'approval.self_approval_rejected' AND target_id = ?1",
            rusqlite::params![app_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(audit_count, 1);
}

#[tokio::test]
async fn test_api_approvals_double_resolve_rejected() {
    let (base_url, store, _state, _tmp) = spawn_test_server().await;
    let client = reqwest::Client::new();
    let token = get_token(&client, &base_url).await;

    let app_id = Uuid::new_v4().to_string();
    let run_id = Uuid::new_v4().to_string();
    let author_name = "alice_author";
    let approver_name = "bob_approver";

    seed_pending_approval(&store, &app_id, &run_id, author_name);

    let mut headers = HeaderMap::new();
    headers.insert(
        AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {}", token)).unwrap(),
    );
    headers.insert("x-actor-id", HeaderValue::from_str(approver_name).unwrap());

    let first_resp = client
        .post(format!("{}/api/v1/approvals/{}/resolve", base_url, app_id))
        .headers(headers.clone())
        .json(&serde_json::json!({
            "decision": "allow_once",
            "feedback": "Approved by supervisor"
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(first_resp.status(), StatusCode::OK);
    let first_body: Value = first_resp.json().await.unwrap();
    assert_eq!(first_body["status"], "RESOLVED");
    assert_eq!(first_body["decision"], "allow_once");

    let second_resp = client
        .post(format!("{}/api/v1/approvals/{}/resolve", base_url, app_id))
        .headers(headers)
        .json(&serde_json::json!({
            "decision": "deny",
            "feedback": "Attempting second decision"
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(second_resp.status(), StatusCode::CONFLICT);
    let second_body: Value = second_resp.json().await.unwrap();
    assert!(second_body["detail"]
        .as_str()
        .unwrap()
        .contains("already decided"));

    let conn = store.connection();
    let guard = conn.lock().unwrap();
    let (status, decided_by): (String, Option<String>) = guard
        .query_row(
            "SELECT status, decided_by FROM approvals WHERE id = ?1",
            rusqlite::params![app_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(status, "allowed_once");
    assert_eq!(decided_by.as_deref(), Some(approver_name));

    let audit_count: i64 = guard
        .query_row(
            "SELECT count(*) FROM audit_logs WHERE action = 'approval.already_decided_rejected' AND target_id = ?1",
            rusqlite::params![app_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(audit_count, 1);
}

#[tokio::test]
async fn test_api_approvals_list_endpoint() {
    let (base_url, store, _state, _tmp) = spawn_test_server().await;
    let client = reqwest::Client::new();
    let token = get_token(&client, &base_url).await;

    let app_id = Uuid::new_v4().to_string();
    let run_id = Uuid::new_v4().to_string();
    seed_pending_approval(&store, &app_id, &run_id, "test_author");

    let resp = client
        .get(format!("{}/api/v1/approvals?run_id={}", base_url, run_id))
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let list: Vec<Value> = resp.json().await.unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0]["id"], app_id);
    assert_eq!(list[0]["run_id"], run_id);
    assert_eq!(list[0]["status"], "pending");
}
