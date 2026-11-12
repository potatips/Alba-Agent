use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use nexus_albaai::agent::engine::AgentEngine;
use nexus_albaai::agent::state::EngineOutcome;
use nexus_albaai::checkpoints::sqlite::SqliteCheckpointStore;
use nexus_albaai::checkpoints::store::CheckpointStore;
use nexus_albaai::checkpoints::types::{
    AuditLogEntry, ForkRunRequest, RunCheckpoint, TrajectoryBranch,
};
use nexus_albaai::domain::{ApprovalDecision, ApprovalRequest, ExecutionScope, HistoryLimits};
use nexus_albaai::events::sink::{EventError, EventSink, InMemoryEventSink};
use nexus_albaai::events::types::RuntimeEvent;
use nexus_albaai::governance::policy::DefaultApprovalPolicy;
use nexus_albaai::models::adapter::OpenAiCompatibleClient;
use nexus_albaai::models::types::ModelConfig;
use nexus_albaai::tools::dir_list::scan_directory;
use nexus_albaai::tools::file_read::{ReadFileTool, DEFAULT_MAX_FILE_BYTES};
use nexus_albaai::tools::file_write::preview_write_diff;
use nexus_albaai::tools::register_workspace_tools;
use nexus_albaai::tools::types::ToolRegistry;

pub type EventCallback = Arc<dyn Fn(RuntimeEvent) + Send + Sync>;

pub struct ForwardingEventSink {
    inner: InMemoryEventSink,
    callback: Option<EventCallback>,
}

impl ForwardingEventSink {
    pub fn new(callback: Option<EventCallback>) -> Self {
        Self {
            inner: InMemoryEventSink::new(),
            callback,
        }
    }

    pub fn recorded(&self) -> Vec<RuntimeEvent> {
        self.inner.events()
    }
}

impl EventSink for ForwardingEventSink {
    fn emit(&self, event: RuntimeEvent) -> Result<(), EventError> {
        if let Some(cb) = &self.callback {
            cb(event.clone());
        }
        self.inner.emit(event)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefaultWorkspaceDto {
    pub workspace_root: String,
    pub api_base: String,
    pub model: String,
    pub default_author_id: String,
    pub default_approver_id: String,
    pub has_env_api_key: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingApprovalDto {
    pub id: String,
    pub tool_name: String,
    pub action_description: String,
    pub path: Option<String>,
    pub program: Option<String>,
    pub args: Option<Vec<String>>,
    pub diff: Option<String>,
    pub author_id: Option<String>,
    pub status: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointDto {
    pub id: String,
    pub tenant_id: String,
    pub workspace_id: String,
    pub run_id: String,
    pub sequence: u32,
    pub status: String,
    pub history_len: usize,
    pub active_tool_name: Option<String>,
    pub pending_tool_count: usize,
    pub pending_approval_id: Option<String>,
    pub author_id: Option<String>,
    pub created_at: String,
    pub preview: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrajectoryBranchDto {
    pub id: String,
    pub parent_run_id: String,
    pub parent_sequence: u32,
    pub branch_run_id: String,
    pub branch_label: String,
    pub override_prompt: Option<String>,
    pub override_model: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunOutcomeDto {
    pub tenant_id: String,
    pub workspace_id: String,
    pub run_id: String,
    pub status: String,
    pub message: Option<String>,
    pub approval_id: Option<String>,
    pub pending_approval: Option<PendingApprovalDto>,
    pub checkpoints: Vec<CheckpointDto>,
    pub branches: Vec<TrajectoryBranchDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForkResultDto {
    pub branch: TrajectoryBranchDto,
    pub initial_checkpoint: CheckpointDto,
    pub outcome: Option<RunOutcomeDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceEntryDto {
    pub path: String,
    pub entry_type: String,
    pub size_bytes: u64,
    pub modified: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceFileContentDto {
    pub path: String,
    pub content: String,
    pub byte_len: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogDto {
    pub id: String,
    pub event_type: String,
    pub actor: Option<String>,
    pub details_json: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StartRunPayload {
    pub workspace_root: String,
    pub prompt: String,
    pub model: Option<String>,
    pub api_base: Option<String>,
    pub api_key: Option<String>,
    pub author_id: String,
    pub require_approval_for_reads: Option<bool>,
    pub tenant_id: Option<String>,
    pub workspace_id: Option<String>,
    pub run_id: Option<String>,
    pub db_path: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ResumeRunPayload {
    pub tenant_id: String,
    pub workspace_id: String,
    pub run_id: String,
    pub workspace_root: String,
    pub model: Option<String>,
    pub api_base: Option<String>,
    pub api_key: Option<String>,
    pub author_id: Option<String>,
    pub db_path: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ApproveActionPayload {
    pub tenant_id: String,
    pub workspace_id: String,
    pub run_id: String,
    pub approval_id: String,
    pub decision: String,
    pub approver_id: String,
    pub db_path: Option<String>,
    pub resume_after: Option<bool>,
    pub workspace_root: Option<String>,
    pub model: Option<String>,
    pub api_base: Option<String>,
    pub api_key: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ForkTrajectoryPayload {
    pub tenant_id: String,
    pub workspace_id: String,
    pub source_run_id: String,
    pub checkpoint_sequence: u32,
    pub branch_label: String,
    pub override_prompt: Option<String>,
    pub override_model: Option<String>,
    pub author_id: String,
    pub db_path: Option<String>,
    pub auto_resume: Option<bool>,
    pub workspace_root: Option<String>,
    pub api_base: Option<String>,
    pub api_key: Option<String>,
}

pub struct DesktopEngineService {
    event_callback: Option<EventCallback>,
}

impl Default for DesktopEngineService {
    fn default() -> Self {
        Self::new(None)
    }
}

impl DesktopEngineService {
    pub fn new(event_callback: Option<EventCallback>) -> Self {
        Self { event_callback }
    }

    pub fn get_default_workspace(&self) -> DefaultWorkspaceDto {
        let workspace_root = std::env::current_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| ".".to_string());
        let api_base = std::env::var("NEXUS_API_BASE")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| "https://api.openai.com/v1".to_string());
        let model = std::env::var("NEXUS_MODEL")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| "gpt-4o-mini".to_string());
        let default_author_id = std::env::var("NEXUS_ACTOR_ID")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| "developer@local".to_string());
        let default_approver_id = if default_author_id == "reviewer@local" {
            "security-approver@local".to_string()
        } else {
            "reviewer@local".to_string()
        };
        let has_env_api_key = std::env::var("NEXUS_API_KEY")
            .ok()
            .is_some_and(|v| !v.trim().is_empty());

        DefaultWorkspaceDto {
            workspace_root,
            api_base,
            model,
            default_author_id,
            default_approver_id,
            has_env_api_key,
        }
    }

    pub fn list_workspace_files(
        &self,
        workspace_root: &str,
        max_depth: Option<usize>,
    ) -> Result<Vec<WorkspaceEntryDto>, String> {
        let root = Path::new(workspace_root);
        let scan = scan_directory(root, ".", max_depth.unwrap_or(5), 500, false)
            .map_err(|e| format!("Failed listing workspace files: {}", e))?;

        Ok(scan
            .entries
            .into_iter()
            .map(|entry| {
                let entry_type = if entry.kind == "dir" { "dir" } else { "file" }.to_string();
                let modified = fs::metadata(root.join(&entry.path))
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .map(|t| {
                        let dt: chrono::DateTime<chrono::Utc> = t.into();
                        dt.to_rfc3339()
                    });
                WorkspaceEntryDto {
                    path: entry.path,
                    entry_type,
                    size_bytes: entry.size,
                    modified,
                }
            })
            .collect())
    }

    pub fn read_workspace_file(
        &self,
        workspace_root: &str,
        rel_path: &str,
    ) -> Result<WorkspaceFileContentDto, String> {
        let reader = ReadFileTool::new(workspace_root);
        let resolved = reader
            .validate_and_resolve(rel_path)
            .map_err(|e| format!("File access blocked: {}", e))?;
        let bytes =
            fs::read(&resolved).map_err(|e| format!("Failed reading '{}': {}", rel_path, e))?;
        if bytes.len() > DEFAULT_MAX_FILE_BYTES {
            return Err(format!(
                "File '{}' exceeds {} byte viewer limit",
                rel_path, DEFAULT_MAX_FILE_BYTES
            ));
        }
        let content = String::from_utf8(bytes)
            .map_err(|_| format!("File '{}' is binary and cannot be displayed", rel_path))?;
        let byte_len = content.len();
        Ok(WorkspaceFileContentDto {
            path: rel_path.to_string(),
            content,
            byte_len,
        })
    }

    pub async fn start_run(&self, req: StartRunPayload) -> Result<RunOutcomeDto, String> {
        if req.author_id.trim().is_empty() {
            return Err("Author identity cannot be empty".to_string());
        }
        let scope = parse_or_create_scope(
            req.tenant_id.as_deref(),
            req.workspace_id.as_deref(),
            req.run_id.as_deref(),
        )?;
        let db_path = resolve_db_path(req.db_path.as_deref())?;
        let store = Arc::new(
            SqliteCheckpointStore::new(&db_path)
                .map_err(|e| format!("Failed opening checkpoint database: {}", e))?,
        );
        let events = Arc::new(ForwardingEventSink::new(self.event_callback.clone()));
        let engine = build_engine(
            &req.workspace_root,
            req.api_base.as_deref(),
            req.api_key.as_deref(),
            req.model.as_deref(),
            req.require_approval_for_reads.unwrap_or(false),
            &req.author_id,
            store.clone(),
            events,
        )?;

        let outcome = engine
            .start_run_as(scope, &req.prompt, Some(&req.author_id))
            .await
            .map_err(|e| format!("Engine run failed: {}", e))?;

        build_outcome_dto(store.as_ref(), &req.workspace_root, scope, outcome).await
    }

    pub async fn resume_run(&self, req: ResumeRunPayload) -> Result<RunOutcomeDto, String> {
        let scope = parse_scope(&req.tenant_id, &req.workspace_id, &req.run_id)?;
        let db_path = resolve_db_path(req.db_path.as_deref())?;
        let store = Arc::new(
            SqliteCheckpointStore::new(&db_path)
                .map_err(|e| format!("Failed opening checkpoint database: {}", e))?,
        );
        let events = Arc::new(ForwardingEventSink::new(self.event_callback.clone()));
        let author = req
            .author_id
            .as_deref()
            .filter(|v| !v.trim().is_empty())
            .unwrap_or("developer@local");
        let engine = build_engine(
            &req.workspace_root,
            req.api_base.as_deref(),
            req.api_key.as_deref(),
            req.model.as_deref(),
            false,
            author,
            store.clone(),
            events,
        )?;

        let outcome = engine
            .resume_run(scope)
            .await
            .map_err(|e| format!("Resume run failed: {}", e))?;

        build_outcome_dto(store.as_ref(), &req.workspace_root, scope, outcome).await
    }

    pub async fn approve_action(&self, req: ApproveActionPayload) -> Result<RunOutcomeDto, String> {
        let scope = parse_scope(&req.tenant_id, &req.workspace_id, &req.run_id)?;
        let approval_id = Uuid::parse_str(req.approval_id.trim())
            .map_err(|e| format!("Invalid approval_id: {}", e))?;
        let approver = req.approver_id.trim();
        if approver.is_empty() {
            return Err("Approver identity cannot be empty".to_string());
        }
        let decision = match req.decision.trim().to_ascii_lowercase().as_str() {
            "allow_once" => ApprovalDecision::AllowOnce,
            "deny" => ApprovalDecision::Deny,
            other => return Err(format!("Invalid approval decision '{}'", other)),
        };

        let db_path = resolve_db_path(req.db_path.as_deref())?;
        let store = Arc::new(
            SqliteCheckpointStore::new(&db_path)
                .map_err(|e| format!("Failed opening checkpoint database: {}", e))?,
        );
        store
            .resolve_approval(&scope, approval_id, decision, approver)
            .await
            .map_err(|e| format!("Approval failed: {}", e))?;

        if decision == ApprovalDecision::Deny {
            let events = Arc::new(ForwardingEventSink::new(self.event_callback.clone()));
            let _ = events.emit(RuntimeEvent::new(
                scope,
                nexus_albaai::events::types::RuntimeEventType::ApprovalResolved,
                serde_json::json!({
                    "approval_id": approval_id,
                    "decision": "deny",
                    "actor": approver
                }),
            ));
            let ws_root = req.workspace_root.as_deref().unwrap_or(".");
            return build_outcome_dto(store.as_ref(), ws_root, scope, EngineOutcome::Cancelled)
                .await;
        }

        if req.resume_after.unwrap_or(true) {
            let ws_root = req
                .workspace_root
                .clone()
                .ok_or_else(|| "workspace_root is required to resume after approval".to_string())?;
            return self
                .resume_run(ResumeRunPayload {
                    tenant_id: req.tenant_id,
                    workspace_id: req.workspace_id,
                    run_id: req.run_id,
                    workspace_root: ws_root,
                    model: req.model,
                    api_base: req.api_base,
                    api_key: req.api_key,
                    author_id: None,
                    db_path: req.db_path,
                })
                .await;
        }

        let ws_root = req.workspace_root.as_deref().unwrap_or(".");
        build_outcome_dto(
            store.as_ref(),
            ws_root,
            scope,
            EngineOutcome::Paused {
                approval_id,
                tool_name: String::new(),
                action_description: String::new(),
            },
        )
        .await
    }

    pub async fn get_checkpoints(
        &self,
        tenant_id: &str,
        workspace_id: &str,
        run_id: &str,
        db_path: Option<&str>,
    ) -> Result<Vec<CheckpointDto>, String> {
        let scope = parse_scope(tenant_id, workspace_id, run_id)?;
        let resolved_db = resolve_db_path(db_path)?;
        let store = SqliteCheckpointStore::new(&resolved_db)
            .map_err(|e| format!("Failed opening checkpoint database: {}", e))?;
        let cps = store
            .list_checkpoints(&scope)
            .await
            .map_err(|e| format!("Failed listing checkpoints: {}", e))?;
        Ok(cps.into_iter().map(checkpoint_to_dto).collect())
    }

    pub async fn get_branches(
        &self,
        tenant_id: &str,
        workspace_id: &str,
        run_id: &str,
        db_path: Option<&str>,
    ) -> Result<Vec<TrajectoryBranchDto>, String> {
        let scope = parse_scope(tenant_id, workspace_id, run_id)?;
        let resolved_db = resolve_db_path(db_path)?;
        let store = SqliteCheckpointStore::new(&resolved_db)
            .map_err(|e| format!("Failed opening checkpoint database: {}", e))?;
        let branches = store
            .list_branches(&scope)
            .await
            .map_err(|e| format!("Failed listing branches: {}", e))?;
        Ok(branches.into_iter().map(branch_to_dto).collect())
    }

    pub async fn get_audit_logs(
        &self,
        tenant_id: &str,
        workspace_id: &str,
        run_id: &str,
        db_path: Option<&str>,
    ) -> Result<Vec<AuditLogDto>, String> {
        let scope = parse_scope(tenant_id, workspace_id, run_id)?;
        let resolved_db = resolve_db_path(db_path)?;
        let store = SqliteCheckpointStore::new(&resolved_db)
            .map_err(|e| format!("Failed opening checkpoint database: {}", e))?;
        let logs = store
            .list_audit_logs(&scope)
            .await
            .map_err(|e| format!("Failed listing audit logs: {}", e))?;
        Ok(logs.into_iter().map(audit_to_dto).collect())
    }

    pub async fn fork_trajectory(
        &self,
        req: ForkTrajectoryPayload,
    ) -> Result<ForkResultDto, String> {
        let source_scope = parse_scope(&req.tenant_id, &req.workspace_id, &req.source_run_id)?;
        let db_path = resolve_db_path(req.db_path.as_deref())?;
        let store = Arc::new(
            SqliteCheckpointStore::new(&db_path)
                .map_err(|e| format!("Failed opening checkpoint database: {}", e))?,
        );
        let events = Arc::new(ForwardingEventSink::new(self.event_callback.clone()));
        let ws_root = req.workspace_root.as_deref().unwrap_or(".");
        let engine = build_engine(
            ws_root,
            req.api_base.as_deref(),
            req.api_key.as_deref(),
            req.override_model.as_deref(),
            false,
            &req.author_id,
            store.clone(),
            events,
        )?;

        let (initial_cp, branch) = engine
            .fork_trajectory(ForkRunRequest {
                source_scope,
                target_sequence: req.checkpoint_sequence,
                new_run_id: None,
                branch_label: req.branch_label,
                model_override: req.override_model.clone(),
                prompt_override: req.override_prompt.clone(),
                author_id: Some(req.author_id.clone()),
            })
            .await
            .map_err(|e| format!("Fork trajectory failed: {}", e))?;

        let outcome = if req.auto_resume.unwrap_or(false) {
            let forked_scope = initial_cp.scope;
            let engine_outcome = engine
                .resume_run(forked_scope)
                .await
                .map_err(|e| format!("Resuming forked branch failed: {}", e))?;
            Some(build_outcome_dto(store.as_ref(), ws_root, forked_scope, engine_outcome).await?)
        } else {
            None
        };

        Ok(ForkResultDto {
            branch: branch_to_dto(branch),
            initial_checkpoint: checkpoint_to_dto(initial_cp),
            outcome,
        })
    }
}

#[allow(clippy::too_many_arguments)]
fn build_engine(
    workspace_root: &str,
    api_base: Option<&str>,
    api_key: Option<&str>,
    model: Option<&str>,
    require_approval_for_reads: bool,
    author_id: &str,
    store: Arc<SqliteCheckpointStore>,
    events: Arc<ForwardingEventSink>,
) -> Result<
    AgentEngine<
        OpenAiCompatibleClient,
        SqliteCheckpointStore,
        ForwardingEventSink,
        DefaultApprovalPolicy,
    >,
    String,
> {
    let base = api_base
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(ToString::to_string)
        .or_else(|| {
            std::env::var("NEXUS_API_BASE")
                .ok()
                .filter(|v| !v.trim().is_empty())
        })
        .unwrap_or_else(|| "https://api.openai.com/v1".to_string());

    let resolved_key = api_key
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(ToString::to_string)
        .or_else(|| {
            std::env::var("NEXUS_API_KEY")
                .ok()
                .filter(|v| !v.trim().is_empty())
        })
        .ok_or_else(|| {
            "Missing API key: provide it in Engine Config or set NEXUS_API_KEY".to_string()
        })?;

    let resolved_model = model
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(ToString::to_string)
        .or_else(|| {
            std::env::var("NEXUS_MODEL")
                .ok()
                .filter(|v| !v.trim().is_empty())
        })
        .unwrap_or_else(|| "gpt-4o-mini".to_string());

    let model_cfg = ModelConfig::new(base, resolved_key, resolved_model);
    let client = Arc::new(
        OpenAiCompatibleClient::new(model_cfg)
            .map_err(|e| format!("Invalid model configuration: {}", e))?,
    );
    let policy = Arc::new(
        DefaultApprovalPolicy::new()
            .with_workspace_root(workspace_root)
            .with_require_all(require_approval_for_reads),
    );
    let mut registry = ToolRegistry::new();
    register_workspace_tools(&mut registry, workspace_root, Some(events.clone()));

    Ok(AgentEngine::new(client, store, events, policy, registry)
        .with_history_limits(HistoryLimits::default())
        .with_author_id(author_id))
}

async fn build_outcome_dto(
    store: &SqliteCheckpointStore,
    workspace_root: &str,
    scope: ExecutionScope,
    outcome: EngineOutcome,
) -> Result<RunOutcomeDto, String> {
    let cps = store
        .list_checkpoints(&scope)
        .await
        .map_err(|e| format!("Failed reading checkpoints: {}", e))?;
    let branches = store
        .list_branches(&scope)
        .await
        .map_err(|e| format!("Failed reading branches: {}", e))?;

    let (status, message, approval_id) = match outcome {
        EngineOutcome::Completed(msg) => ("completed".to_string(), Some(msg), None),
        EngineOutcome::Paused {
            approval_id,
            action_description,
            ..
        } => (
            "paused_for_approval".to_string(),
            Some(if action_description.is_empty() {
                format!("Paused waiting for human approval {}", approval_id)
            } else {
                action_description
            }),
            Some(approval_id),
        ),
        EngineOutcome::Failed(err) => ("failed".to_string(), Some(err), None),
        EngineOutcome::Cancelled => (
            "cancelled".to_string(),
            Some("Run cancelled".to_string()),
            None,
        ),
    };

    let latest_cp = cps.last();
    let pending_approval = if let Some(aid) = approval_id {
        store
            .get_approval(&scope, aid)
            .await
            .map_err(|e| format!("Failed reading approval details: {}", e))?
            .map(|req| {
                build_pending_approval_dto(
                    workspace_root,
                    req,
                    latest_cp.and_then(|cp| cp.active_tool_call.as_ref()),
                )
            })
    } else {
        None
    };

    Ok(RunOutcomeDto {
        tenant_id: scope.tenant_id.to_string(),
        workspace_id: scope.workspace_id.to_string(),
        run_id: scope.run_id.to_string(),
        status,
        message,
        approval_id: approval_id.map(|id| id.to_string()),
        pending_approval,
        checkpoints: cps.into_iter().map(checkpoint_to_dto).collect(),
        branches: branches.into_iter().map(branch_to_dto).collect(),
    })
}

fn build_pending_approval_dto(
    workspace_root: &str,
    req: ApprovalRequest,
    active_call: Option<&nexus_albaai::domain::ToolCall>,
) -> PendingApprovalDto {
    let path = active_call
        .and_then(|c| c.arguments.get("path"))
        .and_then(|v| v.as_str())
        .map(ToString::to_string);
    let program = active_call
        .and_then(|c| c.arguments.get("program"))
        .and_then(|v| v.as_str())
        .map(ToString::to_string);
    let args = active_call
        .and_then(|c| c.arguments.get("args"))
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|item| item.as_str().map(ToString::to_string))
                .collect()
        });

    let diff = if req.tool_name == "write_file" {
        active_call.and_then(|call| {
            let target_path = call.arguments.get("path")?.as_str()?;
            let content = call.arguments.get("content")?.as_str()?;
            preview_write_diff(Path::new(workspace_root), target_path, content)
                .ok()
                .map(|p| p.unified_diff)
        })
    } else {
        None
    };

    PendingApprovalDto {
        id: req.id.to_string(),
        tool_name: req.tool_name,
        action_description: req.action_description,
        path,
        program,
        args,
        diff,
        author_id: req.author_id,
        status: format!("{:?}", req.status).to_lowercase(),
        created_at: req.created_at.to_rfc3339(),
    }
}

fn checkpoint_to_dto(cp: RunCheckpoint) -> CheckpointDto {
    let preview = cp
        .history
        .last()
        .map(|m| {
            let trimmed = m.content.trim();
            if trimmed.is_empty() {
                format!("{:?} message", m.role)
            } else {
                trimmed.chars().take(140).collect()
            }
        })
        .unwrap_or_else(|| "Empty state".to_string());

    CheckpointDto {
        id: cp.id.to_string(),
        tenant_id: cp.scope.tenant_id.to_string(),
        workspace_id: cp.scope.workspace_id.to_string(),
        run_id: cp.scope.run_id.to_string(),
        sequence: cp.sequence,
        status: format!("{:?}", cp.status).to_lowercase(),
        history_len: cp.history.len(),
        active_tool_name: cp.active_tool_call.map(|c| c.name),
        pending_tool_count: cp.pending_tool_calls.len(),
        pending_approval_id: cp.pending_approval_id.map(|id| id.to_string()),
        author_id: cp.author_id,
        created_at: cp.created_at.to_rfc3339(),
        preview,
    }
}

fn branch_to_dto(branch: TrajectoryBranch) -> TrajectoryBranchDto {
    TrajectoryBranchDto {
        id: branch.branch_id.to_string(),
        parent_run_id: branch.parent_run_id.to_string(),
        parent_sequence: branch.fork_sequence,
        branch_run_id: branch.forked_run_id.to_string(),
        branch_label: branch.branch_label,
        override_prompt: branch.prompt_override,
        override_model: branch.model_override,
        created_at: branch.created_at.to_rfc3339(),
    }
}

fn audit_to_dto(entry: AuditLogEntry) -> AuditLogDto {
    AuditLogDto {
        id: entry.id.to_string(),
        event_type: entry.action,
        actor: entry.actor_id,
        details_json: entry.details.to_string(),
        created_at: entry.created_at.to_rfc3339(),
    }
}

fn parse_scope(
    tenant_id: &str,
    workspace_id: &str,
    run_id: &str,
) -> Result<ExecutionScope, String> {
    let tid = Uuid::parse_str(tenant_id.trim()).map_err(|e| format!("Invalid tenant_id: {}", e))?;
    let wid =
        Uuid::parse_str(workspace_id.trim()).map_err(|e| format!("Invalid workspace_id: {}", e))?;
    let rid = Uuid::parse_str(run_id.trim()).map_err(|e| format!("Invalid run_id: {}", e))?;
    ExecutionScope::new(tid, wid, rid).map_err(|e| format!("Invalid execution scope: {}", e))
}

fn parse_or_create_scope(
    tenant_id: Option<&str>,
    workspace_id: Option<&str>,
    run_id: Option<&str>,
) -> Result<ExecutionScope, String> {
    let tid = match tenant_id.map(str::trim).filter(|v| !v.is_empty()) {
        Some(raw) => Uuid::parse_str(raw).map_err(|e| format!("Invalid tenant_id: {}", e))?,
        None => Uuid::new_v4(),
    };
    let wid = match workspace_id.map(str::trim).filter(|v| !v.is_empty()) {
        Some(raw) => Uuid::parse_str(raw).map_err(|e| format!("Invalid workspace_id: {}", e))?,
        None => Uuid::new_v4(),
    };
    let rid = match run_id.map(str::trim).filter(|v| !v.is_empty()) {
        Some(raw) => Uuid::parse_str(raw).map_err(|e| format!("Invalid run_id: {}", e))?,
        None => Uuid::new_v4(),
    };
    ExecutionScope::new(tid, wid, rid).map_err(|e| format!("Invalid execution scope: {}", e))
}

fn resolve_db_path(explicit: Option<&str>) -> Result<PathBuf, String> {
    if let Some(raw) = explicit.map(str::trim).filter(|v| !v.is_empty()) {
        return Ok(PathBuf::from(raw));
    }
    if let Ok(env_path) = std::env::var("NEXUS_DB_PATH") {
        let trimmed = env_path.trim();
        if !trimmed.is_empty() {
            return Ok(PathBuf::from(trimmed));
        }
    }
    let home = std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir());
    Ok(home
        .join(".local")
        .join("share")
        .join("alba-agent")
        .join("state.db"))
}
