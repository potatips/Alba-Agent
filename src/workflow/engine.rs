use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use chrono::Utc;
use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;

use crate::checkpoints::store::{CheckpointError, CheckpointStore};
use crate::checkpoints::types::WorkflowStepCheckpoint;
use crate::domain::{
    ActionIntentKind, ApprovalRequest, ApprovalStatus, ChatMessage, DomainError, ExecutionScope,
    MessageRole, RunStatus,
};
use crate::events::sink::{EventError, EventSink};
use crate::events::types::{RuntimeEvent, RuntimeEventType};
use crate::governance::policy::ApprovalPolicy;
use crate::models::adapter::ModelClient;
use crate::models::types::{ModelError, ModelRequest};
use crate::schema::{
    ApprovalGateNodeSpec, CrewNodeSpec, CrewProcess, EndNodeSpec, ParallelBranchNodeSpec,
    RouterNodeSpec, StartNodeSpec, TaskSpec, WorkflowNode, WorkflowSpec,
};
use crate::tools::types::{ToolError, ToolRegistry};
use crate::workflow::state::{WorkflowExecutionContext, WorkflowNodeState, WorkflowRunState};
use crate::workflow::validation::{validate_workflow_spec, WorkflowValidationError};

#[derive(Error, Debug)]
pub enum WorkflowExecutionError {
    #[error("Domain error: {0}")]
    Domain(#[from] DomainError),
    #[error("Checkpoint error: {0}")]
    Checkpoint(#[from] CheckpointError),
    #[error("Event error: {0}")]
    Event(#[from] EventError),
    #[error("Validation error: {0}")]
    Validation(#[from] WorkflowValidationError),
    #[error("Model error: {0}")]
    Model(#[from] ModelError),
    #[error("Tool error: {0}")]
    Tool(#[from] ToolError),
    #[error("Node not found: {0}")]
    NodeNotFound(String),
    #[error("Agent not found: {0}")]
    AgentNotFound(String),
    #[error("Task not found: {0}")]
    TaskNotFound(String),
    #[error("Approval error: {0}")]
    Approval(String),
    #[error("Loop limit exceeded for router '{0}' ({1} iterations)")]
    LoopLimitExceeded(String, u32),
    #[error("Execution failed: {0}")]
    ExecutionFailed(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkflowOutcome {
    Completed(Value),
    AwaitingApproval {
        node_id: String,
        approval_id: Uuid,
        action_description: String,
    },
    Paused {
        node_id: String,
    },
    Failed(String),
    Cancelled,
}

pub struct WorkflowExecutor<M, S, E, P>
where
    M: ModelClient,
    S: CheckpointStore,
    E: EventSink,
    P: ApprovalPolicy,
{
    model_client: Arc<M>,
    checkpoint_store: Arc<S>,
    event_sink: Arc<E>,
    approval_policy: Arc<P>,
    tool_registry: Arc<ToolRegistry>,
    workspace_root: PathBuf,
    author_id: Option<String>,
}

impl<M, S, E, P> WorkflowExecutor<M, S, E, P>
where
    M: ModelClient + 'static,
    S: CheckpointStore + 'static,
    E: EventSink + 'static,
    P: ApprovalPolicy + 'static,
{
    pub fn new(
        model_client: Arc<M>,
        checkpoint_store: Arc<S>,
        event_sink: Arc<E>,
        approval_policy: Arc<P>,
        tool_registry: ToolRegistry,
        workspace_root: impl Into<PathBuf>,
    ) -> Self {
        Self {
            model_client,
            checkpoint_store,
            event_sink,
            approval_policy,
            tool_registry: Arc::new(tool_registry),
            workspace_root: workspace_root.into(),
            author_id: None,
        }
    }

    pub fn with_author_id(mut self, author_id: impl Into<String>) -> Self {
        self.author_id = Some(author_id.into());
        self
    }

    pub fn workspace_root(&self) -> &std::path::Path {
        &self.workspace_root
    }

    pub fn approval_policy(&self) -> &P {
        &self.approval_policy
    }

    pub async fn start_workflow(
        &self,
        spec: &WorkflowSpec,
        scope: ExecutionScope,
        initial_input: Option<Value>,
    ) -> Result<WorkflowOutcome, WorkflowExecutionError> {
        validate_workflow_spec(spec)?;

        if self.checkpoint_store.is_cancelled(&scope).await? {
            return Ok(WorkflowOutcome::Cancelled);
        }

        let start_node = spec
            .nodes
            .iter()
            .find(|n| matches!(n, WorkflowNode::Start(_)))
            .ok_or(WorkflowExecutionError::Validation(
                WorkflowValidationError::MissingStartNode,
            ))?;

        let mut node_states = BTreeMap::new();
        for node in &spec.nodes {
            node_states.insert(node.id().to_string(), WorkflowNodeState::Pending);
        }

        let mut context = WorkflowExecutionContext {
            run_id: scope.run_id,
            current_node_id: start_node.id().to_string(),
            run_state: WorkflowRunState::Running,
            node_states,
            completed_nodes: Vec::new(),
            task_outputs: BTreeMap::new(),
            node_outputs: BTreeMap::new(),
            loop_counts: BTreeMap::new(),
            pending_approval_id: None,
            pending_approval_node_id: None,
            author_id: self.author_id.clone(),
            sequence: 1,
            final_output: None,
            error: None,
            updated_at: Utc::now(),
        };

        if let Some(input) = initial_input {
            context
                .node_outputs
                .insert(start_node.id().to_string(), input);
        }

        self.save_checkpoint_step(scope, &context).await?;
        self.execute_flow(spec, scope, &mut context).await
    }

    pub async fn resume_workflow(
        &self,
        spec: &WorkflowSpec,
        scope: ExecutionScope,
    ) -> Result<WorkflowOutcome, WorkflowExecutionError> {
        validate_workflow_spec(spec)?;

        let latest = self
            .checkpoint_store
            .get_latest_workflow_step(&scope)
            .await?
            .ok_or_else(|| {
                WorkflowExecutionError::ExecutionFailed(format!(
                    "No workflow checkpoints found for run {}",
                    scope.run_id
                ))
            })?;

        let mut context: WorkflowExecutionContext =
            serde_json::from_value(latest.state_json.clone()).map_err(|e| {
                WorkflowExecutionError::ExecutionFailed(format!("Corrupted state JSON: {}", e))
            })?;

        if self.checkpoint_store.is_cancelled(&scope).await? {
            context.run_state = WorkflowRunState::Cancelled;
            context.sequence += 1;
            context.updated_at = Utc::now();
            self.save_checkpoint_step(scope, &context).await?;
            return Ok(WorkflowOutcome::Cancelled);
        }

        match context.run_state {
            WorkflowRunState::Completed => {
                return Ok(WorkflowOutcome::Completed(
                    context.final_output.clone().unwrap_or(Value::Null),
                ));
            }
            WorkflowRunState::Failed => {
                return Ok(WorkflowOutcome::Failed(
                    context
                        .error
                        .clone()
                        .unwrap_or_else(|| "Workflow previously failed".to_string()),
                ));
            }
            WorkflowRunState::Cancelled => {
                return Ok(WorkflowOutcome::Cancelled);
            }
            WorkflowRunState::AwaitingApproval | WorkflowRunState::Paused => {
                let approval_id = context.pending_approval_id.ok_or_else(|| {
                    WorkflowExecutionError::Approval(
                        "Workflow in awaiting_approval state but missing pending_approval_id"
                            .to_string(),
                    )
                })?;

                let approval = self
                    .checkpoint_store
                    .get_approval(&scope, approval_id)
                    .await?
                    .ok_or_else(|| {
                        WorkflowExecutionError::Approval(format!(
                            "Approval {} not found in store",
                            approval_id
                        ))
                    })?;

                match approval.status {
                    ApprovalStatus::Pending => {
                        return Ok(WorkflowOutcome::AwaitingApproval {
                            node_id: context.current_node_id.clone(),
                            approval_id,
                            action_description: approval.action_description,
                        });
                    }
                    ApprovalStatus::Denied => {
                        context.run_state = WorkflowRunState::Failed;
                        context
                            .node_states
                            .insert(context.current_node_id.clone(), WorkflowNodeState::Failed);
                        context.error = Some("Workflow approval denied by operator".to_string());
                        context.sequence += 1;
                        context.updated_at = Utc::now();
                        self.save_checkpoint_step(scope, &context).await?;
                        self.event_sink.emit(RuntimeEvent::new(
                            scope,
                            RuntimeEventType::ApprovalResolved,
                            serde_json::json!({
                                "approval_id": approval_id.to_string(),
                                "decision": "deny"
                            }),
                        ))?;
                        return Ok(WorkflowOutcome::Failed(
                            "Workflow approval denied by operator".to_string(),
                        ));
                    }
                    ApprovalStatus::AllowedOnce => {
                        let executing_step = WorkflowStepCheckpoint {
                            id: Uuid::new_v4(),
                            scope,
                            sequence: context.sequence + 1,
                            node_id: context.current_node_id.clone(),
                            status: RunStatus::Executing,
                            state_json: serde_json::to_value(&context).map_err(|e| {
                                WorkflowExecutionError::ExecutionFailed(e.to_string())
                            })?,
                            pending_approval_id: Some(approval_id),
                            created_at: Utc::now(),
                        };

                        let claimed = self
                            .checkpoint_store
                            .claim_workflow_approval(&scope, approval_id, &executing_step)
                            .await?;

                        if !claimed {
                            return Err(WorkflowExecutionError::Approval(
                                "Approval already claimed or consumed".to_string(),
                            ));
                        }

                        let completed = self
                            .checkpoint_store
                            .complete_approval_execution(&scope, approval_id)
                            .await?;

                        if !completed {
                            return Err(WorkflowExecutionError::Approval(
                                "Failed to mark approval consumed".to_string(),
                            ));
                        }

                        self.event_sink.emit(RuntimeEvent::new(
                            scope,
                            RuntimeEventType::ApprovalResolved,
                            serde_json::json!({
                                "approval_id": approval_id.to_string(),
                                "decision": "allow_once"
                            }),
                        ))?;

                        context.node_states.insert(
                            context.current_node_id.clone(),
                            WorkflowNodeState::Completed,
                        );
                        context
                            .completed_nodes
                            .push(context.current_node_id.clone());
                        context.pending_approval_id = None;
                        context.pending_approval_node_id = None;
                        context.run_state = WorkflowRunState::Running;
                        context.sequence += 2;
                        context.updated_at = Utc::now();

                        let next = self.find_next_node(spec, &context.current_node_id, None);
                        if let Some((next_node, _)) = next {
                            context.current_node_id = next_node.id().to_string();
                        }

                        self.save_checkpoint_step(scope, &context).await?;
                    }
                    ApprovalStatus::Executing => {
                        return Err(WorkflowExecutionError::Approval(
                            "Approval crashed during execution, failing closed".to_string(),
                        ));
                    }
                    ApprovalStatus::Consumed => {
                        return Err(WorkflowExecutionError::Approval(
                            "Approval already consumed".to_string(),
                        ));
                    }
                }
            }
            WorkflowRunState::Pending | WorkflowRunState::Running => {}
        }

        self.execute_flow(spec, scope, &mut context).await
    }

    async fn execute_flow(
        &self,
        spec: &WorkflowSpec,
        scope: ExecutionScope,
        context: &mut WorkflowExecutionContext,
    ) -> Result<WorkflowOutcome, WorkflowExecutionError> {
        loop {
            if self.checkpoint_store.is_cancelled(&scope).await? {
                context.run_state = WorkflowRunState::Cancelled;
                context.sequence += 1;
                context.updated_at = Utc::now();
                self.save_checkpoint_step(scope, context).await?;
                return Ok(WorkflowOutcome::Cancelled);
            }

            match context.run_state {
                WorkflowRunState::Completed => {
                    return Ok(WorkflowOutcome::Completed(
                        context.final_output.clone().unwrap_or(Value::Null),
                    ));
                }
                WorkflowRunState::Failed => {
                    return Ok(WorkflowOutcome::Failed(
                        context
                            .error
                            .clone()
                            .unwrap_or_else(|| "Execution failed".to_string()),
                    ));
                }
                WorkflowRunState::Cancelled => {
                    return Ok(WorkflowOutcome::Cancelled);
                }
                WorkflowRunState::AwaitingApproval => {
                    let approval_id = context.pending_approval_id.unwrap_or_else(Uuid::new_v4);
                    return Ok(WorkflowOutcome::AwaitingApproval {
                        node_id: context.current_node_id.clone(),
                        approval_id,
                        action_description: format!("Node {}", context.current_node_id),
                    });
                }
                WorkflowRunState::Paused => {
                    return Ok(WorkflowOutcome::Paused {
                        node_id: context.current_node_id.clone(),
                    });
                }
                WorkflowRunState::Pending | WorkflowRunState::Running => {}
            }

            let current_node = spec
                .nodes
                .iter()
                .find(|n| n.id() == context.current_node_id)
                .ok_or_else(|| {
                    WorkflowExecutionError::NodeNotFound(context.current_node_id.clone())
                })?;

            match current_node {
                WorkflowNode::Start(start_spec) => {
                    self.execute_start_node(spec, scope, context, start_spec)
                        .await?;
                }
                WorkflowNode::Crew(crew_spec) => {
                    self.execute_crew_node(spec, scope, context, crew_spec)
                        .await?;
                }
                WorkflowNode::Approval(gate_spec) => {
                    return self
                        .execute_approval_gate_node(scope, context, gate_spec)
                        .await;
                }
                WorkflowNode::Router(router_spec) => {
                    self.execute_router_node(spec, scope, context, router_spec)
                        .await?;
                }
                WorkflowNode::ParallelBranch(branch_spec) => {
                    self.execute_parallel_branch_node(spec, scope, context, branch_spec)
                        .await?;
                }
                WorkflowNode::End(end_spec) => {
                    return self.execute_end_node(scope, context, end_spec).await;
                }
                WorkflowNode::Rag(rag_spec) => {
                    return Err(WorkflowExecutionError::ExecutionFailed(format!(
                        "RAG node '{}' is not supported in v1",
                        rag_spec.id
                    )));
                }
                WorkflowNode::Sandbox(sandbox_spec) => {
                    return Err(WorkflowExecutionError::ExecutionFailed(format!(
                        "Sandbox node '{}' is not supported in v1",
                        sandbox_spec.id
                    )));
                }
            }
        }
    }

    async fn execute_start_node(
        &self,
        spec: &WorkflowSpec,
        scope: ExecutionScope,
        context: &mut WorkflowExecutionContext,
        start_spec: &StartNodeSpec,
    ) -> Result<(), WorkflowExecutionError> {
        context
            .node_states
            .insert(start_spec.id.clone(), WorkflowNodeState::Completed);
        context.completed_nodes.push(start_spec.id.clone());

        let next = self.find_next_node(spec, &start_spec.id, None);
        if let Some((next_node, _)) = next {
            context.current_node_id = next_node.id().to_string();
            context.sequence += 1;
            context.updated_at = Utc::now();
            self.save_checkpoint_step(scope, context).await?;
            self.event_sink.emit(
                RuntimeEvent::new(
                    scope,
                    RuntimeEventType::FlowStateChanged,
                    serde_json::json!({
                        "node_id": start_spec.id,
                        "status": "completed",
                        "next_node": next_node.id()
                    }),
                )
                .with_node(&start_spec.id),
            )?;
        } else {
            return Err(WorkflowExecutionError::ExecutionFailed(format!(
                "Start node '{}' has no outgoing edges",
                start_spec.id
            )));
        }

        Ok(())
    }

    async fn execute_crew_node(
        &self,
        spec: &WorkflowSpec,
        scope: ExecutionScope,
        context: &mut WorkflowExecutionContext,
        crew_spec: &CrewNodeSpec,
    ) -> Result<(), WorkflowExecutionError> {
        context
            .node_states
            .insert(crew_spec.id.clone(), WorkflowNodeState::Running);
        context.sequence += 1;
        context.updated_at = Utc::now();
        self.save_checkpoint_step(scope, context).await?;

        match crew_spec.process {
            CrewProcess::Sequential => {
                self.execute_sequential_crew(scope, context, crew_spec)
                    .await?;
            }
            CrewProcess::Hierarchical => {
                self.execute_hierarchical_crew(scope, context, crew_spec)
                    .await?;
            }
        }

        context
            .node_states
            .insert(crew_spec.id.clone(), WorkflowNodeState::Completed);
        context.completed_nodes.push(crew_spec.id.clone());

        let crew_output = serde_json::to_value(&context.task_outputs).unwrap_or(Value::Null);
        context
            .node_outputs
            .insert(crew_spec.id.clone(), crew_output);

        let next = self.find_next_node(spec, &crew_spec.id, None);
        if let Some((next_node, _)) = next {
            context.current_node_id = next_node.id().to_string();
            context.sequence += 1;
            context.updated_at = Utc::now();
            self.save_checkpoint_step(scope, context).await?;
            self.event_sink.emit(
                RuntimeEvent::new(
                    scope,
                    RuntimeEventType::FlowStateChanged,
                    serde_json::json!({
                        "node_id": crew_spec.id,
                        "status": "completed",
                        "next_node": next_node.id()
                    }),
                )
                .with_node(&crew_spec.id),
            )?;
        } else {
            return Err(WorkflowExecutionError::ExecutionFailed(format!(
                "Crew node '{}' has no outgoing edges",
                crew_spec.id
            )));
        }

        Ok(())
    }

    async fn execute_sequential_crew(
        &self,
        scope: ExecutionScope,
        context: &mut WorkflowExecutionContext,
        crew_spec: &CrewNodeSpec,
    ) -> Result<(), WorkflowExecutionError> {
        for task in &crew_spec.tasks {
            self.execute_crew_task(scope, context, crew_spec, task)
                .await?;
        }
        Ok(())
    }

    async fn execute_hierarchical_crew(
        &self,
        scope: ExecutionScope,
        context: &mut WorkflowExecutionContext,
        crew_spec: &CrewNodeSpec,
    ) -> Result<(), WorkflowExecutionError> {
        let manager_id = crew_spec.manager_agent_id.as_ref().ok_or_else(|| {
            WorkflowExecutionError::ExecutionFailed(
                "Hierarchical crew requires manager_agent_id".to_string(),
            )
        })?;

        let manager = crew_spec
            .agents
            .iter()
            .find(|a| &a.id == manager_id)
            .ok_or_else(|| WorkflowExecutionError::AgentNotFound(manager_id.clone()))?;

        let plan_prompt = format!(
            "You are manager {}. Plan the execution for {} tasks in crew {}.",
            manager.role,
            crew_spec.tasks.len(),
            crew_spec.id
        );
        let req = ModelRequest {
            messages: vec![
                ChatMessage::system(&manager.backstory),
                ChatMessage::user(plan_prompt),
            ],
            tools: vec![],
            temperature: Some(manager.temperature as f32),
            max_tokens: None,
            model_override: Some(manager.model.clone()),
        };
        let _plan_resp = self.model_client.complete(&req).await?;

        for task in &crew_spec.tasks {
            self.execute_crew_task(scope, context, crew_spec, task)
                .await?;
        }

        Ok(())
    }

    async fn execute_crew_task(
        &self,
        scope: ExecutionScope,
        context: &mut WorkflowExecutionContext,
        crew_spec: &CrewNodeSpec,
        task: &TaskSpec,
    ) -> Result<(), WorkflowExecutionError> {
        let agent = crew_spec
            .agents
            .iter()
            .find(|a| a.id == task.agent_id)
            .ok_or_else(|| WorkflowExecutionError::AgentNotFound(task.agent_id.clone()))?;

        let mut messages = Vec::new();
        let system_text = format!(
            "Role: {}\nGoal: {}\nBackstory: {}\nGuardrails: {}",
            agent.role,
            agent.goal,
            agent.backstory,
            agent.guardrails.join("; ")
        );
        messages.push(ChatMessage::system(system_text));

        let mut user_text = format!(
            "Task: {}\nExpected Output: {}\nOutput Schema: {}",
            task.description, task.expected_output, task.output_schema
        );

        if !task.context_task_ids.is_empty() {
            user_text.push_str("\n\nContext from previous tasks:\n");
            for ctx_id in &task.context_task_ids {
                if let Some(prev) = context.task_outputs.get(ctx_id) {
                    user_text.push_str(&format!("- Task {}: {}\n", ctx_id, prev));
                }
            }
        }
        messages.push(ChatMessage::user(user_text));

        let mut iterations = 0;
        while iterations < agent.max_iterations {
            iterations += 1;

            let req = ModelRequest {
                messages: messages.clone(),
                tools: self.tool_registry.definitions(),
                temperature: Some(agent.temperature as f32),
                max_tokens: None,
                model_override: Some(agent.model.clone()),
            };

            let response = self.model_client.complete(&req).await?;

            if response.tool_calls.is_empty() {
                let final_text = response
                    .content
                    .unwrap_or_else(|| "Task completed without text content".to_string());
                context.task_outputs.insert(task.id.clone(), final_text);
                break;
            }

            for tc in response.tool_calls {
                self.event_sink.emit(
                    RuntimeEvent::new(
                        scope,
                        RuntimeEventType::ToolInvoked,
                        serde_json::json!({
                            "tool_call_id": tc.id,
                            "tool_name": tc.name,
                            "arguments": tc.arguments,
                        }),
                    )
                    .with_node(&crew_spec.id),
                )?;

                let tool = self
                    .tool_registry
                    .get(&tc.name)
                    .ok_or_else(|| ToolError::NotFound(tc.name.clone()))?;

                let result = tool.execute(&scope, &tc.arguments).await?;

                self.event_sink.emit(
                    RuntimeEvent::new(
                        scope,
                        RuntimeEventType::ToolCompleted,
                        serde_json::json!({
                            "tool_call_id": tc.id,
                            "tool_name": tc.name,
                            "content": result.clone(),
                            "is_error": false,
                        }),
                    )
                    .with_node(&crew_spec.id),
                )?;

                messages.push(ChatMessage {
                    role: MessageRole::Tool,
                    content: result,
                    tool_calls: None,
                    tool_call_id: Some(tc.id),
                });
            }
        }

        Ok(())
    }

    async fn execute_approval_gate_node(
        &self,
        scope: ExecutionScope,
        context: &mut WorkflowExecutionContext,
        gate_spec: &ApprovalGateNodeSpec,
    ) -> Result<WorkflowOutcome, WorkflowExecutionError> {
        let approval_id = Uuid::new_v4();
        let request = ApprovalRequest {
            id: approval_id,
            scope,
            tool_name: "approval_gate".to_string(),
            action_description: gate_spec.action_description.clone(),
            intent: ActionIntentKind::ExternalAction,
            status: ApprovalStatus::Pending,
            author_id: self.author_id.clone(),
            created_at: Utc::now(),
            decided_at: None,
            decided_by: None,
            consumed_at: None,
        };

        self.checkpoint_store.save_approval(&request).await?;

        context
            .node_states
            .insert(gate_spec.id.clone(), WorkflowNodeState::AwaitingApproval);
        context.run_state = WorkflowRunState::AwaitingApproval;
        context.pending_approval_id = Some(approval_id);
        context.pending_approval_node_id = Some(gate_spec.id.clone());
        context.sequence += 1;
        context.updated_at = Utc::now();
        self.save_checkpoint_step(scope, context).await?;

        self.event_sink.emit(
            RuntimeEvent::new(
                scope,
                RuntimeEventType::ApprovalRequired,
                serde_json::json!({
                    "approval_id": approval_id.to_string(),
                    "node_id": gate_spec.id,
                    "action_description": gate_spec.action_description,
                    "required_role": gate_spec.required_role,
                }),
            )
            .with_node(&gate_spec.id),
        )?;

        Ok(WorkflowOutcome::AwaitingApproval {
            node_id: gate_spec.id.clone(),
            approval_id,
            action_description: gate_spec.action_description.clone(),
        })
    }

    async fn execute_router_node(
        &self,
        spec: &WorkflowSpec,
        scope: ExecutionScope,
        context: &mut WorkflowExecutionContext,
        router_spec: &RouterNodeSpec,
    ) -> Result<(), WorkflowExecutionError> {
        let mut selected_edge = None;

        for route in &router_spec.routes {
            let matches = if let Some(expected) = &route.value {
                let actual = context
                    .task_outputs
                    .get(&route.field_path)
                    .map(|s| Value::String(s.clone()))
                    .or_else(|| context.node_outputs.get(&route.field_path).cloned());
                actual.as_ref() == Some(expected)
            } else {
                false
            };

            if matches {
                selected_edge = spec.edges.iter().find(|e| {
                    e.source == router_spec.id
                        && (e.route.as_deref() == Some(&route.name) || e.target == route.name)
                });
                if selected_edge.is_some() {
                    break;
                }
            }
        }

        let edge = match selected_edge {
            Some(e) => e,
            None => spec
                .edges
                .iter()
                .find(|e| {
                    e.source == router_spec.id
                        && (e.route.as_deref() == Some(&router_spec.default_route)
                            || e.target == router_spec.default_route)
                })
                .ok_or_else(|| {
                    WorkflowExecutionError::ExecutionFailed(format!(
                        "Router '{}' default route '{}' edge not found",
                        router_spec.id, router_spec.default_route
                    ))
                })?,
        };

        let target_node_id = edge.target.clone();

        if edge.loop_back {
            let count = context
                .loop_counts
                .entry(router_spec.id.clone())
                .or_insert(0);
            *count += 1;
            if *count > router_spec.max_loop_iterations {
                context.run_state = WorkflowRunState::Failed;
                context.error = Some(format!(
                    "Loop limit exceeded for router '{}' ({})",
                    router_spec.id, router_spec.max_loop_iterations
                ));
                self.save_checkpoint_step(scope, context).await?;
                return Err(WorkflowExecutionError::LoopLimitExceeded(
                    router_spec.id.clone(),
                    router_spec.max_loop_iterations,
                ));
            }
        }

        context
            .node_states
            .insert(router_spec.id.clone(), WorkflowNodeState::Completed);
        context.completed_nodes.push(router_spec.id.clone());
        context.current_node_id = target_node_id;
        context.sequence += 1;
        context.updated_at = Utc::now();
        self.save_checkpoint_step(scope, context).await?;

        Ok(())
    }

    async fn execute_parallel_branch_node(
        &self,
        spec: &WorkflowSpec,
        scope: ExecutionScope,
        context: &mut WorkflowExecutionContext,
        branch_spec: &ParallelBranchNodeSpec,
    ) -> Result<(), WorkflowExecutionError> {
        context
            .node_states
            .insert(branch_spec.id.clone(), WorkflowNodeState::Running);
        context.sequence += 1;
        context.updated_at = Utc::now();
        self.save_checkpoint_step(scope, context).await?;

        for branch_node_id in &branch_spec.branch_nodes {
            let child_node = spec
                .nodes
                .iter()
                .find(|n| n.id() == branch_node_id)
                .ok_or_else(|| WorkflowExecutionError::NodeNotFound(branch_node_id.clone()))?;

            match child_node {
                WorkflowNode::Crew(c) => {
                    self.execute_crew_node(spec, scope, context, c).await?;
                }
                _ => {
                    return Err(WorkflowExecutionError::ExecutionFailed(format!(
                        "Unsupported parallel branch child node '{}'",
                        branch_node_id
                    )));
                }
            }
        }

        context
            .node_states
            .insert(branch_spec.id.clone(), WorkflowNodeState::Completed);
        context.completed_nodes.push(branch_spec.id.clone());

        let next = self.find_next_node(spec, &branch_spec.id, None);
        if let Some((next_node, _)) = next {
            context.current_node_id = next_node.id().to_string();
            context.sequence += 1;
            context.updated_at = Utc::now();
            self.save_checkpoint_step(scope, context).await?;
        }

        Ok(())
    }

    async fn execute_end_node(
        &self,
        scope: ExecutionScope,
        context: &mut WorkflowExecutionContext,
        end_spec: &EndNodeSpec,
    ) -> Result<WorkflowOutcome, WorkflowExecutionError> {
        context
            .node_states
            .insert(end_spec.id.clone(), WorkflowNodeState::Completed);
        context.completed_nodes.push(end_spec.id.clone());

        let final_output = serde_json::to_value(&context.task_outputs).unwrap_or(Value::Null);
        context.final_output = Some(final_output.clone());
        context.run_state = WorkflowRunState::Completed;
        context.sequence += 1;
        context.updated_at = Utc::now();
        self.save_checkpoint_step(scope, context).await?;

        self.event_sink.emit(
            RuntimeEvent::new(
                scope,
                RuntimeEventType::RunCompleted,
                serde_json::json!({
                    "run_id": scope.run_id.to_string(),
                    "final_output": final_output
                }),
            )
            .with_node(&end_spec.id),
        )?;

        Ok(WorkflowOutcome::Completed(final_output))
    }

    fn find_next_node<'a>(
        &self,
        spec: &'a WorkflowSpec,
        current_node_id: &str,
        route_name: Option<&str>,
    ) -> Option<(&'a WorkflowNode, bool)> {
        for edge in &spec.edges {
            if edge.source == current_node_id {
                if let Some(r) = route_name {
                    if edge.route.as_deref() == Some(r) {
                        let target = spec.nodes.iter().find(|n| n.id() == edge.target)?;
                        return Some((target, edge.loop_back));
                    }
                } else if edge.route.is_none() {
                    let target = spec.nodes.iter().find(|n| n.id() == edge.target)?;
                    return Some((target, edge.loop_back));
                }
            }
        }
        None
    }

    async fn save_checkpoint_step(
        &self,
        scope: ExecutionScope,
        context: &WorkflowExecutionContext,
    ) -> Result<(), WorkflowExecutionError> {
        let status = match context.run_state {
            WorkflowRunState::Pending => RunStatus::Queued,
            WorkflowRunState::Running => RunStatus::Running,
            WorkflowRunState::AwaitingApproval => RunStatus::Paused,
            WorkflowRunState::Paused => RunStatus::Paused,
            WorkflowRunState::Completed => RunStatus::Completed,
            WorkflowRunState::Failed => RunStatus::Failed,
            WorkflowRunState::Cancelled => RunStatus::Cancelled,
        };

        let state_json = serde_json::to_value(context)
            .map_err(|e| WorkflowExecutionError::ExecutionFailed(e.to_string()))?;

        let step = WorkflowStepCheckpoint {
            id: Uuid::new_v4(),
            scope,
            sequence: context.sequence,
            node_id: context.current_node_id.clone(),
            status,
            state_json,
            pending_approval_id: context.pending_approval_id,
            created_at: Utc::now(),
        };

        self.checkpoint_store
            .save_workflow_step(&step)
            .await
            .map_err(WorkflowExecutionError::Checkpoint)?;

        Ok(())
    }
}
