use std::str::FromStr;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

#[derive(Error, Debug, PartialEq, Eq)]
pub enum DomainError {
    #[error("Invalid execution scope: {0}")]
    InvalidScope(String),
    #[error("Scope mismatch: expected {expected:?}, found {actual:?}")]
    ScopeMismatch {
        expected: ExecutionScope,
        actual: ExecutionScope,
    },
    #[error("Invalid status transition from {from} to {to}")]
    InvalidStatusTransition { from: String, to: String },
    #[error("Invalid status value: {0}")]
    InvalidStatus(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ExecutionScope {
    pub tenant_id: Uuid,
    pub workspace_id: Uuid,
    pub run_id: Uuid,
}

impl ExecutionScope {
    pub fn new(tenant_id: Uuid, workspace_id: Uuid, run_id: Uuid) -> Result<Self, DomainError> {
        if tenant_id.is_nil() {
            return Err(DomainError::InvalidScope(
                "tenant_id cannot be nil".to_string(),
            ));
        }
        if workspace_id.is_nil() {
            return Err(DomainError::InvalidScope(
                "workspace_id cannot be nil".to_string(),
            ));
        }
        if run_id.is_nil() {
            return Err(DomainError::InvalidScope(
                "run_id cannot be nil".to_string(),
            ));
        }
        Ok(Self {
            tenant_id,
            workspace_id,
            run_id,
        })
    }

    pub fn matches(&self, other: &ExecutionScope) -> bool {
        self.tenant_id == other.tenant_id
            && self.workspace_id == other.workspace_id
            && self.run_id == other.run_id
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Queued,
    Running,
    Paused,
    Executing,
    Completed,
    Failed,
    Cancelled,
}

impl RunStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Paused => "paused",
            Self::Executing => "executing",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }
}

impl FromStr for RunStatus {
    type Err = DomainError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "queued" => Ok(Self::Queued),
            "running" => Ok(Self::Running),
            "paused" => Ok(Self::Paused),
            "executing" => Ok(Self::Executing),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            "cancelled" => Ok(Self::Cancelled),
            _ => Err(DomainError::InvalidStatus(value.to_string())),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionIntentKind {
    ExternalEmail,
    ExternalAction,
    DestructiveCommand,
    HighSpend,
    FileAccess,
    Other,
}

impl ActionIntentKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ExternalEmail => "external_email",
            Self::ExternalAction => "external_action",
            Self::DestructiveCommand => "destructive_command",
            Self::HighSpend => "high_spend",
            Self::FileAccess => "file_access",
            Self::Other => "other",
        }
    }
}

impl FromStr for ActionIntentKind {
    type Err = std::convert::Infallible;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(match value {
            "external_email" => Self::ExternalEmail,
            "external_action" => Self::ExternalAction,
            "destructive_command" => Self::DestructiveCommand,
            "high_spend" => Self::HighSpend,
            "file_access" => Self::FileAccess,
            _ => Self::Other,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionIntent {
    pub kind: ActionIntentKind,
    pub description: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalDecision {
    AllowOnce,
    Deny,
}

impl ApprovalDecision {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::AllowOnce => "allow_once",
            Self::Deny => "deny",
        }
    }
}

impl FromStr for ApprovalDecision {
    type Err = DomainError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "allow_once" => Ok(Self::AllowOnce),
            "deny" => Ok(Self::Deny),
            _ => Err(DomainError::InvalidStatus(format!(
                "Invalid decision: {}",
                value
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalStatus {
    Pending,
    AllowedOnce,
    Executing,
    Denied,
    Consumed,
}

impl ApprovalStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::AllowedOnce => "allowed_once",
            Self::Executing => "executing",
            Self::Denied => "denied",
            Self::Consumed => "consumed",
        }
    }
}

impl FromStr for ApprovalStatus {
    type Err = DomainError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "pending" => Ok(Self::Pending),
            "allowed_once" => Ok(Self::AllowedOnce),
            "executing" => Ok(Self::Executing),
            "denied" => Ok(Self::Denied),
            "consumed" => Ok(Self::Consumed),
            _ => Err(DomainError::InvalidStatus(format!(
                "Invalid status: {}",
                value
            ))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovalRequest {
    pub id: Uuid,
    pub scope: ExecutionScope,
    pub tool_name: String,
    pub action_description: String,
    pub intent: ActionIntentKind,
    pub status: ApprovalStatus,
    #[serde(default)]
    pub author_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub decided_at: Option<DateTime<Utc>>,
    pub decided_by: Option<String>,
    pub consumed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolResult {
    pub call_id: String,
    pub tool_name: String,
    pub content: String,
    pub is_error: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: MessageRole,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

impl ChatMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::System,
            content: content.into(),
            tool_calls: None,
            tool_call_id: None,
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::User,
            content: content.into(),
            tool_calls: None,
            tool_call_id: None,
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::Assistant,
            content: content.into(),
            tool_calls: None,
            tool_call_id: None,
        }
    }

    pub fn assistant_tool_calls(content: impl Into<String>, tool_calls: Vec<ToolCall>) -> Self {
        Self {
            role: MessageRole::Assistant,
            content: content.into(),
            tool_calls: Some(tool_calls),
            tool_call_id: None,
        }
    }

    pub fn tool(call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::Tool,
            content: content.into(),
            tool_calls: None,
            tool_call_id: Some(call_id.into()),
        }
    }

    pub fn estimated_bytes(&self) -> usize {
        let mut total = self.content.len() + 16;
        if let Some(ref id) = self.tool_call_id {
            total += id.len();
        }
        if let Some(ref calls) = self.tool_calls {
            for call in calls {
                total += call.id.len() + call.name.len() + call.arguments.to_string().len();
            }
        }
        total
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryLimits {
    pub max_messages: usize,
    pub max_bytes: usize,
}

impl Default for HistoryLimits {
    fn default() -> Self {
        Self {
            max_messages: 64,
            max_bytes: 512 * 1024,
        }
    }
}

pub fn compact_history(messages: &[ChatMessage], limits: &HistoryLimits) -> Vec<ChatMessage> {
    if messages.is_empty() {
        return Vec::new();
    }

    let mut leading_system = Vec::new();
    let mut idx = 0usize;
    while idx < messages.len() && messages[idx].role == MessageRole::System {
        leading_system.push(messages[idx].clone());
        idx += 1;
    }

    let rest = &messages[idx..];
    if rest.is_empty() {
        return leading_system;
    }

    struct Group {
        messages: Vec<ChatMessage>,
        bytes: usize,
        unanswered: bool,
    }

    let mut groups = Vec::new();
    let mut pos = 0usize;
    while pos < rest.len() {
        let msg = &rest[pos];
        if msg.role == MessageRole::Assistant {
            if let Some(ref calls) = msg.tool_calls {
                if !calls.is_empty() {
                    let expected_ids: Vec<&str> = calls.iter().map(|c| c.id.as_str()).collect();
                    let mut group_msgs = vec![msg.clone()];
                    let mut bytes = msg.estimated_bytes();
                    let mut matched = 0usize;
                    pos += 1;
                    while pos < rest.len() && rest[pos].role == MessageRole::Tool {
                        let belongs = rest[pos]
                            .tool_call_id
                            .as_deref()
                            .is_some_and(|id| expected_ids.contains(&id));
                        if belongs {
                            matched += 1;
                            bytes += rest[pos].estimated_bytes();
                            group_msgs.push(rest[pos].clone());
                            pos += 1;
                        } else {
                            break;
                        }
                    }
                    let unanswered = matched < expected_ids.len();
                    groups.push(Group {
                        messages: group_msgs,
                        bytes,
                        unanswered,
                    });
                    continue;
                }
            }
        }
        groups.push(Group {
            messages: vec![msg.clone()],
            bytes: msg.estimated_bytes(),
            unanswered: false,
        });
        pos += 1;
    }

    let system_count = leading_system.len();
    let system_bytes: usize = leading_system
        .iter()
        .map(ChatMessage::estimated_bytes)
        .sum();

    let first_protected = groups
        .iter()
        .position(|g| g.unanswered)
        .unwrap_or_else(|| groups.len().saturating_sub(1));

    let mut total_msgs: usize =
        system_count + groups.iter().map(|g| g.messages.len()).sum::<usize>();
    let mut total_bytes: usize = system_bytes + groups.iter().map(|g| g.bytes).sum::<usize>();

    let mut start_group = 0usize;
    while start_group < first_protected
        && (total_msgs > limits.max_messages || total_bytes > limits.max_bytes)
    {
        total_msgs -= groups[start_group].messages.len();
        total_bytes -= groups[start_group].bytes;
        start_group += 1;
    }

    let mut out = leading_system;
    for group in groups.into_iter().skip(start_group) {
        out.extend(group.messages);
    }
    out
}
