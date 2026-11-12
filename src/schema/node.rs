use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::agent::AgentSpec;
use super::task::TaskSpec;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum JoinMode {
    #[default]
    Any,
    All,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CrewProcess {
    #[default]
    Sequential,
    Hierarchical,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ConsensusConfigSpec {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_consensus_mode")]
    pub mode: String,
    #[serde(default)]
    pub models: Vec<String>,
    #[serde(default)]
    pub arbiter_model: String,
    #[serde(default = "default_agreement_threshold")]
    pub min_agreement_threshold: f64,
    #[serde(default)]
    pub arena_teams: Vec<String>,
}

fn default_consensus_mode() -> String {
    "consensus".to_string()
}

fn default_agreement_threshold() -> f64 {
    0.85
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StartNodeSpec {
    pub id: String,
    #[serde(default = "default_start_label")]
    pub label: String,
    #[serde(default = "default_trigger_type")]
    pub trigger_type: String,
    #[serde(default)]
    pub input_schema: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schedule: Option<String>,
}

fn default_start_label() -> String {
    "Start".to_string()
}

fn default_trigger_type() -> String {
    "manual".to_string()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrewNodeSpec {
    pub id: String,
    pub label: String,
    pub agents: Vec<AgentSpec>,
    pub tasks: Vec<TaskSpec>,
    #[serde(default)]
    pub process: CrewProcess,
    #[serde(default)]
    pub consensus_config: ConsensusConfigSpec,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manager_agent_id: Option<String>,
    #[serde(default)]
    pub join: JoinMode,
    #[serde(default)]
    pub expected_artifact_paths: Vec<String>,
    #[serde(default)]
    pub requires_visual_evidence: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NexusGenSandboxNodeSpec {
    pub id: String,
    pub label: String,
    pub coder_model: String,
    pub reviewer_model: String,
    #[serde(default = "default_sandbox_provider")]
    pub sandbox_provider: String,
    #[serde(default)]
    pub coder_instructions: String,
    #[serde(default)]
    pub reviewer_instructions: String,
    #[serde(default)]
    pub tool_ids: Vec<String>,
    #[serde(default)]
    pub skill_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub computer_id: Option<Uuid>,
    #[serde(default = "default_max_rounds")]
    pub max_rounds: u32,
    #[serde(default = "default_timeout_seconds")]
    pub timeout_seconds: u32,
    pub verification_command: String,
    #[serde(default = "default_on_unverified")]
    pub on_unverified: String,
    #[serde(default = "default_on_failure")]
    pub on_failure: String,
    #[serde(default)]
    pub min_added_tests: u32,
    #[serde(default)]
    pub mount_artifacts_from: Vec<String>,
    #[serde(default)]
    pub consensus_config: ConsensusConfigSpec,
    #[serde(default)]
    pub join: JoinMode,
    #[serde(default)]
    pub expected_artifact_paths: Vec<String>,
    #[serde(default)]
    pub requires_visual_evidence: bool,
    #[serde(default)]
    pub clean_code_review: bool,
}

fn default_sandbox_provider() -> String {
    "docker".to_string()
}

fn default_max_rounds() -> u32 {
    8
}

fn default_timeout_seconds() -> u32 {
    300
}

fn default_on_unverified() -> String {
    "deliver_labeled".to_string()
}

fn default_on_failure() -> String {
    "fail".to_string()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovalGateNodeSpec {
    pub id: String,
    pub label: String,
    pub action_description: String,
    #[serde(default = "default_required_role")]
    pub required_role: String,
    #[serde(default = "default_approval_timeout")]
    pub timeout_seconds: u32,
    #[serde(default)]
    pub boundary_rules: Vec<String>,
}

fn default_required_role() -> String {
    "admin".to_string()
}

fn default_approval_timeout() -> u32 {
    86400
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RouteSpec {
    pub name: String,
    pub field_path: String,
    pub operator: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RouterNodeSpec {
    pub id: String,
    pub label: String,
    pub routes: Vec<RouteSpec>,
    pub default_route: String,
    #[serde(default)]
    pub artifact_gate: bool,
    #[serde(default)]
    pub max_loop_iterations: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RAGNodeSpec {
    pub id: String,
    pub label: String,
    pub knowledge_base_id: Uuid,
    pub query_template: String,
    #[serde(default = "default_top_k")]
    pub top_k: u32,
    #[serde(default)]
    pub mcp_server_ids: Vec<String>,
}

fn default_top_k() -> u32 {
    5
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WorkflowNode {
    Start(StartNodeSpec),
    Crew(CrewNodeSpec),
    Sandbox(Box<NexusGenSandboxNodeSpec>),
    Approval(ApprovalGateNodeSpec),
    Router(RouterNodeSpec),
    Rag(RAGNodeSpec),
}

impl WorkflowNode {
    pub fn id(&self) -> &str {
        match self {
            Self::Start(n) => &n.id,
            Self::Crew(n) => &n.id,
            Self::Sandbox(n) => &n.id,
            Self::Approval(n) => &n.id,
            Self::Router(n) => &n.id,
            Self::Rag(n) => &n.id,
        }
    }
}
