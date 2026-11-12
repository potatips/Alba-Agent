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
    AuditLogEntry, ForkRunRequest, RunCheckpoint, TrajectoryBranch, WorkflowStepCheckpoint,
};
use crate::domain::{
    ActionIntentKind, ApprovalDecision, ApprovalRequest, ApprovalStatus, ChatMessage,
    ExecutionScope, MessageRole, RunStatus, ToolCall,
};

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

        Ok(())
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
}
