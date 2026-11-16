use axum::extract::{Form, Json};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::Router;
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use nexus_albaai::checkpoints::SqliteCheckpointStore;
use nexus_albaai::credentials::MemoryCredentialStore;
use nexus_albaai::server::create_app;
use nexus_albaai::server::state::AppState;
use reqwest::header::{COOKIE, HOST, ORIGIN, SET_COOKIE};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::net::TcpListener;

const TEST_RSA_KEY_ID: &str = "test-key-id-1";
const TEST_RSA_KEY_ID_ROTATED: &str = "test-key-id-2";

const TEST_RSA_PRIVATE_PEM: &[u8] = b"-----BEGIN RSA PRIVATE KEY-----
MIIEpAIBAAKCAQEAuqRStMR5ADSRH1jJAzMpO7T8Vmb4xzwBurNvJGywd3xy8187
IRFAdf6Nm+EoCC77Uj/9qZik2UQZmumo53w2Z33kiireGLCbJj6aXgupoKCjBAQT
dsprm1f0FsZfnMjEcrg8GBikmrpzdAiyOAZevcgl++7QMXL4DW8AVzVARmnQ2Bjp
CuUY/HDwj43K3CZmJCYc4CI04AsCUVLS+nAX+Oq5+/RdQGt+ODRRTpoXIKf2FgJf
F4SnzRdMw53oQQefNfjdERr9BhKD5lJRkiErPO3pPVmvxAwPzFwBxPY71zoJaxIp
E8DhKKmrFT3HTal1Ksp6QU99ncoyDhV5AOqojwIDAQABAoIBABR4eWnCMtNG8LPT
dRiYv+kAjLydRqPzfO0RSRthoGk6iREQXNffoIZQiQ+TvmRHYuE+i3/SciIySeyn
vBLHmN/4+tv8n6dtl1a2fw/Zj51CgR6ro8bfpLXBC9CDqXD0xDcg5dZuvX8K/Ca5
3XIvoKyIC0J6ivMhfPtcNVlbSdVvDaf1e/jtXLj4j9mtv0wdQvT7zB9xLYWLhgd9
EEUL8nbfm8Mzg03r2h06vCGPYhNO8xmBPZj5TVgKMV2ykZAYmkvr2It/UYTAE0x/
CImWeX7gXEv9eIT7dRXBD7tCbYuJ8+UqNbAFMbkc/XyOsZJGKAwvkD8Vuq65OwrI
OWz78SECgYEA5rejbZuX8SJr2IlnxzZdb0Jcedjh+jSPsuOwX0jlxjk3PzeNa66+
0NqHKU2pBjuaFgCKgjhUMJVWjaMqTXrGZ6SWgySG8tzhZliT44I6Ni7ecAZLFxRQ
7CnelhhwanWlPbXrCBYvTaxuSiuw8UfSAg0BQ7Rbf3rrwMNuSSIl83MCgYEAzxg6
FDVF1NtmFGH6Ywd+EE1jmKfmKQzjCTsX/WB5GWB21x+aI85kcutARMUgO63ixEm9
IVDpS7hEnTmrVSjCfDb9C7y3edjJsB/TDL10BmMK8bkzwp/rN4kyuAVXNHaKapTh
yrkV2+pBslgXqIsxi0BGa3OGYNg5IF/DYvhcx3UCgYAKl8ngIXGzd5IeCgkCw4M8
tX5LdeLCOyGR5BRX+5/ZHw1jUKiklgxayH3GLrOg/qGPewzg97cd1+12aVRq5EUx
wwPKF6Y+hleu/Egl/PexPyulVxShhaRFg5Kdr5eRu78Pe0fEQkh/t3gE3cas/VAu
GezB8Aj3bGxgmFh68R9u8wKBgQCRjraro3HslTSfWOozFn4NmUdKcaA89BjTURyg
c2FKa9m7hDPDvPovJ5hpfaAVMjezvRxYwVToaqCXXYRF+7sOsVuUF1ViJ/RCfWvK
hZJ6Nrznf0OZJOufgigK3X4YmsTY+sHkychKZNE+N9qRFRyCgm+8cFtf0Dn+vjBH
bMdRTQKBgQDT27wf6Tl4ayxyKapcAQ2mMcfDf4X4NUYBmZPA6AcvfnPGVg9DqPvY
K91IE1VU4cwrqiJj7vXGf21lEPvQsDlAcYgKSSKqHAozSuxC4Sfo51cIbVTA36s5
hGUJgCmuQqF79fQn/2id28L0gxQIO1wg2Rv0L/iuf3StlSXI8i+7SA==
-----END RSA PRIVATE KEY-----";

const TEST_RSA_MODULUS_N: &str = "uqRStMR5ADSRH1jJAzMpO7T8Vmb4xzwBurNvJGywd3xy8187IRFAdf6Nm-EoCC77Uj_9qZik2UQZmumo53w2Z33kiireGLCbJj6aXgupoKCjBAQTdsprm1f0FsZfnMjEcrg8GBikmrpzdAiyOAZevcgl--7QMXL4DW8AVzVARmnQ2BjpCuUY_HDwj43K3CZmJCYc4CI04AsCUVLS-nAX-Oq5-_RdQGt-ODRRTpoXIKf2FgJfF4SnzRdMw53oQQefNfjdERr9BhKD5lJRkiErPO3pPVmvxAwPzFwBxPY71zoJaxIpE8DhKKmrFT3HTal1Ksp6QU99ncoyDhV5AOqojw";
const TEST_RSA_EXPONENT_E: &str = "AQAB";

#[derive(Clone)]
struct MockGoogleState {
    revocation_count: Arc<AtomicUsize>,
    include_rotated_key: Arc<std::sync::atomic::AtomicBool>,
    code_exchange_override: Arc<std::sync::Mutex<Option<String>>>,
    active_nonce: Arc<std::sync::Mutex<Option<String>>>,
    client_id: String,
    revoke_status: Arc<std::sync::atomic::AtomicU16>,
    revoke_delay_ms: Arc<std::sync::atomic::AtomicU64>,
}

fn new_mock_state(client_id: &str) -> MockGoogleState {
    MockGoogleState {
        revocation_count: Arc::new(AtomicUsize::new(0)),
        include_rotated_key: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        code_exchange_override: Arc::new(std::sync::Mutex::new(None)),
        active_nonce: Arc::new(std::sync::Mutex::new(None)),
        client_id: client_id.to_string(),
        revoke_status: Arc::new(std::sync::atomic::AtomicU16::new(200)),
        revoke_delay_ms: Arc::new(std::sync::atomic::AtomicU64::new(0)),
    }
}

#[derive(Serialize)]
struct MockJwksKey {
    kty: &'static str,
    alg: &'static str,
    kid: String,
    n: &'static str,
    e: &'static str,
}

#[derive(Serialize)]
struct MockJwksResponse {
    keys: Vec<MockJwksKey>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct MockTokenExchangeForm {
    code: String,
    client_id: String,
    redirect_uri: String,
    grant_type: String,
    code_verifier: String,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct MockRevokeForm {
    token: String,
}

#[derive(Serialize, Deserialize, Clone)]
struct TestTokenClaims {
    sub: String,
    email: Option<String>,
    name: Option<String>,
    picture: Option<String>,
    iss: String,
    aud: String,
    exp: i64,
    nonce: Option<String>,
}

fn create_signed_id_token(
    claims: &TestTokenClaims,
    kid: &str,
    alg: Algorithm,
    encoding_key: &EncodingKey,
) -> String {
    let mut header = Header::new(alg);
    header.kid = Some(kid.to_string());
    encode(&header, claims, encoding_key).unwrap()
}

async fn start_mock_google_server(state: MockGoogleState) -> (String, SocketAddr) {
    let app = Router::new()
        .route(
            "/oauth2/v3/certs",
            get({
                let st = state.clone();
                move || {
                    let st = st.clone();
                    async move {
                        let mut keys = vec![MockJwksKey {
                            kty: "RSA",
                            alg: "RS256",
                            kid: TEST_RSA_KEY_ID.to_string(),
                            n: TEST_RSA_MODULUS_N,
                            e: TEST_RSA_EXPONENT_E,
                        }];
                        if st.include_rotated_key.load(Ordering::SeqCst) {
                            keys.push(MockJwksKey {
                                kty: "RSA",
                                alg: "RS256",
                                kid: TEST_RSA_KEY_ID_ROTATED.to_string(),
                                n: TEST_RSA_MODULUS_N,
                                e: TEST_RSA_EXPONENT_E,
                            });
                        }
                        Json(MockJwksResponse { keys })
                    }
                }
            }),
        )
        .route(
            "/oauth2/v2/token",
            post({
                let st = state.clone();
                move |Form(_form): Form<MockTokenExchangeForm>| {
                    let st = st.clone();
                    async move {
                        if let Some(override_id) =
                            st.code_exchange_override.lock().unwrap().as_ref()
                        {
                            return Json(serde_json::json!({
                                "access_token": "mock-access-token",
                                "refresh_token": "mock-refresh-token",
                                "id_token": override_id,
                                "token_type": "Bearer",
                                "expires_in": 3600
                            }))
                            .into_response();
                        }
                        let now = chrono::Utc::now().timestamp();
                        let nonce = st.active_nonce.lock().unwrap().clone();
                        let claims = TestTokenClaims {
                            sub: "google-sub-12345".to_string(),
                            email: Some("dev@example.com".to_string()),
                            name: Some("Alba Developer".to_string()),
                            picture: Some("https://example.com/avatar.png".to_string()),
                            iss: "https://accounts.google.com".to_string(),
                            aud: st.client_id.clone(),
                            exp: now + 3600,
                            nonce,
                        };
                        let enc_key = EncodingKey::from_rsa_pem(TEST_RSA_PRIVATE_PEM).unwrap();
                        let id_token = create_signed_id_token(
                            &claims,
                            TEST_RSA_KEY_ID,
                            Algorithm::RS256,
                            &enc_key,
                        );
                        Json(serde_json::json!({
                            "access_token": "mock-access-token",
                            "refresh_token": "mock-refresh-token",
                            "id_token": id_token,
                            "token_type": "Bearer",
                            "expires_in": 3600
                        }))
                        .into_response()
                    }
                }
            }),
        )
        .route(
            "/oauth2/v2/revoke",
            post({
                let st = state.clone();
                move |Form(_form): Form<MockRevokeForm>| {
                    let st = st.clone();
                    async move {
                        st.revocation_count.fetch_add(1, Ordering::SeqCst);
                        let delay = st.revoke_delay_ms.load(Ordering::SeqCst);
                        if delay > 0 {
                            tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;
                        }
                        let code = st.revoke_status.load(Ordering::SeqCst);
                        axum::http::StatusCode::from_u16(code).unwrap_or(axum::http::StatusCode::OK)
                    }
                }
            }),
        );

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let base_url = format!("http://{}", addr);

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    (base_url, addr)
}

async fn setup_test_server(mock_base_url: &str, client_id: &str) -> (String, AppState) {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("alba_test.db");
    let store = Arc::new(SqliteCheckpointStore::new(&db_path).unwrap());
    let creds = Arc::new(MemoryCredentialStore::new());

    let state = AppState::new(store, tmp.path().to_path_buf())
        .with_credentials(creds)
        .with_allow_local_auth(true)
        .with_oauth_config(
            Some(client_id.to_string()),
            format!("{mock_base_url}/o/oauth2/v2/auth"),
            format!("{mock_base_url}/oauth2/v2/token"),
            format!("{mock_base_url}/oauth2/v3/certs"),
            format!("{mock_base_url}/oauth2/v2/revoke"),
        );

    let app = create_app(state.clone());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let base_url = format!("http://{}", addr);

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    (base_url, state)
}

#[derive(Clone)]
struct FaultyCredentialStore {
    inner: MemoryCredentialStore,
    fail_get_keys: Arc<std::sync::Mutex<Vec<String>>>,
    fail_delete_keys: Arc<std::sync::Mutex<Vec<String>>>,
}

impl FaultyCredentialStore {
    fn new() -> Self {
        Self {
            inner: MemoryCredentialStore::new(),
            fail_get_keys: Arc::new(std::sync::Mutex::new(Vec::new())),
            fail_delete_keys: Arc::new(std::sync::Mutex::new(Vec::new())),
        }
    }

    fn fail_get_for(&self, key: &str) {
        self.fail_get_keys.lock().unwrap().push(key.to_string());
    }

    fn fail_delete_for(&self, key: &str) {
        self.fail_delete_keys.lock().unwrap().push(key.to_string());
    }
}

impl nexus_albaai::credentials::CredentialStore for FaultyCredentialStore {
    fn get_secret(
        &self,
        key: &str,
    ) -> Result<Option<String>, nexus_albaai::credentials::CredentialStoreError> {
        if self
            .fail_get_keys
            .lock()
            .unwrap()
            .iter()
            .any(|k| key.contains(k))
        {
            return Err(nexus_albaai::credentials::CredentialStoreError::Platform(
                "Injected get error".to_string(),
            ));
        }
        self.inner.get_secret(key)
    }

    fn set_secret(
        &self,
        key: &str,
        secret: &str,
    ) -> Result<(), nexus_albaai::credentials::CredentialStoreError> {
        self.inner.set_secret(key, secret)
    }

    fn get_or_set_secret(
        &self,
        key: &str,
        generate: &dyn Fn() -> String,
    ) -> Result<String, nexus_albaai::credentials::CredentialStoreError> {
        self.inner.get_or_set_secret(key, generate)
    }

    fn delete_secret(
        &self,
        key: &str,
    ) -> Result<(), nexus_albaai::credentials::CredentialStoreError> {
        if self
            .fail_delete_keys
            .lock()
            .unwrap()
            .iter()
            .any(|k| key.contains(k))
        {
            return Err(nexus_albaai::credentials::CredentialStoreError::Platform(
                "Injected delete error".to_string(),
            ));
        }
        self.inner.delete_secret(key)
    }

    fn delete_profile_secrets(
        &self,
        profile_id: &str,
    ) -> Result<(), nexus_albaai::credentials::CredentialStoreError> {
        self.inner.delete_profile_secrets(profile_id)
    }
}

async fn setup_test_server_with_creds(
    mock_base_url: &str,
    client_id: &str,
    creds: Arc<dyn nexus_albaai::credentials::CredentialStore>,
) -> (String, AppState) {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("alba_test.db");
    let store = Arc::new(SqliteCheckpointStore::new(&db_path).unwrap());

    let state = AppState::new(store, tmp.path().to_path_buf())
        .with_credentials(creds)
        .with_allow_local_auth(true)
        .with_oauth_config(
            Some(client_id.to_string()),
            format!("{mock_base_url}/o/oauth2/v2/auth"),
            format!("{mock_base_url}/oauth2/v2/token"),
            format!("{mock_base_url}/oauth2/v3/certs"),
            format!("{mock_base_url}/oauth2/v2/revoke"),
        );

    let app = create_app(state.clone());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let base_url = format!("http://{}", addr);

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    (base_url, state)
}

async fn login_and_get_cookie(
    client: &reqwest::Client,
    base_url: &str,
    mock_state: &MockGoogleState,
) -> String {
    let host_str = base_url.strip_prefix("http://").unwrap();
    let start_resp = client
        .post(format!("{base_url}/api/v1/auth/google/start"))
        .header(HOST, host_str)
        .header(ORIGIN, base_url)
        .send()
        .await
        .unwrap();
    let start_body: Value = start_resp.json().await.unwrap();
    let auth_url = start_body.get("auth_url").unwrap().as_str().unwrap();
    let nonce_val = auth_url
        .split("&nonce=")
        .nth(1)
        .unwrap()
        .split('&')
        .next()
        .unwrap();
    *mock_state.active_nonce.lock().unwrap() = Some(nonce_val.to_string());
    let state_str = start_body.get("state").unwrap().as_str().unwrap();
    let handoff_id = start_body.get("handoff_id").unwrap().as_str().unwrap();

    client
        .get(format!(
            "{base_url}/api/v1/auth/google/callback?code=mock-code&state={state_str}"
        ))
        .header(HOST, host_str)
        .send()
        .await
        .unwrap();

    let handoff_resp = client
        .post(format!("{base_url}/api/v1/auth/google/handoff"))
        .header(HOST, host_str)
        .header(ORIGIN, base_url)
        .json(&serde_json::json!({ "handoff_id": handoff_id }))
        .send()
        .await
        .unwrap();
    let cookie_header = handoff_resp
        .headers()
        .get(SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap();
    cookie_header.split(';').next().unwrap().to_string()
}

#[tokio::test]
async fn test_google_start_returns_auth_url_and_handoff_id() {
    let mock_state = new_mock_state("test-client-id");
    let (mock_url, _) = start_mock_google_server(mock_state).await;
    let (base_url, state) = setup_test_server(&mock_url, "test-client-id").await;

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{base_url}/api/v1/auth/google/start"))
        .header(HOST, base_url.strip_prefix("http://").unwrap())
        .header(ORIGIN, &base_url)
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.unwrap();

    let auth_url = body.get("auth_url").and_then(|v| v.as_str()).unwrap();
    let state_str = body.get("state").and_then(|v| v.as_str()).unwrap();
    let handoff_id = body.get("handoff_id").and_then(|v| v.as_str()).unwrap();

    assert!(auth_url.contains("test-client-id"));
    assert!(auth_url.contains(state_str));
    assert!(auth_url.contains("code_challenge="));
    assert!(auth_url.contains("code_challenge_method=S256"));
    assert!(uuid::Uuid::parse_str(handoff_id).is_ok());

    let pending = state.pending_oauth.lock().await;
    assert!(pending.contains_key(state_str));
}

#[tokio::test]
async fn test_origin_and_host_validation_rules() {
    let mock_state = new_mock_state("test-client-id");
    let (mock_url, _) = start_mock_google_server(mock_state).await;
    let (base_url, _) = setup_test_server(&mock_url, "test-client-id").await;
    let host_str = base_url.strip_prefix("http://").unwrap();

    let client = reqwest::Client::new();

    let resp_no_host = client
        .post(format!("{base_url}/api/v1/auth/google/start"))
        .header(ORIGIN, &base_url)
        .send()
        .await
        .unwrap();
    assert_eq!(resp_no_host.status(), 200);

    let resp_bad_host = client
        .post(format!("{base_url}/api/v1/auth/google/start"))
        .header(HOST, "evil-attacker.com")
        .header(ORIGIN, &base_url)
        .send()
        .await
        .unwrap();
    assert_eq!(resp_bad_host.status(), 403);

    let resp_no_origin = client
        .post(format!("{base_url}/api/v1/auth/google/start"))
        .header(HOST, host_str)
        .send()
        .await
        .unwrap();
    assert_eq!(resp_no_origin.status(), 403);

    let resp_bad_origin = client
        .post(format!("{base_url}/api/v1/auth/google/start"))
        .header(HOST, host_str)
        .header(ORIGIN, "http://malicious-website.com")
        .send()
        .await
        .unwrap();
    assert_eq!(resp_bad_origin.status(), 403);

    let resp_tauri_origin = client
        .post(format!("{base_url}/api/v1/auth/google/start"))
        .header(HOST, host_str)
        .header(ORIGIN, "tauri://localhost")
        .send()
        .await
        .unwrap();
    assert_eq!(resp_tauri_origin.status(), 200);
}

#[tokio::test]
async fn test_callback_state_verification_and_code_replay() {
    let mock_state = new_mock_state("test-client-id");
    let (mock_url, _) = start_mock_google_server(mock_state.clone()).await;
    let (base_url, _) = setup_test_server(&mock_url, "test-client-id").await;
    let host_str = base_url.strip_prefix("http://").unwrap();
    let client = reqwest::Client::new();

    let start_resp = client
        .post(format!("{base_url}/api/v1/auth/google/start"))
        .header(HOST, host_str)
        .header(ORIGIN, &base_url)
        .send()
        .await
        .unwrap();
    let start_body: Value = start_resp.json().await.unwrap();
    let auth_url = start_body.get("auth_url").unwrap().as_str().unwrap();
    let nonce_val = auth_url
        .split("&nonce=")
        .nth(1)
        .unwrap()
        .split('&')
        .next()
        .unwrap();
    *mock_state.active_nonce.lock().unwrap() = Some(nonce_val.to_string());
    let state_str = start_body.get("state").unwrap().as_str().unwrap();

    let resp_no_state = client
        .get(format!(
            "{base_url}/api/v1/auth/google/callback?code=mock-code"
        ))
        .header(HOST, host_str)
        .send()
        .await
        .unwrap();
    assert_eq!(resp_no_state.status(), 400);

    let resp_no_code = client
        .get(format!(
            "{base_url}/api/v1/auth/google/callback?state={state_str}"
        ))
        .header(HOST, host_str)
        .send()
        .await
        .unwrap();
    assert_eq!(resp_no_code.status(), 400);

    let resp_invalid_state = client
        .get(format!(
            "{base_url}/api/v1/auth/google/callback?code=mock-code&state=nonexistent-state"
        ))
        .header(HOST, host_str)
        .send()
        .await
        .unwrap();
    assert_eq!(resp_invalid_state.status(), 401);

    let resp_success = client
        .get(format!(
            "{base_url}/api/v1/auth/google/callback?code=mock-code&state={state_str}"
        ))
        .header(HOST, host_str)
        .send()
        .await
        .unwrap();
    assert_eq!(resp_success.status(), 200);

    let resp_replay = client
        .get(format!(
            "{base_url}/api/v1/auth/google/callback?code=mock-code&state={state_str}"
        ))
        .header(HOST, host_str)
        .send()
        .await
        .unwrap();
    assert_eq!(resp_replay.status(), 401);
}

async fn trigger_pending_flow(state: &AppState, now: i64) -> (String, String) {
    let flow_state = nexus_albaai::credentials::generate_secure_secret();
    let nonce = nexus_albaai::credentials::generate_secure_secret();
    let verifier = nexus_albaai::credentials::generate_secure_secret();
    let handoff_id = uuid::Uuid::new_v4().to_string();
    let flow = nexus_albaai::server::state::PendingOauthFlow {
        state: flow_state.clone(),
        nonce: nonce.clone(),
        code_verifier: verifier,
        handoff_id,
        expires_at: now + 600,
    };
    state
        .pending_oauth
        .lock()
        .await
        .insert(flow_state.clone(), flow);
    (flow_state, nonce)
}

#[tokio::test]
async fn test_id_token_jwks_rs256_verification_and_tampering_cases() {
    let mock_state = new_mock_state("test-client-id");
    let (mock_url, _) = start_mock_google_server(mock_state.clone()).await;
    let (base_url, state) = setup_test_server(&mock_url, "test-client-id").await;
    let host_str = base_url.strip_prefix("http://").unwrap();
    let client = reqwest::Client::new();

    let now = chrono::Utc::now().timestamp();
    let enc_key = EncodingKey::from_rsa_pem(TEST_RSA_PRIVATE_PEM).unwrap();

    let valid_claims = TestTokenClaims {
        sub: "user-test-sub".to_string(),
        email: Some("user@example.com".to_string()),
        name: Some("User".to_string()),
        picture: None,
        iss: "https://accounts.google.com".to_string(),
        aud: "test-client-id".to_string(),
        exp: now + 3600,
        nonce: None,
    };

    let none_token = "eyJhbGciOiJub25lIiwidHlwIjoiSldUIn0.eyJzdWIiOiIxMjM0NSIsImF1ZCI6InRlc3QtY2xpZW50LWlkIiwiaXNzIjoiaHR0cHM6Ly9hY2NvdW50cy5nb29nbGUuY29tIiwiZXhwIjoyMDAwMDAwMDAwfQ.";
    *mock_state.code_exchange_override.lock().unwrap() = Some(none_token.to_string());
    let (st1, _) = trigger_pending_flow(&state, now).await;
    let resp_none = client
        .get(format!(
            "{base_url}/api/v1/auth/google/callback?code=mock-code&state={st1}"
        ))
        .header(HOST, host_str)
        .send()
        .await
        .unwrap();
    assert_eq!(resp_none.status(), 401);

    let hs256_key = EncodingKey::from_secret(TEST_RSA_MODULUS_N.as_bytes());
    let hs256_token = encode(&Header::new(Algorithm::HS256), &valid_claims, &hs256_key).unwrap();
    *mock_state.code_exchange_override.lock().unwrap() = Some(hs256_token);
    let (st2, _) = trigger_pending_flow(&state, now).await;
    let resp_hs256 = client
        .get(format!(
            "{base_url}/api/v1/auth/google/callback?code=mock-code&state={st2}"
        ))
        .header(HOST, host_str)
        .send()
        .await
        .unwrap();
    assert_eq!(resp_hs256.status(), 401);

    let unknown_kid_token =
        create_signed_id_token(&valid_claims, "unknown-kid-999", Algorithm::RS256, &enc_key);
    *mock_state.code_exchange_override.lock().unwrap() = Some(unknown_kid_token);
    let (st3, _) = trigger_pending_flow(&state, now).await;
    let resp_unk_kid = client
        .get(format!(
            "{base_url}/api/v1/auth/google/callback?code=mock-code&state={st3}"
        ))
        .header(HOST, host_str)
        .send()
        .await
        .unwrap();
    assert_eq!(resp_unk_kid.status(), 401);

    let mut expired_claims = valid_claims.clone();
    expired_claims.exp = now - 3600;
    let expired_token =
        create_signed_id_token(&expired_claims, TEST_RSA_KEY_ID, Algorithm::RS256, &enc_key);
    *mock_state.code_exchange_override.lock().unwrap() = Some(expired_token);
    let (st4, _) = trigger_pending_flow(&state, now).await;
    let resp_exp = client
        .get(format!(
            "{base_url}/api/v1/auth/google/callback?code=mock-code&state={st4}"
        ))
        .header(HOST, host_str)
        .send()
        .await
        .unwrap();
    assert_eq!(resp_exp.status(), 401);

    let mut wrong_aud_claims = valid_claims.clone();
    wrong_aud_claims.aud = "wrong-client-id".to_string();
    let wrong_aud_token = create_signed_id_token(
        &wrong_aud_claims,
        TEST_RSA_KEY_ID,
        Algorithm::RS256,
        &enc_key,
    );
    *mock_state.code_exchange_override.lock().unwrap() = Some(wrong_aud_token);
    let (st5, _) = trigger_pending_flow(&state, now).await;
    let resp_aud = client
        .get(format!(
            "{base_url}/api/v1/auth/google/callback?code=mock-code&state={st5}"
        ))
        .header(HOST, host_str)
        .send()
        .await
        .unwrap();
    assert_eq!(resp_aud.status(), 401);

    let mut evil_iss_claims = valid_claims.clone();
    evil_iss_claims.iss = "https://malicious-idp.com".to_string();
    let evil_iss_token = create_signed_id_token(
        &evil_iss_claims,
        TEST_RSA_KEY_ID,
        Algorithm::RS256,
        &enc_key,
    );
    *mock_state.code_exchange_override.lock().unwrap() = Some(evil_iss_token);
    let (st6, _) = trigger_pending_flow(&state, now).await;
    let resp_iss = client
        .get(format!(
            "{base_url}/api/v1/auth/google/callback?code=mock-code&state={st6}"
        ))
        .header(HOST, host_str)
        .send()
        .await
        .unwrap();
    assert_eq!(resp_iss.status(), 401);

    let mut short_iss_claims = valid_claims.clone();
    short_iss_claims.iss = "accounts.google.com".to_string();
    let (st7, nonce7) = trigger_pending_flow(&state, now).await;
    short_iss_claims.nonce = Some(nonce7);
    let short_iss_token = create_signed_id_token(
        &short_iss_claims,
        TEST_RSA_KEY_ID,
        Algorithm::RS256,
        &enc_key,
    );
    *mock_state.code_exchange_override.lock().unwrap() = Some(short_iss_token);
    let resp_short_iss = client
        .get(format!(
            "{base_url}/api/v1/auth/google/callback?code=mock-code&state={st7}"
        ))
        .header(HOST, host_str)
        .send()
        .await
        .unwrap();
    assert_eq!(resp_short_iss.status(), 200);

    let (st8, _) = trigger_pending_flow(&state, now).await;
    let mut wrong_nonce_claims = valid_claims.clone();
    wrong_nonce_claims.nonce = Some("wrong-nonce-123".to_string());
    let wrong_nonce_token = create_signed_id_token(
        &wrong_nonce_claims,
        TEST_RSA_KEY_ID,
        Algorithm::RS256,
        &enc_key,
    );
    *mock_state.code_exchange_override.lock().unwrap() = Some(wrong_nonce_token);
    let resp_nonce = client
        .get(format!(
            "{base_url}/api/v1/auth/google/callback?code=mock-code&state={st8}"
        ))
        .header(HOST, host_str)
        .send()
        .await
        .unwrap();
    assert_eq!(resp_nonce.status(), 401);

    mock_state.include_rotated_key.store(true, Ordering::SeqCst);
    let (st9, nonce9) = trigger_pending_flow(&state, now).await;
    let mut rotated_claims = valid_claims.clone();
    rotated_claims.nonce = Some(nonce9);
    let rotated_token = create_signed_id_token(
        &rotated_claims,
        TEST_RSA_KEY_ID_ROTATED,
        Algorithm::RS256,
        &enc_key,
    );
    *mock_state.code_exchange_override.lock().unwrap() = Some(rotated_token);
    let resp_rotated = client
        .get(format!(
            "{base_url}/api/v1/auth/google/callback?code=mock-code&state={st9}"
        ))
        .header(HOST, host_str)
        .send()
        .await
        .unwrap();
    assert_eq!(resp_rotated.status(), 200);
}

#[tokio::test]
async fn test_handoff_and_zero_tokens_lifecycle() {
    let mock_state = new_mock_state("test-client-id");
    let (mock_url, _) = start_mock_google_server(mock_state.clone()).await;
    let (base_url, _) = setup_test_server(&mock_url, "test-client-id").await;
    let host_str = base_url.strip_prefix("http://").unwrap();
    let client = reqwest::Client::new();

    let start_resp = client
        .post(format!("{base_url}/api/v1/auth/google/start"))
        .header(HOST, host_str)
        .header(ORIGIN, &base_url)
        .send()
        .await
        .unwrap();
    let start_body: Value = start_resp.json().await.unwrap();
    let auth_url = start_body.get("auth_url").unwrap().as_str().unwrap();
    let nonce_val = auth_url
        .split("&nonce=")
        .nth(1)
        .unwrap()
        .split('&')
        .next()
        .unwrap();
    *mock_state.active_nonce.lock().unwrap() = Some(nonce_val.to_string());
    let state_str = start_body.get("state").unwrap().as_str().unwrap();
    let handoff_id = start_body.get("handoff_id").unwrap().as_str().unwrap();

    let cb_resp = client
        .get(format!(
            "{base_url}/api/v1/auth/google/callback?code=mock-code&state={state_str}"
        ))
        .header(HOST, host_str)
        .send()
        .await
        .unwrap();
    assert_eq!(cb_resp.status(), 200);
    let cb_headers = cb_resp.headers().clone();
    let cb_text = cb_resp.text().await.unwrap();

    assert!(!cb_text.contains("token"));
    assert!(!cb_text.contains("access_token"));
    assert!(!cb_text.contains("refresh_token"));
    assert!(!cb_text.contains("id_token"));
    assert!(!cb_text.contains("jwt"));
    assert!(cb_headers.get(SET_COOKIE).is_none());

    let handoff_resp = client
        .post(format!("{base_url}/api/v1/auth/google/handoff"))
        .header(HOST, host_str)
        .header(ORIGIN, &base_url)
        .json(&serde_json::json!({
            "handoff_id": handoff_id
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(handoff_resp.status(), 200);

    let cookie_header = handoff_resp
        .headers()
        .get(SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap();
    assert!(cookie_header.contains("session_id="));
    assert!(cookie_header.contains("HttpOnly"));
    assert!(cookie_header.contains("SameSite=Strict"));

    let handoff_body: Value = handoff_resp.json().await.unwrap();
    assert_eq!(handoff_body.get("status").unwrap(), "authenticated");
    assert!(handoff_body.get("user").is_some());
    assert!(handoff_body.get("token").is_none());
    assert!(handoff_body.get("access_token").is_none());
    assert!(handoff_body.get("jwt").is_none());

    let replay_resp = client
        .post(format!("{base_url}/api/v1/auth/google/handoff"))
        .header(HOST, host_str)
        .header(ORIGIN, &base_url)
        .json(&serde_json::json!({
            "handoff_id": handoff_id
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(replay_resp.status(), 401);

    let wrong_resp = client
        .post(format!("{base_url}/api/v1/auth/google/handoff"))
        .header(HOST, host_str)
        .header(ORIGIN, &base_url)
        .json(&serde_json::json!({
            "handoff_id": "nonexistent-handoff-id"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(wrong_resp.status(), 401);
}

#[tokio::test]
async fn test_sign_out_purges_secrets_and_calls_revocation() {
    let mock_state = new_mock_state("test-client-id");
    let rev_count = mock_state.revocation_count.clone();
    let (mock_url, _) = start_mock_google_server(mock_state.clone()).await;
    let (base_url, state) = setup_test_server(&mock_url, "test-client-id").await;
    let host_str = base_url.strip_prefix("http://").unwrap();
    let client = reqwest::Client::new();

    let start_resp = client
        .post(format!("{base_url}/api/v1/auth/google/start"))
        .header(HOST, host_str)
        .header(ORIGIN, &base_url)
        .send()
        .await
        .unwrap();
    let start_body: Value = start_resp.json().await.unwrap();
    let auth_url = start_body.get("auth_url").unwrap().as_str().unwrap();
    let nonce_val = auth_url
        .split("&nonce=")
        .nth(1)
        .unwrap()
        .split('&')
        .next()
        .unwrap();
    *mock_state.active_nonce.lock().unwrap() = Some(nonce_val.to_string());
    let state_str = start_body.get("state").unwrap().as_str().unwrap();
    let handoff_id = start_body.get("handoff_id").unwrap().as_str().unwrap();

    client
        .get(format!(
            "{base_url}/api/v1/auth/google/callback?code=mock-code&state={state_str}"
        ))
        .header(HOST, host_str)
        .send()
        .await
        .unwrap();

    let handoff_resp = client
        .post(format!("{base_url}/api/v1/auth/google/handoff"))
        .header(HOST, host_str)
        .header(ORIGIN, &base_url)
        .json(&serde_json::json!({ "handoff_id": handoff_id }))
        .send()
        .await
        .unwrap();
    let cookie_header = handoff_resp
        .headers()
        .get(SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap();
    let session_cookie = cookie_header.split(';').next().unwrap();

    let session_resp = client
        .get(format!("{base_url}/api/v1/auth/session"))
        .header(COOKIE, session_cookie)
        .send()
        .await
        .unwrap();
    let session_body: Value = session_resp.json().await.unwrap();
    assert_eq!(session_body.get("authenticated").unwrap(), true);

    let signout_resp = client
        .post(format!("{base_url}/api/v1/auth/sign-out"))
        .header(HOST, host_str)
        .header(ORIGIN, &base_url)
        .header(COOKIE, session_cookie)
        .send()
        .await
        .unwrap();
    assert_eq!(signout_resp.status(), 200);

    assert_eq!(rev_count.load(Ordering::SeqCst), 1);

    let creds = &state.credentials;
    assert_eq!(
        creds
            .get_secret("account:google-sub-12345:jwt_secret")
            .unwrap(),
        None
    );
    assert_eq!(
        creds
            .get_secret("account:google-sub-12345:google_tokens")
            .unwrap(),
        None
    );

    let post_session_resp = client
        .get(format!("{base_url}/api/v1/auth/session"))
        .header(COOKIE, session_cookie)
        .send()
        .await
        .unwrap();
    let post_session_body: Value = post_session_resp.json().await.unwrap();
    assert_eq!(post_session_body.get("authenticated").unwrap(), false);
}

#[tokio::test]
async fn test_legacy_data_claim_flow_and_tenant_isolation() {
    let mock_state = new_mock_state("test-client-id");
    let (mock_url, _) = start_mock_google_server(mock_state.clone()).await;
    let (base_url, _) = setup_test_server(&mock_url, "test-client-id").await;
    let host_str = base_url.strip_prefix("http://").unwrap();
    let client = reqwest::Client::new();

    let start_resp = client
        .post(format!("{base_url}/api/v1/auth/google/start"))
        .header(HOST, host_str)
        .header(ORIGIN, &base_url)
        .send()
        .await
        .unwrap();
    let start_body: Value = start_resp.json().await.unwrap();
    let auth_url = start_body.get("auth_url").unwrap().as_str().unwrap();
    let nonce_val = auth_url
        .split("&nonce=")
        .nth(1)
        .unwrap()
        .split('&')
        .next()
        .unwrap();
    *mock_state.active_nonce.lock().unwrap() = Some(nonce_val.to_string());
    let state_str = start_body.get("state").unwrap().as_str().unwrap();
    let handoff_id = start_body.get("handoff_id").unwrap().as_str().unwrap();

    client
        .get(format!(
            "{base_url}/api/v1/auth/google/callback?code=mock-code&state={state_str}"
        ))
        .header(HOST, host_str)
        .send()
        .await
        .unwrap();

    let handoff_resp = client
        .post(format!("{base_url}/api/v1/auth/google/handoff"))
        .header(HOST, host_str)
        .header(ORIGIN, &base_url)
        .json(&serde_json::json!({ "handoff_id": handoff_id }))
        .send()
        .await
        .unwrap();
    let cookie_header = handoff_resp
        .headers()
        .get(SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap();
    let session_cookie = cookie_header.split(';').next().unwrap();

    let sess_check1: Value = client
        .get(format!("{base_url}/api/v1/auth/session"))
        .header(COOKIE, session_cookie)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(sess_check1.get("has_unclaimed_legacy_data").unwrap(), true);

    let defer_resp = client
        .post(format!("{base_url}/api/v1/auth/legacy-data/claim"))
        .header(HOST, host_str)
        .header(ORIGIN, &base_url)
        .header(COOKIE, session_cookie)
        .json(&serde_json::json!({ "claim": false }))
        .send()
        .await
        .unwrap();
    assert_eq!(defer_resp.status(), 200);
    let defer_body: Value = defer_resp.json().await.unwrap();
    assert_eq!(defer_body.get("deferred").unwrap(), true);

    let sess_check2: Value = client
        .get(format!("{base_url}/api/v1/auth/session"))
        .header(COOKIE, session_cookie)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(sess_check2.get("has_unclaimed_legacy_data").unwrap(), true);

    let claim_resp = client
        .post(format!("{base_url}/api/v1/auth/legacy-data/claim"))
        .header(HOST, host_str)
        .header(ORIGIN, &base_url)
        .header(COOKIE, session_cookie)
        .json(&serde_json::json!({ "claim": true }))
        .send()
        .await
        .unwrap();
    assert_eq!(claim_resp.status(), 200);
    let claim_body: Value = claim_resp.json().await.unwrap();
    assert_eq!(claim_body.get("claimed").unwrap(), true);
    assert_eq!(
        claim_body.get("tenant_id").unwrap(),
        "00000000-0000-0000-0000-000000000001"
    );

    let claim_second_attempt = client
        .post(format!("{base_url}/api/v1/auth/legacy-data/claim"))
        .header(HOST, host_str)
        .header(ORIGIN, &base_url)
        .header(COOKIE, session_cookie)
        .json(&serde_json::json!({ "claim": true }))
        .send()
        .await
        .unwrap();
    assert_eq!(claim_second_attempt.status(), 409);

    let sess_check3: Value = client
        .get(format!("{base_url}/api/v1/auth/session"))
        .header(COOKIE, session_cookie)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(sess_check3.get("has_unclaimed_legacy_data").unwrap(), false);
}

#[tokio::test]
async fn test_release_mode_rejects_local_mode_and_local_session_with_404() {
    let mock_state = new_mock_state("test-client-id");
    let (mock_url, _) = start_mock_google_server(mock_state).await;

    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("alba_release_test.db");
    let store = Arc::new(SqliteCheckpointStore::new(&db_path).unwrap());
    let creds = Arc::new(MemoryCredentialStore::new());

    let state = AppState::new(store, tmp.path().to_path_buf())
        .with_credentials(creds)
        .with_allow_local_auth(false)
        .with_oauth_config(
            Some("test-client-id".to_string()),
            format!("{mock_url}/o/oauth2/v2/auth"),
            format!("{mock_url}/oauth2/v2/token"),
            format!("{mock_url}/oauth2/v3/certs"),
            format!("{mock_url}/oauth2/v2/revoke"),
        );

    let app = create_app(state);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let base_url = format!("http://{}", addr);

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let client = reqwest::Client::new();

    let resp_mode = client
        .get(format!("{base_url}/api/v1/auth/local-mode"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp_mode.status(), 404);

    let resp_sess = client
        .post(format!("{base_url}/api/v1/auth/local-session"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp_sess.status(), 404);
}

#[tokio::test]
async fn test_sign_out_keychain_delete_error() {
    let mock_state = new_mock_state("test-client-id");
    let (mock_url, _) = start_mock_google_server(mock_state.clone()).await;
    let creds = Arc::new(FaultyCredentialStore::new());
    let (base_url, state) =
        setup_test_server_with_creds(&mock_url, "test-client-id", creds.clone()).await;
    let host_str = base_url.strip_prefix("http://").unwrap();
    let client = reqwest::Client::new();

    let session_cookie = login_and_get_cookie(&client, &base_url, &mock_state).await;

    creds.fail_delete_for("jwt_secret");

    let signout_resp = client
        .post(format!("{base_url}/api/v1/auth/sign-out"))
        .header(HOST, host_str)
        .header(ORIGIN, &base_url)
        .header(COOKIE, &session_cookie)
        .send()
        .await
        .unwrap();

    assert_eq!(signout_resp.status(), 207);
    let cookie_hdrs: Vec<_> = signout_resp.headers().get_all(SET_COOKIE).iter().collect();
    assert!(cookie_hdrs
        .iter()
        .any(|h| h.to_str().unwrap().contains("Max-Age=0")));

    let body: Value = signout_resp.json().await.unwrap();
    assert_eq!(body["status"], "signed_out_partial");
    let failed_steps = body["failed_steps"].as_array().unwrap();
    assert!(failed_steps
        .iter()
        .any(|s| s.as_str().unwrap().contains("keychain_delete_jwt_secret")));

    let store_conn = state.store.connection();
    let conn = store_conn.lock().unwrap();
    let mut stmt = conn
        .prepare("SELECT tenant_id, workspace_id, action FROM audit_logs WHERE action = 'auth_sign_out_error'")
        .unwrap();
    let rows: Vec<(String, String, String)> = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    assert!(!rows.is_empty());
    assert_ne!(
        rows[0].0,
        nexus_albaai::server::auth::DEFAULT_LOCAL_TENANT_ID
    );
    assert_ne!(
        rows[0].1,
        nexus_albaai::server::auth::DEFAULT_LOCAL_WORKSPACE_ID
    );
}

#[tokio::test]
async fn test_sign_out_token_read_error() {
    let mock_state = new_mock_state("test-client-id");
    let (mock_url, _) = start_mock_google_server(mock_state.clone()).await;
    let creds = Arc::new(FaultyCredentialStore::new());
    let (base_url, state) =
        setup_test_server_with_creds(&mock_url, "test-client-id", creds.clone()).await;
    let host_str = base_url.strip_prefix("http://").unwrap();
    let client = reqwest::Client::new();

    let session_cookie = login_and_get_cookie(&client, &base_url, &mock_state).await;

    creds.fail_get_for("google_tokens");

    let signout_resp = client
        .post(format!("{base_url}/api/v1/auth/sign-out"))
        .header(HOST, host_str)
        .header(ORIGIN, &base_url)
        .header(COOKIE, &session_cookie)
        .send()
        .await
        .unwrap();

    assert_eq!(signout_resp.status(), 207);
    let body: Value = signout_resp.json().await.unwrap();
    assert_eq!(body["status"], "signed_out_partial");
    let failed_steps = body["failed_steps"].as_array().unwrap();
    assert!(failed_steps
        .iter()
        .any(|s| s.as_str().unwrap().contains("keychain_get_google_tokens")));

    let store_conn = state.store.connection();
    let conn = store_conn.lock().unwrap();
    let mut stmt = conn
        .prepare("SELECT tenant_id, workspace_id, action FROM audit_logs WHERE action = 'auth_sign_out_error'")
        .unwrap();
    let rows: Vec<(String, String, String)> = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    assert!(!rows.is_empty());
    assert_ne!(
        rows[0].0,
        nexus_albaai::server::auth::DEFAULT_LOCAL_TENANT_ID
    );
}

#[tokio::test]
async fn test_sign_out_corrupted_token_json() {
    let mock_state = new_mock_state("test-client-id");
    let (mock_url, _) = start_mock_google_server(mock_state.clone()).await;
    let (base_url, state) = setup_test_server(&mock_url, "test-client-id").await;
    let host_str = base_url.strip_prefix("http://").unwrap();
    let client = reqwest::Client::new();

    let session_cookie = login_and_get_cookie(&client, &base_url, &mock_state).await;

    state
        .credentials
        .set_secret("account:google-sub-12345:google_tokens", "not-valid-json")
        .unwrap();

    let signout_resp = client
        .post(format!("{base_url}/api/v1/auth/sign-out"))
        .header(HOST, host_str)
        .header(ORIGIN, &base_url)
        .header(COOKIE, &session_cookie)
        .send()
        .await
        .unwrap();

    assert_eq!(signout_resp.status(), 207);
    let body: Value = signout_resp.json().await.unwrap();
    assert_eq!(body["status"], "signed_out_partial");
    let failed_steps = body["failed_steps"].as_array().unwrap();
    assert!(failed_steps
        .iter()
        .any(|s| s.as_str().unwrap().contains("parse_google_tokens_json")));

    let store_conn = state.store.connection();
    let conn = store_conn.lock().unwrap();
    let mut stmt = conn
        .prepare("SELECT tenant_id, workspace_id, action FROM audit_logs WHERE action = 'auth_sign_out_error'")
        .unwrap();
    let rows: Vec<(String, String, String)> = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    assert!(!rows.is_empty());
    assert_ne!(
        rows[0].0,
        nexus_albaai::server::auth::DEFAULT_LOCAL_TENANT_ID
    );
}

#[tokio::test]
async fn test_sign_out_google_revocation_http_error() {
    let mock_state = new_mock_state("test-client-id");
    mock_state.revoke_status.store(500, Ordering::SeqCst);
    let (mock_url, _) = start_mock_google_server(mock_state.clone()).await;
    let (base_url, state) = setup_test_server(&mock_url, "test-client-id").await;
    let host_str = base_url.strip_prefix("http://").unwrap();
    let client = reqwest::Client::new();

    let session_cookie = login_and_get_cookie(&client, &base_url, &mock_state).await;

    let signout_resp = client
        .post(format!("{base_url}/api/v1/auth/sign-out"))
        .header(HOST, host_str)
        .header(ORIGIN, &base_url)
        .header(COOKIE, &session_cookie)
        .send()
        .await
        .unwrap();

    assert_eq!(signout_resp.status(), 207);
    let body: Value = signout_resp.json().await.unwrap();
    assert_eq!(body["status"], "signed_out_partial");
    let failed_steps = body["failed_steps"].as_array().unwrap();
    assert!(failed_steps
        .iter()
        .any(|s| s.as_str().unwrap().contains("google_revocation_http_500")));

    let store_conn = state.store.connection();
    let conn = store_conn.lock().unwrap();
    let mut stmt = conn
        .prepare("SELECT tenant_id, workspace_id, action FROM audit_logs WHERE action = 'auth_token_revocation_failed'")
        .unwrap();
    let rows: Vec<(String, String, String)> = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    assert!(!rows.is_empty());
    assert_ne!(
        rows[0].0,
        nexus_albaai::server::auth::DEFAULT_LOCAL_TENANT_ID
    );
}

#[tokio::test]
async fn test_sign_out_google_revocation_timeout() {
    let mock_state = new_mock_state("test-client-id");
    mock_state.revoke_delay_ms.store(5500, Ordering::SeqCst);
    let (mock_url, _) = start_mock_google_server(mock_state.clone()).await;
    let (base_url, state) = setup_test_server(&mock_url, "test-client-id").await;
    let host_str = base_url.strip_prefix("http://").unwrap();
    let client = reqwest::Client::new();

    let session_cookie = login_and_get_cookie(&client, &base_url, &mock_state).await;

    let signout_resp = client
        .post(format!("{base_url}/api/v1/auth/sign-out"))
        .header(HOST, host_str)
        .header(ORIGIN, &base_url)
        .header(COOKIE, &session_cookie)
        .send()
        .await
        .unwrap();

    assert_eq!(signout_resp.status(), 207);
    let body: Value = signout_resp.json().await.unwrap();
    assert_eq!(body["status"], "signed_out_partial");
    let failed_steps = body["failed_steps"].as_array().unwrap();
    assert!(failed_steps
        .iter()
        .any(|s| s.as_str().unwrap().contains("google_revocation_timeout")));

    let store_conn = state.store.connection();
    let conn = store_conn.lock().unwrap();
    let mut stmt = conn
        .prepare("SELECT tenant_id, workspace_id, action FROM audit_logs WHERE action = 'auth_token_revocation_failed'")
        .unwrap();
    let rows: Vec<(String, String, String)> = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    assert!(!rows.is_empty());
    assert_ne!(
        rows[0].0,
        nexus_albaai::server::auth::DEFAULT_LOCAL_TENANT_ID
    );
}

#[tokio::test]
async fn test_sign_out_audit_log_error() {
    let mock_state = new_mock_state("test-client-id");
    let (mock_url, _) = start_mock_google_server(mock_state.clone()).await;
    let (base_url, state) = setup_test_server(&mock_url, "test-client-id").await;
    let host_str = base_url.strip_prefix("http://").unwrap();
    let client = reqwest::Client::new();

    let session_cookie = login_and_get_cookie(&client, &base_url, &mock_state).await;

    state
        .credentials
        .set_secret("account:google-sub-12345:google_tokens", "corrupted")
        .unwrap();
    let store_conn = state.store.connection();
    store_conn
        .lock()
        .unwrap()
        .execute("DROP TABLE audit_logs;", [])
        .unwrap();

    let signout_resp = client
        .post(format!("{base_url}/api/v1/auth/sign-out"))
        .header(HOST, host_str)
        .header(ORIGIN, &base_url)
        .header(COOKIE, &session_cookie)
        .send()
        .await
        .unwrap();

    assert_eq!(signout_resp.status(), 207);
    let body: Value = signout_resp.json().await.unwrap();
    assert_eq!(body["status"], "signed_out_partial");
    let failed_steps = body["failed_steps"].as_array().unwrap();
    assert!(failed_steps
        .iter()
        .any(|s| s.as_str().unwrap().contains("record_audit_log")));
}
