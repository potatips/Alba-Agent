use nexus_albaai::checkpoints::SqliteCheckpointStore;
use nexus_albaai::credentials::{
    generate_secure_secret, CredentialStore, CredentialStoreError, MacosKeychainStore,
    MemoryCredentialStore,
};
use nexus_albaai::profiles::Profile;
use nexus_albaai::server::auth::{
    sign_token, LocalSessionClaims, DEFAULT_LOCAL_TENANT_ID, DEFAULT_LOCAL_USER_DISPLAY_NAME,
    DEFAULT_LOCAL_USER_EMAIL, DEFAULT_LOCAL_USER_PLAN, DEFAULT_LOCAL_WORKSPACE_ID,
};
use nexus_albaai::server::state::AppState;
use reqwest::header::AUTHORIZATION;
use serde_json::Value;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

struct FailingCredentialStore {
    fail_get: AtomicBool,
    fail_set: AtomicBool,
    fail_delete: AtomicBool,
    inner: MemoryCredentialStore,
}

impl FailingCredentialStore {
    fn new() -> Self {
        Self {
            fail_get: AtomicBool::new(false),
            fail_set: AtomicBool::new(false),
            fail_delete: AtomicBool::new(false),
            inner: MemoryCredentialStore::new(),
        }
    }
}

impl CredentialStore for FailingCredentialStore {
    fn get_secret(&self, key: &str) -> Result<Option<String>, CredentialStoreError> {
        if self.fail_get.load(Ordering::SeqCst) {
            return Err(CredentialStoreError::Platform(
                "simulated keychain read error".to_string(),
            ));
        }
        self.inner.get_secret(key)
    }

    fn set_secret(&self, key: &str, secret: &str) -> Result<(), CredentialStoreError> {
        if self.fail_set.load(Ordering::SeqCst) {
            return Err(CredentialStoreError::Platform(
                "simulated keychain write error".to_string(),
            ));
        }
        self.inner.set_secret(key, secret)
    }

    fn get_or_set_secret(
        &self,
        key: &str,
        generate: &dyn Fn() -> String,
    ) -> Result<String, CredentialStoreError> {
        if let Some(existing) = self.get_secret(key)? {
            if !existing.trim().is_empty() {
                return Ok(existing);
            }
        }
        let secret = generate();
        self.set_secret(key, &secret)?;
        Ok(secret)
    }

    fn delete_secret(&self, key: &str) -> Result<(), CredentialStoreError> {
        if self.fail_delete.load(Ordering::SeqCst) {
            return Err(CredentialStoreError::Platform(
                "simulated keychain delete error".to_string(),
            ));
        }
        self.inner.delete_secret(key)
    }

    fn delete_profile_secrets(&self, profile_id: &str) -> Result<(), CredentialStoreError> {
        if self.fail_delete.load(Ordering::SeqCst) {
            return Err(CredentialStoreError::Platform(
                "simulated keychain delete error".to_string(),
            ));
        }
        self.inner.delete_profile_secrets(profile_id)
    }
}

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
#[cfg(target_os = "macos")]
fn test_macos_keychain_store_real_ops() {
    if std::env::var("ALBA_TEST_KEYCHAIN").as_deref() != Ok("1") {
        return;
    }
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
fn test_credentials_module_does_not_use_process_command() {
    let source = include_str!("../src/credentials/mod.rs");
    assert!(!source.contains("std::process::Command"));
    assert!(!source.contains("Command::new"));
    assert!(!source.contains("add-generic-password"));
}

#[test]
fn test_get_or_set_secret_never_overwrites_on_read_failure() {
    let store = FailingCredentialStore::new();
    store
        .set_secret("profile:p1:jwt_secret", "existing-secret-1234")
        .unwrap();
    assert_eq!(
        store.get_secret("profile:p1:jwt_secret").unwrap(),
        Some("existing-secret-1234".to_string())
    );

    store.fail_get.store(true, Ordering::SeqCst);
    let res = store.get_or_set_secret("profile:p1:jwt_secret", &|| "overwritten".to_string());
    assert!(res.is_err());

    store.fail_get.store(false, Ordering::SeqCst);
    assert_eq!(
        store.get_secret("profile:p1:jwt_secret").unwrap(),
        Some("existing-secret-1234".to_string())
    );
}

#[test]
fn test_get_or_set_secret_propagates_set_failure() {
    let store = FailingCredentialStore::new();
    store.fail_set.store(true, Ordering::SeqCst);

    let res = store.get_or_set_secret("profile:p1:jwt_secret", &|| "unpersisted".to_string());
    assert!(res.is_err());

    store.fail_set.store(false, Ordering::SeqCst);
    assert_eq!(store.get_secret("profile:p1:jwt_secret").unwrap(), None);
}

#[test]
fn test_delete_secret_success_on_not_found_and_fails_closed_on_error() {
    let store = FailingCredentialStore::new();
    assert!(store.delete_secret("non_existent_key").is_ok());

    store.fail_delete.store(true, Ordering::SeqCst);
    assert!(store.delete_secret("any_key").is_err());
    assert!(store.delete_profile_secrets("profile-any").is_err());
}

#[test]
fn test_delete_profile_secrets_multi_secret_enumeration() {
    let store = MemoryCredentialStore::new();
    store
        .set_secret("profile:user1:jwt_secret", "jwt-val-1")
        .unwrap();
    store
        .set_secret("profile:user1:provider:openai:secret", "sk-proj-test-user1")
        .unwrap();
    store
        .set_secret(
            "profile:user1:provider:anthropic:secret",
            "sk-ant-test-user1",
        )
        .unwrap();
    store
        .set_secret("profile:user2:jwt_secret", "jwt-val-2")
        .unwrap();

    store.delete_profile_secrets("user1").unwrap();

    assert_eq!(store.get_secret("profile:user1:jwt_secret").unwrap(), None);
    assert_eq!(
        store
            .get_secret("profile:user1:provider:openai:secret")
            .unwrap(),
        None
    );
    assert_eq!(
        store
            .get_secret("profile:user1:provider:anthropic:secret")
            .unwrap(),
        None
    );
    assert_eq!(
        store.get_secret("profile:user2:jwt_secret").unwrap(),
        Some("jwt-val-2".to_string())
    );
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
    assert_eq!(
        default_profile.id,
        nexus_albaai::server::auth::DEFAULT_LOCAL_USER_ID
    );
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

    let old_default = store.get_profile(&default_profile.id).unwrap().unwrap();
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
