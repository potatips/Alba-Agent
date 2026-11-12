use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::workflow::{PropagationScope, SensitivityLevel};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KnowledgeCapsuleSpec {
    pub capsule_id: Uuid,
    pub tenant_id: Uuid,
    pub workspace_id: Uuid,
    pub source_agent_id: String,
    pub source_role: String,
    #[serde(default)]
    pub source_domain_tags: Vec<String>,
    pub trigger_preconditions: String,
    pub distilled_heuristic: String,
    #[serde(default)]
    pub prohibited_contexts: Vec<String>,
    #[serde(default)]
    pub propagation_scope: PropagationScope,
    #[serde(default)]
    pub sensitivity_level: SensitivityLevel,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_workflow_id: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_crew_id: Option<String>,
    #[serde(default = "default_confidence")]
    pub confidence: f64,
}

fn default_confidence() -> f64 {
    0.5
}
