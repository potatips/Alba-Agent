use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{broadcast, watch, Mutex};

use crate::checkpoints::SqliteCheckpointStore;
use crate::events::RuntimeEvent;
use crate::server::auth::DEFAULT_JWT_SECRET;

#[derive(Clone)]
pub struct AppState {
    pub store: Arc<SqliteCheckpointStore>,
    pub events_tx: broadcast::Sender<RuntimeEvent>,
    pub active_cancellations: Arc<Mutex<HashMap<String, watch::Sender<bool>>>>,
    pub jwt_secret: String,
    pub workspace_root: PathBuf,
}

impl AppState {
    pub fn new(store: Arc<SqliteCheckpointStore>, workspace_root: PathBuf) -> Self {
        let (events_tx, _) = broadcast::channel(1024);
        Self {
            store,
            events_tx,
            active_cancellations: Arc::new(Mutex::new(HashMap::new())),
            jwt_secret: DEFAULT_JWT_SECRET.to_string(),
            workspace_root,
        }
    }
}
