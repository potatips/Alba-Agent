use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::ExecutionScope;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuntimeEventType {
    #[serde(rename = "TOOL_INVOKED")]
    ToolInvoked,
    #[serde(rename = "TOOL_COMPLETED")]
    ToolCompleted,
    #[serde(rename = "APPROVAL_REQUIRED")]
    ApprovalRequired,
    #[serde(rename = "APPROVAL_RESOLVED")]
    ApprovalResolved,
    #[serde(rename = "FLOW_STATE_CHANGED")]
    FlowStateChanged,
    #[serde(rename = "SANDBOX_TERMINAL_OUTPUT")]
    SandboxTerminalOutput,
    #[serde(rename = "SANDBOX_FILE_DIFF")]
    SandboxFileDiff,
    #[serde(rename = "TRAJECTORY_FORKED")]
    TrajectoryForked,
    #[serde(rename = "RUN_COMPLETED")]
    RunCompleted,
    #[serde(rename = "RUN_FAILED")]
    RunFailed,
}

impl RuntimeEventType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ToolInvoked => "TOOL_INVOKED",
            Self::ToolCompleted => "TOOL_COMPLETED",
            Self::ApprovalRequired => "APPROVAL_REQUIRED",
            Self::ApprovalResolved => "APPROVAL_RESOLVED",
            Self::FlowStateChanged => "FLOW_STATE_CHANGED",
            Self::SandboxTerminalOutput => "SANDBOX_TERMINAL_OUTPUT",
            Self::SandboxFileDiff => "SANDBOX_FILE_DIFF",
            Self::TrajectoryForked => "TRAJECTORY_FORKED",
            Self::RunCompleted => "RUN_COMPLETED",
            Self::RunFailed => "RUN_FAILED",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeEvent {
    pub id: Uuid,
    #[serde(flatten)]
    pub scope: ExecutionScope,
    pub node_id: Option<String>,
    #[serde(rename = "type")]
    pub event_type: RuntimeEventType,
    pub occurred_at: DateTime<Utc>,
    pub payload: serde_json::Value,
}

impl RuntimeEvent {
    pub fn new(
        scope: ExecutionScope,
        event_type: RuntimeEventType,
        payload: serde_json::Value,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            scope,
            node_id: None,
            event_type,
            occurred_at: Utc::now(),
            payload,
        }
    }

    pub fn with_node(mut self, node_id: impl Into<String>) -> Self {
        self.node_id = Some(node_id.into());
        self
    }
}
