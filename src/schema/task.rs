use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskSpec {
    pub id: String,
    pub agent_id: String,
    pub description: String,
    pub expected_output: String,
    pub output_schema: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_schema_json: Option<serde_json::Value>,
    #[serde(default)]
    pub context_task_ids: Vec<String>,
    #[serde(default)]
    pub expected_artifact_paths: Vec<String>,
}
