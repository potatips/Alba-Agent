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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowSummary {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunRecord {
    pub id: String,
    pub workflow_id: String,
    pub status: String,
    pub input_data: String,
    pub output_data: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub completed_at: Option<String>,
    pub verified: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unverified_reason: Option<String>,
}

impl Default for RunRecord {
    fn default() -> Self {
        Self {
            id: String::new(),
            workflow_id: String::new(),
            status: "PENDING".to_string(),
            input_data: "{}".to_string(),
            output_data: None,
            created_at: Utc::now().to_rfc3339(),
            updated_at: Utc::now().to_rfc3339(),
            completed_at: None,
            verified: false,
            unverified_reason: None,
        }
    }
}

impl RunRecord {
    pub fn new(id: impl Into<String>, workflow_id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            workflow_id: workflow_id.into(),
            ..Default::default()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunEvidenceRecord {
    pub id: String,
    pub run_id: String,
    pub command: String,
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: i64,
    pub verified: bool,
    pub created_at: String,
    #[serde(default)]
    pub output_sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unverified_reason: Option<String>,
}

impl Default for RunEvidenceRecord {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            run_id: String::new(),
            command: String::new(),
            exit_code: 0,
            stdout: String::new(),
            stderr: String::new(),
            duration_ms: 0,
            verified: false,
            created_at: Utc::now().to_rfc3339(),
            output_sha256: String::new(),
            unverified_reason: None,
        }
    }
}

impl RunEvidenceRecord {
    pub fn new(run_id: impl Into<String>, command: impl Into<String>) -> Self {
        Self {
            run_id: run_id.into(),
            command: command.into(),
            ..Default::default()
        }
    }

    pub fn compute_output_sha256(stdout: &str, stderr: &str) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(stdout.as_bytes());
        hasher.update(stderr.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    pub fn output_sha256(&self) -> String {
        if self.output_sha256.is_empty() {
            Self::compute_output_sha256(&self.stdout, &self.stderr)
        } else {
            self.output_sha256.clone()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactRecord {
    pub id: String,
    pub run_id: String,
    pub filename: String,
    pub file_path: String,
    pub file_size: i64,
    pub mime_type: String,
    pub sha256: String,
    pub verified: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapsuleRecord {
    pub id: String,
    pub title: String,
    pub trigger_pattern: String,
    pub advice: String,
    pub scope: String,
    pub sensitivity: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionPolicyRecord {
    pub default_mode: String,
    pub allowed_domains: Vec<String>,
    pub denied_domains: Vec<String>,
}

impl Default for ExecutionPolicyRecord {
    fn default() -> Self {
        Self {
            default_mode: "allow".to_string(),
            allowed_domains: vec![
                "api.anthropic.com".to_string(),
                "generativelanguage.googleapis.com".to_string(),
            ],
            denied_domains: vec![
                "169.254.169.254".to_string(),
                "metadata.google.internal".to_string(),
            ],
        }
    }
}
