use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::{ChatMessage, ExecutionScope, RunStatus, ToolCall};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunCheckpoint {
    pub id: Uuid,
    pub scope: ExecutionScope,
    pub sequence: u32,
    pub status: RunStatus,
    pub history: Vec<ChatMessage>,
    pub active_tool_call: Option<ToolCall>,
    #[serde(default)]
    pub pending_tool_calls: Vec<ToolCall>,
    pub pending_approval_id: Option<Uuid>,
    #[serde(default)]
    pub author_id: Option<String>,
    #[serde(default)]
    pub model_override: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrajectoryBranch {
    pub branch_id: Uuid,
    pub tenant_id: Uuid,
    pub workspace_id: Uuid,
    pub parent_run_id: Uuid,
    pub forked_run_id: Uuid,
    pub fork_sequence: u32,
    pub branch_label: String,
    pub model_override: Option<String>,
    pub prompt_override: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForkRunRequest {
    pub source_scope: ExecutionScope,
    pub target_sequence: u32,
    pub new_run_id: Option<Uuid>,
    pub branch_label: String,
    pub model_override: Option<String>,
    pub prompt_override: Option<String>,
    pub author_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowStepCheckpoint {
    pub id: Uuid,
    pub scope: ExecutionScope,
    pub sequence: u32,
    pub node_id: String,
    pub status: RunStatus,
    pub state_json: serde_json::Value,
    pub pending_approval_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditLogEntry {
    pub id: Uuid,
    pub scope: ExecutionScope,
    pub actor_id: Option<String>,
    pub action: String,
    pub target_id: Option<Uuid>,
    pub details: serde_json::Value,
    pub created_at: DateTime<Utc>,
}
