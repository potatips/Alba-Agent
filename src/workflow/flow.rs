use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::task::JoinSet;
use uuid::Uuid;

use crate::checkpoints::store::{CheckpointError, CheckpointStore};
use crate::checkpoints::types::{AuditLogEntry, WorkflowStepCheckpoint};
use crate::domain::{
    ActionIntentKind, ApprovalRequest, ApprovalStatus, DomainError, ExecutionScope, RunStatus,
};
use crate::events::sink::{EventError, EventSink};
use crate::events::types::{RuntimeEvent, RuntimeEventType};
use crate::governance::policy::ApprovalPolicy;
use crate::sandbox::{SandboxBackendKind, SandboxPolicy, SandboxRunner, SandboxStreamChunk};
use crate::tools::types::ToolRegistry;

#[derive(Error, Debug)]
pub enum FlowError {
    #[error("Domain error: {0}")]
    Domain(#[from] DomainError),
    #[error("Checkpoint error: {0}")]
    Checkpoint(#[from] CheckpointError),
    #[error("Event error: {0}")]
    Event(#[from] EventError),
    #[error("Invalid workflow graph: {0}")]
    InvalidGraph(String),
    #[error("Approval {0} already consumed or invalid")]
    ApprovalAlreadyConsumed(Uuid),
    #[error("Workflow execution failed: {0}")]
    ExecutionFailed(String),
    #[error("Maximum workflow step limit ({0}) exceeded")]
    StepLimitExceeded(u32),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowRouteRule {
    pub state_key: String,
    pub equals: serde_json::Value,
    pub target_node_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FlowNodeKind {
    Start,
    Transform {
        assignments: BTreeMap<String, serde_json::Value>,
    },
    ToolTask {
        tool_name: String,
        arguments: serde_json::Value,
        output_key: String,
    },
    CommandTask {
        program: String,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default)]
        allow_network: bool,
        output_key: String,
    },
    ParallelBranch {
        branch_nodes: Vec<String>,
        output_key: String,
    },
    Router {
        routes: Vec<FlowRouteRule>,
        default_node_id: String,
    },
    ApprovalGate {
        description: String,
        #[serde(default = "default_gate_intent")]
        intent_kind: ActionIntentKind,
    },
    End {
        output_key: Option<String>,
    },
}

fn default_gate_intent() -> ActionIntentKind {
    ActionIntentKind::ExternalAction
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowNode {
    pub id: String,
    pub kind: FlowNodeKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowEdge {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowSpec {
    pub id: Uuid,
    pub name: String,
    pub start_node_id: String,
    pub nodes: BTreeMap<String, FlowNode>,
    pub edges: Vec<FlowEdge>,
    #[serde(default = "default_max_steps")]
    pub max_steps: u32,
}

fn default_max_steps() -> u32 {
    64
}

impl FlowSpec {
    pub fn new(id: Uuid, name: impl Into<String>, start_node_id: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            start_node_id: start_node_id.into(),
            nodes: BTreeMap::new(),
            edges: Vec::new(),
            max_steps: default_max_steps(),
        }
    }

    pub fn add_node(&mut self, node: FlowNode) {
        self.nodes.insert(node.id.clone(), node);
    }

    pub fn add_edge(&mut self, from: impl Into<String>, to: impl Into<String>) {
        self.edges.push(FlowEdge {
            from: from.into(),
            to: to.into(),
        });
    }

    pub fn validate(&self) -> Result<(), FlowError> {
        if self.max_steps == 0 {
            return Err(FlowError::InvalidGraph(
                "max_steps must be greater than 0".to_string(),
            ));
        }
        if !self.nodes.contains_key(&self.start_node_id) {
            return Err(FlowError::InvalidGraph(format!(
                "Start node '{}' does not exist",
                self.start_node_id
            )));
        }
        for (key, node) in &self.nodes {
            if key != &node.id {
                return Err(FlowError::InvalidGraph(format!(
                    "Node key '{}' does not match node id '{}'",
                    key, node.id
                )));
            }
            match &node.kind {
                FlowNodeKind::Router {
                    routes,
                    default_node_id,
                } => {
                    if !self.nodes.contains_key(default_node_id) {
                        return Err(FlowError::InvalidGraph(format!(
                            "Router '{}' default target '{}' does not exist",
                            node.id, default_node_id
                        )));
                    }
                    for rule in routes {
                        if !self.nodes.contains_key(&rule.target_node_id) {
                            return Err(FlowError::InvalidGraph(format!(
                                "Router '{}' route target '{}' does not exist",
                                node.id, rule.target_node_id
                            )));
                        }
                    }
                }
                FlowNodeKind::ParallelBranch { branch_nodes, .. } => {
                    if branch_nodes.is_empty() {
                        return Err(FlowError::InvalidGraph(format!(
                            "ParallelBranch '{}' must contain at least one branch node",
                            node.id
                        )));
                    }
                    for b in branch_nodes {
                        match self.nodes.get(b) {
                            None => {
                                return Err(FlowError::InvalidGraph(format!(
                                    "ParallelBranch '{}' references missing node '{}'",
                                    node.id, b
                                )));
                            }
                            Some(target) => match target.kind {
                                FlowNodeKind::ToolTask { .. }
                                | FlowNodeKind::CommandTask { .. }
                                | FlowNodeKind::Transform { .. } => {}
                                _ => {
                                    return Err(FlowError::InvalidGraph(format!(
                                        "ParallelBranch '{}' child '{}' must be ToolTask, CommandTask, or Transform",
                                        node.id, b
                                    )));
                                }
                            },
                        }
                    }
                }
                _ => {}
            }
        }
        for edge in &self.edges {
            if !self.nodes.contains_key(&edge.from) || !self.nodes.contains_key(&edge.to) {
                return Err(FlowError::InvalidGraph(format!(
                    "Edge '{}' -> '{}' references missing node",
                    edge.from, edge.to
                )));
            }
        }
        Ok(())
    }

    fn next_linear_node(&self, from: &str) -> Option<String> {
        self.edges
            .iter()
            .find(|e| e.from == from)
            .map(|e| e.to.clone())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowState {
    pub next_node_id: Option<String>,
    pub completed_nodes: Vec<String>,
    pub variables: BTreeMap<String, serde_json::Value>,
    pub sequence: u32,
    pub status: RunStatus,
    pub pending_approval_id: Option<Uuid>,
    pub author_id: Option<String>,
    pub final_output: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlowOutcome {
    Completed(serde_json::Value),
    Paused {
        node_id: String,
        approval_id: Uuid,
        description: String,
    },
    Failed(String),
    Cancelled,
}

pub struct FlowExecutor<S, E, P>
where
    S: CheckpointStore,
    E: EventSink,
    P: ApprovalPolicy,
{
    checkpoint_store: Arc<S>,
    event_sink: Arc<E>,
    approval_policy: Arc<P>,
    tool_registry: Arc<ToolRegistry>,
    workspace_root: PathBuf,
    sandbox_backend: SandboxBackendKind,
    author_id: Option<String>,
}

impl<S, E, P> FlowExecutor<S, E, P>
where
    S: CheckpointStore + 'static,
    E: EventSink + 'static,
    P: ApprovalPolicy + 'static,
{
    pub fn new(
        checkpoint_store: Arc<S>,
        event_sink: Arc<E>,
        approval_policy: Arc<P>,
        tool_registry: ToolRegistry,
        workspace_root: impl Into<PathBuf>,
    ) -> Self {
        Self {
            checkpoint_store,
            event_sink,
            approval_policy,
            tool_registry: Arc::new(tool_registry),
            workspace_root: workspace_root.into(),
            sandbox_backend: SandboxBackendKind::Auto,
            author_id: None,
        }
    }

    pub fn with_sandbox_backend(mut self, backend: SandboxBackendKind) -> Self {
        self.sandbox_backend = backend;
        self
    }

    pub fn with_author_id(mut self, author_id: impl Into<String>) -> Self {
        self.author_id = Some(author_id.into());
        self
    }

    pub async fn start_flow(
        &self,
        spec: &FlowSpec,
        scope: ExecutionScope,
        initial_vars: BTreeMap<String, serde_json::Value>,
    ) -> Result<FlowOutcome, FlowError> {
        spec.validate()?;

        if self.checkpoint_store.is_cancelled(&scope).await? {
            return Ok(FlowOutcome::Cancelled);
        }

        let mut state = FlowState {
            next_node_id: Some(spec.start_node_id.clone()),
            completed_nodes: Vec::new(),
            variables: initial_vars,
            sequence: 1,
            status: RunStatus::Running,
            pending_approval_id: None,
            author_id: self.author_id.clone(),
            final_output: None,
        };

        self.persist_step(scope, &spec.start_node_id, &state)
            .await?;

        self.run_steps(spec, scope, &mut state).await
    }

    pub async fn resume_flow(
        &self,
        spec: &FlowSpec,
        scope: ExecutionScope,
    ) -> Result<FlowOutcome, FlowError> {
        spec.validate()?;

        let latest = self
            .checkpoint_store
            .get_latest_workflow_step(&scope)
            .await?
            .ok_or_else(|| {
                FlowError::InvalidGraph(format!(
                    "No workflow checkpoints found for run {}",
                    scope.run_id
                ))
            })?;

        let mut state: FlowState = serde_json::from_value(latest.state_json.clone())
            .map_err(|e| FlowError::ExecutionFailed(format!("Corrupt workflow state: {}", e)))?;

        if state.status.is_terminal() {
            return match state.status {
                RunStatus::Completed => Ok(FlowOutcome::Completed(
                    state.final_output.unwrap_or(serde_json::Value::Null),
                )),
                RunStatus::Cancelled => Ok(FlowOutcome::Cancelled),
                RunStatus::Failed => Ok(FlowOutcome::Failed("Workflow already failed".to_string())),
                _ => unreachable!(),
            };
        }

        if self.checkpoint_store.is_cancelled(&scope).await? {
            state.status = RunStatus::Cancelled;
            state.sequence += 1;
            self.persist_step(scope, &latest.node_id, &state).await?;
            return Ok(FlowOutcome::Cancelled);
        }

        if state.status == RunStatus::Paused {
            let approval_id = latest.pending_approval_id.ok_or_else(|| {
                FlowError::ExecutionFailed(
                    "Paused workflow missing pending_approval_id".to_string(),
                )
            })?;

            let approval = self
                .checkpoint_store
                .get_approval(&scope, approval_id)
                .await?
                .ok_or_else(|| {
                    FlowError::ExecutionFailed(format!("Approval {} not found", approval_id))
                })?;

            if approval.status == ApprovalStatus::Pending {
                return Ok(FlowOutcome::Paused {
                    node_id: latest.node_id,
                    approval_id,
                    description: approval.action_description,
                });
            }

            if approval.status == ApprovalStatus::Denied {
                state.status = RunStatus::Failed;
                state.pending_approval_id = None;
                state.sequence += 1;
                self.persist_step(scope, &latest.node_id, &state).await?;
                self.event_sink.emit(RuntimeEvent::new(
                    scope,
                    RuntimeEventType::ApprovalResolved,
                    serde_json::json!({
                        "approval_id": approval_id.to_string(),
                        "decision": "deny"
                    }),
                ))?;
                return Ok(FlowOutcome::Failed(
                    "Workflow approval denied by human operator".to_string(),
                ));
            }

            if approval.status != ApprovalStatus::AllowedOnce {
                return Err(FlowError::ApprovalAlreadyConsumed(approval_id));
            }

            let mut executing_state = state.clone();
            executing_state.status = RunStatus::Executing;
            executing_state.sequence = state.sequence + 1;
            let executing_cp = WorkflowStepCheckpoint {
                id: Uuid::new_v4(),
                scope,
                sequence: executing_state.sequence,
                node_id: latest.node_id.clone(),
                status: RunStatus::Executing,
                state_json: serde_json::to_value(&executing_state)
                    .map_err(|e| FlowError::ExecutionFailed(e.to_string()))?,
                pending_approval_id: Some(approval_id),
                created_at: Utc::now(),
            };

            let claimed = self
                .checkpoint_store
                .claim_workflow_approval(&scope, approval_id, &executing_cp)
                .await?;
            if !claimed {
                return Err(FlowError::ApprovalAlreadyConsumed(approval_id));
            }

            let gated_node = spec.nodes.get(&latest.node_id).ok_or_else(|| {
                FlowError::InvalidGraph(format!("Node '{}' missing from workflow", latest.node_id))
            })?;

            self.event_sink.emit(RuntimeEvent::new(
                scope,
                RuntimeEventType::ApprovalResolved,
                serde_json::json!({
                    "approval_id": approval_id.to_string(),
                    "decision": "allow_once"
                }),
            ))?;

            self.execute_node_body(spec, scope, gated_node, &mut state)
                .await?;

            let completed = self
                .checkpoint_store
                .complete_approval_execution(&scope, approval_id)
                .await?;
            if !completed {
                return Err(FlowError::ApprovalAlreadyConsumed(approval_id));
            }

            self.checkpoint_store
                .record_audit_log(&AuditLogEntry {
                    id: Uuid::new_v4(),
                    scope,
                    actor_id: approval.decided_by.clone(),
                    action: "approval.consumed".to_string(),
                    target_id: Some(approval_id),
                    details: serde_json::json!({
                        "node_id": latest.node_id,
                        "author_id": approval.author_id,
                        "approver_id": approval.decided_by,
                    }),
                    created_at: Utc::now(),
                })
                .await?;

            state.pending_approval_id = None;
            state.status = RunStatus::Running;
            state.sequence = executing_state.sequence + 1;
            state.completed_nodes.push(latest.node_id.clone());
            state.next_node_id = spec.next_linear_node(&latest.node_id);
            self.persist_step(scope, &latest.node_id, &state).await?;
        }

        self.run_steps(spec, scope, &mut state).await
    }

    pub async fn rewind_and_fork_flow(
        &self,
        source_scope: ExecutionScope,
        target_sequence: u32,
        new_run_id: Option<Uuid>,
        state_patch: BTreeMap<String, serde_json::Value>,
    ) -> Result<ExecutionScope, FlowError> {
        let steps = self
            .checkpoint_store
            .list_workflow_steps(&source_scope)
            .await?;
        let target_step = steps
            .into_iter()
            .find(|s| s.sequence == target_sequence)
            .ok_or_else(|| {
                FlowError::ExecutionFailed(format!(
                    "Workflow checkpoint sequence {} not found",
                    target_sequence
                ))
            })?;

        let mut state: FlowState = serde_json::from_value(target_step.state_json)
            .map_err(|e| FlowError::ExecutionFailed(e.to_string()))?;

        for (k, v) in state_patch {
            state.variables.insert(k, v);
        }
        state.pending_approval_id = None;
        state.status = RunStatus::Running;
        state.sequence = 1;

        let forked_scope = ExecutionScope::new(
            source_scope.tenant_id,
            source_scope.workspace_id,
            new_run_id.unwrap_or_else(Uuid::new_v4),
        )?;

        self.persist_step(forked_scope, &target_step.node_id, &state)
            .await?;

        self.event_sink.emit(RuntimeEvent::new(
            forked_scope,
            RuntimeEventType::TrajectoryForked,
            serde_json::json!({
                "parent_run_id": source_scope.run_id.to_string(),
                "forked_run_id": forked_scope.run_id.to_string(),
                "fork_sequence": target_sequence,
                "node_id": target_step.node_id,
            }),
        ))?;

        Ok(forked_scope)
    }

    async fn run_steps(
        &self,
        spec: &FlowSpec,
        scope: ExecutionScope,
        state: &mut FlowState,
    ) -> Result<FlowOutcome, FlowError> {
        let mut steps_executed: u32 = 0;

        while let Some(node_id) = state.next_node_id.clone() {
            if steps_executed >= spec.max_steps {
                state.status = RunStatus::Failed;
                state.sequence += 1;
                self.persist_step(scope, &node_id, state).await?;
                return Err(FlowError::StepLimitExceeded(spec.max_steps));
            }
            steps_executed += 1;

            if self.checkpoint_store.is_cancelled(&scope).await? {
                state.status = RunStatus::Cancelled;
                state.sequence += 1;
                self.persist_step(scope, &node_id, state).await?;
                return Ok(FlowOutcome::Cancelled);
            }

            let node = spec.nodes.get(&node_id).ok_or_else(|| {
                FlowError::InvalidGraph(format!("Node '{}' missing from workflow", node_id))
            })?;

            match &node.kind {
                FlowNodeKind::ApprovalGate {
                    description,
                    intent_kind,
                } => {
                    return self
                        .pause_for_approval(scope, node_id, description, *intent_kind, state)
                        .await;
                }
                FlowNodeKind::ToolTask {
                    tool_name,
                    arguments,
                    ..
                } => {
                    if let Some(intent) = self.approval_policy.check_approval(tool_name, arguments)
                    {
                        return self
                            .pause_for_approval(
                                scope,
                                node_id,
                                &intent.description,
                                intent.kind,
                                state,
                            )
                            .await;
                    }
                }
                FlowNodeKind::CommandTask { program, args, .. } => {
                    let arg_val = serde_json::json!({ "program": program, "args": args });
                    if let Some(intent) =
                        self.approval_policy.check_approval("run_command", &arg_val)
                    {
                        return self
                            .pause_for_approval(
                                scope,
                                node_id,
                                &intent.description,
                                intent.kind,
                                state,
                            )
                            .await;
                    }
                }
                _ => {}
            }

            if let FlowNodeKind::End { output_key } = &node.kind {
                let output = output_key
                    .as_ref()
                    .and_then(|k| state.variables.get(k).cloned())
                    .unwrap_or_else(|| serde_json::to_value(&state.variables).unwrap_or_default());
                state.final_output = Some(output.clone());
                state.status = RunStatus::Completed;
                state.completed_nodes.push(node_id.clone());
                state.next_node_id = None;
                state.sequence += 1;
                self.persist_step(scope, &node_id, state).await?;
                self.event_sink.emit(
                    RuntimeEvent::new(
                        scope,
                        RuntimeEventType::RunCompleted,
                        serde_json::json!({ "node_id": node_id, "output": output }),
                    )
                    .with_node(&node_id),
                )?;
                return Ok(FlowOutcome::Completed(output));
            }

            let next_override = self.execute_node_body(spec, scope, node, state).await?;
            state.completed_nodes.push(node_id.clone());
            state.next_node_id = next_override.or_else(|| spec.next_linear_node(&node_id));
            state.sequence += 1;
            self.persist_step(scope, &node_id, state).await?;
        }

        state.status = RunStatus::Completed;
        let output = serde_json::to_value(&state.variables).unwrap_or_default();
        state.final_output = Some(output.clone());
        Ok(FlowOutcome::Completed(output))
    }

    async fn pause_for_approval(
        &self,
        scope: ExecutionScope,
        node_id: String,
        description: &str,
        intent_kind: ActionIntentKind,
        state: &mut FlowState,
    ) -> Result<FlowOutcome, FlowError> {
        let approval = ApprovalRequest {
            id: Uuid::new_v4(),
            scope,
            tool_name: format!("workflow:{}", node_id),
            action_description: description.to_string(),
            intent: intent_kind,
            status: ApprovalStatus::Pending,
            author_id: state.author_id.clone(),
            created_at: Utc::now(),
            decided_at: None,
            decided_by: None,
            consumed_at: None,
        };

        self.checkpoint_store.save_approval(&approval).await?;
        state.status = RunStatus::Paused;
        state.pending_approval_id = Some(approval.id);
        state.next_node_id = Some(node_id.clone());
        state.sequence += 1;
        self.persist_step(scope, &node_id, state).await?;

        self.event_sink.emit(
            RuntimeEvent::new(
                scope,
                RuntimeEventType::ApprovalRequired,
                serde_json::json!({
                    "approval_id": approval.id.to_string(),
                    "node_id": node_id,
                    "tool_name": approval.tool_name,
                    "action_description": description,
                    "intent_kind": intent_kind.as_str(),
                    "author_id": state.author_id,
                }),
            )
            .with_node(&node_id),
        )?;

        Ok(FlowOutcome::Paused {
            node_id,
            approval_id: approval.id,
            description: description.to_string(),
        })
    }

    async fn execute_node_body(
        &self,
        spec: &FlowSpec,
        scope: ExecutionScope,
        node: &FlowNode,
        state: &mut FlowState,
    ) -> Result<Option<String>, FlowError> {
        match &node.kind {
            FlowNodeKind::Start | FlowNodeKind::ApprovalGate { .. } | FlowNodeKind::End { .. } => {
                Ok(None)
            }
            FlowNodeKind::Transform { assignments } => {
                for (k, v) in assignments {
                    state.variables.insert(k.clone(), v.clone());
                }
                Ok(None)
            }
            FlowNodeKind::Router {
                routes,
                default_node_id,
            } => {
                for rule in routes {
                    if state.variables.get(&rule.state_key) == Some(&rule.equals) {
                        return Ok(Some(rule.target_node_id.clone()));
                    }
                }
                Ok(Some(default_node_id.clone()))
            }
            FlowNodeKind::ToolTask {
                tool_name,
                arguments,
                output_key,
            } => {
                let raw = self
                    .tool_registry
                    .execute(&scope, tool_name, arguments)
                    .await
                    .map_err(|e| FlowError::ExecutionFailed(e.to_string()))?;
                let parsed = serde_json::from_str(&raw).unwrap_or(serde_json::Value::String(raw));
                state.variables.insert(output_key.clone(), parsed);
                Ok(None)
            }
            FlowNodeKind::CommandTask {
                program,
                args,
                allow_network,
                output_key,
            } => {
                let output = self
                    .run_sandboxed_command(scope, &node.id, program, args, *allow_network)
                    .await?;
                let val = serde_json::to_value(output)
                    .map_err(|e| FlowError::ExecutionFailed(e.to_string()))?;
                state.variables.insert(output_key.clone(), val);
                Ok(None)
            }
            FlowNodeKind::ParallelBranch {
                branch_nodes,
                output_key,
            } => {
                let mut set = JoinSet::new();
                for branch_id in branch_nodes {
                    let b_node = spec.nodes.get(branch_id).cloned().ok_or_else(|| {
                        FlowError::InvalidGraph(format!("Parallel branch '{}' missing", branch_id))
                    })?;
                    let reg = Arc::clone(&self.tool_registry);
                    let sink = Arc::clone(&self.event_sink);
                    let ws = self.workspace_root.clone();
                    let backend = self.sandbox_backend;
                    set.spawn(async move {
                        let out = match b_node.kind {
                            FlowNodeKind::ToolTask {
                                tool_name,
                                arguments,
                                ..
                            } => {
                                let raw = reg
                                    .execute(&scope, &tool_name, &arguments)
                                    .await
                                    .map_err(|e| FlowError::ExecutionFailed(e.to_string()))?;
                                serde_json::from_str(&raw).unwrap_or(serde_json::Value::String(raw))
                            }
                            FlowNodeKind::CommandTask {
                                program,
                                args,
                                allow_network,
                                ..
                            } => {
                                let res = execute_sandboxed_with_sink(
                                    scope,
                                    &ws,
                                    backend,
                                    &b_node.id,
                                    &program,
                                    &args,
                                    allow_network,
                                    sink,
                                )
                                .await?;
                                serde_json::to_value(res)
                                    .map_err(|e| FlowError::ExecutionFailed(e.to_string()))?
                            }
                            FlowNodeKind::Transform { assignments } => {
                                serde_json::to_value(assignments)
                                    .map_err(|e| FlowError::ExecutionFailed(e.to_string()))?
                            }
                            _ => {
                                return Err(FlowError::InvalidGraph(format!(
                                    "Unsupported parallel branch node kind for '{}'",
                                    b_node.id
                                )));
                            }
                        };
                        Ok::<(String, serde_json::Value), FlowError>((b_node.id, out))
                    });
                }

                let mut combined = serde_json::Map::new();
                while let Some(res) = set.join_next().await {
                    let (bid, val) = res.map_err(|e| {
                        FlowError::ExecutionFailed(format!("Parallel task panic: {}", e))
                    })??;
                    combined.insert(bid, val);
                }
                state
                    .variables
                    .insert(output_key.clone(), serde_json::Value::Object(combined));
                Ok(None)
            }
        }
    }

    async fn run_sandboxed_command(
        &self,
        scope: ExecutionScope,
        node_id: &str,
        program: &str,
        args: &[String],
        allow_network: bool,
    ) -> Result<crate::sandbox::SandboxOutput, FlowError> {
        execute_sandboxed_with_sink(
            scope,
            &self.workspace_root,
            self.sandbox_backend,
            node_id,
            program,
            args,
            allow_network,
            Arc::clone(&self.event_sink),
        )
        .await
    }

    async fn persist_step(
        &self,
        scope: ExecutionScope,
        node_id: &str,
        state: &FlowState,
    ) -> Result<(), FlowError> {
        let step = WorkflowStepCheckpoint {
            id: Uuid::new_v4(),
            scope,
            sequence: state.sequence,
            node_id: node_id.to_string(),
            status: state.status,
            state_json: serde_json::to_value(state)
                .map_err(|e| FlowError::ExecutionFailed(e.to_string()))?,
            pending_approval_id: state.pending_approval_id,
            created_at: Utc::now(),
        };
        self.checkpoint_store.save_workflow_step(&step).await?;
        self.event_sink.emit(
            RuntimeEvent::new(
                scope,
                RuntimeEventType::FlowStateChanged,
                serde_json::json!({
                    "node_id": node_id,
                    "sequence": state.sequence,
                    "status": state.status.as_str(),
                    "next_node_id": state.next_node_id,
                }),
            )
            .with_node(node_id),
        )?;
        Ok(())
    }
}

#[allow(clippy::too_many_arguments)]
async fn execute_sandboxed_with_sink<E: EventSink + 'static>(
    scope: ExecutionScope,
    workspace_root: &std::path::Path,
    backend: SandboxBackendKind,
    node_id: &str,
    program: &str,
    args: &[String],
    allow_network: bool,
    sink: Arc<E>,
) -> Result<crate::sandbox::SandboxOutput, FlowError> {
    let policy = SandboxPolicy::new(workspace_root)
        .with_network(allow_network)
        .with_timeout(Duration::from_secs(30))
        .with_backend(backend);
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<SandboxStreamChunk>();
    let node_label = node_id.to_string();
    let prog_label = program.to_string();
    let fwd = tokio::spawn(async move {
        while let Some(chunk) = rx.recv().await {
            let _ = sink.emit(
                RuntimeEvent::new(
                    scope,
                    RuntimeEventType::SandboxTerminalOutput,
                    serde_json::json!({
                        "program": prog_label,
                        "stream": chunk.stream.as_str(),
                        "chunk": chunk.chunk,
                    }),
                )
                .with_node(&node_label),
            );
        }
    });
    let output = SandboxRunner::execute(&policy, program, args, Some(tx))
        .await
        .map_err(|e| FlowError::ExecutionFailed(e.to_string()))?;
    let _ = fwd.await;
    Ok(output)
}
