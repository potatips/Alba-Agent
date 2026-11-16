use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{broadcast, watch, Mutex};

use crate::checkpoints::SqliteCheckpointStore;
use crate::credentials::{default_credential_store, generate_secure_secret, CredentialStore};
use crate::events::RuntimeEvent;
use crate::server::error::ApiError;
use crate::server::workflow_runner::ServerModelClient;

#[derive(Clone)]
pub struct AppState {
    pub store: Arc<SqliteCheckpointStore>,
    pub credentials: Arc<dyn CredentialStore>,
    pub events_tx: broadcast::Sender<RuntimeEvent>,
    pub active_cancellations: Arc<Mutex<HashMap<String, watch::Sender<bool>>>>,
    pub workspace_root: PathBuf,
    pub model: Option<ServerModelClient>,
    pub static_dir: Option<PathBuf>,
}

impl AppState {
    pub fn new(store: Arc<SqliteCheckpointStore>, workspace_root: PathBuf) -> Self {
        let (events_tx, _) = broadcast::channel(1024);
        Self {
            store,
            credentials: Arc::from(default_credential_store()),
            events_tx,
            active_cancellations: Arc::new(Mutex::new(HashMap::new())),
            workspace_root,
            model: None,
            static_dir: None,
        }
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
