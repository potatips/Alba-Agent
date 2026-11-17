use axum::extract::State as AxumState;
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::post;
use axum::{Json, Router};
use chrono::Utc;
use nexus_albaai::checkpoints::{CheckpointStore, SqliteCheckpointStore};
use nexus_albaai::credentials::{CredentialStore, MemoryCredentialStore};
use nexus_albaai::domain::{ChatMessage, ExecutionScope, MessageRole};
use nexus_albaai::models::adapter::{AnthropicClient, GeminiClient, OpenAiCompatibleClient};
use nexus_albaai::models::types::{ModelConfig, ModelRequest};
use nexus_albaai::models::ModelClient;
use nexus_albaai::profiles::{GoogleAccountRecord, ProviderConnection};
use nexus_albaai::server::create_app;
use nexus_albaai::server::state::{ActiveSessionInfo, AppState};
use nexus_albaai::server::workflow_runner::ServerModelClient;
use reqwest::header::{COOKIE, HOST, ORIGIN};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::net::TcpListener;
use uuid::Uuid;

async fn setup_test_server(
    sub: &str,
    tenant_id: &str,
    workspace_id: &str,
) -> (String, AppState, String) {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("pinned_hosts_test.db");
    let store = Arc::new(SqliteCheckpointStore::new(&db_path).unwrap());
    let mut state = AppState::new(store.clone(), dir.path().to_path_buf());
    let cred_store = Arc::new(MemoryCredentialStore::new());
    state.credentials = cred_store.clone();

    let now = Utc::now().timestamp();
    let acct = GoogleAccountRecord {
        sub: sub.to_string(),
        email: format!("{}@example.com", sub),
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
                email: format!("{}@example.com", sub),
                display_name: format!("User {}", sub),
                picture: None,
                tenant_id: tenant_id.to_string(),
                workspace_id: workspace_id.to_string(),
                expires_at: now + 3600,
            },
        );
    }

    let jwt_key = format!("account:{sub}:jwt_secret");
    let _ = cred_store.set_secret(&jwt_key, "test-jwt-secret");

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
async fn test_official_presets_reject_client_base_url() {
    let (base_url, _state, cookie) = setup_test_server(
        "sub-official-check",
        "00000000-0000-0000-0000-000000000001",
        "00000000-0000-0000-0000-000000000002",
    )
    .await;
    let client = reqwest::Client::new();
    let host_str = base_url.strip_prefix("http://").unwrap();

    let official_presets = ["openai", "anthropic", "google_gemini", "deepseek"];
    for preset in official_presets {
        let resp = client
            .post(format!("{base_url}/api/v1/settings/credentials"))
            .header(HOST, host_str)
            .header(ORIGIN, &base_url)
            .header(COOKIE, &cookie)
            .json(&json!({
                "preset_id": preset,
                "api_key": "sk-test-key-12345678",
                "base_url": "http://127.0.0.1:8080"
            }))
            .send()
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let body: Value = resp.json().await.unwrap();
        let err_msg = body["detail"].as_str().unwrap();
        assert!(err_msg.contains("official preset"));
        assert!(err_msg.contains("base_url"));
    }
}

#[tokio::test]
async fn test_old_connection_with_wrong_base_url_in_sqlite_fails_closed() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("db_tamper.db");
    let store = Arc::new(SqliteCheckpointStore::new(&db_path).unwrap());
    let mut state = AppState::new(store.clone(), dir.path().to_path_buf());
    let cred_store = Arc::new(MemoryCredentialStore::new());
    state.credentials = cred_store.clone();

    let sub = "sub-tamper-check";
    let tenant_id = Uuid::new_v4();
    let workspace_id = Uuid::new_v4();
    let run_id = Uuid::new_v4();
    let scope = ExecutionScope::new(tenant_id, workspace_id, run_id).unwrap();
    let now = Utc::now().timestamp();

    let acct = GoogleAccountRecord {
        sub: sub.to_string(),
        email: "tamper@example.com".to_string(),
        display_name: "Tamper Check".to_string(),
        picture: None,
        tenant_id: tenant_id.to_string(),
        workspace_id: workspace_id.to_string(),
        active: true,
        created_at: now,
        updated_at: now,
    };
    store.upsert_google_account(&acct).unwrap();

    let conn_id = "conn-tampered-anthropic";
    let tampered_conn = ProviderConnection {
        id: conn_id.to_string(),
        profile_id: sub.to_string(),
        provider: "anthropic".to_string(),
        preset_id: "anthropic".to_string(),
        auth_type: "api_key".to_string(),
        account_label: None,
        status: "connected".to_string(),
        base_url: Some("http://127.0.0.1:9090".to_string()),
        is_default: true,
        available_models: vec!["claude-3-7-sonnet".to_string()],
        default_models: json!({ "chat": "claude-3-7-sonnet" }),
        settings: json!({}),
        masked_key: Some("sk-ant-...test".to_string()),
        safe_error: None,
        created_at: now,
        updated_at: now,
    };
    store
        .create_or_update_provider_connection(&tampered_conn)
        .unwrap();
    cred_store
        .set_secret(
            &format!("account:{}:provider:{}", sub, conn_id),
            "sk-ant-tampered-key",
        )
        .unwrap();

    let runner_client = ServerModelClient::for_account(
        store.clone(),
        cred_store.clone(),
        sub.to_string(),
        Some(scope),
    );

    let req = ModelRequest {
        messages: vec![ChatMessage {
            role: MessageRole::User,
            content: "hello".to_string(),
            tool_calls: None,
            tool_call_id: None,
        }],
        tools: Vec::new(),
        temperature: None,
        max_tokens: Some(5),
        model_override: None,
    };

    let result = runner_client.complete(&req).await;
    assert!(result.is_err());
    let err_str = format!("{:?}", result.unwrap_err());
    assert!(err_str.contains("ConfigurationError"));
    assert!(err_str.contains("host mismatch for official preset 'anthropic'"));

    let logs = store.list_audit_logs(&scope).await.unwrap();
    let fail_log = logs.iter().find(|l| l.action == "model.completion_failed");
    assert!(fail_log.is_some());
    let details = &fail_log.unwrap().details;
    let err_str = details["error"].as_str().unwrap();
    assert!(err_str.contains("host mismatch"));
}

#[derive(Clone)]
struct RedirectState {
    redirect_target: String,
    target_hits: Arc<AtomicUsize>,
}

async fn redirect_handler(
    AxumState(st): AxumState<RedirectState>,
    _headers: HeaderMap,
) -> impl IntoResponse {
    (
        StatusCode::TEMPORARY_REDIRECT,
        [("Location", st.redirect_target)],
    )
}

async fn target_handler(
    AxumState(st): AxumState<RedirectState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    st.target_hits.fetch_add(1, Ordering::SeqCst);
    let auth = headers
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .unwrap_or_default();
    (StatusCode::OK, Json(json!({"received_auth": auth})))
}

#[tokio::test]
async fn test_redirect_does_not_forward_api_key_or_follow() {
    let target_hits = Arc::new(AtomicUsize::new(0));

    let target_app = Router::new()
        .route("/target", post(target_handler))
        .route("/v1/chat/completions", post(target_handler))
        .route("/v1/messages", post(target_handler))
        .with_state(RedirectState {
            redirect_target: String::new(),
            target_hits: target_hits.clone(),
        });
    let target_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let target_addr = target_listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(target_listener, target_app).await.unwrap();
    });

    let target_url = format!("http://{}", target_addr);
    let redirect_app = Router::new()
        .route("/v1/chat/completions", post(redirect_handler))
        .route("/v1/messages", post(redirect_handler))
        .with_state(RedirectState {
            redirect_target: format!("{}/v1/chat/completions", target_url),
            target_hits: target_hits.clone(),
        });
    let redirect_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let redirect_addr = redirect_listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(redirect_listener, redirect_app).await.unwrap();
    });

    let redirect_url = format!("http://{}", redirect_addr);

    let openai_cfg = ModelConfig::new(&redirect_url, "sk-secret-never-leak-openai", "gpt-4o");
    let openai_client = OpenAiCompatibleClient::new(openai_cfg).unwrap();
    let req = ModelRequest {
        messages: vec![ChatMessage {
            role: MessageRole::User,
            content: "test redirect".to_string(),
            tool_calls: None,
            tool_call_id: None,
        }],
        tools: Vec::new(),
        temperature: None,
        max_tokens: Some(10),
        model_override: None,
    };

    let result = openai_client.complete(&req).await;
    assert!(result.is_err());
    assert_eq!(target_hits.load(Ordering::SeqCst), 0);

    let anthropic_cfg = ModelConfig::new(&redirect_url, "sk-ant-never-leak", "claude-3-7-sonnet");
    let anthropic_client = AnthropicClient::new(anthropic_cfg).unwrap();
    let result_ant = anthropic_client.complete(&req).await;
    assert!(result_ant.is_err());
    assert_eq!(target_hits.load(Ordering::SeqCst), 0);

    let gemini_cfg = ModelConfig::new(&redirect_url, "AIzaSy-never-leak", "gemini-1.5-flash");
    let gemini_client = GeminiClient::new(gemini_cfg).unwrap();
    let result_gem = gemini_client.complete(&req).await;
    assert!(result_gem.is_err());
    assert_eq!(target_hits.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn test_unknown_preset_rejected_with_400() {
    let (base_url, _state, cookie) = setup_test_server(
        "sub-unknown-preset",
        "00000000-0000-0000-0000-000000000001",
        "00000000-0000-0000-0000-000000000002",
    )
    .await;
    let client = reqwest::Client::new();
    let host_str = base_url.strip_prefix("http://").unwrap();

    let resp = client
        .post(format!("{base_url}/api/v1/settings/credentials"))
        .header(HOST, host_str)
        .header(ORIGIN, &base_url)
        .header(COOKIE, &cookie)
        .json(&json!({
            "preset_id": "malicious_or_unknown_preset",
            "api_key": "sk-12345678"
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let body: Value = resp.json().await.unwrap();
    let err_msg = body["detail"].as_str().unwrap();
    assert!(err_msg.to_lowercase().contains("unknown"));
}

#[tokio::test]
async fn test_custom_preset_validation_and_host_warning() {
    let (base_url, _state, cookie) = setup_test_server(
        "sub-custom-preset",
        "00000000-0000-0000-0000-000000000001",
        "00000000-0000-0000-0000-000000000002",
    )
    .await;
    let client = reqwest::Client::new();
    let host_str = base_url.strip_prefix("http://").unwrap();

    let resp_no_confirm = client
        .post(format!("{base_url}/api/v1/settings/credentials"))
        .header(HOST, host_str)
        .header(ORIGIN, &base_url)
        .header(COOKIE, &cookie)
        .json(&json!({
            "preset_id": "custom_openai",
            "api_key": "sk-custom-12345",
            "base_url": "http://127.0.0.1:11434/v1"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp_no_confirm.status(), StatusCode::BAD_REQUEST);
    let body: Value = resp_no_confirm.json().await.unwrap();
    assert!(body["detail"]
        .as_str()
        .unwrap()
        .contains("confirm_custom_host"));

    let resp_credentials_in_url = client
        .post(format!("{base_url}/api/v1/settings/credentials"))
        .header(HOST, host_str)
        .header(ORIGIN, &base_url)
        .header(COOKIE, &cookie)
        .json(&json!({
            "preset_id": "custom_openai",
            "api_key": "sk-custom-12345",
            "base_url": "http://user:pass@127.0.0.1:11434/v1",
            "confirm_custom_host": true
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp_credentials_in_url.status(), StatusCode::BAD_REQUEST);
    let body: Value = resp_credentials_in_url.json().await.unwrap();
    assert!(body["detail"]
        .as_str()
        .unwrap()
        .contains("credentials are not permitted in base_url"));

    let resp_non_loopback_http = client
        .post(format!("{base_url}/api/v1/settings/credentials"))
        .header(HOST, host_str)
        .header(ORIGIN, &base_url)
        .header(COOKIE, &cookie)
        .json(&json!({
            "preset_id": "custom_openai",
            "api_key": "sk-custom-12345",
            "base_url": "http://example.com/v1",
            "confirm_custom_host": true
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp_non_loopback_http.status(), StatusCode::BAD_REQUEST);
    let body: Value = resp_non_loopback_http.json().await.unwrap();
    assert!(body["detail"].as_str().unwrap().contains("loopback"));

    let resp_internal_domain = client
        .post(format!("{base_url}/api/v1/settings/credentials"))
        .header(HOST, host_str)
        .header(ORIGIN, &base_url)
        .header(COOKIE, &cookie)
        .json(&json!({
            "preset_id": "custom_openai",
            "api_key": "sk-custom-12345",
            "base_url": "https://service.internal/v1",
            "confirm_custom_host": true
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp_internal_domain.status(), StatusCode::BAD_REQUEST);
    let body: Value = resp_internal_domain.json().await.unwrap();
    assert!(body["detail"]
        .as_str()
        .unwrap()
        .contains("internal network"));

    let resp_valid = client
        .post(format!("{base_url}/api/v1/settings/credentials"))
        .header(HOST, host_str)
        .header(ORIGIN, &base_url)
        .header(COOKIE, &cookie)
        .json(&json!({
            "preset_id": "custom_openai",
            "api_key": "sk-custom-valid-key-12345",
            "base_url": "http://127.0.0.1:11434/v1",
            "confirm_custom_host": true
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp_valid.status(), StatusCode::OK);
    let body: Value = resp_valid.json().await.unwrap();
    assert_eq!(body["preset_id"], "custom_openai");
    let warning = body["warning"].as_str().unwrap();
    assert_eq!(warning, "khóa API sẽ được gửi tới 127.0.0.1");
}

#[tokio::test]
async fn test_create_credential_corrupt_uuid_fails_clearly() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("db_corrupt_uuid.db");
    let store = Arc::new(SqliteCheckpointStore::new(&db_path).unwrap());
    let mut state = AppState::new(store.clone(), dir.path().to_path_buf());
    let cred_store = Arc::new(MemoryCredentialStore::new());
    state.credentials = cred_store.clone();

    let sub = "sub-corrupt-uuid";
    let now = Utc::now().timestamp();
    let session_id = "sess_corrupt_uuid".to_string();
    {
        let mut sessions = state.active_sessions.lock().await;
        sessions.insert(
            session_id.clone(),
            ActiveSessionInfo {
                session_id: session_id.clone(),
                sub: sub.to_string(),
                email: "corrupt@example.com".to_string(),
                display_name: "Corrupt User".to_string(),
                picture: None,
                tenant_id: "corrupted-tenant-uuid".to_string(),
                workspace_id: "00000000-0000-0000-0000-000000000002".to_string(),
                expires_at: now + 3600,
            },
        );
    }
    let jwt_key = format!("account:{sub}:jwt_secret");
    let _ = cred_store.set_secret(&jwt_key, "test-jwt-secret");

    let app = create_app(state.clone());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let base_url = format!("http://{}", addr);
    let cookie = format!("session_id={}", session_id);
    let client = reqwest::Client::new();
    let host_str = base_url.strip_prefix("http://").unwrap();

    let resp = client
        .post(format!("{base_url}/api/v1/settings/credentials"))
        .header(HOST, host_str)
        .header(ORIGIN, &base_url)
        .header(COOKIE, &cookie)
        .json(&json!({
            "preset_id": "openai",
            "api_key": "sk-test-valid-length-key-12345"
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let body: Value = resp.json().await.unwrap();
    let err_msg = body["detail"].as_str().unwrap();
    assert!(err_msg.contains("invalid tenant_id in session claims"));
}
