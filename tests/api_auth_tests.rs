use nexus_albaai::checkpoints::SqliteCheckpointStore;
use nexus_albaai::server::auth::{
    sign_token, LocalSessionClaims, DEFAULT_JWT_SECRET, DEFAULT_LOCAL_TENANT_ID,
    DEFAULT_LOCAL_USER_DISPLAY_NAME, DEFAULT_LOCAL_USER_EMAIL, DEFAULT_LOCAL_USER_ID,
    DEFAULT_LOCAL_USER_PLAN, DEFAULT_LOCAL_WORKSPACE_ID,
};
use nexus_albaai::server::state::AppState;
use reqwest::header::AUTHORIZATION;
use serde_json::Value;
use std::sync::Arc;

async fn spawn_test_server() -> (String, tempfile::TempDir) {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("test.db");
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

#[tokio::test]
async fn test_health_endpoint() {
    let (base_url, _tmp) = spawn_test_server().await;
    let client = reqwest::Client::new();
    let res = client
        .get(format!("{}/health", base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), reqwest::StatusCode::OK);
    let body: Value = res.json().await.unwrap();
    assert_eq!(body["status"], "ok");
}

#[tokio::test]
async fn test_local_mode_endpoint() {
    let (base_url, _tmp) = spawn_test_server().await;
    let client = reqwest::Client::new();
    let res = client
        .get(format!("{}/api/v1/auth/local-mode", base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), reqwest::StatusCode::OK);
    let body: Value = res.json().await.unwrap();
    assert_eq!(body["enabled"], true);
}

#[tokio::test]
async fn test_local_session_endpoint() {
    let (base_url, _tmp) = spawn_test_server().await;
    let client = reqwest::Client::new();
    let res = client
        .post(format!("{}/api/v1/auth/local-session", base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), reqwest::StatusCode::OK);
    let body: Value = res.json().await.unwrap();
    let token = body["access_token"].as_str().unwrap();
    assert_eq!(token.split('.').count(), 3);
    assert_eq!(body["tenant_id"], DEFAULT_LOCAL_TENANT_ID);
    assert_eq!(body["workspace_id"], DEFAULT_LOCAL_WORKSPACE_ID);
    assert_eq!(body["expires_in"], 86400);
}

#[tokio::test]
async fn test_auth_me_with_valid_token() {
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
        .get(format!("{}/api/v1/auth/me", base_url))
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), reqwest::StatusCode::OK);
    let body: Value = res.json().await.unwrap();
    assert_eq!(body["email"], DEFAULT_LOCAL_USER_EMAIL);
    assert_eq!(body["display_name"], DEFAULT_LOCAL_USER_DISPLAY_NAME);
    assert_eq!(body["plan"], DEFAULT_LOCAL_USER_PLAN);
}

#[tokio::test]
async fn test_auth_me_missing_token_rejected() {
    let (base_url, _tmp) = spawn_test_server().await;
    let client = reqwest::Client::new();
    let res = client
        .get(format!("{}/api/v1/auth/me", base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), reqwest::StatusCode::UNAUTHORIZED);
    let body: Value = res.json().await.unwrap();
    assert_eq!(body["detail"], "Missing authorization header");
}

#[tokio::test]
async fn test_auth_me_invalid_token_rejected() {
    let (base_url, _tmp) = spawn_test_server().await;
    let client = reqwest::Client::new();
    let res = client
        .get(format!("{}/api/v1/auth/me", base_url))
        .header(AUTHORIZATION, "Bearer invalid-token-string")
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), reqwest::StatusCode::UNAUTHORIZED);
    let body: Value = res.json().await.unwrap();
    assert!(body["detail"].as_str().unwrap().contains("Invalid token"));
}

#[tokio::test]
async fn test_auth_me_expired_token_rejected() {
    let (base_url, _tmp) = spawn_test_server().await;
    let client = reqwest::Client::new();
    let expired_claims = LocalSessionClaims {
        sub: DEFAULT_LOCAL_USER_ID.to_string(),
        tenant_id: DEFAULT_LOCAL_TENANT_ID.to_string(),
        workspace_id: DEFAULT_LOCAL_WORKSPACE_ID.to_string(),
        email: DEFAULT_LOCAL_USER_EMAIL.to_string(),
        display_name: DEFAULT_LOCAL_USER_DISPLAY_NAME.to_string(),
        plan: DEFAULT_LOCAL_USER_PLAN.to_string(),
        exp: chrono::Utc::now().timestamp() - 3600,
    };
    let expired_token = sign_token(&expired_claims, DEFAULT_JWT_SECRET).unwrap();

    let res = client
        .get(format!("{}/api/v1/auth/me", base_url))
        .header(AUTHORIZATION, format!("Bearer {}", expired_token))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), reqwest::StatusCode::UNAUTHORIZED);
    let body: Value = res.json().await.unwrap();
    assert_eq!(body["detail"], "Token expired");
}

#[tokio::test]
async fn test_unknown_route_returns_404_json() {
    let (base_url, _tmp) = spawn_test_server().await;
    let client = reqwest::Client::new();
    let res = client
        .get(format!(
            "{}/api/v1/unknown-route-that-does-not-exist",
            base_url
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), reqwest::StatusCode::NOT_FOUND);
    let body: Value = res.json().await.unwrap();
    assert_eq!(body["detail"], "Not Found");
}
