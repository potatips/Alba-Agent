use std::fs::OpenOptions;
#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::Path;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use uuid::Uuid;

use crate::checkpoints::store::{CheckpointError, CheckpointStore};
use crate::checkpoints::types::{
    ArtifactRecord, AuditLogEntry, CapsuleRecord, ExecutionPolicyRecord, ForkRunRequest,
    KnowledgeCapsuleRecord, RunCheckpoint, RunEvidenceRecord, RunRecord, TrajectoryBranch,
    WorkflowStepCheckpoint, WorkflowSummary,
};
use crate::domain::{
    ActionIntentKind, ApprovalDecision, ApprovalRequest, ApprovalStatus, ChatMessage,
    ExecutionScope, MessageRole, RunStatus, ToolCall,
};
use crate::events::types::{RuntimeEvent, RuntimeEventType};
use crate::profiles::Profile;
use crate::schema::ApprovalRequestSpec;

#[derive(serde::Serialize, serde::Deserialize)]
struct SerializedCheckpointState {
    history: Vec<ChatMessage>,
    active_tool_call: Option<ToolCall>,
    #[serde(default)]
    pending_tool_calls: Vec<ToolCall>,
    pending_approval_id: Option<Uuid>,
    #[serde(default)]
    author_id: Option<String>,
    #[serde(default)]
    model_override: Option<String>,
}

fn serialize_checkpoint_state(checkpoint: &RunCheckpoint) -> Result<String, CheckpointError> {
    let state_val = SerializedCheckpointState {
        history: checkpoint.history.clone(),
        active_tool_call: checkpoint.active_tool_call.clone(),
        pending_tool_calls: checkpoint.pending_tool_calls.clone(),
        pending_approval_id: checkpoint.pending_approval_id,
        author_id: checkpoint.author_id.clone(),
        model_override: checkpoint.model_override.clone(),
    };

    serde_json::to_string(&state_val)
        .map_err(|e| CheckpointError::SerializationError(e.to_string()))
}

fn build_checkpoint_from_row(
    scope: ExecutionScope,
    id_str: String,
    sequence: u32,
    status_str: String,
    state_json: String,
    created_at_str: String,
) -> Result<RunCheckpoint, CheckpointError> {
    let id =
        Uuid::parse_str(&id_str).map_err(|e| CheckpointError::SerializationError(e.to_string()))?;
    let status = RunStatus::from_str(&status_str)
        .map_err(|e| CheckpointError::SerializationError(e.to_string()))?;
    let created_at = DateTime::parse_from_rfc3339(&created_at_str)
        .map_err(|e| CheckpointError::SerializationError(e.to_string()))?
        .with_timezone(&Utc);
    let state_val: SerializedCheckpointState = serde_json::from_str(&state_json)
        .map_err(|e| CheckpointError::SerializationError(e.to_string()))?;

    Ok(RunCheckpoint {
        id,
        scope,
        sequence,
        status,
        history: state_val.history,
        active_tool_call: state_val.active_tool_call,
        pending_tool_calls: state_val.pending_tool_calls,
        pending_approval_id: state_val.pending_approval_id,
        author_id: state_val.author_id,
        model_override: state_val.model_override,
        created_at,
    })
}

#[allow(clippy::too_many_arguments)]
fn build_approval_from_row(
    scope: ExecutionScope,
    id_str: String,
    tool_name: String,
    action_description: String,
    intent_str: String,
    status_str: String,
    author_id: Option<String>,
    created_str: String,
    decided_str: Option<String>,
    decided_by: Option<String>,
    consumed_str: Option<String>,
) -> Result<ApprovalRequest, CheckpointError> {
    let id =
        Uuid::parse_str(&id_str).map_err(|e| CheckpointError::SerializationError(e.to_string()))?;
    let intent = ActionIntentKind::from_str(&intent_str).unwrap();
    let status = ApprovalStatus::from_str(&status_str)
        .map_err(|e| CheckpointError::InvalidState(e.to_string()))?;
    let created_at = DateTime::parse_from_rfc3339(&created_str)
        .map_err(|e| CheckpointError::SerializationError(e.to_string()))?
        .with_timezone(&Utc);
    let decided_at = match decided_str {
        Some(s) => Some(
            DateTime::parse_from_rfc3339(&s)
                .map_err(|e| CheckpointError::SerializationError(e.to_string()))?
                .with_timezone(&Utc),
        ),
        None => None,
    };
    let consumed_at = match consumed_str {
        Some(s) => Some(
            DateTime::parse_from_rfc3339(&s)
                .map_err(|e| CheckpointError::SerializationError(e.to_string()))?
                .with_timezone(&Utc),
        ),
        None => None,
    };

    Ok(ApprovalRequest {
        id,
        scope,
        tool_name,
        action_description,
        intent,
        status,
        author_id,
        created_at,
        decided_at,
        decided_by,
        consumed_at,
    })
}

#[derive(Clone)]
pub struct SqliteCheckpointStore {
    conn: Arc<Mutex<Connection>>,
}

impl SqliteCheckpointStore {
    pub fn connection(&self) -> Arc<Mutex<Connection>> {
        self.conn.clone()
    }

    fn ensure_private_db_file(path: &Path) -> Result<(), CheckpointError> {
        if path.as_os_str() == ":memory:" {
            return Ok(());
        }
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
            }
        }
        let mut opts = OpenOptions::new();
        opts.read(true).write(true).create(true);
        #[cfg(unix)]
        opts.mode(0o600);
        opts.open(path)
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        #[cfg(unix)]
        {
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
                .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        }
        Ok(())
    }

    fn enforce_sidecar_permissions(path: &Path) -> Result<(), CheckpointError> {
        #[cfg(unix)]
        {
            if path.as_os_str() != ":memory:" {
                let base = path.to_string_lossy();
                for suffix in ["-wal", "-shm", "-journal"] {
                    let sidecar = Path::new(&format!("{}{}", base, suffix)).to_path_buf();
                    if sidecar.exists() {
                        std::fs::set_permissions(&sidecar, std::fs::Permissions::from_mode(0o600))
                            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
                    }
                }
            }
        }
        #[cfg(not(unix))]
        let _ = path;
        Ok(())
    }

    pub fn new(path: impl AsRef<Path>) -> Result<Self, CheckpointError> {
        let path_ref = path.as_ref();
        Self::ensure_private_db_file(path_ref)?;
        let conn = Connection::open(path_ref)
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        conn.busy_timeout(Duration::from_secs(10))
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        conn.execute_batch("PRAGMA journal_mode = WAL;")
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        Self::enforce_sidecar_permissions(path_ref)?;
        let store = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        store.run_migrations()?;
        Self::enforce_sidecar_permissions(path_ref)?;
        Ok(store)
    }

    pub fn in_memory() -> Result<Self, CheckpointError> {
        let conn = Connection::open_in_memory()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        conn.busy_timeout(Duration::from_secs(10))
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        let store = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        store.run_migrations()?;
        Ok(store)
    }

    fn run_migrations(&self) -> Result<(), CheckpointError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS schema_migrations (
                version INTEGER PRIMARY KEY,
                applied_at TEXT NOT NULL
            )",
            [],
        )
        .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        let current_version: i64 = conn
            .query_row(
                "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        if current_version < 1 {
            conn.execute_batch(
                "
                CREATE TABLE checkpoints (
                    id TEXT PRIMARY KEY,
                    run_id TEXT NOT NULL,
                    tenant_id TEXT NOT NULL,
                    workspace_id TEXT NOT NULL,
                    sequence INTEGER NOT NULL,
                    status TEXT NOT NULL,
                    state_json TEXT NOT NULL,
                    created_at TEXT NOT NULL,
                    UNIQUE(run_id, sequence)
                );
                CREATE INDEX idx_checkpoints_scope ON checkpoints(tenant_id, workspace_id, run_id);
                CREATE INDEX idx_checkpoints_run_seq ON checkpoints(run_id, sequence DESC);

                CREATE TABLE approvals (
                    id TEXT PRIMARY KEY,
                    run_id TEXT NOT NULL,
                    tenant_id TEXT NOT NULL,
                    workspace_id TEXT NOT NULL,
                    tool_name TEXT NOT NULL,
                    action_description TEXT NOT NULL,
                    intent_kind TEXT NOT NULL,
                    status TEXT NOT NULL,
                    created_at TEXT NOT NULL,
                    decided_at TEXT,
                    consumed_at TEXT
                );
                CREATE INDEX idx_approvals_scope ON approvals(tenant_id, workspace_id, run_id);
                CREATE INDEX idx_approvals_status ON approvals(run_id, status);

                CREATE TABLE audit_logs (
                    id TEXT PRIMARY KEY,
                    tenant_id TEXT NOT NULL,
                    workspace_id TEXT NOT NULL,
                    run_id TEXT NOT NULL,
                    action TEXT NOT NULL,
                    target_id TEXT,
                    details_json TEXT NOT NULL,
                    created_at TEXT NOT NULL
                );
                CREATE INDEX idx_audit_logs_scope ON audit_logs(tenant_id, workspace_id, run_id);

                CREATE TABLE cancellations (
                    run_id TEXT PRIMARY KEY,
                    tenant_id TEXT NOT NULL,
                    workspace_id TEXT NOT NULL,
                    requested_at TEXT NOT NULL
                );

                INSERT INTO schema_migrations (version, applied_at)
                VALUES (1, datetime('now'));
                ",
            )
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        }

        if current_version < 2 {
            conn.execute_batch(
                "
                ALTER TABLE approvals ADD COLUMN decided_by TEXT;
                ALTER TABLE audit_logs ADD COLUMN actor_id TEXT;

                INSERT INTO schema_migrations (version, applied_at)
                VALUES (2, datetime('now'));
                ",
            )
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        }

        if current_version < 3 {
            conn.execute_batch(
                "
                ALTER TABLE approvals ADD COLUMN author_id TEXT;
                ALTER TABLE checkpoints ADD COLUMN author_id TEXT;

                CREATE TABLE IF NOT EXISTS trajectory_branches (
                    branch_id TEXT PRIMARY KEY,
                    tenant_id TEXT NOT NULL,
                    workspace_id TEXT NOT NULL,
                    parent_run_id TEXT NOT NULL,
                    forked_run_id TEXT NOT NULL,
                    fork_sequence INTEGER NOT NULL,
                    branch_label TEXT NOT NULL,
                    model_override TEXT,
                    prompt_override TEXT,
                    created_at TEXT NOT NULL
                );
                CREATE INDEX IF NOT EXISTS idx_branches_parent ON trajectory_branches(tenant_id, workspace_id, parent_run_id);
                CREATE INDEX IF NOT EXISTS idx_branches_forked ON trajectory_branches(tenant_id, workspace_id, forked_run_id);

                CREATE TABLE IF NOT EXISTS workflow_checkpoints (
                    id TEXT PRIMARY KEY,
                    run_id TEXT NOT NULL,
                    tenant_id TEXT NOT NULL,
                    workspace_id TEXT NOT NULL,
                    sequence INTEGER NOT NULL,
                    node_id TEXT NOT NULL,
                    status TEXT NOT NULL,
                    state_json TEXT NOT NULL,
                    pending_approval_id TEXT,
                    created_at TEXT NOT NULL,
                    UNIQUE(run_id, sequence)
                );
                CREATE INDEX IF NOT EXISTS idx_wf_scope ON workflow_checkpoints(tenant_id, workspace_id, run_id, sequence DESC);

                INSERT INTO schema_migrations (version, applied_at)
                VALUES (3, datetime('now'));
                ",
            )
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        }

        if current_version < 4 {
            conn.execute_batch(
                "
                CREATE TABLE IF NOT EXISTS tool_journal (
                    run_id TEXT NOT NULL,
                    tenant_id TEXT NOT NULL,
                    workspace_id TEXT NOT NULL,
                    node_id TEXT NOT NULL,
                    tool_call_id TEXT NOT NULL,
                    tool_name TEXT NOT NULL,
                    arguments_json TEXT NOT NULL,
                    status TEXT NOT NULL,
                    output_text TEXT,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL,
                    PRIMARY KEY(run_id, node_id, tool_call_id)
                );
                CREATE INDEX IF NOT EXISTS idx_tool_journal_scope ON tool_journal(tenant_id, workspace_id, run_id);

                INSERT INTO schema_migrations (version, applied_at)
                VALUES (4, datetime('now'));
                ",
            )
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        }

        if current_version < 5 {
            conn.execute_batch(
                "
                CREATE TABLE IF NOT EXISTS workflow_specs (
                    id TEXT PRIMARY KEY,
                    name TEXT NOT NULL,
                    description TEXT,
                    spec_json TEXT NOT NULL,
                    version INTEGER NOT NULL DEFAULT 1,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                );

                CREATE TABLE IF NOT EXISTS runs (
                    id TEXT PRIMARY KEY,
                    workflow_id TEXT NOT NULL,
                    status TEXT NOT NULL,
                    input_data TEXT NOT NULL,
                    output_data TEXT,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL,
                    completed_at TEXT
                );

                CREATE TABLE IF NOT EXISTS artifacts (
                    id TEXT PRIMARY KEY,
                    run_id TEXT NOT NULL,
                    filename TEXT NOT NULL,
                    file_path TEXT NOT NULL,
                    file_size INTEGER NOT NULL,
                    mime_type TEXT NOT NULL,
                    sha256 TEXT NOT NULL,
                    verified INTEGER NOT NULL DEFAULT 0,
                    created_at TEXT NOT NULL,
                    FOREIGN KEY(run_id) REFERENCES runs(id) ON DELETE CASCADE
                );

                CREATE TABLE IF NOT EXISTS capsules (
                    id TEXT PRIMARY KEY,
                    title TEXT NOT NULL,
                    trigger_pattern TEXT NOT NULL,
                    advice TEXT NOT NULL,
                    scope TEXT NOT NULL,
                    sensitivity TEXT NOT NULL,
                    created_at TEXT NOT NULL
                );

                CREATE TABLE IF NOT EXISTS execution_policy (
                    id INTEGER PRIMARY KEY CHECK (id = 1),
                    default_mode TEXT NOT NULL,
                    allowed_domains_json TEXT NOT NULL,
                    denied_domains_json TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                );

                INSERT OR IGNORE INTO execution_policy (id, default_mode, allowed_domains_json, denied_domains_json, updated_at)
                VALUES (1, 'ask', '[\"api.anthropic.com\",\"generativelanguage.googleapis.com\"]', '[\"169.254.169.254\"]', datetime('now'));

                CREATE INDEX IF NOT EXISTS idx_runs_workflow ON runs(workflow_id);
                CREATE INDEX IF NOT EXISTS idx_artifacts_run ON artifacts(run_id);

                INSERT INTO schema_migrations (version, applied_at)
                VALUES (5, datetime('now'));
                ",
            )
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        }

        if current_version < 6 {
            conn.execute_batch(
                "
                CREATE TABLE IF NOT EXISTS run_events (
                    seq INTEGER PRIMARY KEY AUTOINCREMENT,
                    id TEXT UNIQUE NOT NULL,
                    run_id TEXT NOT NULL,
                    tenant_id TEXT NOT NULL,
                    workspace_id TEXT NOT NULL,
                    node_id TEXT,
                    event_type TEXT NOT NULL,
                    occurred_at TEXT NOT NULL,
                    payload_json TEXT NOT NULL
                );
                CREATE INDEX IF NOT EXISTS idx_run_events_run_seq ON run_events(run_id, seq);

                INSERT INTO schema_migrations (version, applied_at)
                VALUES (6, datetime('now'));
                ",
            )
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        }

        if current_version < 7 {
            let has_verified: bool = conn
                .query_row(
                    "SELECT COUNT(*) FROM pragma_table_info('runs') WHERE name = 'verified'",
                    [],
                    |r| r.get::<_, i64>(0).map(|c| c > 0),
                )
                .unwrap_or(false);

            if !has_verified {
                let _ = conn.execute(
                    "ALTER TABLE runs ADD COLUMN verified INTEGER NOT NULL DEFAULT 0",
                    [],
                );
            }

            conn.execute_batch(
                "
                CREATE TABLE IF NOT EXISTS run_evidence (
                    id TEXT PRIMARY KEY,
                    run_id TEXT NOT NULL,
                    command TEXT NOT NULL,
                    exit_code INTEGER NOT NULL,
                    stdout TEXT NOT NULL,
                    stderr TEXT NOT NULL,
                    duration_ms INTEGER NOT NULL,
                    verified INTEGER NOT NULL,
                    created_at TEXT NOT NULL,
                    output_sha256 TEXT NOT NULL DEFAULT '',
                    FOREIGN KEY(run_id) REFERENCES runs(id) ON DELETE CASCADE
                );
                CREATE INDEX IF NOT EXISTS idx_run_evidence_run ON run_evidence(run_id);

                INSERT INTO schema_migrations (version, applied_at)
                VALUES (7, datetime('now'));
                ",
            )
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        }

        if current_version < 8 {
            let has_col: bool = conn
                .query_row(
                    "SELECT COUNT(*) FROM pragma_table_info('run_evidence') WHERE name = 'output_sha256'",
                    [],
                    |r| r.get::<_, i64>(0).map(|c| c > 0),
                )
                .unwrap_or(false);

            if !has_col {
                let _ = conn.execute(
                    "ALTER TABLE run_evidence ADD COLUMN output_sha256 TEXT NOT NULL DEFAULT ''",
                    [],
                );
            }

            conn.execute(
                "INSERT INTO schema_migrations (version, applied_at) VALUES (8, datetime('now'))",
                [],
            )
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        }

        if current_version < 9 {
            let has_run_reason: bool = conn
                .query_row(
                    "SELECT COUNT(*) FROM pragma_table_info('runs') WHERE name = 'unverified_reason'",
                    [],
                    |r| r.get::<_, i64>(0).map(|c| c > 0),
                )
                .unwrap_or(false);

            if !has_run_reason {
                let _ = conn.execute("ALTER TABLE runs ADD COLUMN unverified_reason TEXT", []);
            }

            let has_evidence_reason: bool = conn
                .query_row(
                    "SELECT COUNT(*) FROM pragma_table_info('run_evidence') WHERE name = 'unverified_reason'",
                    [],
                    |r| r.get::<_, i64>(0).map(|c| c > 0),
                )
                .unwrap_or(false);

            if !has_evidence_reason {
                let _ = conn.execute(
                    "ALTER TABLE run_evidence ADD COLUMN unverified_reason TEXT",
                    [],
                );
            }

            conn.execute(
                "INSERT INTO schema_migrations (version, applied_at) VALUES (9, datetime('now'))",
                [],
            )
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        }

        if current_version < 10 {
            let has_capsule_tenant: bool = conn
                .query_row(
                    "SELECT COUNT(*) FROM pragma_table_info('capsules') WHERE name = 'tenant_id'",
                    [],
                    |r| r.get::<_, i64>(0).map(|c| c > 0),
                )
                .unwrap_or(false);

            if !has_capsule_tenant {
                conn.execute(
                    "ALTER TABLE capsules ADD COLUMN tenant_id TEXT NOT NULL DEFAULT ''",
                    [],
                )
                .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
                conn.execute(
                    "ALTER TABLE capsules ADD COLUMN workspace_id TEXT NOT NULL DEFAULT ''",
                    [],
                )
                .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
                conn.execute("ALTER TABLE capsules ADD COLUMN source_run_id TEXT", [])
                    .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
            }

            conn.execute_batch(
                "
                CREATE TABLE IF NOT EXISTS knowledge_capsules (
                    id TEXT PRIMARY KEY,
                    tenant_id TEXT NOT NULL,
                    workspace_id TEXT NOT NULL,
                    source_run_id TEXT,
                    source_agent_id TEXT NOT NULL,
                    source_role TEXT NOT NULL,
                    source_domain_tags_json TEXT NOT NULL,
                    trigger_preconditions TEXT NOT NULL,
                    distilled_heuristic TEXT NOT NULL,
                    prohibited_contexts_json TEXT NOT NULL,
                    propagation_scope TEXT NOT NULL,
                    sensitivity_level TEXT NOT NULL,
                    source_workflow_id TEXT,
                    source_crew_id TEXT,
                    confidence REAL NOT NULL DEFAULT 0.5,
                    negative_transfer INTEGER NOT NULL DEFAULT 0,
                    created_at TEXT NOT NULL
                );

                CREATE INDEX IF NOT EXISTS idx_knowledge_capsules_scope ON knowledge_capsules(tenant_id, workspace_id);
                CREATE INDEX IF NOT EXISTS idx_knowledge_capsules_run ON knowledge_capsules(source_run_id);

                INSERT INTO schema_migrations (version, applied_at)
                VALUES (10, datetime('now'));
                ",
            )
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        }

        if current_version < 11 {
            conn.execute_batch(
                "
                CREATE TABLE IF NOT EXISTS capsule_vectors (
                    id TEXT PRIMARY KEY,
                    capsule_id TEXT NOT NULL,
                    tenant_id TEXT NOT NULL,
                    workspace_id TEXT NOT NULL,
                    vector BLOB NOT NULL,
                    created_at TEXT NOT NULL
                );

                CREATE INDEX IF NOT EXISTS idx_capsule_vectors_scope ON capsule_vectors(tenant_id, workspace_id);
                CREATE INDEX IF NOT EXISTS idx_capsule_vectors_capsule ON capsule_vectors(capsule_id);

                INSERT INTO schema_migrations (version, applied_at)
                VALUES (11, datetime('now'));
                ",
            )
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        }

        if current_version < 12 {
            conn.execute_batch(
                "
                CREATE TABLE IF NOT EXISTS profiles (
                    id TEXT PRIMARY KEY,
                    name TEXT NOT NULL,
                    email TEXT,
                    is_local INTEGER NOT NULL DEFAULT 1,
                    active INTEGER NOT NULL DEFAULT 0,
                    created_at INTEGER NOT NULL,
                    updated_at INTEGER NOT NULL
                );

                CREATE TABLE IF NOT EXISTS provider_connections (
                    id TEXT PRIMARY KEY,
                    profile_id TEXT NOT NULL,
                    provider TEXT NOT NULL,
                    auth_type TEXT NOT NULL,
                    account_label TEXT,
                    created_at INTEGER NOT NULL,
                    updated_at INTEGER NOT NULL,
                    FOREIGN KEY(profile_id) REFERENCES profiles(id) ON DELETE CASCADE
                );

                CREATE INDEX IF NOT EXISTS idx_provider_connections_profile ON provider_connections(profile_id);

                INSERT INTO schema_migrations (version, applied_at)
                VALUES (12, datetime('now'));
                ",
            )
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        }

        if current_version < 13 {
            conn.execute_batch(
                "
                ALTER TABLE profiles ADD COLUMN sub TEXT;
                ALTER TABLE profiles ADD COLUMN picture TEXT;
                ALTER TABLE profiles ADD COLUMN tenant_id TEXT;
                ALTER TABLE profiles ADD COLUMN workspace_id TEXT;
                CREATE UNIQUE INDEX IF NOT EXISTS idx_profiles_sub ON profiles(sub);

                CREATE TABLE IF NOT EXISTS account_legacy_claim (
                    id INTEGER PRIMARY KEY CHECK (id = 1),
                    claimed_by_sub TEXT NOT NULL,
                    claimed_at INTEGER NOT NULL
                );

                INSERT INTO schema_migrations (version, applied_at)
                VALUES (13, datetime('now'));
                ",
            )
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        }

        Ok(())
    }

    pub fn insert_workflow(
        &self,
        id: &str,
        name: &str,
        description: Option<&str>,
        spec_json: &str,
    ) -> Result<(), CheckpointError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO workflow_specs (id, name, description, spec_json, version, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, 1, ?5, ?6)",
            params![id, name, description, spec_json, now, now],
        )
        .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        Ok(())
    }

    pub fn update_workflow(
        &self,
        id: &str,
        name: &str,
        description: Option<&str>,
        spec_json: &str,
    ) -> Result<bool, CheckpointError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        let now = Utc::now().to_rfc3339();
        let rows = conn
            .execute(
                "UPDATE workflow_specs SET name = ?1, description = ?2, spec_json = ?3, updated_at = ?4 WHERE id = ?5",
                params![name, description, spec_json, now, id],
            )
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        Ok(rows > 0)
    }

    pub fn get_workflow(&self, id: &str) -> Result<Option<String>, CheckpointError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        conn.query_row(
            "SELECT spec_json FROM workflow_specs WHERE id = ?1",
            params![id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| CheckpointError::DatabaseError(e.to_string()))
    }

    pub fn list_workflows(&self, limit: usize) -> Result<Vec<WorkflowSummary>, CheckpointError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        let mut stmt = conn
            .prepare(
                "SELECT id, name, description, created_at, updated_at
                 FROM workflow_specs
                 ORDER BY created_at DESC
                 LIMIT ?1",
            )
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        let rows = stmt
            .query_map(params![limit as i64], |r| {
                Ok(WorkflowSummary {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    description: r.get(2)?,
                    created_at: r.get(3)?,
                    updated_at: r.get(4)?,
                })
            })
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        let mut out = Vec::new();
        for r in rows {
            out.push(r.map_err(|e| CheckpointError::DatabaseError(e.to_string()))?);
        }
        Ok(out)
    }

    pub fn insert_run(&self, run: &RunRecord) -> Result<(), CheckpointError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        let verified_int = if run.verified { 1 } else { 0 };
        conn.execute(
            "INSERT INTO runs (id, workflow_id, status, input_data, output_data, created_at, updated_at, completed_at, verified, unverified_reason)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                run.id,
                run.workflow_id,
                run.status,
                run.input_data,
                run.output_data,
                run.created_at,
                run.updated_at,
                run.completed_at,
                verified_int,
                run.unverified_reason,
            ],
        )
        .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        Ok(())
    }

    pub fn update_run_status(
        &self,
        id: &str,
        status: &str,
        output_data: Option<&str>,
        completed_at: Option<&str>,
    ) -> Result<bool, CheckpointError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        let now = Utc::now().to_rfc3339();
        let rows = conn
            .execute(
                "UPDATE runs SET status = ?1, output_data = coalesce(?2, output_data), completed_at = coalesce(?3, completed_at), updated_at = ?4 WHERE id = ?5",
                params![status, output_data, completed_at, now, id],
            )
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        Ok(rows > 0)
    }

    pub fn get_run(&self, id: &str) -> Result<Option<RunRecord>, CheckpointError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        conn.query_row(
            "SELECT id, workflow_id, status, input_data, output_data, created_at, updated_at, completed_at, verified, unverified_reason
             FROM runs WHERE id = ?1",
            params![id],
            |r| {
                let verified_int: i64 = r.get(8)?;
                Ok(RunRecord {
                    id: r.get(0)?,
                    workflow_id: r.get(1)?,
                    status: r.get(2)?,
                    input_data: r.get(3)?,
                    output_data: r.get(4)?,
                    created_at: r.get(5)?,
                    updated_at: r.get(6)?,
                    completed_at: r.get(7)?,
                    verified: verified_int == 1,
                    unverified_reason: r.get(9)?,
                })
            },
        )
        .optional()
        .map_err(|e| CheckpointError::DatabaseError(e.to_string()))
    }

    pub fn list_runs(
        &self,
        workflow_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<RunRecord>, CheckpointError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        let mut out = Vec::new();
        if let Some(wf_id) = workflow_id {
            let mut stmt = conn
                .prepare(
                    "SELECT id, workflow_id, status, input_data, output_data, created_at, updated_at, completed_at, verified, unverified_reason
                     FROM runs
                     WHERE workflow_id = ?1
                     ORDER BY created_at DESC
                     LIMIT ?2",
                )
                .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
            let rows = stmt
                .query_map(params![wf_id, limit as i64], |r| {
                    let verified_int: i64 = r.get(8)?;
                    Ok(RunRecord {
                        id: r.get(0)?,
                        workflow_id: r.get(1)?,
                        status: r.get(2)?,
                        input_data: r.get(3)?,
                        output_data: r.get(4)?,
                        created_at: r.get(5)?,
                        updated_at: r.get(6)?,
                        completed_at: r.get(7)?,
                        verified: verified_int == 1,
                        unverified_reason: r.get(9)?,
                    })
                })
                .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
            for r in rows {
                out.push(r.map_err(|e| CheckpointError::DatabaseError(e.to_string()))?);
            }
        } else {
            let mut stmt = conn
                .prepare(
                    "SELECT id, workflow_id, status, input_data, output_data, created_at, updated_at, completed_at, verified, unverified_reason
                     FROM runs
                     ORDER BY created_at DESC
                     LIMIT ?1",
                )
                .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
            let rows = stmt
                .query_map(params![limit as i64], |r| {
                    let verified_int: i64 = r.get(8)?;
                    Ok(RunRecord {
                        id: r.get(0)?,
                        workflow_id: r.get(1)?,
                        status: r.get(2)?,
                        input_data: r.get(3)?,
                        output_data: r.get(4)?,
                        created_at: r.get(5)?,
                        updated_at: r.get(6)?,
                        completed_at: r.get(7)?,
                        verified: verified_int == 1,
                        unverified_reason: r.get(9)?,
                    })
                })
                .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
            for r in rows {
                out.push(r.map_err(|e| CheckpointError::DatabaseError(e.to_string()))?);
            }
        }
        Ok(out)
    }

    pub fn insert_run_evidence(&self, record: &RunEvidenceRecord) -> Result<(), CheckpointError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        let effective_verified = record.verified && (record.exit_code == 0);
        let verified_int = if effective_verified { 1 } else { 0 };
        let output_hash = record.output_sha256();

        conn.execute(
            "INSERT INTO run_evidence (id, run_id, command, exit_code, stdout, stderr, duration_ms, verified, created_at, output_sha256, unverified_reason)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                record.id,
                record.run_id,
                record.command,
                record.exit_code,
                record.stdout,
                record.stderr,
                record.duration_ms,
                verified_int,
                record.created_at,
                output_hash,
                record.unverified_reason,
            ],
        )
        .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        if record.exit_code != 0 || !record.verified {
            let reason = record.unverified_reason.clone().unwrap_or_else(|| {
                if record.exit_code != 0 {
                    format!("Verification command exited with code {}", record.exit_code)
                } else {
                    "Verification command marked unverified".to_string()
                }
            });
            let _ = conn.execute(
                "UPDATE runs SET verified = 0, unverified_reason = ?2, updated_at = ?3 WHERE id = ?1",
                params![record.run_id, reason, Utc::now().to_rfc3339()],
            );
        }

        Ok(())
    }

    pub fn list_run_evidence(
        &self,
        run_id: &str,
    ) -> Result<Vec<RunEvidenceRecord>, CheckpointError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        let mut stmt = conn
            .prepare(
                "SELECT id, run_id, command, exit_code, stdout, stderr, duration_ms, verified, created_at, output_sha256, unverified_reason
                 FROM run_evidence WHERE run_id = ?1 ORDER BY created_at ASC",
            )
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        let rows = stmt
            .query_map(params![run_id], |r| {
                let verified_int: i64 = r.get(7)?;
                Ok(RunEvidenceRecord {
                    id: r.get(0)?,
                    run_id: r.get(1)?,
                    command: r.get(2)?,
                    exit_code: r.get(3)?,
                    stdout: r.get(4)?,
                    stderr: r.get(5)?,
                    duration_ms: r.get(6)?,
                    verified: verified_int == 1,
                    created_at: r.get(8)?,
                    output_sha256: r.get::<_, Option<String>>(9)?.unwrap_or_default(),
                    unverified_reason: r.get(10)?,
                })
            })
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        let mut out = Vec::new();
        for row in rows {
            out.push(row.map_err(|e| CheckpointError::DatabaseError(e.to_string()))?);
        }
        Ok(out)
    }

    pub fn mark_run_verified(&self, run_id: &str, verified: bool) -> Result<bool, CheckpointError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        if !verified {
            let reason = "Verification cleared";
            let _ = conn.execute(
                "UPDATE runs SET verified = 0, unverified_reason = coalesce(unverified_reason, ?2), updated_at = ?3 WHERE id = ?1",
                params![run_id, reason, Utc::now().to_rfc3339()],
            );
            return Ok(false);
        }

        let evidence_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM run_evidence WHERE run_id = ?1",
                params![run_id],
                |r| r.get(0),
            )
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        if evidence_count == 0 {
            let reason = "No verification evidence recorded for run";
            let _ = conn.execute(
                "UPDATE runs SET verified = 0, unverified_reason = ?2, updated_at = ?3 WHERE id = ?1",
                params![run_id, reason, Utc::now().to_rfc3339()],
            );
            return Ok(false);
        }

        let (latest_exit, latest_verified): (i32, i64) = conn
            .query_row(
                "SELECT exit_code, verified FROM run_evidence WHERE run_id = ?1 ORDER BY created_at DESC, id DESC LIMIT 1",
                params![run_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        if latest_exit != 0 {
            let reason = format!(
                "Latest verification command exited with non-zero code {}",
                latest_exit
            );
            let _ = conn.execute(
                "UPDATE runs SET verified = 0, unverified_reason = ?2, updated_at = ?3 WHERE id = ?1",
                params![run_id, reason, Utc::now().to_rfc3339()],
            );
            return Ok(false);
        }

        if latest_verified == 0 {
            let reason = "Latest verification evidence is marked unverified".to_string();
            let _ = conn.execute(
                "UPDATE runs SET verified = 0, unverified_reason = ?2, updated_at = ?3 WHERE id = ?1",
                params![run_id, reason, Utc::now().to_rfc3339()],
            );
            return Ok(false);
        }

        let unverified_evidence_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM run_evidence WHERE run_id = ?1 AND (exit_code != 0 OR verified = 0)",
                params![run_id],
                |r| r.get(0),
            )
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        if unverified_evidence_count > 0 {
            let reason = "Run contains unverified or non-zero exit evidence records".to_string();
            let _ = conn.execute(
                "UPDATE runs SET verified = 0, unverified_reason = ?2, updated_at = ?3 WHERE id = ?1",
                params![run_id, reason, Utc::now().to_rfc3339()],
            );
            return Ok(false);
        }

        let mut stmt = conn
            .prepare(
                "SELECT id, run_id, filename, file_path, file_size, mime_type, sha256, verified, created_at FROM artifacts WHERE run_id = ?1",
            )
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        let artifacts: Vec<ArtifactRecord> = stmt
            .query_map(params![run_id], |r| {
                let v: i64 = r.get(7)?;
                Ok(ArtifactRecord {
                    id: r.get(0)?,
                    run_id: r.get(1)?,
                    filename: r.get(2)?,
                    file_path: r.get(3)?,
                    file_size: r.get(4)?,
                    mime_type: r.get(5)?,
                    sha256: r.get(6)?,
                    verified: v == 1,
                    created_at: r.get(8)?,
                })
            })
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        if artifacts.is_empty() {
            let reason =
                "Refused verification: forged exit-0 evidence row without artifact records"
                    .to_string();
            let _ = conn.execute(
                "UPDATE runs SET verified = 0, unverified_reason = ?2, updated_at = ?3 WHERE id = ?1",
                params![run_id, reason, Utc::now().to_rfc3339()],
            );
            return Ok(false);
        }

        use sha2::{Digest, Sha256};
        for art in &artifacts {
            if !art.verified {
                let reason = format!("Artifact '{}' is marked unverified", art.filename);
                let _ = conn.execute(
                    "UPDATE runs SET verified = 0, unverified_reason = ?2, updated_at = ?3 WHERE id = ?1",
                    params![run_id, reason, Utc::now().to_rfc3339()],
                );
                return Ok(false);
            }

            let path = Path::new(&art.file_path);
            let target = if path.exists() {
                path.to_path_buf()
            } else {
                let fallback = Path::new(&art.filename);
                if fallback.exists() {
                    fallback.to_path_buf()
                } else {
                    let reason = format!(
                        "Artifact '{}' file missing on disk at '{}'",
                        art.filename, art.file_path
                    );
                    let _ = conn.execute(
                        "UPDATE runs SET verified = 0, unverified_reason = ?2, updated_at = ?3 WHERE id = ?1",
                        params![run_id, reason, Utc::now().to_rfc3339()],
                    );
                    return Ok(false);
                }
            };

            let bytes = match std::fs::read(&target) {
                Ok(b) => b,
                Err(e) => {
                    let reason = format!("Cannot read artifact '{}' on disk: {}", art.filename, e);
                    let _ = conn.execute(
                        "UPDATE runs SET verified = 0, unverified_reason = ?2, updated_at = ?3 WHERE id = ?1",
                        params![run_id, reason, Utc::now().to_rfc3339()],
                    );
                    return Ok(false);
                }
            };

            if bytes.len() as i64 != art.file_size {
                let reason = format!(
                    "Artifact '{}' size mismatch: recorded {} bytes, disk {} bytes",
                    art.filename,
                    art.file_size,
                    bytes.len()
                );
                let _ = conn.execute(
                    "UPDATE runs SET verified = 0, unverified_reason = ?2, updated_at = ?3 WHERE id = ?1",
                    params![run_id, reason, Utc::now().to_rfc3339()],
                );
                return Ok(false);
            }

            let mut hasher = Sha256::new();
            hasher.update(&bytes);
            let current_hash = format!("{:x}", hasher.finalize());

            if current_hash != art.sha256 {
                let reason = format!(
                    "Artifact '{}' SHA-256 hash mismatch: recorded {}, disk {}",
                    art.filename, art.sha256, current_hash
                );
                let _ = conn.execute(
                    "UPDATE runs SET verified = 0, unverified_reason = ?2, updated_at = ?3 WHERE id = ?1",
                    params![run_id, reason, Utc::now().to_rfc3339()],
                );
                return Ok(false);
            }
        }

        let updated = conn
            .execute(
                "UPDATE runs SET verified = 1, unverified_reason = NULL, updated_at = ?2 WHERE id = ?1",
                params![run_id, Utc::now().to_rfc3339()],
            )
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        Ok(updated > 0)
    }

    pub fn is_run_verified(&self, run_id: &str) -> Result<bool, CheckpointError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        let verified: i64 = conn
            .query_row(
                "SELECT verified FROM runs WHERE id = ?1",
                params![run_id],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?
            .unwrap_or(0);
        Ok(verified == 1)
    }

    pub fn get_run_evidence_output_hash(
        &self,
        evidence_id: &str,
    ) -> Result<Option<String>, CheckpointError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        conn.query_row(
            "SELECT output_sha256 FROM run_evidence WHERE id = ?1",
            params![evidence_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| CheckpointError::DatabaseError(e.to_string()))
    }

    pub fn check_artifact_tampering(
        &self,
        workspace_root: &std::path::Path,
        run_id: &str,
    ) -> Result<bool, CheckpointError> {
        let artifacts = self.list_artifacts(run_id)?;
        if artifacts.is_empty() {
            return Ok(false);
        }

        use sha2::{Digest, Sha256};
        for art in &artifacts {
            let path = workspace_root.join(&art.filename);
            let target = if path.exists() {
                path
            } else {
                workspace_root.join(&art.file_path)
            };

            if !target.exists() {
                let _ = self.mark_run_verified(run_id, false);
                return Ok(true);
            }

            let data = match std::fs::read(&target) {
                Ok(d) => d,
                Err(_) => {
                    let _ = self.mark_run_verified(run_id, false);
                    return Ok(true);
                }
            };

            let mut hasher = Sha256::new();
            hasher.update(&data);
            let current_hash = format!("{:x}", hasher.finalize());

            if current_hash != art.sha256 {
                let _ = self.mark_run_verified(run_id, false);
                return Ok(true);
            }
        }

        Ok(false)
    }

    pub fn insert_artifact(&self, artifact: &ArtifactRecord) -> Result<(), CheckpointError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        conn.execute(
            "INSERT INTO artifacts (id, run_id, filename, file_path, file_size, mime_type, sha256, verified, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                artifact.id,
                artifact.run_id,
                artifact.filename,
                artifact.file_path,
                artifact.file_size,
                artifact.mime_type,
                artifact.sha256,
                if artifact.verified { 1 } else { 0 },
                artifact.created_at,
            ],
        )
        .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        Ok(())
    }

    pub fn list_artifacts(&self, run_id: &str) -> Result<Vec<ArtifactRecord>, CheckpointError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        let mut stmt = conn
            .prepare(
                "SELECT id, run_id, filename, file_path, file_size, mime_type, sha256, verified, created_at
                 FROM artifacts
                 WHERE run_id = ?1
                 ORDER BY created_at ASC",
            )
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        let rows = stmt
            .query_map(params![run_id], |r| {
                let v: i64 = r.get(7)?;
                Ok(ArtifactRecord {
                    id: r.get(0)?,
                    run_id: r.get(1)?,
                    filename: r.get(2)?,
                    file_path: r.get(3)?,
                    file_size: r.get(4)?,
                    mime_type: r.get(5)?,
                    sha256: r.get(6)?,
                    verified: v != 0,
                    created_at: r.get(8)?,
                })
            })
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r.map_err(|e| CheckpointError::DatabaseError(e.to_string()))?);
        }
        Ok(out)
    }

    pub fn insert_capsule(&self, capsule: &CapsuleRecord) -> Result<(), CheckpointError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        conn.execute(
            "INSERT INTO capsules (id, title, trigger_pattern, advice, scope, sensitivity, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                capsule.id,
                capsule.title,
                capsule.trigger_pattern,
                capsule.advice,
                capsule.scope,
                capsule.sensitivity,
                capsule.created_at,
            ],
        )
        .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        Ok(())
    }

    pub fn list_capsules(&self) -> Result<Vec<CapsuleRecord>, CheckpointError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        let mut stmt = conn
            .prepare(
                "SELECT id, title, trigger_pattern, advice, scope, sensitivity, created_at
                 FROM capsules
                 ORDER BY created_at DESC",
            )
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        let rows = stmt
            .query_map([], |r| {
                Ok(CapsuleRecord {
                    id: r.get(0)?,
                    title: r.get(1)?,
                    trigger_pattern: r.get(2)?,
                    advice: r.get(3)?,
                    scope: r.get(4)?,
                    sensitivity: r.get(5)?,
                    created_at: r.get(6)?,
                })
            })
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r.map_err(|e| CheckpointError::DatabaseError(e.to_string()))?);
        }
        Ok(out)
    }

    pub fn insert_knowledge_capsule(
        &self,
        capsule: &KnowledgeCapsuleRecord,
    ) -> Result<(), CheckpointError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        let tags_json = serde_json::to_string(&capsule.source_domain_tags)
            .map_err(|e| CheckpointError::SerializationError(e.to_string()))?;
        let prohibited_json = serde_json::to_string(&capsule.prohibited_contexts)
            .map_err(|e| CheckpointError::SerializationError(e.to_string()))?;

        conn.execute(
            "INSERT INTO knowledge_capsules (
                id, tenant_id, workspace_id, source_run_id, source_agent_id, source_role,
                source_domain_tags_json, trigger_preconditions, distilled_heuristic,
                prohibited_contexts_json, propagation_scope, sensitivity_level,
                source_workflow_id, source_crew_id, confidence, negative_transfer, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
            params![
                capsule.id,
                capsule.tenant_id,
                capsule.workspace_id,
                capsule.source_run_id,
                capsule.source_agent_id,
                capsule.source_role,
                tags_json,
                capsule.trigger_preconditions,
                capsule.distilled_heuristic,
                prohibited_json,
                capsule.propagation_scope,
                capsule.sensitivity_level,
                capsule.source_workflow_id,
                capsule.source_crew_id,
                capsule.confidence,
                if capsule.negative_transfer { 1 } else { 0 },
                capsule.created_at,
            ],
        )
        .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        Ok(())
    }

    pub fn get_knowledge_capsule(
        &self,
        id: &str,
    ) -> Result<Option<KnowledgeCapsuleRecord>, CheckpointError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        let mut stmt = conn
            .prepare(
                "SELECT id, tenant_id, workspace_id, source_run_id, source_agent_id, source_role,
                        source_domain_tags_json, trigger_preconditions, distilled_heuristic,
                        prohibited_contexts_json, propagation_scope, sensitivity_level,
                        source_workflow_id, source_crew_id, confidence, negative_transfer, created_at
                 FROM knowledge_capsules
                 WHERE id = ?1",
            )
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        let mut rows = stmt
            .query_map([id], |r| {
                let tags_str: String = r.get(6)?;
                let prohibited_str: String = r.get(9)?;
                let neg_i: i64 = r.get(15)?;
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, Option<String>>(3)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, String>(5)?,
                    tags_str,
                    r.get::<_, String>(7)?,
                    r.get::<_, String>(8)?,
                    prohibited_str,
                    r.get::<_, String>(10)?,
                    r.get::<_, String>(11)?,
                    r.get::<_, Option<String>>(12)?,
                    r.get::<_, Option<String>>(13)?,
                    r.get::<_, f64>(14)?,
                    neg_i != 0,
                    r.get::<_, String>(16)?,
                ))
            })
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        if let Some(res) = rows.next() {
            let (
                id,
                tenant_id,
                workspace_id,
                source_run_id,
                source_agent_id,
                source_role,
                tags_str,
                trigger_preconditions,
                distilled_heuristic,
                prohibited_str,
                propagation_scope,
                sensitivity_level,
                source_workflow_id,
                source_crew_id,
                confidence,
                negative_transfer,
                created_at,
            ) = res.map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
            let source_domain_tags: Vec<String> =
                serde_json::from_str(&tags_str).unwrap_or_default();
            let prohibited_contexts: Vec<String> =
                serde_json::from_str(&prohibited_str).unwrap_or_default();

            Ok(Some(KnowledgeCapsuleRecord {
                id,
                tenant_id,
                workspace_id,
                source_run_id,
                source_agent_id,
                source_role,
                source_domain_tags,
                trigger_preconditions,
                distilled_heuristic,
                prohibited_contexts,
                propagation_scope,
                sensitivity_level,
                source_workflow_id,
                source_crew_id,
                confidence,
                negative_transfer,
                created_at,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn list_knowledge_capsules(
        &self,
        tenant_id: &str,
        workspace_id: &str,
    ) -> Result<Vec<KnowledgeCapsuleRecord>, CheckpointError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        let mut stmt = conn
            .prepare(
                "SELECT id, tenant_id, workspace_id, source_run_id, source_agent_id, source_role,
                        source_domain_tags_json, trigger_preconditions, distilled_heuristic,
                        prohibited_contexts_json, propagation_scope, sensitivity_level,
                        source_workflow_id, source_crew_id, confidence, negative_transfer, created_at
                 FROM knowledge_capsules
                 WHERE tenant_id = ?1 AND workspace_id = ?2
                 ORDER BY created_at DESC",
            )
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        let rows = stmt
            .query_map(params![tenant_id, workspace_id], |r| {
                let tags_str: String = r.get(6)?;
                let prohibited_str: String = r.get(9)?;
                let neg_i: i64 = r.get(15)?;
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, Option<String>>(3)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, String>(5)?,
                    tags_str,
                    r.get::<_, String>(7)?,
                    r.get::<_, String>(8)?,
                    prohibited_str,
                    r.get::<_, String>(10)?,
                    r.get::<_, String>(11)?,
                    r.get::<_, Option<String>>(12)?,
                    r.get::<_, Option<String>>(13)?,
                    r.get::<_, f64>(14)?,
                    neg_i != 0,
                    r.get::<_, String>(16)?,
                ))
            })
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        let mut res = Vec::new();
        for row in rows {
            let (
                id,
                tenant_id,
                workspace_id,
                source_run_id,
                source_agent_id,
                source_role,
                tags_str,
                trigger_preconditions,
                distilled_heuristic,
                prohibited_str,
                propagation_scope,
                sensitivity_level,
                source_workflow_id,
                source_crew_id,
                confidence,
                negative_transfer,
                created_at,
            ) = row.map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
            let source_domain_tags: Vec<String> =
                serde_json::from_str(&tags_str).unwrap_or_default();
            let prohibited_contexts: Vec<String> =
                serde_json::from_str(&prohibited_str).unwrap_or_default();

            res.push(KnowledgeCapsuleRecord {
                id,
                tenant_id,
                workspace_id,
                source_run_id,
                source_agent_id,
                source_role,
                source_domain_tags,
                trigger_preconditions,
                distilled_heuristic,
                prohibited_contexts,
                propagation_scope,
                sensitivity_level,
                source_workflow_id,
                source_crew_id,
                confidence,
                negative_transfer,
                created_at,
            });
        }
        Ok(res)
    }

    pub fn set_knowledge_capsule_negative_transfer(
        &self,
        id: &str,
        negative: bool,
    ) -> Result<(), CheckpointError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        let rows_affected = conn
            .execute(
                "UPDATE knowledge_capsules SET negative_transfer = ?1 WHERE id = ?2",
                params![if negative { 1 } else { 0 }, id],
            )
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        if rows_affected == 0 {
            return Err(CheckpointError::NotFound(format!(
                "Knowledge capsule {} not found",
                id
            )));
        }
        Ok(())
    }

    pub fn get_execution_policy(&self) -> Result<ExecutionPolicyRecord, CheckpointError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        conn.query_row(
            "SELECT default_mode, allowed_domains_json, denied_domains_json FROM execution_policy WHERE id = 1",
            [],
            |r| {
                let default_mode: String = r.get(0)?;
                let allowed_json: String = r.get(1)?;
                let denied_json: String = r.get(2)?;
                let allowed_domains = serde_json::from_str(&allowed_json).unwrap_or_default();
                let denied_domains = serde_json::from_str(&denied_json).unwrap_or_default();
                Ok(ExecutionPolicyRecord {
                    default_mode,
                    allowed_domains,
                    denied_domains,
                })
            },
        )
        .map_err(|e| CheckpointError::DatabaseError(e.to_string()))
    }

    pub fn update_execution_policy(
        &self,
        policy: &ExecutionPolicyRecord,
    ) -> Result<(), CheckpointError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        let allowed_json = serde_json::to_string(&policy.allowed_domains)
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        let denied_json = serde_json::to_string(&policy.denied_domains)
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE execution_policy SET default_mode = ?1, allowed_domains_json = ?2, denied_domains_json = ?3, updated_at = ?4 WHERE id = 1",
            params![policy.default_mode, allowed_json, denied_json, now],
        )
        .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        Ok(())
    }

    pub fn list_pending_approvals_by_run(
        &self,
        run_id: Option<&str>,
    ) -> Result<Vec<ApprovalRequestSpec>, CheckpointError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        let mut out = Vec::new();
        if let Some(r_id) = run_id {
            let mut stmt = conn
                .prepare(
                    "SELECT id, run_id, tenant_id, workspace_id, tool_name, action_description, author_id, status, decided_by
                     FROM approvals
                     WHERE run_id = ?1 AND status = 'pending'
                     ORDER BY created_at ASC",
                )
                .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
            let rows = stmt
                .query_map(params![r_id], |r| {
                    let id_str: String = r.get(0)?;
                    let run_id_str: String = r.get(1)?;
                    let tenant_id_str: String = r.get(2)?;
                    let workspace_id_str: String = r.get(3)?;
                    let tool_name: String = r.get(4)?;
                    let action_desc: String = r.get(5)?;
                    let author_actor: String = r.get::<_, Option<String>>(6)?.unwrap_or_default();
                    let status: String = r.get(7)?;
                    let approver_actor: Option<String> = r.get(8)?;
                    let parameters = serde_json::json!({ "action": action_desc });
                    Ok(ApprovalRequestSpec {
                        id: Uuid::from_str(&id_str).unwrap_or_default(),
                        run_id: Uuid::from_str(&run_id_str).unwrap_or_default(),
                        tenant_id: Uuid::from_str(&tenant_id_str).unwrap_or_default(),
                        workspace_id: Uuid::from_str(&workspace_id_str).unwrap_or_default(),
                        tool_name,
                        parameters,
                        diff_preview: None,
                        author_actor,
                        status,
                        approver_actor,
                        decision: None,
                    })
                })
                .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
            for row in rows {
                out.push(row.map_err(|e| CheckpointError::DatabaseError(e.to_string()))?);
            }
        } else {
            let mut stmt = conn
                .prepare(
                    "SELECT id, run_id, tenant_id, workspace_id, tool_name, action_description, author_id, status, decided_by
                     FROM approvals
                     WHERE status = 'pending'
                     ORDER BY created_at ASC",
                )
                .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
            let rows = stmt
                .query_map([], |r| {
                    let id_str: String = r.get(0)?;
                    let run_id_str: String = r.get(1)?;
                    let tenant_id_str: String = r.get(2)?;
                    let workspace_id_str: String = r.get(3)?;
                    let tool_name: String = r.get(4)?;
                    let action_desc: String = r.get(5)?;
                    let author_actor: String = r.get::<_, Option<String>>(6)?.unwrap_or_default();
                    let status: String = r.get(7)?;
                    let approver_actor: Option<String> = r.get(8)?;
                    let parameters = serde_json::json!({ "action": action_desc });
                    Ok(ApprovalRequestSpec {
                        id: Uuid::from_str(&id_str).unwrap_or_default(),
                        run_id: Uuid::from_str(&run_id_str).unwrap_or_default(),
                        tenant_id: Uuid::from_str(&tenant_id_str).unwrap_or_default(),
                        workspace_id: Uuid::from_str(&workspace_id_str).unwrap_or_default(),
                        tool_name,
                        parameters,
                        diff_preview: None,
                        author_actor,
                        status,
                        approver_actor,
                        decision: None,
                    })
                })
                .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
            for row in rows {
                out.push(row.map_err(|e| CheckpointError::DatabaseError(e.to_string()))?);
            }
        }
        Ok(out)
    }

    pub fn get_approval_details(
        &self,
        approval_id: &str,
    ) -> Result<Option<(ExecutionScope, String, String)>, CheckpointError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        let res = conn
            .query_row(
                "SELECT tenant_id, workspace_id, run_id, author_id, status FROM approvals WHERE id = ?1",
                params![approval_id],
                |row| {
                    let t_str: String = row.get(0)?;
                    let w_str: String = row.get(1)?;
                    let r_str: String = row.get(2)?;
                    let author: String = row.get::<_, Option<String>>(3)?.unwrap_or_default();
                    let status: String = row.get(4)?;
                    Ok((t_str, w_str, r_str, author, status))
                },
            )
            .optional()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        if let Some((t_str, w_str, r_str, author, status)) = res {
            let tenant_id = Uuid::from_str(&t_str)
                .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
            let workspace_id = Uuid::from_str(&w_str)
                .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
            let run_id = Uuid::from_str(&r_str)
                .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
            Ok(Some((
                ExecutionScope {
                    tenant_id,
                    workspace_id,
                    run_id,
                },
                author,
                status,
            )))
        } else {
            Ok(None)
        }
    }

    pub fn insert_run_event(&self, event: &RuntimeEvent) -> Result<(), CheckpointError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        let payload_json = serde_json::to_string(&event.payload)
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        let id_str = event.id.to_string();
        let run_id_str = event.scope.run_id.to_string();
        let tenant_id_str = event.scope.tenant_id.to_string();
        let workspace_id_str = event.scope.workspace_id.to_string();
        let occurred_at_str = event.occurred_at.to_rfc3339();

        conn.execute(
            "INSERT OR IGNORE INTO run_events (id, run_id, tenant_id, workspace_id, node_id, event_type, occurred_at, payload_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                id_str,
                run_id_str,
                tenant_id_str,
                workspace_id_str,
                event.node_id,
                event.event_type.as_str(),
                occurred_at_str,
                payload_json
            ],
        )
        .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    pub fn list_run_events(
        &self,
        run_id: &str,
        after_event_id: Option<&str>,
    ) -> Result<Vec<RuntimeEvent>, CheckpointError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        let after_seq: i64 = if let Some(after_id) = after_event_id {
            conn.query_row(
                "SELECT seq FROM run_events WHERE id = ?1",
                params![after_id],
                |row| row.get(0),
            )
            .unwrap_or(0)
        } else {
            0
        };

        let mut stmt = conn
            .prepare(
                "SELECT id, run_id, tenant_id, workspace_id, node_id, event_type, occurred_at, payload_json
                 FROM run_events
                 WHERE run_id = ?1 AND seq > ?2
                 ORDER BY seq ASC",
            )
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        let rows = stmt
            .query_map(params![run_id, after_seq], |row| {
                let id_str: String = row.get(0)?;
                let run_id_str: String = row.get(1)?;
                let tenant_id_str: String = row.get(2)?;
                let workspace_id_str: String = row.get(3)?;
                let node_id: Option<String> = row.get(4)?;
                let event_type_str: String = row.get(5)?;
                let occurred_at_str: String = row.get(6)?;
                let payload_json: String = row.get(7)?;

                Ok((
                    id_str,
                    run_id_str,
                    tenant_id_str,
                    workspace_id_str,
                    node_id,
                    event_type_str,
                    occurred_at_str,
                    payload_json,
                ))
            })
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        let mut events = Vec::new();
        for r in rows {
            let (
                id_str,
                run_id_str,
                tenant_id_str,
                workspace_id_str,
                node_id,
                event_type_str,
                occurred_at_str,
                payload_json,
            ) = r.map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

            let id = Uuid::from_str(&id_str)
                .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
            let run_id = Uuid::from_str(&run_id_str)
                .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
            let tenant_id = Uuid::from_str(&tenant_id_str)
                .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
            let workspace_id = Uuid::from_str(&workspace_id_str)
                .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
            let occurred_at = chrono::DateTime::parse_from_rfc3339(&occurred_at_str)
                .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?
                .with_timezone(&Utc);
            let payload: serde_json::Value = serde_json::from_str(&payload_json)
                .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

            let event_type = match event_type_str.as_str() {
                "TOOL_INVOKED" => RuntimeEventType::ToolInvoked,
                "TOOL_COMPLETED" => RuntimeEventType::ToolCompleted,
                "APPROVAL_REQUIRED" => RuntimeEventType::ApprovalRequired,
                "APPROVAL_RESOLVED" => RuntimeEventType::ApprovalResolved,
                "FLOW_STATE_CHANGED" => RuntimeEventType::FlowStateChanged,
                "SANDBOX_TERMINAL_OUTPUT" => RuntimeEventType::SandboxTerminalOutput,
                "SANDBOX_FILE_DIFF" => RuntimeEventType::SandboxFileDiff,
                "TRAJECTORY_FORKED" => RuntimeEventType::TrajectoryForked,
                "RUN_COMPLETED" => RuntimeEventType::RunCompleted,
                "RUN_FAILED" => RuntimeEventType::RunFailed,
                _ => RuntimeEventType::FlowStateChanged,
            };

            events.push(RuntimeEvent {
                id,
                scope: ExecutionScope {
                    tenant_id,
                    workspace_id,
                    run_id,
                },
                node_id,
                event_type,
                occurred_at,
                payload,
            });
        }

        Ok(events)
    }

    pub fn insert_capsule_vector(
        &self,
        capsule_id: &str,
        tenant_id: &str,
        workspace_id: &str,
        vector: &[f32],
    ) -> Result<(), CheckpointError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        let id = uuid::Uuid::new_v4().to_string();
        let bytes = crate::memory::vector::vector_to_bytes(vector);
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO capsule_vectors (id, capsule_id, tenant_id, workspace_id, vector, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![id, capsule_id, tenant_id, workspace_id, bytes, now],
        )
        .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        Ok(())
    }

    pub fn search_capsules_vector(
        &self,
        tenant_id: &str,
        workspace_id: &str,
        query_vector: &[f32],
        top_k: usize,
    ) -> Result<Vec<(KnowledgeCapsuleRecord, f32)>, CheckpointError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        let mut stmt = conn
            .prepare(
                "SELECT c.id, c.tenant_id, c.workspace_id, c.source_run_id, c.source_agent_id, c.source_role,
                        c.source_domain_tags_json, c.trigger_preconditions, c.distilled_heuristic,
                        c.prohibited_contexts_json, c.propagation_scope, c.sensitivity_level,
                        c.source_workflow_id, c.source_crew_id, c.confidence, c.negative_transfer,
                        c.created_at, v.vector
                 FROM capsule_vectors v
                 JOIN knowledge_capsules c ON v.capsule_id = c.id
                 WHERE v.tenant_id = ?1 AND v.workspace_id = ?2 AND c.negative_transfer = 0",
            )
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        let rows = stmt
            .query_map(params![tenant_id, workspace_id], |r| {
                let tags_str: String = r.get(6)?;
                let prohibited_str: String = r.get(9)?;
                let neg_i: i64 = r.get(15)?;
                let vec_blob: Vec<u8> = r.get(17)?;
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, Option<String>>(3)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, String>(5)?,
                    tags_str,
                    r.get::<_, String>(7)?,
                    r.get::<_, String>(8)?,
                    prohibited_str,
                    r.get::<_, String>(10)?,
                    r.get::<_, String>(11)?,
                    r.get::<_, Option<String>>(12)?,
                    r.get::<_, Option<String>>(13)?,
                    r.get::<_, f64>(14)?,
                    neg_i != 0,
                    r.get::<_, String>(16)?,
                    vec_blob,
                ))
            })
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        let mut scored = Vec::new();
        for row in rows {
            let (
                id,
                tenant_id,
                workspace_id,
                source_run_id,
                source_agent_id,
                source_role,
                tags_str,
                trigger_preconditions,
                distilled_heuristic,
                prohibited_str,
                propagation_scope,
                sensitivity_level,
                source_workflow_id,
                source_crew_id,
                confidence,
                negative_transfer,
                created_at,
                vec_blob,
            ) = row.map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

            let stored_vec = crate::memory::vector::bytes_to_vector(&vec_blob)
                .map_err(CheckpointError::SerializationError)?;
            let score = crate::memory::vector::cosine_similarity(query_vector, &stored_vec);

            let source_domain_tags: Vec<String> =
                serde_json::from_str(&tags_str).unwrap_or_default();
            let prohibited_contexts: Vec<String> =
                serde_json::from_str(&prohibited_str).unwrap_or_default();

            let capsule = KnowledgeCapsuleRecord {
                id,
                tenant_id,
                workspace_id,
                source_run_id,
                source_agent_id,
                source_role,
                source_domain_tags,
                trigger_preconditions,
                distilled_heuristic,
                prohibited_contexts,
                propagation_scope,
                sensitivity_level,
                source_workflow_id,
                source_crew_id,
                confidence,
                negative_transfer,
                created_at,
            };
            scored.push((capsule, score));
        }

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        if scored.len() > top_k {
            scored.truncate(top_k);
        }
        Ok(scored)
    }

    pub fn create_profile(&self, profile: &Profile) -> Result<Profile, CheckpointError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        conn.execute(
            "INSERT INTO profiles (id, name, email, is_local, active, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                profile.id,
                profile.name,
                profile.email,
                if profile.is_local { 1 } else { 0 },
                if profile.active { 1 } else { 0 },
                profile.created_at,
                profile.updated_at,
            ],
        )
        .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        Ok(profile.clone())
    }

    pub fn get_profile(&self, id: &str) -> Result<Option<Profile>, CheckpointError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        let mut stmt = conn
            .prepare(
                "SELECT id, name, email, is_local, active, created_at, updated_at
                 FROM profiles WHERE id = ?1",
            )
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        let mut rows = stmt
            .query(params![id])
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        if let Some(row) = rows
            .next()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?
        {
            let is_local_int: i64 = row
                .get(3)
                .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
            let active_int: i64 = row
                .get(4)
                .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
            Ok(Some(Profile {
                id: row
                    .get(0)
                    .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?,
                name: row
                    .get(1)
                    .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?,
                email: row
                    .get(2)
                    .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?,
                is_local: is_local_int != 0,
                active: active_int != 0,
                created_at: row
                    .get(5)
                    .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?,
                updated_at: row
                    .get(6)
                    .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn get_active_profile(&self) -> Result<Option<Profile>, CheckpointError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        let mut stmt = conn
            .prepare(
                "SELECT id, name, email, is_local, active, created_at, updated_at
                 FROM profiles WHERE active = 1 ORDER BY updated_at DESC LIMIT 1",
            )
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        let mut rows = stmt
            .query([])
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        if let Some(row) = rows
            .next()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?
        {
            let is_local_int: i64 = row
                .get(3)
                .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
            let active_int: i64 = row
                .get(4)
                .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
            Ok(Some(Profile {
                id: row
                    .get(0)
                    .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?,
                name: row
                    .get(1)
                    .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?,
                email: row
                    .get(2)
                    .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?,
                is_local: is_local_int != 0,
                active: active_int != 0,
                created_at: row
                    .get(5)
                    .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?,
                updated_at: row
                    .get(6)
                    .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn set_active_profile(&self, id: &str) -> Result<(), CheckpointError> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        let tx = conn
            .transaction()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        tx.execute("UPDATE profiles SET active = 0", [])
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        let now = Utc::now().timestamp();
        let changed = tx
            .execute(
                "UPDATE profiles SET active = 1, updated_at = ?2 WHERE id = ?1",
                params![id, now],
            )
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        if changed == 0 {
            return Err(CheckpointError::NotFound(format!("Profile {id} not found")));
        }
        tx.commit()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        Ok(())
    }

    pub fn list_profiles(&self) -> Result<Vec<Profile>, CheckpointError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        let mut stmt = conn
            .prepare(
                "SELECT id, name, email, is_local, active, created_at, updated_at
                 FROM profiles ORDER BY created_at ASC",
            )
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        let mut rows = stmt
            .query([])
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        let mut list = Vec::new();
        while let Some(row) = rows
            .next()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?
        {
            let is_local_int: i64 = row
                .get(3)
                .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
            let active_int: i64 = row
                .get(4)
                .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
            list.push(Profile {
                id: row
                    .get(0)
                    .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?,
                name: row
                    .get(1)
                    .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?,
                email: row
                    .get(2)
                    .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?,
                is_local: is_local_int != 0,
                active: active_int != 0,
                created_at: row
                    .get(5)
                    .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?,
                updated_at: row
                    .get(6)
                    .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?,
            });
        }
        Ok(list)
    }

    pub fn delete_profile(&self, id: &str) -> Result<bool, CheckpointError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        let changed = conn
            .execute("DELETE FROM profiles WHERE id = ?1", params![id])
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        Ok(changed > 0)
    }

    pub fn ensure_default_profile(&self) -> Result<Profile, CheckpointError> {
        if let Some(profile) = self.get_active_profile()? {
            return Ok(profile);
        }
        let list = self.list_profiles()?;
        if let Some(first) = list.first() {
            self.set_active_profile(&first.id)?;
            return Ok(first.clone());
        }
        let now = Utc::now().timestamp();
        let default_profile = Profile {
            id: "default".to_string(),
            name: "Local Profile".to_string(),
            email: None,
            is_local: true,
            active: true,
            created_at: now,
            updated_at: now,
        };
        self.create_profile(&default_profile)
    }

    pub fn upsert_google_account(
        &self,
        account: &crate::profiles::GoogleAccountRecord,
    ) -> Result<(), CheckpointError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        conn.execute(
            "INSERT INTO profiles (id, name, email, is_local, active, created_at, updated_at, sub, picture, tenant_id, workspace_id)
             VALUES (?1, ?2, ?3, 0, ?4, ?5, ?6, ?1, ?7, ?8, ?9)
             ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                email = excluded.email,
                active = excluded.active,
                updated_at = excluded.updated_at,
                picture = excluded.picture,
                tenant_id = excluded.tenant_id,
                workspace_id = excluded.workspace_id",
            params![
                account.sub,
                account.display_name,
                account.email,
                if account.active { 1 } else { 0 },
                account.created_at,
                account.updated_at,
                account.picture,
                account.tenant_id,
                account.workspace_id,
            ],
        )
        .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        Ok(())
    }

    pub fn get_account_by_sub(
        &self,
        sub: &str,
    ) -> Result<Option<crate::profiles::GoogleAccountRecord>, CheckpointError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        let mut stmt = conn
            .prepare(
                "SELECT sub, email, name, picture, tenant_id, workspace_id, active, created_at, updated_at
                 FROM profiles WHERE sub = ?1 LIMIT 1",
            )
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        let mut rows = stmt
            .query(params![sub])
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        if let Some(row) = rows
            .next()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?
        {
            let sub_val: String = row
                .get(0)
                .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
            let email: String = row
                .get(1)
                .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
            let name: String = row
                .get(2)
                .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
            let picture: Option<String> = row
                .get(3)
                .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
            let tenant_id: String = row
                .get(4)
                .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
            let workspace_id: String = row
                .get(5)
                .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
            let active_int: i64 = row
                .get(6)
                .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
            let created_at: i64 = row
                .get(7)
                .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
            let updated_at: i64 = row
                .get(8)
                .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
            Ok(Some(crate::profiles::GoogleAccountRecord {
                sub: sub_val,
                email,
                display_name: name,
                picture,
                tenant_id,
                workspace_id,
                active: active_int == 1,
                created_at,
                updated_at,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn get_active_google_account(
        &self,
    ) -> Result<Option<crate::profiles::GoogleAccountRecord>, CheckpointError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        let mut stmt = conn
            .prepare(
                "SELECT sub, email, name, picture, tenant_id, workspace_id, active, created_at, updated_at
                 FROM profiles WHERE active = 1 AND sub IS NOT NULL ORDER BY updated_at DESC LIMIT 1",
            )
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        let mut rows = stmt
            .query([])
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        if let Some(row) = rows
            .next()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?
        {
            let sub_val: String = row
                .get(0)
                .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
            let email: String = row
                .get(1)
                .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
            let name: String = row
                .get(2)
                .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
            let picture: Option<String> = row
                .get(3)
                .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
            let tenant_id: String = row
                .get(4)
                .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
            let workspace_id: String = row
                .get(5)
                .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
            let active_int: i64 = row
                .get(6)
                .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
            let created_at: i64 = row
                .get(7)
                .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
            let updated_at: i64 = row
                .get(8)
                .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
            Ok(Some(crate::profiles::GoogleAccountRecord {
                sub: sub_val,
                email,
                display_name: name,
                picture,
                tenant_id,
                workspace_id,
                active: active_int == 1,
                created_at,
                updated_at,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn set_active_google_account(&self, sub: &str) -> Result<(), CheckpointError> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        let tx = conn
            .transaction()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        tx.execute("UPDATE profiles SET active = 0", [])
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        let now = Utc::now().timestamp();
        let changed = tx
            .execute(
                "UPDATE profiles SET active = 1, updated_at = ?2 WHERE sub = ?1",
                params![sub, now],
            )
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        if changed == 0 {
            return Err(CheckpointError::NotFound(format!(
                "Account {sub} not found"
            )));
        }
        tx.commit()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        Ok(())
    }

    pub fn deactivate_all_google_accounts(&self) -> Result<(), CheckpointError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        conn.execute("UPDATE profiles SET active = 0", [])
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        Ok(())
    }

    pub fn is_legacy_data_claimed(&self) -> Result<Option<String>, CheckpointError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        let mut stmt = conn
            .prepare("SELECT claimed_by_sub FROM account_legacy_claim WHERE id = 1 LIMIT 1")
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        let mut rows = stmt
            .query([])
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        if let Some(row) = rows
            .next()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?
        {
            let claimed_by: String = row
                .get(0)
                .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
            Ok(Some(claimed_by))
        } else {
            Ok(None)
        }
    }

    pub fn claim_legacy_data(&self, sub: &str) -> Result<bool, CheckpointError> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        let tx = conn
            .transaction()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        let mut check_stmt = tx
            .prepare("SELECT claimed_by_sub FROM account_legacy_claim WHERE id = 1")
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        let mut rows = check_stmt
            .query([])
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        if rows
            .next()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?
            .is_some()
        {
            return Ok(false);
        }
        drop(rows);
        drop(check_stmt);

        let now = Utc::now().timestamp();
        tx.execute(
            "INSERT INTO account_legacy_claim (id, claimed_by_sub, claimed_at) VALUES (1, ?1, ?2)",
            params![sub, now],
        )
        .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        tx.execute(
            "UPDATE profiles SET tenant_id = '00000000-0000-0000-0000-000000000001', updated_at = ?2 WHERE sub = ?1",
            params![sub, now],
        )
        .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        tx.commit()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        Ok(true)
    }
}

impl CheckpointStore for SqliteCheckpointStore {
    async fn save_checkpoint(&self, checkpoint: &RunCheckpoint) -> Result<(), CheckpointError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        let state_json = serialize_checkpoint_state(checkpoint)?;

        conn.execute(
            "INSERT INTO checkpoints (id, run_id, tenant_id, workspace_id, sequence, status, state_json, author_id, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                checkpoint.id.to_string(),
                checkpoint.scope.run_id.to_string(),
                checkpoint.scope.tenant_id.to_string(),
                checkpoint.scope.workspace_id.to_string(),
                checkpoint.sequence,
                checkpoint.status.as_str(),
                state_json,
                checkpoint.author_id,
                checkpoint.created_at.to_rfc3339(),
            ],
        )
        .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    async fn get_latest_checkpoint(
        &self,
        scope: &ExecutionScope,
    ) -> Result<Option<RunCheckpoint>, CheckpointError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        let mut stmt = conn
            .prepare(
                "SELECT id, sequence, status, state_json, created_at
                 FROM checkpoints
                 WHERE run_id = ?1 AND tenant_id = ?2 AND workspace_id = ?3
                 ORDER BY sequence DESC
                 LIMIT 1",
            )
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        let row = stmt
            .query_row(
                params![
                    scope.run_id.to_string(),
                    scope.tenant_id.to_string(),
                    scope.workspace_id.to_string(),
                ],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, u32>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                    ))
                },
            )
            .optional()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        let Some((id_str, sequence, status_str, state_json, created_at_str)) = row else {
            return Ok(None);
        };

        Ok(Some(build_checkpoint_from_row(
            *scope,
            id_str,
            sequence,
            status_str,
            state_json,
            created_at_str,
        )?))
    }

    async fn list_checkpoints(
        &self,
        scope: &ExecutionScope,
    ) -> Result<Vec<RunCheckpoint>, CheckpointError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        let mut stmt = conn
            .prepare(
                "SELECT id, sequence, status, state_json, created_at
                 FROM checkpoints
                 WHERE run_id = ?1 AND tenant_id = ?2 AND workspace_id = ?3
                 ORDER BY sequence ASC",
            )
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        let rows = stmt
            .query_map(
                params![
                    scope.run_id.to_string(),
                    scope.tenant_id.to_string(),
                    scope.workspace_id.to_string(),
                ],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, u32>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                    ))
                },
            )
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        let mut list = Vec::new();
        for r in rows {
            let (id_str, sequence, status_str, state_json, created_at_str) =
                r.map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
            list.push(build_checkpoint_from_row(
                *scope,
                id_str,
                sequence,
                status_str,
                state_json,
                created_at_str,
            )?);
        }

        Ok(list)
    }

    async fn save_approval(&self, approval: &ApprovalRequest) -> Result<(), CheckpointError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        conn.execute(
            "INSERT INTO approvals (id, run_id, tenant_id, workspace_id, tool_name, action_description, intent_kind, status, author_id, created_at, decided_at, decided_by, consumed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                approval.id.to_string(),
                approval.scope.run_id.to_string(),
                approval.scope.tenant_id.to_string(),
                approval.scope.workspace_id.to_string(),
                approval.tool_name,
                approval.action_description,
                approval.intent.as_str(),
                approval.status.as_str(),
                approval.author_id,
                approval.created_at.to_rfc3339(),
                approval.decided_at.map(|d| d.to_rfc3339()),
                approval.decided_by,
                approval.consumed_at.map(|d| d.to_rfc3339()),
            ],
        )
        .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    async fn get_approval(
        &self,
        scope: &ExecutionScope,
        approval_id: Uuid,
    ) -> Result<Option<ApprovalRequest>, CheckpointError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        let mut stmt = conn
            .prepare(
                "SELECT id, tool_name, action_description, intent_kind, status, author_id, created_at, decided_at, decided_by, consumed_at
                 FROM approvals
                 WHERE id = ?1 AND run_id = ?2 AND tenant_id = ?3 AND workspace_id = ?4",
            )
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        let row = stmt
            .query_row(
                params![
                    approval_id.to_string(),
                    scope.run_id.to_string(),
                    scope.tenant_id.to_string(),
                    scope.workspace_id.to_string(),
                ],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, String>(3)?,
                        r.get::<_, String>(4)?,
                        r.get::<_, Option<String>>(5)?,
                        r.get::<_, String>(6)?,
                        r.get::<_, Option<String>>(7)?,
                        r.get::<_, Option<String>>(8)?,
                        r.get::<_, Option<String>>(9)?,
                    ))
                },
            )
            .optional()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        let Some((
            id_str,
            tool_name,
            action_description,
            intent_str,
            status_str,
            author_id,
            created_str,
            decided_str,
            decided_by,
            consumed_str,
        )) = row
        else {
            return Ok(None);
        };

        Ok(Some(build_approval_from_row(
            *scope,
            id_str,
            tool_name,
            action_description,
            intent_str,
            status_str,
            author_id,
            created_str,
            decided_str,
            decided_by,
            consumed_str,
        )?))
    }

    async fn get_pending_approval(
        &self,
        scope: &ExecutionScope,
    ) -> Result<Option<ApprovalRequest>, CheckpointError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        let mut stmt = conn
            .prepare(
                "SELECT id, tool_name, action_description, intent_kind, status, author_id, created_at, decided_at, decided_by, consumed_at
                 FROM approvals
                 WHERE run_id = ?1 AND tenant_id = ?2 AND workspace_id = ?3 AND status = 'pending'
                 ORDER BY created_at DESC
                 LIMIT 1",
            )
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        let row = stmt
            .query_row(
                params![
                    scope.run_id.to_string(),
                    scope.tenant_id.to_string(),
                    scope.workspace_id.to_string(),
                ],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, String>(3)?,
                        r.get::<_, String>(4)?,
                        r.get::<_, Option<String>>(5)?,
                        r.get::<_, String>(6)?,
                        r.get::<_, Option<String>>(7)?,
                        r.get::<_, Option<String>>(8)?,
                        r.get::<_, Option<String>>(9)?,
                    ))
                },
            )
            .optional()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        let Some((
            id_str,
            tool_name,
            action_description,
            intent_str,
            status_str,
            author_id,
            created_str,
            decided_str,
            decided_by,
            consumed_str,
        )) = row
        else {
            return Ok(None);
        };

        Ok(Some(build_approval_from_row(
            *scope,
            id_str,
            tool_name,
            action_description,
            intent_str,
            status_str,
            author_id,
            created_str,
            decided_str,
            decided_by,
            consumed_str,
        )?))
    }

    async fn resolve_approval(
        &self,
        scope: &ExecutionScope,
        approval_id: Uuid,
        decision: ApprovalDecision,
        actor: &str,
    ) -> Result<(), CheckpointError> {
        let trimmed_actor = actor.trim();
        if trimmed_actor.is_empty() {
            return Err(CheckpointError::InvalidState(
                "Approver identity (actor) is required and cannot be empty".to_string(),
            ));
        }

        let mut conn = self
            .conn
            .lock()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        let existing: Option<(String, Option<String>)> = conn
            .query_row(
                "SELECT status, author_id
                 FROM approvals
                 WHERE id = ?1 AND run_id = ?2 AND tenant_id = ?3 AND workspace_id = ?4",
                params![
                    approval_id.to_string(),
                    scope.run_id.to_string(),
                    scope.tenant_id.to_string(),
                    scope.workspace_id.to_string(),
                ],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        let Some((existing_status, author_id)) = existing else {
            return Err(CheckpointError::InvalidState(
                "Approval request not found, scope mismatched, or already decided".to_string(),
            ));
        };

        if existing_status != "pending" {
            let now_str = Utc::now().to_rfc3339();
            let reject_details = serde_json::json!({
                "approver_id": trimmed_actor,
                "existing_status": existing_status,
                "reason": "approval_already_decided",
            })
            .to_string();
            let _ = conn.execute(
                "INSERT INTO audit_logs (id, tenant_id, workspace_id, run_id, actor_id, action, target_id, details_json, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    Uuid::new_v4().to_string(),
                    scope.tenant_id.to_string(),
                    scope.workspace_id.to_string(),
                    scope.run_id.to_string(),
                    trimmed_actor,
                    "approval.already_decided_rejected",
                    approval_id.to_string(),
                    reject_details,
                    now_str,
                ],
            );
            return Err(CheckpointError::InvalidState(
                "Approval request not found, scope mismatched, or already decided".to_string(),
            ));
        }

        if decision == ApprovalDecision::AllowOnce {
            if let Some(ref author) = author_id {
                if author.trim() == trimmed_actor {
                    let now_str = Utc::now().to_rfc3339();
                    let reject_details = serde_json::json!({
                        "author_id": author,
                        "approver_id": trimmed_actor,
                        "reason": "author_id_equals_approver_id",
                    })
                    .to_string();
                    let _ = conn.execute(
                        "INSERT INTO audit_logs (id, tenant_id, workspace_id, run_id, actor_id, action, target_id, details_json, created_at)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                        params![
                            Uuid::new_v4().to_string(),
                            scope.tenant_id.to_string(),
                            scope.workspace_id.to_string(),
                            scope.run_id.to_string(),
                            trimmed_actor,
                            "approval.self_approval_rejected",
                            approval_id.to_string(),
                            reject_details,
                            now_str,
                        ],
                    );
                    return Err(CheckpointError::InvalidState(format!(
                        "Separation of duties violation: approver '{}' cannot approve action authored by '{}'",
                        trimmed_actor, author
                    )));
                }
            }
        }

        let target_status = match decision {
            ApprovalDecision::AllowOnce => ApprovalStatus::AllowedOnce,
            ApprovalDecision::Deny => ApprovalStatus::Denied,
        };

        let now_str = Utc::now().to_rfc3339();
        let tx = conn
            .transaction()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        let updated = tx
            .execute(
                "UPDATE approvals
                 SET status = ?1, decided_at = ?2, decided_by = ?3
                 WHERE id = ?4 AND run_id = ?5 AND tenant_id = ?6 AND workspace_id = ?7 AND status = 'pending'",
                params![
                    target_status.as_str(),
                    now_str,
                    trimmed_actor,
                    approval_id.to_string(),
                    scope.run_id.to_string(),
                    scope.tenant_id.to_string(),
                    scope.workspace_id.to_string(),
                ],
            )
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        if updated == 0 {
            return Err(CheckpointError::InvalidState(
                "Approval request not found, scope mismatched, or already decided".to_string(),
            ));
        }

        let details = serde_json::json!({
            "decision": decision.as_str(),
            "actor": trimmed_actor,
            "approver_id": trimmed_actor,
            "author_id": author_id,
        });
        let details_json = serde_json::to_string(&details)
            .map_err(|e| CheckpointError::SerializationError(e.to_string()))?;

        tx.execute(
            "INSERT INTO audit_logs (id, tenant_id, workspace_id, run_id, actor_id, action, target_id, details_json, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                Uuid::new_v4().to_string(),
                scope.tenant_id.to_string(),
                scope.workspace_id.to_string(),
                scope.run_id.to_string(),
                trimmed_actor,
                "approval.resolved",
                approval_id.to_string(),
                details_json,
                now_str,
            ],
        )
        .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        tx.commit()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    async fn consume_approval(
        &self,
        scope: &ExecutionScope,
        approval_id: Uuid,
    ) -> Result<bool, CheckpointError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        let now_str = Utc::now().to_rfc3339();

        let updated = conn
            .execute(
                "UPDATE approvals
                 SET status = 'consumed', consumed_at = ?1
                 WHERE id = ?2 AND run_id = ?3 AND tenant_id = ?4 AND workspace_id = ?5 AND status = 'allowed_once' AND consumed_at IS NULL",
                params![
                    now_str,
                    approval_id.to_string(),
                    scope.run_id.to_string(),
                    scope.tenant_id.to_string(),
                    scope.workspace_id.to_string(),
                ],
            )
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        Ok(updated > 0)
    }

    async fn claim_approved_execution(
        &self,
        scope: &ExecutionScope,
        approval_id: Uuid,
        executing_checkpoint: &RunCheckpoint,
    ) -> Result<bool, CheckpointError> {
        if executing_checkpoint.status != RunStatus::Executing {
            return Err(CheckpointError::InvalidState(
                "Claim checkpoint must have status 'executing'".to_string(),
            ));
        }

        let state_json = serialize_checkpoint_state(executing_checkpoint)?;
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        let updated = tx
            .execute(
                "UPDATE approvals
                 SET status = 'executing'
                 WHERE id = ?1 AND run_id = ?2 AND tenant_id = ?3 AND workspace_id = ?4 AND status = 'allowed_once' AND consumed_at IS NULL",
                params![
                    approval_id.to_string(),
                    scope.run_id.to_string(),
                    scope.tenant_id.to_string(),
                    scope.workspace_id.to_string(),
                ],
            )
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        if updated == 0 {
            return Ok(false);
        }

        tx.execute(
            "INSERT INTO checkpoints (id, run_id, tenant_id, workspace_id, sequence, status, state_json, author_id, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                executing_checkpoint.id.to_string(),
                executing_checkpoint.scope.run_id.to_string(),
                executing_checkpoint.scope.tenant_id.to_string(),
                executing_checkpoint.scope.workspace_id.to_string(),
                executing_checkpoint.sequence,
                executing_checkpoint.status.as_str(),
                state_json,
                executing_checkpoint.author_id,
                executing_checkpoint.created_at.to_rfc3339(),
            ],
        )
        .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        let now_str = Utc::now().to_rfc3339();
        let details_json = serde_json::json!({
            "approval_id": approval_id.to_string(),
            "sequence": executing_checkpoint.sequence,
        })
        .to_string();

        tx.execute(
            "INSERT INTO audit_logs (id, tenant_id, workspace_id, run_id, actor_id, action, target_id, details_json, created_at)
             VALUES (?1, ?2, ?3, ?4, NULL, ?5, ?6, ?7, ?8)",
            params![
                Uuid::new_v4().to_string(),
                scope.tenant_id.to_string(),
                scope.workspace_id.to_string(),
                scope.run_id.to_string(),
                "approval.executing",
                approval_id.to_string(),
                details_json,
                now_str,
            ],
        )
        .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        tx.commit()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        Ok(true)
    }

    async fn complete_approval_execution(
        &self,
        scope: &ExecutionScope,
        approval_id: Uuid,
    ) -> Result<bool, CheckpointError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        let now_str = Utc::now().to_rfc3339();

        let updated = conn
            .execute(
                "UPDATE approvals
                 SET status = 'consumed', consumed_at = ?1
                 WHERE id = ?2 AND run_id = ?3 AND tenant_id = ?4 AND workspace_id = ?5 AND status = 'executing' AND consumed_at IS NULL",
                params![
                    now_str,
                    approval_id.to_string(),
                    scope.run_id.to_string(),
                    scope.tenant_id.to_string(),
                    scope.workspace_id.to_string(),
                ],
            )
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        Ok(updated > 0)
    }

    async fn record_audit_log(&self, entry: &AuditLogEntry) -> Result<(), CheckpointError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        let details_json = serde_json::to_string(&entry.details)
            .map_err(|e| CheckpointError::SerializationError(e.to_string()))?;

        conn.execute(
            "INSERT INTO audit_logs (id, tenant_id, workspace_id, run_id, actor_id, action, target_id, details_json, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                entry.id.to_string(),
                entry.scope.tenant_id.to_string(),
                entry.scope.workspace_id.to_string(),
                entry.scope.run_id.to_string(),
                entry.actor_id,
                entry.action,
                entry.target_id.map(|t| t.to_string()),
                details_json,
                entry.created_at.to_rfc3339(),
            ],
        )
        .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    async fn list_audit_logs(
        &self,
        scope: &ExecutionScope,
    ) -> Result<Vec<AuditLogEntry>, CheckpointError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        let mut stmt = conn
            .prepare(
                "SELECT id, actor_id, action, target_id, details_json, created_at
                 FROM audit_logs
                 WHERE run_id = ?1 AND tenant_id = ?2 AND workspace_id = ?3
                 ORDER BY created_at ASC",
            )
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        let rows = stmt
            .query_map(
                params![
                    scope.run_id.to_string(),
                    scope.tenant_id.to_string(),
                    scope.workspace_id.to_string(),
                ],
                |r| {
                    let id_str: String = r.get(0)?;
                    let actor_id: Option<String> = r.get(1)?;
                    let action: String = r.get(2)?;
                    let target_id_str: Option<String> = r.get(3)?;
                    let details_json: String = r.get(4)?;
                    let created_str: String = r.get(5)?;
                    Ok((
                        id_str,
                        actor_id,
                        action,
                        target_id_str,
                        details_json,
                        created_str,
                    ))
                },
            )
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        let mut list = Vec::new();
        for r in rows {
            let (id_str, actor_id, action, target_id_str, details_json, created_str) =
                r.map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
            let id = Uuid::parse_str(&id_str)
                .map_err(|e| CheckpointError::SerializationError(e.to_string()))?;
            let target_id = match target_id_str {
                Some(s) => Some(
                    Uuid::parse_str(&s)
                        .map_err(|e| CheckpointError::SerializationError(e.to_string()))?,
                ),
                None => None,
            };
            let details: serde_json::Value = serde_json::from_str(&details_json)
                .map_err(|e| CheckpointError::SerializationError(e.to_string()))?;
            let created_at = DateTime::parse_from_rfc3339(&created_str)
                .map_err(|e| CheckpointError::SerializationError(e.to_string()))?
                .with_timezone(&Utc);

            list.push(AuditLogEntry {
                id,
                scope: *scope,
                actor_id,
                action,
                target_id,
                details,
                created_at,
            });
        }

        Ok(list)
    }

    async fn request_cancellation(&self, scope: &ExecutionScope) -> Result<(), CheckpointError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        conn.execute(
            "INSERT OR REPLACE INTO cancellations (run_id, tenant_id, workspace_id, requested_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                scope.run_id.to_string(),
                scope.tenant_id.to_string(),
                scope.workspace_id.to_string(),
                Utc::now().to_rfc3339(),
            ],
        )
        .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    async fn is_cancelled(&self, scope: &ExecutionScope) -> Result<bool, CheckpointError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        let mut stmt = conn
            .prepare(
                "SELECT 1 FROM cancellations
                 WHERE run_id = ?1 AND tenant_id = ?2 AND workspace_id = ?3",
            )
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        let exists = stmt
            .exists(params![
                scope.run_id.to_string(),
                scope.tenant_id.to_string(),
                scope.workspace_id.to_string(),
            ])
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        Ok(exists)
    }

    async fn get_checkpoint_at_sequence(
        &self,
        scope: &ExecutionScope,
        sequence: u32,
    ) -> Result<Option<RunCheckpoint>, CheckpointError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        let mut stmt = conn
            .prepare(
                "SELECT id, sequence, status, state_json, created_at
                 FROM checkpoints
                 WHERE run_id = ?1 AND tenant_id = ?2 AND workspace_id = ?3 AND sequence = ?4
                 LIMIT 1",
            )
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        let row = stmt
            .query_row(
                params![
                    scope.run_id.to_string(),
                    scope.tenant_id.to_string(),
                    scope.workspace_id.to_string(),
                    sequence,
                ],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, u32>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                    ))
                },
            )
            .optional()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        let Some((id_str, seq, status_str, state_json, created_at_str)) = row else {
            return Ok(None);
        };

        Ok(Some(build_checkpoint_from_row(
            *scope,
            id_str,
            seq,
            status_str,
            state_json,
            created_at_str,
        )?))
    }

    async fn fork_run(
        &self,
        req: &ForkRunRequest,
    ) -> Result<(RunCheckpoint, TrajectoryBranch), CheckpointError> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        let source_row = tx
            .query_row(
                "SELECT id, sequence, status, state_json, created_at
                 FROM checkpoints
                 WHERE run_id = ?1 AND tenant_id = ?2 AND workspace_id = ?3 AND sequence = ?4
                 LIMIT 1",
                params![
                    req.source_scope.run_id.to_string(),
                    req.source_scope.tenant_id.to_string(),
                    req.source_scope.workspace_id.to_string(),
                    req.target_sequence,
                ],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, u32>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                    ))
                },
            )
            .optional()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?
            .ok_or_else(|| {
                CheckpointError::NotFound(format!(
                    "Checkpoint at sequence {} not found for run {} in scope",
                    req.target_sequence, req.source_scope.run_id
                ))
            })?;

        let source_cp = build_checkpoint_from_row(
            req.source_scope,
            source_row.0,
            source_row.1,
            source_row.2,
            source_row.3,
            source_row.4,
        )?;

        let new_run_id = req.new_run_id.unwrap_or_else(Uuid::new_v4);
        let forked_scope = ExecutionScope::new(
            req.source_scope.tenant_id,
            req.source_scope.workspace_id,
            new_run_id,
        )
        .map_err(|e| CheckpointError::InvalidState(e.to_string()))?;

        let mut forked_history = source_cp.history.clone();
        let mut forked_pending_tools = source_cp.pending_tool_calls.clone();

        if let Some(ref prompt_override) = req.prompt_override {
            let trimmed = prompt_override.trim();
            if !trimmed.is_empty() {
                if forked_history
                    .last()
                    .is_some_and(|m| m.role == MessageRole::User)
                {
                    if let Some(last) = forked_history.last_mut() {
                        last.content = trimmed.to_string();
                    }
                } else {
                    forked_history.push(ChatMessage::user(trimmed));
                }
                forked_pending_tools.clear();
            }
        } else if let Some(last) = forked_history.last() {
            if last.role != MessageRole::Assistant
                || last
                    .tool_calls
                    .as_ref()
                    .is_none_or(|calls| calls.is_empty())
            {
                forked_pending_tools.clear();
            }
        }

        let branch_id = Uuid::new_v4();
        let now = Utc::now();
        let author_id = req.author_id.clone().or(source_cp.author_id.clone());
        let model_override = req
            .model_override
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .or_else(|| source_cp.model_override.clone());

        let forked_cp = RunCheckpoint {
            id: Uuid::new_v4(),
            scope: forked_scope,
            sequence: 1,
            status: RunStatus::Queued,
            history: forked_history,
            active_tool_call: None,
            pending_tool_calls: forked_pending_tools,
            pending_approval_id: None,
            author_id: author_id.clone(),
            model_override,
            created_at: now,
        };

        let forked_state_json = serialize_checkpoint_state(&forked_cp)?;
        tx.execute(
            "INSERT INTO checkpoints (id, run_id, tenant_id, workspace_id, sequence, status, state_json, author_id, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                forked_cp.id.to_string(),
                forked_scope.run_id.to_string(),
                forked_scope.tenant_id.to_string(),
                forked_scope.workspace_id.to_string(),
                forked_cp.sequence,
                forked_cp.status.as_str(),
                forked_state_json,
                forked_cp.author_id,
                forked_cp.created_at.to_rfc3339(),
            ],
        )
        .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        let branch = TrajectoryBranch {
            branch_id,
            tenant_id: forked_scope.tenant_id,
            workspace_id: forked_scope.workspace_id,
            parent_run_id: req.source_scope.run_id,
            forked_run_id: new_run_id,
            fork_sequence: req.target_sequence,
            branch_label: req.branch_label.clone(),
            model_override: req.model_override.clone(),
            prompt_override: req.prompt_override.clone(),
            created_at: now,
        };

        tx.execute(
            "INSERT INTO trajectory_branches (branch_id, tenant_id, workspace_id, parent_run_id, forked_run_id, fork_sequence, branch_label, model_override, prompt_override, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                branch.branch_id.to_string(),
                branch.tenant_id.to_string(),
                branch.workspace_id.to_string(),
                branch.parent_run_id.to_string(),
                branch.forked_run_id.to_string(),
                branch.fork_sequence,
                branch.branch_label,
                branch.model_override,
                branch.prompt_override,
                branch.created_at.to_rfc3339(),
            ],
        )
        .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        let audit_details = serde_json::json!({
            "branch_id": branch.branch_id.to_string(),
            "parent_run_id": req.source_scope.run_id.to_string(),
            "forked_run_id": new_run_id.to_string(),
            "fork_sequence": req.target_sequence,
            "branch_label": req.branch_label,
            "model_override": req.model_override,
        })
        .to_string();

        tx.execute(
            "INSERT INTO audit_logs (id, tenant_id, workspace_id, run_id, actor_id, action, target_id, details_json, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                Uuid::new_v4().to_string(),
                forked_scope.tenant_id.to_string(),
                forked_scope.workspace_id.to_string(),
                forked_scope.run_id.to_string(),
                author_id,
                "trajectory.forked",
                branch.branch_id.to_string(),
                audit_details,
                now.to_rfc3339(),
            ],
        )
        .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        tx.commit()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        Ok((forked_cp, branch))
    }

    async fn list_branches(
        &self,
        scope: &ExecutionScope,
    ) -> Result<Vec<TrajectoryBranch>, CheckpointError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        let mut stmt = conn
            .prepare(
                "SELECT branch_id, tenant_id, workspace_id, parent_run_id, forked_run_id, fork_sequence, branch_label, model_override, prompt_override, created_at
                 FROM trajectory_branches
                 WHERE tenant_id = ?1 AND workspace_id = ?2 AND (parent_run_id = ?3 OR forked_run_id = ?3)
                 ORDER BY created_at ASC",
            )
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        let rows = stmt
            .query_map(
                params![
                    scope.tenant_id.to_string(),
                    scope.workspace_id.to_string(),
                    scope.run_id.to_string(),
                ],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, String>(3)?,
                        r.get::<_, String>(4)?,
                        r.get::<_, u32>(5)?,
                        r.get::<_, String>(6)?,
                        r.get::<_, Option<String>>(7)?,
                        r.get::<_, Option<String>>(8)?,
                        r.get::<_, String>(9)?,
                    ))
                },
            )
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        let mut out = Vec::new();
        for r in rows {
            let (
                branch_id_s,
                tenant_id_s,
                workspace_id_s,
                parent_run_id_s,
                forked_run_id_s,
                fork_sequence,
                branch_label,
                model_override,
                prompt_override,
                created_at_s,
            ) = r.map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
            out.push(TrajectoryBranch {
                branch_id: Uuid::parse_str(&branch_id_s)
                    .map_err(|e| CheckpointError::SerializationError(e.to_string()))?,
                tenant_id: Uuid::parse_str(&tenant_id_s)
                    .map_err(|e| CheckpointError::SerializationError(e.to_string()))?,
                workspace_id: Uuid::parse_str(&workspace_id_s)
                    .map_err(|e| CheckpointError::SerializationError(e.to_string()))?,
                parent_run_id: Uuid::parse_str(&parent_run_id_s)
                    .map_err(|e| CheckpointError::SerializationError(e.to_string()))?,
                forked_run_id: Uuid::parse_str(&forked_run_id_s)
                    .map_err(|e| CheckpointError::SerializationError(e.to_string()))?,
                fork_sequence,
                branch_label,
                model_override,
                prompt_override,
                created_at: DateTime::parse_from_rfc3339(&created_at_s)
                    .map_err(|e| CheckpointError::SerializationError(e.to_string()))?
                    .with_timezone(&Utc),
            });
        }
        Ok(out)
    }

    async fn save_workflow_step(
        &self,
        step: &WorkflowStepCheckpoint,
    ) -> Result<(), CheckpointError> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        let state_json = serde_json::to_string(&step.state_json)
            .map_err(|e| CheckpointError::SerializationError(e.to_string()))?;

        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        tx.execute(
            "INSERT INTO workflow_checkpoints (id, run_id, tenant_id, workspace_id, sequence, node_id, status, state_json, pending_approval_id, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                step.id.to_string(),
                step.scope.run_id.to_string(),
                step.scope.tenant_id.to_string(),
                step.scope.workspace_id.to_string(),
                step.sequence,
                step.node_id,
                step.status.as_str(),
                state_json,
                step.pending_approval_id.map(|id| id.to_string()),
                step.created_at.to_rfc3339(),
            ],
        )
        .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        tx.commit()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    async fn get_latest_workflow_step(
        &self,
        scope: &ExecutionScope,
    ) -> Result<Option<WorkflowStepCheckpoint>, CheckpointError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        let mut stmt = conn
            .prepare(
                "SELECT id, sequence, node_id, status, state_json, pending_approval_id, created_at
                 FROM workflow_checkpoints
                 WHERE run_id = ?1 AND tenant_id = ?2 AND workspace_id = ?3
                 ORDER BY sequence DESC
                 LIMIT 1",
            )
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        let row = stmt
            .query_row(
                params![
                    scope.run_id.to_string(),
                    scope.tenant_id.to_string(),
                    scope.workspace_id.to_string(),
                ],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, u32>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, String>(3)?,
                        r.get::<_, String>(4)?,
                        r.get::<_, Option<String>>(5)?,
                        r.get::<_, String>(6)?,
                    ))
                },
            )
            .optional()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        let Some((id_s, sequence, node_id, status_s, state_s, pending_s, created_s)) = row else {
            return Ok(None);
        };

        Ok(Some(WorkflowStepCheckpoint {
            id: Uuid::parse_str(&id_s)
                .map_err(|e| CheckpointError::SerializationError(e.to_string()))?,
            scope: *scope,
            sequence,
            node_id,
            status: RunStatus::from_str(&status_s)
                .map_err(|e| CheckpointError::SerializationError(e.to_string()))?,
            state_json: serde_json::from_str(&state_s)
                .map_err(|e| CheckpointError::SerializationError(e.to_string()))?,
            pending_approval_id: match pending_s {
                Some(p) => Some(
                    Uuid::parse_str(&p)
                        .map_err(|e| CheckpointError::SerializationError(e.to_string()))?,
                ),
                None => None,
            },
            created_at: DateTime::parse_from_rfc3339(&created_s)
                .map_err(|e| CheckpointError::SerializationError(e.to_string()))?
                .with_timezone(&Utc),
        }))
    }

    async fn list_workflow_steps(
        &self,
        scope: &ExecutionScope,
    ) -> Result<Vec<WorkflowStepCheckpoint>, CheckpointError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        let mut stmt = conn
            .prepare(
                "SELECT id, sequence, node_id, status, state_json, pending_approval_id, created_at
                 FROM workflow_checkpoints
                 WHERE run_id = ?1 AND tenant_id = ?2 AND workspace_id = ?3
                 ORDER BY sequence ASC",
            )
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        let rows = stmt
            .query_map(
                params![
                    scope.run_id.to_string(),
                    scope.tenant_id.to_string(),
                    scope.workspace_id.to_string(),
                ],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, u32>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, String>(3)?,
                        r.get::<_, String>(4)?,
                        r.get::<_, Option<String>>(5)?,
                        r.get::<_, String>(6)?,
                    ))
                },
            )
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        let mut list = Vec::new();
        for r in rows {
            let (id_s, sequence, node_id, status_s, state_s, pending_s, created_s) =
                r.map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
            list.push(WorkflowStepCheckpoint {
                id: Uuid::parse_str(&id_s)
                    .map_err(|e| CheckpointError::SerializationError(e.to_string()))?,
                scope: *scope,
                sequence,
                node_id,
                status: RunStatus::from_str(&status_s)
                    .map_err(|e| CheckpointError::SerializationError(e.to_string()))?,
                state_json: serde_json::from_str(&state_s)
                    .map_err(|e| CheckpointError::SerializationError(e.to_string()))?,
                pending_approval_id: match pending_s {
                    Some(p) => Some(
                        Uuid::parse_str(&p)
                            .map_err(|e| CheckpointError::SerializationError(e.to_string()))?,
                    ),
                    None => None,
                },
                created_at: DateTime::parse_from_rfc3339(&created_s)
                    .map_err(|e| CheckpointError::SerializationError(e.to_string()))?
                    .with_timezone(&Utc),
            });
        }
        Ok(list)
    }

    async fn claim_workflow_approval(
        &self,
        scope: &ExecutionScope,
        approval_id: Uuid,
        executing_step: &WorkflowStepCheckpoint,
    ) -> Result<bool, CheckpointError> {
        if executing_step.status != RunStatus::Executing {
            return Err(CheckpointError::InvalidState(
                "Claim workflow checkpoint must have status 'executing'".to_string(),
            ));
        }

        let state_json = serde_json::to_string(&executing_step.state_json)
            .map_err(|e| CheckpointError::SerializationError(e.to_string()))?;
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        let updated = tx
            .execute(
                "UPDATE approvals
                 SET status = 'executing'
                 WHERE id = ?1 AND run_id = ?2 AND tenant_id = ?3 AND workspace_id = ?4 AND status = 'allowed_once' AND consumed_at IS NULL",
                params![
                    approval_id.to_string(),
                    scope.run_id.to_string(),
                    scope.tenant_id.to_string(),
                    scope.workspace_id.to_string(),
                ],
            )
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        if updated == 0 {
            return Ok(false);
        }

        tx.execute(
            "INSERT INTO workflow_checkpoints (id, run_id, tenant_id, workspace_id, sequence, node_id, status, state_json, pending_approval_id, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                executing_step.id.to_string(),
                executing_step.scope.run_id.to_string(),
                executing_step.scope.tenant_id.to_string(),
                executing_step.scope.workspace_id.to_string(),
                executing_step.sequence,
                executing_step.node_id,
                executing_step.status.as_str(),
                state_json,
                executing_step.pending_approval_id.map(|id| id.to_string()),
                executing_step.created_at.to_rfc3339(),
            ],
        )
        .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        tx.commit()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        Ok(true)
    }

    async fn record_tool_call(
        &self,
        scope: &ExecutionScope,
        node_id: &str,
        tool_call_id: &str,
        tool_name: &str,
        arguments: &serde_json::Value,
    ) -> Result<(), CheckpointError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        let now = Utc::now().to_rfc3339();
        let args_json = serde_json::to_string(arguments)
            .map_err(|e| CheckpointError::SerializationError(e.to_string()))?;

        conn.execute(
            "INSERT INTO tool_journal (run_id, tenant_id, workspace_id, node_id, tool_call_id, tool_name, arguments_json, status, output_text, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'executing', NULL, ?8, ?9)
             ON CONFLICT(run_id, node_id, tool_call_id) DO UPDATE SET status = 'executing', updated_at = ?9",
            params![
                scope.run_id.to_string(),
                scope.tenant_id.to_string(),
                scope.workspace_id.to_string(),
                node_id,
                tool_call_id,
                tool_name,
                args_json,
                now,
                now,
            ],
        )
        .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        Ok(())
    }

    async fn get_tool_call_status(
        &self,
        scope: &ExecutionScope,
        node_id: &str,
        tool_call_id: &str,
    ) -> Result<Option<(String, Option<String>)>, CheckpointError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        let mut stmt = conn
            .prepare(
                "SELECT status, output_text FROM tool_journal
                 WHERE run_id = ?1 AND node_id = ?2 AND tool_call_id = ?3 AND tenant_id = ?4 AND workspace_id = ?5",
            )
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        let mut rows = stmt
            .query(params![
                scope.run_id.to_string(),
                node_id,
                tool_call_id,
                scope.tenant_id.to_string(),
                scope.workspace_id.to_string(),
            ])
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        if let Some(row) = rows
            .next()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?
        {
            let status: String = row
                .get(0)
                .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
            let output: Option<String> = row
                .get(1)
                .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
            Ok(Some((status, output)))
        } else {
            Ok(None)
        }
    }

    async fn complete_tool_call(
        &self,
        scope: &ExecutionScope,
        node_id: &str,
        tool_call_id: &str,
        output: &str,
    ) -> Result<(), CheckpointError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        let now = Utc::now().to_rfc3339();

        conn.execute(
            "UPDATE tool_journal
             SET status = 'consumed', output_text = ?1, updated_at = ?2
             WHERE run_id = ?3 AND node_id = ?4 AND tool_call_id = ?5 AND tenant_id = ?6 AND workspace_id = ?7",
            params![
                output,
                now,
                scope.run_id.to_string(),
                node_id,
                tool_call_id,
                scope.tenant_id.to_string(),
                scope.workspace_id.to_string(),
            ],
        )
        .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        Ok(())
    }

    async fn fork_workflow_run(
        &self,
        source_scope: &ExecutionScope,
        target_sequence: u32,
        branch_label: &str,
    ) -> Result<(ExecutionScope, TrajectoryBranch), CheckpointError> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        let mut stmt = conn
            .prepare(
                "SELECT id, sequence, node_id, status, state_json, pending_approval_id, created_at
                 FROM workflow_checkpoints
                 WHERE run_id = ?1 AND tenant_id = ?2 AND workspace_id = ?3 AND sequence <= ?4
                 ORDER BY sequence ASC",
            )
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        let rows = stmt
            .query_map(
                params![
                    source_scope.run_id.to_string(),
                    source_scope.tenant_id.to_string(),
                    source_scope.workspace_id.to_string(),
                    target_sequence,
                ],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, u32>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, Option<String>>(5)?,
                        row.get::<_, String>(6)?,
                    ))
                },
            )
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        let mut steps = Vec::new();
        for r in rows {
            steps.push(r.map_err(|e| CheckpointError::DatabaseError(e.to_string()))?);
        }
        drop(stmt);

        if steps.is_empty() {
            return Err(CheckpointError::NotFound(format!(
                "No checkpoints found for run {} at or before sequence {}",
                source_scope.run_id, target_sequence
            )));
        }

        let has_target = steps.iter().any(|s| s.1 == target_sequence);
        if !has_target {
            return Err(CheckpointError::NotFound(format!(
                "Target sequence {} not found for run {}",
                target_sequence, source_scope.run_id
            )));
        }

        let forked_run_id = Uuid::new_v4();
        let forked_scope = ExecutionScope::new(
            source_scope.tenant_id,
            source_scope.workspace_id,
            forked_run_id,
        )
        .map_err(|e| CheckpointError::InvalidState(e.to_string()))?;

        let now = Utc::now();
        let branch = TrajectoryBranch {
            branch_id: Uuid::new_v4(),
            tenant_id: source_scope.tenant_id,
            workspace_id: source_scope.workspace_id,
            parent_run_id: source_scope.run_id,
            forked_run_id,
            fork_sequence: target_sequence,
            branch_label: branch_label.to_string(),
            model_override: None,
            prompt_override: None,
            created_at: now,
        };

        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        for (_, seq, node_id, status_str, state_json, pending_app, created_at) in steps {
            tx.execute(
                "INSERT INTO workflow_checkpoints (id, run_id, tenant_id, workspace_id, sequence, node_id, status, state_json, pending_approval_id, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    Uuid::new_v4().to_string(),
                    forked_run_id.to_string(),
                    source_scope.tenant_id.to_string(),
                    source_scope.workspace_id.to_string(),
                    seq,
                    node_id,
                    status_str,
                    state_json,
                    pending_app,
                    created_at,
                ],
            )
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;
        }

        tx.execute(
            "INSERT INTO trajectory_branches (branch_id, tenant_id, workspace_id, parent_run_id, forked_run_id, fork_sequence, branch_label, model_override, prompt_override, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                branch.branch_id.to_string(),
                branch.tenant_id.to_string(),
                branch.workspace_id.to_string(),
                branch.parent_run_id.to_string(),
                branch.forked_run_id.to_string(),
                branch.fork_sequence,
                branch.branch_label,
                branch.model_override,
                branch.prompt_override,
                branch.created_at.to_rfc3339(),
            ],
        )
        .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        tx.commit()
            .map_err(|e| CheckpointError::DatabaseError(e.to_string()))?;

        Ok((forked_scope, branch))
    }

    async fn record_run_evidence(&self, record: &RunEvidenceRecord) -> Result<(), CheckpointError> {
        self.insert_run_evidence(record)
    }

    async fn record_artifact(&self, artifact: &ArtifactRecord) -> Result<(), CheckpointError> {
        self.insert_artifact(artifact)
    }

    async fn mark_verified(&self, run_id: &str, verified: bool) -> Result<bool, CheckpointError> {
        self.mark_run_verified(run_id, verified)
    }

    async fn get_execution_policy(&self) -> Result<ExecutionPolicyRecord, CheckpointError> {
        self.get_execution_policy()
    }

    async fn insert_knowledge_capsule(
        &self,
        capsule: &KnowledgeCapsuleRecord,
    ) -> Result<(), CheckpointError> {
        self.insert_knowledge_capsule(capsule)
    }

    async fn get_knowledge_capsule(
        &self,
        id: &str,
    ) -> Result<Option<KnowledgeCapsuleRecord>, CheckpointError> {
        self.get_knowledge_capsule(id)
    }

    async fn list_knowledge_capsules(
        &self,
        tenant_id: &str,
        workspace_id: &str,
    ) -> Result<Vec<KnowledgeCapsuleRecord>, CheckpointError> {
        self.list_knowledge_capsules(tenant_id, workspace_id)
    }

    async fn set_knowledge_capsule_negative_transfer(
        &self,
        id: &str,
        negative: bool,
    ) -> Result<(), CheckpointError> {
        self.set_knowledge_capsule_negative_transfer(id, negative)
    }

    async fn insert_capsule_vector(
        &self,
        capsule_id: &str,
        tenant_id: &str,
        workspace_id: &str,
        vector: &[f32],
    ) -> Result<(), CheckpointError> {
        self.insert_capsule_vector(capsule_id, tenant_id, workspace_id, vector)
    }

    async fn search_capsules_vector(
        &self,
        tenant_id: &str,
        workspace_id: &str,
        query_vector: &[f32],
        top_k: usize,
    ) -> Result<Vec<(KnowledgeCapsuleRecord, f32)>, CheckpointError> {
        self.search_capsules_vector(tenant_id, workspace_id, query_vector, top_k)
    }
}
