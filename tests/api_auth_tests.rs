use nexus_albaai::checkpoints::SqliteCheckpointStore;
use nexus_albaai::server::auth::{
    sign_token, LocalSessionClaims, DEFAULT_LOCAL_TENANT_ID, DEFAULT_LOCAL_USER_DISPLAY_NAME,
    DEFAULT_LOCAL_USER_EMAIL, DEFAULT_LOCAL_USER_ID, DEFAULT_LOCAL_USER_PLAN,
    DEFAULT_LOCAL_WORKSPACE_ID,
};
use nexus_albaai::server::state::AppState;
use reqwest::header::AUTHORIZATION;
use serde_json::Value;
use std::sync::Arc;

async fn spawn_test_server() -> (String, AppState, tempfile::TempDir) {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("test.db");
    let store = Arc::new(SqliteCheckpointStore::new(&db_path).unwrap());
    let state = AppState::new(store, tmp.path().to_path_buf());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let base_url = format!("http://{}", addr);
    let state_clone = state.clone();
    tokio::spawn(async move {
        nexus_albaai::server::serve_app(state_clone, listener)
            .await
            .unwrap();
    });
    (base_url, state, tmp)
}

#[tokio::test]
async fn test_health_endpoint() {
    let (base_url, _state, _tmp) = spawn_test_server().await;
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
    let (base_url, _state, _tmp) = spawn_test_server().await;
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
    let (base_url, _state, _tmp) = spawn_test_server().await;
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
    let (base_url, _state, _tmp) = spawn_test_server().await;
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
    let (base_url, _state, _tmp) = spawn_test_server().await;
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
    let (base_url, _state, _tmp) = spawn_test_server().await;
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
    let (base_url, state, _tmp) = spawn_test_server().await;
    let client = reqwest::Client::new();
    let secret = state
        .get_or_create_jwt_secret(DEFAULT_LOCAL_USER_ID)
        .unwrap();
    let expired_claims = LocalSessionClaims {
        sub: DEFAULT_LOCAL_USER_ID.to_string(),
        tenant_id: DEFAULT_LOCAL_TENANT_ID.to_string(),
        workspace_id: DEFAULT_LOCAL_WORKSPACE_ID.to_string(),
        email: DEFAULT_LOCAL_USER_EMAIL.to_string(),
        display_name: DEFAULT_LOCAL_USER_DISPLAY_NAME.to_string(),
        plan: DEFAULT_LOCAL_USER_PLAN.to_string(),
        exp: chrono::Utc::now().timestamp() - 3600,
    };
    let expired_token = sign_token(&expired_claims, &secret).unwrap();

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
async fn test_auth_me_token_signed_with_old_default_secret_rejected() {
    let (base_url, _state, _tmp) = spawn_test_server().await;
    let client = reqwest::Client::new();
    let old_static_secret = "alba-local-development-secret-key-32b!";
    let claims = LocalSessionClaims {
        sub: DEFAULT_LOCAL_USER_ID.to_string(),
        tenant_id: DEFAULT_LOCAL_TENANT_ID.to_string(),
        workspace_id: DEFAULT_LOCAL_WORKSPACE_ID.to_string(),
        email: DEFAULT_LOCAL_USER_EMAIL.to_string(),
        display_name: DEFAULT_LOCAL_USER_DISPLAY_NAME.to_string(),
        plan: DEFAULT_LOCAL_USER_PLAN.to_string(),
        exp: chrono::Utc::now().timestamp() + 3600,
    };
    let forged_token = sign_token(&claims, old_static_secret).unwrap();

    let res = client
        .get(format!("{}/api/v1/auth/me", base_url))
        .header(AUTHORIZATION, format!("Bearer {}", forged_token))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), reqwest::StatusCode::UNAUTHORIZED);
    let body: Value = res.json().await.unwrap();
    assert_eq!(body["detail"], "Invalid token signature");
}

#[tokio::test]
async fn test_unknown_route_returns_404_json() {
    let (base_url, _state, _tmp) = spawn_test_server().await;
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

#[tokio::test]
async fn test_fresh_local_session_credential_lifecycle_and_honest_status() {
    let (base_url, _state, _tmp) = spawn_test_server().await;
    let client = reqwest::Client::new();

    let session_res = client
        .post(format!("{}/api/v1/auth/local-session", base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(session_res.status(), reqwest::StatusCode::OK);
    let session_body: Value = session_res.json().await.unwrap();
    let token = session_body["access_token"].as_str().unwrap();

    let cred_res = client
        .post(format!("{}/api/v1/settings/credentials", base_url))
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .json(&serde_json::json!({
            "preset_id": "custom_openai",
            "base_url": "https://api.openai.com/v1",
            "api_key": "sk-test-secret-12345678",
            "confirm_custom_host": true,
            "available_models": ["gpt-4o", "meta-llama/llama-3.3-70b"],
            "settings": {
                "custom_model_ids": "gpt-4o, meta-llama/llama-3.3-70b"
            }
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(cred_res.status(), reqwest::StatusCode::OK);
    let cred_body: Value = cred_res.json().await.unwrap();
    let cred_id = cred_body["id"].as_str().unwrap();
    assert_eq!(cred_body["preset_id"], "custom_openai");
    assert_eq!(cred_body["status"], "UNCONFIGURED");
    let models = cred_body["available_models"].as_array().unwrap();
    assert_eq!(models.len(), 2);
    assert_eq!(models[0], "gpt-4o");
    assert_eq!(models[1], "meta-llama/llama-3.3-70b");
    assert!(!models.iter().any(|m| m == "default"));

    let shop_res = client
        .post(format!("{}/api/v1/settings/credentials", base_url))
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .json(&serde_json::json!({
            "preset_id": "shopaikey",
            "base_url": "https://api.shopaikey.com/v1",
            "api_key": "sk-shop-secret-abcdef",
            "confirm_custom_host": true,
            "settings": {
                "custom_model_ids": "gpt-4o, claude-3-5-sonnet"
            }
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(shop_res.status(), reqwest::StatusCode::OK);
    let shop_body: Value = shop_res.json().await.unwrap();
    assert_eq!(shop_body["preset_id"], "shopaikey");
    assert_eq!(shop_body["status"], "UNCONFIGURED");
    let shop_models = shop_body["available_models"].as_array().unwrap();
    assert_eq!(shop_models[0], "gpt-4o");
    assert_eq!(shop_models[1], "claude-3-5-sonnet");
    assert!(!shop_models.iter().any(|m| m == "default"));

    let test_res = client
        .post(format!(
            "{}/api/v1/settings/credentials/{}/test",
            base_url, cred_id
        ))
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .send()
        .await
        .unwrap();
    assert_eq!(test_res.status(), reqwest::StatusCode::OK);
    let test_body: Value = test_res.json().await.unwrap();
    assert_eq!(test_body["status"], "ERROR");
    let err_str = test_body["error"].as_str().unwrap_or("");
    assert!(!err_str.contains("sk-test-secret-12345678"));

    let chat_models_res = client
        .get(format!("{}/api/v1/settings/chat-models", base_url))
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .send()
        .await
        .unwrap();
    assert_eq!(chat_models_res.status(), reqwest::StatusCode::OK);
    let chat_models_body: Value = chat_models_res.json().await.unwrap();
    assert!(chat_models_body["models"].is_array());
}
