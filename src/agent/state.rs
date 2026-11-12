use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::{ChatMessage, ExecutionScope, RunStatus, ToolCall};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentState {
    pub scope: ExecutionScope,
    pub status: RunStatus,
    pub sequence: u32,
    pub history: Vec<ChatMessage>,
    pub active_tool_call: Option<ToolCall>,
    #[serde(default)]
    pub pending_tool_calls: Vec<ToolCall>,
    pub pending_approval_id: Option<Uuid>,
    #[serde(default)]
    pub author_id: Option<String>,
    #[serde(default)]
    pub model_override: Option<String>,
    pub final_response: Option<String>,
    pub error_message: Option<String>,
}

impl AgentState {
    pub fn new(scope: ExecutionScope) -> Self {
        Self {
            scope,
            status: RunStatus::Queued,
            sequence: 0,
            history: Vec::new(),
            active_tool_call: None,
            pending_tool_calls: Vec::new(),
            pending_approval_id: None,
            author_id: None,
            model_override: None,
            final_response: None,
            error_message: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineOutcome {
    Completed(String),
    Paused {
        approval_id: Uuid,
        tool_name: String,
        action_description: String,
    },
    Failed(String),
    Cancelled,
}
