use nexus_albaai::checkpoints::SqliteCheckpointStore;
use nexus_albaai::credentials::{
    generate_secure_secret, CredentialStore, MacosKeychainStore, MemoryCredentialStore,
};
use nexus_albaai::profiles::Profile;
use nexus_albaai::server::auth::{
    sign_token, LocalSessionClaims, DEFAULT_LOCAL_TENANT_ID, DEFAULT_LOCAL_USER_DISPLAY_NAME,
    DEFAULT_LOCAL_USER_EMAIL, DEFAULT_LOCAL_USER_PLAN, DEFAULT_LOCAL_WORKSPACE_ID,
};
use nexus_albaai::server::state::AppState;
use reqwest::header::AUTHORIZATION;
use serde_json::Value;
use std::sync::Arc;

#[test]
fn test_credential_store_memory_implementation() {
    let store = MemoryCredentialStore::new();
    assert_eq!(store.get_secret("test:key").unwrap(), None);

    store.set_secret("test:key", "my-secret-val").unwrap();
    assert_eq!(
        store.get_secret("test:key").unwrap(),
        Some("my-secret-val".to_string())
    );

    store.set_secret("profile:p1:jwt_secret", "jwt-p1").unwrap();
    store
        .set_secret("profile:p1:provider:openai:secret", "sk-proj-test")
        .unwrap();
    store.set_secret("profile:p2:jwt_secret", "jwt-p2").unwrap();

    store.delete_profile_secrets("p1").unwrap();
    assert_eq!(store.get_secret("profile:p1:jwt_secret").unwrap(), None);
    assert_eq!(
        store
            .get_secret("profile:p1:provider:openai:secret")
            .unwrap(),
        None
    );
    assert_eq!(
        store.get_secret("profile:p2:jwt_secret").unwrap(),
        Some("jwt-p2".to_string())
    );

    store.delete_secret("test:key").unwrap();
    assert_eq!(store.get_secret("test:key").unwrap(), None);
}

#[test]
fn test_macos_keychain_store_real_ops() {
    let store = MacosKeychainStore::new("alba-test-s2-keychain");
    let key = format!("test-key-{}", uuid::Uuid::new_v4());
    let val = "super-secret-token-12345";

    assert_eq!(store.get_secret(&key).unwrap(), None);

    store.set_secret(&key, val).unwrap();
    assert_eq!(store.get_secret(&key).unwrap(), Some(val.to_string()));

    store.delete_secret(&key).unwrap();
    assert_eq!(store.get_secret(&key).unwrap(), None);
}

#[test]
fn test_sqlite_stores_zero_secrets_for_profiles() {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("profiles_test.db");
    let store = SqliteCheckpointStore::new(&db_path).unwrap();

    let conn = rusqlite::Connection::open(&db_path).unwrap();
    let mut stmt = conn.prepare("PRAGMA table_info(profiles)").unwrap();
    let mut rows = stmt.query([]).unwrap();
    let mut columns = Vec::new();
    while let Some(row) = rows.next().unwrap() {
        let name: String = row.get(1).unwrap();
        columns.push(name);
    }

    for col in &columns {
        let lower = col.to_lowercase();
        assert!(!lower.contains("secret"));
        assert!(!lower.contains("password"));
        assert!(!lower.contains("token"));
        assert!(!lower.contains("credential"));
        assert!(!lower.contains("key"));
    }

    let mut stmt2 = conn
        .prepare("PRAGMA table_info(provider_connections)")
        .unwrap();
    let mut rows2 = stmt2.query([]).unwrap();
    let mut columns2 = Vec::new();
    while let Some(row) = rows2.next().unwrap() {
        let name: String = row.get(1).unwrap();
        columns2.push(name);
    }

    for col in &columns2 {
        let lower = col.to_lowercase();
        assert!(!lower.contains("secret"));
        assert!(!lower.contains("password"));
        assert!(!lower.contains("token"));
        assert!(!lower.contains("credential"));
    }

    let p = Profile {
        id: "profile-alpha".to_string(),
        name: "Alpha User".to_string(),
        email: Some("alpha@example.com".to_string()),
        is_local: false,
        active: true,
        created_at: 1000,
        updated_at: 1000,
    };
    store.create_profile(&p).unwrap();

    let fetched = store.get_profile("profile-alpha").unwrap().unwrap();
    assert_eq!(fetched, p);

    let db_bytes = std::fs::read(&db_path).unwrap();
    let db_content = String::from_utf8_lossy(&db_bytes);
    assert!(!db_content.contains("sk-ant-"));
    assert!(!db_content.contains("sk-proj-"));
    assert!(!db_content.contains("AIzaSy"));
}

#[test]
fn test_random_jwt_secret_generated_per_profile() {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("jwt_test.db");
    let store = Arc::new(SqliteCheckpointStore::new(&db_path).unwrap());
    let memory_creds = Arc::new(MemoryCredentialStore::new());
    let state =
        AppState::new(store, tmp.path().to_path_buf()).with_credentials(memory_creds.clone());

    let secret1 = state.get_or_create_jwt_secret("profile-1").unwrap();
    let secret2 = state.get_or_create_jwt_secret("profile-2").unwrap();

    assert_eq!(secret1.len(), 64);
    assert_eq!(secret2.len(), 64);
    assert_ne!(secret1, secret2);

    let secret1_again = state.get_or_create_jwt_secret("profile-1").unwrap();
    assert_eq!(secret1, secret1_again);

    let stored1 = memory_creds
        .get_secret("profile:profile-1:jwt_secret")
        .unwrap();
    assert_eq!(stored1, Some(secret1.clone()));

    let db_bytes = std::fs::read(&db_path).unwrap();
    let db_content = String::from_utf8_lossy(&db_bytes);
    assert!(!db_content.contains(&secret1));
    assert!(!db_content.contains(&secret2));
}

#[tokio::test]
async fn test_token_signed_with_old_default_secret_rejected() {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("old_secret_test.db");
    let store = Arc::new(SqliteCheckpointStore::new(&db_path).unwrap());
    let state = AppState::new(store, tmp.path().to_path_buf());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let base_url = format!("http://127.0.0.1:{}", port);

    let state_clone = state.clone();
    tokio::spawn(async move {
        nexus_albaai::server::serve_app(state_clone, listener)
            .await
            .unwrap();
    });

    let client = reqwest::Client::new();
    let old_static_secret = "alba-local-development-secret-key-32b!";
    let claims = LocalSessionClaims {
        sub: "00000000-0000-0000-0000-000000000003".to_string(),
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
async fn test_cross_profile_token_isolation() {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("isolation_test.db");
    let store = Arc::new(SqliteCheckpointStore::new(&db_path).unwrap());
    let creds = Arc::new(MemoryCredentialStore::new());
    let state = AppState::new(store, tmp.path().to_path_buf()).with_credentials(creds);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let base_url = format!("http://127.0.0.1:{}", port);

    let state_clone = state.clone();
    tokio::spawn(async move {
        nexus_albaai::server::serve_app(state_clone, listener)
            .await
            .unwrap();
    });

    let secret_a = state.get_or_create_jwt_secret("profile-a").unwrap();
    let _secret_b = state.get_or_create_jwt_secret("profile-b").unwrap();

    let claims_b_signed_with_a = LocalSessionClaims {
        sub: "profile-b".to_string(),
        tenant_id: DEFAULT_LOCAL_TENANT_ID.to_string(),
        workspace_id: DEFAULT_LOCAL_WORKSPACE_ID.to_string(),
        email: "b@example.com".to_string(),
        display_name: "Profile B".to_string(),
        plan: "LOCAL".to_string(),
        exp: chrono::Utc::now().timestamp() + 3600,
    };
    let tampered_token = sign_token(&claims_b_signed_with_a, &secret_a).unwrap();

    let client = reqwest::Client::new();
    let res = client
        .get(format!("{}/api/v1/auth/me", base_url))
        .header(AUTHORIZATION, format!("Bearer {}", tampered_token))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), reqwest::StatusCode::UNAUTHORIZED);
    let body: Value = res.json().await.unwrap();
    assert_eq!(body["detail"], "Invalid token signature");
}

#[test]
fn test_profile_crud_in_sqlite() {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("crud_test.db");
    let store = SqliteCheckpointStore::new(&db_path).unwrap();

    let default_profile = store.ensure_default_profile().unwrap();
    assert_eq!(default_profile.id, "default");
    assert!(default_profile.active);

    let p2 = Profile {
        id: "profile-work".to_string(),
        name: "Work Profile".to_string(),
        email: Some("work@company.com".to_string()),
        is_local: false,
        active: false,
        created_at: 2000,
        updated_at: 2000,
    };
    store.create_profile(&p2).unwrap();

    let list = store.list_profiles().unwrap();
    assert_eq!(list.len(), 2);

    store.set_active_profile("profile-work").unwrap();
    let active = store.get_active_profile().unwrap().unwrap();
    assert_eq!(active.id, "profile-work");

    let old_default = store.get_profile("default").unwrap().unwrap();
    assert!(!old_default.active);

    let deleted = store.delete_profile("profile-work").unwrap();
    assert!(deleted);
    assert_eq!(store.get_profile("profile-work").unwrap(), None);
}

#[test]
fn test_generate_secure_secret_entropy() {
    let s1 = generate_secure_secret();
    let s2 = generate_secure_secret();
    assert_eq!(s1.len(), 64);
    assert_eq!(s2.len(), 64);
    assert_ne!(s1, s2);
}
