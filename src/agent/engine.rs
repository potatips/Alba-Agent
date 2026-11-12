use std::sync::Arc;

use chrono::Utc;
use thiserror::Error;
use uuid::Uuid;

use crate::agent::state::{AgentState, EngineOutcome};
use crate::checkpoints::store::{CheckpointError, CheckpointStore};
use crate::checkpoints::types::{AuditLogEntry, ForkRunRequest, RunCheckpoint, TrajectoryBranch};
use crate::domain::{
    compact_history, ApprovalRequest, ApprovalStatus, ChatMessage, DomainError, ExecutionScope,
    HistoryLimits, RunStatus,
};
use crate::events::sink::{EventError, EventSink};
use crate::events::types::{RuntimeEvent, RuntimeEventType};
use crate::governance::policy::ApprovalPolicy;
use crate::models::adapter::ModelClient;
use crate::models::types::{ModelError, ModelRequest};
use crate::tools::types::{ToolError, ToolRegistry};

#[derive(Error, Debug)]
pub enum EngineError {
    #[error("Domain error: {0}")]
    Domain(#[from] DomainError),
    #[error("Model error: {0}")]
    Model(#[from] ModelError),
    #[error("Tool error: {0}")]
    Tool(#[from] ToolError),
    #[error("Checkpoint error: {0}")]
    Checkpoint(#[from] CheckpointError),
    #[error("Event error: {0}")]
    Event(#[from] EventError),
    #[error("Approval {0} has already been consumed and cannot be reused")]
    ApprovalAlreadyConsumed(Uuid),
    #[error("Run was cancelled")]
    RunCancelled,
    #[error("Maximum turn limit of {0} exceeded")]
    TurnLimitExceeded(usize),
    #[error("Invalid state: {0}")]
    InvalidState(String),
}

pub struct AgentEngine<M, S, E, P>
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
    tool_registry: ToolRegistry,
    max_turns: usize,
    system_prompt: String,
    history_limits: HistoryLimits,
    author_id: Option<String>,
}

impl<M, S, E, P> AgentEngine<M, S, E, P>
where
    M: ModelClient,
    S: CheckpointStore,
    E: EventSink,
    P: ApprovalPolicy,
{
    pub fn new(
        model_client: Arc<M>,
        checkpoint_store: Arc<S>,
        event_sink: Arc<E>,
        approval_policy: Arc<P>,
        tool_registry: ToolRegistry,
    ) -> Self {
        Self {
            model_client,
            checkpoint_store,
            event_sink,
            approval_policy,
            tool_registry,
            max_turns: 10,
            system_prompt: "You are an action-capable assistant. Use tools when needed. Produce honest final answers.".to_string(),
            history_limits: HistoryLimits::default(),
            author_id: None,
        }
    }

    pub fn with_max_turns(mut self, max_turns: usize) -> Self {
        self.max_turns = max_turns;
        self
    }

    pub fn with_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = prompt.into();
        self
    }

    pub fn with_history_limits(mut self, history_limits: HistoryLimits) -> Self {
        self.history_limits = history_limits;
        self
    }

    pub fn with_author_id(mut self, author_id: impl Into<String>) -> Self {
        self.author_id = Some(author_id.into());
        self
    }

    pub async fn start_run(
        &self,
        scope: ExecutionScope,
        prompt: &str,
    ) -> Result<EngineOutcome, EngineError> {
        self.start_run_as(scope, prompt, self.author_id.as_deref())
            .await
    }

    pub async fn start_run_as(
        &self,
        scope: ExecutionScope,
        prompt: &str,
        author_id: Option<&str>,
    ) -> Result<EngineOutcome, EngineError> {
        if self.checkpoint_store.is_cancelled(&scope).await? {
            self.event_sink.emit(RuntimeEvent::new(
                scope,
                RuntimeEventType::RunFailed,
                serde_json::json!({ "error": "RunCancelled" }),
            ))?;
            return Ok(EngineOutcome::Cancelled);
        }

        let resolved_author = author_id
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string);

        let mut state = AgentState::new(scope);
        state.status = RunStatus::Running;
        state.sequence = 1;
        state.author_id = resolved_author.clone();
        state.history = vec![
            ChatMessage::system(&self.system_prompt),
            ChatMessage::user(prompt),
        ];
        state.history = compact_history(&state.history, &self.history_limits);

        self.save_state_checkpoint(&state).await?;

        self.checkpoint_store
            .record_audit_log(&AuditLogEntry {
                id: Uuid::new_v4(),
                scope,
                actor_id: resolved_author,
                action: "run.started".to_string(),
                target_id: None,
                details: serde_json::json!({ "prompt": prompt }),
                created_at: Utc::now(),
            })
            .await?;

        self.run_loop(&mut state).await
    }

    pub async fn fork_trajectory(
        &self,
        req: ForkRunRequest,
    ) -> Result<(RunCheckpoint, TrajectoryBranch), EngineError> {
        let (cp, branch) = self.checkpoint_store.fork_run(&req).await?;
        self.event_sink.emit(RuntimeEvent::new(
            cp.scope,
            RuntimeEventType::TrajectoryForked,
            serde_json::json!({
                "branch_id": branch.branch_id.to_string(),
                "parent_run_id": branch.parent_run_id.to_string(),
                "forked_run_id": branch.forked_run_id.to_string(),
                "fork_sequence": branch.fork_sequence,
                "branch_label": branch.branch_label,
            }),
        ))?;
        Ok((cp, branch))
    }

    pub async fn resume_run(&self, scope: ExecutionScope) -> Result<EngineOutcome, EngineError> {
        let checkpoint = self
            .checkpoint_store
            .get_latest_checkpoint(&scope)
            .await?
            .ok_or_else(|| {
                EngineError::InvalidState(format!("No checkpoint found for run {}", scope.run_id))
            })?;

        if !checkpoint.scope.matches(&scope) {
            return Err(EngineError::Domain(DomainError::ScopeMismatch {
                expected: scope,
                actual: checkpoint.scope,
            }));
        }

        if checkpoint.status.is_terminal() {
            return match checkpoint.status {
                RunStatus::Completed => Ok(EngineOutcome::Completed(
                    checkpoint
                        .history
                        .last()
                        .map(|m| m.content.clone())
                        .unwrap_or_default(),
                )),
                RunStatus::Cancelled => Ok(EngineOutcome::Cancelled),
                RunStatus::Failed => Ok(EngineOutcome::Failed("Run already failed".to_string())),
                _ => unreachable!(),
            };
        }

        if self.checkpoint_store.is_cancelled(&scope).await? {
            let state = AgentState {
                scope,
                status: RunStatus::Cancelled,
                sequence: checkpoint.sequence + 1,
                history: checkpoint.history,
                active_tool_call: None,
                pending_tool_calls: Vec::new(),
                pending_approval_id: None,
                author_id: checkpoint.author_id,
                model_override: checkpoint.model_override,
                final_response: None,
                error_message: Some("Run cancelled prior to resume".to_string()),
            };
            self.save_state_checkpoint(&state).await?;
            self.event_sink.emit(RuntimeEvent::new(
                scope,
                RuntimeEventType::RunFailed,
                serde_json::json!({ "error": "RunCancelled" }),
            ))?;
            return Ok(EngineOutcome::Cancelled);
        }

        if checkpoint.status == RunStatus::Queued {
            let mut state = AgentState {
                scope,
                status: RunStatus::Running,
                sequence: checkpoint.sequence,
                history: checkpoint.history,
                active_tool_call: None,
                pending_tool_calls: checkpoint.pending_tool_calls,
                pending_approval_id: None,
                author_id: checkpoint.author_id,
                model_override: checkpoint.model_override,
                final_response: None,
                error_message: None,
            };
            if !state.pending_tool_calls.is_empty() {
                if let Some(outcome) = self.drain_pending_tool_calls(&mut state).await? {
                    return Ok(outcome);
                }
            }
            return self.run_loop(&mut state).await;
        }

        if checkpoint.status != RunStatus::Paused {
            return Err(EngineError::InvalidState(format!(
                "Run is in status '{}', only paused runs can be resumed",
                checkpoint.status.as_str()
            )));
        }

        let approval_id = checkpoint.pending_approval_id.ok_or_else(|| {
            EngineError::InvalidState("Paused run is missing pending_approval_id".to_string())
        })?;

        let approval = self
            .checkpoint_store
            .get_approval(&scope, approval_id)
            .await?
            .ok_or_else(|| {
                EngineError::InvalidState(format!("Approval {} not found", approval_id))
            })?;

        if approval.status == ApprovalStatus::Pending {
            return Ok(EngineOutcome::Paused {
                approval_id,
                tool_name: approval.tool_name,
                action_description: approval.action_description,
            });
        }

        if approval.status == ApprovalStatus::Denied {
            let state = AgentState {
                scope,
                status: RunStatus::Failed,
                sequence: checkpoint.sequence + 1,
                history: checkpoint.history,
                active_tool_call: None,
                pending_tool_calls: Vec::new(),
                pending_approval_id: None,
                author_id: checkpoint.author_id,
                model_override: checkpoint.model_override,
                final_response: None,
                error_message: Some("Approval denied by human operator".to_string()),
            };

            self.save_state_checkpoint(&state).await?;

            self.checkpoint_store
                .record_audit_log(&AuditLogEntry {
                    id: Uuid::new_v4(),
                    scope,
                    actor_id: approval.decided_by.clone(),
                    action: "approval.denied".to_string(),
                    target_id: Some(approval_id),
                    details: serde_json::json!({
                        "reason": "operator_denied",
                        "author_id": approval.author_id,
                        "approver_id": approval.decided_by,
                    }),
                    created_at: Utc::now(),
                })
                .await?;

            self.event_sink.emit(RuntimeEvent::new(
                scope,
                RuntimeEventType::ApprovalResolved,
                serde_json::json!({
                    "approval_id": approval_id.to_string(),
                    "decision": "deny"
                }),
            ))?;

            self.event_sink.emit(RuntimeEvent::new(
                scope,
                RuntimeEventType::RunFailed,
                serde_json::json!({ "error": "ApprovalDenied" }),
            ))?;

            return Ok(EngineOutcome::Failed(
                "Approval denied by human operator".to_string(),
            ));
        }

        if approval.status == ApprovalStatus::Consumed
            || approval.status == ApprovalStatus::Executing
        {
            return Err(EngineError::ApprovalAlreadyConsumed(approval_id));
        }

        if approval.status != ApprovalStatus::AllowedOnce {
            return Err(EngineError::InvalidState(format!(
                "Unexpected approval status: {:?}",
                approval.status
            )));
        }

        let tool_call = checkpoint.active_tool_call.ok_or_else(|| {
            EngineError::InvalidState("Paused checkpoint missing active_tool_call".to_string())
        })?;

        let executing_checkpoint = RunCheckpoint {
            id: Uuid::new_v4(),
            scope,
            sequence: checkpoint.sequence + 1,
            status: RunStatus::Executing,
            history: checkpoint.history.clone(),
            active_tool_call: Some(tool_call.clone()),
            pending_tool_calls: checkpoint.pending_tool_calls.clone(),
            pending_approval_id: Some(approval_id),
            author_id: checkpoint.author_id.clone(),
            model_override: checkpoint.model_override.clone(),
            created_at: Utc::now(),
        };

        let claimed = self
            .checkpoint_store
            .claim_approved_execution(&scope, approval_id, &executing_checkpoint)
            .await?;
        if !claimed {
            return Err(EngineError::ApprovalAlreadyConsumed(approval_id));
        }

        self.event_sink.emit(RuntimeEvent::new(
            scope,
            RuntimeEventType::ApprovalResolved,
            serde_json::json!({
                "approval_id": approval_id.to_string(),
                "decision": "allow_once"
            }),
        ))?;

        self.event_sink.emit(RuntimeEvent::new(
            scope,
            RuntimeEventType::ToolInvoked,
            serde_json::json!({
                "tool_name": tool_call.name,
                "tool_call_id": tool_call.id
            }),
        ))?;

        let execution_result = self
            .tool_registry
            .execute(&scope, &tool_call.name, &tool_call.arguments)
            .await;

        let completed_approval = self
            .checkpoint_store
            .complete_approval_execution(&scope, approval_id)
            .await?;
        if !completed_approval {
            return Err(EngineError::InvalidState(format!(
                "Approval {} left 'executing' state unexpectedly",
                approval_id
            )));
        }

        self.checkpoint_store
            .record_audit_log(&AuditLogEntry {
                id: Uuid::new_v4(),
                scope,
                actor_id: approval.decided_by.clone(),
                action: "approval.consumed".to_string(),
                target_id: Some(approval_id),
                details: serde_json::json!({
                    "decision": "allowed_once",
                    "author_id": approval.author_id,
                    "approver_id": approval.decided_by,
                }),
                created_at: Utc::now(),
            })
            .await?;

        let (result_content, is_error) = match execution_result {
            Ok(content) => (content, false),
            Err(e) => (format!("Tool execution error: {}", e), true),
        };

        self.event_sink.emit(RuntimeEvent::new(
            scope,
            RuntimeEventType::ToolCompleted,
            serde_json::json!({
                "tool_name": tool_call.name,
                "tool_call_id": tool_call.id,
                "is_error": is_error,
            }),
        ))?;

        let mut remaining_calls = checkpoint.pending_tool_calls;
        if remaining_calls
            .first()
            .is_some_and(|pending| pending.id == tool_call.id)
        {
            remaining_calls.remove(0);
        }

        let mut updated_history = checkpoint.history;
        updated_history.push(ChatMessage::tool(&tool_call.id, &result_content));
        updated_history = compact_history(&updated_history, &self.history_limits);

        let mut state = AgentState {
            scope,
            status: RunStatus::Running,
            sequence: checkpoint.sequence + 2,
            history: updated_history,
            active_tool_call: None,
            pending_tool_calls: remaining_calls,
            pending_approval_id: None,
            author_id: checkpoint.author_id,
            model_override: checkpoint.model_override,
            final_response: None,
            error_message: if is_error {
                Some(result_content.clone())
            } else {
                None
            },
        };

        self.save_state_checkpoint(&state).await?;

        if !state.pending_tool_calls.is_empty() {
            if let Some(outcome) = self.drain_pending_tool_calls(&mut state).await? {
                return Ok(outcome);
            }
        }

        self.run_loop(&mut state).await
    }

    async fn drain_pending_tool_calls(
        &self,
        state: &mut AgentState,
    ) -> Result<Option<EngineOutcome>, EngineError> {
        while let Some(tool_call) = state.pending_tool_calls.first().cloned() {
            if self.checkpoint_store.is_cancelled(&state.scope).await? {
                state.status = RunStatus::Cancelled;
                state.sequence += 1;
                self.save_state_checkpoint(state).await?;
                self.event_sink.emit(RuntimeEvent::new(
                    state.scope,
                    RuntimeEventType::RunFailed,
                    serde_json::json!({ "error": "RunCancelled" }),
                ))?;
                return Ok(Some(EngineOutcome::Cancelled));
            }

            if self.tool_registry.get(&tool_call.name).is_none() {
                state.history.push(ChatMessage::tool(
                    &tool_call.id,
                    format!("Error: tool '{}' is not registered", tool_call.name),
                ));
                state.pending_tool_calls.remove(0);
                state.history = compact_history(&state.history, &self.history_limits);
                state.sequence += 1;
                self.save_state_checkpoint(state).await?;
                continue;
            }

            if let Some(intent) = self
                .approval_policy
                .check_approval(&tool_call.name, &tool_call.arguments)
            {
                let approval = ApprovalRequest {
                    id: Uuid::new_v4(),
                    scope: state.scope,
                    tool_name: tool_call.name.clone(),
                    action_description: intent.description.clone(),
                    intent: intent.kind,
                    status: ApprovalStatus::Pending,
                    author_id: state.author_id.clone(),
                    created_at: Utc::now(),
                    decided_at: None,
                    decided_by: None,
                    consumed_at: None,
                };

                self.checkpoint_store.save_approval(&approval).await?;

                state.status = RunStatus::Paused;
                state.active_tool_call = Some(tool_call.clone());
                state.pending_approval_id = Some(approval.id);
                state.sequence += 1;

                self.save_state_checkpoint(state).await?;

                let mut req_payload = serde_json::json!({
                    "approval_id": approval.id.to_string(),
                    "tool_name": tool_call.name,
                    "action_description": intent.description,
                    "intent_kind": intent.kind.as_str(),
                    "author_id": state.author_id,
                });
                if let Some(obj) = req_payload.as_object_mut() {
                    if let Some(p) = tool_call.arguments.get("path").and_then(|v| v.as_str()) {
                        obj.insert("path".to_string(), serde_json::Value::String(p.to_string()));
                    }
                    if let Some(prog) = tool_call.arguments.get("program").and_then(|v| v.as_str())
                    {
                        obj.insert(
                            "program".to_string(),
                            serde_json::Value::String(prog.to_string()),
                        );
                    }
                    if let Some(args_val) = tool_call.arguments.get("args") {
                        obj.insert("args".to_string(), args_val.clone());
                    }
                }
                self.event_sink.emit(RuntimeEvent::new(
                    state.scope,
                    RuntimeEventType::ApprovalRequired,
                    req_payload,
                ))?;

                self.checkpoint_store
                    .record_audit_log(&AuditLogEntry {
                        id: Uuid::new_v4(),
                        scope: state.scope,
                        actor_id: state.author_id.clone(),
                        action: "approval.requested".to_string(),
                        target_id: Some(approval.id),
                        details: serde_json::json!({
                            "tool_name": tool_call.name,
                            "action_description": intent.description,
                            "author_id": state.author_id,
                        }),
                        created_at: Utc::now(),
                    })
                    .await?;

                return Ok(Some(EngineOutcome::Paused {
                    approval_id: approval.id,
                    tool_name: tool_call.name,
                    action_description: intent.description,
                }));
            }

            self.event_sink.emit(RuntimeEvent::new(
                state.scope,
                RuntimeEventType::ToolInvoked,
                serde_json::json!({
                    "tool_name": tool_call.name,
                    "tool_call_id": tool_call.id
                }),
            ))?;

            let execution_result = self
                .tool_registry
                .execute(&state.scope, &tool_call.name, &tool_call.arguments)
                .await;

            let (result_content, is_error) = match execution_result {
                Ok(content) => (content, false),
                Err(e) => (format!("Tool execution error: {}", e), true),
            };

            self.event_sink.emit(RuntimeEvent::new(
                state.scope,
                RuntimeEventType::ToolCompleted,
                serde_json::json!({
                    "tool_name": tool_call.name,
                    "tool_call_id": tool_call.id,
                    "is_error": is_error,
                }),
            ))?;

            state
                .history
                .push(ChatMessage::tool(&tool_call.id, &result_content));
            state.pending_tool_calls.remove(0);
            state.active_tool_call = None;
            state.history = compact_history(&state.history, &self.history_limits);
            state.sequence += 1;

            self.save_state_checkpoint(state).await?;
        }

        Ok(None)
    }

    async fn run_loop(&self, state: &mut AgentState) -> Result<EngineOutcome, EngineError> {
        let mut turn = 0;

        while turn < self.max_turns {
            turn += 1;

            if self.checkpoint_store.is_cancelled(&state.scope).await? {
                state.status = RunStatus::Cancelled;
                state.sequence += 1;
                self.save_state_checkpoint(state).await?;

                self.event_sink.emit(RuntimeEvent::new(
                    state.scope,
                    RuntimeEventType::RunFailed,
                    serde_json::json!({ "error": "RunCancelled" }),
                ))?;

                return Ok(EngineOutcome::Cancelled);
            }

            state.history = compact_history(&state.history, &self.history_limits);

            let model_req = ModelRequest {
                messages: state.history.clone(),
                tools: self.tool_registry.definitions(),
                temperature: Some(0.2),
                max_tokens: Some(4096),
                model_override: state.model_override.clone(),
            };

            let response = match self.model_client.complete(&model_req).await {
                Ok(resp) => resp,
                Err(e) => {
                    state.status = RunStatus::Failed;
                    state.error_message = Some(e.to_string());
                    state.sequence += 1;
                    self.save_state_checkpoint(state).await?;

                    self.event_sink.emit(RuntimeEvent::new(
                        state.scope,
                        RuntimeEventType::RunFailed,
                        serde_json::json!({ "error": e.to_string() }),
                    ))?;

                    return Ok(EngineOutcome::Failed(e.to_string()));
                }
            };

            if response.tool_calls.is_empty() {
                let content = response.content.unwrap_or_default().trim().to_string();
                if content.is_empty() {
                    let err = "Model returned empty response without tool calls".to_string();
                    state.status = RunStatus::Failed;
                    state.error_message = Some(err.clone());
                    state.sequence += 1;
                    self.save_state_checkpoint(state).await?;

                    self.event_sink.emit(RuntimeEvent::new(
                        state.scope,
                        RuntimeEventType::RunFailed,
                        serde_json::json!({ "error": err }),
                    ))?;

                    return Ok(EngineOutcome::Failed(err));
                }

                state.history.push(ChatMessage::assistant(&content));
                state.history = compact_history(&state.history, &self.history_limits);
                state.final_response = Some(content.clone());
                state.status = RunStatus::Completed;
                state.sequence += 1;

                self.save_state_checkpoint(state).await?;

                self.event_sink.emit(RuntimeEvent::new(
                    state.scope,
                    RuntimeEventType::RunCompleted,
                    serde_json::json!({ "result": content }),
                ))?;

                self.checkpoint_store
                    .record_audit_log(&AuditLogEntry {
                        id: Uuid::new_v4(),
                        scope: state.scope,
                        actor_id: state.author_id.clone(),
                        action: "run.completed".to_string(),
                        target_id: None,
                        details: serde_json::json!({ "final_response": content }),
                        created_at: Utc::now(),
                    })
                    .await?;

                return Ok(EngineOutcome::Completed(content));
            }

            state.history.push(ChatMessage::assistant_tool_calls(
                response.content.unwrap_or_default(),
                response.tool_calls.clone(),
            ));
            state.pending_tool_calls = response.tool_calls;

            if let Some(outcome) = self.drain_pending_tool_calls(state).await? {
                return Ok(outcome);
            }
        }

        let err = format!("Exceeded maximum turns limit ({})", self.max_turns);
        state.status = RunStatus::Failed;
        state.error_message = Some(err.clone());
        state.sequence += 1;
        self.save_state_checkpoint(state).await?;

        self.event_sink.emit(RuntimeEvent::new(
            state.scope,
            RuntimeEventType::RunFailed,
            serde_json::json!({ "error": "TurnLimitExceeded" }),
        ))?;

        Ok(EngineOutcome::Failed(err))
    }

    async fn save_state_checkpoint(&self, state: &AgentState) -> Result<(), EngineError> {
        let cp = RunCheckpoint {
            id: Uuid::new_v4(),
            scope: state.scope,
            sequence: state.sequence,
            status: state.status,
            history: state.history.clone(),
            active_tool_call: state.active_tool_call.clone(),
            pending_tool_calls: state.pending_tool_calls.clone(),
            pending_approval_id: state.pending_approval_id,
            author_id: state.author_id.clone(),
            model_override: state.model_override.clone(),
            created_at: Utc::now(),
        };
        self.checkpoint_store.save_checkpoint(&cp).await?;
        Ok(())
    }
}
