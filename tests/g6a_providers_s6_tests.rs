use axum::extract::Path;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::post;
use axum::{Json, Router};
use nexus_albaai::checkpoints::SqliteCheckpointStore;
use nexus_albaai::credentials::{CredentialStore, MemoryCredentialStore};
use nexus_albaai::server::create_app;
use nexus_albaai::server::state::{ActiveSessionInfo, AppState};
use reqwest::header::{COOKIE, HOST, ORIGIN};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::net::TcpListener;

async fn mock_openai_handler(Json(_body): Json<Value>) -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(json!({
            "id": "chatcmpl-test",
            "object": "chat.completion",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": "pong"
                },
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": 1,
                "completion_tokens": 1,
                "total_tokens": 2
            }
        })),
    )
}

async fn mock_anthropic_handler(Json(_body): Json<Value>) -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(json!({
            "id": "msg-test",
            "type": "message",
            "role": "assistant",
            "content": [{
                "type": "text",
                "text": "pong"
            }],
            "usage": {
                "input_tokens": 1,
                "output_tokens": 1
            }
        })),
    )
}

async fn mock_gemini_handler(
    Path(_action): Path<String>,
    Json(_body): Json<Value>,
) -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(json!({
            "candidates": [{
                "content": {
                    "parts": [{ "text": "pong" }],
                    "role": "model"
                },
                "finishReason": "STOP",
                "index": 0
            }],
            "usageMetadata": {
                "promptTokenCount": 1,
                "candidatesTokenCount": 1,
                "totalTokenCount": 2
            }
        })),
    )
}

async fn start_mock_llm_server() -> String {
    let app = Router::new()
        .route("/v1/chat/completions", post(mock_openai_handler))
        .route("/v1/messages", post(mock_anthropic_handler))
        .route("/v1beta/models/{action}", post(mock_gemini_handler));

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    format!("http://{}", addr)
}

async fn setup_app_and_account(
    sub: &str,
    email: &str,
    tenant_id: &str,
    workspace_id: &str,
) -> (String, AppState, String) {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("test_s6.db");
    let store = Arc::new(SqliteCheckpointStore::new(&db_path).unwrap());
    let mut state = AppState::new(store.clone(), dir.path().to_path_buf());
    let cred_store = Arc::new(MemoryCredentialStore::new());
    state.credentials = cred_store.clone();

    let now = chrono::Utc::now().timestamp();
    let acct = nexus_albaai::profiles::GoogleAccountRecord {
        sub: sub.to_string(),
        email: email.to_string(),
        display_name: format!("User {}", sub),
        picture: None,
        tenant_id: tenant_id.to_string(),
        workspace_id: workspace_id.to_string(),
        active: true,
        created_at: now,
        updated_at: now,
    };
    store.upsert_google_account(&acct).unwrap();

    let session_id = format!("sess_{}", sub);
    {
        let mut sessions = state.active_sessions.lock().await;
        sessions.insert(
            session_id.clone(),
            ActiveSessionInfo {
                session_id: session_id.clone(),
                sub: sub.to_string(),
                email: email.to_string(),
                display_name: format!("User {}", sub),
                picture: None,
                tenant_id: tenant_id.to_string(),
                workspace_id: workspace_id.to_string(),
                expires_at: now + 3600,
            },
        );
    }

    let jwt_key = format!("account:{sub}:jwt_secret");
    let _ = cred_store.set_secret(&jwt_key, "test-jwt-secret-for-s6");

    let app = create_app(state.clone());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let base_url = format!("http://{}", addr);
    let cookie = format!("session_id={}", session_id);
    (base_url, state, cookie)
}

#[tokio::test]
async fn test_provider_presets_contract() {
    let (base_url, _state, _cookie) = setup_app_and_account(
        "sub-1",
        "user1@example.com",
        "00000000-0000-0000-0000-000000000001",
        "00000000-0000-0000-0000-000000000002",
    )
    .await;
    let client = reqwest::Client::new();
    let host_str = base_url.strip_prefix("http://").unwrap();

    let resp = client
        .get(format!("{base_url}/api/v1/settings/provider-presets"))
        .header(HOST, host_str)
        .header(ORIGIN, &base_url)
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.unwrap();
    let direct = body["categories"]["direct"].as_array().unwrap();
    let preset_ids: Vec<&str> = direct.iter().map(|p| p["id"].as_str().unwrap()).collect();
    assert!(preset_ids.contains(&"openai"));
    assert!(preset_ids.contains(&"anthropic"));
    assert!(preset_ids.contains(&"google_gemini"));
    assert!(preset_ids.contains(&"deepseek"));
}

#[tokio::test]
async fn test_unauthenticated_request_fails_closed() {
    let (base_url, _state, _cookie) = setup_app_and_account(
        "sub-1",
        "user1@example.com",
        "00000000-0000-0000-0000-000000000001",
        "00000000-0000-0000-0000-000000000002",
    )
    .await;
    let client = reqwest::Client::new();
    let host_str = base_url.strip_prefix("http://").unwrap();

    let resp_get = client
        .get(format!("{base_url}/api/v1/settings/credentials"))
        .header(HOST, host_str)
        .header(ORIGIN, &base_url)
        .send()
        .await
        .unwrap();
    assert_eq!(resp_get.status(), 401);

    let resp_post = client
        .post(format!("{base_url}/api/v1/settings/credentials"))
        .header(HOST, host_str)
        .header(ORIGIN, &base_url)
        .json(&json!({
            "preset_id": "anthropic",
            "api_key": "sk-ant-123456789"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp_post.status(), 401);

    let resp_test = client
        .post(format!(
            "{base_url}/api/v1/settings/credentials/fake-id/test"
        ))
        .header(HOST, host_str)
        .header(ORIGIN, &base_url)
        .send()
        .await
        .unwrap();
    assert_eq!(resp_test.status(), 401);

    let resp_del = client
        .delete(format!("{base_url}/api/v1/settings/credentials/fake-id"))
        .header(HOST, host_str)
        .header(ORIGIN, &base_url)
        .send()
        .await
        .unwrap();
    assert_eq!(resp_del.status(), 401);
}

#[tokio::test]
async fn test_credentials_lifecycle_and_zero_secret_leakage() {
    let mock_llm_url = start_mock_llm_server().await;
    let raw_secret = "sk-ant-api03-very-secret-key-123456789";

    let (base_url, state, cookie) = setup_app_and_account(
        "sub-user-a",
        "usera@example.com",
        "10000000-0000-0000-0000-000000000001",
        "10000000-0000-0000-0000-000000000002",
    )
    .await;
    let client = reqwest::Client::new();
    let host_str = base_url.strip_prefix("http://").unwrap();

    let create_resp = client
        .post(format!("{base_url}/api/v1/settings/credentials"))
        .header(HOST, host_str)
        .header(ORIGIN, &base_url)
        .header(COOKIE, &cookie)
        .json(&json!({
            "preset_id": "anthropic",
            "api_key": raw_secret,
            "base_url": mock_llm_url,
            "is_default": true
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(create_resp.status(), 200);
    let create_body_text = create_resp.text().await.unwrap();
    assert!(!create_body_text.contains(raw_secret));

    let created: Value = serde_json::from_str(&create_body_text).unwrap();
    let cred_id = created["id"].as_str().unwrap().to_string();
    assert_eq!(created["preset_id"], "anthropic");
    assert_eq!(created["provider"], "anthropic");
    assert_eq!(created["status"], "CONNECTED");
    let masked = created["masked_key"].as_str().unwrap();
    assert!(masked.starts_with("sk-ant-"));
    assert!(masked.ends_with("6789"));
    assert!(!masked.contains("very-secret-key"));

    let secret_in_keychain = state
        .credentials
        .get_secret(&format!("account:sub-user-a:provider:{}", cred_id))
        .unwrap();
    assert_eq!(secret_in_keychain.as_deref(), Some(raw_secret));

    let list_resp = client
        .get(format!("{base_url}/api/v1/settings/credentials"))
        .header(HOST, host_str)
        .header(ORIGIN, &base_url)
        .header(COOKIE, &cookie)
        .send()
        .await
        .unwrap();
    assert_eq!(list_resp.status(), 200);
    let list_text = list_resp.text().await.unwrap();
    assert!(!list_text.contains(raw_secret));
    let list_json: Value = serde_json::from_str(&list_text).unwrap();
    assert_eq!(list_json.as_array().unwrap().len(), 1);

    let test_resp = client
        .post(format!(
            "{base_url}/api/v1/settings/credentials/{cred_id}/test"
        ))
        .header(HOST, host_str)
        .header(ORIGIN, &base_url)
        .header(COOKIE, &cookie)
        .send()
        .await
        .unwrap();
    assert_eq!(test_resp.status(), 200);
    let test_body: Value = test_resp.json().await.unwrap();
    assert_eq!(test_body["status"], "CONNECTED");
    assert!(test_body["error"].is_null());

    let def_resp = client
        .get(format!("{base_url}/api/v1/settings/default-models"))
        .header(HOST, host_str)
        .header(ORIGIN, &base_url)
        .header(COOKIE, &cookie)
        .send()
        .await
        .unwrap();
    assert_eq!(def_resp.status(), 200);

    let update_def_resp = client
        .put(format!("{base_url}/api/v1/settings/default-models"))
        .header(HOST, host_str)
        .header(ORIGIN, &base_url)
        .header(COOKIE, &cookie)
        .json(&json!({
            "orchestrator": "anthropic/claude-3-7-sonnet"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(update_def_resp.status(), 200);

    let del_resp = client
        .delete(format!("{base_url}/api/v1/settings/credentials/{cred_id}"))
        .header(HOST, host_str)
        .header(ORIGIN, &base_url)
        .header(COOKIE, &cookie)
        .send()
        .await
        .unwrap();
    assert_eq!(del_resp.status(), 200);
    let del_body: Value = del_resp.json().await.unwrap();
    assert_eq!(del_body["deleted"], true);

    let secret_after_del = state
        .credentials
        .get_secret(&format!("account:sub-user-a:provider:{}", cred_id))
        .unwrap();
    assert_eq!(secret_after_del, None);

    let list_after_del = client
        .get(format!("{base_url}/api/v1/settings/credentials"))
        .header(HOST, host_str)
        .header(ORIGIN, &base_url)
        .header(COOKIE, &cookie)
        .send()
        .await
        .unwrap();
    let after_json: Value = list_after_del.json().await.unwrap();
    assert!(after_json.as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_multi_account_credential_isolation() {
    let mock_llm_url = start_mock_llm_server().await;
    let secret_a = "sk-ant-secret-for-user-a-9999";

    let (base_url, state, cookie_a) = setup_app_and_account(
        "sub-user-a",
        "usera@example.com",
        "10000000-0000-0000-0000-000000000001",
        "10000000-0000-0000-0000-000000000002",
    )
    .await;
    let client = reqwest::Client::new();
    let host_str = base_url.strip_prefix("http://").unwrap();

    let create_resp = client
        .post(format!("{base_url}/api/v1/settings/credentials"))
        .header(HOST, host_str)
        .header(ORIGIN, &base_url)
        .header(COOKIE, &cookie_a)
        .json(&json!({
            "preset_id": "anthropic",
            "api_key": secret_a,
            "base_url": mock_llm_url,
            "is_default": true
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(create_resp.status(), 200);
    let cred_a: Value = create_resp.json().await.unwrap();
    let cred_a_id = cred_a["id"].as_str().unwrap().to_string();

    let now = chrono::Utc::now().timestamp();
    let acct_b = nexus_albaai::profiles::GoogleAccountRecord {
        sub: "sub-user-b".to_string(),
        email: "userb@example.com".to_string(),
        display_name: "User B".to_string(),
        picture: None,
        tenant_id: "20000000-0000-0000-0000-000000000001".to_string(),
        workspace_id: "20000000-0000-0000-0000-000000000002".to_string(),
        active: true,
        created_at: now,
        updated_at: now,
    };
    state.store.upsert_google_account(&acct_b).unwrap();
    let session_b_id = "sess_sub-user-b".to_string();
    {
        let mut sessions = state.active_sessions.lock().await;
        sessions.insert(
            session_b_id.clone(),
            ActiveSessionInfo {
                session_id: session_b_id.clone(),
                sub: "sub-user-b".to_string(),
                email: "userb@example.com".to_string(),
                display_name: "User B".to_string(),
                picture: None,
                tenant_id: "20000000-0000-0000-0000-000000000001".to_string(),
                workspace_id: "20000000-0000-0000-0000-000000000002".to_string(),
                expires_at: now + 3600,
            },
        );
    }
    let jwt_key_b = "account:sub-user-b:jwt_secret";
    let _ = state
        .credentials
        .set_secret(jwt_key_b, "test-jwt-secret-for-s6");
    let cookie_b = format!("session_id={}", session_b_id);

    let list_b = client
        .get(format!("{base_url}/api/v1/settings/credentials"))
        .header(HOST, host_str)
        .header(ORIGIN, &base_url)
        .header(COOKIE, &cookie_b)
        .send()
        .await
        .unwrap();
    assert_eq!(list_b.status(), 200);
    let list_b_json: Value = list_b.json().await.unwrap();
    assert!(list_b_json.as_array().unwrap().is_empty());

    let test_by_b = client
        .post(format!(
            "{base_url}/api/v1/settings/credentials/{cred_a_id}/test"
        ))
        .header(HOST, host_str)
        .header(ORIGIN, &base_url)
        .header(COOKIE, &cookie_b)
        .send()
        .await
        .unwrap();
    assert_eq!(test_by_b.status(), 404);

    let del_by_b = client
        .delete(format!(
            "{base_url}/api/v1/settings/credentials/{cred_a_id}"
        ))
        .header(HOST, host_str)
        .header(ORIGIN, &base_url)
        .header(COOKIE, &cookie_b)
        .send()
        .await
        .unwrap();
    assert_eq!(del_by_b.status(), 404);

    let secret_a_still_alive = state
        .credentials
        .get_secret(&format!("account:sub-user-a:provider:{}", cred_a_id))
        .unwrap();
    assert_eq!(secret_a_still_alive.as_deref(), Some(secret_a));
}

#[tokio::test]
async fn test_sign_out_purges_provider_secrets() {
    let mock_llm_url = start_mock_llm_server().await;
    let secret = "sk-ant-test-purge-secret-1111";

    let (base_url, state, cookie) = setup_app_and_account(
        "sub-user-purge",
        "purge@example.com",
        "30000000-0000-0000-0000-000000000001",
        "30000000-0000-0000-0000-000000000002",
    )
    .await;
    let client = reqwest::Client::new();
    let host_str = base_url.strip_prefix("http://").unwrap();

    let create_resp = client
        .post(format!("{base_url}/api/v1/settings/credentials"))
        .header(HOST, host_str)
        .header(ORIGIN, &base_url)
        .header(COOKIE, &cookie)
        .json(&json!({
            "preset_id": "anthropic",
            "api_key": secret,
            "base_url": mock_llm_url,
            "is_default": true
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(create_resp.status(), 200);
    let cred: Value = create_resp.json().await.unwrap();
    let cred_id = cred["id"].as_str().unwrap().to_string();

    assert!(state
        .credentials
        .get_secret(&format!("account:sub-user-purge:provider:{}", cred_id))
        .unwrap()
        .is_some());

    let signout_resp = client
        .post(format!("{base_url}/api/v1/auth/sign-out"))
        .header(HOST, host_str)
        .header(ORIGIN, &base_url)
        .header(COOKIE, &cookie)
        .send()
        .await
        .unwrap();
    assert_eq!(signout_resp.status(), 200);

    let secret_after_signout = state
        .credentials
        .get_secret(&format!("account:sub-user-purge:provider:{}", cred_id))
        .unwrap();
    assert_eq!(secret_after_signout, None);

    let conns_after_signout = state
        .store
        .get_provider_connections_by_profile("sub-user-purge")
        .unwrap();
    assert!(conns_after_signout.is_empty());
}
