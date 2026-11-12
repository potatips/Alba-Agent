use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentTier {
    Director,
    Lead,
    Specialist,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentSpec {
    pub id: String,
    pub role: String,
    pub goal: String,
    #[serde(default)]
    pub backstory: String,
    pub model: String,
    #[serde(default)]
    pub tool_ids: Vec<String>,
    #[serde(default)]
    pub skill_ids: Vec<String>,
    #[serde(default = "default_max_iterations")]
    pub max_iterations: u32,
    #[serde(default = "default_temperature")]
    pub temperature: f64,
    #[serde(default)]
    pub guardrails: Vec<String>,
    #[serde(default)]
    pub knowledge_capsule_ids: Vec<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tier: Option<AgentTier>,
}

fn default_max_iterations() -> u32 {
    10
}

fn default_temperature() -> f64 {
    0.2
}
