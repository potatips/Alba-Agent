use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{broadcast, watch, Mutex};

use crate::checkpoints::SqliteCheckpointStore;
use crate::credentials::{default_credential_store, generate_secure_secret, CredentialStore};
use crate::events::RuntimeEvent;
use crate::server::error::ApiError;
use crate::server::workflow_runner::ServerModelClient;

#[derive(Clone, Debug)]
pub struct PendingOauthFlow {
    pub state: String,
    pub nonce: String,
    pub code_verifier: String,
    pub handoff_id: String,
    pub expires_at: i64,
}

#[derive(Clone, Debug)]
pub struct ReadyHandoffSession {
    pub handoff_id: String,
    pub sub: String,
    pub session_id: String,
    pub expires_at: i64,
}

#[derive(Clone, Debug)]
pub struct ActiveSessionInfo {
    pub session_id: String,
    pub sub: String,
    pub email: String,
    pub display_name: String,
    pub picture: Option<String>,
    pub tenant_id: String,
    pub workspace_id: String,
    pub expires_at: i64,
}

#[derive(Clone)]
pub struct AppState {
    pub store: Arc<SqliteCheckpointStore>,
    pub credentials: Arc<dyn CredentialStore>,
    pub events_tx: broadcast::Sender<RuntimeEvent>,
    pub active_cancellations: Arc<Mutex<HashMap<String, watch::Sender<bool>>>>,
    pub workspace_root: PathBuf,
    pub model: Option<ServerModelClient>,
    pub static_dir: Option<PathBuf>,
    pub oauth_client_id: Option<String>,
    pub google_auth_url: String,
    pub google_token_url: String,
    pub google_jwks_url: String,
    pub google_revoke_url: String,
    pub allow_local_auth: bool,
    pub pending_oauth: Arc<Mutex<HashMap<String, PendingOauthFlow>>>,
    pub ready_handoffs: Arc<Mutex<HashMap<String, ReadyHandoffSession>>>,
    pub active_sessions: Arc<Mutex<HashMap<String, ActiveSessionInfo>>>,
}

impl AppState {
    pub fn new(store: Arc<SqliteCheckpointStore>, workspace_root: PathBuf) -> Self {
        let (events_tx, _) = broadcast::channel(1024);
        let allow_local_auth = cfg!(debug_assertions)
            || cfg!(test)
            || std::env::var("ALBA_ALLOW_LOCAL_AUTH")
                .map(|v| v == "1" || v == "true")
                .unwrap_or(false);
        Self {
            store,
            credentials: Arc::from(default_credential_store()),
            events_tx,
            active_cancellations: Arc::new(Mutex::new(HashMap::new())),
            workspace_root,
            model: None,
            static_dir: None,
            oauth_client_id: std::env::var("GOOGLE_CLIENT_ID").ok(),
            google_auth_url: "https://accounts.google.com/o/oauth2/v2/auth".to_string(),
            google_token_url: "https://oauth2.googleapis.com/token".to_string(),
            google_jwks_url: "https://www.googleapis.com/oauth2/v3/certs".to_string(),
            google_revoke_url: "https://oauth2.googleapis.com/revoke".to_string(),
            allow_local_auth,
            pending_oauth: Arc::new(Mutex::new(HashMap::new())),
            ready_handoffs: Arc::new(Mutex::new(HashMap::new())),
            active_sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn with_allow_local_auth(mut self, allow: bool) -> Self {
        self.allow_local_auth = allow;
        self
    }

    pub fn with_oauth_config(
        mut self,
        client_id: Option<String>,
        auth_url: String,
        token_url: String,
        jwks_url: String,
        revoke_url: String,
    ) -> Self {
        self.oauth_client_id = client_id;
        self.google_auth_url = auth_url;
        self.google_token_url = token_url;
        self.google_jwks_url = jwks_url;
        self.google_revoke_url = revoke_url;
        self
    }

    pub fn with_credentials(mut self, credentials: Arc<dyn CredentialStore>) -> Self {
        self.credentials = credentials;
        self
    }

    pub fn with_model(mut self, model: ServerModelClient) -> Self {
        self.model = Some(model);
        self
    }

    pub fn with_static_dir(mut self, static_dir: PathBuf) -> Self {
        self.static_dir = Some(static_dir);
        self
    }

    pub fn get_or_create_jwt_secret(&self, profile_id: &str) -> Result<String, ApiError> {
        let key = format!("profile:{profile_id}:jwt_secret");
        self.credentials
            .get_or_set_secret(&key, &generate_secure_secret)
            .map_err(|e| ApiError::internal(e.to_string()))
    }
}
