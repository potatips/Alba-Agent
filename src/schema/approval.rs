use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovalRequestSpec {
    pub id: Uuid,
    pub run_id: Uuid,
    pub tenant_id: Uuid,
    pub workspace_id: Uuid,
    pub tool_name: String,
    pub parameters: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diff_preview: Option<String>,
    pub author_actor: String,
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approver_actor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decision: Option<String>,
}
