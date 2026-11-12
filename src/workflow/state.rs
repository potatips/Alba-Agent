use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowRunState {
    Pending,
    Running,
    AwaitingApproval,
    Paused,
    Completed,
    Failed,
    Cancelled,
}

impl WorkflowRunState {
    pub fn is_terminal(&self) -> bool {
        match self {
            Self::Pending => false,
            Self::Running => false,
            Self::AwaitingApproval => false,
            Self::Paused => false,
            Self::Completed => true,
            Self::Failed => true,
            Self::Cancelled => true,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::AwaitingApproval => "awaiting_approval",
            Self::Paused => "paused",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowNodeState {
    Pending,
    Running,
    AwaitingApproval,
    Paused,
    Completed,
    Failed,
    Cancelled,
}

impl WorkflowNodeState {
    pub fn is_terminal(&self) -> bool {
        match self {
            Self::Pending => false,
            Self::Running => false,
            Self::AwaitingApproval => false,
            Self::Paused => false,
            Self::Completed => true,
            Self::Failed => true,
            Self::Cancelled => true,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::AwaitingApproval => "awaiting_approval",
            Self::Paused => "paused",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowExecutionContext {
    pub run_id: Uuid,
    pub current_node_id: String,
    pub run_state: WorkflowRunState,
    pub node_states: BTreeMap<String, WorkflowNodeState>,
    pub completed_nodes: Vec<String>,
    pub task_outputs: BTreeMap<String, String>,
    pub node_outputs: BTreeMap<String, serde_json::Value>,
    pub loop_counts: BTreeMap<String, u32>,
    pub pending_approval_id: Option<Uuid>,
    pub pending_approval_node_id: Option<String>,
    pub author_id: Option<String>,
    pub sequence: u32,
    pub final_output: Option<serde_json::Value>,
    pub error: Option<String>,
    pub updated_at: DateTime<Utc>,
}
