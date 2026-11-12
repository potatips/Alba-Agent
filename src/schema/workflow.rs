use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

use super::node::WorkflowNode;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NodePosition {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowEdgeSpec {
    pub source: String,
    pub target: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route: Option<String>,
    #[serde(default)]
    pub loop_back: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PathRuleSpec {
    pub pattern: String,
    pub rule: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct WorkflowModesSpec {
    #[serde(default = "default_rigor")]
    pub rigor: String,
    #[serde(default)]
    pub path_rules: Vec<PathRuleSpec>,
    #[serde(default)]
    pub channels: Vec<String>,
}

fn default_rigor() -> String {
    "minimal".to_string()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum PropagationScope {
    #[serde(rename = "AGENT_PRIVATE")]
    AgentPrivate,
    #[serde(rename = "CREW_LOCAL")]
    #[default]
    CrewLocal,
    #[serde(rename = "WORKFLOW_SCOPED")]
    WorkflowScoped,
    #[serde(rename = "WORKSPACE_QUARANTINE")]
    WorkspaceQuarantine,
    #[serde(rename = "WORKSPACE_VERIFIED")]
    WorkspaceVerified,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SensitivityLevel {
    #[serde(rename = "PUBLIC")]
    Public,
    #[serde(rename = "INTERNAL")]
    #[default]
    Internal,
    #[serde(rename = "CONFIDENTIAL_PII_STRIPPED")]
    ConfidentialPiiStripped,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LearningEdgeSpec {
    pub source_agent_id: String,
    pub target_agent_id: String,
    #[serde(default)]
    pub scope: PropagationScope,
    #[serde(default = "default_true")]
    pub allow_heuristics: bool,
    #[serde(default)]
    pub allow_code_patterns: bool,
    #[serde(default)]
    pub require_human_approval: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowSpec {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub workspace_id: Uuid,
    pub name: String,
    #[serde(default = "default_version")]
    pub version: u32,
    pub nodes: Vec<WorkflowNode>,
    #[serde(default)]
    pub edges: Vec<WorkflowEdgeSpec>,
    #[serde(default)]
    pub learning_edges: Vec<LearningEdgeSpec>,
    #[serde(default)]
    pub layout: BTreeMap<String, NodePosition>,
    #[serde(default)]
    pub modes: WorkflowModesSpec,
}

fn default_version() -> u32 {
    1
}
